// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the integration picker.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::integration_picker::{
    Integration, IntegrationPicker, IntegrationPickerEvent,
};

/// The integrations page of a bookkeeping app's command menu, with ten
/// made-up services, three of them connected.
pub struct IntegrationPickerStory {
    picker: Entity<IntegrationPicker>,
    last: SharedString,
    _events: Subscription,
}

impl IntegrationPickerStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let picker = cx.new(|cx| {
                IntegrationPicker::new("Isonomic", window, cx)
                    .with_back(true)
                    .service(
                        Integration::new("bank", "Bankfeed", "Banking")
                            .domain("bankfeed.example")
                            .icon(IconName::Building2)
                            .description("Brings every account's lines in each morning and matches what it can.")
                            .feature("Daily statement import")
                            .feature("Suggested matches")
                            .feature("Balances checked against the bank")
                            .connected(true),
                    )
                    .service(
                        Integration::new("docs", "Docuvault", "Documents")
                            .domain("docuvault.example")
                            .icon(IconName::FolderClosed)
                            .description("Files receipts and invoices next to the entries they belong to.")
                            .feature("Receipts attached to entries")
                            .feature("Search inside scanned files")
                            .connected(true),
                    )
                    .service(
                        Integration::new("pay", "Paystream", "Payments")
                            .domain("paystream.example")
                            .description("Records card and transfer payments as they settle, fees split out.")
                            .feature("Payouts matched to sales")
                            .feature("Fees booked on their own line")
                            .connected(true),
                    )
                    .service(
                        Integration::new("payroll", "Payroller", "Payroll")
                            .domain("payroller.example")
                            .icon(IconName::User)
                            .description("Turns each pay run into a journal with the right cost centres.")
                            .feature("One journal per pay run")
                            .feature("Cost centres kept"),
                    )
                    .service(
                        Integration::new("tax", "Taxbox", "Tax filing")
                            .domain("taxbox.example")
                            .description("Sends the quarter's return straight from the ledger.")
                            .feature("Returns filled from the ledger")
                            .feature("Filing receipts kept"),
                    )
                    .service(
                        Integration::new("shop", "Shopfront", "Commerce")
                            .domain("shopfront.example")
                            .description("Brings orders, refunds and stock movements in as they happen.")
                            .feature("Orders as sales invoices")
                            .feature("Refunds as credit notes"),
                    )
                    .service(
                        Integration::new("chat", "Teamchat", "Team chat")
                            .domain("teamchat.example")
                            .icon(IconName::Inbox)
                            .description("Posts approvals and month-end reminders where the team talks.")
                            .feature("Approve bills from a message"),
                    )
                    .service(
                        Integration::new("time", "Hourly", "Time tracking")
                            .domain("hourly.example")
                            .icon(IconName::Calendar)
                            .description("Bills tracked hours to the right client each month.")
                            .feature("Hours become invoice lines"),
                    )
                    .service(
                        Integration::new("expense", "Expensely", "Expenses")
                            .domain("expensely.example")
                            .description("Pulls approved expense claims in with their receipts.")
                            .feature("Claims with receipts attached"),
                    )
                    .service(
                        Integration::new("crm", "Clientbook", "Clients")
                            .domain("clientbook.example")
                            .icon(IconName::CircleUser)
                            .description("Keeps client details the same in both places.")
                            .feature("Two-way client sync"),
                    )
            });
            let events = cx.subscribe(&picker, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    IntegrationPickerEvent::Toggled(id, true) => format!("Connected {id}"),
                    IntegrationPickerEvent::Toggled(id, false) => format!("Disconnected {id}"),
                    IntegrationPickerEvent::Explore(id) => format!("Learn more about {id}"),
                    IntegrationPickerEvent::Back => "Back pressed".to_string(),
                    IntegrationPickerEvent::Close => "Closed".to_string(),
                }
                .into();
                cx.notify();
            });
            Self {
                picker,
                last: "Nothing yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for IntegrationPickerStory {
    fn title() -> &'static str {
        "Integration picker"
    }

    fn description() -> &'static str {
        "Find a service and switch it on, without leaving."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for IntegrationPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.picker.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
