//! Gallery story for the recommendation card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::recommendation_card::{
    Confidence, RecommendationCard, RecommendationOption,
};

/// Shows a recommendation with two alternatives and reports accept and undo.
pub struct RecommendationCardStory {
    last: SharedString,
}

impl RecommendationCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing accepted yet".into(),
        })
    }
}

impl Story for RecommendationCardStory {
    fn title() -> &'static str {
        "Recommendation card"
    }

    fn description() -> &'static str {
        "One suggested option with the reason for it, how sure"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RecommendationCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accepted = cx.entity();
        let undone = accepted.clone();

        v_flex()
            .gap_3()
            .child(
                RecommendationCard::new("backup")
                    .title("Backup schedule")
                    .option(
                        RecommendationOption::new("nightly", "Nightly backup")
                            .reason(
                                "Most edits land between 9 and 6, so a 2 am run misses nothing.",
                            )
                            .confidence(Confidence::High),
                    )
                    .option(
                        RecommendationOption::new("hourly", "Hourly backup")
                            .reason("Safer, but uses about six times the storage.")
                            .confidence(Confidence::Medium),
                    )
                    .option(
                        RecommendationOption::new("s3", "S3 mirror")
                            .reason("Keeps a second copy off site. Slow to restore from.")
                            .confidence(Confidence::Low),
                    )
                    .on_accept(move |id, _, cx| {
                        let line = format!("Accepted {id}");
                        accepted.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    })
                    .on_undo(move |id, _, cx| {
                        let line = format!("Undid {id}");
                        undone.update(cx, |this, cx| {
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
