//! Gallery story for the thinking indicator.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px};
use isonomic_ui_extra::thinking_indicator::{ActivityOrb, OrbState, ThinkingIndicator};

/// A bookkeeping assistant working on a month-end question, through each
/// kind of work, and every orb side by side.
pub struct ThinkingIndicatorStory;

impl ThinkingIndicatorStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for ThinkingIndicatorStory {
    fn title() -> &'static str {
        "Thinking indicator"
    }

    fn description() -> &'static str {
        "The row that shows an assistant is working before it answers, with an orb for each kind of work."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

const STATES: [(OrbState, &str, &str); 6] = [
    (OrbState::Reasoning, "Reasoning", "Working out what changed since February\u{2026}"),
    (OrbState::Searching, "Searching", "Searching 3 bank statements\u{2026}"),
    (OrbState::Using, "Using", "Running the reconciliation\u{2026}"),
    (OrbState::Waiting, "Waiting", "Waiting for your approval"),
    (OrbState::Retrying, "Retrying", "The bank didn't answer. Trying again\u{2026}"),
    (OrbState::Compacting, "Compacting", "Tidying the conversation\u{2026}"),
];

impl Render for ThinkingIndicatorStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .gap_6()
            .child(
                v_flex().gap_3().children(STATES.iter().enumerate().map(|(ix, (state, _, label))| {
                    ThinkingIndicator::new(("thinking", ix), *state).label(*label)
                })),
            )
            .child(div().text_xs().text_color(quiet).child("Orbs on their own, at 20 px and 32 px"))
            .child(h_flex().gap_6().children(STATES.iter().enumerate().map(|(ix, (state, word, _))| {
                v_flex()
                    .items_center()
                    .gap_1()
                    .child(h_flex().gap_2().child(ActivityOrb::new(("orb", ix), *state)).child(ActivityOrb::new(("orb-big", ix), *state).size(32.)))
                    .child(div().w(px(80.)).text_center().text_xs().text_color(quiet).child(*word))
            })))
    }
}
