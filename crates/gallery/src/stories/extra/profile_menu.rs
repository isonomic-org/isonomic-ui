// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the profile menu.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::profile_menu::{ProfileMenu, ProfileMenuItem, ProfileSetup};

/// Shows a menu with a badge and setup progress, and reports the last choice.
pub struct ProfileMenuStory {
    last: SharedString,
}

impl ProfileMenuStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing chosen yet".into(),
        })
    }
}

impl Story for ProfileMenuStory {
    fn title() -> &'static str {
        "Profile menu"
    }

    fn description() -> &'static str {
        "The card behind an avatar, with the signed-in user, account"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ProfileMenuStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let report = move |line: &'static str| {
            let view = view.clone();
            move |_: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| {
                    this.last = line.into();
                    cx.notify();
                })
            }
        };

        v_flex()
            .gap_3()
            .items_start()
            .child(
                ProfileMenu::new("account", "Noor Castellanos")
                    .email("noor.castellanos@northwind-freight.example")
                    .item(
                        ProfileMenuItem::new("settings", "Settings")
                            .icon(IconName::Settings)
                            .on_select(report("Settings")),
                    )
                    .item(
                        ProfileMenuItem::new("billing", "Billing")
                            .icon(IconName::FileText)
                            .badge("Due")
                            .on_select(report("Billing")),
                    )
                    .item(
                        ProfileMenuItem::new("sign-out", "Sign out")
                            .icon(IconName::ExternalLink)
                            .on_select(report("Sign out")),
                    )
                    .setup(
                        ProfileSetup::new(2, 4)
                            .title("Workspace setup")
                            .next("Invite a teammate")
                            .on_continue(report("Continue setup")),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
