// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composable questionnaire controls.
//!
//! The behavior — answers, validation, navigation, focus and shortcuts — lives
//! in [`isonomic_ui_base::questionnaire`]; this module is its skin. The public path
//! stays `isonomic_ui_component::questionnaire::*` for both halves.

mod components;

pub use components::*;
pub use isonomic_ui_base::questionnaire::{
    QuestionnaireAnswer, QuestionnaireAnswerChange, QuestionnaireAnswers,
    QuestionnaireChoiceDefinition, QuestionnaireChoiceState, QuestionnaireEvent,
    QuestionnaireInputDefinition, QuestionnaireItemDefinition, QuestionnaireItemState,
    QuestionnaireItemStatus, QuestionnaireNavigationState, QuestionnaireProgressState,
    QuestionnaireSchemaError, QuestionnaireShortcutMode, QuestionnaireState,
    QuestionnaireSubmission, QuestionnaireSubmissionItem, QuestionnaireValidationContext,
    QuestionnaireValidationError, QuestionnaireValidator,
};

pub(crate) fn init(_: &mut gpui::App) {}
