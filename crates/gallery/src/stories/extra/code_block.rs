// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the code block.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::code_block::CodeBlock;

const RETRY: &str = r#"use std::time::Duration;

pub struct Retry {
    attempts: u32,
    wait: Duration,
}

impl Retry {
    // Double the wait after every failed attempt.
    pub fn next(&mut self) -> Option<Duration> {
        self.attempts = self.attempts.checked_sub(1)?;
        self.wait *= 2;
        Some(self.wait)
    }
}
"#;

const LOG: &str = "2026-03-02T08:14:07Z worker-3 picked up job 4417 after 212 ms in the queue and finished it in 1.84 s\n2026-03-02T08:14:09Z worker-1 retried job 4409 because the upstream service answered with a timeout";

const SCHEDULE: &str = r#"[pumps.north]
start = "06:00"
stop = "18:30"
flow_litres_per_minute = 240
alarm_below = 40

[pumps.south]
start = "05:30"
stop = "19:00"
flow_litres_per_minute = 310
alarm_below = 55

[pumps.east]
start = "07:00"
stop = "17:00"
flow_litres_per_minute = 180
alarm_below = 30

[pumps.west]
start = "06:15"
stop = "18:45"
flow_litres_per_minute = 205
alarm_below = 35

[reports]
send_to = ["ops@example.test", "night-shift@example.test"]
every_minutes = 15
"#;

/// Shows a numbered file with marked lines and a language tag, a one-line
/// command, wrapped text with a wrap toggle, and a long file folded to its
/// first lines.
pub struct CodeBlockStory {
    copies: usize,
}

impl CodeBlockStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { copies: 0 })
    }
}

impl Story for CodeBlockStory {
    fn title() -> &'static str {
        "Code block"
    }

    fn description() -> &'static str {
        "Code to read and copy."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CodeBlockStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        v_flex()
            .w_full()
            .max_w(px(480.))
            .gap_4()
            .child(
                CodeBlock::new("file", RETRY)
                    .language("rust")
                    .filename("src/retry.rs")
                    .line_numbers(true)
                    .highlight_lines([11, 12])
                    .language_tag(true)
                    .on_copy(move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            this.copies += 1;
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Copied {} times", self.copies)),
            )
            .child(CodeBlock::new("command", "cargo run --example gallery").language("bash"))
            .child(
                CodeBlock::new("log", LOG)
                    .wrap(true)
                    .wrap_toggle(true)
                    .max_height(px(96.))
                    .copyable(false),
            )
            .child(
                CodeBlock::new("schedule", SCHEDULE)
                    .language("toml")
                    .filename("pumps.toml")
                    .language_tag(true)
                    .line_numbers(true)
                    .wrap_toggle(true)
                    .fold_after(8),
            )
    }
}
