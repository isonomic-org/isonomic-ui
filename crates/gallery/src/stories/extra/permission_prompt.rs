// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the permission prompt.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::inline_diff_review::InlineDiffReview;
use isonomic_ui_extra::permission_prompt::{
    PermissionDecision, PermissionItem, PermissionPreview, PermissionPrompt, PermissionRisk,
    PermissionScope,
};

/// Shows each case: every choice with "this chat", a long command shown whole,
/// a change with a diff, a destructive request that cannot be allowed for good,
/// a safe one that plain Enter allows, and one limited to once.
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

/// A command long enough to need the whole preview: about 2,000 characters.
fn long_command() -> String {
    let mut command = String::from("./scripts/publish --mirror --delete --checksum \\\n");
    let mut ix = 0;
    while command.chars().count() < 1950 {
        command.push_str(&format!(
            "  --exclude 'drafts/{ix:02}/**' --exclude 'cache/{ix:02}/*.tmp' \\\n"
        ));
        ix += 1;
    }
    command.push_str("  ./site/ deploy@web-01:/srv/www/site/");
    command
}

/// One column of prompts, as wide as a prompt.
fn column() -> isonomic_ui::Div {
    v_flex().gap_4().w(px(400.))
}

impl Render for PermissionPromptStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let report = move |name: &'static str| {
            let story = story.clone();
            move |decision: Option<&PermissionDecision>, _: &mut Window, cx: &mut App| {
                let line = match decision {
                    Some(decision) if decision.allow => format!("{name}: allowed"),
                    Some(_) => format!("{name}: denied"),
                    None => format!("{name}: asked again"),
                };
                story.update(cx, |this, cx| {
                    this.last = line.into();
                    cx.notify();
                })
            }
        };

        v_flex()
            .gap_4()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Tab to a prompt. Left and right arrows choose how long. Risky and \
                         destructive prompts allow only on Command or Control with Enter; a \
                         plain Enter does nothing. Escape denies.",
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap_4()
                    .child(column().child(
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
                    .default_scope(PermissionScope::Chat)
                    .on_decision_change(report("Reminders")),
            )
            .child(
                PermissionPrompt::new("deploy", "Build assistant", "run a command")
                    .target("site, read and write")
                    .target_icon(IconName::SquareTerminal)
                    .reason("The site build finished and the release notes say to publish it.")
                    .preview(PermissionPreview::new(long_command()).title("Command"))
                    .scopes([PermissionScope::Once])
                    .on_decision_change(report("Command")),
            ))
            .child(column().child(
                PermissionPrompt::new("terms", "Writing assistant", "edit a file")
                    .target("contracts/terms.md")
                    .target_icon(IconName::File)
                    .preview(
                        PermissionPreview::new("")
                            .title("Change")
                            .diff(
                                InlineDiffReview::new(
                                    "terms-diff",
                                    "Invoices are due within thirty days.",
                                    "Invoices are due within 14 days of delivery.",
                                )
                                .title("Edits to the terms"),
                            ),
                    )
                    .scopes([PermissionScope::Once, PermissionScope::Chat])
                    .on_decision_change(report("Edit")),
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
                    .item(PermissionItem::new("General ledger 2019").meta("5.2 MB"))
                    .on_decision_change(report("Ledgers")),
            )
            .child(
                PermissionPrompt::new("read", "Research assistant", "read your notes folder")
                    .target("Notes, read only")
                    .target_icon(IconName::Folder)
                    .risk(PermissionRisk::Safe)
                    .reason("Plain Enter allows a safe request.")
                    .on_decision_change(report("Notes")),
            )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
