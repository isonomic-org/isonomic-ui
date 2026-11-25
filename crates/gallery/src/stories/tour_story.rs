// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    tour::{Tour, TourEvent, TourState, TourStep, TourTarget},
    v_flex,
};
use isonomic_ui::*;

use crate::section;

pub struct TourStory {
    tour: Entity<TourState>,
    status: SharedString,
    _subscription: Subscription,
}

impl super::Story for TourStory {
    fn title() -> &'static str {
        "Product Tour"
    }

    fn description() -> &'static str {
        "A short sequence of cards, each pointing at a part of the interface."
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| {
            let tour = cx.new(|cx| {
                TourState::new(
                    vec![
                        TourStep::new(
                            "search",
                            "Search everything",
                            "Find chats, files and people from one place.",
                        ),
                        TourStep::new(
                            "new",
                            "Start a new chat",
                            "Each chat keeps its own context and history.",
                        ),
                        TourStep::new(
                            "settings",
                            "Make it yours",
                            "Choose a theme, models and what is remembered.",
                        ),
                    ],
                    cx,
                )
            });
            let subscription = cx.subscribe(&tour, |this: &mut Self, _, event, cx| {
                this.status = match event {
                    TourEvent::Step(ix) => format!("On step {}", ix + 1).into(),
                    TourEvent::Finished => "Finished".into(),
                    TourEvent::Skipped(ix) => format!("Skipped at step {}", ix + 1).into(),
                };
                cx.notify();
            });
            Self {
                tour,
                status: "Not started".into(),
                _subscription: subscription,
            }
        })
    }
}

impl Focusable for TourStory {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.tour.focus_handle(cx)
    }
}

impl Render for TourStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tour = self.tour.clone();
        v_flex()
            .gap_3()
            .child(
                section("Three steps")
                    .description(
                        "Next and Back move between steps; the arrow keys do too, and Escape skips.",
                    )
                    .v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(px(520.))
                            .p_2()
                            .gap_2()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius_lg)
                            .child(
                                TourTarget::new(&self.tour, "search").child(
                                    Button::new("search")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Search)
                                        .label("Search"),
                                ),
                            )
                            .child(div().flex_1())
                            .child(
                                TourTarget::new(&self.tour, "new").child(
                                    Button::new("new")
                                        .primary()
                                        .small()
                                        .icon(IconName::Plus)
                                        .label("New chat"),
                                ),
                            )
                            .child(
                                TourTarget::new(&self.tour, "settings").child(
                                    Button::new("settings")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Settings)
                                        .accessibility_label("Settings")
                                        .tooltip("Settings"),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                Button::new("start")
                                    .outline()
                                    .label("Start the tour")
                                    .on_click(move |_, window, cx| {
                                        tour.update(cx, |tour, cx| tour.start(window, cx))
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.status.clone()),
                            ),
                    ),
            )
            .child(Tour::new(&self.tour))
    }
}
