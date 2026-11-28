// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the fit field.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::fit_field::{Candidate, Criterion, FitField};

/// Made-up applicants for three open roles, scored on four criteria.
fn candidates() -> Vec<Candidate> {
    [
        ("ana", "Ana Ruiz", 0, [92., 84., 78., 88.]),
        ("ben", "Ben Okafor", 0, [74., 90., 66., 70.]),
        ("chloe", "Chloe Martin", 0, [58., 62., 70., 55.]),
        ("dev", "Dev Patel", 0, [40., 52., 61., 48.]),
        ("eli", "Eli Novak", 1, [88., 72., 91., 80.]),
        ("fay", "Fay Lindqvist", 1, [81., 86., 84., 79.]),
        ("gus", "Gus Tanaka", 1, [64., 58., 72., 69.]),
        ("hana", "Hana Kim", 1, [35., 44., 50., 41.]),
        ("ivo", "Ivo Petrov", 2, [77., 69., 83., 74.]),
        ("jade", "Jade Mensah", 2, [95., 88., 90., 93.]),
        ("kai", "Kai Silva", 2, [55., 71., 49., 60.]),
        ("lena", "Lena Weber", 2, [68., 75., 63., 66.]),
        ("milo", "Milo Haddad", 2, [30., 38., 45., 33.]),
    ]
    .into_iter()
    .map(|(id, name, category, scores)| Candidate::new(id, name, category, scores))
    .collect()
}

/// Applicants for three roles on one field.
pub struct FitFieldStory {
    picked: SharedString,
}

impl FitFieldStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Pick a candidate".into(),
        })
    }
}

impl Story for FitFieldStory {
    fn title() -> &'static str {
        "Fit field"
    }

    fn description() -> &'static str {
        "Candidates placed by score around a centre, in sectors by category, with threshold rings and a score breakdown."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FitFieldStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                FitField::new(
                    "applicants",
                    "Applicant fit",
                    vec!["Design".into(), "Engineering".into(), "Operations".into()],
                    vec![
                        Criterion::new("Skills", 3.),
                        Criterion::new("Experience", 2.),
                        Criterion::new("Interview", 3.),
                        Criterion::new("References", 1.),
                    ],
                    candidates(),
                )
                .subtitle("Spring hiring round")
                .on_select(move |id, _, cx| {
                    this.update(cx, |this, cx| {
                        this.picked = format!("Picked {id}").into();
                        cx.notify();
                    })
                }),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
