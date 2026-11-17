// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the option tree.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::option_tree::{Branch, BranchRisk, OptionTree};

/// The decision a reconciliation run stopped at. The story keeps the pick.
pub struct OptionTreeStory {
    chosen: Option<SharedString>,
}

fn options() -> Vec<Branch> {
    vec![
        Branch::new(
            "unlock",
            "Unlock March and post",
            "201 matches post to the right month; the close finishes today.",
        )
        .recommended()
        .child(
            Branch::new(
                "unlock-relock",
                "Relock after posting",
                "March closes again with the matches in.",
            )
            .recommended(),
        )
        .child(
            Branch::new(
                "unlock-report",
                "Re-send the March report",
                "The figures sent on the 3rd change by $412.",
            )
            .risk(BranchRisk::Some, "The board already has the old report"),
        ),
        Branch::new(
            "april",
            "Post the matches to April",
            "March stays locked; the matches land a month late.",
        )
        .risk(BranchRisk::Some, "March cash looks $412 short")
        .child(Branch::new(
            "april-note",
            "Add a note to March",
            "Explains the gap to whoever reads March.",
        ))
        .child(
            Branch::new(
                "april-tax",
                "Quarterly figures shift",
                "Q1 sales tax is worked out on March.",
            )
            .risk(
                BranchRisk::High,
                "Q1 sales tax would be filed on the wrong total",
            ),
        ),
        Branch::new(
            "wait",
            "Hold the matches",
            "Nothing posts until someone decides; the close stays open.",
        )
        .child(Branch::new(
            "wait-late",
            "The close runs past Friday",
            "The payroll journal waits behind it.",
        )),
    ]
}

impl OptionTreeStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { chosen: None })
    }
}

impl Story for OptionTreeStory {
    fn title() -> &'static str {
        "Option tree"
    }

    fn description() -> &'static str {
        "The branches open at a decision point, what each leads to, and the risks flagged on the way."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for OptionTreeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pick = cx.entity();
        OptionTree::new(
            "locked-period",
            "The March ledger is locked. What should happen to 201 card matches?",
            options(),
        )
        .chosen(self.chosen.clone())
        .on_pick(move |id, _, cx| {
            pick.update(cx, |this, cx| {
                this.chosen = Some(id.clone());
                cx.notify();
            })
        })
    }
}
