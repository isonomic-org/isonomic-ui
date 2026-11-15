//! Gallery story for the background tasks pill.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::background_tasks_pill::{BackgroundState, BackgroundTask, BackgroundTasksPill};

/// A header with the pill at its right, over four bookkeeping tasks. The
/// story keeps the panel open or closed and pauses tasks.
pub struct BackgroundTasksPillStory {
    tasks: Vec<BackgroundTask>,
    open: bool,
    picked: Option<String>,
}

fn tasks() -> Vec<BackgroundTask> {
    vec![
        BackgroundTask::new("recon", "Reconcile March")
            .state(BackgroundState::Running, "Matching card payouts")
            .progress(0.62),
        BackgroundTask::new("receipts", "File receipts from the inbox")
            .state(BackgroundState::Running, "18 of 43 filed")
            .progress(0.42),
        BackgroundTask::new("payables", "Friday's supplier run")
            .state(BackgroundState::Waiting, "Waiting for your approval")
            .progress(0.8),
        BackgroundTask::new("export", "Export the year-end packet")
            .state(BackgroundState::Paused, "Paused by you")
            .progress(0.2),
    ]
}

impl BackgroundTasksPillStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            tasks: tasks(),
            open: true,
            picked: None,
        })
    }
}

impl Story for BackgroundTasksPillStory {
    fn title() -> &'static str {
        "Background tasks pill"
    }

    fn description() -> &'static str {
        "A small header pill that opens into the tasks running behind the scenes."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for BackgroundTasksPillStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = cx.entity();
        let pause = cx.entity();
        let pick = cx.entity();
        let theme = cx.theme();
        let (line, quiet) = (theme.border, theme.muted_foreground);
        v_flex()
            .w_full()
            .min_h(px(320.))
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(line)
                    .child(div().flex_1().font_medium().child("Bookkeeping"))
                    .child(
                        BackgroundTasksPill::new("header-tasks", self.tasks.clone())
                            .open(self.open)
                            .on_toggle(move |_, cx| {
                                toggle.update(cx, |this, cx| {
                                    this.open = !this.open;
                                    cx.notify();
                                })
                            })
                            .on_pause(move |id, _, cx| {
                                pause.update(cx, |this, cx| {
                                    if let Some(task) = this.tasks.iter_mut().find(|t| &t.id == id) {
                                        task.state = match task.state {
                                            BackgroundState::Paused => BackgroundState::Running,
                                            _ => BackgroundState::Paused,
                                        };
                                        task.note = match task.state {
                                            BackgroundState::Paused => "Paused by you".into(),
                                            _ => "Carrying on".into(),
                                        };
                                    }
                                    cx.notify();
                                })
                            })
                            .on_open_task(move |id, _, cx| {
                                pick.update(cx, |this, cx| {
                                    this.picked = Some(id.to_string());
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(div().px_3().text_xs().text_color(quiet).child(match &self.picked {
                Some(id) => format!("Opened task: {id}"),
                None => "Click the pill to open or close the list.".into(),
            }))
    }
}
