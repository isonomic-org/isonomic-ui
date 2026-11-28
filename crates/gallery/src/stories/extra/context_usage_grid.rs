// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the context usage grid.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div, px,
};
use isonomic_ui_extra::context_usage_grid::{
    BudgetItem, BudgetPart, ContextDisplay, ContextUsageGrid,
};

/// The attached files: id, name and size.
const FILES: [(&str, &str, u64); 3] = [
    ("march", "march-statement.csv", 18_200),
    ("chart", "chart-of-accounts.xlsx", 6_400),
    ("policy", "expense-policy.pdf", 11_900),
];

/// Shows a bookkeeping assistant's context, close to full, with files to drop.
/// The story plays the session: it drops the files, compacts the conversation
/// to a quarter and starts afresh when asked.
///
/// Under it, the same budget as a slim meter for a coding session close to
/// its limit: the warning offers to compact everything, shows it working
/// for a moment, then the conversation shrinks.
pub struct ContextUsageGridStory {
    removed: Vec<SharedString>,
    compacted: bool,
    last: SharedString,
    meter_compacting: bool,
    meter_compacted: bool,
    _compact: Option<Task<()>>,
}

impl ContextUsageGridStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            removed: Vec::new(),
            compacted: false,
            last: "Nothing changed yet".into(),
            meter_compacting: false,
            meter_compacted: false,
            _compact: None,
        })
    }
}

impl Story for ContextUsageGridStory {
    fn title() -> &'static str {
        "Context usage grid"
    }

    fn description() -> &'static str {
        "What fills a model's context budget."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ContextUsageGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let removing = story.clone();
        let compacting = story.clone();
        let files = FILES
            .iter()
            .filter(|(id, _, _)| !self.removed.iter().any(|removed| removed == id))
            .fold(
                BudgetPart::new("files", "Attached files", 0),
                |part, (id, name, tokens)| part.item(BudgetItem::new(*id, *name, *tokens)),
            );
        let chat = if self.compacted { 61_300 / 4 } else { 61_300 };

        v_flex()
            .gap_3()
            .child(
                ContextUsageGrid::new("assistant-context", 128_000)
                    .model("ledger-large")
                    .part(BudgetPart::new("rules", "Instructions", 3_100).locked(true))
                    .part(BudgetPart::new("tools", "Tools", 9_800))
                    .part(files)
                    .part(
                        BudgetPart::new("chat", "Conversation", chat)
                            .compactable(true)
                            .compacted(self.compacted),
                    )
                    .part(BudgetPart::new("notes", "Saved notes", 2_700))
                    .on_remove_item(move |_, item, _, cx| {
                        let item = item.clone();
                        let name = FILES
                            .iter()
                            .find(|(id, _, _)| *id == item.as_ref())
                            .map_or(item.to_string(), |(_, name, _)| name.to_string());
                        removing.update(cx, |this, cx| {
                            this.last = format!("Removed {name}").into();
                            this.removed.push(item);
                            cx.notify();
                        })
                    })
                    .on_compact(move |_, _, cx| {
                        compacting.update(cx, |this, cx| {
                            this.compacted = true;
                            this.last = "Compacted the conversation".into();
                            cx.notify();
                        })
                    })
                    .action("Start afresh", move |_, cx| {
                        story.update(cx, |this, cx| {
                            this.removed.clear();
                            this.compacted = false;
                            this.last = "Started afresh".into();
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
            .child(div().h(px(12.)))
            .child({
                let compact = cx.entity();
                let restart = cx.entity();
                let history = if self.meter_compacted {
                    21_000
                } else {
                    148_500
                };
                ContextUsageGrid::new("session-meter", 200_000)
                    .title("Session context")
                    .model("planner-long")
                    .display(ContextDisplay::Meter)
                    .warn_at(0.8)
                    .part(BudgetPart::new("rules", "Instructions", 4_200).locked(true))
                    .part(BudgetPart::new("tools", "Tools", 12_600))
                    .part(BudgetPart::new("repo", "Open files", 18_900))
                    .part(BudgetPart::new("chat", "Conversation", history))
                    .frees(127_500)
                    .compacting(self.meter_compacting)
                    .on_compact_all(move |_, cx| {
                        compact.update(cx, |this, cx| {
                            this.meter_compacting = true;
                            this._compact = Some(cx.spawn(
                                async move |this: isonomic_ui::WeakEntity<Self>,
                                            cx: &mut isonomic_ui::AsyncApp| {
                                    cx.background_executor()
                                        .timer(std::time::Duration::from_millis(1_600))
                                        .await;
                                    this.update(cx, |this, cx| {
                                        this.meter_compacting = false;
                                        this.meter_compacted = true;
                                        cx.notify();
                                    })
                                    .ok();
                                },
                            ));
                            cx.notify();
                        })
                    })
                    .action("Reset", move |_, cx| {
                        restart.update(cx, |this, cx| {
                            this.meter_compacted = false;
                            this.meter_compacting = false;
                            this._compact = None;
                            cx.notify();
                        })
                    })
            })
    }
}
