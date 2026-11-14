//! Gallery story for the step form wizard.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::step_form_wizard::{
    StepFormEvent, StepFormWizard, WizardChoice, WizardField, WizardStep,
};

/// Setting up a made-up bookkeeping workspace in three steps and a review.
pub struct StepFormWizardStory {
    wizard: Entity<StepFormWizard>,
    last: SharedString,
    _events: Subscription,
}

impl StepFormWizardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let wizard = cx.new(|cx| {
                StepFormWizard::new(cx)
                    .step(
                        WizardStep::new("Business")
                            .heading("Name your books")
                            .description("This is where the ledger and reports will live.")
                            .field(
                                WizardField::text("name", "Business name")
                                    .default("Harbour & Pine Books"),
                            )
                            .field(
                                WizardField::text("address", "Ledger address")
                                    .prefix("books.example/")
                                    .slug_of("name")
                                    .hint("Lowercase letters, numbers and dashes")
                                    .check(|slug| {
                                        (!slug.chars().all(|c| {
                                            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
                                        }))
                                        .then(|| "Use lowercase letters, numbers and dashes".into())
                                    }),
                            ),
                    )
                    .step(
                        WizardStep::new("People")
                            .heading("Bring in your accountant")
                            .description("They get an email with a link to join.")
                            .field(
                                WizardField::tags("people", "Email addresses")
                                    .placeholder("name@firm.example")
                                    .hint("Press Enter or a comma to add")
                                    .max(8)
                                    .optional()
                                    .review_empty("Only you for now"),
                            ),
                    )
                    .step(
                        WizardStep::new("Plan")
                            .heading("Pick a plan")
                            .description("Every plan starts with a month free.")
                            .field(
                                WizardField::choice("plan", "Plan")
                                    .default("practice")
                                    .option(
                                        WizardChoice::new("solo", "Solo")
                                            .aside("Free")
                                            .description("One business and one bank feed"),
                                    )
                                    .option(
                                        WizardChoice::new("practice", "Practice")
                                            .badge("Most picked")
                                            .aside("9.00 a month")
                                            .description("Unlimited feeds and payroll"),
                                    )
                                    .option(
                                        WizardChoice::new("firm", "Firm")
                                            .aside("29.00 a month")
                                            .description("Many businesses and client access"),
                                    ),
                            ),
                    )
                    .review(
                        "Review and create",
                        "Everything can be changed later in settings.",
                    )
                    .submit_label("Create books", "Creating\u{2026}")
                    .success(
                        "Your books are ready",
                        "Invites are on their way and the first bank feed is syncing.",
                    )
            });
            let events = cx.subscribe(&wizard, |this: &mut Self, wizard, event, cx| {
                this.last = match event {
                    StepFormEvent::StepChanged(step) => format!("Step {}", step + 1),
                    StepFormEvent::Submit(values) => {
                        // Pretend the work takes a moment, then succeed.
                        let wizard = wizard.clone();
                        cx.spawn(async move |_, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(1200))
                                .await;
                            wizard.update(cx, |wizard, cx| wizard.complete(Ok(()), cx));
                        })
                        .detach();
                        format!("Submitted {} values", values.len())
                    }
                    StepFormEvent::Reset => "Started over".to_string(),
                }
                .into();
                cx.notify();
            });
            Self {
                wizard,
                last: "Nothing yet".into(),
                _events: events,
            }
        })
    }
}

impl Render for StepFormWizardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.wizard.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
