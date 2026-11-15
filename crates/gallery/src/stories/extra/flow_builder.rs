//! Gallery story for the flow builder.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use isonomic_ui_extra::flow_builder::{
    FlowBuilder, FlowStep, FlowStepKind, InsertAt, StepRun, insert_step, move_step, reading_order,
    remove_step,
};

/// Shows a collections flow for overdue invoices. The story plays the
/// session: it keeps the steps, applies what the builder asks for, and runs
/// a test that walks the steps, takes the No lane and fails on the last.
pub struct FlowBuilderStory {
    steps: Vec<FlowStep>,
    selected: Option<SharedString>,
    testing: bool,
    made: usize,
    _run: Option<Task<()>>,
}

fn starting_flow() -> Vec<FlowStep> {
    vec![
        FlowStep::new(
            "overdue",
            FlowStepKind::Trigger,
            "An invoice is 7 days overdue",
        )
        .detail("Checked every morning at 8:00"),
        FlowStep::new("big", FlowStepKind::Branch, "Is it over 1,000?").lanes(
            vec![
                FlowStep::new(
                    "call",
                    FlowStepKind::Action,
                    "Ask the account owner to call",
                )
                .detail("Creates a task for the owner"),
            ],
            vec![
                FlowStep::new("remind", FlowStepKind::Action, "Send the friendly reminder")
                    .detail("Uses the reminder template"),
                FlowStep::new("wait", FlowStepKind::Delay, "Wait 5 days"),
            ],
        ),
        FlowStep::new("note", FlowStepKind::Action, "Add a note to the ledger")
            .detail("Writes what was sent and when"),
    ]
}

fn set_run(steps: &mut [FlowStep], id: &str, run: StepRun) {
    for step in steps {
        if step.id.as_ref() == id {
            step.run = run.clone();
        }
        set_run(&mut step.yes, id, run.clone());
        set_run(&mut step.no, id, run.clone());
    }
}

impl FlowBuilderStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            steps: starting_flow(),
            selected: None,
            testing: false,
            made: 0,
            _run: None,
        })
    }

    fn test(&mut self, cx: &mut Context<Self>) {
        let order = reading_order(&self.steps);
        for id in &order {
            set_run(&mut self.steps, id, StepRun::Waiting);
        }
        self.testing = true;
        cx.notify();
        self._run = Some(cx.spawn(async move |this, cx| {
            // The test takes the No lane, and the last step fails.
            let yes_lane: Vec<SharedString> = this
                .read_with(cx, |this, _| {
                    this.steps
                        .iter()
                        .find(|s| s.id.as_ref() == "big")
                        .map(|s| reading_order(&s.yes))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let last = order.last().cloned();
            for id in order {
                if yes_lane.contains(&id) {
                    this.update(cx, |this, cx| {
                        set_run(&mut this.steps, &id, StepRun::Skipped);
                        cx.notify();
                    })
                    .ok();
                    continue;
                }
                this.update(cx, |this, cx| {
                    set_run(&mut this.steps, &id, StepRun::Running);
                    cx.notify();
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
                let run = if Some(&id) == last.as_ref() {
                    StepRun::Failed("The ledger is locked for March".into())
                } else if id.as_ref() == "big" {
                    StepRun::Passed("No: 640.00 is under 1,000".into())
                } else {
                    StepRun::Passed("Done".into())
                };
                this.update(cx, |this, cx| {
                    set_run(&mut this.steps, &id, run);
                    cx.notify();
                })
                .ok();
            }
            this.update(cx, |this, cx| {
                this.testing = false;
                cx.notify();
            })
            .ok();
        }));
    }
}

impl Story for FlowBuilderStory {
    fn title() -> &'static str {
        "Flow builder"
    }

    fn description() -> &'static str {
        "An automation's steps from top to bottom, with branches and a test run."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FlowBuilderStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let (add, remove, select, moved, test) = (
            story.clone(),
            story.clone(),
            story.clone(),
            story.clone(),
            story,
        );
        v_flex()
            .w_full()
            .gap_3()
            .child(
                FlowBuilder::new("chase-flow", self.steps.clone())
                    .title("Chase overdue invoices")
                    .selected(self.selected.clone())
                    .testing(self.testing)
                    .on_add(move |at: &InsertAt, kind, _, cx| {
                        add.update(cx, |this, cx| {
                            this.made += 1;
                            let title = match kind {
                                FlowStepKind::Branch => "New question",
                                FlowStepKind::Delay => "Wait 1 day",
                                _ => "New action",
                            };
                            let id: SharedString = format!("new-{}", this.made).into();
                            let step = FlowStep::new(id.clone(), kind, title);
                            if insert_step(&mut this.steps, at, step) {
                                this.selected = Some(id);
                            }
                            cx.notify();
                        })
                    })
                    .on_remove(move |id, _, cx| {
                        remove.update(cx, |this, cx| {
                            remove_step(&mut this.steps, id);
                            this.selected = None;
                            cx.notify();
                        })
                    })
                    .on_select(move |id, _, cx| {
                        select.update(cx, |this, cx| {
                            this.selected = Some(id.clone());
                            cx.notify();
                        })
                    })
                    .on_move(move |id, up, _, cx| {
                        moved.update(cx, |this, cx| {
                            move_step(&mut this.steps, id, up);
                            cx.notify();
                        })
                    })
                    .on_test(move |_, cx| test.update(cx, |this, cx| this.test(cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Select a step to move or remove it. Test walks the flow."),
            )
    }
}
