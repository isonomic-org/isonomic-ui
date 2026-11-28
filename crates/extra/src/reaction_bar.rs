// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reaction bar: a row of small toggles with public counts, such as likes or saves.
//!
//! Pressing a toggle marks it as yours with a light tint, pops its icon and
//! rolls the changed digits of its count in from below.

use std::{rc::Rc, sync::Arc, time::Duration};

use isonomic_ui::base::Toggle;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StyleRefinement, Styled, Transformation, Window,
    div, prelude::FluentBuilder as _, px, size,
};

const POP: Duration = Duration::from_millis(320);
const ROLL: Duration = Duration::from_millis(260);

/// The state of one reaction after it was toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactionState {
    /// How many people reacted, the user included when `active`.
    pub count: u64,
    /// Whether the user has reacted.
    pub active: bool,
}

/// One toggle in the bar.
#[derive(Clone)]
pub struct Reaction {
    key: SharedString,
    label: SharedString,
    noun: Option<(SharedString, SharedString)>,
    icon: Icon,
    active_icon: Option<Icon>,
    count: u64,
    active: bool,
}

impl Reaction {
    /// Create a reaction. `key` identifies it in [`ReactionBar::on_react`];
    /// `label` is the action as a verb, such as "Like".
    pub fn new(
        key: impl Into<SharedString>,
        icon: impl Into<Icon>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            noun: None,
            icon: icon.into(),
            active_icon: None,
            count: 0,
            active: false,
        }
    }

    /// A like, drawn as a heart.
    pub fn like() -> Self {
        Self::new("like", IconName::Heart, "Like").noun("like", "likes")
    }

    /// A save, drawn as a star that fills when active.
    pub fn save() -> Self {
        Self::new("save", IconName::Star, "Save")
            .active_icon(IconName::StarFill)
            .noun("save", "saves")
    }

    /// The count to start from. Zero shows no number.
    pub fn count(mut self, count: u64) -> Self {
        self.count = count;
        self
    }

    /// Whether the user has already reacted.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The icon to show while active, for example a filled version.
    pub fn active_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.active_icon = Some(icon.into());
        self
    }

    /// What is being counted, singular and plural, for the spoken label.
    pub fn noun(mut self, one: impl Into<SharedString>, many: impl Into<SharedString>) -> Self {
        self.noun = Some((one.into(), many.into()));
        self
    }

    /// The name a screen reader announces: the action, then the exact count.
    fn spoken(&self, count: u64) -> SharedString {
        if count == 0 {
            return self.label.clone();
        }
        let exact = grouped(count);
        match &self.noun {
            Some((one, many)) => {
                let noun = if count == 1 { one } else { many };
                format!("{}, {exact} {noun}", self.label).into()
            }
            None => format!("{}, {exact}", self.label).into(),
        }
    }
}

type ReactHandler = Rc<dyn Fn(&SharedString, ReactionState, &mut Window, &mut App)>;

/// A row of reaction toggles.
#[derive(IntoElement)]
pub struct ReactionBar {
    id: ElementId,
    style: StyleRefinement,
    reactions: Vec<Reaction>,
    on_react: Option<ReactHandler>,
}

impl ReactionBar {
    /// Create an empty bar.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            reactions: Vec::new(),
            on_react: None,
        }
    }

    /// Add a reaction. They are shown in the order they were added.
    pub fn reaction(mut self, reaction: Reaction) -> Self {
        self.reactions.push(reaction);
        self
    }

    /// Run after a reaction was toggled, with its key and new state.
    ///
    /// The bar shows the new state at once. When the caller later passes a
    /// different count or active flag for that reaction, the bar follows it.
    pub fn on_react(
        mut self,
        handler: impl Fn(&SharedString, ReactionState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_react = Some(Rc::new(handler));
        self
    }
}

impl Styled for ReactionBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// What the bar remembers about one reaction between frames.
struct Slot {
    /// The state last passed in by the caller, to notice when it changes.
    given: ReactionState,
    /// The state on screen.
    shown: ReactionState,
    /// Counts presses. It keys the pop animation so every press replays it.
    presses: usize,
}

