//! Background tasks pill: a small header pill that opens into the tasks running behind the scenes.
//!
//! Closed, the pill says how many tasks are running, with a spinner while
//! any are, and turns amber when one waits on you or red when one failed.
//! Open, a panel below it lists each task with its progress and a pause or
//! resume button. Opening, closing and every button go to the owner.
//!
//! Keyboard: the pill is a button; Escape closes the panel.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, div,
    percentage, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, lifted_shadow};

/// Where a background task is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundState {
    /// Working.
    Running,
    /// Held.
    Paused,
    /// Waiting on a person.
    Waiting,
    /// Stopped with a problem.
    Failed,
    /// Finished.
    Done,
}

impl BackgroundState {
    fn tone(self) -> Tone {
        match self {
            Self::Running => Tone::Info,
            Self::Paused => Tone::Neutral,
            Self::Waiting => Tone::Warning,
            Self::Failed => Tone::Danger,
            Self::Done => Tone::Success,
        }
    }
}

/// One task.
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundTask {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// Where it is.
    pub state: BackgroundState,
    /// How far along, from 0 to 1, if known.
    pub progress: Option<f32>,
    /// A short note, such as what it's on or why it stopped.
    pub note: SharedString,
}

impl BackgroundTask {
    /// Create a running task.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: BackgroundState::Running,
            progress: None,
            note: SharedString::default(),
        }
    }

    /// Set where it is, with a note.
    pub fn state(mut self, state: BackgroundState, note: impl Into<SharedString>) -> Self {
        self.state = state;
        self.note = note.into();
        self
    }

    /// Set how far along it is.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress.clamp(0., 1.));
        self
    }
}

