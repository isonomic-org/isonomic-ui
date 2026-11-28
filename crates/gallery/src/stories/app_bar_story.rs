// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, Size, StyledExt,
    app_bar::{AppBar, AppBarAction},
    avatar::Avatar,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    v_flex,
};
use isonomic_ui::*;

use crate::{ChangeStorySize, section, story_toolbar};

pub struct AppBarStory {
    search: Entity<InputState>,
    last_action: SharedString,
    size: Size,
}

impl super::Story for AppBarStory {
    fn title() -> &'static str {
        "AppBar"
    }

    fn description() -> &'static str {
        "The header of a window or page: navigation, a title, content and commands."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl AppBarStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            search: cx.new(|cx| InputState::new(window, cx).placeholder("Search invoices")),
            last_action: "None yet".into(),
            size: Size::Medium,
        }
    }

    /// An action that reports its label when chosen, from the bar or its menu.
    fn action(
        &self,
        id: &'static str,
        label: &'static str,
        icon: Option<IconName>,
        cx: &mut Context<Self>,
    ) -> AppBarAction {
        let action = AppBarAction::new(id, label);
        let action = match icon {
            Some(icon) => action.icon(icon),
            None => action,
        };
        action.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.last_action = label.into();
            cx.notify();
        }))
    }
}

/// A bar above a stand-in for the page it heads.
fn frame(bar: AppBar, cx: &App) -> impl IntoElement {
    v_flex()
        .w_full()
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius_lg)
        .overflow_hidden()
        .child(bar)
        .child(div().h_20().bg(cx.theme().muted.opacity(0.4)))
}

fn menu_button() -> Button {
    Button::new("navigation")
        .ghost()
        .small()
        .icon(IconName::Menu)
        .tooltip("Menu")
        .accessibility_label("Menu")
}

impl Focusable for AppBarStory {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl Render for AppBarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = self.size;
        let default_bar = AppBar::new("default-bar")
            .with_size(size)
            .navigation(menu_button())
            .title("Projects")
            .subtitle("12 open")
            .action(self.action("search", "Search", Some(IconName::Search), cx))
            .action(self.action("notifications", "Notifications", Some(IconName::Bell), cx))
            .action(self.action("new-project", "New project", None, cx))
            .trailing(Avatar::new().name("Sam Rivera").small());

        let content_bar = AppBar::new("content-bar")
            .with_size(size)
            .navigation(
                Button::new("back")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Back")
                    .accessibility_label("Back"),
            )
            .title("Invoices")
            .child(Input::new(&self.search).small().max_w_64())
            .action(self.action("export", "Export", None, cx))
            .action(self.action("new-invoice", "New invoice", Some(IconName::Plus), cx));

        let overflow_bar = AppBar::new("overflow-bar")
            .with_size(size)
            .navigation(menu_button())
            .title("Quarterly planning review")
            .actions([
                self.action("share", "Share", None, cx),
                self.action("star", "Add to favorites", Some(IconName::Star), cx),
                self.action("duplicate", "Duplicate", Some(IconName::Copy), cx),
                self.action("delete", "Delete", Some(IconName::Delete), cx),
            ])
            .max_visible_actions(1);

        let plain_bar = AppBar::new("plain-bar")
            .with_size(size)
            .bordered(false)
            .title("Settings")
            .action(self.action("help", "Help", Some(IconName::Info), cx));

        v_flex()
            .gap_3()
            .on_action(cx.listener(|this, action: &ChangeStorySize, _, cx| {
                this.size = action.0;
                cx.notify();
            }))
            .child(story_toolbar(self.size))
            .child(
                section("Default")
                    .description(
                        "Navigation, a title and subtitle, commands, and trailing content.",
                    )
                    .v_flex()
                    .gap_3()
                    .child(frame(default_bar, cx))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Last action: {}", self.last_action)),
                    ),
            )
            .child(
                section("With content")
                    .description("A search field or tabs fill the space before the commands.")
                    .child(frame(content_bar, cx)),
            )
            .child(
                section("Overflow")
                    .description(
                        "Extra commands move into a More actions menu; long titles truncate.",
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(360.))
                            .child(frame(overflow_bar, cx)),
                    ),
            )
            .child(
                section("Without a border")
                    .description("Drop the border when the page below draws its own edge.")
                    .child(frame(plain_bar, cx)),
            )
    }
}
