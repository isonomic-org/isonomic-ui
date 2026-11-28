// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A coding agent at work: the request and the tools it called, the tests it
//! ran, the run's status over the composer, and beside them the change to
//! review and the requests waiting for approval.

use std::time::Duration;

use crate::Story;
use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{ActiveTheme as _, IconName, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};
use isonomic_ui_extra::approval_stack::{
    ApprovalStack, PendingRequest, RequestRisk, StackDecision,
};
use isonomic_ui_extra::file_change_review::{
    ChangeKind, FileChange, FileChangeReview, Hunk, HunkLine,
};
use isonomic_ui_extra::prompt_composer::{
    ComposerMode, ContextItem, PromptComposer, PromptComposerEvent,
};
use isonomic_ui_extra::run_status_bar::{RunStatus, RunStatusBar, RunStatusButton};
use isonomic_ui_extra::terminal_output::{
    RunStatus as CommandStatus, TerminalLine, TerminalOutput,
};
use isonomic_ui_extra::tool_call_card::{ToolCallCard, ToolCallGroup, ToolCallStatus};

use super::window_frame;

const REQUEST: &str = "Two people booked the last cargo bike for the same Saturday. Find out \
                       how, fix it, and add a test that would have caught it.";

const FINDING: &str = "Found it: book() checks the bikes left and then saves the booking in a \
                       second query, so two requests a few milliseconds apart both see one bike \
                       free. I moved the check into the insert so the database refuses the \
                       second one, and added a test that books the last bike twice at once.";

fn change() -> Vec<FileChange> {
    vec![
        FileChange::new("booking", "src/booking.rs", ChangeKind::Modified)
            .hunk(Hunk::new("check", 58, 58).lines([
                HunkLine::context("pub async fn book(db: &Db, day: Date) -> Result<Booking> {"),
                HunkLine::removed("    if bikes_left(db, day).await? == 0 {"),
                HunkLine::removed("        return Err(Error::SoldOut);"),
                HunkLine::removed("    }"),
                HunkLine::removed("    db.insert_booking(day).await"),
                HunkLine::added("    // One statement, so two at once cannot both win."),
                HunkLine::added("    db.insert_if_free(day)"),
                HunkLine::added("        .await?"),
                HunkLine::added("        .ok_or(Error::SoldOut)"),
                HunkLine::context("}"),
            ]))
            .hunk(Hunk::new("log", 91, 90).lines([
                HunkLine::context("fn receipt(b: &Booking) -> String {"),
                HunkLine::removed("    format!(\"Booked {}\", b.bike)"),
                HunkLine::added("    format!(\"Booked {} for {}\", b.bike, b.day)"),
                HunkLine::context("}"),
            ])),
        FileChange::new("db", "src/db.rs", ChangeKind::Modified).hunk(
            Hunk::new("insert", 120, 120).lines([
                HunkLine::added("pub async fn insert_if_free(&self, day: Date)"),
                HunkLine::added("    -> Result<Option<Booking>> {"),
                HunkLine::added("    self.query_opt(INSERT_IF_FREE, day).await"),
                HunkLine::added("}"),
            ]),
        ),
        FileChange::new("test", "tests/race.rs", ChangeKind::Added).hunk(
            Hunk::new("race", 0, 1).lines([
                HunkLine::added("#[tokio::test]"),
                HunkLine::added("async fn last_bike_is_booked_once() {"),
                HunkLine::added("    let (a, b) = join!(book(&db, SAT), book(&db, SAT));"),
                HunkLine::added("    assert!(a.is_ok() ^ b.is_ok());"),
                HunkLine::added("}"),
            ]),
        ),
    ]
}

fn requests() -> Vec<PendingRequest> {
    vec![
        PendingRequest::new(
            "migrate",
            "Coding agent",
            "Run the migration on the staging database",
            RequestRisk::High,
        )
        .detail("Command", "sqlx migrate run")
        .detail("Database", "staging, 4 tables touched")
        .asked("just now"),
        PendingRequest::new(
            "push",
            "Coding agent",
            "Push fix/double-booking and open a pull request",
            RequestRisk::Medium,
        )
        .asked("1 min ago"),
        PendingRequest::new(
            "deps",
            "Coding agent",
            "Add a dev dependency for the async test",
            RequestRisk::Low,
        )
        .asked("3 min ago"),
    ]
}

fn modes() -> Vec<ComposerMode> {
    vec![
        ComposerMode::new("ask", "Ask")
            .icon(CatalogIcon::Chat)
            .hint("Answers without changing anything"),
        ComposerMode::new("plan", "Plan")
            .icon(CatalogIcon::Map)
            .hint("Writes a plan for you to approve"),
        ComposerMode::new("act", "Act")
            .icon(CatalogIcon::Flow)
            .hint("Makes the changes as it goes"),
    ]
}

/// A coding agent fixing a double booking in a bike rental's service.
pub struct CodingAgentScreenStory {
    composer: Entity<PromptComposer>,
    waiting: Vec<PendingRequest>,
    history: Vec<(PendingRequest, StackDecision)>,
    _events: Subscription,
}

