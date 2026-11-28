// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::component::{
    Sizable as _, Size, StyledExt as _,
    meter::Meter,
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use isonomic_ui::*;

use crate::{ChangeStorySize, section, story_toolbar};

pub struct MeterStory {
    usage: Entity<SliderState>,
    size: Size,
    _subscription: Subscription,
}

impl super::Story for MeterStory {
    fn title() -> &'static str {
        "Meter"
    }

    fn description() -> &'static str {
        "A measured value within a known range, coloured by its thresholds."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl MeterStory {
    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let usage = cx.new(|_| SliderState::new().min(0.).max(100.).default_value(72.));
        let subscription = cx.subscribe(&usage, |_, _, _: &SliderEvent, cx| cx.notify());
        Self {
            usage,
            size: Size::Medium,
            _subscription: subscription,
        }
    }
}

impl Render for MeterStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let used = self.usage.read(cx).value().start() as f64;
        v_flex()
            .gap_3()
            .on_action(cx.listener(|this, action: &ChangeStorySize, _, cx| {
                this.size = action.0;
                cx.notify();
            }))
            .child(story_toolbar(self.size))
            .child(
                section("Storage")
                    .description(
                        "Warns from 80 GB and turns critical from 95 GB. Drag to see it change.",
                    )
                    .v_flex()
                    .gap_4()
                    .child(
                        div().w_full().max_w(px(420.)).child(
                            Meter::new("storage", used)
                                .with_size(self.size)
                                .label("Storage")
                                .value_text(format!("{used:.0} GB of 100 GB"))
                                .warning_at(80.)
                                .critical_at(95.),
                        ),
                    )
                    .child(
                        div()
                            .w_full()
                            .max_w(px(420.))
                            .child(Slider::new(&self.usage).accessibility_label("Storage used")),
                    ),
            )
            .child(
                section("Higher is better")
                    .description("A battery warns as it falls below 20% and 10%.")
                    .v_flex()
                    .gap_4()
                    .children([64., 18., 6.].into_iter().map(|level| {
                        div().w_full().max_w(px(420.)).child(
                            Meter::new(SharedString::from(format!("battery-{level}")), level)
                                .with_size(self.size)
                                .label("Battery")
                                .higher_is_better(true)
                                .warning_at(20.)
                                .critical_at(10.),
                        )
                    })),
            )
            .child(
                section("Bar only")
                    .description("Without a label or value, for tight rows; it is still named.")
                    .child(
                        div().w(px(160.)).child(
                            Meter::new("quota", 41.)
                                .small()
                                .show_value(false)
                                .label("Quota"),
                        ),
                    ),
            )
    }
}
