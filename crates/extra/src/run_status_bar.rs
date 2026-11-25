// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Run status bar: one bar above the composer that says what the run is doing or how it ended, with the things to do about it.
//!
//! There are seven states. A run that is working, a run the person stopped,
//! a run that was cut short and can be continued, a run that failed, a
//! connection that is being restored, a chat that is closed, and a run that
//! finished. The bar owns no state: its owner picks the state, may change the
//! words, passes the buttons to show and is told when one is pressed. A
//! screen reader hears the bar as a status, and each button is an ordinary
//! button.
//!
//! Every state has the same quiet fill and edge except a failure, which is
//! pale red with red words.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::Tone;

/// What the bar is saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunStatus {
    /// The run is working. Shown with a spinner and, when the owner gives
    /// it, the time it has taken so far.
    Running,
    /// The person stopped the run.
    Stopped,
    /// The run ended before it was finished, such as at a length limit. It
    /// can be continued.
    CutShort,
    /// The run failed. It can be tried again. This is the one state drawn
    /// in red.
    Failed,
    /// The connection dropped and is being restored. Nothing to do but wait.
    Reconnecting,
    /// The chat is closed and takes no more messages.
    Closed,
    /// The run finished.
    Done,
}

impl RunStatus {
    /// Every state, in the order above.
    pub const ALL: [RunStatus; 7] = [
        RunStatus::Running,
        RunStatus::Stopped,
        RunStatus::CutShort,
        RunStatus::Failed,
        RunStatus::Reconnecting,
        RunStatus::Closed,
        RunStatus::Done,
    ];

    /// The words shown when the owner gives none.
    pub fn default_message(self) -> &'static str {
        match self {
            Self::Running => "Working",
            Self::Stopped => "You stopped this response",
            Self::CutShort => "The answer was cut short.",
            Self::Failed => "Something went wrong.",
            Self::Reconnecting => "Reconnecting\u{2026}",
            Self::Closed => "This chat is closed.",
            Self::Done => "Done",
        }
    }

    /// The label of the one button [`RunStatusBar::on_action`] adds, for the
    /// states that have one.
    pub fn default_action(self) -> Option<&'static str> {
        match self {
            Self::CutShort => Some("Continue"),
            Self::Failed => Some("Try again"),
            Self::Running | Self::Stopped | Self::Reconnecting | Self::Closed | Self::Done => None,
        }
    }

    /// The meaning the state carries. Only a failure is coloured by it; the
    /// others share one quiet look.
    pub fn tone(self) -> Tone {
        match self {
            Self::Stopped | Self::Closed => Tone::Neutral,
            Self::CutShort => Tone::Warning,
            Self::Failed => Tone::Danger,
            Self::Running | Self::Reconnecting => Tone::Info,
            Self::Done => Tone::Success,
        }
    }

    /// The state's icon. The two states that are waiting on something show a
    /// spinner instead.
    /// A stopped run shows a filled stop mark, so it never reads as an
    /// empty checkbox.
    fn icon(self) -> Option<CatalogIcon> {
        match self {
            Self::Running | Self::Reconnecting => None,
            Self::Stopped => Some(CatalogIcon::StopFilledAlt),
            Self::CutShort => Some(IconName::TriangleAlert.into()),
            Self::Failed => Some(IconName::CircleAlert.into()),
            Self::Closed => Some(IconName::Info.into()),
            Self::Done => Some(IconName::CircleCheck.into()),
        }
    }
}

/// How long a run has taken, as minutes and seconds: "0:14", "12:05".
pub fn elapsed_words(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// What a screen reader hears for the bar, such as "Something went wrong.
/// The model ran out of memory."
fn spoken(message: &str, detail: Option<&str>) -> String {
    match detail {
        Some(detail) if !detail.is_empty() => format!("{message} {detail}"),
        _ => message.to_string(),
    }
}

/// How a button in the bar looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RunStatusButtonStyle {
    /// The surface colour with a grey edge. For most buttons.
    #[default]
    Secondary,
    /// The ink colour. For the one button that carries on, such as Resume.
    Primary,
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&RunStatus, &mut Window, &mut App)>;

/// One button in the bar: Steer, Stop, Dismiss, Retry, Resume and the like.
#[derive(Clone)]
pub struct RunStatusButton {
    label: SharedString,
    style: RunStatusButtonStyle,
    busy: bool,
    on_click: Option<ClickHandler>,
}

impl RunStatusButton {
    /// Create a secondary button with a label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            style: RunStatusButtonStyle::Secondary,
            busy: false,
            on_click: None,
        }
    }

    /// Make it the primary button.
    pub fn primary(mut self) -> Self {
        self.style = RunStatusButtonStyle::Primary;
        self
    }

    /// Set how it looks.
    pub fn style(mut self, style: RunStatusButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Show it as working, and stop it being pressed again.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Hear when it is pressed.
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }
}

/// The bar above the composer that says what the run is doing or how it ended.
#[derive(IntoElement)]
pub struct RunStatusBar {
    id: ElementId,
    style: StyleRefinement,
    status: RunStatus,
    message: Option<SharedString>,
    detail: Option<SharedString>,
    elapsed: Option<Duration>,
    buttons: Vec<RunStatusButton>,
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
            elapsed: None,
            buttons: Vec::new(),
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

    /// Add a quieter second line, such as the reason a run failed. There is
    /// none unless this is called.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Say how long the run has taken. It follows the message after a dot:
    /// "Working · 0:14". The owner keeps the clock and passes the time on
    /// each draw.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    /// Add a button. Buttons show in the order they are added, at the end of
    /// the bar.
    pub fn button(mut self, button: RunStatusButton) -> Self {
        self.buttons.push(button);
        self
    }

