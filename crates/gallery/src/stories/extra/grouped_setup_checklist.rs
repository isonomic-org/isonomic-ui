// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the grouped setup checklist.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::grouped_setup_checklist::{GroupedSetupChecklist, SetupGroup, SetupTask};

/// Shows three groups, one of them already finished, and reports progress.
pub struct GroupedSetupChecklistStory {
    last: SharedString,
}

impl GroupedSetupChecklistStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing changed yet".into(),
        })
    }
}

impl Story for GroupedSetupChecklistStory {
    fn title() -> &'static str {
        "Grouped setup checklist"
    }

    fn description() -> &'static str {
        "Onboarding split into areas, each a few steps that"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for GroupedSetupChecklistStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changed = cx.entity();
        let edited = changed.clone();

        v_flex()
            .gap_3()
            .child(
                GroupedSetupChecklist::new("setup")
                    .title("Open the books")
                    .group(
                        SetupGroup::new("company", "Company details")
                            .task(SetupTask::new("legal-name", "Enter the legal name"))
                            .task(SetupTask::new("tax-id", "Add the tax number"))
                            .task(SetupTask::new("year", "Pick the financial year")),
                    )
                    .group(
                        SetupGroup::new("bank", "Bank feeds")
                            .task(SetupTask::new("connect", "Connect the main account"))
                            .task(SetupTask::new("history", "Import 12 months of history")),
                    )
                    .group(
                        SetupGroup::new("owner", "Confirm the owner")
                            .task(SetupTask::new("identity", "Verify identity").done(true)),
                    )
                    .on_change(move |done, _, cx| {
                        let line = format!("{} tasks finished", done.len());
                        changed.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    })
                    .on_edit(move |_, cx| {
                        edited.update(cx, |this, cx| {
                            this.last = "Edit pressed".into();
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
