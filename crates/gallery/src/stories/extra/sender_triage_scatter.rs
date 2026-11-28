// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the sender triage scatter.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::sender_triage_scatter::{Sender, SenderTriageScatter};

/// Made-up senders for one inbox: shops, newsletters, alerts and people.
fn senders() -> Vec<Sender> {
    [
        ("corner-books", "Corner Books", 34., 0.62),
        ("weekly-garden", "Weekly Garden Notes", 4., 0.91),
        ("deal-bell", "Deal Bell", 182., 0.03),
        ("shoe-depot", "Shoe Depot", 146., 0.06),
        ("city-transit", "City Transit Alerts", 58., 0.48),
        ("bakery-club", "Bakery Club", 22., 0.35),
        ("flash-sales", "Flash Sales Daily", 161., 0.11),
        ("home-goods", "Home Goods Outlet", 118., 0.09),
        ("travel-picks", "Travel Picks", 92., 0.14),
        ("school-office", "School Office", 12., 0.97),
        ("team-digest", "Team Digest", 40., 0.81),
        ("gadget-news", "Gadget News", 77., 0.22),
        ("recipe-box", "Recipe Box", 30., 0.41),
        ("survey-desk", "Survey Desk", 64., 0.04),
        ("points-club", "Points Club", 104., 0.18),
        ("local-paper", "Local Paper", 28., 0.66),
        ("bank-notices", "Bank Notices", 9., 0.88),
        ("fitness-hub", "Fitness Hub", 52., 0.12),
        ("music-drops", "Music Drops", 86., 0.31),
        ("pet-supplies", "Pet Supplies", 44., 0.27),
        ("event-tickets", "Event Tickets", 70., 0.08),
        ("app-updates", "App Updates", 132., 0.15),
    ]
    .into_iter()
    .map(|(id, name, volume, open)| Sender::new(id, name, volume, open))
    .collect()
}

/// An inbox's senders with the zone over busy, unread mail.
pub struct SenderTriageScatterStory {
    last: SharedString,
}

impl SenderTriageScatterStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Move the zone, then pick an action".into(),
        })
    }
}

impl Story for SenderTriageScatterStory {
    fn title() -> &'static str {
        "Sender triage scatter"
    }

    fn description() -> &'static str {
        "Email senders by volume and open rate, with a draggable, resizable zone that picks the ones to archive or unsubscribe from."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SenderTriageScatterStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                SenderTriageScatter::new("inbox-triage", "Inbox senders", senders())
                    .subtitle("Last 30 days")
                    .on_action(move |action, ids, _, cx| {
                        this.update(cx, |this, cx| {
                            this.last = format!("{action}: {} senders", ids.len()).into();
                            cx.notify();
                        })
                    }),
            )
            .child(div().text_sm().text_color(quiet).child(self.last.clone()))
    }
}
