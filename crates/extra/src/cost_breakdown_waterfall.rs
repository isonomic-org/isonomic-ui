//! Cost breakdown waterfall: what each step of a run cost, stacked up to the total.
//!
//! Each row is a step, and its bar starts where the step before it ended,
//! so the bars walk down and across to the run's total at the foot. Steps
//! that ran helpers fold open into their own steps, which split the
//! parent's bar. A switch shows cost or tokens. Selecting a row names its
//! share of the whole.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One step's cost, and the steps inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct CostStep {
    /// Identifies it.
    pub id: SharedString,
    /// What it was.
    pub label: SharedString,
    /// What it cost on its own, in hundredths of a cent.
    pub cost: u32,
    /// Tokens it used on its own.
    pub tokens: u32,
    /// Steps inside it.
    pub children: Vec<CostStep>,
}

impl CostStep {
    /// Create a step with its own cost in hundredths of a cent and its tokens.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        cost: u32,
        tokens: u32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            cost,
            tokens,
            children: Vec::new(),
        }
    }

    /// Add a step inside it.
    pub fn child(mut self, step: CostStep) -> Self {
        self.children.push(step);
        self
    }

    /// Its total, itself and everything inside, in the chosen measure.
    pub fn total(&self, tokens: bool) -> u64 {
        (if tokens { self.tokens } else { self.cost }) as u64
            + self.children.iter().map(|c| c.total(tokens)).sum::<u64>()
    }
}

/// A row of the waterfall: where its bar starts and how long it is.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterfallRow {
    /// The step's id.
    pub id: SharedString,
    /// How deep it sits.
    pub depth: usize,
    /// Where its bar starts.
    pub start: u64,
    /// How long its bar is.
    pub value: u64,
    /// Whether it has steps inside.
    pub has_children: bool,
}

/// The rows showing when `open` steps are unfolded. An open step's bar
/// covers only its own share; its steps follow on from there.
pub fn waterfall(steps: &[CostStep], open: &[SharedString], tokens: bool) -> Vec<WaterfallRow> {
    fn walk(
        steps: &[CostStep],
        open: &[SharedString],
        tokens: bool,
        depth: usize,
        at: &mut u64,
        rows: &mut Vec<WaterfallRow>,
    ) {
        for step in steps {
            let is_open = open.contains(&step.id) && !step.children.is_empty();
            let value = if is_open {
                (if tokens { step.tokens } else { step.cost }) as u64
            } else {
                step.total(tokens)
            };
            rows.push(WaterfallRow {
                id: step.id.clone(),
                depth,
                start: *at,
                value,
                has_children: !step.children.is_empty(),
            });
            *at += value;
            if is_open {
                walk(&step.children, open, tokens, depth + 1, at, rows);
            }
        }
    }
    let mut rows = Vec::new();
    let mut at = 0;
    walk(steps, open, tokens, 0, &mut at, &mut rows);
    rows
}

/// Write a measure: hundredths of a cent as dollars, or tokens short.
pub fn measure_words(value: u64, tokens: bool) -> String {
    if tokens {
        match value {
            0..=999 => value.to_string(),
            1_000..=999_999 => format!("{:.1}k", value as f64 / 1000.),
            _ => format!("{:.2}M", value as f64 / 1_000_000.),
        }
    } else {
        let cents = value as f64 / 100.;
        if cents < 100. {
            format!("{cents:.1}\u{a2}")
        } else {
            format!("${:.2}", cents / 100.)
        }
    }
}

fn find<'a>(steps: &'a [CostStep], id: &str) -> Option<&'a CostStep> {
    steps.iter().find_map(|s| {
        if s.id.as_ref() == id {
            Some(s)
        } else {
            find(&s.children, id)
        }
    })
}

type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type MeasureHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A run's cost as a waterfall of steps.
#[derive(IntoElement)]
pub struct CostBreakdownWaterfall {
    id: ElementId,
    style: StyleRefinement,
    steps: Vec<CostStep>,
    tokens: bool,
    open: Vec<SharedString>,
    selected: Option<SharedString>,
    on_select: Option<IdHandler>,
    on_toggle: Option<IdHandler>,
    on_measure: Option<MeasureHandler>,
}

impl CostBreakdownWaterfall {
    /// Create a waterfall of a run's top-level steps.
    pub fn new(id: impl Into<ElementId>, steps: Vec<CostStep>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            steps,
            tokens: false,
            open: Vec::new(),
            selected: None,
            on_select: None,
            on_toggle: None,
            on_measure: None,
        }
    }

    /// Show tokens instead of cost.
    pub fn tokens(mut self, tokens: bool) -> Self {
        self.tokens = tokens;
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

    /// Run when a row is clicked.
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

    /// Run when the measure switch is clicked, with `true` for tokens.
    pub fn on_measure(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_measure = Some(Rc::new(handler));
        self
    }
}

impl Styled for CostBreakdownWaterfall {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for CostBreakdownWaterfall {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let tokens = self.tokens;

        let rows = waterfall(&self.steps, &self.open, tokens);
        let total: u64 = self.steps.iter().map(|s| s.total(tokens)).sum();
        let scale = total.max(1) as f32;
        let biggest = rows.iter().map(|r| r.value).max().unwrap_or(0);

