// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the quick edit popover.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div, px,
};
use isonomic_ui_extra::quick_edit_popover::{QuickEditEvent, QuickEditPopover};

/// An invoice header whose title and reference edit in place. A reference
/// with a space in it is refused.
pub struct QuickEditPopoverStory {
    title: Entity<QuickEditPopover>,
    reference: Entity<QuickEditPopover>,
    _events: Vec<Subscription>,
}

fn saves(
    cx: &mut Context<QuickEditPopoverStory>,
    field: &Entity<QuickEditPopover>,
    refuse_spaces: bool,
) -> Subscription {
    cx.subscribe(field, move |_, field, event: &QuickEditEvent, cx| {
        let QuickEditEvent::Save(text) = event.clone();
        cx.spawn(async move |_, cx: &mut isonomic_ui::AsyncApp| {
            cx.background_executor()
                .timer(Duration::from_millis(600))
                .await;
            field.update(cx, |field, cx| {
                if refuse_spaces && text.contains(' ') {
                    field.save_failed("References can't contain spaces.", cx);
                } else {
                    field.saved(text, cx);
                }
            });
        })
        .detach();
    })
}

impl QuickEditPopoverStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let title = cx.new(|cx| {
                QuickEditPopover::new("title", "Invoice title", "March retainer", window, cx)
            });
            let reference = cx.new(|cx| {
                QuickEditPopover::new("reference", "Client reference", "PO-4471", window, cx)
                    .allow_blank()
            });
            let events = vec![saves(cx, &title, false), saves(cx, &reference, true)];
            Self {
                title,
                reference,
                _events: events,
            }
        })
    }
}

impl Story for QuickEditPopoverStory {
    fn title() -> &'static str {
        "Quick edit popover"
    }

    fn description() -> &'static str {
        "A value shown in place that opens a small popover to change it, with Save and Cancel."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for QuickEditPopoverStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let row = |label: &'static str| div().w(px(120.)).text_sm().text_color(quiet).child(label);
        v_flex()
            .gap_2()
            .child(div().text_lg().child("INV-2041 \u{b7} Harbor Dental"))
            .child(
                h_flex()
                    .gap_2()
                    .child(row("Title"))
                    .child(self.title.clone()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(row("Reference"))
                    .child(self.reference.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Try a reference with a space to see a refused save."),
            )
    }
}
