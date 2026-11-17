// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use gpui::App;

pub use isonomic_ui_base::GlobalState;

pub(crate) fn init(cx: &mut App) {
    // Preserve the legacy initialization point while `isonomic_ui_base::init` remains
    // after Root initialization for focus-trap ordering compatibility.
    GlobalState::init(cx);
}