        let row_views: Vec<_> = rows
            .iter()
            .filter_map(|row| find(&self.steps, &row.id).map(|step| (row, step)))
            .map(|(row, step)| {
                let is_selected = self.selected.as_ref() == Some(&row.id);
                let is_open = self.open.contains(&row.id);
                let select = self.on_select.clone();
                let toggle = self.on_toggle.clone();
                let (id, toggle_id) = (row.id.clone(), row.id.clone());
                let heavy = row.value == biggest && biggest > 0;
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-{}", row.id).into(),
                    ))
                    .h(px(28.))
                    .gap_2()
                    .px_2()
                    .rounded(radius)
                    .cursor_pointer()
                    .when(is_selected, |this| this.bg(chip))
                    .hover(|style| style.bg(chip.opacity(0.6)))
                    .role(Role::Button)
                    .aria_label(format!(
                        "{}, {}",
                        step.label,
                        measure_words(row.value, tokens)
                    ))
                    .aria_selected(is_selected)
                    .when(row.has_children, |this| this.aria_expanded(is_open))
                    .on_click(move |_, window, cx| {
                        if let Some(select) = &select {
                            select(&id, window, cx);
                        }
                    })
                    .child(
                        h_flex()
                            .w(px(190.))
                            .flex_none()
                            .gap_1()
                            .pl(px(row.depth as f32 * 14.))
                            .child(
                                div()
                                    .id(ElementId::NamedChild(
                                        self.id.clone().into(),
                                        format!("fold-{}", row.id).into(),
                                    ))
                                    .size(px(16.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(row.has_children, |this| {
                                        this.role(Role::Button)
                                            .aria_label(format!(
                                                "{} {}",
                                                if is_open { "Fold" } else { "Unfold" },
                                                step.label
                                            ))
                                            .aria_expanded(is_open)
                                            .rounded(px(4.))
                                            .hover(|s| s.bg(line))
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
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .when(heavy, |this| this.font_medium())
                                    .child(step.label.clone()),
                            ),
                    )
                    .child(
                        div().relative().flex_1().h(px(12.)).child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(relative(row.start as f32 / scale))
                                .w(relative((row.value as f32 / scale).max(0.004)))
                                .rounded(px(2.))
                                .bg(if is_selected || heavy {
                                    accent
                                } else {
                                    accent.opacity(0.55)
                                }),
                        ),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_right()
                            .text_xs()
                            .text_color(if heavy { ink } else { quiet })
                            .child(measure_words(row.value, tokens)),
                    )
            })
            .collect();

        let switch = |label: &'static str, want: bool| {
            let on_measure = self.on_measure.clone();
            let active = tokens == want;
            div()
                .id(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("measure-{label}").into(),
                ))
                .px_2()
                .py_0p5()
                .rounded(px(5.))
                .cursor_pointer()
                .text_xs()
                .when(active, |this| {
                    this.bg(fill).shadow(resting_shadow(cx)).font_medium()
                })
                .when(!active, |this| this.text_color(quiet))
                .role(Role::RadioButton)
                .aria_label(format!("Measure in {}", label.to_lowercase()))
                .aria_selected(active)
                .on_click(move |_, window, cx| {
                    if let Some(on_measure) = &on_measure {
                        on_measure(want, window, cx);
                    }
                })
                .child(label)
        };

        let detail = self
            .selected
            .as_ref()
            .and_then(|id| find(&self.steps, id))
            .map(|step| {
                let value = step.total(tokens);
                format!(
                    "{}: {} with everything inside, {:.0}% of the run",
                    step.label,
                    measure_words(value, tokens),
                    100. * value as f64 / total.max(1) as f64
                )
            })
            .unwrap_or_else(|| "Select a row to see its share of the run.".into());

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(760.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .font_medium()
                            .child(format!("Run total {}", measure_words(total, tokens))),
                    )
                    .child(
                        h_flex()
                            .p_0p5()
                            .rounded(px(7.))
                            .bg(chip)
                            .child(switch("Cost", false))
                            .child(switch("Tokens", true)),
                    ),
            )
            .child(v_flex().p_1p5().gap_px().children(row_views))
            .child(
                h_flex()
                    .gap_2()
                    .mx_1p5()
                    .px_2()
                    .h(px(30.))
                    .border_t_1()
                    .border_color(line)
                    .child(div().w(px(190.)).flex_none().font_semibold().child("Total"))
                    .child(
                        div()
                            .flex_1()
                            .h(px(12.))
                            .rounded(px(2.))
                            .bg(ink.opacity(0.7)),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_right()
                            .text_xs()
                            .font_semibold()
                            .child(measure_words(total, tokens)),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
                    .child(detail),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Vec<CostStep> {
        vec![
            CostStep::new("a", "Plan", 100, 1_000),
            CostStep::new("b", "Match", 50, 500)
                .child(CostStep::new("b1", "Cards", 300, 3_000))
                .child(CostStep::new("b2", "Bank", 200, 2_000)),
            CostStep::new("c", "Report", 80, 800),
        ]
    }

    #[test]
    fn folded_steps_carry_their_whole_cost() {
        let rows = waterfall(&run(), &[], false);
        let spans: Vec<_> = rows.iter().map(|r| (r.start, r.value)).collect();
        assert_eq!(spans, [(0, 100), (100, 550), (650, 80)]);
    }

    #[test]
    fn open_steps_split_into_their_own() {
        let rows = waterfall(&run(), &["b".into()], true);
        let spans: Vec<_> = rows
            .iter()
            .map(|r| (r.id.to_string(), r.start, r.value))
            .collect();
        assert_eq!(
            spans,
            [
                ("a".into(), 0, 1_000),
                ("b".into(), 1_000, 500),
                ("b1".into(), 1_500, 3_000),
                ("b2".into(), 4_500, 2_000),
                ("c".into(), 6_500, 800)
            ]
        );
    }

    #[test]
    fn measures_read_naturally() {
        assert_eq!(measure_words(4_250, false), "42.5\u{a2}");
        assert_eq!(measure_words(61_000, false), "$6.10");
        assert_eq!(measure_words(12_400, true), "12.4k");
    }
}
