// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Identity avatar: the kit's avatar for people and assistants, with presence, a working ring and a stack that counts the rest.
//!
//! A person shows their picture or initials; an assistant shows a bot mark
//! on a tinted square. A dot in the corner says whether they're online,
//! busy or away, and an assistant that's working gets a turning ring. The
//! stack overlaps a few and counts the rest, naming everyone for screen
//! readers.
//!
//! Beyond presence an avatar can show what someone is doing: working (the
//! turning ring), typing (three dots in a bubble at the top corner) or
//! speaking (a soft ring that breathes). Under reduced motion each is drawn
//! still. A stack can spread its faces apart while the pointer is over it,
//! easing over the theme's hover fade, and names the people the count hides
//! in a tooltip.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, avatar::Avatar, h_flex, look,
    tooltip::Tooltip,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Transformation, Window, div, percentage, prelude::FluentBuilder as _,
    px,
};

use crate::common::Tone;

/// Whether someone is around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Presence {
    /// Nothing shown.
    #[default]
    Unknown,
    /// Here.
    Online,
    /// Here but occupied.
    Busy,
    /// Not here.
    Away,
    /// Signed out: a hollow dot.
    Offline,
    /// Here, but asks not to be interrupted.
    DoNotDisturb,
}

impl Presence {
    /// The state in words, lower case; empty when there is nothing to say.
    pub fn word(self) -> &'static str {
        match self {
            Self::Unknown => "",
            Self::Online => "online",
            Self::Busy => "busy",
            Self::Away => "away",
            Self::Offline => "offline",
            Self::DoNotDisturb => "do not disturb",
        }
    }
}

/// What someone is doing right now, drawn over the avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Activity {
    /// Nothing shown.
    #[default]
    Idle,
    /// Busy on a task: a turning ring.
    Working,
    /// Writing a message: three dots in a bubble.
    Typing,
    /// Talking in a call: a ring that breathes.
    Speaking,
}

impl Activity {
    /// The state in words, lower case; empty when there is nothing to say.
    pub fn word(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Working => "working",
            Self::Typing => "typing",
            Self::Speaking => "speaking",
        }
    }
}

/// Who an avatar stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    /// Their name.
    pub name: SharedString,
    /// Whether they're an assistant.
    pub agent: bool,
    /// Whether they're around.
    pub presence: Presence,
    /// Whether an assistant is working right now.
    pub working: bool,
    /// What they're doing right now.
    pub activity: Activity,
}

impl Identity {
    /// A person.
    pub fn person(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            agent: false,
            presence: Presence::Unknown,
            working: false,
            activity: Activity::Idle,
        }
    }

    /// An assistant.
    pub fn agent(name: impl Into<SharedString>) -> Self {
        Self {
            agent: true,
            ..Self::person(name)
        }
    }

    /// Set whether they're around.
    pub fn presence(mut self, presence: Presence) -> Self {
        self.presence = presence;
        self
    }

    /// Mark an assistant as working.
    pub fn working(mut self, working: bool) -> Self {
        self.working = working;
        self
    }

    /// Set what they're doing right now.
    pub fn activity(mut self, activity: Activity) -> Self {
        self.activity = activity;
        self
    }

    /// What the avatar draws: [`Activity::Working`] for an assistant marked
    /// working, otherwise the activity set.
    pub fn shown_activity(&self) -> Activity {
        if self.agent && self.working && self.activity == Activity::Idle {
            Activity::Working
        } else {
            self.activity
        }
    }

    /// The words a screen reader hears.
    pub fn spoken(&self) -> String {
        let mut words = vec![self.name.to_string()];
        if self.agent {
            words.push("assistant".into());
        }
        let activity = self.shown_activity();
        if activity != Activity::Idle {
            words.push(activity.word().into());
        }
        if self.presence != Presence::Unknown {
            words.push(self.presence.word().into());
        }
        words.join(", ")
    }
}

/// One avatar.
#[derive(IntoElement)]
pub struct IdentityAvatar {
    id: ElementId,
    style: StyleRefinement,
    who: Identity,
    size: f32,
}

impl IdentityAvatar {
    /// Create an avatar, 32 pixels across.
    pub fn new(id: impl Into<ElementId>, who: Identity) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            who,
            size: 32.,
        }
    }

    /// Set how many pixels across it is.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(16.);
        self
    }
}

