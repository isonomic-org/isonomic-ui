// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the labelled divider.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::labelled_divider::{LabelPlacement, LabelledDivider, day_heading};

/// An activity feed split into days, a centred "or" between two sign-in
/// choices, and upright rules in a toolbar.
pub struct LabelledDividerStory {
    read: bool,
}

impl LabelledDividerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { read: false })
    }
}

impl Story for LabelledDividerStory {
    fn title() -> &'static str {
        "Labelled divider"
    }

    fn description() -> &'static str {
        "A rule that splits a list or feed into named runs, with an optional action at the end."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for LabelledDividerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let read = self.read;
        let mark = cx.entity();
        let ink = cx.theme().foreground;
        let entry_text = move |text: &'static str| {
            div()
                .py_1()
                .text_sm()
                .text_color(if read { quiet } else { ink })
                .child(text)
        };
        let days = [
            (
                0,
                "Sat 15 Nov",
                [
                    "Invoice INV-2041 paid by Harbor Dental",
                    "Receipt matched to fuel card",
                ],
            ),
            (
                -1,
                "Fri 14 Nov",
                [
                    "Bank feed synced, 38 transactions",
                    "Payroll approved for 12 people",
                ],
            ),
            (
                -4,
                "Tue 11 Nov",
                [
                    "Quarterly estimate drafted",
                    "Vendor W-9 received from Brightline",
                ],
            ),
        ];
        v_flex()
            .gap_6()
            .child(
                v_flex()
                    .w(px(440.))
                    .gap_1()
                    .children(days.into_iter().enumerate().flat_map(
                        |(ix, (offset, date, items))| {
                            let mark = mark.clone();
                            let divider =
                                LabelledDivider::new(("day", ix)).label(day_heading(offset, date));
                            let divider = if ix == 0 && !read {
                                divider.action("Mark all read", move |_, _, cx| {
                                    mark.update(cx, |this, cx| {
                                        this.read = true;
                                        cx.notify();
                                    })
                                })
                            } else {
                                divider
                            };
                            std::iter::once(divider.mt_2().into_any_element()).chain(
                                items
                                    .into_iter()
                                    .map(move |item| entry_text(item).into_any_element()),
                            )
                        },
                    )),
            )
            .child(
                v_flex()
                    .w(px(280.))
                    .gap_2()
                    .child(div().text_sm().child("Continue with your work email"))
                    .child(
                        LabelledDivider::new("or")
                            .label("or")
                            .placement(LabelPlacement::Center),
                    )
                    .child(div().text_sm().child("Use a one-time code instead")),
            )
            .child(
                h_flex()
                    .h(px(24.))
                    .gap_3()
                    .text_sm()
                    .child("Bold")
                    .child("Italic")
                    .child(LabelledDivider::new("toolbar-rule").vertical())
                    .child("Link")
                    .child(LabelledDivider::new("toolbar-rule-2").vertical())
                    .child(div().text_color(quiet).child("Saved")),
            )
    }
}
