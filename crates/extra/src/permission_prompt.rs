//! Permission prompt: an assistant asks for one specific right.
//!
//! The prompt says who asks, for what, why, and exactly what will happen. The
//! right can be given just once, for a while, or always. Once answered the
//! prompt folds into one line with the time left and a way to revoke it.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, Keystroke, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task,
    Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    kbd::Kbd,
    v_flex,
};

use crate::common::resting_shadow;

/// How much harm the action can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionRisk {
    /// Easy to take back.
    Safe,
    /// Changes something others will see.
    Risky,
    /// Cannot be taken back. It can never be allowed for always.
    Destructive,
}

/// How long a right is given for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionScope {
    /// This one time.
    Once,
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
    scopes: Vec<PermissionScope>,
    default_scope: PermissionScope,
    duration: Duration,
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
            preview_title: "What will happen".into(),
            preview_limit: 3,
            scopes: vec![
                PermissionScope::Once,
                PermissionScope::Timed,
                PermissionScope::Always,
            ],
            default_scope: PermissionScope::Once,
            duration: Duration::from_secs(60 * 60),
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

/// The choices on offer. A destructive action is never offered "always".
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

        let scopes = offered_scopes(&self.scopes, self.risk);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| PromptState {
            scope: self.default_scope,
            decision: None,
            show_all: false,
            ticker: None,
        });
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
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| decide(None, window, cx)
        };

        let card = v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(400.))
            .overflow_hidden()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
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
                    format!("{} was told and will not do it", self.who),
                    "Change",
                ),
                (true, _) if ran_out => (
                    "Ran out".to_string(),
                    "It has to be asked for again".to_string(),
                    "Ask again",
                ),
                (true, PermissionScope::Once) => (
                    "Allowed once".to_string(),
                    "It will not carry over".to_string(),
                    "Change",
                ),
                (true, PermissionScope::Timed) => (
                    format!("Allowed for {}", duration_words(duration)),
                    left.map(time_left).unwrap_or_default(),
                    "Revoke",
                ),
                (true, PermissionScope::Always) => (
                    "Always allowed".to_string(),
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
                                .child(div().truncate().text_xs().text_color(quiet).child(note)),
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
            PermissionScope::Once => "Allow once".to_string(),
            PermissionScope::Timed => format!("Allow for {}", duration_words(duration)),
            PermissionScope::Always => "Always allow".to_string(),
        };
        let scope_label = |scope: PermissionScope| match scope {
            PermissionScope::Once => "Just once".to_string(),
            PermissionScope::Timed => duration_words(duration),
            PermissionScope::Always => "Always".to_string(),
        };
        let pick = |to: PermissionScope| {
            let state = state.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.scope = to;
                    cx.notify();
                })
            }
        };
        let hint = |keys: &'static str| Keystroke::parse(keys).ok().map(Kbd::new);
        let shown = if show_all {
            self.items.len()
        } else {
            self.preview_limit.min(self.items.len())
        };
        let hidden = self.items.len() - shown;

        card.on_key_down({
            let allow = allow.clone();
            let deny = deny.clone();
            let state = state.clone();
            let scopes = scopes.clone();
            move |event, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" => allow(window, cx),
                    "escape" => deny(window, cx),
                    key => match step_scope(&scopes, scope, key) {
                        Some(next) => state.update(cx, |state, cx| {
                            state.scope = next;
                            cx.notify();
                        }),
                        None => return,
                    },
                }
                cx.stop_propagation();
            }
        })
        .child(
            v_flex()
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
                                .text_xs()
                                .font_medium()
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
                                .text_xs()
                                .font_medium()
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
        .when(!self.items.is_empty(), |this| {
            this.child(
                v_flex()
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
                                    .text_xs()
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
                    this.child(h_flex().gap_1().p_0p5().rounded(radius).bg(chip).children(
                        scopes.iter().enumerate().map(|(ix, option)| {
                            div()
                                .id((self.id.clone(), SharedString::from(format!("scope-{ix}"))))
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
                        }),
                    ))
                })
                .when(self.risk == PermissionRisk::Destructive, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child("Destructive actions cannot be allowed for always"),
                    )
                })
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new((self.id.clone(), "deny"))
                                .outline()
                                .small()
                                .child(h_flex().gap_1p5().child("Deny").children(hint("escape")))
                                .on_click(move |_, window, cx| deny(window, cx)),
                        )
                        .child({
                            let button = Button::new((self.id.clone(), "allow")).small();
                            let button = if self.risk == PermissionRisk::Destructive {
                                button.danger()
                            } else {
                                button.primary()
                            };
                            button
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(allow_label)
                                        .children(hint("enter")),
                                )
                                .on_click(move |_, window, cx| allow(window, cx))
                        }),
                ),
        )
        .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PermissionScope::{Always, Once, Timed};

    #[test]
    fn destructive_actions_are_never_offered_always() {
        let all = [Once, Timed, Always];
        assert_eq!(offered_scopes(&all, PermissionRisk::Risky), all);
        assert_eq!(
            offered_scopes(&all, PermissionRisk::Destructive),
            [Once, Timed]
        );
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
