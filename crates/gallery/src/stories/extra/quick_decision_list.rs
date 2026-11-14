//! Gallery story for the quick decision list.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::quick_decision_list::{DecisionAnswer, QuickDecision, QuickDecisionList};

/// Shows two open decisions and a folded one, and reports the last answer.
pub struct QuickDecisionListStory {
    last: SharedString,
}

impl QuickDecisionListStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "No answer yet".into(),
        })
    }
}

impl Render for QuickDecisionListStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        v_flex()
            .gap_3()
            .child(
                QuickDecisionList::new("decisions")
                    .decision(
                        QuickDecision::new("refund", "Refund order 4417 in full?")
                            .detail("Arrived damaged · 86.40 · customer since 2021")
                            .answers("Refund", "Offer credit"),
                    )
                    .decision(
                        QuickDecision::new("invite", "Let Dana invite outside reviewers?")
                            .detail("Applies to the Contracts workspace only"),
                    )
                    .decision(
                        QuickDecision::new("archive", "Archive 14 projects idle for a year?")
                            .answers("Archive", "Keep")
                            .collapsed(true),
                    )
                    .on_decide(move |id, answer, _, cx| {
                        let picked = match answer {
                            DecisionAnswer::Primary => "main answer",
                            DecisionAnswer::Secondary => "other answer",
                        };
                        let line = format!("{id}: {picked}");
                        view.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
