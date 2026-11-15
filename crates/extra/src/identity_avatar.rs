//! Identity avatar: the kit's avatar for people and assistants, with presence, a working ring and a stack that counts the rest.
//!
//! A person shows their picture or initials; an assistant shows a bot mark
//! on a tinted square. A dot in the corner says whether they're online,
//! busy or away, and an assistant that's working gets a turning ring. The
//! stack overlaps a few and counts the rest, naming everyone for screen
//! readers.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, avatar::Avatar, h_flex,
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
}

impl Presence {
    fn word(self) -> &'static str {
        match self {
            Self::Unknown => "",
            Self::Online => "online",
            Self::Busy => "busy",
            Self::Away => "away",
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
}

impl Identity {
    /// A person.
    pub fn person(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            agent: false,
            presence: Presence::Unknown,
            working: false,
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

    /// The words a screen reader hears.
    pub fn spoken(&self) -> String {
        let mut words = vec![self.name.to_string()];
        if self.agent {
            words.push("assistant".into());
        }
        if self.working {
            words.push("working".into());
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
    }
}

impl RenderOnce for IdentityAvatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let fill = theme.background;
        let accent = theme.primary;
        let size = self.size;
        let dot = presence_color(self.who.presence, cx);
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
        let ring = (self.who.agent && self.who.working).then(|| {
            div()
                .absolute()
                .top(px(-3.))
                .left(px(-3.))
                .size(px(size + 6.))
                .child(
                    Icon::new(IconName::LoaderCircle)
                        .size(px(size + 6.))
                        .text_color(accent.opacity(0.7))
                        .with_animation(
                            ElementId::NamedChild(self.id.clone().into(), "ring".into()),
                            Animation::new(std::time::Duration::from_millis(1400)).repeat(),
                            |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                        ),
                )
        });
        div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .size(px(size))
            .role(Role::Image)
            .aria_label(self.who.spoken())
            .children(ring)
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
            }))
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

/// Overlapping avatars with a count of the rest.
#[derive(IntoElement)]
pub struct IdentityStack {
    id: ElementId,
    people: Vec<Identity>,
    limit: usize,
    size: f32,
}

impl IdentityStack {
    /// Create a stack showing at most four.
    pub fn new(id: impl Into<ElementId>, people: Vec<Identity>) -> Self {
        Self {
            id: id.into(),
            people,
            limit: 4,
            size: 28.,
        }
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (shown, rest) = stack_split(self.people.len(), self.limit);
        let names = self
            .people
            .iter()
            .map(|p| p.name.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let size = self.size;
        h_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label(format!(
                "{} {}: {names}",
                self.people.len(),
                if self.people.len() == 1 {
                    "person"
                } else {
                    "people"
                }
            ))
            .children(self.people.iter().take(shown).enumerate().map(|(ix, who)| {
                div()
                    .when(ix > 0, |this| this.ml(px(-size * 0.3)))
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
                        .ml(px(-size * 0.3))
                        .size(px(size + 4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_2()
                        .border_color(theme.background)
                        .bg(theme.muted)
                        .text_xs()
                        .font_semibold()
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
    fn identities_read_out_in_words() {
        let who = Identity::agent("Reconciler")
            .working(true)
            .presence(Presence::Busy);
        assert_eq!(who.spoken(), "Reconciler, assistant, working, busy");
        assert_eq!(Identity::person("Maya").spoken(), "Maya");
    }
}
