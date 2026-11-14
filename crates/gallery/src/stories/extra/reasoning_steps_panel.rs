//! Gallery story for the reasoning steps panel.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::reasoning_steps_panel::{
    ReasoningSource, ReasoningStep, ReasoningStepsPanel,
};

/// Shows a looping run with a search step, and one that folds away when done.
pub struct ReasoningStepsPanelStory {
    last: SharedString,
}

impl ReasoningStepsPanelStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "The second run has not finished yet".into(),
        })
    }
}

impl Render for ReasoningStepsPanelStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();

        v_flex()
            .gap_6()
            .child(
                ReasoningStepsPanel::new("looping")
                    .looping(true)
                    .step(
                        ReasoningStep::new("Searching")
                            .finished_label("Searched")
                            .source(
                                ReasoningSource::new("Ledger: March entries")
                                    .detail("books.example/ledger/2026-03"),
                            )
                            .source(
                                ReasoningSource::new("Bank feed: main account")
                                    .detail("books.example/feeds/main"),
                            )
                            .source(ReasoningSource::new("Supplier statements"))
                            .hidden_source(ReasoningSource::new("Payroll run 06"))
                            .hidden_source(ReasoningSource::new("VAT return draft")),
                    )
                    .step(ReasoningStep::new("Matching 41 payments to invoices"))
                    .step(ReasoningStep::new("Checking the totals against the bank"))
                    .step(ReasoningStep::new("Writing up the differences")),
            )
            .child(
                ReasoningStepsPanel::new("folding")
                    .working_label("Reconciling")
                    .done_label("Books reconciled")
                    .step_interval(Duration::from_millis(900))
                    .collapse_when_done(true)
                    .step(ReasoningStep::new("Reading the open invoices"))
                    .step(ReasoningStep::new("Grouping them by customer"))
                    .on_done(move |elapsed, _, cx| {
                        let line = format!("The second run took {} ms", elapsed.as_millis());
                        story.update(cx, |this, cx| {
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
