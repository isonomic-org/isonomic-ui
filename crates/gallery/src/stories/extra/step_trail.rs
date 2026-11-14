//! Gallery story for the step trail.

use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui_extra::step_trail::{StepTrail, SubstepState, TrailState, TrailStep, TrailSubstep};

/// How a person answered the payment step.
#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Waiting,
    Approved,
    Declined,
}

/// An agent chasing overdue invoices. The step it is on has a long list of
/// sub-steps, and the payment run at the end waits for a person.
pub struct StepTrailStory {
    answer: Answer,
    last: SharedString,
    chase_started: Instant,
    pay_started: Instant,
}

impl StepTrailStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        let now = Instant::now();
        cx.new(|_| Self {
            answer: Answer::Waiting,
            last: "Nothing yet".into(),
            chase_started: now.checked_sub(Duration::from_secs(41)).unwrap_or(now),
            pay_started: now,
        })
    }
}

impl Render for StepTrailStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let (approve_story, decline_story) = (story.clone(), story);
        let secs = Duration::from_secs;

        let chase = TrailStep::new("chase", "Draft reminders for overdue invoices")
            .icon(IconName::Inbox)
            .running("Writing", self.chase_started)
            .substep(TrailSubstep::new("Found 9 invoices past 30 days").detail("aging.csv"))
            .substep(TrailSubstep::new("Skipped two on payment plans"))
            .substep(TrailSubstep::new("Northwind, gentle reminder").detail("#4471"))
            .substep(TrailSubstep::new("Harbor Supply, second notice").detail("#4480"))
            .substep(TrailSubstep::new("Kestrel Foods, gentle reminder").detail("#4492"))
            .substep(
                TrailSubstep::new("Alder & Finch, second notice")
                    .state(SubstepState::Running)
                    .detail("ledgers/2026/receivables/alder-finch.csv:88"),
            )
            .substep(TrailSubstep::new("Moss Lane Studio").state(SubstepState::Pending))
            .substep(TrailSubstep::new("Pinecrest Dental").state(SubstepState::Pending))
            .substep(TrailSubstep::new("Quay Street Bakery").state(SubstepState::Pending));

        let pay = TrailStep::new("pay", "Schedule this week's supplier payments")
            .icon(IconName::Calendar)
            .gate(true)
            .ask("Pay 6 suppliers from the operating account, 18,240.00 in total, on Thursday");
        let pay = match self.answer {
            Answer::Waiting => pay.state(TrailState::Waiting),
            Answer::Approved => pay.running("Scheduling", self.pay_started),
            Answer::Declined => pay.state(TrailState::Declined),
        };

        v_flex()
            .gap_3()
            .child(
                StepTrail::new("collections-trail")
                    .title("Tidy up receivables")
                    .agent("Ledger assistant")
                    .step(
                        TrailStep::new("sync", "Sync the bank feed")
                            .icon(IconName::RefreshCw)
                            .done(secs(12)),
                    )
                    .step(
                        TrailStep::new("match", "Match payments to invoices")
                            .icon(IconName::Search)
                            .done(secs(94))
                            .substep(TrailSubstep::new("Matched 131 of 138").detail("95%"))
                            .substep(TrailSubstep::new("Left 7 for review")),
                    )
                    .step(chase)
                    .step(
                        TrailStep::new("note", "Write the weekly cash note")
                            .icon(IconName::FileText),
                    )
                    .step(pay)
                    .on_approve(move |id, _, cx| {
                        approve_story.update(cx, |this, cx| {
                            this.answer = Answer::Approved;
                            this.pay_started = Instant::now();
                            this.last = format!("Approved {id}").into();
                            cx.notify();
                        })
                    })
                    .on_decline(move |id, _, cx| {
                        decline_story.update(cx, |this, cx| {
                            this.answer = Answer::Declined;
                            this.last = format!("Declined {id}").into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
