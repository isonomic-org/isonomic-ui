// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Sequence diagram: messages between participants along their lifelines, newest one lit as it arrives.
//!
//! Participants head evenly spaced columns, each with a dashed lifeline
//! down the page. Every message is an arrow from one lifeline to another
//! on its own row, labelled above; replies are dashed, and a message to
//! oneself loops back. The newest message is drawn in the accent colour
//! and fades in, so a diagram fed live reads like a conversation.
//! Clicking a message selects it.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One message on the diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceMessage {
    /// Who sent it.
    pub from: SharedString,
    /// Who got it.
    pub to: SharedString,
    /// What it said, in a few words.
    pub label: SharedString,
    /// Whether it answers an earlier message.
    pub reply: bool,
}

impl SequenceMessage {
    /// Create a message.
    pub fn new(
        from: impl Into<SharedString>,
        to: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            label: label.into(),
            reply: false,
        }
    }

    /// Mark it as a reply.
    pub fn reply(mut self) -> Self {
        self.reply = true;
        self
    }
}

/// Where a participant's lifeline sits, from 0 to 1 across.
pub fn lifeline_at(index: usize, count: usize) -> f32 {
    if count <= 1 {
        0.5
    } else {
        (index as f32 + 0.5) / count as f32
    }
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Messages between participants along lifelines.
#[derive(IntoElement)]
pub struct SequenceDiagram {
    id: ElementId,
    style: StyleRefinement,
    participants: Vec<(SharedString, SharedString)>,
    messages: Vec<SequenceMessage>,
    selected: Option<usize>,
    on_select: Option<SelectHandler>,
}

impl SequenceDiagram {
    /// Create a diagram for participants given as id and name.
    pub fn new(
        id: impl Into<ElementId>,
        participants: Vec<(SharedString, SharedString)>,
        messages: Vec<SequenceMessage>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            participants,
            messages,
            selected: None,
            on_select: None,
        }
    }

    /// The selected message, by its place in the list.
    pub fn selected(mut self, ix: Option<usize>) -> Self {
        self.selected = ix;
        self
    }

    /// Run when a message is clicked.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for SequenceDiagram {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

const HEAD: f32 = 40.;
const ROW: f32 = 40.;

struct Arrow {
    from: f32,
    to: f32,
    row: usize,
    reply: bool,
    color: Hsla,
}

fn arrowhead(tip: Point<Pixels>, left: bool) -> PathBuilder {
    let dir = if left { 1. } else { -1. };
    let mut head = PathBuilder::fill();
    head.move_to(tip);
    head.line_to(point(tip.x + px(7. * dir), tip.y - px(4.)));
    head.line_to(point(tip.x + px(7. * dir), tip.y + px(4.)));
    head.close();
    head
}

impl RenderOnce for SequenceDiagram {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let accent = theme.primary;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let count = self.participants.len();
        let at = |id: &SharedString| {
            self.participants
                .iter()
                .position(|(pid, _)| pid == id)
                .map(|ix| lifeline_at(ix, count))
        };
        let newest = self.messages.len().checked_sub(1);
        let height = HEAD + ROW * self.messages.len() as f32 + 16.;

        let arrows: Vec<Arrow> = self
            .messages
            .iter()
            .enumerate()
            .filter_map(|(row, m)| {
                let highlighted = Some(row) == newest || Some(row) == self.selected;
                Some(Arrow {
                    from: at(&m.from)?,
                    to: at(&m.to)?,
                    row,
                    reply: m.reply,
                    color: if highlighted { accent } else { quiet },
                })
            })
            .collect();
        let lifelines: Vec<f32> = (0..count).map(|ix| lifeline_at(ix, count)).collect();

        let heads: Vec<_> = self
            .participants
            .iter()
            .enumerate()
            .map(|(ix, (_, name))| {
                div()
                    .absolute()
                    .top_0()
                    .left(relative(lifeline_at(ix, count)))
                    .ml(px(-60.))
                    .w(px(120.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_2()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .bg(fill)
                    .text_label_small()
                    .child(div().truncate().child(name.clone()))
            })
            .collect();

        let labels: Vec<_> = self
            .messages
            .iter()
            .enumerate()
            .filter_map(|(row, m)| {
                let (a, b) = (at(&m.from)?, at(&m.to)?);
                let mid = if (a - b).abs() < f32::EPSILON {
                    a + 0.04
                } else {
                    (a + b) / 2.
                };
                let is_new = Some(row) == newest;
                let is_selected = Some(row) == self.selected;
                let select = self.on_select.clone();
                let label = div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("msg-{row}").into(),
                    ))
                    .absolute()
                    .top(px(HEAD + ROW * row as f32 + 4.))
                    .left(relative(mid))
                    .ml(px(-90.))
                    .w(px(180.))
                    .flex()
                    .justify_center()
                    .cursor_pointer()
                    .role(Role::Button)
                    .aria_label(format!("{} to {}: {}", m.from, m.to, m.label))
                    .aria_selected(is_selected)
                    .on_click(move |_, window, cx| {
                        if let Some(select) = &select {
                            select(row, window, cx);
                        }
                    })
                    .child(
                        div()
                            .px_1p5()
                            .rounded(px(4.))
                            .bg(if is_selected {
                                accent.opacity(0.12)
                            } else {
                                fill.opacity(0.9)
                            })
                            .truncate()
                            .text_caption()
                            .text_color(if is_new || is_selected { ink } else { quiet })
                            .when(is_new, |this| this.font_medium())
                            .child(format!("{}. {}", row + 1, m.label)),
                    );
                Some(if is_new {
                    label
                        .with_animation(
                            ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("arrive-{row}").into(),
                            ),
                            Animation::new(std::time::Duration::from_millis(400)),
                            |el, delta| el.opacity(delta),
                        )
                        .into_any_element()
                } else {
                    label.into_any_element()
                })
            })
            .collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(760.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .role(Role::Group)
            .aria_label(format!(
                "Sequence of {} messages between {count}",
                self.messages.len()
            ))
            .child(
                h_flex()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(line)
                    .text_caption()
                    .text_color(quiet)
                    .child(
                        div()
                            .flex_1()
                            .child(format!("{} messages", self.messages.len())),
                    )
                    .child("Dashed arrows are replies"),
            )
            .child(
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "scroll".into(),
                    ))
                    .overflow_x_scroll()
                    .child(
                        div()
                            .relative()
                            .min_w(px(140. * count as f32))
                            .mx_2()
                            .my_3()
                            .h(px(height))
                            .child(
                                canvas(
                                    |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        let x = |share: f32| {
                                            bounds.origin.x + bounds.size.width * share
                                        };
                                        let top = bounds.origin.y + px(28.);
                                        let bottom = bounds.origin.y + bounds.size.height;
                                        for share in &lifelines {
                                            let mut path = PathBuilder::stroke(px(1.))
                                                .dash_array(&[px(4.), px(4.)]);
                                            path.move_to(point(x(*share), top));
                                            path.line_to(point(x(*share), bottom));
                                            if let Ok(path) = path.build() {
                                                window.paint_path(path, line);
                                            }
                                        }
                                        for arrow in &arrows {
                                            let y = bounds.origin.y
                                                + px(HEAD + ROW * arrow.row as f32 + 26.);
                                            let mut path = PathBuilder::stroke(px(1.5));
                                            if arrow.reply {
                                                path = path.dash_array(&[px(5.), px(3.)]);
                                            }
                                            let (a, b) = (x(arrow.from), x(arrow.to));
                                            if (arrow.from - arrow.to).abs() < f32::EPSILON {
                                                let out = a + px(28.);
                                                path.move_to(point(a, y - px(6.)));
                                                path.line_to(point(out, y - px(6.)));
                                                path.line_to(point(out, y + px(6.)));
                                                path.line_to(point(a + px(2.), y + px(6.)));
                                                if let Ok(path) = path.build() {
                                                    window.paint_path(path, arrow.color);
                                                }
                                                if let Ok(head) =
                                                    arrowhead(point(a + px(1.), y + px(6.)), true)
                                                        .build()
                                                {
                                                    window.paint_path(head, arrow.color);
                                                }
                                                continue;
                                            }
                                            let tip = if b > a { b - px(1.) } else { b + px(1.) };
                                            path.move_to(point(a, y));
                                            path.line_to(point(tip, y));
                                            if let Ok(path) = path.build() {
                                                window.paint_path(path, arrow.color);
                                            }
                                            if let Ok(head) =
                                                arrowhead(point(tip, y), b < a).build()
                                            {
                                                window.paint_path(head, arrow.color);
                                            }
                                        }
                                    },
                                )
                                .absolute()
                                .inset_0()
                                .size_full(),
                            )
                            .children(heads)
                            .children(labels),
                    ),
            )
            .when(self.messages.is_empty(), |this| {
                this.child(
                    div()
                        .px_4()
                        .pb_3()
                        .text_color(quiet)
                        .child("No messages yet."),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifelines_spread_evenly() {
        assert_eq!(lifeline_at(0, 1), 0.5);
        assert_eq!(lifeline_at(0, 4), 0.125);
        assert_eq!(lifeline_at(3, 4), 0.875);
    }
}
