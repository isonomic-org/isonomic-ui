// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the hourly delivery heatmap.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::hourly_delivery_heatmap::{HeatmapRow, HourBucket, HourlyDeliveryHeatmap};

const STATIONS: [&str; 5] = ["ovens", "front-counter", "couriers", "ledger", "stock-room"];

/// Made-up notices a bakery sends to its stations: a morning rush, a quiet
/// night, and the couriers' line dropping notices in the early afternoon.
fn rows(columns: usize, scale: f64) -> Vec<HeatmapRow> {
    let mut seed: u32 = 11;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1u32 << 24)
    };
    STATIONS
        .iter()
        .enumerate()
        .map(|(row, name)| {
            let weight = [1., 0.7, 0.55, 0.35, 0.2][row];
            let buckets = (0..columns)
                .map(|column| {
                    let along = column as f64 / columns as f64;
                    // Two humps: before opening and at lunch.
                    let rush = (-(along - 0.3).powi(2) / 0.012).exp()
                        + 0.7 * (-(along - 0.55).powi(2) / 0.02).exp();
                    let attempts = (scale * weight * (0.06 + rush) * (0.8 + 0.4 * next())) as u32;
                    let dropping = row == 2 && (0.54..0.64).contains(&along);
                    if dropping {
                        HourBucket::new(attempts, attempts * 2 / 5).status("503")
                    } else if row == 3 && column == columns - 2 {
                        HourBucket::new(attempts.max(40), 1).status("timeout")
                    } else {
                        HourBucket::new(attempts, 0)
                    }
                })
                .collect();
            HeatmapRow::new(*name, buckets)
        })
        .collect()
}

fn hour_labels() -> Vec<SharedString> {
    (0..24).map(|hour| format!("{hour:02}:00").into()).collect()
}

/// A bakery's notices by station over an hour, a day and a week.
pub struct HourlyDeliveryHeatmapStory {
    range: usize,
    picked: SharedString,
}

impl HourlyDeliveryHeatmapStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            range: 1,
            picked: "Click a cell".into(),
        })
    }
}

impl Story for HourlyDeliveryHeatmapStory {
    fn title() -> &'static str {
        "Hourly delivery heatmap"
    }

    fn description() -> &'static str {
        "Volume per endpoint and hour as bar height in each cell, with failing hours marked and every hour totalled in dots."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for HourlyDeliveryHeatmapStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let ranged = cx.entity();
        let picked = cx.entity();
        let (columns, data, date, failing): (Vec<SharedString>, _, _, _) = match self.range {
            0 => (
                (0..12)
                    .map(|slot| format!("11:{:02}", slot * 5).into())
                    .collect(),
                rows(12, 160.),
                "11:00 to 12:00",
                "Failing 5 min",
            ),
            1 => (hour_labels(), rows(24, 1900.), "Thursday", "Failing hour"),
            _ => (
                ["Fri", "Sat", "Sun", "Mon", "Tue", "Wed", "Thu"]
                    .map(SharedString::from)
                    .to_vec(),
                rows(7, 21000.),
                "Last 7 days",
                "Failing day",
            ),
        };
        let names: Vec<SharedString> = columns.clone();
        v_flex()
            .gap_6()
            .child(
                HourlyDeliveryHeatmap::new("notices", "Deliveries", columns, data)
                    .section("Notices")
                    .subtitle("Every try to reach a station, retries included")
                    .heading("Attempts by station")
                    .date(date)
                    .failing_label(failing)
                    .ranges(["1h", "24h", "7d"], self.range, move |range, _, cx| {
                        ranged.update(cx, |this, cx| {
                            this.range = range;
                            cx.notify();
                        })
                    })
                    .on_select(move |row, column, _, cx| {
                        let words = format!("Picked {} at {}", STATIONS[row], names[column]);
                        picked.update(cx, |this, cx| {
                            this.picked = words.into();
                            cx.notify();
                        })
                    }),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
            .child(
                HourlyDeliveryHeatmap::new(
                    "quiet",
                    "Deliveries",
                    hour_labels(),
                    vec![
                        HeatmapRow::new(
                            "night-shift",
                            (0..24)
                                .map(|hour| {
                                    HourBucket::new(if hour < 6 { 30 + hour * 9 } else { 0 }, 0)
                                })
                                .collect(),
                        ),
                        HeatmapRow::new("day-shift", Vec::new()),
                    ],
                )
                .heading("A quiet day")
                .ticks([0, 6, 12, 18, 23]),
            )
    }
}
