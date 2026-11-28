// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the contrast colour picker.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window, hsla,
};
use isonomic_ui_extra::contrast_color_picker::ContrastColorPicker;

/// A palette for paid, due and overdue amounts, checked against the theme's
/// background. The story keeps the pick and the recent ones.
pub struct ContrastColorPickerStory {
    picked: Option<usize>,
    recent: Vec<usize>,
}

fn palette() -> Vec<(SharedString, isonomic_ui::Hsla)> {
    vec![
        ("Ink".into(), hsla(220. / 360., 0.15, 0.15, 1.)),
        ("Slate".into(), hsla(215. / 360., 0.12, 0.45, 1.)),
        ("Mist".into(), hsla(215. / 360., 0.1, 0.72, 1.)),
        ("Forest".into(), hsla(150. / 360., 0.55, 0.3, 1.)),
        ("Mint".into(), hsla(150. / 360., 0.45, 0.6, 1.)),
        ("Amber".into(), hsla(38. / 360., 0.9, 0.5, 1.)),
        ("Rust".into(), hsla(12. / 360., 0.7, 0.42, 1.)),
        ("Coral".into(), hsla(8. / 360., 0.8, 0.68, 1.)),
        ("Indigo".into(), hsla(235. / 360., 0.6, 0.45, 1.)),
        ("Sky".into(), hsla(205. / 360., 0.8, 0.65, 1.)),
    ]
}

impl ContrastColorPickerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: Some(3),
            recent: vec![6, 0],
        })
    }
}

impl Story for ContrastColorPickerStory {
    fn title() -> &'static str {
        "Contrast colour picker"
    }

    fn description() -> &'static str {
        "A palette of swatches that checks each one as text on a background, with the ratio and whether it passes."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ContrastColorPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pick = cx.entity();
        ContrastColorPicker::new("paid-colour", palette())
            .picked(self.picked)
            .recent(self.recent.clone())
            .on_pick(move |ix, _, cx| {
                pick.update(cx, |this, cx| {
                    if let Some(old) = this.picked {
                        this.recent.retain(|r| *r != old && *r != ix);
                        this.recent.insert(0, old);
                    }
                    this.picked = Some(ix);
                    cx.notify();
                })
            })
    }
}
