// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the reach rings.

use crate::Story;
use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{ActiveTheme as _, Icon, tag::Tag, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::reach_rings::{ReachNode, ReachRings};

/// What a lost staff badge of a made-up museum opens, door by door.
fn badge_reach() -> Vec<ReachNode> {
    vec![
        ReachNode::new("entrance", "Staff entrance", "door"),
        ReachNode::new("workshop", "Workshop", "door").note("Open on weekdays"),
        ReachNode::new("cage", "Tool cage", "cabinet").through("workshop"),
        ReachNode::new("kiln", "Kiln room", "room").through("workshop"),
        ReachNode::new("lift", "Archive lift", "door").note("Badge only, no key"),
        ReachNode::new("papers", "Paper store", "room")
            .through("lift")
            .affected()
            .note("Entered twice since the badge went missing"),
        ReachNode::new("ledgers", "Loan ledgers", "cabinet")
            .through("papers")
            .affected(),
        ReachNode::new("maps", "Map drawers", "cabinet")
            .through("papers")
            .affected()
            .indirect()
            .note("Opened with the key kept in the paper store"),
        ReachNode::new("prints", "Print racks", "cabinet").through("lift"),
        ReachNode::new("desk", "Ticket desk", "door"),
        ReachNode::new("till", "Till drawer", "cabinet").through("desk"),
        ReachNode::new("bay", "Loading bay", "door"),
        ReachNode::new("crates", "Crate store", "room").through("bay"),
        ReachNode::new("frames", "Frame racks", "cabinet")
            .through("crates")
            .indirect(),
    ]
}

/// A lost badge with a traced path, and a small ring with nothing affected.
pub struct ReachRingsStory {
    picked: SharedString,
}

impl ReachRingsStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a node".into(),
        })
    }
}

impl Story for ReachRingsStory {
    fn title() -> &'static str {
        "Reach rings"
    }

    fn description() -> &'static str {
        "What one thing can reach hop by hop as concentric rings, with the affected paths traced back to the middle."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ReachRingsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                ReachRings::new("badge", "Lost badge", "Badge 0417")
                    .subtitle("Everything it still opens")
                    .icon(Icon::new(CatalogIcon::Badge))
                    .nodes(badge_reach())
                    .kind("door", "Door")
                    .kind("room", "Room")
                    .kind("cabinet", "Cabinet")
                    .aside(Tag::danger().rounded_full().child("3 affected"))
                    .on_select(move |node, _, cx| {
                        let said = isonomic_ui_extra::reach_rings::node_reading(node, "node", 0);
                        this.update(cx, |this, cx| {
                            this.picked = said.into();
                            cx.notify();
                        })
                    }),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
            .child(
                ReachRings::new("kettle", "Kitchen circuit", "Fuse 6")
                    .subtitle("One hop, nothing affected")
                    .initials("F6")
                    .nodes([
                        ReachNode::new("kettle", "Kettle", "socket"),
                        ReachNode::new("toaster", "Toaster", "socket"),
                        ReachNode::new("fridge", "Fridge", "socket"),
                        ReachNode::new("hood", "Cooker hood", "light"),
                        ReachNode::new("strip", "Counter strip", "light"),
                    ]),
            )
    }
}