impl Styled for IdentityAvatar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn presence_color(presence: Presence, cx: &App) -> Option<Hsla> {
    match presence {
        Presence::Unknown => None,
        Presence::Online => Some(Tone::Success.color(cx)),
        Presence::Busy => Some(Tone::Warning.color(cx)),
        Presence::Away => Some(cx.theme().muted_foreground),
        Presence::Offline => Some(cx.theme().muted_foreground),
        Presence::DoNotDisturb => Some(Tone::Danger.color(cx)),
    }
}

/// The size of a typing bubble's dots and their gap, for an avatar `size`
/// pixels across, on whole pixels.
pub fn typing_dot_metrics(size: f32) -> (f32, f32) {
    let dot = (size * 0.09).round().clamp(2., 4.);
    let gap = (dot * 0.75).round().max(1.);
    (dot, gap)
}

/// How strongly typing dot `ix` of three shows at animation `phase` (0 to 1):
/// each lifts in turn, the rest rest at a third.
pub fn typing_dot_strength(ix: usize, phase: f32) -> f32 {
    let center = (ix as f32 + 0.5) / 3.;
    let distance = (phase - center).abs().min(1. - (phase - center).abs());
    let lift = (1. - distance * 4.).max(0.);
    0.35 + 0.65 * lift
}

impl RenderOnce for IdentityAvatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let fill = theme.background;
        let accent = theme.primary;
        let size = self.size;
        let still = cx.reduce_motion();
        let dot = presence_color(self.who.presence, cx);
        let hollow = self.who.presence == Presence::Offline;
        let activity = self.who.shown_activity();
        let child_id =
            |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let face = if self.who.agent {
            div()
                .size(px(size))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(size * 0.28))
                .bg(accent.opacity(0.14))
                .child(
                    Icon::new(IconName::Bot)
                        .size(px(size * 0.55))
                        .text_color(accent),
                )
                .into_any_element()
        } else {
            Avatar::new()
                .name(self.who.name.clone())
                .with_size(px(size))
                .into_any_element()
        };
        let ring = (activity == Activity::Working).then(|| {
            let icon = Icon::new(IconName::LoaderCircle)
                .size(px(size + 6.))
                .text_color(accent.opacity(0.7));
            div()
                .absolute()
                .top(px(-3.))
                .left(px(-3.))
                .size(px(size + 6.))
                .map(|this| {
                    if still {
                        this.child(icon)
                    } else {
                        this.child(icon.with_animation(
                            child_id("ring"),
                            Animation::new(std::time::Duration::from_millis(1400)).repeat(),
                            |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                        ))
                    }
                })
                .into_any_element()
        });
        // Speaking: a ring in the success tone just outside the face, its
        // strength breathing between a third and full.
        let halo = (activity == Activity::Speaking).then(|| {
            let color = Tone::Success.color(cx);
            let halo = div()
                .absolute()
                .top(px(-3.))
                .left(px(-3.))
                .size(px(size + 6.))
                .rounded(if self.who.agent {
                    px(size * 0.28 + 3.)
                } else {
                    px(size)
                })
                .border_2()
                .border_color(color);
            if still {
                halo.into_any_element()
            } else {
                halo.with_animation(
                    child_id("halo"),
                    Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                    move |this, delta| {
                        let wave = 0.5 - 0.5 * (delta * std::f32::consts::TAU).cos();
                        this.border_color(color.opacity(0.35 + 0.65 * wave))
                    },
                )
                .into_any_element()
            }
        });
        // Typing: a small bubble of three dots over the top corner.
        let bubble = (activity == Activity::Typing).then(|| {
            let (d, gap) = typing_dot_metrics(size);
            let ink = look::ink::strong(cx);
            let dots = (0..3).map(move |ix| {
                let dot = div().size(px(d)).rounded_full().bg(ink);
                if still {
                    dot.opacity(typing_dot_strength(ix, 1. / 6.))
                        .into_any_element()
                } else {
                    dot.with_animation(
                        ("typing-dot", ix),
                        Animation::new(std::time::Duration::from_millis(1100)).repeat(),
                        move |this, phase| this.opacity(typing_dot_strength(ix, phase)),
                    )
                    .into_any_element()
                }
            });
            h_flex()
                .absolute()
                .top(px(-(d + 2.)))
                .right(px(-(d * 2. + gap)))
                .h(px(d + 6.))
                .px(px(3.))
                .gap(px(gap))
                .rounded_full()
                .bg(theme.popover)
                .shadow(look::surface::floating(cx))
                .children(dots)
        });
        div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .size(px(size))
            .role(Role::Image)
            .aria_label(self.who.spoken())
            .children(ring)
            .children(halo)
            .child(face)
            .children(dot.map(|color| {
                let d = (size * 0.3).max(8.);
                div()
                    .absolute()
                    .right(px(-1.))
                    .bottom(px(-1.))
                    .size(px(d))
                    .rounded_full()
                    .border_2()
                    .border_color(fill)
                    .bg(color)
                    // Offline: the dot's centre punched out to the surface.
                    .when(hollow, |this| {
                        this.child(
                            div()
                                .size_full()
                                .rounded_full()
                                .border_1()
                                .border_color(color)
                                .bg(fill),
                        )
                    })
            }))
            .children(bubble)
            .refine_style(&self.style)
    }
}