impl CodingAgentScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer = cx.new(|cx| {
                PromptComposer::new("Tell the agent what to do next\u{2026}", window, cx)
            });
            composer.update(cx, |composer, cx| {
                composer.set_dictation(false, cx);
                composer.set_modes(modes(), Some("act".into()), cx);
                composer.set_context(
                    vec![
                        ContextItem::new("booking", "src/booking.rs", 3_400),
                        ContextItem::new("db", "src/db.rs", 5_100),
                        ContextItem::new("issue", "Issue 212: double booking", 900).pinned(true),
                    ],
                    Some(64_000),
                    cx,
                );
            });
            let events = cx.subscribe(&composer, |_: &mut Self, composer, event, cx| {
                if let PromptComposerEvent::SelectMode(id) = event {
                    let id = id.clone();
                    composer.update(cx, |composer, cx| composer.set_modes(modes(), Some(id), cx));
                }
            });
            Self {
                composer,
                waiting: requests(),
                history: Vec::new(),
                _events: events,
            }
        })
    }

    fn transcript(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .gap_4()
            .child(
                div()
                    .self_end()
                    .max_w(px(420.))
                    .px_3()
                    .py_2()
                    .rounded(theme.radius_lg)
                    .bg(theme.muted)
                    .text_sm()
                    .child(REQUEST),
            )
            .child(
                ToolCallGroup::new("agent-tools")
                    .default_open(true)
                    .call(
                        ToolCallCard::new("grep", "code.search", ToolCallStatus::Success)
                            .icon(IconName::Search)
                            .summary("Searched for where bookings are saved")
                            .arguments(r#"{"query":"insert_booking","path":"src/"}"#)
                            .result(r#"{"matches":[{"file":"src/booking.rs","line":62},{"file":"src/db.rs","line":88}]}"#)
                            .duration(Duration::from_millis(240)),
                    )
                    .call(
                        ToolCallCard::new("read", "file.read", ToolCallStatus::Success)
                            .icon(IconName::FileText)
                            .summary("Read src/booking.rs, lines 40 to 110")
                            .arguments(r#"{"path":"src/booking.rs","from":40,"to":110}"#)
                            .duration(Duration::from_millis(90)),
                    )
                    .call(
                        ToolCallCard::new("edit", "file.edit", ToolCallStatus::Success)
                            .icon(IconName::File)
                            .summary("Edited 3 files, +16 -5")
                            .arguments(r#"{"files":["src/booking.rs","src/db.rs","tests/race.rs"]}"#)
                            .duration(Duration::from_millis(1310)),
                    ),
            )
            .child(div().text_sm().child(FINDING))
            .child(
                TerminalOutput::new("agent-tests", "cargo test race")
                    .cwd("~/code/bike-rental")
                    .lines(vec![
                        TerminalLine::new("    Finished test profile in 6.81s"),
                        TerminalLine::new("     Running tests/race.rs"),
                        TerminalLine::new("test last_bike_is_booked_once ... ok"),
                        TerminalLine::new(""),
                        TerminalLine::info("test result: ok. 1 passed; 0 failed; finished in 0.42s"),
                    ])
                    .status(CommandStatus::Succeeded)
                    .exit_code(0)
                    .time(Duration::from_millis(7_230))
                    .max_height(px(150.)),
            )
    }
}

impl Story for CodingAgentScreenStory {
    fn title() -> &'static str {
        "Coding agent screen"
    }

    fn description() -> &'static str {
        "A coding agent built from the library: its tool calls and test run, the status over the composer, the change to review and the requests to approve."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CodingAgentScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transcript = self.transcript(cx);
        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;
        let decide = cx.entity();
        let undo = cx.entity();
        let last = self
            .history
            .last()
            .map(|(request, decision)| (SharedString::from(request.action.to_string()), *decision));

        let header = h_flex()
            .flex_none()
            .h(px(48.))
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(line)
            .child(div().text_sm().font_medium().child("bike-rental"))
            .child(
                div()
                    .text_sm()
                    .text_color(quiet)
                    .child("fix/double-booking"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("3 files changed, 1 test added"),
            );

        let work = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                div()
                    .id("agent-transcript")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .child(transcript),
            )
            .child(
                v_flex()
                    .flex_none()
                    .gap_2()
                    .px_4()
                    .pb_4()
                    .child(
                        RunStatusBar::new("agent-status", RunStatus::Running)
                            .step(5, Some(6))
                            .message("Waiting for approval to migrate staging")
                            .detail("Tests pass locally")
                            .elapsed(Duration::from_secs(134))
                            .button(RunStatusButton::new("Steer"))
                            .button(RunStatusButton::new("Stop")),
                    )
                    .child(self.composer.clone()),
            );

        let review = v_flex()
            .flex_none()
            .w(px(420.))
            .h_full()
            .border_l_1()
            .border_color(line)
            .child(
                div()
                    .id("agent-review")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .child(
                        FileChangeReview::new("agent-change", change())
                            .stacked()
                            .default_selected("booking"),
                    ),
            )
            .child(
                v_flex()
                    .flex_none()
                    .gap_2()
                    .p_4()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        div()
                            .text_xs()
                            .font_medium()
                            .text_color(quiet)
                            .child("Waiting for you"),
                    )
                    .child(
                        ApprovalStack::new("agent-approvals", self.waiting.clone())
                            .decided(self.history.len())
                            .last(last)
                            .on_decide(move |id, decision, _, cx| {
                                decide.update(cx, |this, cx| {
                                    if let Some(at) = this.waiting.iter().position(|r| &r.id == id)
                                    {
                                        let request = this.waiting.remove(at);
                                        this.history.push((request, decision));
                                    }
                                    cx.notify();
                                })
                            })
                            .on_undo(move |_, cx| {
                                undo.update(cx, |this, cx| {
                                    if let Some((request, _)) = this.history.pop() {
                                        this.waiting.insert(0, request);
                                    }
                                    cx.notify();
                                })
                            }),
                    ),
            );

        window_frame(cx).child(header).child(
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(work)
                .child(review),
        )
    }
}
