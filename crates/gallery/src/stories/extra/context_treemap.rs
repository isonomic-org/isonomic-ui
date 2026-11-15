//! Gallery story for the context treemap.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::context_treemap::{ContextBlock, ContextTreemap};

/// A bookkeeping assistant's context in the middle of a close. The story
/// keeps the view and the selection.
pub struct ContextTreemapStory {
    after: bool,
    selected: Option<SharedString>,
}

fn blocks() -> Vec<ContextBlock> {
    vec![
        ContextBlock::new("sys", "Instructions", "Assistant instructions", 6_200),
        ContextBlock::new("rules", "Instructions", "Close checklist", 2_100),
        ContextBlock::new("ledger", "Files", "March ledger export", 38_400).compacts_to(9_000),
        ContextBlock::new("bank", "Files", "Operating statement", 14_800).compacts_to(4_100),
        ContextBlock::new("policy", "Files", "Expense policy", 5_300),
        ContextBlock::new("talk1", "Conversation", "This morning's turns", 21_600)
            .compacts_to(3_200),
        ContextBlock::new("talk2", "Conversation", "Last hour", 9_800),
        ContextBlock::new("tool1", "Tool results", "Unmatched card lines", 26_900)
            .compacts_to(2_400),
        ContextBlock::new("tool2", "Tool results", "Vendor balances", 7_700).compacts_to(1_100),
        ContextBlock::new("tool3", "Tool results", "Posting errors", 1_900),
    ]
}

impl ContextTreemapStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            after: false,
            selected: Some("ledger".into()),
        })
    }
}

impl Story for ContextTreemapStory {
    fn title() -> &'static str {
        "Context treemap"
    }

    fn description() -> &'static str {
        "What fills a context budget as nested boxes, before and after compaction."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ContextTreemapStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let view = cx.entity();
        ContextTreemap::new("close-context", blocks(), 160_000)
            .after(self.after)
            .selected(self.selected.clone())
            .on_select(move |id, _, cx| {
                select.update(cx, |this, cx| {
                    this.selected = id.cloned();
                    cx.notify();
                })
            })
            .on_view(move |after, _, cx| {
                view.update(cx, |this, cx| {
                    this.after = after;
                    cx.notify();
                })
            })
    }
}
