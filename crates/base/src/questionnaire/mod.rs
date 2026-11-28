// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Questionnaire behavior: answers, validation, navigation, focus and shortcuts.
//!
//! The state machine lives here so an application can replace the visual
//! language without reimplementing it. `isonomic-ui-component` owns the skin.

mod control;
mod keyboard;
mod state;
mod types;

pub use control::QuestionnaireChoiceControl;
pub use keyboard::handle_key_down;
pub use state::*;
pub use types::*;
