//! Gallery story for the total breakdown card.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window,
};
use gpui_component::v_flex;
use isonomic_ui_extra::total_breakdown_card::{BreakdownPart, TotalBreakdownCard};

/// Shows expenses by category for three periods, and a card with its own last line.
pub struct TotalBreakdownCardStory {
    period: SharedString,
}

impl TotalBreakdownCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            period: "month".into(),
        })
    }
}

impl Render for TotalBreakdownCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        // Payroll, rent, software, travel, bank fees.
        let (amounts, change) = match self.period.as_ref() {
            "week" => ([6200., 0., 410., 980., 35.], -4.5),
            "quarter" => ([81_400., 12_600., 5300., 7450., 610.], 2.),
            _ => ([27_100., 4200., 1760., 1290., 180.], 6.8),
        };

        v_flex()
            .gap_4()
            .child(
                TotalBreakdownCard::new("expenses", "Expenses by category")
                    .period("week", "Week")
                    .period("month", "Month")
                    .period("quarter", "Quarter")
                    .selected_period(self.period.clone())
                    .change(change)
                    .total_label("Paid out")
                    .part(BreakdownPart::new("Payroll", amounts[0]))
                    .part(BreakdownPart::new("Rent", amounts[1]))
                    .part(BreakdownPart::new("Software", amounts[2]))
                    .part(BreakdownPart::new("Travel", amounts[3]))
                    .part(BreakdownPart::new("Bank fees", amounts[4]))
                    .on_period_change(move |id, _, cx| {
                        let id = id.clone();
                        story.update(cx, |this, cx| {
                            this.period = id;
                            cx.notify();
                        })
                    }),
            )
            .child(
                TotalBreakdownCard::new("open-invoices", "Open invoices by age")
                    .total_label("Invoices")
                    .part(BreakdownPart::new("Not yet due", 38.))
                    .part(BreakdownPart::new("1 to 30 days late", 11.))
                    .part(BreakdownPart::new("Over 30 days late", 4.))
                    .footer("Counted this morning"),
            )
    }
}
