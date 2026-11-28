// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the evaluation case grid.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::evaluation_case_grid::{
    CaseFilter, CaseStatus, EvaluationCase, EvaluationCaseGrid, EvaluationMetric,
};

const DISHES: [&str; 8] = [
    "sourdough",
    "focaccia",
    "brioche",
    "rye-loaf",
    "bagels",
    "ciabatta",
    "pretzels",
    "flatbread",
];
const CHECKS: [&str; 6] = [
    "halve",
    "double",
    "to-grams",
    "to-cups",
    "for-12",
    "rest-time",
];

/// Made-up checks of a recipe scaler: most pass, a few fail or flip.
fn cases(running: usize) -> Vec<EvaluationCase> {
    let total = DISHES.len() * CHECKS.len();
    (0..total)
        .map(|ix| {
            let name = format!("{}/{}", DISHES[ix % 8], CHECKS[ix / 8]);
            let status = if ix >= total - running {
                if ix % 3 == 0 {
                    CaseStatus::Running
                } else {
                    CaseStatus::Pending
                }
            } else {
                match ix {
                    5 | 22 | 31 => CaseStatus::Fail,
                    9 | 27 => CaseStatus::Regressed,
                    3 | 14 | 36 => CaseStatus::Improved,
                    _ => CaseStatus::Pass,
                }
            };
            let case = EvaluationCase::new(name, status);
            match ix {
                9 => case
                    .score(0.41)
                    .input("Scale focaccia from 8 to 12 portions")
                    .expected("750 g flour, 600 g water")
                    .actual("750 g flour, 480 g water")
                    .reason("Flour was scaled but the water kept its old amount."),
                27 => case
                    .score(0.38)
                    .input("Convert 3 cups of rye flour to grams")
                    .expected("306 g")
                    .actual("360 g")
                    .reason("Used the weight of wheat flour for rye."),
                5 => case
                    .score(0.12)
                    .input("Halve a recipe that calls for 3 eggs")
                    .expected("1 egg and 1 yolk, or 75 g beaten egg")
                    .actual("1.5 eggs")
                    .reason("Half an egg is not an amount a baker can measure."),
                3 => case
                    .score(0.94)
                    .input("Halve the rye loaf")
                    .expected("250 g rye, 190 g water, 5 g salt")
                    .actual("250 g rye, 190 g water, 5 g salt"),
                _ if status == CaseStatus::Pass => case.score(0.9 + (ix % 10) as f64 / 100.),
                _ => case,
            }
        })
        .collect()
}

/// A recipe scaler's test run: finished, as a comparison, and still running.
pub struct EvaluationCaseGridStory {
    picked: SharedString,
}

impl EvaluationCaseGridStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Pick a case".into(),
        })
    }
}

impl Story for EvaluationCaseGridStory {
    fn title() -> &'static str {
        "Evaluation case grid"
    }

    fn description() -> &'static str {
        "A test run as a grid of cases, each pass, fail or regressed against a baseline, with the picked case in detail."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EvaluationCaseGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        let filtered = cx.entity();
        let column = || v_flex().w_full().max_w(px(512.)).gap_6();
        let finished = EvaluationCaseGrid::new("scaler", "recipe-scaling", cases(0))
            .label("scaler v8 against v7")
            .baseline_pass_rate(0.875)
            .on_select(move |_, case, _, cx| {
                this.update(cx, |this, cx| {
                    this.picked =
                        format!("Picked {}: {}", case.name(), case.status().words()).into();
                    cx.notify();
                })
            })
            .on_filter(move |filter, _, cx| {
                filtered.update(cx, |this, cx| {
                    this.picked = format!("Showing {}", filter.words()).into();
                    cx.notify();
                })
            });
        let compared = EvaluationCaseGrid::new("scaler-compare", "recipe-scaling", cases(0))
            .label("scaler v8 against v7")
            .baseline_pass_rate(0.875)
            .compare(vec![
                EvaluationMetric::new("Pass rate", 87.5, 89.6)
                    .format(|value| format!("{value:.1}%")),
                EvaluationMetric::new("Time per case", 1.8, 2.3)
                    .lower_is_better()
                    .format(|value| format!("{value:.1} s")),
                EvaluationMetric::new("Retries", 6., 6.),
            ]);
        let running = EvaluationCaseGrid::new("scaler-running", "unit-conversion", cases(14))
            .label("nightly")
            .baseline_pass_rate(0.9)
            .filter(CaseFilter::Failed);
        h_flex()
            .w_full()
            .flex_wrap()
            .items_start()
            .gap_6()
            .child(
                column()
                    .child(finished)
                    .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
                    .child(running),
            )
            .child(column().child(compared))
    }
}