/// The pill's words and tone for a set of tasks.
pub fn pill_summary(tasks: &[BackgroundTask]) -> (String, Tone) {
    let n = |s| tasks.iter().filter(|t| t.state == s).count();
    let (running, waiting, failed) = (
        n(BackgroundState::Running),
        n(BackgroundState::Waiting),
        n(BackgroundState::Failed),
    );
    let words = match (running, waiting, failed) {
        (0, 0, 0) if tasks.is_empty() => "No tasks".to_string(),
        (0, 0, 0) if tasks.len() == 1 => "1 task".to_string(),
        (0, 0, 0) => format!("{} tasks", tasks.len()),
        (r, w, f) => {
            let mut parts = Vec::new();
            if r > 0 {
                parts.push(format!("{r} running"));
            }
            if w > 0 {
                parts.push(format!("{w} waiting"));
            }
            if f > 0 {
                parts.push(format!("{f} failed"));
            }
            parts.join(", ")
        }
    };
    let tone = if failed > 0 {
        Tone::Danger
    } else if waiting > 0 {
        Tone::Warning
    } else if running > 0 {
        Tone::Info
    } else {
        Tone::Neutral
    };
    (words, tone)
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type TaskHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A pill that opens into the running tasks.
#[derive(IntoElement)]
pub struct BackgroundTasksPill {
    id: ElementId,
    style: StyleRefinement,
    tasks: Vec<BackgroundTask>,
    open: bool,
    on_toggle: Option<Handler>,
    on_pause: Option<TaskHandler>,
    on_open_task: Option<TaskHandler>,
}

impl BackgroundTasksPill {
    /// Create a pill for these tasks.
    pub fn new(id: impl Into<ElementId>, tasks: Vec<BackgroundTask>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            tasks,
            open: false,
            on_toggle: None,
            on_pause: None,
            on_open_task: None,
        }
    }

    /// Show the panel.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Run when the pill is clicked, or Escape closes the panel.
    pub fn on_toggle(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Run when a task's pause or resume button is clicked.
    pub fn on_pause(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pause = Some(Rc::new(handler));
        self
    }

    /// Run when a task's row is clicked.
    pub fn on_open_task(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_task = Some(Rc::new(handler));
        self
    }
}

impl Styled for BackgroundTasksPill {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for BackgroundTasksPill {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let (words, tone) = pill_summary(&self.tasks);
        let tone_color = tone.color(cx);
        let running = self
            .tasks
            .iter()
            .any(|t| t.state == BackgroundState::Running);

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let spinner = |key: &str| -> AnyElement {
            Icon::new(IconName::LoaderCircle)
                .xsmall()
                .text_color(Tone::Info.color(cx))
                .with_animation(
                    ElementId::NamedChild(self.id.clone().into(), format!("spin-{key}").into()),
                    Animation::new(std::time::Duration::from_secs(1)).repeat(),
                    |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                )
                .into_any_element()
        };

        let toggle = self.on_toggle.clone();
        let close = self.on_toggle.clone();
        let open = self.open;
        let pill = h_flex()
            .id(ElementId::NamedChild(self.id.clone().into(), "pill".into()))
            .track_focus(&focus)
            .gap_1p5()
            .h(px(28.))
            .px_2p5()
            .rounded_full()
            .border_1()
            .border_color(if tone == Tone::Neutral {
                line
            } else {
                tone_color.opacity(0.5)
            })
            .bg(if open { chip } else { fill })
            .hover(|style| style.bg(chip))
            .cursor_pointer()
            .text_xs()
            .text_color(ink)
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .role(Role::Button)
            .aria_label(format!("Background tasks, {words}"))
            .aria_expanded(open)
            .on_click(move |_, window, cx| {
                if let Some(toggle) = &toggle {
                    toggle(window, cx);
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if open && event.keystroke.key == "escape" {
                    if let Some(close) = &close {
                        close(window, cx);
                    }
                    cx.stop_propagation();
                }
            })
            .child(if running {
                spinner("pill")
            } else {
                div()
                    .size(px(7.))
                    .rounded_full()
                    .bg(tone_color)
                    .into_any_element()
            })
            .child(words)
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .text_color(quiet),
            );

        let panel = open.then(|| {
            let rows: Vec<_> =
                self.tasks
                    .iter()
                    .map(|task| {
                        let tone = task.state.tone();
                        let color = tone.color(cx);
                        let pause = self.on_pause.clone();
                        let open_task = self.on_open_task.clone();
                        let id = task.id.clone();
                        let open_id = task.id.clone();
                        let can_pause = matches!(
                            task.state,
                            BackgroundState::Running | BackgroundState::Paused
                        );
                        v_flex()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("task-{}", task.id).into(),
                            ))
                            .gap_1()
                            .px_3()
                            .py_2()
                            .rounded(radius)
                            .cursor_pointer()
                            .hover(|style| style.bg(chip.opacity(0.6)))
                            .role(Role::Button)
                            .aria_label(format!("{}, {}", task.label, task.note))
                            .on_click(move |_, window, cx| {
                                if let Some(open_task) = &open_task {
                                    open_task(&open_id, window, cx);
                                }
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(match task.state {
                                        BackgroundState::Running => spinner(&task.id),
                                        _ => div()
                                            .size(px(7.))
                                            .rounded_full()
                                            .bg(color)
                                            .into_any_element(),
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .font_medium()
                                            .child(task.label.clone()),
                                    )
                                    .when(can_pause, |this| {
                                        let paused = task.state == BackgroundState::Paused;
                                        this.child(
                                            Button::new(ElementId::NamedChild(
                                                self.id.clone().into(),
                                                format!("pause-{}", task.id).into(),
                                            ))
                                            .xsmall()
                                            .ghost()
                                            .icon(if paused {
                                                IconName::Play
                                            } else {
                                                IconName::Pause
                                            })
                                            .accessibility_label(format!(
                                                "{} {}",
                                                if paused { "Resume" } else { "Pause" },
                                                task.label
                                            ))
                                            .on_click(move |_, window, cx| {
                                                cx.stop_propagation();
                                                if let Some(pause) = &pause {
                                                    pause(&id, window, cx);
                                                }
                                            }),
                                        )
                                    }),
                            )
                            .when(!task.note.is_empty(), |this| {
                                this.child(
                                    div()
                                        .pl(px(15.))
                                        .text_xs()
                                        .text_color(
                                            if matches!(
                                                task.state,
                                                BackgroundState::Failed | BackgroundState::Waiting
                                            ) {
                                                color
                                            } else {
                                                quiet
                                            },
                                        )
                                        .child(task.note.clone()),
                                )
                            })
                            .children(task.progress.map(|share| {
                                div().ml(px(15.)).h(px(3.)).rounded_full().bg(chip).child(
                                    div().h_full().rounded_full().bg(color).w(relative(share)),
                                )
                            }))
                    })
                    .collect();
            v_flex()
                .absolute()
                .top(px(34.))
                .right_0()
                .w(px(320.))
                .max_w(px(320.))
                .p_1()
                .rounded(radius_lg)
                .border_1()
                .border_color(line)
                .bg(fill)
                .shadow(lifted_shadow(cx))
                .text_sm()
                .text_color(ink)
                .child(
                    div()
                        .px_3()
                        .pt_2()
                        .pb_1()
                        .text_xs()
                        .font_medium()
                        .text_color(quiet)
                        .child("Running in the background"),
                )
                .children(rows)
                .when(self.tasks.is_empty(), |this| {
                    this.child(
                        div()
                            .px_3()
                            .py_3()
                            .text_color(quiet)
                            .child("Nothing is running."),
                    )
                })
        });

        div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .child(pill)
            .children(panel)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_names_what_matters() {
        let tasks = vec![
            BackgroundTask::new("a", "A"),
            BackgroundTask::new("b", "B"),
            BackgroundTask::new("c", "C").state(BackgroundState::Waiting, "your approval"),
        ];
        assert_eq!(
            pill_summary(&tasks),
            ("2 running, 1 waiting".to_string(), Tone::Warning)
        );
        let done = vec![BackgroundTask::new("a", "A").state(BackgroundState::Done, "")];
        assert_eq!(pill_summary(&done), ("1 task".to_string(), Tone::Neutral));
        assert_eq!(pill_summary(&[]).0, "No tasks");
    }
}