    /// Add several buttons.
    pub fn buttons(mut self, buttons: impl IntoIterator<Item = RunStatusButton>) -> Self {
        self.buttons.extend(buttons);
        self
    }

    /// Name the one button [`Self::on_action`] adds in other words than the
    /// state's own. Giving a label also gives that button to a state that
    /// has none of its own.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    /// Show the [`Self::on_action`] button as working, and stop it being
    /// pressed again.
    pub fn action_busy(mut self, busy: bool) -> Self {
        self.action_busy = busy;
        self
    }

    /// A shorthand for one button: Continue for a run that was cut short,
    /// Try again for one that failed. It goes after any buttons added with
    /// [`Self::button`]. For more than one button, or other labels, pass
    /// buttons instead.
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
        let status = self.status;
        let failed = status == RunStatus::Failed;
        // One quiet look for every state; a failure is pale red with red words.
        let (fill, edge, words_color) = if failed {
            (
                theme.danger.opacity(0.08),
                theme.danger.opacity(0.22),
                theme.danger,
            )
        } else {
            (theme.muted, theme.border, theme.foreground.opacity(0.85))
        };
        let quiet = theme.muted_foreground;

        let message: SharedString = self
            .message
            .unwrap_or_else(|| status.default_message().into());
        let message: SharedString = match self.elapsed {
            Some(elapsed) => format!("{message} \u{b7} {}", elapsed_words(elapsed)).into(),
            None => message,
        };
        let words = spoken(&message, self.detail.as_deref());

        // The shorthand button goes after the ones the owner passed, under
        // the id it has always had.
        let mut buttons: Vec<(SharedString, RunStatusButton)> = self
            .buttons
            .into_iter()
            .enumerate()
            .map(|(ix, button)| (format!("button-{ix}").into(), button))
            .collect();
        let action_label: Option<SharedString> = self
            .action_label
            .or_else(|| status.default_action().map(Into::into));
        if let Some((on_action, label)) = self.on_action.zip(action_label) {
            buttons.push((
                "action".into(),
                RunStatusButton::new(label)
                    .busy(self.action_busy)
                    .on_click(move |_, window, cx| on_action(&status, window, cx)),
            ));
        }
        let id = self.id.clone();
        let buttons = buttons.into_iter().map(move |(name, button)| {
            let on_click = button.on_click;
            Button::new(ElementId::NamedChild(id.clone().into(), name))
                .map(|this| match button.style {
                    RunStatusButtonStyle::Primary => this.primary(),
                    RunStatusButtonStyle::Secondary => this.outline(),
                })
                .xsmall()
                .label(button.label)
                .loading(button.busy)
                .h(px(30.))
                .px_2p5()
                .rounded(px(4.))
                .text_label_small()
                .when_some(on_click, |this, on_click| {
                    this.on_click(move |event, window, cx| on_click(event, window, cx))
                })
        });

        h_flex()
            .id(self.id.clone())
            .w_full()
            .min_h(px(44.))
            // In a narrow column the buttons drop to a line of their own
            // rather than squeezing the words.
            .flex_wrap()
            .gap_2()
            .pl_3()
            .pr_1p5()
            .py_1p5()
            .rounded(px(10.))
            .border_1()
            .border_color(edge)
            .bg(fill)
            .text_color(words_color)
            .text_body()
            .role(Role::Status)
            .aria_label(words)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size_4()
                    .map(|this| match status.icon() {
                        Some(icon) => this.child(Icon::new(icon).small()),
                        None => this.child(Spinner::new().xsmall()),
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(140.))
                    .child(div().font_medium().child(message))
                    .when_some(self.detail, |this, detail| {
                        this.child(div().text_caption().text_color(quiet).child(detail))
                    }),
            )
            .child(h_flex().flex_none().ml_auto().gap_1p5().children(buttons))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_recoverable_states_have_a_shorthand_action() {
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
        assert_eq!(
            RunStatus::Stopped.default_message(),
            "You stopped this response"
        );
    }

    #[test]
    fn the_tone_follows_what_the_state_means() {
        assert_eq!(RunStatus::Running.tone(), Tone::Info);
        assert_eq!(RunStatus::Stopped.tone(), Tone::Neutral);
        assert_eq!(RunStatus::CutShort.tone(), Tone::Warning);
        assert_eq!(RunStatus::Failed.tone(), Tone::Danger);
        assert_eq!(RunStatus::Reconnecting.tone(), Tone::Info);
        assert_eq!(RunStatus::Closed.tone(), Tone::Neutral);
        assert_eq!(RunStatus::Done.tone(), Tone::Success);
    }

    #[test]
    fn a_stopped_run_shows_a_filled_stop_not_an_empty_square() {
        assert_eq!(RunStatus::Stopped.icon(), Some(CatalogIcon::StopFilledAlt));
    }

    #[test]
    fn the_waiting_states_spin_and_the_rest_have_an_icon() {
        for status in RunStatus::ALL {
            let spins = matches!(status, RunStatus::Running | RunStatus::Reconnecting);
            assert_eq!(status.icon().is_none(), spins, "{status:?}");
        }
    }

    #[test]
    fn elapsed_time_reads_as_minutes_and_seconds() {
        assert_eq!(elapsed_words(Duration::from_secs(0)), "0:00");
        assert_eq!(elapsed_words(Duration::from_secs(14)), "0:14");
        assert_eq!(elapsed_words(Duration::from_secs(725)), "12:05");
        assert_eq!(elapsed_words(Duration::from_millis(59_900)), "0:59");
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

    #[test]
    fn a_button_is_secondary_until_made_primary() {
        let button = RunStatusButton::new("Resume");
        assert_eq!(button.style, RunStatusButtonStyle::Secondary);
        assert_eq!(button.primary().style, RunStatusButtonStyle::Primary);
    }
}
