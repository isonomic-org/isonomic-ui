// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Action button: the kit's button with the states an action goes through: working, done, failed, and a second click to confirm.
//!
//! Idle, it's a plain button. Working, it shows a spinner and its working
//! words, and ignores clicks. Done, it shows a tick and the done words for
//! a moment (the owner decides how long). Failed, it turns to the danger
//! colour with its failure words and can be clicked to try again. A
//! destructive action asks for a second click, saying what will happen.
//!
//! It comes in two sizes (small, the default, and medium), four emphases
//! (primary, secondary, ghost and danger) and an icon-only form that keeps
//! its words for the tooltip and for a screen reader.

use std::rc::Rc;

use isonomic_ui::assets::IconName as CatalogIcon;

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, Size,
    button::{Button, ButtonVariants as _},
};
use isonomic_ui::{
    App, ElementId, IntoElement, RenderOnce, SharedString, Styled as _, Window,
    prelude::FluentBuilder as _,
};

/// Where the action is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionState {
    /// Ready.
    #[default]
    Idle,
    /// Asking for a second click.
    Confirming,
    /// Under way.
    Working,
    /// Finished.
    Done,
    /// Went wrong.
    Failed,
}

/// What a click means in a state, for a destructive or ordinary action.
pub fn click_means(state: ActionState, destructive: bool) -> Option<ActionState> {
    match (state, destructive) {
        (ActionState::Idle, true) => Some(ActionState::Confirming),
        (ActionState::Idle | ActionState::Confirming | ActionState::Failed, _) => {
            Some(ActionState::Working)
        }
        (ActionState::Working | ActionState::Done, _) => None,
    }
}

/// How loudly the button asks to be pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionEmphasis {
    /// Solid ink: the one main action.
    #[default]
    Primary,
    /// Outlined: an action beside the main one.
    Secondary,
    /// No fill until the pointer is over it: a quiet action in a toolbar.
    Ghost,
    /// The danger colour: an action that destroys something.
    Danger,
}

/// The emphasis a state paints with: asking to confirm and failing always
/// use the danger colour, and so does a destructive action.
pub fn emphasis_in(
    state: ActionState,
    destructive: bool,
    chosen: ActionEmphasis,
) -> ActionEmphasis {
    match (state, destructive) {
        (ActionState::Confirming | ActionState::Failed, _) | (_, true) => ActionEmphasis::Danger,
        _ => chosen,
    }
}

/// What a screen reader hears: the action alone while idle, then the action
/// and where it is.
pub fn spoken_label(label: &str, state_words: &str, state: ActionState) -> String {
    match state {
        ActionState::Idle => label.to_string(),
        _ => format!("{label}: {state_words}"),
    }
}

type ClickHandler = Rc<dyn Fn(ActionState, &mut Window, &mut App)>;

/// A button that shows an action's progress.
#[derive(IntoElement)]
pub struct ActionButton {
    id: ElementId,
    label: SharedString,
    working: SharedString,
    done: SharedString,
    failed: SharedString,
    confirm: SharedString,
    state: ActionState,
    destructive: bool,
    emphasis: ActionEmphasis,
    size: Size,
    icon_only: bool,
    icon: Option<Icon>,
    on_click: Option<ClickHandler>,
}

