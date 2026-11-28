// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the status badge.

use crate::Story;
use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::common::Tone;
use isonomic_ui_extra::status_badge::StatusBadge;

/// The five tones and the word the story gives each.
const TONES: [(Tone, &str); 5] = [
    (Tone::Neutral, "Draft"),
    (Tone::Info, "Scheduled"),
    (Tone::Success, "Approved"),
    (Tone::Warning, "Expiring"),
    (Tone::Danger, "Rejected"),
];

/// Invoice and run statuses, plus removable filter badges the story takes away.
pub struct StatusBadgeStory {
    filters: Vec<SharedString>,
}

impl StatusBadgeStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            filters: vec![
                "Overdue".into(),
                "Over $1,000".into(),
                "Northwind Dental".into(),
            ],
        })
    }
}

impl Story for StatusBadgeStory {
    fn title() -> &'static str {
        "Status badge"
    }

    fn description() -> &'static str {
        "A labelled pill in a tone, with an optional live dot, a capped count and a remove button."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for StatusBadgeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let label = |text: &'static str| div().text_xs().text_color(quiet).child(text);
        let filters: Vec<_> = self
            .filters
            .iter()
            .enumerate()
            .map(|(ix, name)| {
                let story = cx.entity();
                let name_for_remove = name.clone();
                StatusBadge::new(
                    isonomic_ui::ElementId::Name(format!("filter-{ix}").into()),
                    name.clone(),
                )
                .on_remove(move |_, cx| {
                    story.update(cx, |this, cx| {
                        this.filters.retain(|f| f != &name_for_remove);
                        cx.notify();
                    })
                })
            })
            .collect();
        v_flex()
            .gap_4()
            .child(label("Invoices"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(StatusBadge::new("draft", "Draft"))
                    .child(StatusBadge::new("sent", "Sent").tone(Tone::Info))
                    .child(
                        StatusBadge::new("overdue", "Overdue")
                            .tone(Tone::Warning)
                            .count(7, 99),
                    )
                    .child(StatusBadge::new("paid", "Paid").tone(Tone::Success))
                    .child(StatusBadge::new("void", "Void").tone(Tone::Danger)),
            )
            .child(label("Runs"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        StatusBadge::new("running", "Running")
                            .tone(Tone::Info)
                            .live(true),
                    )
                    .child(
                        StatusBadge::new("review", "Needs review")
                            .tone(Tone::Warning)
                            .solid(true)
                            .count(140, 99),
                    )
                    .child(
                        StatusBadge::new("failed", "Failed")
                            .tone(Tone::Danger)
                            .solid(true),
                    ),
            )
            .child(label("Five tones, with a dot"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .children(TONES.map(|(tone, word)| {
                        StatusBadge::new(format!("dot-{word}"), word)
                            .tone(tone)
                            .dot()
                    })),
            )
            .child(label("Five tones, with their icons"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .children(TONES.map(|(tone, word)| {
                        StatusBadge::new(format!("icon-{word}"), word)
                            .tone(tone)
                            .tone_icon()
                    })),
            )
            .child(label("Solid, with an icon of their own"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        StatusBadge::new("locked", "Locked")
                            .tone(Tone::Warning)
                            .solid(true)
                            .icon(CatalogIcon::Locked),
                    )
                    .child(
                        StatusBadge::new("synced", "Synced")
                            .tone(Tone::Success)
                            .solid(true)
                            .icon(CatalogIcon::Checkmark),
                    ),
            )
            .child(label("Filters"))
            .child(h_flex().flex_wrap().gap_2().children(filters))
    }
}
