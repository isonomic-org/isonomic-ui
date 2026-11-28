// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the split view.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, Sizable as _, button::Button, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div, px,
};
use isonomic_ui_extra::split_view::{SplitEvent, SplitView};

struct InvoiceList;

impl Render for InvoiceList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex().p_3().gap_2().children(
            [
                ("INV-2041", "Harbor Dental", "$4,200"),
                ("INV-2040", "Brightline Cafe", "$860"),
                ("INV-2039", "Tally & Rowe", "$1,975"),
            ]
            .into_iter()
            .map(move |(no, who, amount)| {
                v_flex().gap_0p5().child(div().text_sm().child(who)).child(
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child(format!("{no} \u{b7} {amount}")),
                )
            }),
        )
    }
}

struct InvoiceDetail;

impl Render for InvoiceDetail {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .p_4()
            .gap_2()
            .child(div().text_lg().child("INV-2041"))
            .child(
                div()
                    .text_sm()
                    .text_color(quiet)
                    .child("Harbor Dental \u{b7} due 1 Dec \u{b7} $4,200.00"),
            )
            .child(
                div().text_sm().child(
                    "Monthly bookkeeping, payroll for 12 people, and the quarterly estimate.",
                ),
            )
    }
}

/// An invoice list beside the open invoice. Drag the handle, focus it and
/// use the arrow keys, or double-click it to put it back.
pub struct SplitViewStory {
    split: Entity<SplitView>,
    remembered: f32,
    _events: Subscription,
}

impl SplitViewStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let list = cx.new(|_| InvoiceList);
            let detail = cx.new(|_| InvoiceDetail);
            let split = cx.new(|cx| {
                SplitView::new(list, detail, 0.35, cx)
                    .min_widths(180.0, 240.0)
                    .label("Resize the invoice list")
            });
            let events = cx.subscribe(&split, |this: &mut Self, _, event: &SplitEvent, cx| {
                if let SplitEvent::Resized(share) = event {
                    this.remembered = *share;
                    cx.notify();
                }
            });
            Self {
                split,
                remembered: 0.35,
                _events: events,
            }
        })
    }
}

impl Story for SplitViewStory {
    fn title() -> &'static str {
        "Split view"
    }

    fn description() -> &'static str {
        "Two panes side by side with a handle between them that you drag, nudge with the arrow keys, or double-click to put back."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SplitViewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (line, quiet) = (theme.border, theme.muted_foreground);
        let folded = cx.entity();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("fold")
                            .small()
                            .outline()
                            .label("Fold the list")
                            .on_click(move |_, _, cx| {
                                folded.update(cx, |this, cx| {
                                    this.split.update(cx, |s, cx| s.set_folded(true, cx))
                                })
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child(format!("Remembered split: {:.0}%", self.remembered * 100.0)),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(260.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(line)
                    .overflow_hidden()
                    .child(self.split.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Double-click the handle to open a folded list."),
            )
    }
}
