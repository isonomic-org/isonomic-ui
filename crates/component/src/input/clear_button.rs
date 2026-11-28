// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::App;
use rust_i18n::t;

use crate::{
    Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};

#[inline]
pub(crate) fn clear_button(_: &App) -> Button {
    Button::new("clean")
        .icon(Icon::new(IconName::Close))
        // Icon-only: screen readers have nothing else to announce.
        .accessibility_label(t!("Input.Clear"))
        .text()
        .xsmall()
        .tab_stop(false)
}
