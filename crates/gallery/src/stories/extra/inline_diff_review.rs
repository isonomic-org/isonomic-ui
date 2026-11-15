//! Gallery story for the inline diff review.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::inline_diff_review::{
    InlineDiffReview, change_count, resolved_text, word_diff,
};

const BEFORE: &str = "Hi Dana, this is a reminder that invoice 2291 for 1,240.50 was due on the first of the month. Please pay within thirty days to avoid a late fee. Let us know if anything is wrong with the invoice.";
const AFTER: &str = "Hi Dana, a quick note that invoice 2291 for 1,240.50 fell due on 1 March. Please pay in 30 days to avoid a late fee. Reply here if anything on the invoice looks wrong.";

/// Shows an assistant's edits to a payment reminder. The story keeps the
/// decisions and shows the text they add up to.
pub struct InlineDiffReviewStory {
    decisions: Vec<Option<bool>>,
}

impl InlineDiffReviewStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        let count = change_count(&word_diff(BEFORE, AFTER));
        cx.new(|_| Self {
            decisions: vec![None; count],
        })
    }
}

impl Story for InlineDiffReviewStory {
    fn title() -> &'static str {
        "Inline diff review"
    }

    fn description() -> &'static str {
        "Word-level changes to a text, accepted or rejected one at a time."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for InlineDiffReviewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let all = story.clone();
        let result: SharedString = resolved_text(&word_diff(BEFORE, AFTER), &self.decisions).into();

        v_flex()
            .w_full()
            .max_w(px(560.))
            .gap_3()
            .child(
                InlineDiffReview::new("reminder-edits", BEFORE, AFTER)
                    .title("Edits to the payment reminder")
                    .decisions(self.decisions.clone())
                    .on_decide(move |ix, decision, _, cx| {
                        story.update(cx, |this, cx| {
                            if let Some(slot) = this.decisions.get_mut(ix) {
                                *slot = decision;
                            }
                            cx.notify();
                        })
                    })
                    .on_decide_all(move |accept, _, cx| {
                        all.update(cx, |this, cx| {
                            for slot in &mut this.decisions {
                                if slot.is_none() {
                                    *slot = Some(accept);
                                }
                            }
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("What would be sent:"),
            )
            .child(div().text_sm().child(result))
    }
}
