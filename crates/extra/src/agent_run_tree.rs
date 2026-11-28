// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Agent run tree: everything one run did, as a folding outline with a detail pane.
//!
//! Each row is a step: a thought, a tool call, a helper it started, a
//! message, or a problem. Steps that contain others fold. The selected
//! step's input and output show beside the outline. The owner keeps which
//! steps are open and which is selected.
//!
//! Keyboard: the outline is a Tab stop. Up and Down move, Right opens or
//! goes in, Left closes or goes out.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// What a step was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStepKind {
    /// The assistant working something out.
    Thought,
    /// A call to a tool.
    Tool,
    /// A helper it started.
    Helper,
    /// Something it said.
    Message,
}

impl RunStepKind {
    fn icon(self) -> IconName {
        match self {
            Self::Thought => IconName::Asterisk,
            Self::Tool => IconName::SquareTerminal,
            Self::Helper => IconName::Bot,
            Self::Message => IconName::FileText,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Thought => "Thought",
            Self::Tool => "Tool call",
            Self::Helper => "Helper",
            Self::Message => "Message",
        }
    }
}

/// How a step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStepOutcome {
    /// Still going.
    Running,
    /// Finished well.
    Ok,
    /// Finished with a problem.
    Failed,
    /// Never ran.
    Skipped,
}

/// One step, and the steps inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunStep {
    /// Identifies it.
    pub id: SharedString,
    /// What kind of step.
    pub kind: RunStepKind,
    /// A short label, such as a tool's name.
    pub label: SharedString,
    /// How it ended.
    pub outcome: RunStepOutcome,
    /// How long it took, in milliseconds.
    pub millis: u32,
    /// What went in, shown in the detail pane.
    pub input: Option<SharedString>,
    /// What came out, shown in the detail pane.
    pub output: Option<SharedString>,
    /// The steps inside it.
    pub children: Vec<RunStep>,
}

impl RunStep {
    /// Create a step that finished well and has nothing inside.
    pub fn new(
        id: impl Into<SharedString>,
        kind: RunStepKind,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
            outcome: RunStepOutcome::Ok,
            millis: 0,
            input: None,
            output: None,
            children: Vec::new(),
        }
    }

    /// Set how it ended and how long it took.
    pub fn took(mut self, outcome: RunStepOutcome, millis: u32) -> Self {
        self.outcome = outcome;
        self.millis = millis;
        self
    }

    /// Set what went in and came out.
    pub fn io(mut self, input: impl Into<SharedString>, output: impl Into<SharedString>) -> Self {
        self.input = Some(input.into());
        self.output = Some(output.into());
        self
    }

    /// Add a step inside it.
    pub fn child(mut self, step: RunStep) -> Self {
        self.children.push(step);
        self
    }
}

/// A visible row of the outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    /// The step's id.
    pub id: SharedString,
    /// How deep it sits.
    pub depth: usize,
    /// Its parent's id.
    pub parent: Option<SharedString>,
    /// Whether it has steps inside.
    pub has_children: bool,
}

/// The rows showing when `open` steps are unfolded.
pub fn visible_rows(steps: &[RunStep], open: &[SharedString]) -> Vec<RunRow> {
    fn walk(
        steps: &[RunStep],
        open: &[SharedString],
        depth: usize,
        parent: Option<&SharedString>,
        rows: &mut Vec<RunRow>,
    ) {
        for step in steps {
            rows.push(RunRow {
                id: step.id.clone(),
                depth,
                parent: parent.cloned(),
                has_children: !step.children.is_empty(),
            });
            if open.contains(&step.id) {
                walk(&step.children, open, depth + 1, Some(&step.id), rows);
            }
        }
    }
    let mut rows = Vec::new();
    walk(steps, open, 0, None, &mut rows);
    rows
}

/// Find a step anywhere in the tree.
pub fn find_step<'a>(steps: &'a [RunStep], id: &str) -> Option<&'a RunStep> {
    steps.iter().find_map(|step| {
        if step.id.as_ref() == id {
            Some(step)
        } else {
            find_step(&step.children, id)
        }
    })
}

/// Write a duration short: "840 ms", "2.4 s", "1 m 5 s".
pub fn duration_words(millis: u32) -> String {
    match millis {
        0..=999 => format!("{millis} ms"),
        1_000..=59_999 => format!("{:.1} s", millis as f32 / 1000.),
        _ => format!("{} m {} s", millis / 60_000, (millis % 60_000) / 1000),
    }
}

