// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::component::{ActiveTheme as _, IconName, Sizable, Size, spinner::Spinner, v_flex};
use isonomic_ui::{
    App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Window, bounce, ease_in_out, ease_out_quint, linear, px,
};

use crate::{ChangeStorySize, section, story_toolbar};

pub struct SpinnerStory {
    focus_handle: isonomic_ui::FocusHandle,
    value: f32,
    size: Size,
}

impl super::Story for SpinnerStory {
    fn title() -> &'static str {
        "Spinner"
    }

    fn description() -> &'static str {
        "Displays an spinner showing the completion progress of a task."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl SpinnerStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            value: 50.,
            size: Size::Medium,
        }
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value;
    }
}

impl Focusable for SpinnerStory {
    fn focus_handle(&self, _: &isonomic_ui::App) -> isonomic_ui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SpinnerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_3()
            .on_action(cx.listener(|this, action: &ChangeStorySize, _, cx| {
                this.size = action.0;
                cx.notify();
            }))
            .child(story_toolbar(self.size))
            .child(
                section("Default")
                    .description("An indeterminate loading indicator.")
                    .gap_x_2()
                    .child(Spinner::new().with_size(self.size)),
            )
            .child(
                section("Arc")
                    .description("A plain turning arc, for quiet inline waits.")
                    .gap_x_2()
                    .child(Spinner::new().arc().with_size(self.size))
                    .child(
                        Spinner::new()
                            .arc()
                            .with_size(self.size)
                            .color(cx.theme().blue),
                    ),
            )
            .child(
                section("Rays and dots")
                    .description(
                        "Thinking indicators: rays running round, or three dots brightening in \
                         turn. The dots carry a spoken label, so a screen reader hears \
                         \"Loading\".",
                    )
                    .gap_x_2()
                    .child(Spinner::new().rays().with_size(self.size))
                    .child(
                        Spinner::new()
                            .dots()
                            .with_size(self.size)
                            .loading_label("spinner-dots", "Loading"),
                    ),
            )
            .child(
                section("Every size")
                    .description("Each style at 12, 14, 16 and 20 px.")
                    .gap_x_4()
                    .children(
                        [Size::XSmall, Size::Small, Size::Medium, Size::Large]
                            .into_iter()
                            .map(|size| {
                                isonomic_ui::component::h_flex()
                                    .gap_2()
                                    .child(Spinner::new().with_size(size))
                                    .child(Spinner::new().arc().with_size(size))
                                    .child(Spinner::new().rays().with_size(size))
                                    .child(Spinner::new().dots().with_size(size))
                            }),
                    ),
            )
            .child(
                section("Color")
                    .description("Use a color that suits the surrounding status.")
                    .gap_x_2()
                    .child(Spinner::new().with_size(self.size).color(cx.theme().blue))
                    .child(Spinner::new().with_size(self.size).color(cx.theme().green)),
            )
            .child(
                section("Custom size")
                    .description("A fixed pixel size is also supported.")
                    .gap_x_2()
                    .child(Spinner::new().with_size(px(64.))),
            )
            .child(
                section("Icon")
                    .description("Replace the default spinner glyph.")
                    .gap_x_2()
                    .child(
                        Spinner::new()
                            .with_size(self.size)
                            .icon(IconName::LoaderCircle),
                    )
                    .child(
                        Spinner::new()
                            .with_size(self.size)
                            .icon(IconName::LoaderCircle)
                            .color(cx.theme().cyan),
                    ),
            )
            .child(
                section("Easing")
                    .description("Customize the rotation timing curve.")
                    .gap_x_2()
                    .child(
                        Spinner::new()
                            .with_size(self.size)
                            .icon(IconName::Loader)
                            .ease(linear),
                    )
                    .child(
                        Spinner::new()
                            .with_size(self.size)
                            .icon(IconName::Loader)
                            .ease(bounce(ease_in_out)),
                    )
                    .child(
                        Spinner::new()
                            .with_size(self.size)
                            .icon(IconName::Loader)
                            .ease(ease_out_quint()),
                    ),
            )
    }
}
