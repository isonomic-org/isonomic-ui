// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the identity avatar.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::identity_avatar::{Identity, IdentityAvatar, IdentityStack, Presence};

/// People and assistants on a bookkeeping team, at three sizes and stacked.
pub struct IdentityAvatarStory;

impl IdentityAvatarStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for IdentityAvatarStory {
    fn title() -> &'static str {
        "Identity avatar"
    }

    fn description() -> &'static str {
        "The kit's avatar for people and assistants, with presence, a working ring and a stack that counts the rest."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

fn team() -> Vec<Identity> {
    vec![
        Identity::person("Maya Okafor").presence(Presence::Online),
        Identity::agent("Reconciler")
            .working(true)
            .presence(Presence::Busy),
        Identity::person("Tom Brandt").presence(Presence::Away),
        Identity::agent("Payables").presence(Presence::Online),
        Identity::person("Ana Ruiz"),
        Identity::agent("Report writer"),
        Identity::person("Dev Patel").presence(Presence::Online),
    ]
}

impl Render for IdentityAvatarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let row = |size: f32, key: &str| {
            h_flex()
                .gap_4()
                .children(
                    team()
                        .into_iter()
                        .take(4)
                        .enumerate()
                        .map(move |(ix, who)| {
                            v_flex()
                                .items_center()
                                .gap_1()
                                .w(isonomic_ui::px(84.))
                                .child(
                                    IdentityAvatar::new(
                                        isonomic_ui::ElementId::Name(format!("{key}-{ix}").into()),
                                        who.clone(),
                                    )
                                    .size(size),
                                )
                                .child(div().text_xs().text_color(quiet).truncate().child(who.name))
                        }),
                )
        };
        v_flex()
            .gap_6()
            .child(row(40., "large"))
            .child(row(28., "medium"))
            .child(row(20., "small"))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child("Everyone on the March close"),
                    )
                    .child(IdentityStack::new("team-stack", team()).limit(5)),
            )
    }
}
