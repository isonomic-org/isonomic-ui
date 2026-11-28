// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Every path `isonomic-ui` promises resolves, under every feature set.
//!
//! These are compile-time checks: each `use` or type alias fails to build if
//! a re-export goes missing or moves, which is the regression that matters
//! for an umbrella crate.

#![allow(unused_imports, dead_code)]

/// `use isonomic_ui::*;` alone is GPUI, so a file needs nothing else for it.
mod glob_is_gpui {
    use isonomic_ui::*;

    type Element = Div;
    type Window_ = Window;
    type App_ = App;
    type Geometry = Size<Pixels>;

    actions!(exports, [Quit]);

    #[derive(Action, Clone, PartialEq)]
    struct DerivedAction;

    #[derive(Render)]
    struct DerivedRender;

    #[derive(IntoElement)]
    struct DerivedElement;

    impl RenderOnce for DerivedElement {
        fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
            div()
        }
    }

    fn build() -> impl IntoElement {
        div().child("hello").bg(red())
    }

    fn startup(cx: &mut App) {
        isonomic_ui::init(cx);
        let _ = isonomic_ui::application;
    }
}

/// Importing only the macro must not require a transitive crate named `gpui`.
mod selective_actions_import {
    use isonomic_ui::actions;

    actions!(exports, [SelectivelyImportedAction]);

    fn assert_action<T: isonomic_ui::Action>() {}

    fn action_is_from_the_facade() {
        assert_action::<SelectivelyImportedAction>();
    }
}

/// `isonomic_ui::gpui` is hidden but kept, for code that keeps `gpui::…` paths.
mod gpui_by_name {
    use isonomic_ui::*;

    type Through = gpui::Window;
    type Spelled = isonomic_ui::gpui::Window;
}

/// The layers that are always present.
mod always {
    type Base = isonomic_ui::base::Button;
    const _APPLICATION: fn() -> isonomic_ui::Application = isonomic_ui::application;
    const _PLATFORM_APPLICATION: fn() -> isonomic_ui::Application =
        isonomic_ui::platform::application;
    const _INIT: fn(&mut isonomic_ui::App) = isonomic_ui::init;
}

#[cfg(feature = "component")]
mod component {
    use isonomic_ui::component::button::*;
    use isonomic_ui::component::plot::{IntoPlot, Plot};
    use isonomic_ui::component::{ActiveTheme, Root, Size};
    use isonomic_ui::*;

    type Sizing = Size;

    #[derive(IntoPlot)]
    struct DerivedPlot;

    impl Plot for DerivedPlot {
        fn paint(&mut self, _: Bounds<Pixels>, _: &mut Window, _: &mut App) {}
    }

    fn theme(cx: &App) -> Hsla {
        cx.theme().background
    }

    fn button() -> Button {
        Button::new("ok").primary()
    }

    const _INIT: fn(&mut App) = isonomic_ui::component::init;
}

#[cfg(feature = "assets")]
mod assets {
    type Assets = isonomic_ui::assets::Assets;
}

/// The opt-in facade feature enables GPUI's feature-gated profiler API.
#[cfg(feature = "profiler")]
use isonomic_ui::profiler::WindowProfiler;