#[derive(Default)]
struct BarState {
    slots: Vec<(SharedString, Slot)>,
}

impl BarState {
    fn slot(&mut self, reaction: &Reaction) -> &mut Slot {
        let given = ReactionState {
            count: reaction.count,
            active: reaction.active,
        };
        let ix = match self.slots.iter().position(|(key, _)| key == &reaction.key) {
            Some(ix) => ix,
            None => {
                let slot = Slot {
                    given,
                    shown: given,
                    presses: 0,
                };
                self.slots.push((reaction.key.clone(), slot));
                self.slots.len() - 1
            }
        };
        let slot = &mut self.slots[ix].1;
        if slot.given != given {
            slot.given = given;
            slot.shown = given;
        }
        slot
    }
}

/// The state after a press: the flag flips and the count follows it.
fn toggled(state: ReactionState) -> ReactionState {
    ReactionState {
        active: !state.active,
        count: if state.active {
            state.count.saturating_sub(1)
        } else {
            state.count.saturating_add(1)
        },
    }
}

/// Shorten a count the way feeds do: `1.2K`, `12K`, `1M`.
///
/// One decimal is kept below ten of a unit and dropped above it, and a value
/// that rounds up to a thousand of a unit moves to the next one.
pub fn short_count(count: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K")];
    if count < 1_000 {
        return count.to_string();
    }
    for (ix, (unit, suffix)) in UNITS.iter().enumerate() {
        if count < *unit {
            continue;
        }
        // Work in tenths of the unit so rounding never needs floating point.
        let tenths = (count * 10 + unit / 2) / unit;
        let (whole, tenth) = if tenths >= 100 {
            ((count + unit / 2) / unit, 0)
        } else {
            (tenths / 10, tenths % 10)
        };
        if whole >= 1_000 && ix > 0 {
            return format!("1{}", UNITS[ix - 1].1);
        }
        return if tenth == 0 {
            format!("{whole}{suffix}")
        } else {
            format!("{whole}.{tenth}{suffix}")
        };
    }
    count.to_string()
}

