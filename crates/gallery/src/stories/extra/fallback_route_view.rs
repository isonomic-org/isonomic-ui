//! Gallery story for the fallback route view.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::fallback_route_view::{AttemptStatus, FallbackRouteView, RouteAttempt};

/// Shows a route in progress, one that answered, and one where everything failed.
pub struct FallbackRouteViewStory {
    last: SharedString,
}

impl FallbackRouteViewStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing retried yet".into(),
        })
    }
}

impl Story for FallbackRouteViewStory {
    fn title() -> &'static str {
        "Fallback route view"
    }

    fn description() -> &'static str {
        "One request finding a target that answers."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FallbackRouteViewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();

        v_flex()
            .gap_4()
            .child(
                FallbackRouteView::new("in-progress", "Read the March bank statement")
                    .route("statements/auto")
                    .total_cost("0.0004")
                    .attempt(
                        RouteAttempt::new("Reader Large")
                            .provider("Region north")
                            .status(AttemptStatus::Failed)
                            .note("Over its hourly limit")
                            .elapsed(Duration::from_millis(1400))
                            .cost("0"),
                    )
                    .attempt(
                        RouteAttempt::new("Reader Large")
                            .provider("Region west")
                            .status(AttemptStatus::Trying)
                            .elapsed(Duration::from_millis(820))
                            .cost("0.0004"),
                    )
                    .attempt(RouteAttempt::new("Reader Small").provider("Region north"))
                    .attempt(RouteAttempt::new("Reader Small").provider("On site")),
            )
            .child(
                FallbackRouteView::new("answered", "Match receipt 2291 to a payment")
                    .route("matching/cheapest")
                    .total_cost("0.0011")
                    .attempt(
                        RouteAttempt::new("Matcher Quick")
                            .provider("Region west")
                            .status(AttemptStatus::Failed)
                            .note("Gave up after 5 s")
                            .elapsed(Duration::from_secs(5))
                            .cost("0.0003"),
                    )
                    .attempt(
                        RouteAttempt::new("Matcher Careful")
                            .provider("Region west")
                            .status(AttemptStatus::Answered)
                            .elapsed(Duration::from_millis(2300))
                            .cost("0.0008"),
                    )
                    .attempt(
                        RouteAttempt::new("Matcher Careful")
                            .provider("On site")
                            .status(AttemptStatus::Skipped),
                    ),
            )
            .child(
                FallbackRouteView::new("failed", "Draft the VAT summary")
                    .route("drafting/auto")
                    .attempt(
                        RouteAttempt::new("Writer")
                            .provider("Region north")
                            .status(AttemptStatus::Failed)
                            .note("Service unavailable")
                            .elapsed(Duration::from_millis(300)),
                    )
                    .attempt(
                        RouteAttempt::new("Writer")
                            .provider("Region west")
                            .status(AttemptStatus::Failed)
                            .note("Service unavailable")
                            .elapsed(Duration::from_millis(280)),
                    )
                    .on_retry(move |_, cx| {
                        story.update(cx, |this, cx| {
                            this.last = "Asked to try the VAT summary again".into();
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
