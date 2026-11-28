// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::App;

pub use isonomic_ui_base::GlobalState;

pub(crate) fn init(cx: &mut App) {
    // Preserve the legacy initialization point while `isonomic_ui_base::init` remains
    // after Root initialization for focus-trap ordering compatibility.
    GlobalState::init(cx);
}
