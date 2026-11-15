//! Gallery story for the elimination bracket.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::elimination_bracket::{Contender, EliminationBracket, Side, Verdict};

/// Shows eight late-fee policies judged down to one, with a semifinal under way.
pub struct EliminationBracketStory {
    last: SharedString,
}

impl EliminationBracketStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing used yet".into(),
        })
    }
}

impl Story for EliminationBracketStory {
    fn title() -> &'static str {
        "Elimination bracket"
    }

    fn description() -> &'static str {
        "A judge picks the best of several answers, two at a"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EliminationBracketStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let contender = |id: &'static str, name: &'static str, output: &'static str| {
            Contender::new(id, name)
                .meta("Drafted by the policy assistant")
                .output(output)
        };

        v_flex()
            .gap_3()
            .max_w(px(760.))
            .child(
                EliminationBracket::new("late-fees")
                    .task("Pick a late-fee policy for overdue invoices")
                    .judge("Review assistant")
                    .contender(contender(
                        "flat",
                        "Flat fee",
                        "Adds 25.00 once an invoice is 14 days overdue.",
                    ))
                    .contender(contender(
                        "daily",
                        "Daily interest",
                        "Adds 0.05% of the balance for every day past due.",
                    ))
                    .contender(contender(
                        "tiered",
                        "Tiered fee",
                        "Adds 2% at 14 days and another 3% at 45 days.",
                    ))
                    .contender(contender(
                        "none",
                        "No fee",
                        "Sends reminders only and never adds a fee.",
                    ))
                    .contender(contender(
                        "grace",
                        "Grace then fee",
                        "Waives the first late invoice of the year, then charges 2%.",
                    ))
                    .contender(contender(
                        "discount",
                        "Early discount",
                        "Takes 2% off when paid within 10 days instead of charging late.",
                    ))
                    .contender(contender(
                        "monthly",
                        "Monthly charge",
                        "Adds 1.5% of the balance on the first of each month it is open.",
                    ))
                    .contender(contender(
                        "hold",
                        "Hold the account",
                        "Pauses new orders until the oldest invoice is paid.",
                    ))
                    .verdict(
                        0,
                        0,
                        Verdict::new(Side::Upper)
                            .scores(7.4, 6.1)
                            .reason("Simple to explain, and customers know the cost up front."),
                    )
                    .verdict(
                        0,
                        1,
                        Verdict::new(Side::Upper).scores(8.2, 3.9).reason(
                            "Two steps follow how late the invoice is; no fee changes nothing.",
                        ),
                    )
                    .verdict(
                        0,
                        2,
                        Verdict::new(Side::Upper).scores(8.6, 7.7).reason(
                            "Keeps goodwill with good payers while still charging repeat lateness.",
                        ),
                    )
                    .verdict(
                        0,
                        3,
                        Verdict::new(Side::Lower).scores(5.2, 6.0).reason(
                            "Holding the account stops the loss sooner than a monthly charge.",
                        ),
                    )
                    .verdict(
                        1,
                        0,
                        Verdict::new(Side::Lower).scores(7.1, 8.4).reason(
                            "Two clear steps beat a flat fee that treats 15 and 90 days alike.",
                        ),
                    )
                    .judging(1, 1)
                    .on_pick(move |id, _, cx| {
                        let line = format!("Using {id}");
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
