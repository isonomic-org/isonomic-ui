//! Gallery story for the context usage grid.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::context_usage_grid::{BudgetItem, BudgetPart, ContextUsageGrid};

/// Shows a bookkeeping assistant's context, close to full, with files to drop.
pub struct ContextUsageGridStory {
    last: SharedString,
}

impl ContextUsageGridStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing changed yet".into(),
        })
    }
}

impl Render for ContextUsageGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();

        v_flex()
            .gap_3()
            .child(
                ContextUsageGrid::new("assistant-context", 128_000)
                    .model("ledger-large")
                    .part(BudgetPart::new("rules", "Instructions", 3_100).locked(true))
                    .part(BudgetPart::new("tools", "Tools", 9_800))
                    .part(
                        BudgetPart::new("files", "Attached files", 0)
                            .item(BudgetItem::new("march", "march-statement.csv", 18_200))
                            .item(BudgetItem::new("chart", "chart-of-accounts.xlsx", 6_400))
                            .item(BudgetItem::new("policy", "expense-policy.pdf", 11_900)),
                    )
                    .part(BudgetPart::new("chat", "Conversation", 61_300).compactable(0.25))
                    .part(BudgetPart::new("notes", "Saved notes", 2_700))
                    .action("Start afresh", move |_, cx| {
                        story.update(cx, |this, cx| {
                            this.last = "Asked to start afresh".into();
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
