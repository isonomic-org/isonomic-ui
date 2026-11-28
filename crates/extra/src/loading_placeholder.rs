// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Loading placeholder: grey shapes that stand in for content while it loads, shaped like the lines, rows, cards or table it will become.
//!
//! Pick a shape close to what will arrive so the page doesn't jump when it
//! does. The shapes pulse gently, and hold still when the system asks for
//! less motion. A screen reader hears one "Loading" label, not the shapes.

use std::time::Duration;

use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, px, relative,
};

/// What the content will look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderShape {
    /// A paragraph of this many lines.
    Lines(usize),
    /// A round picture beside a title and a line, as in a list of people.
    MediaRow,
    /// A picture above a title and two lines.
    Card,
    /// A table with this many rows and columns.
    Table {
        /// Rows under the header.
        rows: usize,
        /// Columns across.
        columns: usize,
    },
}

/// Widths for a paragraph's lines, as shares of the full width: uneven, with a short last line.
pub fn line_widths(count: usize) -> Vec<f32> {
    (0..count)
        .map(|ix| {
            if count > 1 && ix + 1 == count {
                0.6
            } else {
                [1.0, 0.94, 0.97, 0.9][ix % 4]
            }
        })
        .collect()
}

/// How solid the shapes are at a point in the pulse, from 0 to 1.
pub fn pulse_opacity(delta: f32) -> f32 {
    let wave = 1.0 - (2.0 * delta.clamp(0.0, 1.0) - 1.0).abs();
    0.55 + 0.45 * wave
}

/// Shapes standing in for content.
#[derive(IntoElement)]
pub struct LoadingPlaceholder {
    id: ElementId,
    style: StyleRefinement,
    shape: PlaceholderShape,
    repeat: usize,
    label: SharedString,
}

impl LoadingPlaceholder {
    /// Create a placeholder of one shape.
    pub fn new(id: impl Into<ElementId>, shape: PlaceholderShape) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            shape,
            repeat: 1,
            label: "Loading".into(),
        }
    }

    /// Stack the shape this many times, as for a list.
    pub fn repeat(mut self, count: usize) -> Self {
        self.repeat = count.max(1);
        self
    }

    /// Say what is loading, such as "Loading invoices".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
}

impl Styled for LoadingPlaceholder {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn bar(color: Hsla, width: f32, height: f32) -> isonomic_ui::Div {
    div()
        .w(relative(width))
        .h(px(height))
        .rounded(px(4.))
        .bg(color)
}

fn shape(shape: PlaceholderShape, color: Hsla, line: Hsla) -> AnyElement {
    match shape {
        PlaceholderShape::Lines(count) => v_flex()
            .w_full()
            .gap_2()
            .children(line_widths(count).into_iter().map(|w| bar(color, w, 10.)))
            .into_any_element(),
        PlaceholderShape::MediaRow => h_flex()
            .w_full()
            .gap_3()
            .child(div().size(px(36.)).flex_none().rounded_full().bg(color))
            .child(
                v_flex()
                    .flex_1()
                    .gap_2()
                    .child(bar(color, 0.45, 11.))
                    .child(bar(color, 0.75, 9.)),
            )
            .into_any_element(),
        PlaceholderShape::Card => v_flex()
            .w_full()
            .gap_2()
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(line)
            .child(div().w_full().h(px(96.)).rounded(px(6.)).bg(color))
            .child(bar(color, 0.55, 12.))
            .child(bar(color, 0.9, 9.))
            .child(bar(color, 0.7, 9.))
            .into_any_element(),
        PlaceholderShape::Table { rows, columns } => {
            let columns = columns.max(1);
            let row = |height: f32| {
                h_flex()
                    .w_full()
                    .gap_4()
                    .py_2()
                    .border_b_1()
                    .border_color(line)
                    .children((0..columns).map(move |c| {
                        div()
                            .flex_1()
                            .child(bar(color, if c == 0 { 0.8 } else { 0.5 }, height))
                    }))
            };
            v_flex()
                .w_full()
                .child(row(8.))
                .children((0..rows).map(|_| row(10.)))
                .into_any_element()
        }
    }
}

impl RenderOnce for LoadingPlaceholder {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = cx.theme().muted;
        let line = cx.theme().border;
        let still = cx.reduce_motion();
        let shapes = v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_4()
            .role(Role::ProgressIndicator)
            .aria_label(self.label.clone())
            .children((0..self.repeat).map(|_| shape(self.shape, color, line)))
            .refine_style(&self.style);
        if still {
            shapes.into_any_element()
        } else {
            shapes
                .with_animation(
                    ElementId::NamedChild(self.id.into(), "pulse".into()),
                    Animation::new(Duration::from_millis(1600)).repeat(),
                    |this, delta| this.opacity(pulse_opacity(delta)),
                )
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_end_short_and_pulse_stays_visible() {
        assert_eq!(line_widths(1), [1.0]);
        assert_eq!(line_widths(3), [1.0, 0.94, 0.6]);
        assert_eq!(pulse_opacity(0.0), 0.55);
        assert_eq!(pulse_opacity(0.5), 1.0);
        assert_eq!(pulse_opacity(1.0), 0.55);
    }
}
