// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::Duration;

use isonomic_ui::component::{
    ActiveTheme as _, StyledExt as _,
    formatted_value::{
        FormattedValue, format_bytes, format_compact, format_duration, format_number,
        format_percent, format_relative_time,
    },
    h_flex, v_flex,
};
use isonomic_ui::*;

use crate::section;

pub struct FormattedValueStory;

impl super::Story for FormattedValueStory {
    fn title() -> &'static str {
        "Formatted Values"
    }

    fn description() -> &'static str {
        "Numbers, sizes, durations and times written the way people read them."
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|_| Self)
    }
}

fn row(name: &'static str, input: String, output: String, cx: &App) -> impl IntoElement {
    h_flex()
        .w_full()
        .gap_4()
        .py_1p5()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .child(
            div()
                .w(px(140.))
                .text_color(cx.theme().muted_foreground)
                .child(name),
        )
        .child(
            div()
                .flex_1()
                .text_color(cx.theme().muted_foreground)
                .child(input),
        )
        .child(div().child(output))
}

impl Render for FormattedValueStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = chrono::Utc::now();
        v_flex()
            .gap_3()
            .child(
                section("Formatters")
                    .description(
                        "Plain functions, so the same text works in labels, cells and tooltips.",
                    )
                    .v_flex()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(520.))
                            .child(row(
                                "Number",
                                "1234567.891, 2 places".into(),
                                format_number(1_234_567.891, 2),
                                cx,
                            ))
                            .child(row("Compact", "1234".into(), format_compact(1_234.), cx))
                            .child(row(
                                "Compact",
                                "3400000".into(),
                                format_compact(3_400_000.),
                                cx,
                            ))
                            .child(row("Bytes", "1536".into(), format_bytes(1536), cx))
                            .child(row(
                                "Bytes",
                                "3435973837".into(),
                                format_bytes(3_435_973_837),
                                cx,
                            ))
                            .child(row(
                                "Percent",
                                "0.4251, 1 place".into(),
                                format_percent(0.4251, 1),
                                cx,
                            ))
                            .child(row(
                                "Duration",
                                "3840 s".into(),
                                format_duration(Duration::from_secs(3_840)),
                                cx,
                            ))
                            .child(row(
                                "Relative time",
                                "5 minutes ago".into(),
                                format_relative_time(now - chrono::Duration::minutes(5), now),
                                cx,
                            ))
                            .child(row(
                                "Relative time",
                                "4 days ago".into(),
                                format_relative_time(now - chrono::Duration::days(4), now),
                                cx,
                            )),
                    ),
            )
            .child(
                section("Short on screen, full for screen readers")
                    .description("“12.3K” is read as “12,345 followers”.")
                    .child(
                        h_flex()
                            .gap_2()
                            .items_baseline()
                            .child(
                                FormattedValue::new("followers", format_compact(12_345.))
                                    .full_text("12,345 followers")
                                    .text_2xl()
                                    .font_semibold(),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("followers"),
                            ),
                    ),
            )
    }
}
