//! Gallery story for the onboarding checklist.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::onboarding_checklist::{ChecklistStep, OnboardingChecklist};

/// Shows a checklist to tick by hand next to one that plays by itself.
pub struct OnboardingChecklistStory {
    last: SharedString,
}

impl OnboardingChecklistStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing ticked yet".into(),
        })
    }

    fn steps() -> Vec<ChecklistStep> {
        vec![
            ChecklistStep::new("workspace", "Name the workspace").done(true),
            ChecklistStep::new("import", "Import last year's ledger").icon(IconName::FileText),
            ChecklistStep::new("team", "Invite the finance team").icon(IconName::User),
            ChecklistStep::new("alerts", "Choose which alerts to get").icon(IconName::Bell),
        ]
    }
}

impl Story for OnboardingChecklistStory {
    fn title() -> &'static str {
        "Onboarding checklist"
    }

    fn description() -> &'static str {
        "A short list of first steps with a progress bar."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for OnboardingChecklistStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        OnboardingChecklist::new("by-hand")
                            .steps(Self::steps())
                            .on_change(move |steps, _, cx| {
                                let ticked = steps.iter().filter(|(_, done)| *done).count();
                                let line = format!("{ticked} of {} ticked", steps.len());
                                view.update(cx, |this, cx| {
                                    this.last = line.into();
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        OnboardingChecklist::new("playing")
                            .title("Plays by itself")
                            .steps(Self::steps())
                            .auto_play(true),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
