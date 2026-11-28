// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::component::{ActiveTheme as _, aspect_ratio::AspectRatio, h_flex, v_flex};
use isonomic_ui::*;

use crate::section;

pub struct AspectRatioStory;

impl super::Story for AspectRatioStory {
    fn title() -> &'static str {
        "Aspect Ratio"
    }

    fn description() -> &'static str {
        "A box whose height follows its width, for media and previews."
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|_| Self)
    }
}

fn swatch(label: &'static str, cx: &App) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .bg(linear_gradient(
            135.,
            linear_color_stop(cx.theme().muted, 0.),
            linear_color_stop(cx.theme().secondary, 1.),
        ))
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(label)
}

impl Render for AspectRatioStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                section("Common ratios")
                    .description("Each box fills its width and sets its height from the ratio.")
                    .child(
                        h_flex()
                            .w_full()
                            .gap_4()
                            .items_start()
                            .child(
                                div()
                                    .flex_1()
                                    .child(AspectRatio::widescreen().child(swatch("16:9", cx))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(AspectRatio::photo().child(swatch("4:3", cx))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(AspectRatio::square().child(swatch("1:1", cx))),
                            ),
                    ),
            )
            .child(
                section("Square corners")
                    .description("Turn the rounding off where the box sits flush in a card.")
                    .child(
                        div().w(px(240.)).child(
                            AspectRatio::new(3. / 2.)
                                .rounded(false)
                                .child(swatch("3:2", cx)),
                        ),
                    ),
            )
    }
}
