//! Gallery story for the phase loader.

use crate::Story;
use isonomic_ui::component::{Sizable as _, button::Button, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::phase_loader::{PhaseLoader, PhaseLoaderVariant};

const PHASES: [&str; 4] = [
    "Planning",
    "Checking 9 invoices",
    "Matching payments",
    "Writing summary",
];

/// Shows a loader on a timer, one driven step by step, and the plain compact variant.
pub struct PhaseLoaderStory {
    done: bool,
    stage: usize,
}

impl PhaseLoaderStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            done: false,
            stage: 0,
        })
    }
}

impl Story for PhaseLoaderStory {
    fn title() -> &'static str {
        "Phase loader"
    }

    fn description() -> &'static str {
        "A small pill that says what is being worked on right now."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PhaseLoaderStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .items_start()
            .child(PhaseLoader::new("timed").phases(PHASES).done(self.done))
            .child(
                PhaseLoader::new("driven")
                    .phases(PHASES)
                    .phase(self.stage)
                    .done(self.done)
                    .done_label("Summary ready"),
            )
            .child(
                PhaseLoader::new("plain")
                    .small()
                    .variant(PhaseLoaderVariant::Plain)
                    .phases(["Saving"])
                    .done(self.done)
                    .done_label("Saved"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("next")
                            .outline()
                            .small()
                            .label("Next phase")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stage = (this.stage + 1) % PHASES.len();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle")
                            .outline()
                            .small()
                            .label(if self.done { "Start over" } else { "Finish" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.done = !this.done;
                                this.stage = 0;
                                cx.notify();
                            })),
                    ),
            )
    }
}