/// What a key press asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunTreeMove {
    /// Select this step.
    Select(SharedString),
    /// Open or close this step.
    Toggle(SharedString),
}

/// Where a key takes the outline from the selected row.
pub fn key_move(
    rows: &[RunRow],
    open: &[SharedString],
    selected: Option<&SharedString>,
    key: &str,
) -> Option<RunTreeMove> {
    let at = selected.and_then(|id| rows.iter().position(|r| &r.id == id));
    let Some(at) = at else {
        return rows.first().map(|r| RunTreeMove::Select(r.id.clone()));
    };
    let row = &rows[at];
    match key {
        "up" => at
            .checked_sub(1)
            .map(|ix| RunTreeMove::Select(rows[ix].id.clone())),
        "down" => rows.get(at + 1).map(|r| RunTreeMove::Select(r.id.clone())),
        "right" if row.has_children && !open.contains(&row.id) => {
            Some(RunTreeMove::Toggle(row.id.clone()))
        }
        "right" if row.has_children => rows.get(at + 1).map(|r| RunTreeMove::Select(r.id.clone())),
        "left" if row.has_children && open.contains(&row.id) => {
            Some(RunTreeMove::Toggle(row.id.clone()))
        }
        "left" => row.parent.clone().map(RunTreeMove::Select),
        _ => None,
    }
}

type StepHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A run's steps as a folding outline beside a detail pane.
#[derive(IntoElement)]
pub struct AgentRunTree {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    steps: Vec<RunStep>,
    open: Vec<SharedString>,
    selected: Option<SharedString>,
    on_select: Option<StepHandler>,
    on_toggle: Option<StepHandler>,
}

impl AgentRunTree {
    /// Create the outline for a run's top-level steps.
    pub fn new(id: impl Into<ElementId>, steps: Vec<RunStep>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Run".into(),
            steps,
            open: Vec::new(),
            selected: None,
            on_select: None,
            on_toggle: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The steps that are unfolded.
    pub fn open(mut self, ids: Vec<SharedString>) -> Self {
        self.open = ids;
        self
    }

    /// The selected step.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when a step is selected.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when a step is folded or unfolded.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Styled for AgentRunTree {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn outcome_tone(outcome: RunStepOutcome) -> Tone {
    match outcome {
        RunStepOutcome::Running => Tone::Info,
        RunStepOutcome::Ok => Tone::Success,
        RunStepOutcome::Failed => Tone::Danger,
        RunStepOutcome::Skipped => Tone::Neutral,
    }
}

fn count(steps: &[RunStep], tally: &mut [u32; 3], total: &mut u32) {
    for step in steps {
        *total += 1;
        match step.outcome {
            RunStepOutcome::Failed => tally[0] += 1,
            RunStepOutcome::Running => tally[1] += 1,
            RunStepOutcome::Ok => tally[2] += 1,
            RunStepOutcome::Skipped => {}
        }
        count(&step.children, tally, total);
    }
}

impl RenderOnce for AgentRunTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let rows = visible_rows(&self.steps, &self.open);
        let longest = rows
            .iter()
            .filter_map(|r| find_step(&self.steps, &r.id))
            .map(|s| s.millis)
            .max()
            .unwrap_or(1)
            .max(1);
        let mut tally = [0; 3];
        let mut total = 0;
        count(&self.steps, &mut tally, &mut total);
        let run_millis: u32 = self.steps.iter().map(|s| s.millis).sum();

