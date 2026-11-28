// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Permission prompt: an assistant asks for one specific right.
//!
//! The prompt says who asks, for what, why, and exactly what will happen. The
//! right can be given just once, for this chat, for a while, or always. Once
//! answered the prompt folds into one line with the time left and a way to
//! revoke it.
//!
//! A preview shows long text whole, such as a full command, and can hold a
//! diff. For risky and destructive prompts the allow key is deliberate: only
//! the modified Enter allows, a plain Enter does nothing, and Escape denies.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex,
    kbd::{Kbd, spoken_keystroke},
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    Keystroke, ParentElement as _, Pixels, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Task, TestSupportExt as _, Window,
    accesskit::Toggled, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How much harm the action can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionRisk {
    /// Easy to take back.
    Safe,
    /// Changes something others will see.
    Risky,
    /// Cannot be taken back. It can never be allowed for good.
    Destructive,
}

/// How long a right is given for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionScope {
    /// This one time.
    Once,
    /// Until this chat ends. The owner ends it with the chat.
    Chat,
    /// For a set time.
    Timed,
    /// Until it is revoked.
    Always,
}

/// The answer to a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionDecision {
    /// Whether the right was given.
    pub allow: bool,
    /// For how long.
    pub scope: PermissionScope,
    /// When the answer was given.
    pub at: Instant,
    /// When a timed right runs out.
    pub until: Option<Instant>,
}

impl PermissionDecision {
    /// Whether the right holds at `now`. Check this where the assistant acts;
    /// a right given once is for the owner to use up.
    pub fn permits(&self, now: Instant) -> bool {
        self.allow && self.until.is_none_or(|until| now < until)
    }
}

/// One thing that will happen: a file, a recipient, an amount.
#[derive(Clone)]
pub struct PermissionItem {
    label: SharedString,
    meta: Option<SharedString>,
    icon: IconName,
}

impl PermissionItem {
    /// Create an item.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            meta: None,
            icon: IconName::File,
        }
    }

    /// A short value on the right, such as an amount.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Set the icon in front of the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }
}

/// Long text shown whole, such as a full command or a file's new content.
///
/// The text is set in the mono face and wrapped, never cut with an ellipsis.
/// Past its height the box scrolls, by wheel or by the arrow keys once it has
/// focus. A diff can sit under the text, or stand alone.
pub struct PermissionPreview {
    title: SharedString,
    text: SharedString,
    diff: Option<AnyElement>,
    max_height: Pixels,
}

impl PermissionPreview {
    /// Create a preview of `text`. Pass empty text for a diff alone.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            title: "Preview".into(),
            text: text.into(),
            diff: None,
            max_height: px(240.),
        }
    }

    /// Set the heading over the preview, such as "Command". "Preview" by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Show a diff under the text, such as an inline diff review.
    pub fn diff(mut self, diff: impl IntoElement) -> Self {
        self.diff = Some(diff.into_any_element());
        self
    }

    /// How tall the box grows before it scrolls. 240 pixels by default.
    pub fn max_height(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = height.into();
        self
    }
}

