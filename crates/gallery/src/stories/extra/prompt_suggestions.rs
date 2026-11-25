// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for prompt suggestions.

use std::time::Duration;

use crate::Story;
use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::prompt_suggestions::{PromptSuggestions, Suggestion};

const FOLLOW_UPS: [[&str; 3]; 2] = [
    [
        "Which glaze needs the hottest kiln?",
        "Make it a table",
        "What would a cheaper clay change?",
    ],
    [
        "Compare stoneware and porcelain",
        "Shorter, please",
        "Which step fails most often for beginners?",
    ],
];

/// Starting a conversation about a pottery studio, then following up.
pub struct PromptSuggestionsStory {
    sent: SharedString,
    set: usize,
    refreshing: bool,
}

impl PromptSuggestionsStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            sent: "Nothing sent yet".into(),
            set: 0,
            refreshing: false,
        })
    }

    fn starters() -> Vec<Suggestion> {
        vec![
            Suggestion::new("schedule", "Plan this week's firings")
                .hint("Two kilns, eleven orders")
                .icon(IconName::Calendar)
                .prompt("Plan this week's kiln firings for eleven orders across two kilns"),
            Suggestion::new("glaze", "Explain a glaze defect")
                .hint("Crawling, pinholes, crazing")
                .icon(CatalogIcon::Idea),
            Suggestion::new("price", "Price a custom dinner set")
                .hint("Materials, hours and margin")
                .icon(CatalogIcon::Calculator),
            Suggestion::new("reply", "Draft a reply to a late-order complaint")
                .hint("Kind, short, with a new date")
                .icon(CatalogIcon::Email),
        ]
    }
}

impl Story for PromptSuggestionsStory {
    fn title() -> &'static str {
        "Prompt suggestions"
    }

    fn description() -> &'static str {
        "Prompts to send with one click: cards for an empty conversation, chips for follow-ups."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PromptSuggestionsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let on_cards = cx.entity();
        let on_wide = cx.entity();
        let on_chips = cx.entity();
        let on_refresh = cx.entity();
        let chips = FOLLOW_UPS[self.set]
            .iter()
            .enumerate()
            .map(|(ix, title)| Suggestion::new(format!("{}-{ix}", self.set), *title))
            .collect();

        v_flex()
            .gap_8()
            .child(
                PromptSuggestions::new("starters", Self::starters())
                    .heading("What are we making today?")
                    .columns(2)
                    .on_select(move |_, text, _, cx| {
                        on_wide.update(cx, |this, cx| {
                            this.sent = format!("Sent: {text}").into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                PromptSuggestions::new("starters-narrow", Self::starters())
                    .w(px(328.))
                    .on_select(move |_, text, _, cx| {
                        on_cards.update(cx, |this, cx| {
                            this.sent = format!("Sent: {text}").into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                PromptSuggestions::new("follow-ups", chips)
                    .chips()
                    .heading("Follow up")
                    .refreshing(self.refreshing)
                    .on_select(move |_, text, _, cx| {
                        on_chips.update(cx, |this, cx| {
                            this.sent = format!("Sent: {text}").into();
                            cx.notify();
                        })
                    })
                    .on_refresh(move |_, cx| {
                        on_refresh.update(cx, |this, cx| {
                            this.refreshing = true;
                            cx.notify();
                            cx.spawn(async move |this, cx| {
                                cx.background_executor()
                                    .timer(Duration::from_millis(900))
                                    .await;
                                let _ = this.update(cx, |this, cx| {
                                    this.refreshing = false;
                                    this.set = (this.set + 1) % FOLLOW_UPS.len();
                                    cx.notify();
                                });
                            })
                            .detach();
                        })
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(quiet)
                    .child(self.sent.clone()),
            )
    }
}
