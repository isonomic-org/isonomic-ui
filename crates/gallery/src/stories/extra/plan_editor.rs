//! Gallery story for the plan editor.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::plan_editor::{PlanEditor, PlanEditorEvent, PlanStep, PlanStepStatus};

/// How long each pretend step takes.
const STEP_FOR: Duration = Duration::from_millis(1400);

/// A month-end close plan. Pressing Run plays a pretend run in which the
/// report step fails once, so Try again can be seen.
pub struct PlanEditorStory {
    editor: Entity<PlanEditor>,
    last: SharedString,
    failed_once: bool,
    _run: Option<Task<()>>,
    _events: Subscription,
}

impl PlanEditorStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let editor = cx.new(|cx| {
                PlanEditor::new(window, cx)
                    .title("Close the March books")
                    .estimate("about 1 min")
                    .cancellable(true)
                    .step(
                        PlanStep::new("fetch", "Fetch bank statements")
                            .detail("Both current accounts and the card")
                            .tool("bank.export"),
                    )
                    .step(
                        PlanStep::new("match", "Match payments to invoices")
                            .detail("Leave anything unsure for review")
                            .tool("ledger.match"),
                    )
                    .step(
                        PlanStep::new("accrue", "Book accruals")
                            .detail("Rent, payroll and the audit fee")
                            .tool("ledger.journal"),
                    )
                    .step(
                        PlanStep::new("report", "Draft the management report")
                            .detail("Two pages with the margin bridge")
                            .tool("docs.write"),
                    )
                    .step(PlanStep::new("send", "Send it to the finance lead").tool("mail.draft"))
            });
            let events = cx.subscribe_in(
                &editor,
                window,
                |this: &mut Self, _, event: &PlanEditorEvent, window, cx| {
                    match event {
                        PlanEditorEvent::Edited(steps) => {
                            this.last = format!("Edited, {} steps", steps.len()).into();
                        }
                        PlanEditorEvent::Run(steps) => {
                            this.last = format!("Run pressed with {} steps", steps.len()).into();
                            this.failed_once = false;
                            this.editor.update(cx, |editor, cx| editor.start(cx));
                            this.play(0, window, cx);
                        }
                        PlanEditorEvent::Cancel => this.last = "Cancelled".into(),
                        PlanEditorEvent::Retry(id) => {
                            this.last = format!("Trying again from {id}").into();
                            let from = this
                                .editor
                                .read(cx)
                                .plan()
                                .iter()
                                .position(|step| step.id() == id)
                                .unwrap_or(0);
                            this.play(from, window, cx);
                        }
                    }
                    cx.notify();
                },
            );
            Self {
                editor,
                last: "Nothing yet".into(),
                failed_once: false,
                _run: None,
                _events: events,
            }
        })
    }

    /// Pretend to carry out the plan from step `from`.
    fn play(&mut self, from: usize, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<SharedString> = self
            .editor
            .read(cx)
            .plan()
            .iter()
            .map(|step| step.id().clone())
            .collect();
        let editor = self.editor.clone();
        self._run = Some(cx.spawn_in(window, async move |this, cx| {
            for id in ids.into_iter().skip(from) {
                editor.update(cx, |editor, cx| {
                    editor.mark(&id, PlanStepStatus::Running, cx)
                });
                cx.background_executor().timer(STEP_FOR).await;
                let fail = this
                    .update(cx, |this, _| {
                        let fail = id == "report" && !this.failed_once;
                        this.failed_once |= fail;
                        fail
                    })
                    .unwrap_or(false);
                editor.update(cx, |editor, cx| {
                    if fail {
                        editor.fail(&id, "The report template is open elsewhere", cx);
                    } else {
                        editor.mark(&id, PlanStepStatus::Done, cx);
                    }
                });
                if fail {
                    return;
                }
            }
        }));
    }
}

impl Render for PlanEditorStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.editor.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
