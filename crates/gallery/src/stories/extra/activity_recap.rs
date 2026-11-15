//! Gallery story for the activity recap.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::activity_recap::{ActivityRecap, RecapBucket, RecapItem, RecapLane};

/// A bookkeeping team's day, hour by hour. The story keeps the bar the
/// pointer is on.
pub struct ActivityRecapStory {
    picked: Option<usize>,
}

impl ActivityRecapStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { picked: None })
    }
}

impl Story for ActivityRecapStory {
    fn title() -> &'static str {
        "Activity recap"
    }

    fn description() -> &'static str {
        "A day's work as a stacked bar chart over time, then lanes of what needs review, what failed and what's done."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ActivityRecapStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use RecapLane::*;
        let pick = cx.entity();
        ActivityRecap::new(
            "today",
            "Today, Monday",
            vec![
                RecapBucket::new("8 am", 3, 0, 0),
                RecapBucket::new("9 am", 7, 1, 1),
                RecapBucket::new("10 am", 9, 0, 2),
                RecapBucket::new("11 am", 6, 2, 0),
                RecapBucket::new("12 pm", 2, 0, 0),
                RecapBucket::new("1 pm", 5, 1, 1),
                RecapBucket::new("2 pm", 8, 0, 1),
                RecapBucket::new("3 pm", 4, 0, 0),
            ],
            vec![
                RecapItem::new("r1", Review, "Approve Friday's supplier run, $14,822.10", "Payables \u{b7} 2:14 pm"),
                RecapItem::new("r2", Review, "Reply to Northwind Dental's payment plan request", "Collections \u{b7} 1:40 pm"),
                RecapItem::new("r3", Review, "Check two contacts merged as one", "Contacts \u{b7} 10:22 am"),
                RecapItem::new("f1", Failed, "Post 201 card matches: March is locked", "Card matcher \u{b7} 11:55 am"),
                RecapItem::new("f2", Failed, "Send the third reminder: mail refused it", "Collections \u{b7} 9:31 am"),
                RecapItem::new("d1", Done, "Matched all 61 operating account lines", "Bank matcher \u{b7} 10:12 am"),
                RecapItem::new("d2", Done, "Filed 43 receipts from the inbox", "Receipt filer \u{b7} 2:50 pm"),
            ],
        )
        .picked(self.picked)
        .on_pick(move |ix, _, cx| {
            pick.update(cx, |this, cx| {
                this.picked = ix;
                cx.notify();
            })
        })
    }
}
