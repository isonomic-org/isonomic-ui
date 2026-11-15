//! Gallery story for the autonomy level control.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::autonomy_level_control::{
    ActionRisk, AgentAction, AutonomyLevel, AutonomyLevelControl,
};

/// Shows a bookkeeping assistant's actions and reports every change.
pub struct AutonomyLevelControlStory {
    last: SharedString,
}

impl AutonomyLevelControlStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Paying suppliers starts locked".into(),
        })
    }
}

impl Story for AutonomyLevelControlStory {
    fn title() -> &'static str {
        "Autonomy level control"
    }

    fn description() -> &'static str {
        "How much an assistant may do without asking."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AutonomyLevelControlStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let levels = cx.entity();
        let locks = levels.clone();

        v_flex()
            .gap_3()
            .child(
                AutonomyLevelControl::new("bookkeeper")
                    .title("Bookkeeping assistant")
                    .action(
                        AgentAction::new(
                            "read",
                            "Read the ledger and statements",
                            ActionRisk::Safe,
                        )
                        .icon(IconName::BookOpen),
                    )
                    .action(
                        AgentAction::new("draft", "Draft reminders", ActionRisk::Safe)
                            .icon(IconName::FileText),
                    )
                    .action(
                        AgentAction::new(
                            "categorise",
                            "Categorise transactions",
                            ActionRisk::Risky,
                        )
                        .icon(IconName::Replace),
                    )
                    .action(
                        AgentAction::new("send", "Send reminders to customers", ActionRisk::Risky)
                            .icon(IconName::Inbox),
                    )
                    .action(
                        AgentAction::new("void", "Void invoices", ActionRisk::Destructive)
                            .icon(IconName::Delete),
                    )
                    .action(
                        AgentAction::new("pay", "Pay suppliers", ActionRisk::Destructive)
                            .detail("Up to the daily limit")
                            .icon(IconName::Building2),
                    )
                    .default_locked(["pay"])
                    .on_level_change(move |level, _, cx| {
                        let line = match level {
                            AutonomyLevel::AskEveryTime => "Level: ask every time",
                            AutonomyLevel::AskWhenRisky => "Level: ask when risky",
                            AutonomyLevel::RunFreely => "Level: run freely",
                        };
                        levels.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    })
                    .on_locked_change(move |locked, _, cx| {
                        let line = format!("{} locked", locked.len());
                        locks.update(cx, |this, cx| {
                            this.last = line.into();
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