        let key_down = {
            let rows = rows.clone();
            let open = self.open.clone();
            let selected = self.selected.clone();
            let select = self.on_select.clone();
            let toggle = self.on_toggle.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match key_move(
                    &rows,
                    &open,
                    selected.as_ref(),
                    event.keystroke.key.as_str(),
                ) {
                    Some(RunTreeMove::Select(id)) => {
                        if let Some(select) = &select {
                            select(&id, window, cx);
                        }
                        cx.stop_propagation();
                    }
                    Some(RunTreeMove::Toggle(id)) => {
                        if let Some(toggle) = &toggle {
                            toggle(&id, window, cx);
                        }
                        cx.stop_propagation();
                    }
                    None => {}
                }
            }
        };

        let row_views: Vec<_> = rows
            .iter()
            .filter_map(|row| find_step(&self.steps, &row.id).map(|step| (row, step)))
            .map(|(row, step)| {
                let is_selected = self.selected.as_ref() == Some(&step.id);
                let is_open = self.open.contains(&step.id);
                let tone_color = outcome_tone(step.outcome).color(cx);
                let select = self.on_select.clone();
                let toggle = self.on_toggle.clone();
                let id = step.id.clone();
                let toggle_id = step.id.clone();
                let share = step.millis as f32 / longest as f32;
                let named =
                    |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
                let (row_fade, row_on_hover) = isonomic_ui::component::look::tracked_hover(
                    named(format!("row-hover-{}", step.id)),
                    true,
                    window,
                    cx,
                );
                let (fold_fade, fold_on_hover) = isonomic_ui::component::look::tracked_hover(
                    named(format!("fold-hover-{}", step.id)),
                    row.has_children,
                    window,
                    cx,
                );
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-{}", step.id).into(),
                    ))
                    .h(px(30.))
                    .gap_1p5()
                    .pr_2()
                    .pl(px(8. + row.depth as f32 * 16.))
                    .rounded(radius)
                    .cursor_pointer()
                    .bg(if is_selected {
                        chip
                    } else {
                        crate::common::faded(chip.opacity(0.5), row_fade)
                    })
                    .on_hover(row_on_hover)
                    .role(Role::TreeItem)
                    .aria_label(format!(
                        "{}, {}, {}{}",
                        step.kind.word(),
                        step.label,
                        match step.outcome {
                            RunStepOutcome::Running => "running",
                            RunStepOutcome::Ok => "ok",
                            RunStepOutcome::Failed => "failed",
                            RunStepOutcome::Skipped => "skipped",
                        },
                        if step.millis > 0 {
                            format!(", {}", duration_words(step.millis))
                        } else {
                            String::new()
                        }
                    ))
                    .aria_selected(is_selected)
                    .when(row.has_children, |this| this.aria_expanded(is_open))
                    .on_click(move |_, window, cx| {
                        if let Some(select) = &select {
                            select(&id, window, cx);
                        }
                    })
                    .child(
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("fold-{}", step.id).into(),
                            ))
                            .size(px(16.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(row.has_children, |this| {
                                this.rounded(px(4.))
                                    .bg(crate::common::faded(line, fold_fade))
                                    .on_hover(fold_on_hover)
                                    .on_click(move |_, window, cx| {
                                        cx.stop_propagation();
                                        if let Some(toggle) = &toggle {
                                            toggle(&toggle_id, window, cx);
                                        }
                                    })
                                    .child(
                                        Icon::new(if is_open {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        })
                                        .xsmall()
                                        .text_color(quiet),
                                    )
                            }),
                    )
                    .child(Icon::new(step.kind.icon()).xsmall().text_color(
                        if step.outcome == RunStepOutcome::Failed {
                            tone_color
                        } else {
                            quiet
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(step.kind == RunStepKind::Tool, |this| {
                                this.font_family(mono.clone()).text_caption()
                            })
                            .when(step.outcome == RunStepOutcome::Skipped, |this| {
                                this.text_color(quiet).line_through()
                            })
                            .child(step.label.clone()),
                    )
                    .child(
                        div()
                            .w(px(56.))
                            .flex_none()
                            .h(px(4.))
                            .rounded_full()
                            .bg(chip)
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(tone_color.opacity(0.7))
                                    .w(relative(share)),
                            ),
                    )
                    .child(
                        div()
                            .w(px(54.))
                            .flex_none()
                            .text_right()
                            .text_caption()
                            .text_color(quiet)
                            .child(match step.outcome {
                                RunStepOutcome::Running => "running".to_string(),
                                RunStepOutcome::Skipped => "skipped".to_string(),
                                _ => duration_words(step.millis),
                            }),
                    )
            })
            .collect();

        let detail = self
            .selected
            .as_ref()
            .and_then(|id| find_step(&self.steps, id))
            .map(|step| {
                let tone = outcome_tone(step.outcome);
                let block = |label: &'static str, text: &SharedString| {
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_caption()
                                .font_medium()
                                .text_color(quiet)
                                .child(label),
                        )
                        .child(
                            div()
                                .p_2()
                                .rounded(radius)
                                .bg(chip)
                                .font_family(mono.clone())
                                .text_caption()
                                .line_height(relative(1.5))
                                .child(text.clone()),
                        )
                };
                v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(step.kind.icon()).small().text_color(quiet))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_medium()
                                    .child(step.label.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .text_caption()
                            .child(
                                div()
                                    .px_1p5()
                                    .rounded(px(4.))
                                    .bg(tone.tint(cx))
                                    .text_color(tone.color(cx))
                                    .child(match step.outcome {
                                        RunStepOutcome::Running => "Running",
                                        RunStepOutcome::Ok => "Finished",
                                        RunStepOutcome::Failed => "Failed",
                                        RunStepOutcome::Skipped => "Skipped",
                                    }),
                            )
                            .child(div().text_color(quiet).child(step.kind.word()))
                            .when(step.millis > 0, |this| {
                                this.child(
                                    div().text_color(quiet).child(duration_words(step.millis)),
                                )
                            })
                            .when(!step.children.is_empty(), |this| {
                                this.child(
                                    div()
                                        .text_color(quiet)
                                        .child(format!("{} inside", step.children.len())),
                                )
                            }),
                    )
                    .children(step.input.as_ref().map(|text| block("In", text)))
                    .children(step.output.as_ref().map(|text| {
                        block(
                            if step.outcome == RunStepOutcome::Failed {
                                "Problem"
                            } else {
                                "Out"
                            },
                            text,
                        )
                    }))
            });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(860.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .child(Icon::new(IconName::Network).small().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .child(self.title),
                    )
                    .child(div().text_caption().text_color(quiet).child(format!(
                        "{total} steps, {}{}{}",
                        duration_words(run_millis),
                        if tally[0] > 0 {
                            format!(", {} failed", tally[0])
                        } else {
                            String::new()
                        },
                        if tally[1] > 0 {
                            format!(", {} running", tally[1])
                        } else {
                            String::new()
                        },
                    ))),
            )
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                "outline".into(),
                            ))
                            .track_focus(&focus)
                            .on_key_down(key_down)
                            .role(Role::Tree)
                            .aria_label(format!("Steps, {} showing", rows.len()))
                            .flex_grow(3.)
                            .w(px(380.))
                            .max_w_full()
                            .p_1p5()
                            .gap_px()
                            .rounded(radius)
                            .when(is_focused, |this| this.focus_ring_style(window, cx))
                            .children(row_views),
                    )
                    .child(
                        div()
                            .flex_grow(2.)
                            .w(px(260.))
                            .max_w_full()
                            .min_h(px(160.))
                            .p_4()
                            .border_l_1()
                            .border_color(line)
                            .map(|this| match detail {
                                Some(detail) => this.child(detail),
                                None => this.child(
                                    div()
                                        .text_color(quiet)
                                        .child("Select a step to see what went in and out."),
                                ),
                            }),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<RunStep> {
        vec![
            RunStep::new("a", RunStepKind::Thought, "Plan"),
            RunStep::new("b", RunStepKind::Helper, "Matcher")
                .child(RunStep::new("b1", RunStepKind::Tool, "fetch"))
                .child(RunStep::new("b2", RunStepKind::Tool, "post")),
            RunStep::new("c", RunStepKind::Message, "Report"),
        ]
    }

    #[test]
    fn rows_follow_what_is_open() {
        let closed = visible_rows(&steps(), &[]);
        assert_eq!(closed.len(), 3);
        let open = visible_rows(&steps(), &["b".into()]);
        let ids: Vec<_> = open.iter().map(|r| r.id.to_string()).collect();
        assert_eq!(ids, ["a", "b", "b1", "b2", "c"]);
        assert_eq!(open[2].depth, 1);
        assert_eq!(open[2].parent.as_deref(), Some("b"));
    }

    #[test]
    fn arrows_open_close_and_climb() {
        let open: Vec<SharedString> = vec!["b".into()];
        let rows = visible_rows(&steps(), &open);
        let b: SharedString = "b".into();
        let b1: SharedString = "b1".into();
        assert_eq!(
            key_move(&rows, &open, Some(&b), "left"),
            Some(RunTreeMove::Toggle(b.clone()))
        );
        assert_eq!(
            key_move(&rows, &open, Some(&b), "right"),
            Some(RunTreeMove::Select(b1.clone()))
        );
        assert_eq!(
            key_move(&rows, &open, Some(&b1), "left"),
            Some(RunTreeMove::Select(b.clone()))
        );
        let closed = visible_rows(&steps(), &[]);
        assert_eq!(
            key_move(&closed, &[], Some(&b), "right"),
            Some(RunTreeMove::Toggle(b))
        );
    }

    #[test]
    fn durations_read_short() {
        assert_eq!(duration_words(840), "840 ms");
        assert_eq!(duration_words(2_400), "2.4 s");
        assert_eq!(duration_words(65_000), "1 m 5 s");
    }
}
