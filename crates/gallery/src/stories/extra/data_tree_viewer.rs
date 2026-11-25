// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the data tree viewer.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::data_tree_viewer::{DataTreeEvent, DataTreeViewer, DataValue};

fn order() -> DataValue {
    let item = |sku: &str, name: &str, qty: f64, price: f64| {
        DataValue::map([
            ("sku", DataValue::text(sku)),
            ("name", DataValue::text(name)),
            ("qty", DataValue::Number(qty)),
            ("unit_price", DataValue::Number(price)),
        ])
    };
    DataValue::map([
        ("order_id", DataValue::text("CER-20418")),
        ("status", DataValue::text("packed")),
        ("paid", DataValue::Bool(true)),
        ("gift_note", DataValue::Null),
        (
            "customer",
            DataValue::map([
                ("name", DataValue::text("Ines Okafor")),
                ("city", DataValue::text("Porto")),
                ("repeat buyer", DataValue::Bool(false)),
            ]),
        ),
        (
            "items",
            DataValue::list([
                item("mug-cobalt", "Cobalt mug", 2., 18.5),
                item("plate-speckle", "Speckled plate", 6., 14.),
                item("vase-tall", "Tall vase", 1., 62.),
            ]),
        ),
        (
            "shipping",
            DataValue::map([
                ("carrier", DataValue::text("road")),
                ("weight_kg", DataValue::Number(7.4)),
                ("fragile", DataValue::Bool(true)),
            ]),
        ),
    ])
}

/// A pottery shop's order, as a tool returned it.
pub struct DataTreeViewerStory {
    viewer: Entity<DataTreeViewer>,
    copied: SharedString,
    _events: Subscription,
}

impl DataTreeViewerStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let viewer = cx.new(|cx| DataTreeViewer::new("Order lookup", order(), 1, window, cx));
            let events = cx.subscribe(&viewer, |this: &mut Self, _, event, cx| {
                this.copied = match event {
                    DataTreeEvent::CopiedPath(path) => format!("Copied path {path}").into(),
                    DataTreeEvent::CopiedValue(value) => {
                        format!("Copied {} characters", value.chars().count()).into()
                    }
                };
                cx.notify();
            });
            Self {
                viewer,
                copied: "Nothing copied yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for DataTreeViewerStory {
    fn title() -> &'static str {
        "Data tree viewer"
    }

    fn description() -> &'static str {
        "Structured output as a tree: open branches, search keys and values, copy a path or a value."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DataTreeViewerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_4().child(self.viewer.clone()).child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(self.copied.clone()),
        )
    }
}
