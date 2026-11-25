// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the radial dot cycle.

use crate::Story;
use isonomic_ui::component::h_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window,
};
use isonomic_ui_extra::radial_dot_cycle::{CycleSeries, RadialDotCycle};

/// A made-up café's day by the hour, and a climbing gym's week.
pub struct RadialDotCycleStory;

impl RadialDotCycleStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for RadialDotCycleStory {
    fn title() -> &'static str {
        "Radial dot cycle"
    }

    fn description() -> &'static str {
        "A daily or weekly rhythm as rings of dots round a dial, with the picked slot's total in the middle."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RadialDotCycleStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let hours: Vec<SharedString> = (0..24).map(|hour| format!("{hour:02}:00").into()).collect();
        let counter = [
            0., 0., 0., 0., 0., 2., 9., 31., 44., 38., 22., 18., 35., 41., 20., 14., 17., 12., 6.,
            3., 1., 0., 0., 0.,
        ];
        let takeaway = [
            0., 0., 0., 0., 0., 4., 14., 26., 30., 12., 8., 9., 15., 12., 6., 5., 8., 4., 2., 0.,
            0., 0., 0., 0.,
        ];
        let days: Vec<SharedString> = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .into_iter()
            .map(SharedString::from)
            .collect();
        h_flex()
            .gap_6()
            .items_start()
            .flex_wrap()
            .child(
                RadialDotCycle::new("cafe", "Coffees by hour", hours, "562")
                    .subtitle("Tuesday")
                    .caption("coffees sold")
                    .series(CycleSeries::new("At the counter", counter))
                    .series(CycleSeries::new("To take away", takeaway))
                    .format(|cups| format!("{cups:.0} coffees")),
            )
            .child(
                RadialDotCycle::new("gym", "Check-ins by day", days, "1,930")
                    .caption("this week")
                    .axis([0, 2, 4, 5])
                    .series(CycleSeries::new(
                        "Check-ins",
                        [210., 250., 240., 300., 260., 380., 290.],
                    )),
            )
    }
}
