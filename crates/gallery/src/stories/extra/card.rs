// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the card.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::card::{Card, CardFooter, CardHeader};

const MODES: [(&str, &str); 2] = [
    ("Review first", "Nothing goes out until you approve it"),
    ("Send automatically", "Pauses only for payments"),
];

/// Shows a plain card with an action and a footer, and a pair of cards used as choices.
pub struct CardStory {
    mode: usize,
    opened: usize,
    last: SharedString,
}

impl CardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            mode: 0,
            opened: 0,
            last: "Nothing pressed yet".into(),
        })
    }

    fn stat(label: &'static str, value: &'static str, cx: &App) -> impl IntoElement {
        v_flex()
            .flex_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(div().text_base().child(value))
    }
}

impl Story for CardStory {
    fn title() -> &'static str {
        "Card"
    }

    fn description() -> &'static str {
        "A self-contained surface with a header, content and a footer."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .max_w(px(400.))
            .gap_6()
            .child(
                Card::new("sync")
                    .header(
                        CardHeader::new()
                            .title("Invoice sync")
                            .description("Matches bank lines to open invoices every hour")
                            .action(
                                Button::new("sync-more")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Ellipsis)
                                    .accessibility_label("More actions")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.last = "Asked for more actions".into();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .child(Self::stat("Matched", "318", cx))
                            .child(Self::stat("Unmatched", "4", cx))
                            .child(Self::stat("Last run", "09:00", cx)),
                    )
                    .footer(
                        CardFooter::new().child(self.last.clone()).end(
                            Button::new("sync-open")
                                .outline()
                                .small()
                                .label("Open")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.opened += 1;
                                    this.last = match this.opened {
                                        1 => "Opened once".into(),
                                        n => format!("Opened {n} times").into(),
                                    };
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .items_stretch()
                    .gap_2()
                    .children(MODES.iter().enumerate().map(|(ix, (title, text))| {
                        Card::new(("mode", ix))
                            .flex_1()
                            .selected(self.mode == ix)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mode = ix;
                                cx.notify();
                            }))
                            .header(CardHeader::new().title(*title).description(*text))
                    })),
            )
    }
}
