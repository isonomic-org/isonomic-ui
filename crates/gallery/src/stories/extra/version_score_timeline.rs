// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the version score timeline.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::version_score_timeline::{ShippedVersion, VersionScoreTimeline};

/// Made-up releases of a route planner, scored on a fixed set of trips.
fn planner_releases() -> Vec<ShippedVersion> {
    vec![
        ShippedVersion::new("v1.0", 0., "3 Feb", 71.2).note("First release on the trip suite."),
        ShippedVersion::new("v1.1", 6., "9 Feb", 73.9).note("Ferry crossings counted as legs."),
        ShippedVersion::new("v1.2", 11., "14 Feb", 76.4)
            .note("Walking transfers under five minutes merged."),
        ShippedVersion::new("v1.3", 18., "21 Feb", 72.1)
            .note("New fare table: night buses priced wrong."),
        ShippedVersion::new("v1.2", 19., "22 Feb", 76.4)
            .restores("v1.2")
            .note("Rolled back while the fare table is fixed."),
        ShippedVersion::new("v1.4", 27., "2 Mar", 78.8).note("Fare table fixed; night buses back."),
        ShippedVersion::new("v1.5", 34., "9 Mar", 81.3).note("Live delays folded into transfers."),
        ShippedVersion::new("v1.6", 40., "15 Mar", 81.0).note("Map tiles cached for offline use."),
        ShippedVersion::new("v1.7", 47., "22 Mar", 84.6).note("Bike share docks as stops."),
        ShippedVersion::new("v1.8", 55., "30 Mar", 79.9)
            .note("Step-free routes forced on by default."),
    ]
}

/// A planner's releases against a target, and a short lower-is-better run.
pub struct VersionScoreTimelineStory {
    picked: SharedString,
}

impl VersionScoreTimelineStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Point at a version".into(),
        })
    }
}

impl Story for VersionScoreTimelineStory {
    fn title() -> &'static str {
        "Version score timeline"
    }

    fn description() -> &'static str {
        "Versions over time at the score they shipped with, with regressions and rollbacks marked."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for VersionScoreTimelineStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                VersionScoreTimeline::new("planner", "Trip suite score", planner_releases())
                    .subtitle("Route planner, every release since February")
                    .target(80., "Ship bar")
                    .tolerance(0.5)
                    .on_select(move |version, _, cx| {
                        this.update(cx, |this, cx| {
                            this.picked =
                                format!("{} shipped {}", version.name, version.date).into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div().max_w(px(360.)).child(
                    VersionScoreTimeline::new(
                        "misreads",
                        "Misread rate",
                        vec![
                            ShippedVersion::new("r12", 0., "Mon", 4.8),
                            ShippedVersion::new("r13", 1., "Tue", 4.1),
                            ShippedVersion::new("r14", 2., "Wed", 3.6),
                            ShippedVersion::new("r15", 3., "Thu", 5.2)
                                .note("Handwriting model swapped."),
                            ShippedVersion::new("r14", 3.5, "Thu", 3.6).restores("r14"),
                            ShippedVersion::new("r16", 5., "Sat", 3.1),
                        ],
                    )
                    .subtitle("Receipt reader, lower is better")
                    .lower_is_better()
                    .format(|rate| format!("{rate:.1}%")),
                ),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
