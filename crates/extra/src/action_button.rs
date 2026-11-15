//! Action button: the kit's button with the states an action goes through: working, done, failed, and a second click to confirm.
//!
//! Idle, it's a plain button. Working, it shows a spinner and its working
//! words, and ignores clicks. Done, it shows a tick and the done words for
//! a moment (the owner decides how long). Failed, it turns to the danger
//! colour with its failure words and can be clicked to try again. A
//! destructive action asks for a second click, saying what will happen.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Transformation, Window, percentage, prelude::FluentBuilder as _,
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
    primary: bool,
    icon: Option<IconName>,
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
            primary: true,
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
        self.primary = false;
        self
    }

    /// Show an icon when idle.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
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
        let spinner_id = ElementId::NamedChild(self.id.clone().into(), "spin".into());
        Button::new(self.id.clone())
            .small()
            .map(|b| match (state, destructive, self.primary) {
                (ActionState::Confirming | ActionState::Failed, _, _) | (_, true, _) => b.danger(),
                (_, _, true) => b.primary(),
                _ => b.outline(),
            })
            .when(state == ActionState::Idle, |b| match self.icon {
                Some(icon) => b.icon(icon),
                None => b,
            })
            .when(state == ActionState::Done, |b| {
                b.icon(Icon::new(IconName::Check).text_color(success))
            })
            .when(state == ActionState::Failed, |b| {
                b.icon(IconName::RefreshCw)
            })
            .when(state == ActionState::Working, |b| {
                b.child(Icon::new(IconName::LoaderCircle).xsmall().with_animation(
                    spinner_id,
                    Animation::new(std::time::Duration::from_secs(1)).repeat(),
                    |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                ))
            })
            .label(label.clone())
            .disabled(matches!(state, ActionState::Working | ActionState::Done))
            .accessibility_label(match state {
                ActionState::Idle => self.label.to_string(),
                _ => format!("{}: {label}", self.label),
            })
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
}
