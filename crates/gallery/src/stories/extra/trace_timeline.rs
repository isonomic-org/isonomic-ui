//! Gallery story for the trace timeline.

use std::time::Duration;

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Task, Window};
use isonomic_ui_extra::trace_timeline::{TraceSpan, TraceTimeline};

/// A live reconciliation trace whose clock ticks while two spans run. The
/// story keeps the open spans and the selection.
pub struct TraceTimelineStory {
    now: u32,
    open: Vec<SharedString>,
    selected: Option<SharedString>,
    _tick: Task<()>,
}

fn spans() -> Vec<TraceSpan> {
    vec![
        TraceSpan::new("plan", "Plan the close", 0, 1_840).usage(6_400, 1_850),
        TraceSpan::new("recon", "Reconciler", 1_900, 0)
            .running()
            .usage(3_100, 900)
            .child(TraceSpan::new("list", "ledger.list_unmatched", 2_000, 2_920).usage(400, 60))
            .child(
                TraceSpan::new("cards", "Card matcher", 3_000, 41_200)
                    .usage(41_800, 11_200)
                    .child(TraceSpan::new("match", "match.by_amount_and_date", 3_100, 9_400).usage(12_000, 3_100))
                    .child(TraceSpan::new("post", "ledger.post_matches", 40_800, 41_200).failed().usage(600, 90)),
            )
            .child(TraceSpan::new("bank", "Bank matcher", 3_000, 25_900).usage(22_900, 6_300))
            .child(TraceSpan::new("decide", "Decide where to post", 41_400, 0).running().usage(2_100, 520)),
        TraceSpan::new("pay", "Payables assistant", 2_100, 30_400).usage(14_700, 4_100),
    ]
}

impl TraceTimelineStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tick = cx.spawn(async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                loop {
                    cx.background_executor().timer(Duration::from_millis(500)).await;
                    let alive = this.update(cx, |this, cx| {
                        this.now = if this.now > 70_000 { 44_000 } else { this.now + 500 };
                        cx.notify();
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            });
            Self {
                now: 44_000,
                open: vec!["recon".into(), "cards".into()],
                selected: Some("cards".into()),
                _tick: tick,
            }
        })
    }
}

impl Story for TraceTimelineStory {
    fn title() -> &'static str {
        "Trace timeline"
    }

    fn description() -> &'static str {
        "A run's spans as a folding tree beside bars on a shared time axis, with time, tokens and cost per step."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TraceTimelineStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let toggle = cx.entity();
        TraceTimeline::new("close-trace", spans(), self.now)
            .open(self.open.clone())
            .selected(self.selected.clone())
            .on_select(move |id, _, cx| {
                select.update(cx, |this, cx| {
                    this.selected = Some(id.clone());
                    cx.notify();
                })
            })
            .on_toggle(move |id, _, cx| {
                toggle.update(cx, |this, cx| {
                    if let Some(at) = this.open.iter().position(|o| o == id) {
                        this.open.remove(at);
                    } else {
                        this.open.push(id.clone());
                    }
                    cx.notify();
                })
            })
    }
}
