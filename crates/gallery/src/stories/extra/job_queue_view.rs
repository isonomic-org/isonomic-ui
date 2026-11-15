//! Gallery story for the job queue view.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::job_queue_view::{JobQueueView, WaitingJob, WorkerSlot};

/// A month-end backlog spread over four workers, one offline. The story
/// moves and cancels jobs.
pub struct JobQueueViewStory {
    jobs: Vec<WaitingJob>,
}

fn jobs() -> Vec<WaitingJob> {
    vec![
        WaitingJob::new("j1", "Post the payroll journal", 540).urgent(),
        WaitingJob::new("j2", "Match the savings account", 420),
        WaitingJob::new("j3", "Draft reminders for 7 overdue invoices", 380),
        WaitingJob::new("j4", "File 43 receipts from the inbox", 300),
        WaitingJob::new("j5", "Refresh vendor balances", 120),
        WaitingJob::new("j6", "Write the close note", 45),
    ]
}

impl JobQueueViewStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { jobs: jobs() })
    }
}

impl Story for JobQueueViewStory {
    fn title() -> &'static str {
        "Job queue view"
    }

    fn description() -> &'static str {
        "The work waiting, the workers taking it, and how deep the backlog has run."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for JobQueueViewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let front = cx.entity();
        let cancel = cx.entity();
        JobQueueView::new(
            "close-queue",
            vec![
                WorkerSlot::new("w1", "Worker 1").busy("Match card payouts", 0.62),
                WorkerSlot::new("w2", "Worker 2").busy("Pull the bank statement", 0.3),
                WorkerSlot::new("w3", "Worker 3"),
                WorkerSlot::new("w4", "Worker 4").offline(),
            ],
            self.jobs.clone(),
        )
        .depth(vec![2, 3, 5, 8, 9, 11, 10, 9, 8, 8, 7, 6])
        .typical(90)
        .on_front(move |id, _, cx| {
            front.update(cx, |this, cx| {
                if let Some(at) = this.jobs.iter().position(|j| &j.id == id) {
                    let job = this.jobs.remove(at);
                    this.jobs.insert(0, job);
                }
                cx.notify();
            })
        })
        .on_cancel(move |id, _, cx| {
            cancel.update(cx, |this, cx| {
                this.jobs.retain(|j| &j.id != id);
                cx.notify();
            })
        })
    }
}
