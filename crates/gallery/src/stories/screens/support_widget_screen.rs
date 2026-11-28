// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A support panel floating over a shop's page, twice: just opened, with a
//! greeting and suggested questions, and later, with a short thread, a photo
//! attached and the conversation handed to a person.

use crate::Story;
use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _,
    attachment::{
        Attachment, AttachmentContent, AttachmentDescription, AttachmentMedia, AttachmentTitle,
    },
    button::{Button, ButtonVariants as _},
    h_flex,
    skeleton::Skeleton,
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div, px,
};
use isonomic_ui_extra::chat_thread::{ChatThread, Density, Speaker, ThreadMessage};
use isonomic_ui_extra::common::Tone;
use isonomic_ui_extra::identity_avatar::{Identity, IdentityAvatar, IdentityStack, Presence};
use isonomic_ui_extra::prompt_composer::{ComposerAttachment, PromptComposer};
use isonomic_ui_extra::prompt_suggestions::{PromptSuggestions, Suggestion};
use isonomic_ui_extra::status_badge::StatusBadge;

use super::window_frame;

/// Tuesday 17 November 2026, 14:20 UTC: the made-up present of this screen.
const NOW: i64 = 1_794_925_200;
const MINUTE: i64 = 60;
/// How wide and tall each floating panel is.
const PANEL_WIDTH: f32 = 360.;
const PANEL_HEIGHT: f32 = 640.;

fn team() -> Vec<Identity> {
    vec![
        Identity::agent("Leafline assistant").presence(Presence::Online),
        Identity::person("Priya Nair").presence(Presence::Online),
        Identity::person("Sam Okoro").presence(Presence::Away),
    ]
}

/// A plant shop's help panel, opened fresh and handed over to a person.
pub struct SupportWidgetScreenStory {
    fresh: Entity<PromptComposer>,
    handed: Entity<PromptComposer>,
    asked: Option<SharedString>,
}

impl SupportWidgetScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let fresh = cx.new(|cx| PromptComposer::new("Ask a question\u{2026}", window, cx));
            fresh.update(cx, |composer, cx| composer.set_dictation(false, cx));
            let handed = cx.new(|cx| PromptComposer::new("Write to Priya\u{2026}", window, cx));
            handed.update(cx, |composer, cx| {
                composer.set_dictation(false, cx);
                composer.set_attachments(
                    vec![
                        ComposerAttachment::new("underside", "leaf-underside.jpg").detail("1.2 MB"),
                    ],
                    cx,
                );
            });
            Self {
                fresh,
                handed,
                asked: None,
            }
        })
    }

    fn header(&self, id: &'static str, cx: &App, handed: bool) -> AnyElement {
        let quiet = cx.theme().muted_foreground;
        h_flex()
            .flex_none()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(if handed {
                IdentityAvatar::new(
                    (id, 0usize),
                    Identity::person("Priya Nair").presence(Presence::Online),
                )
                .size(32.)
                .into_any_element()
            } else {
                IdentityStack::new((id, 1usize), team())
                    .size(28.)
                    .into_any_element()
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().font_medium().child(if handed {
                        "Priya, plant care team"
                    } else {
                        "Leafline help"
                    }))
                    .child(div().text_xs().text_color(quiet).child(if handed {
                        "Joins in about 2 minutes"
                    } else {
                        "We usually reply in a few minutes"
                    })),
            )
            .child(
                Button::new((id, 2usize))
                    .ghost()
                    .xsmall()
                    .icon(CatalogIcon::Subtract)
                    .tooltip("Minimise"),
            )
            .into_any_element()
    }

    fn panel(&self, cx: &App, body: AnyElement) -> AnyElement {
        let theme = cx.theme();
        v_flex()
            .flex_none()
            .w(px(PANEL_WIDTH))
            .h(px(PANEL_HEIGHT))
            .overflow_hidden()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow_lg()
            .child(body)
            .into_any_element()
    }

    fn fresh_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let asked = cx.entity();
        let quiet = cx.theme().muted_foreground;
        let body = v_flex()
            .size_full()
            .child(self.header("fresh-panel", cx, false))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_4()
                    .p_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .child("Hi, how can we help?"),
                            )
                            .child(div().text_sm().text_color(quiet).child(
                                "Ask about an order, a plant that looks unwell, or delivery \
                                 to your area.",
                            )),
                    )
                    .child(
                        PromptSuggestions::new(
                            "fresh-suggestions",
                            vec![
                                Suggestion::new("where", "Where is my order?")
                                    .hint("Tracking and delivery days")
                                    .icon(CatalogIcon::Delivery),
                                Suggestion::new("leaves", "Why are my leaves turning yellow?")
                                    .hint("Send a photo for a quicker answer")
                                    .icon(CatalogIcon::Idea),
                                Suggestion::new("return", "Return a plant that arrived damaged")
                                    .hint("Within 14 days of delivery")
                                    .icon(CatalogIcon::Email),
                            ],
                        )
                        .columns(1)
                        .on_select(move |_, text, _, cx| {
                            let text = text.clone();
                            asked.update(cx, |this, cx| {
                                this.asked = Some(text);
                                cx.notify();
                            })
                        }),
                    )
                    .when_asked(self.asked.clone(), quiet),
            )
            .child(div().flex_none().p_3().child(self.fresh.clone()));
        self.panel(cx, body.into_any_element())
    }

    fn handed_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let photo = Attachment::new()
            .id("support-photo")
            .media(AttachmentMedia::new().child(Icon::new(CatalogIcon::Image)))
            .content(
                AttachmentContent::new()
                    .title(AttachmentTitle::new("monstera-leaf.jpg"))
                    .description(AttachmentDescription::new("Photo \u{b7} 2.4 MB")),
            );
        let handoff = v_flex()
            .gap_2()
            .p_3()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        IdentityAvatar::new(
                            "handoff-person",
                            Identity::person("Priya Nair").presence(Presence::Online),
                        )
                        .size(24.),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_medium()
                            .child("Handing you to Priya"),
                    )
                    .child(
                        StatusBadge::new("handoff-wait", "In line")
                            .tone(Tone::Info)
                            .live(true),
                    ),
            )
            .child(div().text_xs().text_color(quiet).child(
                "You are second in line, about 2 minutes. She can see this conversation and \
                 your photos, so there is no need to repeat anything.",
            ));
        let messages = vec![
            ThreadMessage::new(
                "ask",
                Speaker::Person,
                "My monstera has brown spots with yellow edges. Bought it here three weeks ago.",
                NOW - 6 * MINUTE,
            ),
            ThreadMessage::new(
                "photo",
                Speaker::Person,
                "monstera-leaf.jpg",
                NOW - 6 * MINUTE,
            )
            .content(photo),
            ThreadMessage::new(
                "answer",
                Speaker::Assistant,
                "Spots with a yellow ring are often too much water. Let the top 5 cm of soil dry \
                 out before watering again, and check that the pot drains.",
                NOW - 5 * MINUTE,
            ),
            ThreadMessage::new(
                "person",
                Speaker::Person,
                "I water once a week. Can I talk to someone? It is a 60 euro plant.",
                NOW - 2 * MINUTE,
            ),
            ThreadMessage::new(
                "handoff",
                Speaker::Assistant,
                "Handing you to Priya",
                NOW - MINUTE,
            )
            .content(handoff),
        ];
        let body = v_flex()
            .size_full()
            .child(self.header("handed-panel", cx, true))
            .child(
                div().flex_1().min_h_0().child(
                    ChatThread::new("support-thread", messages)
                        .bare()
                        .density(Density::Dense)
                        .assistant_name("Assistant")
                        .person_name("You")
                        .now(NOW, 0),
                ),
            )
            .child(div().flex_none().p_3().child(self.handed.clone()));
        self.panel(cx, body.into_any_element())
    }
}

