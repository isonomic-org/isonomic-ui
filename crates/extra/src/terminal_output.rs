// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Terminal output: one command and what it printed, while it runs and after.
//!
//! The header shows the command, where it ran, and a pill with its state and
//! time. The output follows new lines until the reader scrolls up; then a
//! button offers the way back once more lines arrive. Error lines are tinted
//! and marked. The footer counts lines and carries copy and stop.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    clipboard::Clipboard,
    h_flex,
    rolling_number::RollingNumber,
    spinner::Spinner,
    tag::Tag,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// How close to the end still counts as following the output.
const FOLLOW_WITHIN: f32 = 24.;
/// How long the jump button takes to appear.
const ENTER: Duration = Duration::from_millis(200);

/// Where a line came from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Stream {
    /// Ordinary output.
    #[default]
    Output,
    /// Error output: tinted and marked.
    Error,
    /// A note about the run, shown quieter.
    Info,
}

/// One printed line.
#[derive(Debug, Clone)]
pub struct TerminalLine {
    text: SharedString,
    stream: Stream,
}

impl TerminalLine {
    /// A line of ordinary output.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            stream: Stream::Output,
        }
    }

    /// A line of error output.
    pub fn error(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            stream: Stream::Error,
        }
    }

    /// A quieter note about the run.
    pub fn info(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            stream: Stream::Info,
        }
    }
}

/// Where the command stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// Still running.
    Running,
    /// Ended well.
    Succeeded,
    /// Ended badly.
    Failed,
}

/// A time as the header shows it: "840ms", "12.4s".
pub fn format_duration(time: Duration) -> String {
    let ms = time.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", time.as_secs_f32())
    }
}

/// The pill's words: "Running", or "Exit 0".
pub fn status_text(status: RunStatus, exit_code: Option<i32>) -> String {
    match status {
        RunStatus::Running => "Running".to_string(),
        _ => format!("Exit {}", shown_exit_code(status, exit_code)),
    }
}

/// The exit code to show: the given one, else 0 for success and 1 for failure.
fn shown_exit_code(status: RunStatus, exit_code: Option<i32>) -> i32 {
    exit_code.unwrap_or(if status == RunStatus::Failed { 1 } else { 0 })
}

/// Everything printed, one line per row, as it is copied.
pub fn plain_text(lines: &[TerminalLine]) -> String {
    lines
        .iter()
        .map(|line| line.text.as_ref())
        .collect::<Vec<_>>()
        .join("\n")
}

/// What following the output looks like from one frame to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Follow {
    /// The view is kept at the end as lines arrive.
    stick: bool,
    /// How many lines there were when the reader scrolled away.
    seen: usize,
}

impl Follow {
    /// Take in this frame's scroll position and line count.
    fn step(self, at_end: bool, lines: usize) -> Self {
        let seen = if self.stick && !at_end {
            lines
        } else {
            self.seen
        };
        Self {
            stick: at_end,
            seen,
        }
    }

    /// Lines arrived that the reader has not scrolled down to.
    fn unseen(self, lines: usize) -> bool {
        !self.stick && lines > self.seen
    }
}

type StopHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A command and its output.
#[derive(IntoElement)]
pub struct TerminalOutput {
    id: ElementId,
    style: StyleRefinement,
    command: SharedString,
    cwd: Option<SharedString>,
    lines: Vec<TerminalLine>,
    status: RunStatus,
    exit_code: Option<i32>,
    time: Duration,
    max_height: Pixels,
    on_stop: Option<StopHandler>,
}

impl TerminalOutput {
    /// Create the panel for a command that is running.
    pub fn new(id: impl Into<ElementId>, command: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            command: command.into(),
            cwd: None,
            lines: Vec::new(),
            status: RunStatus::Running,
            exit_code: None,
            time: Duration::ZERO,
            max_height: px(280.),
            on_stop: None,
        }
    }

    /// The folder the command ran in.
    pub fn cwd(mut self, cwd: impl Into<SharedString>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// The lines printed so far.
    pub fn lines(mut self, lines: Vec<TerminalLine>) -> Self {
        self.lines = lines;
        self
    }

    /// Where the command stands.
    pub fn status(mut self, status: RunStatus) -> Self {
        self.status = status;
        self
    }

    /// The exit code. Without it, success shows 0 and failure 1.
    pub fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Time so far while running, or the final time. The host keeps it.
    pub fn time(mut self, time: Duration) -> Self {
        self.time = time;
        self
    }

    /// The height after which the output scrolls. 280px by default.
    pub fn max_height(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = height.into();
        self
    }

    /// Show a Stop button while the command runs.
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }
}

