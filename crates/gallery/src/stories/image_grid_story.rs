// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use isonomic_ui::component::{
    ActiveTheme as _, Icon, StyledExt as _,
    image_grid::{ImageGrid, ImageGridItem},
    v_flex,
};
use isonomic_ui::*;

use crate::section;

pub struct ImageGridStory {
    opened: Option<usize>,
}

impl super::Story for ImageGridStory {
    fn title() -> &'static str {
        "Image Grid"
    }

    fn description() -> &'static str {
        "Images in equal tiles, with a count for the ones that do not fit."
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|_| Self { opened: None })
    }
}

/// A stand-in photo: a gradient with a picture icon, so the story needs no
/// network.
fn photo(ix: usize, cx: &App) -> AnyElement {
    let hue = (ix as f32 * 0.13) % 1.;
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(linear_gradient(
            160.,
            linear_color_stop(hsla(hue, 0.35, 0.82, 1.), 0.),
            linear_color_stop(hsla((hue + 0.08) % 1., 0.4, 0.62, 1.), 1.),
        ))
        .child(
            Icon::new(isonomic_ui::assets::IconName::Image)
                .size_6()
                .text_color(cx.theme().background.opacity(0.8)),
        )
        .into_any_element()
}

fn photos(count: usize, cx: &App) -> Vec<ImageGridItem> {
    (0..count)
        .map(|ix| ImageGridItem::element(photo(ix, cx), format!("Photo {}", ix + 1)))
        .collect()
}

impl Render for ImageGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opened = match self.opened {
            Some(ix) => format!("Opened photo {}", ix + 1),
            None => "No photo opened yet".into(),
        };
        v_flex()
            .gap_3()
            .child(
                section("Gallery")
                    .description("Three square columns. Press a photo; Tab and Enter work too.")
                    .v_flex()
                    .gap_2()
                    .child(
                        div().w_full().max_w(px(480.)).child(
                            ImageGrid::new("gallery")
                                .accessibility_label("Holiday photos")
                                .items(photos(6, cx))
                                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                    this.opened = Some(*ix);
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(opened),
                    ),
            )
            .child(
                section("Attachments")
                    .description("Two wide columns, at most four tiles; the last counts the rest.")
                    .child(
                        div().w(px(320.)).child(
                            ImageGrid::new("attachments")
                                .accessibility_label("Attachments")
                                .columns(2)
                                .ratio(4. / 3.)
                                .max_visible(4)
                                .items(photos(7, cx)),
                        ),
                    ),
            )
    }
}
