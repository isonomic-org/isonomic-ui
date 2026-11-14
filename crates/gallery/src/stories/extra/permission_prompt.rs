//! Gallery story for the permission prompt.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui_extra::permission_prompt::{
    PermissionItem, PermissionPrompt, PermissionRisk, PermissionScope,
};

/// Shows a risky request with every choice, and a destructive one without "always".
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
                PermissionPrompt::new("refund", "Billing assistant", "refund 240.00")
                    .target("Payments, live")
                    .target_icon(IconName::Building2)
                    .risk(PermissionRisk::Destructive)
                    .reason("Two orders were charged twice during this morning's outage.")
                    .preview_title("Refunds")
                    .item(PermissionItem::new("Order 4821, Lena Fischer").meta("120.00"))
                    .item(PermissionItem::new("Order 4822, Tom Okafor").meta("120.00")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