/// Write a count in full with thousands separators: `1,204`.
fn grouped(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The icon's scale during a pop: it dips, springs past its size and settles.
fn pop_scale(time: f32) -> f32 {
    if time < 0.3 {
        1. - 0.28 * (time / 0.3)
    } else {
        let rest = (time - 0.3) / 0.7;
        let spring = 1. - (1. - rest).powi(3);
        0.72 + 0.28 * spring + 0.12 * (rest * std::f32::consts::PI).sin()
    }
}

impl RenderOnce for ReactionBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let accent = theme.info;
        let tint = theme.info.opacity(0.1);
        let tint_edge = theme.info.opacity(0.4);
        let wash = theme.foreground.opacity(0.04);
        let radius = theme.radius;
        let animate = !cx.reduce_motion();

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| BarState::default());
        let on_react = self.on_react;
        let bar_id = self.id.clone();

        let toggles = self.reactions.into_iter().map(|reaction| {
            let (shown, presses) = state.update(cx, |state, _| {
                let slot = state.slot(&reaction);
                (slot.shown, slot.presses)
            });
            let key = reaction.key.clone();
            // Nested under the bar's id, so two bars can use the same keys.
            let toggle_id = ElementId::NamedChild(Arc::new(bar_id.clone()), key.clone());
            let focus_handle = window
                .use_keyed_state((toggle_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone();
            let is_focused = focus_handle.is_focused(window);

            let icon = reaction
                .active_icon
                .clone()
                .filter(|_| shown.active)
                .unwrap_or_else(|| reaction.icon.clone())
                .small();
            let icon = if animate && presses > 0 {
                icon.with_animation(("pop", presses), Animation::new(POP), |icon, time| {
                    let scale = pop_scale(time);
                    icon.transform(Transformation::scale(size(scale, scale)))
                })
                .into_any_element()
            } else {
                icon.into_any_element()
            };

            // Digits are keyed by their place and value, so only the places
            // that changed are new elements and roll in.
            let text = short_count(shown.count);
            let places = text.chars().count();
            let digits = (shown.count > 0).then(|| {
                h_flex()
                    .h(px(20.))
                    .overflow_hidden()
                    .children(text.chars().enumerate().map(|(ix, digit)| {
                        let cell = div().relative().child(digit.to_string());
                        if animate && presses > 0 {
                            let place = (places - ix) as u32 * 256 + digit as u32 % 256;
                            cell.with_animation(
                                ("digit", place as usize),
                                Animation::new(ROLL),
                                |this, time| {
                                    let eased = 1. - (1. - time).powi(3);
                                    this.opacity(eased).top(px(10. * (1. - eased)))
                                },
                            )
                            .into_any_element()
                        } else {
                            cell.into_any_element()
                        }
                    }))
            });

            let state = state.clone();
            let on_react = on_react.clone();
            Toggle::new(toggle_id)
                .pressed(shown.active)
                .accessibility_label(reaction.spoken(shown.count))
                .track_focus(&focus_handle)
                .on_change(move |_, _, window, cx| {
                    let next = state.update(cx, |state, cx| {
                        let slot = state
                            .slots
                            .iter_mut()
                            .find(|(slot_key, _)| slot_key == &key)
                            .map(|(_, slot)| slot)?;
                        slot.shown = toggled(slot.shown);
                        slot.presses += 1;
                        cx.notify();
                        Some(slot.shown)
                    });
                    if let (Some(next), Some(on_react)) = (next, &on_react) {
                        on_react(&key, next, window, cx);
                    }
                })
                .h_flex()
                .flex_none()
                .gap(px(6.))
                .h(px(30.))
                .px(px(10.))
                .rounded(radius)
                .border_1()
                .text_label()
                .cursor_pointer()
                .map(|this| {
                    if shown.active {
                        this.bg(tint).border_color(tint_edge).text_color(accent)
                    } else {
                        this.bg(fill)
                            .border_color(line)
                            .text_color(quiet)
                            .hover(|style| style.bg(wash).text_color(ink))
                    }
                })
                .when(is_focused, |this| this.focus_ring_style(window, cx))
                .child(icon)
                .children(digits)
        });
        let toggles: Vec<_> = toggles.collect();

        h_flex()
            .id(self.id)
            .gap_1p5()
            .flex_wrap()
            .children(toggles)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_flips_the_flag_and_moves_the_count() {
        let fresh = ReactionState {
            count: 128,
            active: false,
        };
        let pressed = toggled(fresh);
        assert_eq!((pressed.count, pressed.active), (129, true));
        assert_eq!(toggled(pressed), fresh);
    }

    #[test]
    fn a_count_never_goes_below_zero() {
        let odd = ReactionState {
            count: 0,
            active: true,
        };
        assert_eq!(toggled(odd).count, 0);
    }

    #[test]
    fn counts_shorten_like_a_feed() {
        assert_eq!(short_count(0), "0");
        assert_eq!(short_count(999), "999");
        assert_eq!(short_count(1_000), "1K");
        assert_eq!(short_count(1_234), "1.2K");
        assert_eq!(short_count(9_960), "10K");
        assert_eq!(short_count(12_499), "12K");
        assert_eq!(short_count(2_500_000), "2.5M");
    }

    #[test]
    fn a_count_that_rounds_up_moves_to_the_next_unit() {
        assert_eq!(short_count(999_950), "1M");
        assert_eq!(short_count(999_499), "999K");
    }

    #[test]
    fn exact_counts_are_grouped() {
        assert_eq!(grouped(7), "7");
        assert_eq!(grouped(1_204), "1,204");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }

    #[test]
    fn the_spoken_label_carries_the_exact_count() {
        let like = Reaction::like();
        assert_eq!(like.spoken(0).as_ref(), "Like");
        assert_eq!(like.spoken(1).as_ref(), "Like, 1 like");
        assert_eq!(like.spoken(1_204).as_ref(), "Like, 1,204 likes");
    }

    #[test]
    fn the_pop_starts_and_ends_at_rest() {
        assert!((pop_scale(0.) - 1.).abs() < 0.001);
        assert!((pop_scale(1.) - 1.).abs() < 0.001);
        assert!(pop_scale(0.3) < 0.8);
    }
}
