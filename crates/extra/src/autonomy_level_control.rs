// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Autonomy level control: how much an assistant may do without asking.
//!
//! Three levels, and under them the assistant's real actions sorted into what
//! goes ahead and what stops for a yes, so the level is never abstract. Any
//! action can be locked to stop every time, whatever the level.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
    ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How much harm an action can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRisk {
    /// Reads, drafts and other things that are easy to take back.
    Safe,
    /// Changes something others will see.
    Risky,
    /// Cannot be taken back.
    Destructive,
}

/// How much the assistant may do on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutonomyLevel {
    /// Every action waits for a person.
    AskEveryTime,
    /// Safe actions run; the rest wait.
    AskWhenRisky,
    /// Everything runs unless it is locked.
    RunFreely,
}

impl AutonomyLevel {
    const ALL: [Self; 3] = [Self::AskEveryTime, Self::AskWhenRisky, Self::RunFreely];

    fn label(self) -> &'static str {
        match self {
            Self::AskEveryTime => "Ask every time",
            Self::AskWhenRisky => "Ask when risky",
            Self::RunFreely => "Run freely",
        }
    }

    /// What the level means, in one line.
    fn meaning(self) -> &'static str {
        match self {
            Self::AskEveryTime => "Nothing runs until you say yes",
            Self::AskWhenRisky => "Safe steps go ahead, risky ones stop for a yes",
            Self::RunFreely => "All of it goes ahead, apart from what you locked",
        }
    }
}

/// Whether an action waits for a person. A locked action always asks; below
/// that the level decides. Use the same rule when the assistant runs, so the
/// setting and the behaviour never drift apart.
pub fn asks_first(risk: ActionRisk, level: AutonomyLevel, locked: bool) -> bool {
    locked
        || match level {
            AutonomyLevel::AskEveryTime => true,
            AutonomyLevel::AskWhenRisky => risk != ActionRisk::Safe,
            AutonomyLevel::RunFreely => false,
        }
}

/// The level an arrow key moves to. It stops at the ends.
fn step_level(level: AutonomyLevel, key: &str) -> Option<AutonomyLevel> {
    let ix = AutonomyLevel::ALL
        .iter()
        .position(|other| *other == level)?;
    let next = match key {
        "right" | "down" => (ix + 1).min(AutonomyLevel::ALL.len() - 1),
        "left" | "up" => ix.saturating_sub(1),
        "home" => 0,
        "end" => AutonomyLevel::ALL.len() - 1,
        _ => return None,
    };
    Some(AutonomyLevel::ALL[next])
}

/// Something the assistant can do.
#[derive(Clone)]
pub struct AgentAction {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    risk: ActionRisk,
    icon: Option<IconName>,
}

impl AgentAction {
    /// Create an action with its risk.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        risk: ActionRisk,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            risk,
            icon: None,
        }
    }

    /// A quiet line under the label.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// An icon in front of the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
}

type LevelHandler = Rc<dyn Fn(AutonomyLevel, &mut Window, &mut App)>;
type LockHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;

/// A level selector with the actions it lets through.
#[derive(IntoElement)]
pub struct AutonomyLevelControl {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: SharedString,
    actions: Vec<AgentAction>,
    level: Option<AutonomyLevel>,
    default_level: AutonomyLevel,
    locked: Option<Vec<SharedString>>,
    default_locked: Vec<SharedString>,
    on_level_change: Option<LevelHandler>,
    on_locked_change: Option<LockHandler>,
}

