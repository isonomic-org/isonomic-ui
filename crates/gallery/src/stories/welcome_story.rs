// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use isonomic_ui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, Render, Styled as _, Window, px,
};

use isonomic_ui::component::{dock::PanelControl, text::markdown};

use crate::Story;

pub struct WelcomeStory {
    focus_handle: FocusHandle,
}

impl WelcomeStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Story for WelcomeStory {
    fn title() -> &'static str {
        "Introduction"
    }

    fn description() -> &'static str {
        "UI components for building fantastic desktop application by using GPUI."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn zoomable() -> Option<PanelControl> {
        None
    }

    fn paddings() -> isonomic_ui::Pixels {
        px(0.)
    }
}

impl Focusable for WelcomeStory {
    fn focus_handle(&self, _: &isonomic_ui::App) -> isonomic_ui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WelcomeStory {
    fn render(
        &mut self,
        _: &mut isonomic_ui::Window,
        _: &mut isonomic_ui::Context<Self>,
    ) -> impl isonomic_ui::IntoElement {
        markdown(include_str!("../../../../README.md"))
            .px_4()
            .scrollable(true)
            .selectable(true)
    }
}