/// How a stack shows its members: the first few, then a count of the rest.
pub fn stack_split(count: usize, limit: usize) -> (usize, usize) {
    let limit = limit.max(1);
    if count <= limit {
        (count, 0)
    } else {
        (limit - 1, count - (limit - 1))
    }
}

/// What a screen reader hears for a stack: how many, who, then how many
/// of them are online and active ("5 people: Maya, Tom, …; 2 online, 1
/// typing").
pub fn stack_spoken(people: &[Identity]) -> String {
    let names = people
        .iter()
        .map(|p| p.name.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let noun = if people.len() == 1 {
        "person"
    } else {
        "people"
    };
    let mut tallies = Vec::new();
    let online = people
        .iter()
        .filter(|p| p.presence == Presence::Online)
        .count();
    if online > 0 {
        tallies.push(format!("{online} online"));
    }
    for activity in [Activity::Working, Activity::Typing, Activity::Speaking] {
        let n = people
            .iter()
            .filter(|p| p.shown_activity() == activity)
            .count();
        if n > 0 {
            tallies.push(format!("{n} {}", activity.word()));
        }
    }
    let mut spoken = format!("{} {noun}: {names}", people.len());
    if !tallies.is_empty() {
        spoken.push_str("; ");
        spoken.push_str(&tallies.join(", "));
    }
    spoken
}

/// How far each face in a stack sits from the previous one's right edge, in
/// pixels, for faces `size` across: overlapped by `overlap` of a face at
/// rest, spread to a small gap as `spread` (0 to 1) goes to 1.
pub fn stack_offset(size: f32, overlap: f32, spread: f32) -> f32 {
    let rest = -size * overlap.clamp(0., 0.9);
    let open = size * 0.12;
    rest + (open - rest) * spread.clamp(0., 1.)
}

/// Overlapping avatars with a count of the rest.
#[derive(IntoElement)]
pub struct IdentityStack {
    id: ElementId,
    people: Vec<Identity>,
    limit: usize,
    size: f32,
    overlap: f32,
    spread_on_hover: bool,
}

impl IdentityStack {
    /// Create a stack showing at most four.
    pub fn new(id: impl Into<ElementId>, people: Vec<Identity>) -> Self {
        Self {
            id: id.into(),
            people,
            limit: 4,
            size: 28.,
            overlap: 0.3,
            spread_on_hover: false,
        }
    }

    /// How much of a face the next one covers, from 0 (side by side) to 0.9;
    /// 0.3 by default.
    pub fn overlap(mut self, overlap: f32) -> Self {
        self.overlap = overlap.clamp(0., 0.9);
        self
    }

    /// Spread the faces apart while the pointer is over the stack, easing
    /// over the theme's hover fade. Off by default.
    pub fn spread_on_hover(mut self, spread: bool) -> Self {
        self.spread_on_hover = spread;
        self
    }

    /// Show at most this many, the last place going to the count.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    /// Set each avatar's size in pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(16.);
        self
    }
}

