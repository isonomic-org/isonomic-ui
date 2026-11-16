//! Gallery story for the run status bar.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div, px,
};
use isonomic_ui_extra::prompt_composer::PromptComposer;
use isonomic_ui_extra::run_status_bar::{RunStatus, RunStatusBar};

/// A chat with a local model: the bar in each of its five states, the two
/// actions at work, the bar where it lives above the composer, and the bar
/// in a narrow column with a long reason.
pub struct RunStatusBarStory {
    composer: Entity<PromptComposer>,
    /// The state whose action is working, for a moment after it is pressed.
    busy: Option<RunStatus>,
    last: SharedString,
    _work: Option<Task<()>>,
}

impl RunStatusBarStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer = cx.new(|cx| PromptComposer::new("Message\u{2026}", window, cx));
            composer.update(cx, |composer, cx| {
                composer.set_attach(false, cx);
                composer.set_dictation(false, cx);
            });
            Self {
                composer,
                busy: None,
                last: "Nothing pressed yet".into(),
                _work: None,
            }
        })
    }

    /// The action shows as working for a moment, then says what it did.
    fn act(&mut self, status: RunStatus, cx: &mut Context<Self>) {
        self.busy = Some(status);
        self.last = match status {
            RunStatus::CutShort => "Continuing\u{2026}".into(),
            _ => "Trying again\u{2026}".into(),
        };
        cx.notify();
        self._work = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            this.update(cx, |this, cx| {
                this.busy = None;
                this.last = match status {
                    RunStatus::CutShort => "Continued".into(),
                    _ => "Tried again".into(),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    fn bar(&self, name: &'static str, status: RunStatus, cx: &mut Context<Self>) -> RunStatusBar {
        RunStatusBar::new(name, status)
            .action_busy(self.busy == Some(status))
            .on_action(cx.listener(|this, status: &RunStatus, _, cx| this.act(*status, cx)))
    }
}

impl Story for RunStatusBarStory {
    fn title() -> &'static str {
        "Run status bar"
    }

    fn description() -> &'static str {
        "The bar above the composer that says how the last run ended, with the one thing to do about it."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RunStatusBarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let heading = |text: &'static str| div().text_xs().text_color(quiet).child(text);

        v_flex()
            .w_full()
            .max_w(px(640.))
            .gap_6()
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Each state"))
                    .child(self.bar("stopped", RunStatus::Stopped, cx))
                    .child(self.bar("cut-short", RunStatus::CutShort, cx))
                    .child(
                        self.bar("failed", RunStatus::Failed, cx)
                            .detail("The model ran out of memory."),
                    )
                    .child(self.bar("reconnecting", RunStatus::Reconnecting, cx))
                    .child(self.bar("closed", RunStatus::Closed, cx))
                    .child(div().text_xs().text_color(quiet).child(self.last.clone())),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Above the composer"))
                    .child(
                        self.bar("above-composer", RunStatus::CutShort, cx)
                            .message("The answer stopped at the length limit."),
                    )
                    .child(self.composer.clone()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .w_full()
                    .max_w(px(300.))
                    .child(heading("In a narrow column, with a long reason"))
                    .child(
                        self.bar("narrow", RunStatus::Failed, cx)
                            .message("The model could not be loaded.")
                            .detail(
                                "There is not enough free memory for this model. Close other \
                                 apps or pick a smaller model, then try again.",
                            ),
                    ),
            )
    }
}
