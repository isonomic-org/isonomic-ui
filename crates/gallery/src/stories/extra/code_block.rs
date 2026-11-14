//! Gallery story for the code block.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use gpui_component::{ActiveTheme as _, v_flex};
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

/// Shows a numbered file with marked lines, a one-line command, and wrapped text that scrolls.
pub struct CodeBlockStory {
    copies: usize,
}

impl CodeBlockStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { copies: 0 })
    }
}

impl Render for CodeBlockStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        v_flex()
            .w(px(480.))
            .gap_4()
            .child(
                CodeBlock::new("file", RETRY)
                    .language("rust")
                    .filename("src/retry.rs")
                    .line_numbers(true)
                    .highlight_lines([11, 12])
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
                    .max_height(px(96.))
                    .copyable(false),
            )
    }
}
