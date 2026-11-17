// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the project summary card.

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window,
};
use isonomic_ui_extra::project_summary_card::{
    ProjectEdit, ProjectStatus, ProjectSummaryCard, Subtask,
};

/// Two bookkeeping projects; the first is open. The story applies every edit.
pub struct ProjectSummaryCardStory {
    open: bool,
    status: ProjectStatus,
    people: Vec<SharedString>,
    subtasks: Vec<Subtask>,
}

impl ProjectSummaryCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            open: true,
            status: ProjectStatus::AtRisk,
            people: vec![
                "Maya Okafor".into(),
                "Tom Brandt".into(),
                "Reconciler".into(),
            ],
            subtasks: vec![
                Subtask::new("s1", "Match bank lines").done(true),
                Subtask::new("s2", "Match card payouts").done(true),
                Subtask::new("s3", "Unlock March and post the matches"),
                Subtask::new("s4", "Approve Friday's supplier run"),
                Subtask::new("s5", "Send the close note to the board"),
            ],
        })
    }
}

impl Story for ProjectSummaryCardStory {
    fn title() -> &'static str {
        "Project summary card"
    }

    fn description() -> &'static str {
        "A project at a glance, opening for quick edits to its status, people and subtasks."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ProjectSummaryCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let edit = cx.entity();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                ProjectSummaryCard::new("march-close", "March close", self.status)
                    .people(self.people.clone())
                    .due("Friday")
                    .subtasks(self.subtasks.clone())
                    .open(self.open)
                    .on_edit(move |change, _, cx| {
                        edit.update(cx, |this, cx| {
                            match change {
                                ProjectEdit::Toggle => this.open = !this.open,
                                ProjectEdit::Status(status) => this.status = status,
                                ProjectEdit::RemovePerson(name) => {
                                    this.people.retain(|p| p != &name)
                                }
                                ProjectEdit::Subtask(id, done) => {
                                    if let Some(task) =
                                        this.subtasks.iter_mut().find(|t| t.id == id)
                                    {
                                        task.done = done;
                                    }
                                }
                            }
                            cx.notify();
                        })
                    }),
            )
            .child(
                ProjectSummaryCard::new(
                    "year-end",
                    "Year-end packet for the accountant",
                    ProjectStatus::OnTrack,
                )
                .people(vec!["Maya Okafor".into(), "Report writer".into()])
                .due("15 January")
                .subtasks(vec![
                    Subtask::new("y1", "Collect 1099 details").done(true),
                    Subtask::new("y2", "Export the trial balance"),
                ]),
            )
    }
}
