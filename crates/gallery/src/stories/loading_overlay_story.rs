// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::component::{
    ActiveTheme as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    description_list::DescriptionList,
    h_flex,
    loading_overlay::LoadingOverlay,
    switch::Switch,
    v_flex,
};
use isonomic_ui::*;

use crate::section;

pub struct LoadingOverlayStory {
    saving: bool,
    refreshing: bool,
}

impl super::Story for LoadingOverlayStory {
    fn title() -> &'static str {
        "Loading Overlay"
    }

    fn description() -> &'static str {
        "A busy layer over content that is loading or saving."
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|_| Self {
            saving: true,
            refreshing: false,
        })
    }
}

fn invoice(cx: &App) -> impl IntoElement {
    v_flex()
        .w_full()
        .p_4()
        .gap_3()
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius_lg)
        .child(div().text_sm().font_semibold().child("Invoice INV-2041"))
        .child(
            DescriptionList::new()
                .columns(1)
                .item("Customer", "Northwind Studio", 1)
                .item("Amount", "$2,400.00", 1)
                .item("Due", "30 Nov 2026", 1)
                .item("Status", "Draft", 1),
        )
        .child(
            h_flex()
                .gap_2()
                .child(Button::new("send").primary().label("Send"))
                .child(Button::new("edit").outline().label("Edit")),
        )
}

impl Render for LoadingOverlayStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                section("Saving")
                    .description(
                        "The content keeps its place and cannot be pressed while it saves.",
                    )
                    .v_flex()
                    .gap_3()
                    .child(
                        Switch::new("saving")
                            .label("Saving")
                            .checked(self.saving)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.saving = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        div().w_full().max_w(px(420.)).child(
                            LoadingOverlay::new("saving-overlay")
                                .loading(self.saving)
                                .label("Saving")
                                .child(invoice(cx)),
                        ),
                    ),
            )
            .child(
                section("Refreshing in place")
                    .description("Without dimming, for data that stays legible while it reloads.")
                    .v_flex()
                    .gap_3()
                    .child(
                        Switch::new("refreshing")
                            .label("Refreshing")
                            .checked(self.refreshing)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.refreshing = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        div().w_full().max_w(px(420.)).child(
                            LoadingOverlay::new("refresh-overlay")
                                .loading(self.refreshing)
                                .dim_content(false)
                                .child(invoice(cx)),
                        ),
                    ),
            )
    }
}
