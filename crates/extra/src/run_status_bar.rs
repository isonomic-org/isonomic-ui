//! Run status bar: one bar above the composer that says how the last run ended, or why the chat can't go on, with the one thing to do about it.
//!
//! There are five states. A run the person stopped, a run that was cut short
//! and can be continued, a run that failed and can be tried again, a
//! connection that is being restored, and a chat that is closed. The bar
//! owns no state: its owner picks the state, may change the words, and is
//! told when the action is pressed. A screen reader hears the bar as a
//! status, and the action is an ordinary button.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, button::Button, h_flex,
    spinner::Spinner, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::Tone;

/// What the bar is saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunStatus {
    /// The person stopped the run. Nothing to do.
    Stopped,
    /// The run ended before it was finished, such as at a length limit. It
    /// can be continued.
    CutShort,
    /// The run failed. It can be tried again.
    Failed,
    /// The connection dropped and is being restored. Nothing to do but wait.
    Reconnecting,
    /// The chat is closed and takes no more messages.
    Closed,
}

impl RunStatus {
    /// Every state, in the order above.
    pub const ALL: [RunStatus; 5] = [
        RunStatus::Stopped,
        RunStatus::CutShort,
        RunStatus::Failed,
        RunStatus::Reconnecting,
        RunStatus::Closed,
    ];

    /// The words shown when the owner gives none.
    pub fn default_message(self) -> &'static str {
        match self {
            Self::Stopped => "You stopped this answer.",
            Self::CutShort => "The answer was cut short.",
            Self::Failed => "Something went wrong.",
            Self::Reconnecting => "Reconnecting\u{2026}",
            Self::Closed => "This chat is closed.",
        }
    }

    /// The action's label when the owner gives none, for the states that
    /// have an action.
    pub fn default_action(self) -> Option<&'static str> {
        match self {
            Self::CutShort => Some("Continue"),
            Self::Failed => Some("Try again"),
            Self::Stopped | Self::Reconnecting | Self::Closed => None,
        }
    }

    /// The meaning the state carries, which picks its colour.
    pub fn tone(self) -> Tone {
        match self {
            Self::Stopped | Self::Closed => Tone::Neutral,
            Self::CutShort => Tone::Warning,
            Self::Failed => Tone::Danger,
            Self::Reconnecting => Tone::Info,
        }
    }

    fn icon(self) -> Option<IconName> {
        match self {
            Self::Stopped => Some(IconName::Square),
            Self::CutShort => Some(IconName::TriangleAlert),
            Self::Failed => Some(IconName::CircleAlert),
            // Reconnecting shows a spinner instead.
            Self::Reconnecting => None,
            Self::Closed => Some(IconName::Info),
        }
    }
}

/// What a screen reader hears for the bar, such as "Something went wrong.
/// The model ran out of memory."
fn spoken(message: &str, detail: Option<&str>) -> String {
    match detail {
        Some(detail) if !detail.is_empty() => format!("{message} {detail}"),
        _ => message.to_string(),
    }
}

type ActionHandler = Rc<dyn Fn(&RunStatus, &mut Window, &mut App)>;

/// The bar above the composer that says how the last run ended.
#[derive(IntoElement)]
pub struct RunStatusBar {
    id: ElementId,
    style: StyleRefinement,
    status: RunStatus,
    message: Option<SharedString>,
    detail: Option<SharedString>,
    action_label: Option<SharedString>,
    action_busy: bool,
    on_action: Option<ActionHandler>,
}

impl RunStatusBar {
    /// Create a bar in one state.
    pub fn new(id: impl Into<ElementId>, status: RunStatus) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            status,
            message: None,
            detail: None,
            action_label: None,
            action_busy: false,
            on_action: None,
        }
    }

    /// Say it in other words than the state's own.
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Add a quieter second part, such as the reason a run failed.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Name the action in other words than the state's own. Giving a label
    /// also gives an action to a state that has none of its own.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    /// Show the action as working, and stop it being pressed again.
    pub fn action_busy(mut self, busy: bool) -> Self {
        self.action_busy = busy;
        self
    }

    /// Hear when the action is pressed. Without this the bar shows no action.
    pub fn on_action(
        mut self,
        on_action: impl Fn(&RunStatus, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(on_action));
        self
    }
}

impl Styled for RunStatusBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RunStatusBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let radius = theme.radius;
        let status = self.status;
        let tone = status.tone();
        let mark = tone.color(cx);
        let tint = tone.tint(cx);

        let message: SharedString = self
            .message
            .unwrap_or_else(|| status.default_message().into());
        let words = spoken(&message, self.detail.as_deref());
        let action_label: Option<SharedString> = self
            .action_label
            .or_else(|| status.default_action().map(Into::into));
        let action = self.on_action.zip(action_label).map(|(on_action, label)| {
            Button::new(ElementId::NamedChild(
                self.id.clone().into(),
                "action".into(),
            ))
            .small()
            .outline()
            .label(label)
            .loading(self.action_busy)
            .on_click(move |_, window, cx| on_action(&status, window, cx))
        });

        h_flex()
            .id(self.id.clone())
            .w_full()
            .min_h(px(36.))
            .gap_2()
            .px_3()
            .py_1p5()
            .rounded(radius)
            .border_1()
            .border_color(line)
            .bg(tint)
            .text_color(ink)
            .text_sm()
            .role(Role::Status)
            .aria_label(words)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size_4()
                    .text_color(mark)
                    .map(|this| match status.icon() {
                        Some(icon) => this.child(Icon::new(icon).small()),
                        None => this.child(Spinner::new().xsmall()),
                    }),
            )
            .child(
                // The words wrap in a narrow column; the action keeps its place.
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().font_medium().child(message))
                    .when_some(self.detail, |this, detail| {
                        this.child(div().text_xs().text_color(quiet).child(detail))
                    }),
            )
            .children(action.map(|action| div().flex_none().child(action)))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_recoverable_states_have_an_action() {
        let with_action: Vec<_> = RunStatus::ALL
            .into_iter()
            .filter(|status| status.default_action().is_some())
            .collect();
        assert_eq!(with_action, [RunStatus::CutShort, RunStatus::Failed]);
        assert_eq!(RunStatus::CutShort.default_action(), Some("Continue"));
        assert_eq!(RunStatus::Failed.default_action(), Some("Try again"));
    }

    #[test]
    fn each_state_has_its_own_words() {
        let mut words: Vec<_> = RunStatus::ALL
            .into_iter()
            .map(RunStatus::default_message)
            .collect();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), RunStatus::ALL.len());
    }

    #[test]
    fn the_tone_follows_how_bad_it_is() {
        assert_eq!(RunStatus::Stopped.tone(), Tone::Neutral);
        assert_eq!(RunStatus::CutShort.tone(), Tone::Warning);
        assert_eq!(RunStatus::Failed.tone(), Tone::Danger);
        assert_eq!(RunStatus::Reconnecting.tone(), Tone::Info);
        assert_eq!(RunStatus::Closed.tone(), Tone::Neutral);
    }

    #[test]
    fn the_detail_is_spoken_after_the_message() {
        assert_eq!(
            spoken("Something went wrong.", None),
            "Something went wrong."
        );
        assert_eq!(
            spoken("Something went wrong.", Some("")),
            "Something went wrong."
        );
        assert_eq!(
            spoken(
                "Something went wrong.",
                Some("The model ran out of memory.")
            ),
            "Something went wrong. The model ran out of memory."
        );
    }
}
