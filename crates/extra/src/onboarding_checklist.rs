//! Onboarding checklist: a short list of first steps with a progress bar.
//!
//! Steps can be ticked in any order. The bar and the count follow, and an
//! optional auto play ticks the open steps one by one for demos, stopping as
//! soon as someone clicks or types.

use std::{rc::Rc, time::Duration};

use isonomic_ui::base::Checkbox;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    progress::Progress, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    RenderOnce, SharedString, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

const DEFAULT_INTERVAL: Duration = Duration::from_millis(2200);

/// One step of the checklist.
#[derive(Clone)]
pub struct ChecklistStep {
    id: SharedString,
    label: SharedString,
    done: bool,
    icon: Option<Icon>,
}

impl ChecklistStep {
    /// Create an open step.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            done: false,
            icon: None,
        }
    }

    /// Whether the step starts ticked.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    /// A small icon at the end of the row, such as the tool the step belongs to.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

type ChangeHandler = Rc<dyn Fn(&[(SharedString, bool)], &mut Window, &mut App)>;

/// A card with a title, a progress bar and steps to tick.
#[derive(IntoElement)]
pub struct OnboardingChecklist {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    steps: Vec<ChecklistStep>,
    auto_play: bool,
    interval: Duration,
    on_change: Option<ChangeHandler>,
}

