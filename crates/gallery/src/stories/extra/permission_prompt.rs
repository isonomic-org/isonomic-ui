//! Gallery story for the permission prompt.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::permission_prompt::{
    PermissionItem, PermissionPrompt, PermissionRisk, PermissionScope,
};

/// Shows a risky request with every choice, and a destructive one that cannot be allowed for good.
pub struct PermissionPromptStory {
    last: SharedString,
}

impl PermissionPromptStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing answered yet".into(),
        })
    }
}

impl Story for PermissionPromptStory {
    fn title() -> &'static str {
        "Permission prompt"
    }

    fn description() -> &'static str {
        "An assistant asks for one specific right."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PermissionPromptStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();

        v_flex()
            .gap_4()
            .child(
                PermissionPrompt::new("reminders", "Collections assistant", "send 14 reminders")
                    .target("Outgoing mail")
                    .target_icon(IconName::Inbox)
                    .reason("These invoices are more than two weeks overdue and had one reminder.")
                    .preview_title("Recipients")
                    .item(PermissionItem::new("Harbour Supply").meta("1,240.50"))
                    .item(PermissionItem::new("Northgate Print").meta("310.00"))
                    .item(PermissionItem::new("Quay Street Bakery").meta("87.25"))
                    .item(PermissionItem::new("Eleven more customers").meta("16,782.25"))
                    .duration(Duration::from_secs(90))
                    .default_scope(PermissionScope::Timed)
                    .on_decision_change(move |decision, _, cx| {
                        let line = match decision {
                            Some(decision) if decision.allow => "Reminders allowed",
                            Some(_) => "Reminders denied",
                            None => "Reminders: asked again",
                        };
                        story.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                PermissionPrompt::new("purge", "Archive assistant", "delete 3 closed ledgers")
                    .target("Company files")
                    .target_icon(IconName::Folder)
                    .risk(PermissionRisk::Destructive)
                    .reason("The retention period for these years ended in January.")
                    .preview_title("Ledgers")
                    .item(PermissionItem::new("General ledger 2017").meta("4.1 MB"))
                    .item(PermissionItem::new("General ledger 2018").meta("4.6 MB"))
                    .item(PermissionItem::new("General ledger 2019").meta("5.2 MB")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