impl RenderOnce for IdentityStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (shown, rest) = stack_split(self.people.len(), self.limit);
        let size = self.size;
        let hovered = window.use_keyed_state(
            ElementId::NamedChild(self.id.clone().into(), "hovered".into()),
            cx,
            |_, _| false,
        );
        let spread = if self.spread_on_hover {
            look::hover_fade(
                ElementId::NamedChild(self.id.clone().into(), "spread".into()),
                *hovered.read(cx),
                window,
                cx,
            )
        } else {
            0.
        };
        let offset = stack_offset(size, self.overlap, spread);
        let hidden_names = self
            .people
            .iter()
            .skip(shown)
            .map(|p| p.name.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let theme = cx.theme();
        h_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label(stack_spoken(&self.people))
            .when(self.spread_on_hover, |this| {
                this.on_hover(move |over, _, cx| {
                    hovered.update(cx, |state, cx| {
                        if *state != *over {
                            *state = *over;
                            cx.notify();
                        }
                    })
                })
            })
            .children(self.people.iter().take(shown).enumerate().map(|(ix, who)| {
                div()
                    .when(ix > 0, |this| this.ml(px(offset)))
                    .rounded_full()
                    .border_2()
                    .border_color(theme.background)
                    .child(
                        IdentityAvatar::new(
                            ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("face-{ix}").into(),
                            ),
                            who.clone(),
                        )
                        .size(size),
                    )
            }))
            .when(rest > 0, |this| {
                this.child(
                    div()
                        .id(ElementId::NamedChild(self.id.clone().into(), "rest".into()))
                        .tooltip(move |window, cx| {
                            Tooltip::new(hidden_names.clone()).build(window, cx)
                        })
                        .ml(px(offset))
                        .size(px(size + 4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_2()
                        .border_color(theme.background)
                        .bg(theme.muted)
                        .text_label_small()
                        .child(format!("+{rest}")),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacks_keep_the_last_place_for_the_count() {
        assert_eq!(stack_split(3, 4), (3, 0));
        assert_eq!(stack_split(4, 4), (4, 0));
        assert_eq!(stack_split(7, 4), (3, 4));
    }

    #[test]
    fn activity_and_new_presences_read_out_in_words() {
        let who = Identity::person("Maya")
            .activity(Activity::Typing)
            .presence(Presence::DoNotDisturb);
        assert_eq!(who.spoken(), "Maya, typing, do not disturb");
        let who = Identity::person("Tom")
            .activity(Activity::Speaking)
            .presence(Presence::Offline);
        assert_eq!(who.spoken(), "Tom, speaking, offline");
    }

    #[test]
    fn a_working_assistant_shows_the_working_ring() {
        assert_eq!(
            Identity::agent("Payables").working(true).shown_activity(),
            Activity::Working
        );
        // An activity set explicitly wins.
        assert_eq!(
            Identity::agent("Payables")
                .working(true)
                .activity(Activity::Typing)
                .shown_activity(),
            Activity::Typing
        );
        // `working` is an assistant's flag; a person needs the activity.
        assert_eq!(
            Identity::person("Ana").working(true).shown_activity(),
            Activity::Idle
        );
    }

    #[test]
    fn stacks_tally_presence_and_activity() {
        let people = vec![
            Identity::person("Maya").presence(Presence::Online),
            Identity::person("Tom").activity(Activity::Typing),
            Identity::agent("Payables")
                .working(true)
                .presence(Presence::Online),
        ];
        assert_eq!(
            stack_spoken(&people),
            "3 people: Maya, Tom, Payables; 2 online, 1 working, 1 typing"
        );
        assert_eq!(stack_spoken(&people[..1]), "1 person: Maya; 1 online");
        assert_eq!(stack_spoken(&[Identity::person("Ana")]), "1 person: Ana");
    }

    #[test]
    fn stack_offset_overlaps_at_rest_and_spreads_open() {
        assert_eq!(stack_offset(30., 0.3, 0.), -9.);
        assert!((stack_offset(30., 0.3, 1.) - 3.6).abs() < 1e-4);
        assert!((stack_offset(30., 0.3, 0.5) - (-2.7)).abs() < 1e-4);
        assert_eq!(stack_offset(30., 2., 0.), -27., "overlap is capped");
    }

    #[test]
    fn typing_dots_scale_and_take_turns() {
        assert_eq!(typing_dot_metrics(20.), (2., 2.));
        assert_eq!(typing_dot_metrics(40.), (4., 3.));
        assert_eq!(typing_dot_metrics(100.), (4., 3.));
        // At each dot's moment it is fully lit and the others rest.
        for ix in 0..3 {
            let phase = (ix as f32 + 0.5) / 3.;
            assert!((typing_dot_strength(ix, phase) - 1.).abs() < 1e-5);
            assert!((typing_dot_strength((ix + 1) % 3, phase) - 0.35).abs() < 1e-5);
        }
    }

    #[test]
    fn identities_read_out_in_words() {
        let who = Identity::agent("Reconciler")
            .working(true)
            .presence(Presence::Busy);
        assert_eq!(who.spoken(), "Reconciler, assistant, working, busy");
        assert_eq!(Identity::person("Maya").spoken(), "Maya");
    }
}