impl OnboardingChecklist {
    /// Create an empty checklist titled "Getting set up".
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Getting set up".into(),
            steps: Vec::new(),
            auto_play: false,
            interval: DEFAULT_INTERVAL,
            on_change: None,
        }
    }

    /// Set the heading above the bar.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Add one step.
    pub fn step(mut self, step: ChecklistStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Add several steps.
    pub fn steps(mut self, steps: impl IntoIterator<Item = ChecklistStep>) -> Self {
        self.steps.extend(steps);
        self
    }

    /// Tick the open steps one after another, then start over. Any click or
    /// key press on the card stops it for good.
    pub fn auto_play(mut self, auto_play: bool) -> Self {
        self.auto_play = auto_play;
        self
    }

    /// The pause between two automatic ticks.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Run after a step was ticked or unticked, with every step's id and state.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[(SharedString, bool)], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for OnboardingChecklist {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// What the checklist remembers between frames.
#[derive(Default)]
struct ChecklistState {
    /// Ids of the ticked steps.
    done: Vec<SharedString>,
    /// Ids already seen, so a step added later brings its own starting state.
    seen: Vec<SharedString>,
    stopped: bool,
    /// Counts changes to `done`. The auto play timer is restarted for each one.
    changes: usize,
    timer_for: Option<usize>,
    timer: Option<Task<()>>,
}

impl ChecklistState {
    fn sync(&mut self, steps: &[ChecklistStep]) {
        for step in steps {
            if !self.seen.contains(&step.id) {
                self.seen.push(step.id.clone());
                if step.done {
                    self.done.push(step.id.clone());
                }
            }
        }
    }

    fn toggle(&mut self, id: &SharedString) {
        match self.done.iter().position(|done| done == id) {
            Some(ix) => {
                self.done.remove(ix);
            }
            None => self.done.push(id.clone()),
        }
        self.changes += 1;
    }
}

/// One automatic tick: the first open step is ticked, or, with nothing left
/// open, the list goes back to how it started.
fn advance(done: &mut Vec<SharedString>, steps: &[(SharedString, bool)]) {
    match steps.iter().find(|(id, _)| !done.contains(id)) {
        Some((id, _)) => done.push(id.clone()),
        None => {
            *done = steps
                .iter()
                .filter(|(_, starts_done)| *starts_done)
                .map(|(id, _)| id.clone())
                .collect();
        }
    }
}

impl RenderOnce for OnboardingChecklist {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let accent = theme.info;
        let on_accent = theme.info_foreground;
        let wash = theme.foreground.opacity(0.04);
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ChecklistState::default());
        let starts: Vec<(SharedString, bool)> = self
            .steps
            .iter()
            .map(|step| (step.id.clone(), step.done))
            .collect();
        let interval = self.interval;
        let auto_play = self.auto_play;
        let done = state.update(cx, |state, cx| {
            state.sync(&self.steps);
            let playing = auto_play && !state.stopped;
            if !playing {
                state.timer = None;
                state.timer_for = None;
            } else if state.timer_for != Some(state.changes) {
                // One timer per change: it ticks once and the next render
                // starts the following one. A full list rests twice as long.
                let all_done = starts.iter().all(|(id, _)| state.done.contains(id));
                let wait = if all_done { interval * 2 } else { interval };
                let starts = starts.clone();
                state.timer_for = Some(state.changes);
                state.timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    _ = this.update(cx, |state, cx| {
                        advance(&mut state.done, &starts);
                        state.changes += 1;
                        cx.notify();
                    });
                }));
            }
            state.done.clone()
        });

        let total = self.steps.len();
        let count = self
            .steps
            .iter()
            .filter(|step| done.contains(&step.id))
            .count();
        let percent = if total == 0 {
            0.
        } else {
            count as f32 * 100. / total as f32
        };
        let on_change = self.on_change;
        let list_id = self.id.clone();

        let rows = self.steps.into_iter().map(|step| {
            let checked = done.contains(&step.id);
            let row_id = ElementId::NamedChild(list_id.clone().into(), step.id.clone());
            let focus_handle = window
                .use_keyed_state((row_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone();
            let is_focused = focus_handle.is_focused(window);
            let state = state.clone();
            let on_change = on_change.clone();
            let starts = starts.clone();
            let id = step.id.clone();

            Checkbox::new(row_id)
                .checked(checked)
                .accessibility_label(step.label.clone())
                .track_focus(&focus_handle)
                .on_change(move |_, _, window, cx| {
                    let done = state.update(cx, |state, cx| {
                        state.toggle(&id);
                        cx.notify();
                        state.done.clone()
                    });
                    if let Some(on_change) = &on_change {
                        let steps: Vec<(SharedString, bool)> = starts
                            .iter()
                            .map(|(id, _)| (id.clone(), done.contains(id)))
                            .collect();
                        on_change(&steps, window, cx);
                    }
                })
                .h_flex()
                .gap_2p5()
                .w_full()
                .h(px(32.))
                .px_1()
                .rounded(radius)
                .cursor_pointer()
                .hover(|style| style.bg(wash))
                .when(is_focused, |this| this.focus_ring_style(window, cx))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(18.))
                        .rounded_full_style(cx)
                        .border_1()
                        .map(|this| {
                            if checked {
                                this.border_color(accent)
                                    .bg(accent)
                                    .text_color(on_accent)
                                    .child(Icon::new(IconName::Check).size_3())
                            } else {
                                this.border_color(line)
                            }
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(checked, |this| this.text_color(quiet).line_through())
                        .child(step.label),
                )
                .children(
                    step.icon
                        .map(|icon| icon.small().flex_none().text_color(quiet)),
                )
        });
        let rows: Vec<_> = rows.collect();

        let stop = {
            let state = state.clone();
            move |cx: &mut App| {
                state.update(cx, |state, _| state.stopped = true);
            }
        };
        let stop_on_key = stop.clone();

        v_flex()
            .id(self.id)
            .w_full()
            .max_w(px(380.))
            .gap_3()
            .p_4()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| stop(cx))
            .on_key_down(move |_, _, cx| stop_on_key(cx))
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_base()
                            .font_medium()
                            .child(self.title.clone()),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .text_color(quiet)
                            .child(div().text_color(ink).child(count.to_string()))
                            .child(format!("/{total}")),
                    ),
            )
            .child(
                Progress::new("progress")
                    .value(percent)
                    .accessibility_label(self.title),
            )
            .child(v_flex().children(rows))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<(SharedString, bool)> {
        vec![("a".into(), true), ("b".into(), false), ("c".into(), false)]
    }

    #[test]
    fn auto_play_ticks_the_first_open_step() {
        let mut done: Vec<SharedString> = vec!["a".into()];
        advance(&mut done, &steps());
        assert_eq!(done, ["a", "b"]);
        advance(&mut done, &steps());
        assert_eq!(done, ["a", "b", "c"]);
    }

    #[test]
    fn auto_play_starts_over_when_everything_is_ticked() {
        let mut done: Vec<SharedString> = vec!["a".into(), "b".into(), "c".into()];
        advance(&mut done, &steps());
        assert_eq!(done, ["a"]);
    }

    #[test]
    fn toggling_ticks_and_unticks() {
        let mut state = ChecklistState::default();
        state.toggle(&"a".into());
        assert_eq!(state.done, ["a"]);
        state.toggle(&"a".into());
        assert!(state.done.is_empty());
        assert_eq!(state.changes, 2);
    }

    #[test]
    fn a_step_added_later_brings_its_own_state() {
        let mut state = ChecklistState::default();
        state.sync(&[ChecklistStep::new("a", "A")]);
        state.toggle(&"a".into());
        state.sync(&[
            ChecklistStep::new("a", "A"),
            ChecklistStep::new("b", "B").done(true),
        ]);
        assert_eq!(state.done, ["a", "b"]);
    }
}