impl Styled for TerminalOutput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TerminalOutput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let mono = theme.mono_font_family.clone();
        let still = cx.reduce_motion();
        let running = self.status == RunStatus::Running;
        let count = self.lines.len();
        let time = format_duration(self.time);
        let words = status_text(self.status, self.exit_code);

        let handle = window
            .use_keyed_state(part("scroll"), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let follow = window.use_keyed_state(part("follow"), cx, |_, _| Follow {
            stick: true,
            seen: 0,
        });
        let scrolled = -f32::from(handle.offset().y);
        let most = f32::from(handle.max_offset().y);
        let at_end = most - scrolled < FOLLOW_WITHIN;
        let before = *follow.read(cx);
        let now = before.step(at_end, count);
        if now != before {
            follow.update(cx, |state, _| *state = now);
        }
        if now.stick {
            handle.scroll_to_bottom();
        }
        let unseen = now.unseen(count);

        let pill = {
            let mark = match self.status {
                RunStatus::Running => Spinner::new().xsmall().into_any_element(),
                RunStatus::Succeeded => Icon::new(CatalogIcon::CheckmarkFilled)
                    .size(px(14.))
                    .into_any_element(),
                RunStatus::Failed => Icon::new(CatalogIcon::ErrorFilled)
                    .size(px(14.))
                    .into_any_element(),
            };
            let tag = match self.status {
                RunStatus::Running => Tag::info(),
                RunStatus::Succeeded => Tag::success(),
                RunStatus::Failed => Tag::danger(),
            };
            tag.rounded_full()
                .h(px(28.))
                .px(px(10.))
                .gap(px(6.))
                .text_size(px(12.))
                .child(mark)
                .child(words.clone())
                .child(
                    div()
                        .font_family(mono.clone())
                        .opacity(0.7)
                        .child(format!("· {time}")),
                )
        };

        let title = h_flex()
            .font_family(mono.clone())
            .text_size(px(13.))
            .font_normal()
            .child(div().text_color(quiet).child("$ "))
            .child(div().min_w_0().truncate().child(self.command.clone()));
        let subtitle = self.cwd.clone().map(|cwd| {
            div()
                .font_family(mono.clone())
                .text_size(px(12.))
                .child(cwd)
                .into_any_element()
        });

        let rows = self.lines.iter().map(|line| {
            let error = line.stream == Stream::Error;
            h_flex()
                .items_start()
                .gap_2()
                .px_4()
                .when(error, |this| {
                    this.bg(danger.opacity(0.1)).text_color(danger)
                })
                .when(line.stream == Stream::Info, |this| this.text_color(quiet))
                .child(
                    div()
                        .w(px(8.))
                        .flex_none()
                        .child(if error { "!" } else { "" }),
                )
                .child(div().min_w_0().flex_1().child(if line.text.is_empty() {
                    SharedString::from(" ")
                } else {
                    line.text.clone()
                }))
        });

        let jump_handle = handle.clone();
        let jump_follow = follow.clone();
        let jump = unseen.then(|| {
            let button = div().absolute().right_3().bottom_3().child(
                Button::new(part("jump"))
                    .secondary()
                    .small()
                    .icon(IconName::ArrowDown)
                    .label("Jump to latest")
                    .h(px(28.))
                    .px_3()
                    .text_size(px(12.))
                    .on_click(move |_, window, cx| {
                        jump_handle.scroll_to_bottom();
                        jump_follow.update(cx, |state, _| state.stick = true);
                        window.refresh();
                    }),
            );
            if still {
                button.into_any_element()
            } else {
                button
                    .with_animation(part("jump-in"), Animation::new(ENTER), |this, time| {
                        let shown = 1. - (1. - time).powi(3);
                        this.opacity(shown).bottom(px(12. - 6. * (1. - shown)))
                    })
                    .into_any_element()
            }
        });

        let copy_text = plain_text(&self.lines);
        let stop = self.on_stop.clone().filter(|_| running).map(|stop| {
            Button::new(part("stop"))
                .danger()
                .icon(Icon::new(CatalogIcon::StopFilled))
                .label("Stop")
                .on_click(move |_, window, cx| stop(window, cx))
        });

        shell(cx)
            .id(self.id.clone())
            .max_w(px(576.))
            .role(Role::Region)
            .aria_label(format!(
                "Terminal: {}, {}, {}",
                self.command,
                words.to_lowercase(),
                time
            ))
            .child(shell_header(
                Some(Icon::new(IconName::SquareTerminal)),
                title,
                subtitle,
                Some(pill.into_any_element()),
                cx,
            ))
            .child(
                panel(cx)
                    .relative()
                    .child(
                        div()
                            .id(part("output"))
                            .max_h(self.max_height)
                            .overflow_y_scroll()
                            .track_scroll(&handle)
                            .on_scroll(|_, window, _| window.refresh())
                            .rounded(px(16.))
                            .py_3()
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .line_height(px(20.))
                            .role(Role::Log)
                            .aria_label(format!("Output of {}", self.command))
                            .when(count == 0, |this| {
                                this.child(
                                    div().px_4().text_color(quiet).child("Waiting for output…"),
                                )
                            })
                            .children(rows),
                    )
                    .children(jump),
            )
            .child(
                h_flex()
                    .min_h(px(44.))
                    .justify_between()
                    .gap_3()
                    .py(px(6.))
                    .pl(px(14.))
                    .pr(px(6.))
                    .child(
                        h_flex()
                            .gap_1()
                            .font_family(mono)
                            .text_size(px(12.))
                            .text_color(quiet)
                            .child(
                                RollingNumber::new(part("count"), count.to_string())
                                    .line_height(px(16.)),
                            )
                            .child(if count == 1 { "line" } else { "lines" }),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(Clipboard::new(part("copy")).label("Copy").value(copy_text))
                            .children(stop),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_in_milliseconds_then_seconds() {
        assert_eq!(format_duration(Duration::from_millis(0)), "0ms");
        assert_eq!(format_duration(Duration::from_millis(840)), "840ms");
        assert_eq!(format_duration(Duration::from_millis(1000)), "1.0s");
        assert_eq!(format_duration(Duration::from_millis(12_440)), "12.4s");
    }

    #[test]
    fn the_pill_names_the_state_and_the_exit_code() {
        assert_eq!(status_text(RunStatus::Running, None), "Running");
        assert_eq!(status_text(RunStatus::Succeeded, None), "Exit 0");
        assert_eq!(status_text(RunStatus::Failed, None), "Exit 1");
        assert_eq!(status_text(RunStatus::Failed, Some(137)), "Exit 137");
    }

    #[test]
    fn copying_takes_every_line_as_printed() {
        let lines = [
            TerminalLine::new("compiling"),
            TerminalLine::error("warning: unused"),
            TerminalLine::new(""),
            TerminalLine::info("done"),
        ];
        assert_eq!(plain_text(&lines), "compiling\nwarning: unused\n\ndone");
        assert_eq!(plain_text(&[]), "");
    }

    #[test]
    fn output_is_followed_until_the_reader_scrolls_away() {
        let start = Follow {
            stick: true,
            seen: 0,
        };
        // At the end with new lines: still following, nothing unseen.
        let following = start.step(true, 10);
        assert!(following.stick && !following.unseen(10));
        // Scrolled up at ten lines: those ten count as seen.
        let away = following.step(false, 10);
        assert!(!away.stick && away.seen == 10 && !away.unseen(10));
        // Two more arrive while away.
        let more = away.step(false, 12);
        assert!(more.seen == 10 && more.unseen(12));
        // Back at the end.
        let back = more.step(true, 12);
        assert!(back.stick && !back.unseen(12));
    }
}
