// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Loading placeholder: grey shapes that stand in for content while it loads, shaped like the lines, rows, cards or table it will become.
//!
//! Pick a shape close to what will arrive so the page doesn't jump when it
//! does. The shapes pulse gently, and hold still when the system asks for
//! less motion. A screen reader hears one "Loading" label, not the shapes.
//!
//! Besides lines, rows, a card and a table there are shapes for an
//! assistant's answer (a mark beside paragraphs of uneven lines), a plain
//! list (icon, title and trailing meta between hairlines) and a row of
//! cards.

use std::time::Duration;

use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
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
    /// An assistant's answer: a small mark beside this many paragraphs.
    Answer(usize),
    /// A list of this many rows: a small icon, a title and a trailing
    /// meta, divided by hairlines.
    List(usize),
    /// Cards side by side, this many across.
    Cards(usize),
}

/// Widths for an answer's paragraphs, as shares of the full width. Each
/// paragraph runs three or four lines and ends short; the last paragraph is
/// the shortest, as a closing sentence would be.
pub fn answer_line_widths(paragraphs: usize) -> Vec<Vec<f32>> {
    (0..paragraphs)
        .map(|p| {
            let lines = if p + 1 == paragraphs && paragraphs > 1 {
                2
            } else {
                3 + p % 2
            };
            let mut widths = line_widths(lines);
            if let Some(last) = widths.last_mut()
                && lines > 1
            {
                *last = [0.72, 0.58, 0.66][p % 3];
            }
            widths
        })
        .collect()
}

/// Widths for a list row's title and trailing meta, as shares of the row,
/// varied by row so a list doesn't read as a grid.
pub fn list_row_widths(ix: usize) -> (f32, f32) {
    ([0.42, 0.56, 0.36, 0.5][ix % 4], [0.12, 0.08, 0.1][ix % 3])
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

fn card(color: Hsla, line: Hsla) -> isonomic_ui::Div {
    v_flex()
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
}

fn shape(shape: PlaceholderShape, color: Hsla, line: Hsla) -> AnyElement {
    match shape {
        PlaceholderShape::Answer(paragraphs) => {
            h_flex()
                .w_full()
                .items_start()
                .gap_3()
                .child(div().size(px(24.)).flex_none().rounded(px(7.)).bg(color))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        // The first line sits level with the mark's middle.
                        .pt(px(7.))
                        .gap(px(16.))
                        .children(answer_line_widths(paragraphs.max(1)).into_iter().map(
                            |widths| {
                                v_flex()
                                    .w_full()
                                    .gap(px(10.))
                                    .children(widths.into_iter().map(|w| bar(color, w, 10.)))
                            },
                        )),
                )
                .into_any_element()
        }
        PlaceholderShape::List(rows) => v_flex()
            .w_full()
            .children((0..rows.max(1)).map(|ix| {
                let (title, meta) = list_row_widths(ix);
                h_flex()
                    .w_full()
                    .h(px(40.))
                    .gap_3()
                    .when(ix > 0, |this| this.border_t_1().border_color(line))
                    .child(div().size(px(16.)).flex_none().rounded(px(4.)).bg(color))
                    .child(div().flex_1().child(bar(color, title, 10.)))
                    .child(
                        div()
                            .w(relative(meta))
                            .flex_none()
                            .child(bar(color, 1., 8.)),
                    )
            }))
            .into_any_element(),
        PlaceholderShape::Cards(across) => h_flex()
            .w_full()
            .items_start()
            .gap_3()
            .children((0..across.max(1)).map(|_| div().flex_1().min_w_0().child(card(color, line))))
            .into_any_element(),
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
        PlaceholderShape::Card => card(color, line).into_any_element(),
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
    fn answers_run_paragraphs_that_end_short() {
        let answer = answer_line_widths(3);
        assert_eq!(answer.len(), 3);
        assert_eq!(answer[0], [1.0, 0.94, 0.72]);
        assert_eq!(answer[1], [1.0, 0.94, 0.97, 0.58]);
        assert_eq!(answer[2], [1.0, 0.66], "the closing paragraph is two lines");
        assert_eq!(answer_line_widths(1), [vec![1.0, 0.94, 0.72]]);
        assert!(answer_line_widths(0).is_empty());
    }

    #[test]
    fn list_rows_vary_their_widths() {
        assert_eq!(list_row_widths(0), (0.42, 0.12));
        assert_eq!(list_row_widths(1), (0.56, 0.08));
        assert_ne!(list_row_widths(2), list_row_widths(3));
    }

    #[test]
    fn lines_end_short_and_pulse_stays_visible() {
        assert_eq!(line_widths(1), [1.0]);
        assert_eq!(line_widths(3), [1.0, 0.94, 0.6]);
        assert_eq!(pulse_opacity(0.0), 0.55);
        assert_eq!(pulse_opacity(0.5), 1.0);
        assert_eq!(pulse_opacity(1.0), 0.55);
    }
}