impl AutonomyLevelControl {
    /// Create a control with no actions, starting at "Ask when risky".
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Permissions".into(),
            subtitle: "How much it does before checking with you".into(),
            actions: Vec::new(),
            level: None,
            default_level: AutonomyLevel::AskWhenRisky,
            locked: None,
            default_locked: Vec::new(),
            on_level_change: None,
            on_locked_change: None,
        }
    }

    /// Set the heading, such as the assistant's name.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the line under the heading.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// Add an action after the ones already added.
    pub fn action(mut self, action: AgentAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Control the level. Pair it with `on_level_change`.
    pub fn level(mut self, level: AutonomyLevel) -> Self {
        self.level = Some(level);
        self
    }

    /// Set the level at first, when the control keeps it itself.
    pub fn default_level(mut self, level: AutonomyLevel) -> Self {
        self.default_level = level;
        self
    }

    /// Control which actions are locked to stop every time, by id. Pair it with
    /// `on_locked_change`.
    pub fn locked(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.locked = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Set the locks at first, when the control keeps them itself.
    pub fn default_locked(
        mut self,
        ids: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_locked = ids.into_iter().map(Into::into).collect();
        self
    }

    /// Run when another level is picked.
    pub fn on_level_change(
        mut self,
        handler: impl Fn(AutonomyLevel, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_level_change = Some(Rc::new(handler));
        self
    }

    /// Run when a lock is set or removed, with the ids of all locked actions.
    pub fn on_locked_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_locked_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for AutonomyLevelControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct ControlState {
    level: AutonomyLevel,
    locked: Vec<SharedString>,
}

impl RenderOnce for AutonomyLevelControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ControlState {
            level: self.default_level,
            locked: self.default_locked.clone(),
        });
        let (kept_level, kept_locked) = {
            let state = state.read(cx);
            (state.level, state.locked.clone())
        };
        let level = self.level.unwrap_or(kept_level);
        let locked = self.locked.clone().unwrap_or(kept_locked);
        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());

        let set_level: LevelHandler = {
            let state = state.clone();
            let on_level_change = self.on_level_change.clone();
            Rc::new(move |next, window, cx| {
                if next == level {
                    return;
                }
                state.update(cx, |state, cx| {
                    state.level = next;
                    cx.notify();
                });
                if let Some(handler) = &on_level_change {
                    handler(next, window, cx);
                }
            })
        };

        let focus = window
            .use_keyed_state((self.id.clone(), "levels-focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);
        let levels = h_flex()
            .id(child_id("levels".into()))
            .track_focus(&focus)
            .role(Role::RadioGroup)
            .aria_label("How much to ask")
            .on_key_down({
                let set_level = set_level.clone();
                move |event, window, cx| {
                    if let Some(next) = step_level(level, event.keystroke.key.as_str()) {
                        set_level(next, window, cx);
                        cx.stop_propagation();
                    }
                }
            })
            .w_full()
            .items_stretch()
            .gap_1()
            .p_0p5()
            .rounded(radius)
            .bg(chip)
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .children(
                AutonomyLevel::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(ix, option)| {
                        div()
                            .id(child_id(format!("level-{ix}")))
                            .role(Role::RadioButton)
                            .aria_label(option.label())
                            .aria_selected(option == level)
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .justify_center()
                            .min_h(px(28.))
                            .px_1p5()
                            .py_1()
                            .rounded(radius)
                            .cursor_pointer()
                            .map(|this| {
                                if option == level {
                                    this.bg(fill).font_medium().shadow(resting_shadow(cx))
                                } else {
                                    this.text_color(quiet)
                                }
                            })
                            .on_click({
                                let set_level = set_level.clone();
                                let focus = focus.clone();
                                move |_, window, cx| {
                                    focus.focus(window, cx);
                                    set_level(option, window, cx);
                                }
                            })
                            // In a narrow column the label wraps instead of
                            // running into its neighbour.
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_center()
                                    .line_height(relative(1.25))
                                    .child(option.label()),
                            )
                    }),
            );

        let row = |action: &AgentAction, is_locked: bool| {
            let tag = match action.risk {
                ActionRisk::Safe => None,
                ActionRisk::Risky => Some(("Risky", warning, IconName::TriangleAlert)),
                ActionRisk::Destructive => Some(("Destructive", danger, IconName::CircleAlert)),
            };
            let id = action.id.clone();
            h_flex()
                .gap_2p5()
                .min_h(px(34.))
                .px_2()
                .children(
                    action
                        .icon
                        .clone()
                        .map(|icon| Icon::new(icon).small().flex_none().text_color(quiet)),
                )
                .child(
                    v_flex()
                        .min_w_0()
                        .child(div().truncate().child(action.label.clone()))
                        .children(action.detail.clone().map(|detail| {
                            div().truncate().text_caption().text_color(quiet).child(detail)
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
                }))
                .child(div().flex_1())
                .child(
                    Button::new(child_id(format!("lock-{id}")))
                        .ghost()
                        .xsmall()
                        .label(if is_locked { "Locked" } else { "Lock" })
                        .selected(is_locked)
                        .accessibility_label(format!("Stop every time before: {}", action.label))
                        .on_click({
                            let state = state.clone();
                            let locked = locked.clone();
                            let on_locked_change = self.on_locked_change.clone();
                            move |_, window, cx| {
                                let mut next = locked.clone();
                                match next.iter().position(|other| *other == id) {
                                    Some(ix) => {
                                        next.remove(ix);
                                    }
                                    None => next.push(id.clone()),
                                }
                                state.update(cx, |state, cx| {
                                    state.locked = next.clone();
                                    cx.notify();
                                });
                                if let Some(handler) = &on_locked_change {
                                    handler(&next, window, cx);
                                }
                            }
                        }),
                )
        };
        let mut runs = Vec::new();
        let mut asks = Vec::new();
        for action in &self.actions {
            let is_locked = locked.contains(&action.id);
            if asks_first(action.risk, level, is_locked) {
                asks.push(row(action, is_locked));
            } else {
                runs.push(row(action, is_locked));
            }
        }

        let section = |title: &'static str, rows: Vec<isonomic_ui::Div>, empty: &'static str| {
            v_flex()
                .gap_0p5()
                .child(
                    h_flex()
                        .gap_2()
                        .px_2()
                        .text_color(quiet)
                        .child(title)
                        .child(
                            div()
                                .px_1()
                                .rounded(px(4.))
                                .border_1()
                                .border_color(line)
                                .text_caption()
                                .child(rows.len().to_string()),
                        ),
                )
                .when(rows.is_empty(), |this| {
                    this.child(
                        div()
                            .mx_1()
                            .py_1p5()
                            .rounded(radius)
                            .bg(chip)
                            .text_center()
                            .text_color(quiet)
                            .child(empty),
                    )
                })
                .children(rows)
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(400.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .child(
                        v_flex()
                            .child(div().font_medium().child(self.title))
                            .child(div().text_color(quiet).child(self.subtitle)),
                    )
                    .child(levels)
                    .child(div().text_caption().text_color(quiet).child(level.meaning())),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .pb_3()
                    .border_t_1()
                    .border_color(line)
                    .child(section("Goes ahead", runs, "Nothing goes ahead unasked"))
                    .child(section("Stops for a yes", asks, "Nothing stops for a yes")),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ActionRisk::{Destructive, Risky, Safe};
    use AutonomyLevel::{AskEveryTime, AskWhenRisky, RunFreely};

    #[test]
    fn the_level_decides_what_asks_first() {
        assert!(asks_first(Safe, AskEveryTime, false));
        assert!(!asks_first(Safe, AskWhenRisky, false));
        assert!(asks_first(Risky, AskWhenRisky, false));
        assert!(asks_first(Destructive, AskWhenRisky, false));
        assert!(!asks_first(Destructive, RunFreely, false));
    }

    #[test]
    fn a_lock_always_asks() {
        assert!(asks_first(Safe, RunFreely, true));
        assert!(asks_first(Safe, AskWhenRisky, true));
    }

    #[test]
    fn arrows_step_through_the_levels_and_stop_at_the_ends() {
        assert_eq!(step_level(AskEveryTime, "right"), Some(AskWhenRisky));
        assert_eq!(step_level(AskWhenRisky, "down"), Some(RunFreely));
        assert_eq!(step_level(RunFreely, "right"), Some(RunFreely));
        assert_eq!(step_level(AskEveryTime, "left"), Some(AskEveryTime));
        assert_eq!(step_level(AskWhenRisky, "home"), Some(AskEveryTime));
        assert_eq!(step_level(AskWhenRisky, "end"), Some(RunFreely));
        assert_eq!(step_level(AskWhenRisky, "a"), None);
    }
}
