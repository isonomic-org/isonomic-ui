// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the integration settings card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::integration_settings_card::{
    IntegrationSettingsCard, IntegrationSettingsEvent, IntegrationValues,
};

/// The settings of a made-up shop service linked to a bookkeeping app.
pub struct IntegrationSettingsCardStory {
    card: Entity<IntegrationSettingsCard>,
    last: SharedString,
    _events: Subscription,
}

impl IntegrationSettingsCardStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let card = cx.new(|cx| {
                IntegrationSettingsCard::new(
                    "Shopfront orders",
                    IntegrationValues {
                        enabled: true,
                        site: "harbour-and-pine.com".into(),
                        time_zone: Some("(UTC +00:00) Lisbon".into()),
                        api_key: None,
                    },
                    window,
                    cx,
                )
                .count(214)
                .meta("Linked on 2 March 2026 at 14:05")
                .site_hint("The address your shop is served from, without a path.")
                .time_zones([
                    "(UTC -08:00) Vancouver",
                    "(UTC -05:00) Toronto",
                    "(UTC +00:00) Lisbon",
                    "(UTC +01:00) Oslo",
                    "(UTC +05:30) Pune",
                    "(UTC +10:00) Brisbane",
                ])
            });
            let events = cx.subscribe(&card, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    IntegrationSettingsEvent::Toggled(true) => "Switched on".to_string(),
                    IntegrationSettingsEvent::Toggled(false) => "Switched off".to_string(),
                    IntegrationSettingsEvent::KeyMade => "New key made, not saved yet".to_string(),
                    IntegrationSettingsEvent::Saved(values) => format!(
                        "Saved {} in {}",
                        values.site,
                        values.time_zone.clone().unwrap_or_default()
                    ),
                }
                .into();
                cx.notify();
            });
            Self {
                card,
                last: "Nothing yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for IntegrationSettingsCardStory {
    fn title() -> &'static str {
        "Integration settings card"
    }

    fn description() -> &'static str {
        "The settings of one connected service."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for IntegrationSettingsCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.card.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
