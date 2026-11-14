//! Gallery story for the card.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
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
}

impl CardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { mode: 0, opened: 0 })
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

impl Render for CardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opened = match self.opened {
            0 => "Not opened yet".to_string(),
            1 => "Opened once".to_string(),
            n => format!("Opened {n} times"),
        };

        v_flex()
            .w(px(400.))
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
                                    .icon(IconName::Ellipsis),
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
                        CardFooter::new().child(opened).end(
                            Button::new("sync-open")
                                .outline()
                                .small()
                                .label("Open")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.opened += 1;
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
