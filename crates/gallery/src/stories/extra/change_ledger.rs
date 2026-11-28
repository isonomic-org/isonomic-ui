// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the change ledger.

use std::time::{Duration, Instant};

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::change_ledger::{ChangeLedger, LedgerEntry};

/// What a pricing update agent changed across a small firm's systems. The
/// ledger keeps its own undone list; the story notes each undo and redo.
pub struct ChangeLedgerStory {
    started: Instant,
    last: SharedString,
}

impl ChangeLedgerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            started: Instant::now(),
            last: "Nothing undone yet".into(),
        })
    }
}

impl Story for ChangeLedgerStory {
    fn title() -> &'static str {
        "Change ledger"
    }

    fn description() -> &'static str {
        "Everything an agent changed, each with an undo."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ChangeLedgerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (undo_story, redo_story) = (cx.entity(), cx.entity());
        let ago = |secs| {
            self.started
                .checked_sub(Duration::from_secs(secs))
                .unwrap_or(self.started)
        };

        let ledger = ChangeLedger::new("fee-ledger")
            .agent("Fee update agent")
            .entry(
                LedgerEntry::new("rate", "Changed hourly rate", "Billing", ago(170))
                    .target("Bookkeeping, standard")
                    .icon(IconName::Settings2)
                    .change("85.00 / hour", "92.00 / hour"),
            )
            .entry(
                LedgerEntry::new("terms", "Updated terms", "Documents", ago(125))
                    .target("Engagement letter")
                    .icon(IconName::FileText)
                    .change("Rates from 2025", "Rates from April 2026")
                    .builds_on("rate"),
            )
            .entry(
                LedgerEntry::new("clients", "Updated 38 clients", "Practice", ago(80))
                    .target("Monthly retainers")
                    .icon(IconName::User)
                    .change("340.00 / month", "368.00 / month")
                    .builds_on("rate"),
            )
            .entry(
                LedgerEntry::new("draft", "Drafted invoices", "Billing", ago(42))
                    .target("April run")
                    .icon(IconName::Inbox)
                    .detail("38 drafts at the new retainer")
                    .builds_on("clients"),
            )
            .entry(
                LedgerEntry::new("email", "Emailed clients", "Mail", ago(9))
                    .target("Fee notice")
                    .icon(IconName::Bell)
                    .detail("Sent to 38 contacts, with the new letter")
                    .builds_on("terms")
                    .irreversible(),
            )
            .on_undo(move |ids, _, cx| {
                undo_story.update(cx, |this, cx| {
                    this.last = format!("Undid {}", ids.join(", ")).into();
                    cx.notify();
                })
            })
            .on_redo(move |ids, _, cx| {
                redo_story.update(cx, |this, cx| {
                    this.last = format!("Redid {}", ids.join(", ")).into();
                    cx.notify();
                })
            });

        v_flex().gap_3().child(ledger).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
