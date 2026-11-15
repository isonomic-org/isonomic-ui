//! Gallery story for the catch-up summary.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::catch_up_summary::{CatchUpItem, CatchUpKind, CatchUpSummary};

/// What bookkeeping assistants did over a lunch break. The story keeps the
/// fold and says which jump was clicked.
pub struct CatchUpSummaryStory {
    folded: bool,
    opened: Option<String>,
}

fn items() -> Vec<CatchUpItem> {
    use CatchUpKind::*;
    vec![
        CatchUpItem::new("recon", Done, "Matched all 61 operating account lines for March").when("1:12 pm"),
        CatchUpItem::new("approve", NeedsYou, "Friday's supplier run is ready: 9 bills, $14,822.10")
            .when("1:40 pm")
            .action("Review"),
        CatchUpItem::new("cards", Failed, "201 card matches couldn't post because the March ledger is locked")
            .when("1:55 pm")
            .action("See why"),
        CatchUpItem::new("reminders", Done, "Sent 5 of 7 payment reminders").when("2:05 pm"),
        CatchUpItem::new("reply", NeedsYou, "Northwind Dental replied asking for a payment plan")
            .when("2:20 pm")
            .action("Open"),
        CatchUpItem::new("note", Note, "Spend today is $0.84 of the $5 daily budget"),
    ]
}

impl CatchUpSummaryStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            folded: false,
            opened: None,
        })
    }
}

impl Story for CatchUpSummaryStory {
    fn title() -> &'static str {
        "Catch-up summary"
    }

    fn description() -> &'static str {
        "What happened while you were away, as a card that folds to a divider."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CatchUpSummaryStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fold = cx.entity();
        let open = cx.entity();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                CatchUpSummary::new("lunch", "2 h 10 min", items())
                    .summary("The March reconciliation is nearly done. Two things wait on you, and the card matches need the ledger unlocked.")
                    .folded(self.folded)
                    .on_fold(move |_, cx| {
                        fold.update(cx, |this, cx| {
                            this.folded = !this.folded;
                            cx.notify();
                        })
                    })
                    .on_open(move |id, _, cx| {
                        open.update(cx, |this, cx| {
                            this.opened = Some(id.to_string());
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match &self.opened {
                        Some(id) => format!("Jumped to: {id}"),
                        None => "Click the header to fold it to a divider.".into(),
                    }),
            )
    }
}
