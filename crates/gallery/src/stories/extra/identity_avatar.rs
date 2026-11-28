// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the identity avatar.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::identity_avatar::{
    Activity, Identity, IdentityAvatar, IdentityStack, Presence,
};

/// People and assistants on a bookkeeping team, at three sizes, in every
/// presence and activity, and stacked.
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

fn states() -> Vec<Identity> {
    vec![
        Identity::person("Lena Fisher").presence(Presence::Online),
        Identity::person("Omid Sadr").presence(Presence::Busy),
        Identity::person("June Park").presence(Presence::Away),
        Identity::person("Rafael Costa").presence(Presence::DoNotDisturb),
        Identity::person("Ines Moreau").presence(Presence::Offline),
        Identity::person("Kofi Mensah")
            .presence(Presence::Online)
            .activity(Activity::Typing),
        Identity::person("Sara Lind")
            .presence(Presence::Online)
            .activity(Activity::Speaking),
        Identity::agent("Ledger check")
            .presence(Presence::Online)
            .activity(Activity::Speaking),
    ]
}

/// One short word under each avatar: what they are doing, else whether they
/// are here.
fn caption(who: &Identity) -> String {
    let word = match who.shown_activity() {
        Activity::Idle => who.presence.word(),
        activity => activity.word(),
    };
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
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
        let state_row =
            h_flex()
                .gap_4()
                .flex_wrap()
                .children(states().into_iter().enumerate().map(move |(ix, who)| {
                    v_flex()
                        .items_center()
                        .gap_1()
                        .w(isonomic_ui::px(84.))
                        .pt_2()
                        .child(
                            IdentityAvatar::new(
                                isonomic_ui::ElementId::Name(format!("state-{ix}").into()),
                                who.clone(),
                            )
                            .size(36.),
                        )
                        .child(div().text_xs().truncate().child(who.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(quiet)
                                .truncate()
                                .child(caption(&who)),
                        )
                }));
        let busy_team = vec![
            Identity::person("Maya Okafor")
                .presence(Presence::Online)
                .activity(Activity::Speaking),
            Identity::person("Kofi Mensah").activity(Activity::Typing),
            Identity::agent("Reconciler").working(true),
            Identity::person("Tom Brandt").presence(Presence::Away),
            Identity::person("Ana Ruiz"),
            Identity::person("Dev Patel"),
            Identity::person("Lena Fisher"),
        ];
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
                            .child("Presence and activity"),
                    )
                    .child(state_row),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div().text_xs().text_color(quiet).child(
                            "Review call, spreads under the pointer; the count names the rest",
                        ),
                    )
                    .child(
                        IdentityStack::new("call-stack", busy_team)
                            .limit(5)
                            .size(32.)
                            .overlap(0.4)
                            .spread_on_hover(true),
                    ),
            )
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