impl ActionButton {
    /// Create an ordinary action.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            working: "Working\u{2026}".into(),
            done: "Done".into(),
            failed: "Failed, try again".into(),
            confirm: "Click again to confirm".into(),
            state: ActionState::Idle,
            destructive: false,
            emphasis: ActionEmphasis::Primary,
            size: Size::Small,
            icon_only: false,
            icon: None,
            on_click: None,
        }
    }

    /// Set the words for working, done and failed.
    pub fn words(
        mut self,
        working: impl Into<SharedString>,
        done: impl Into<SharedString>,
        failed: impl Into<SharedString>,
    ) -> Self {
        self.working = working.into();
        self.done = done.into();
        self.failed = failed.into();
        self
    }

    /// Ask for a second click first, saying what will happen.
    pub fn destructive(mut self, confirm: impl Into<SharedString>) -> Self {
        self.destructive = true;
        self.confirm = confirm.into();
        self
    }

    /// Use the outline look instead of the primary one.
    pub fn secondary(mut self) -> Self {
        self.emphasis = ActionEmphasis::Secondary;
        self
    }

    /// Choose how loudly the button asks to be pressed. Asking to confirm
    /// and failing still use the danger colour.
    pub fn emphasis(mut self, emphasis: ActionEmphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Use the medium size, 28px tall, instead of the small 24px one.
    pub fn medium(mut self) -> Self {
        self.size = Size::Medium;
        self
    }

    /// Show only the icon. The words move to the tooltip and the spoken
    /// name; working shows the spinner in the icon's place.
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// Show an icon when idle.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set where the action is.
    pub fn state(mut self, state: ActionState) -> Self {
        self.state = state;
        self
    }

    /// Run with the state a click asks for: confirming, or working.
    pub fn on_click(
        mut self,
        handler: impl Fn(ActionState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let success = cx.theme().success;
        let state = self.state;
        let destructive = self.destructive;
        let on_click = self.on_click.clone();
        let label = match state {
            ActionState::Idle => self.label.clone(),
            ActionState::Confirming => self.confirm.clone(),
            ActionState::Working => self.working.clone(),
            ActionState::Done => self.done.clone(),
            ActionState::Failed => self.failed.clone(),
        };
        let icon_only = self.icon_only;
        let icon: Option<Icon> = match state {
            ActionState::Idle => self.icon.clone(),
            ActionState::Confirming => icon_only.then(|| Icon::new(CatalogIcon::WarningAlt)),
            ActionState::Working => self.icon.clone().filter(|_| icon_only),
            ActionState::Done => Some(Icon::new(IconName::Check).text_color(success)),
            ActionState::Failed => Some(Icon::new(IconName::RefreshCw)),
        };
        // An icon-only button needs something to show while idle.
        let icon = icon.or_else(|| icon_only.then(|| Icon::new(CatalogIcon::Play)));
        Button::new(self.id.clone())
            .with_size(self.size)
            .map(|b| match emphasis_in(state, destructive, self.emphasis) {
                ActionEmphasis::Primary => b.primary(),
                ActionEmphasis::Secondary => b.outline(),
                ActionEmphasis::Ghost => b.ghost(),
                ActionEmphasis::Danger => b.danger(),
            })
            .when_some(icon, |b, icon| b.icon(icon))
            .when(!icon_only, |b| b.label(label.clone()))
            .when(icon_only, |b| b.tooltip(label.clone()))
            // Working uses the base loading state: a spinner, the button's
            // own colours, and clicks ignored.
            .loading(state == ActionState::Working)
            .disabled(state == ActionState::Done)
            .accessibility_label(spoken_label(&self.label, &label, state))
            .on_click(move |_, window, cx| {
                if let (Some(next), Some(on_click)) = (click_means(state, destructive), &on_click) {
                    on_click(next, window, cx);
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_actions_ask_twice() {
        assert_eq!(
            click_means(ActionState::Idle, true),
            Some(ActionState::Confirming)
        );
        assert_eq!(
            click_means(ActionState::Confirming, true),
            Some(ActionState::Working)
        );
        assert_eq!(
            click_means(ActionState::Idle, false),
            Some(ActionState::Working)
        );
        assert_eq!(click_means(ActionState::Working, false), None);
        assert_eq!(
            click_means(ActionState::Failed, false),
            Some(ActionState::Working)
        );
    }

    #[test]
    fn confirming_and_failing_paint_as_danger() {
        use ActionEmphasis::*;
        assert_eq!(emphasis_in(ActionState::Idle, false, Ghost), Ghost);
        assert_eq!(
            emphasis_in(ActionState::Working, false, Secondary),
            Secondary
        );
        assert_eq!(emphasis_in(ActionState::Confirming, false, Ghost), Danger);
        assert_eq!(emphasis_in(ActionState::Failed, false, Primary), Danger);
        assert_eq!(emphasis_in(ActionState::Idle, true, Primary), Danger);
    }

    #[test]
    fn spoken_label_names_the_state_after_idle() {
        assert_eq!(
            spoken_label("Archive", "Archive", ActionState::Idle),
            "Archive"
        );
        assert_eq!(
            spoken_label("Archive", "Archiving\u{2026}", ActionState::Working),
            "Archive: Archiving\u{2026}"
        );
    }
}
