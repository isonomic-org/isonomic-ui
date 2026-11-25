// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{
    IconName, Sizable as _,
    button::Button,
    timeline::{Timeline, TimelineItem, TimelineTone},
    v_flex,
};
use isonomic_ui::*;

use crate::section;

pub struct TimelineStory;

impl super::Story for TimelineStory {
    fn title() -> &'static str {
        "Timeline"
    }

    fn description() -> &'static str {
        "Events in order, each with a marker on a line that joins them."
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|_| Self)
    }
}

impl Render for TimelineStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                section("Order history")
                    .description(
                        "Newest first. The current step is emphasised; tones mark outcomes.",
                    )
                    .child(
                        div().w_full().max_w(px(440.)).child(
                            Timeline::new("order")
                                .accessibility_label("Order history")
                                .item(
                                    TimelineItem::new("Out for delivery")
                                        .time("Today, 09:14")
                                        .description("Expected by 18:00")
                                        .tone(TimelineTone::Current),
                                )
                                .item(
                                    TimelineItem::new("Payment received")
                                        .time("Yesterday, 16:02")
                                        .description("Visa ending 4417")
                                        .tone(TimelineTone::Success),
                                )
                                .item(
                                    TimelineItem::new("Address checked")
                                        .time("Yesterday, 15:40")
                                        .description("The postcode was corrected")
                                        .tone(TimelineTone::Warning),
                                )
                                .item(TimelineItem::new("Order placed").time("Yesterday, 15:31")),
                        ),
                    ),
            )
            .child(
                section("With icons and content")
                    .description("An icon in each marker, and any element under an event.")
                    .child(
                        div().w_full().max_w(px(440.)).child(
                            Timeline::new("document")
                                .accessibility_label("Document history")
                                .item(
                                    TimelineItem::new("Comment from Sam Rivera")
                                        .time("2 h ago")
                                        .icon(CatalogIcon::Chat)
                                        .description("“Can we move the launch to the 30th?”")
                                        .content(
                                            Button::new("reply").outline().xsmall().label("Reply"),
                                        ),
                                )
                                .item(
                                    TimelineItem::new("Shared with Finance")
                                        .time("3 h ago")
                                        .icon(IconName::User),
                                )
                                .item(
                                    TimelineItem::new("Upload failed")
                                        .time("4 h ago")
                                        .icon(IconName::CircleX)
                                        .tone(TimelineTone::Danger)
                                        .description("The file was larger than 25 MB"),
                                ),
                        ),
                    ),
            )
    }
}