/// Says which suggestion was picked, under the suggestions.
trait WhenAsked {
    fn when_asked(self, asked: Option<SharedString>, color: isonomic_ui::Hsla) -> Self;
}

impl WhenAsked for isonomic_ui::Div {
    fn when_asked(self, asked: Option<SharedString>, color: isonomic_ui::Hsla) -> Self {
        match asked {
            Some(text) => self.child(
                div()
                    .text_xs()
                    .text_color(color)
                    .child(format!("Sending: {text}")),
            ),
            None => self,
        }
    }
}

impl Story for SupportWidgetScreenStory {
    fn title() -> &'static str {
        "Support widget screen"
    }

    fn description() -> &'static str {
        "A support panel over a shop's page, built from the library: a greeting with suggested questions, and a short thread with a photo handed to a person."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SupportWidgetScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fresh = self.fresh_panel(cx);
        let handed = self.handed_panel(cx);
        let line = cx.theme().border;

        // The shop's own page, quiet behind the panels.
        let page = v_flex()
            .size_full()
            .child(
                h_flex()
                    .flex_none()
                    .h(px(56.))
                    .px_6()
                    .gap_6()
                    .border_b_1()
                    .border_color(line)
                    .child(div().font_semibold().child("Leafline"))
                    .child(Skeleton::new().w(px(64.)).h(px(10.)))
                    .child(Skeleton::new().w(px(80.)).h(px(10.)))
                    .child(Skeleton::new().w(px(56.)).h(px(10.))),
            )
            .child(
                v_flex()
                    .p_6()
                    .gap_4()
                    .child(Skeleton::new().w(px(320.)).h(px(24.)))
                    .child(Skeleton::new().w(px(220.)).h(px(12.)))
                    .child(h_flex().gap_4().children((0..4).map(|_| {
                        v_flex()
                            .gap_2()
                            .child(Skeleton::new().secondary().w(px(180.)).h(px(180.)))
                            .child(Skeleton::new().w(px(120.)).h(px(10.)))
                            .child(Skeleton::new().w(px(60.)).h(px(10.)))
                    }))),
            );

        window_frame(cx).relative().child(page).child(
            h_flex()
                .absolute()
                .right(px(20.))
                .bottom(px(20.))
                .items_end()
                .gap_5()
                .child(fresh)
                .child(handed),
        )
    }
}
