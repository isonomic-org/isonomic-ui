//! Gallery story for the task inbox board.

use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::task_inbox_board::{InboxStatus, InboxStep, InboxTask, TaskInboxBoard};

/// Background bookkeeping tasks. Approve, Reject and Retry are left to the
/// board itself, so it changes them in place; Open reports back here.
pub struct TaskInboxBoardStory {
    started: Instant,
    last: SharedString,
}

impl TaskInboxBoardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            started: Instant::now(),
            last: "Nothing opened yet".into(),
        })
    }
}

impl Render for TaskInboxBoardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let ago = |secs| {
            self.started
                .checked_sub(Duration::from_secs(secs))
                .unwrap_or(self.started)
        };

        let board = TaskInboxBoard::new("books-inbox")
            .title("Background work")
            .task(
                InboxTask::new("bank", "Import the bank feed", InboxStatus::Failed)
                    .description("Read 412 lines, then the bank's export stopped halfway through the 14th.")
                    .agent("ledger-small")
                    .usage(4, 2)
                    .step(InboxStep::new("Downloaded the March statement", "9m ago"))
                    .step(InboxStep::new("Read 412 of 768 lines", "8m ago"))
                    .step(InboxStep::new("The file ended early on the 14th", "7m ago")),
            )
            .task(
                InboxTask::new("writeoff", "Write off old receivables", InboxStatus::Waiting)
                    .description("Seven invoices over 180 days, 3,410.00 in total. Wants to post the write-off.")
                    .agent("ledger-large")
                    .started(ago(260))
                    .usage(6, 3)
                    .step(InboxStep::new("Listed invoices over 180 days", "4m ago"))
                    .step(InboxStep::new("Checked none are in dispute", "3m ago"))
                    .step(InboxStep::new("Wants to post the write-off", "now")),
            )
            .task(
                InboxTask::new("vat", "Prepare the VAT return", InboxStatus::Running)
                    .description("Totalling sales and purchases for the quarter, then filling the boxes.")
                    .agent("ledger-large")
                    .started(ago(361))
                    .usage(14, 9)
                    .step(InboxStep::new("Pulled the quarter's ledgers", "5m ago"))
                    .step(InboxStep::new("Filling in the boxes", "now")),
            )
            .task(
                InboxTask::new("fx", "Revalue foreign balances", InboxStatus::Running)
                    .description("Applying month-end rates to the euro and dollar accounts.")
                    .agent("ledger-small")
                    .started(ago(83))
                    .usage(5, 2),
            )
            .task(
                InboxTask::new("pack", "Send the weekly cash note", InboxStatus::Done)
                    .description("Pulled balances from three accounts and sent the note to the finance lead.")
                    .agent("ledger-small")
                    .elapsed(Duration::from_secs(252))
                    .usage(8, 3)
                    .step(InboxStep::new("Pulled balances from 3 accounts", "22m ago"))
                    .step(InboxStep::new("Sent to the finance lead", "18m ago")),
            )
            .on_open(move |task, _, cx| {
                story.update(cx, |this, cx| {
                    this.last = format!("Opened {}", task.title()).into();
                    cx.notify();
                })
            });

        v_flex().gap_3().child(board).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
