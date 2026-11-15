//! Gallery story for the row actions menu.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::row_actions_menu::{RowAction, RowActionsMenu};

/// Three invoices with their actions. The story says which action ran.
pub struct RowActionsMenuStory {
    ran: Option<String>,
}

fn actions(paid: bool) -> Vec<RowAction> {
    let remind = RowAction::new("remind", "Send", "Send a reminder");
    vec![
        RowAction::new("resend", "Send", "Send the invoice again"),
        if paid {
            remind.unavailable("Already paid")
        } else {
            remind
        },
        RowAction::new("pdf", "Export", "Download as PDF"),
        RowAction::new("copy", "Export", "Copy the payment link"),
        RowAction::new("void", "Remove", "Void the invoice").destructive(),
    ]
}

impl RowActionsMenuStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { ran: None })
    }
}

impl Story for RowActionsMenuStory {
    fn title() -> &'static str {
        "Row actions menu"
    }

    fn description() -> &'static str {
        "One list of actions for a row, opened by right-click on the row or from its more button."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RowActionsMenuStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = [
            (
                "inv-2291",
                "Invoice 2291, Northwind Dental",
                "$3,120.00, 41 days overdue",
                false,
            ),
            (
                "inv-2302",
                "Invoice 2302, Juniper Cafe",
                "$480.00, due in 5 days",
                false,
            ),
            (
                "inv-2240",
                "Invoice 2240, Harbour Supply",
                "$1,240.50, paid",
                true,
            ),
        ];
        let list: Vec<_> = rows
            .into_iter()
            .map(|(id, title, detail, paid)| {
                let story = cx.entity();
                RowActionsMenu::new(id, title, detail, actions(paid)).on_run(
                    move |action, _, cx| {
                        story.update(cx, |this, cx| {
                            this.ran = Some(format!("{action} on {id}"));
                            cx.notify();
                        })
                    },
                )
            })
            .collect();
        v_flex()
            .w_full()
            .max_w(isonomic_ui::px(520.))
            .gap_1()
            .children(list)
            .child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match &self.ran {
                        Some(ran) => format!("Ran: {ran}"),
                        None => "Right-click a row, or use its \u{2026} button.".into(),
                    }),
            )
    }
}
