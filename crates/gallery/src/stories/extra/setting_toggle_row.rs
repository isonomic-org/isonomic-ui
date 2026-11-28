// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the setting toggle row.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, px,
};
use isonomic_ui_extra::setting_toggle_row::SettingToggleRow;

/// Notification settings for a bookkeeping workspace. Saves take a moment,
/// and turning on the weekly digest fails the first time.
pub struct SettingToggleRowStory {
    receipts: bool,
    digest: bool,
    saving: Option<usize>,
    digest_error: Option<SharedString>,
    tries: usize,
}

impl SettingToggleRowStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            receipts: true,
            digest: false,
            saving: None,
            digest_error: None,
            tries: 0,
        })
    }

    fn save(&mut self, row: usize, value: bool, cx: &mut Context<Self>) {
        self.saving = Some(row);
        self.digest_error = None;
        cx.notify();
        cx.spawn(
            async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
                this.update(cx, |this, cx| {
                    this.saving = None;
                    if row == 0 {
                        this.receipts = value;
                    } else {
                        this.tries += 1;
                        if value && this.tries == 1 {
                            this.digest_error =
                                Some("Couldn't reach the mail service. Try again.".into());
                        } else {
                            this.digest = value;
                        }
                    }
                    cx.notify();
                })
                .ok();
            },
        )
        .detach();
    }
}

impl Story for SettingToggleRowStory {
    fn title() -> &'static str {
        "Setting toggle row"
    }

    fn description() -> &'static str {
        "One on-or-off setting with a title, a line on what it does, and a switch, where the whole row flips it."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SettingToggleRowStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let line = cx.theme().border;
        let receipts = cx.entity();
        let digest = cx.entity();
        v_flex()
            .w(px(460.))
            .p_1()
            .gap_0p5()
            .rounded(px(8.))
            .border_1()
            .border_color(line)
            .child(
                SettingToggleRow::new("receipts", "Email receipts", self.receipts)
                    .description("Send a copy of each paid invoice to the client.")
                    .saving(self.saving == Some(0))
                    .on_change(move |value, _, cx| {
                        receipts.update(cx, |this, cx| this.save(0, value, cx))
                    }),
            )
            .child(
                SettingToggleRow::new("digest", "Weekly digest", self.digest)
                    .description("A Monday summary of cash in, cash out and what is overdue.")
                    .saving(self.saving == Some(1))
                    .error(self.digest_error.clone())
                    .on_change(move |value, _, cx| {
                        digest.update(cx, |this, cx| this.save(1, value, cx))
                    }),
            )
            .child(
                SettingToggleRow::new("currency", "Multi-currency", false)
                    .description("Invoice and pay in more than one currency.")
                    .locked("Not on the Starter plan."),
            )
    }
}
