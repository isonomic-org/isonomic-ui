// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Whole screens, each a realistic window composed only from components that
//! already exist in the base, component and extra crates. `build.rs` finds the
//! files here and registers each screen with the gallery, after the stories of
//! the components themselves.

include!(concat!(env!("OUT_DIR"), "/screen_stories.rs"));

use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{App, Div, Pixels, Styled as _, px};

/// How tall every screen is: it fits the gallery's default window with the
/// story header above it.
pub(crate) const SCREEN_HEIGHT: Pixels = px(860.);

/// The window a screen is drawn in: the full width of the story, a fixed
/// height, a hairline edge and the app's background.
pub(crate) fn window_frame(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .min_w(px(880.))
        .h(SCREEN_HEIGHT)
        .overflow_hidden()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
}