type DecisionHandler = Rc<dyn Fn(Option<&PermissionDecision>, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type Decide = Rc<dyn Fn(Option<PermissionDecision>, &mut Window, &mut App)>;

/// A request for one right, with a choice of how long to give it.
#[derive(IntoElement)]
pub struct PermissionPrompt {
    id: ElementId,
    style: StyleRefinement,
    who: SharedString,
    action: SharedString,
    target: Option<SharedString>,
    target_icon: Option<IconName>,
    risk: PermissionRisk,
    reason: Option<SharedString>,
    items: Vec<PermissionItem>,
    preview_title: SharedString,
    preview_limit: usize,
    preview: Option<PermissionPreview>,
    scopes: Vec<PermissionScope>,
    default_scope: PermissionScope,
    duration: Duration,
    deliberate: Option<bool>,
    decision: Option<Option<PermissionDecision>>,
    on_decision_change: Option<DecisionHandler>,
}

impl PermissionPrompt {
    /// Create a prompt: `who` wants to `action`, as in "push to main".
    pub fn new(
        id: impl Into<ElementId>,
        who: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            who: who.into(),
            action: action.into(),
            target: None,
            target_icon: None,
            risk: PermissionRisk::Risky,
            reason: None,
            items: Vec::new(),
            preview_title: "What it will do".into(),
            preview_limit: 3,
            preview: None,
            scopes: vec![
                PermissionScope::Once,
                PermissionScope::Chat,
                PermissionScope::Timed,
                PermissionScope::Always,
            ],
            default_scope: PermissionScope::Once,
            duration: Duration::from_secs(60 * 60),
            deliberate: None,
            decision: None,
            on_decision_change: None,
        }
    }

    /// Where the action happens, such as a repository or an account.
    pub fn target(mut self, target: impl Into<SharedString>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// An icon for the tool, next to the target.
    pub fn target_icon(mut self, icon: IconName) -> Self {
        self.target_icon = Some(icon);
        self
    }

    /// Set the risk. Risky by default.
    pub fn risk(mut self, risk: PermissionRisk) -> Self {
        self.risk = risk;
        self
    }

    /// Why the right is needed, in the assistant's own words.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Add one thing that will happen. Keep these concrete.
    pub fn item(mut self, item: PermissionItem) -> Self {
        self.items.push(item);
        self
    }

    /// Set the heading over the things that will happen.
    pub fn preview_title(mut self, title: impl Into<SharedString>) -> Self {
        self.preview_title = title.into();
        self
    }

    /// Show this many things before "N more". Three by default.
    pub fn preview_limit(mut self, limit: usize) -> Self {
        self.preview_limit = limit;
        self
    }

    /// Show long text whole, such as the full command, above the things that
    /// will happen.
    pub fn preview(mut self, preview: PermissionPreview) -> Self {
        self.preview = Some(preview);
        self
    }

    /// Require the modified Enter (Command or Control) to allow. A plain
    /// Enter then does nothing, the Allow button ignores the keyboard, and
    /// Escape still denies. On by default for risky and destructive prompts.
    pub fn deliberate_allow(mut self, deliberate: bool) -> Self {
        self.deliberate = Some(deliberate);
        self
    }

    /// Set the choices of how long. With one choice the switch is hidden.
    pub fn scopes(mut self, scopes: impl IntoIterator<Item = PermissionScope>) -> Self {
        self.scopes = scopes.into_iter().collect();
        self
    }

    /// Set the choice picked at first.
    pub fn default_scope(mut self, scope: PermissionScope) -> Self {
        self.default_scope = scope;
        self
    }

    /// How long a timed right holds. One hour by default.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Control the answer: `None` while the prompt waits. Pair it with
    /// `on_decision_change`.
    pub fn decision(mut self, decision: Option<PermissionDecision>) -> Self {
        self.decision = Some(decision);
        self
    }

    /// Run with the answer on Allow and Deny, and with `None` on Revoke,
    /// Change and Ask again: treat that as not allowed, ask again.
    pub fn on_decision_change(
        mut self,
        handler: impl Fn(Option<&PermissionDecision>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decision_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for PermissionPrompt {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The choices on offer. A destructive action is never offered for good.
fn offered_scopes(scopes: &[PermissionScope], risk: PermissionRisk) -> Vec<PermissionScope> {
    scopes
        .iter()
        .copied()
        .filter(|scope| risk != PermissionRisk::Destructive || *scope != PermissionScope::Always)
        .collect()
}

/// A length of time in words: "45 minutes", "1 hour", "2 hours".
fn duration_words(duration: Duration) -> String {
    let minutes = duration.as_secs().div_ceil(60).max(1);
    match (minutes / 60, minutes % 60) {
        (0, 1) => "1 minute".into(),
        (0, minutes) => format!("{minutes} minutes"),
        (1, 0) => "1 hour".into(),
        (hours, 0) => format!("{hours} hours"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// The time a right has left, in a few characters.
fn time_left(left: Duration) -> String {
    let seconds = left.as_secs();
    if seconds < 60 {
        format!("{seconds} s left")
    } else if seconds < 3600 {
        format!("{} min left", seconds / 60)
    } else {
        format!("{} h {} min left", seconds / 3600, seconds % 3600 / 60)
    }
}

/// The choice an arrow key moves to. It stops at the ends.
fn step_scope(
    scopes: &[PermissionScope],
    current: PermissionScope,
    key: &str,
) -> Option<PermissionScope> {
    let ix = scopes.iter().position(|scope| *scope == current)?;
    let next = match key {
        "right" => (ix + 1).min(scopes.len() - 1),
        "left" => ix.saturating_sub(1),
        _ => return None,
    };
    Some(scopes[next])
}

/// Whether only the modified Enter allows: on for risky and destructive
/// prompts unless the owner says otherwise.
fn is_deliberate(setting: Option<bool>, risk: PermissionRisk) -> bool {
    setting.unwrap_or(risk != PermissionRisk::Safe)
}

/// What a key pressed inside the prompt does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyOutcome {
    /// Give the right.
    Allow,
    /// Refuse it.
    Deny,
    /// Pick another choice of how long.
    Pick(PermissionScope),
    /// Do nothing, and let nothing around the prompt act on it either.
    Swallow,
    /// Not the prompt's key: let it go on.
    Pass,
}

/// A key pressed somewhere inside the prompt.
#[derive(Debug, Clone, Copy)]
struct PromptKey<'a> {
    key: &'a str,
    /// Any modifier is held.
    modified: bool,
    /// The platform's main modifier is held: Command on a Mac, Control
    /// elsewhere.
    secondary: bool,
    /// The prompt itself has focus, not a button inside it.
    on_prompt: bool,
}

/// What a key does. The modified Enter always allows. A plain Enter allows
/// only on the prompt itself, and never when the allow key is deliberate.
fn key_outcome(
    press: PromptKey<'_>,
    deliberate: bool,
    scopes: &[PermissionScope],
    current: PermissionScope,
) -> KeyOutcome {
    if press.key == "enter" && press.secondary {
        return KeyOutcome::Allow;
    }
    if press.modified {
        return KeyOutcome::Pass;
    }
    match press.key {
        // A focused button answers Enter itself: Deny must deny.
        "enter" if !press.on_prompt => KeyOutcome::Pass,
        "enter" if deliberate => KeyOutcome::Swallow,
        "enter" => KeyOutcome::Allow,
        "escape" => KeyOutcome::Deny,
        key => step_scope(scopes, current, key).map_or(KeyOutcome::Pass, KeyOutcome::Pick),
    }
}

/// How far a key scrolls the preview, in pixels down, given its height.
fn preview_scroll(key: &str, height: Pixels) -> Option<Pixels> {
    let line = px(36.);
    let page = (height - line).max(line);
    match key {
        "down" => Some(line),
        "up" => Some(-line),
        "pagedown" | "space" => Some(page),
        "pageup" => Some(-page),
        _ => None,
    }
}

/// A count with commas between thousands: "2,000".
fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (ix, ch) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Two capitals that stand for whoever asks.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().find(|ch| ch.is_alphanumeric()))
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

struct PromptState {
    scope: PermissionScope,
    decision: Option<PermissionDecision>,
    show_all: bool,
    /// Redraws once a second while a timed right runs.
    ticker: Option<Task<()>>,
    /// The prompt's own focus. Enter only allows while the prompt itself has
    /// it, never while a button inside it does.
    focus: FocusHandle,
    /// The preview's focus, so the keyboard can scroll it.
    preview_focus: FocusHandle,
    preview_scroll: ScrollHandle,
}

impl RenderOnce for PermissionPrompt {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let warning = theme.warning;
        let danger = theme.danger;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let body_font = theme.font_family.clone();

        let scopes = offered_scopes(&self.scopes, self.risk);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| PromptState {
            scope: self.default_scope,
            decision: None,
            show_all: false,
            ticker: None,
            focus: cx.focus_handle().tab_stop(true),
            preview_focus: cx.focus_handle().tab_stop(true),
            preview_scroll: ScrollHandle::new(),
        });
        let focus = state.read(cx).focus.clone();
        let deliberate = is_deliberate(self.deliberate, self.risk);
        let controlled = self.decision.is_some();
        let now = Instant::now();
        let (scope, decision, show_all) = state.update(cx, |state, cx| {
            if let Some(decision) = self.decision {
                state.decision = decision;
            }
            // The picked choice has to be one that is on offer.
            if !scopes.contains(&state.scope) {
                state.scope = scopes.first().copied().unwrap_or(PermissionScope::Once);
            }
            let running = state.decision.is_some_and(|decision| {
                decision.allow && decision.until.is_some_and(|until| now < until)
            });
            if !running {
                state.ticker = None;
            } else if state.ticker.is_none() {
                state.ticker = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
            }
            (state.scope, state.decision, state.show_all)
        });

        let decide: Decide = {
            let state = state.clone();
            let on_decision_change = self.on_decision_change.clone();
            Rc::new(move |decision, window, cx| {
                if !controlled {
                    state.update(cx, |state, cx| {
                        state.decision = decision;
                        cx.notify();
                    });
                }
                if let Some(handler) = &on_decision_change {
                    handler(decision.as_ref(), window, cx);
                }
            })
        };
        let duration = self.duration;
        let answer = move |allow: bool| {
            let at = Instant::now();
            PermissionDecision {
                allow,
                scope,
                at,
                until: (allow && scope == PermissionScope::Timed).then(|| at + duration),
            }
        };
        let allow: Handler = {
            let decide = decide.clone();
            Rc::new(move |window, cx| decide(Some(answer(true)), window, cx))
        };
        let deny: Handler = {
            let decide = decide.clone();
            Rc::new(move |window, cx| decide(Some(answer(false)), window, cx))
        };
        let ask_again = {
            let decide = decide.clone();
            move |_: &isonomic_ui::ClickEvent, window: &mut Window, cx: &mut App| {
                decide(None, window, cx)
            }
        };

        let card = v_flex()
            .id(self.id.clone())
            .test_support()
            .w_full()
            .max_w(px(400.))
            .overflow_hidden()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx));

        // Answered: one line, with what is left of a timed right.
        if let Some(decision) = decision {
            let left = decision
                .until
                .map(|until| until.saturating_duration_since(now));
            let ran_out = left.is_some_and(|left| left.is_zero());
            let (title, note, button) = match (decision.allow, decision.scope) {
                (false, _) => (
                    "Denied".to_string(),
                    format!("{} has been told no", self.who),
                    "Change",
                ),
                (true, _) if ran_out => (
                    "Ran out".to_string(),
                    "It has to be asked for again".to_string(),
                    "Ask again",
                ),
                (true, PermissionScope::Once) => (
                    "Allowed this once".to_string(),
                    "For this one time only".to_string(),
                    "Change",
                ),
                (true, PermissionScope::Chat) => (
                    "Allowed for this chat".to_string(),
                    "Until this chat ends".to_string(),
                    "Revoke",
                ),
                (true, PermissionScope::Timed) => (
                    format!("Allowed for {}", duration_words(duration)),
                    left.map(time_left).unwrap_or_default(),
                    "Revoke",
                ),
                (true, PermissionScope::Always) => (
                    "Allowed for good".to_string(),
                    "Until you revoke it".to_string(),
                    "Revoke",
                ),
            };
            let holds = decision.allow && !ran_out;
            let share = left
                .filter(|_| holds && !duration.is_zero())
                .map(|left| (left.as_secs_f32() / duration.as_secs_f32()).clamp(0., 1.));

            return card
                .child(
                    h_flex()
                        .gap_3()
                        .px_3()
                        .py_2p5()
                        .child(
                            Icon::new(if holds {
                                IconName::CircleCheck
                            } else {
                                IconName::CircleX
                            })
                            .small()
                            .flex_none()
                            .text_color(if holds {
                                success
                            } else {
                                danger
                            }),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(div().flex_none().font_medium().child(title))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .text_color(quiet)
                                                .child(format!("· {}", self.action)),
                                        ),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(note),
                                ),
                        )
                        .child(
                            Button::new((self.id.clone(), "again"))
                                .ghost()
                                .small()
                                .label(button)
                                .on_click(ask_again),
                        ),
                )
                .children(share.map(|share| {
                    div()
                        .h(px(2.))
                        .w_full()
                        .bg(chip)
                        .child(div().h_full().w(relative(share)).bg(success))
                }))
                .refine_style(&self.style);
        }

        let tag = match self.risk {
            PermissionRisk::Safe => None,
            PermissionRisk::Risky => Some(("Risky", warning, IconName::TriangleAlert)),
            PermissionRisk::Destructive => Some(("Destructive", danger, IconName::CircleAlert)),
        };
        let allow_label = match scope {
            PermissionScope::Once => "Allow this once".to_string(),
            PermissionScope::Chat => "Allow for this chat".to_string(),
            PermissionScope::Timed => format!("Allow for {}", duration_words(duration)),
            PermissionScope::Always => "Allow for good".to_string(),
        };
        let scope_label = |scope: PermissionScope| match scope {
            PermissionScope::Once => "This once".to_string(),
            PermissionScope::Chat => "This chat".to_string(),
            PermissionScope::Timed => duration_words(duration),
            PermissionScope::Always => "For good".to_string(),
        };
        // The key that allows, drawn on the button and spoken to a screen reader.
        let allow_keys = if deliberate {
            "secondary-enter"
        } else {
            "enter"
        };
        // `secondary` names Command on a Mac and Control elsewhere, so it can
        // only be read with the platform in hand.
        let os = cx.platform_info().os;
        let spoken_allow = Keystroke::parse_with_os(allow_keys, os)
            .map(|stroke| spoken_keystroke(&stroke, cfg!(target_os = "macos")))
            .unwrap_or_default();
        let keys_note = format!(
            "{}{spoken_allow} allows, Escape denies",
            if scopes.len() > 1 {
                "Left and right arrows choose how long. "
            } else {
                ""
            }
        );
        let pick = |to: PermissionScope| {
            let state = state.clone();
            move |_: &isonomic_ui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.scope = to;
                    cx.notify();
                })
            }
        };
        let hint = |keys: &str| Keystroke::parse_with_os(keys, os).ok().map(Kbd::new);
        let shown = if show_all {
            self.items.len()
        } else {
            self.preview_limit.min(self.items.len())
        };
        let hidden = self.items.len() - shown;

        // Clicking the text of the prompt (not a button) puts focus on it.
        let focus_here = {
            let focus = focus.clone();
            move |_: &isonomic_ui::MouseDownEvent, window: &mut Window, cx: &mut App| {
                window.focus(&focus, cx)
            }
        };
        // The prompt is a named group a keyboard can land on. It says what is
        // picked and which keys answer, so a screen reader hears both.
        let card = card
            .track_focus(&focus)
            .role(Role::Group)
            .aria_label(format!("{} wants to {}", self.who, self.action))
            .aria_description(format!("{allow_label}. {keys_note}"));
        card.on_key_down({
            let allow = allow.clone();
            let deny = deny.clone();
            let state = state.clone();
            let scopes = scopes.clone();
            move |event, window, cx| {
                let stroke = &event.keystroke;
                let press = PromptKey {
                    key: stroke.key.as_str(),
                    modified: stroke.modifiers.modified(),
                    secondary: stroke.modifiers.secondary(cx),
                    on_prompt: focus.is_focused(window),
                };
                match key_outcome(press, deliberate, &scopes, scope) {
                    KeyOutcome::Allow => allow(window, cx),
                    KeyOutcome::Deny => deny(window, cx),
                    KeyOutcome::Pick(next) => state.update(cx, |state, cx| {
                        state.scope = next;
                        cx.notify();
                    }),
                    KeyOutcome::Swallow => {}
                    KeyOutcome::Pass => return,
                }
                cx.stop_propagation();
            }
        })
        .child(
            v_flex()
                .on_mouse_down(isonomic_ui::MouseButton::Left, focus_here.clone())
                .gap_3()
                .p_4()
                .child(
                    h_flex()
                        .items_start()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size_8()
                                .rounded(radius)
                                .border_1()
                                .border_color(line)
                                .text_label_small()
                                .text_color(quiet)
                                .child(initials(&self.who)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap_1()
                                        .child(div().font_medium().child(self.who))
                                        .child(div().text_color(quiet).child("wants to"))
                                        .child(div().font_medium().child(self.action)),
                                )
                                .children(self.target.map(|target| {
                                    h_flex()
                                        .gap_1p5()
                                        .text_color(quiet)
                                        .children(
                                            self.target_icon.map(|icon| Icon::new(icon).xsmall()),
                                        )
                                        .child(div().truncate().child(target))
                                })),
                        )
                        .children(tag.map(|(word, color, icon)| {
                            h_flex()
                                .flex_none()
                                .gap_1()
                                .px_1p5()
                                .rounded(px(5.))
                                .bg(color.opacity(0.16))
                                .text_label_small()
                                .child(Icon::new(icon).size_3().text_color(color))
                                .child(word)
                        })),
                )
                .children(self.reason.map(|reason| {
                    div()
                        .ml(px(44.))
                        .pl_3()
                        .border_l_2()
                        .border_color(line)
                        .text_color(quiet)
                        .child(reason)
                })),
        )
        .children(self.preview.map(|preview| {
            let (preview_focus, scroll) = {
                let state = state.read(cx);
                (state.preview_focus.clone(), state.preview_scroll.clone())
            };
            let chars = preview.text.chars().count();
            let size_note = match chars {
                0 => String::new(),
                1 => "1 character. ".to_string(),
                n => format!("{} characters. ", group_thousands(n)),
            };
            let scroll_keys = scroll.clone();
            v_flex()
                .gap_1p5()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
                .child(div().text_color(quiet).child(preview.title.clone()))
                .child(
                    div()
                        .id((self.id.clone(), "preview"))
                        .test_support()
                        .track_focus(&preview_focus)
                        .role(Role::Document)
                        .aria_label(preview.title)
                        .aria_description(format!("{size_note}Up and down arrows scroll."))
                        .on_key_down(move |event, _, cx| {
                            if event.keystroke.modifiers.modified() {
                                return;
                            }
                            let height = scroll_keys.bounds().size.height;
                            let Some(down) = preview_scroll(&event.keystroke.key, height) else {
                                return;
                            };
                            let offset = scroll_keys.offset();
                            let most = scroll_keys.max_offset().y;
                            let y = (offset.y - down).clamp(-most, px(0.));
                            scroll_keys.set_offset(point(offset.x, y));
                            cx.stop_propagation();
                        })
                        .max_h(preview.max_height)
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .on_scroll(|_, window, _| window.refresh())
                        .p_2()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .bg(chip.opacity(0.5))
                        .when(preview_focus.is_focused(window), |this| {
                            this.border_color(cx.theme().ring)
                        })
                        .font_family(mono.clone())
                        .text_size(px(12.))
                        .line_height(px(18.))
                        // Wrapped and whole: no truncation, no ellipsis.
                        .whitespace_normal()
                        .when(chars > 0, |this| {
                            this.child(
                                div()
                                    .id("preview-text")
                                    .test_support()
                                    .w_full()
                                    .child(preview.text),
                            )
                        })
                        .children(preview.diff.map(|diff| {
                            div()
                                .when(chars > 0, |this| this.mt_2())
                                .font_family(body_font.clone())
                                .child(diff)
                        })),
                )
        }))
        .when(!self.items.is_empty(), |this| {
            this.child(
                v_flex()
                    .on_mouse_down(isonomic_ui::MouseButton::Left, focus_here)
                    .gap_1()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .text_color(quiet)
                            .child(self.preview_title)
                            .child(
                                div()
                                    .px_1()
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(line)
                                    .text_caption()
                                    .child(self.items.len().to_string()),
                            ),
                    )
                    .children(self.items.into_iter().take(shown).map(|item| {
                        h_flex()
                            .gap_2p5()
                            .h(px(30.))
                            .child(Icon::new(item.icon).small().flex_none().text_color(quiet))
                            .child(div().flex_1().min_w_0().truncate().child(item.label))
                            .children(
                                item.meta
                                    .map(|meta| div().flex_none().text_color(quiet).child(meta)),
                            )
                    }))
                    .when(hidden > 0, |this| {
                        this.child(
                            div().child(
                                Button::new((self.id.clone(), "more"))
                                    .ghost()
                                    .xsmall()
                                    .label(format!("{hidden} more"))
                                    .on_click({
                                        let state = state.clone();
                                        move |_, _, cx| {
                                            state.update(cx, |state, cx| {
                                                state.show_all = true;
                                                cx.notify();
                                            })
                                        }
                                    }),
                            ),
                        )
                    }),
            )
        })
        .child(
            v_flex()
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
                .when(scopes.len() > 1, |this| {
                    this.child(
                        h_flex()
                            .id((self.id.clone(), "scopes"))
                            .role(Role::RadioGroup)
                            .aria_label("How long")
                            .gap_1()
                            .p_0p5()
                            .rounded(radius)
                            .bg(chip)
                            .children(scopes.iter().enumerate().map(|(ix, option)| {
                                let picked = *option == scope;
                                div()
                                    .id((
                                        self.id.clone(),
                                        SharedString::from(format!("scope-{ix}")),
                                    ))
                                    .test_support()
                                    .role(Role::RadioButton)
                                    .aria_label(scope_label(*option))
                                    .aria_toggled(if picked {
                                        Toggled::True
                                    } else {
                                        Toggled::False
                                    })
                                    .aria_selected(picked)
                                    .flex()
                                    .flex_1()
                                    .items_center()
                                    .justify_center()
                                    .h(px(28.))
                                    .rounded(radius)
                                    .cursor_pointer()
                                    .map(|this| {
                                        if *option == scope {
                                            this.bg(fill).font_medium().shadow(resting_shadow(cx))
                                        } else {
                                            this.text_color(quiet)
                                        }
                                    })
                                    .on_click(pick(*option))
                                    .child(scope_label(*option))
                            })),
                    )
                })
                .when(self.risk == PermissionRisk::Destructive, |this| {
                    this.child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child("What cannot be undone is never allowed for good"),
                    )
                })
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new((self.id.clone(), "deny"))
                                .large()
                                .accessibility_label("Deny")
                                .child(h_flex().gap_1p5().child("Deny").children(hint("escape")))
                                .on_click(move |_, window, cx| deny(window, cx)),
                        )
                        .child({
                            let button = Button::new((self.id.clone(), "allow")).large();
                            // Allowing something that cannot be undone is the one
                            // place a danger action is solid.
                            let on_fill = if self.risk == PermissionRisk::Destructive {
                                cx.theme().danger_foreground
                            } else {
                                cx.theme().primary_foreground
                            };
                            let button = if self.risk == PermissionRisk::Destructive {
                                let danger = cx.theme().danger;
                                button.custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(danger)
                                        .solid(true)
                                        .foreground(cx.theme().danger_foreground)
                                        .hover(danger.opacity(0.9))
                                        .active(danger.opacity(0.82)),
                                )
                            } else {
                                button.primary()
                            };
                            button
                                .accessibility_label(allow_label.clone())
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(allow_label)
                                        // The hint takes the filled button's
                                        // own ink so it sits on the colour
                                        // instead of punching a grey hole in it.
                                        .children(hint(allow_keys).map(|kbd| {
                                            kbd.bg(on_fill.opacity(0.16))
                                                .text_color(on_fill.opacity(0.72))
                                        })),
                                )
                                .on_click(move |event, window, cx| {
                                    // With a deliberate allow key, Enter or
                                    // Space on a focused Allow button does
                                    // nothing: only a pointer or the modified
                                    // Enter allows.
                                    if deliberate && matches!(event, ClickEvent::Keyboard(_)) {
                                        return;
                                    }
                                    allow(window, cx)
                                })
                        }),
                ),
        )
        .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PermissionScope::{Always, Chat, Once, Timed};

    #[test]
    fn destructive_actions_are_never_offered_for_good() {
        let all = [Once, Timed, Always];
        assert_eq!(offered_scopes(&all, PermissionRisk::Risky), all);
        assert_eq!(
            offered_scopes(&all, PermissionRisk::Destructive),
            [Once, Timed]
        );
    }

    #[test]
    fn this_chat_is_offered_beside_the_others_and_for_good_never_when_destructive() {
        let all = [Once, Chat, Timed, Always];
        assert_eq!(offered_scopes(&all, PermissionRisk::Safe), all);
        assert_eq!(offered_scopes(&all, PermissionRisk::Risky), all);
        assert_eq!(
            offered_scopes(&all, PermissionRisk::Destructive),
            [Once, Chat, Timed]
        );
        assert_eq!(offered_scopes(&[Once], PermissionRisk::Destructive), [Once]);
        assert_eq!(
            offered_scopes(&[Always], PermissionRisk::Destructive),
            [] as [PermissionScope; 0]
        );
    }

    #[test]
    fn the_default_choices_hold_this_chat_and_never_for_good_when_destructive() {
        let prompt = PermissionPrompt::new("p", "assistant", "run a command");
        assert_eq!(prompt.scopes, [Once, Chat, Timed, Always]);
        let destructive = prompt.risk(PermissionRisk::Destructive);
        assert!(!offered_scopes(&destructive.scopes, destructive.risk).contains(&Always));
    }

    #[test]
    fn the_allow_key_is_deliberate_for_risky_and_destructive_prompts() {
        assert!(!is_deliberate(None, PermissionRisk::Safe));
        assert!(is_deliberate(None, PermissionRisk::Risky));
        assert!(is_deliberate(None, PermissionRisk::Destructive));
        assert!(is_deliberate(Some(true), PermissionRisk::Safe));
        assert!(!is_deliberate(Some(false), PermissionRisk::Destructive));
    }

    fn press(key: &str, modified: bool, secondary: bool, on_prompt: bool) -> PromptKey<'_> {
        PromptKey {
            key,
            modified,
            secondary,
            on_prompt,
        }
    }

    #[test]
    fn with_a_deliberate_key_plain_enter_does_nothing_and_escape_denies() {
        let scopes = [Once, Chat, Timed];
        let outcome = |p| key_outcome(p, true, &scopes, Once);
        assert_eq!(
            outcome(press("enter", false, false, true)),
            KeyOutcome::Swallow
        );
        assert_eq!(
            outcome(press("enter", false, false, false)),
            KeyOutcome::Pass
        );
        assert_eq!(
            outcome(press("escape", false, false, true)),
            KeyOutcome::Deny
        );
        assert_eq!(
            outcome(press("escape", false, false, false)),
            KeyOutcome::Deny
        );
        assert_eq!(outcome(press("enter", true, true, true)), KeyOutcome::Allow);
        assert_eq!(
            outcome(press("enter", true, true, false)),
            KeyOutcome::Allow
        );
        // Shift or Alt with Enter is not the allow key.
        assert_eq!(outcome(press("enter", true, false, true)), KeyOutcome::Pass);
        assert_eq!(
            outcome(press("right", false, false, true)),
            KeyOutcome::Pick(Chat)
        );
        assert_eq!(outcome(press("a", false, false, true)), KeyOutcome::Pass);
    }

    #[test]
    fn without_a_deliberate_key_plain_enter_allows_on_the_prompt_only() {
        let scopes = [Once, Chat, Timed, Always];
        let outcome = |p| key_outcome(p, false, &scopes, Chat);
        assert_eq!(
            outcome(press("enter", false, false, true)),
            KeyOutcome::Allow
        );
        assert_eq!(
            outcome(press("enter", false, false, false)),
            KeyOutcome::Pass
        );
        assert_eq!(
            outcome(press("enter", true, true, false)),
            KeyOutcome::Allow
        );
        assert_eq!(
            outcome(press("escape", false, false, true)),
            KeyOutcome::Deny
        );
        assert_eq!(
            outcome(press("escape", true, false, true)),
            KeyOutcome::Pass
        );
        assert_eq!(
            outcome(press("left", false, false, true)),
            KeyOutcome::Pick(Once)
        );
        assert_eq!(
            outcome(press("right", false, false, true)),
            KeyOutcome::Pick(Timed)
        );
    }

    #[test]
    fn no_plain_key_allows_a_deliberate_prompt() {
        let scopes = [Once, Chat, Timed];
        for key in [
            "enter", "space", "a", "y", "tab", "right", "left", "up", "down",
        ] {
            for on_prompt in [true, false] {
                assert_ne!(
                    key_outcome(press(key, false, false, on_prompt), true, &scopes, Once),
                    KeyOutcome::Allow,
                    "{key} allowed"
                );
            }
        }
    }

    #[test]
    fn preview_keys_scroll_by_a_line_or_a_page() {
        let height = px(240.);
        assert_eq!(preview_scroll("down", height), Some(px(36.)));
        assert_eq!(preview_scroll("up", height), Some(px(-36.)));
        assert_eq!(preview_scroll("pagedown", height), Some(px(204.)));
        assert_eq!(preview_scroll("pageup", height), Some(px(-204.)));
        assert_eq!(preview_scroll("pagedown", px(10.)), Some(px(36.)));
        assert_eq!(preview_scroll("enter", height), None);
    }

    #[test]
    fn counts_group_thousands() {
        assert_eq!(group_thousands(7), "7");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(2000), "2,000");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn a_timed_right_holds_until_it_runs_out() {
        let at = Instant::now();
        let hour = Duration::from_secs(3600);
        let timed = PermissionDecision {
            allow: true,
            scope: Timed,
            at,
            until: Some(at + hour),
        };
        assert!(timed.permits(at + hour / 2));
        assert!(!timed.permits(at + hour));
        let denied = PermissionDecision {
            allow: false,
            scope: Once,
            at,
            until: None,
        };
        assert!(!denied.permits(at));
    }

    #[test]
    fn durations_read_in_words() {
        assert_eq!(duration_words(Duration::from_secs(60)), "1 minute");
        assert_eq!(duration_words(Duration::from_secs(45 * 60)), "45 minutes");
        assert_eq!(duration_words(Duration::from_secs(3600)), "1 hour");
        assert_eq!(duration_words(Duration::from_secs(2 * 3600)), "2 hours");
        assert_eq!(duration_words(Duration::from_secs(90 * 60)), "1 h 30 min");
    }

    #[test]
    fn the_time_left_reads_in_the_right_unit() {
        assert_eq!(time_left(Duration::from_secs(42)), "42 s left");
        assert_eq!(time_left(Duration::from_secs(13 * 60 + 5)), "13 min left");
        assert_eq!(time_left(Duration::from_secs(3600 + 120)), "1 h 2 min left");
    }

    #[test]
    fn arrows_pick_how_long_and_stop_at_the_ends() {
        let scopes = [Once, Timed, Always];
        assert_eq!(step_scope(&scopes, Once, "right"), Some(Timed));
        assert_eq!(step_scope(&scopes, Always, "right"), Some(Always));
        assert_eq!(step_scope(&scopes, Once, "left"), Some(Once));
        assert_eq!(step_scope(&scopes, Once, "up"), None);
        assert_eq!(step_scope(&[Once], Timed, "right"), None);
    }

    #[test]
    fn initials_take_two_words() {
        assert_eq!(initials("ops assistant"), "OA");
        assert_eq!(initials("Bookkeeper"), "B");
        assert_eq!(initials("  "), "");
    }
}
