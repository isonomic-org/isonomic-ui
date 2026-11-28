// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::{
    App, AppContext, Context, Entity, Focusable, InteractiveElement as _, IntoElement, Keystroke,
    ParentElement, Render, Styled, Window, div, px,
};

use isonomic_ui::component::{
    ActiveTheme as _, h_flex,
    kbd::{Kbd, KeyPlatform},
    v_flex,
};

use crate::section;

pub struct KbdStory {
    focus_handle: isonomic_ui::FocusHandle,
}

impl super::Story for KbdStory {
    fn title() -> &'static str {
        "Kbd"
    }

    fn description() -> &'static str {
        "A tag style to display keyboard shortcuts"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl KbdStory {
    pub(crate) fn new(_: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}
impl Focusable for KbdStory {
    fn focus_handle(&self, _: &isonomic_ui::App) -> isonomic_ui::FocusHandle {
        self.focus_handle.clone()
    }
}
impl Render for KbdStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let platform_row = |platform: KeyPlatform, name: &'static str| {
            h_flex()
                .id(name)
                .w_full()
                .gap_2()
                .child(div().w(px(80.)).text_xs().text_color(quiet).child(name))
                .children(
                    ["cmd-shift-p", "ctrl-alt-delete", "alt-enter", "escape"]
                        .into_iter()
                        .map(move |keys| {
                            Kbd::new(Keystroke::parse(keys).unwrap()).platform(platform)
                        }),
                )
        };
        v_flex()
            .w_full()
            .items_center()
            .gap_6()
            .child(
                section("Default")
                    .description("Displays single keys and multi-key shortcuts.")
                    .w(px(560.))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_center()
                            .gap_2()
                            .flex_wrap()
                            .child(Kbd::new(Keystroke::parse("cmd-shift-p").unwrap()))
                            .child(Kbd::new(Keystroke::parse("cmd-ctrl-t").unwrap()))
                            .child(Kbd::new(Keystroke::parse("cmd--").unwrap()))
                            .child(Kbd::new(Keystroke::parse("cmd-+").unwrap()))
                            .child(Kbd::new(Keystroke::parse("escape").unwrap()))
                            .child(Kbd::new(Keystroke::parse("backspace").unwrap()))
                            .child(Kbd::new(Keystroke::parse("/").unwrap()))
                            .child(Kbd::new(Keystroke::parse("enter").unwrap())),
                    ),
            )
            .child(
                section("Outlined")
                    .description("An outlined treatment adds emphasis on dense surfaces.")
                    .w(px(560.))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_center()
                            .gap_2()
                            .flex_wrap()
                            .child(Kbd::new(Keystroke::parse("cmd-shift-p").unwrap()).outline())
                            .child(Kbd::new(Keystroke::parse("cmd-ctrl-t").unwrap()).outline())
                            .child(Kbd::new(Keystroke::parse("enter").unwrap()).outline()),
                    ),
            )
            .child(
                section("Per platform")
                    .description(
                        "The same shortcuts written the way each platform writes them, on any \
                         build.",
                    )
                    .w(px(560.))
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(platform_row(KeyPlatform::Mac, "Mac"))
                            .child(platform_row(KeyPlatform::Windows, "Windows"))
                            .child(platform_row(KeyPlatform::Linux, "Linux")),
                    ),
            )
    }
}
