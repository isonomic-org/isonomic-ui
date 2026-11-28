// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Empty state panel: what to show where content would be, told apart by why it's empty, with the next thing to do.
//!
//! "Nothing yet", "nothing matches", "couldn't load" and "not allowed" each
//! get their own mark and wording, so a person can tell a fresh start from
//! a filter that's too narrow, a failure, or a lock. Each can offer a main
//! action and a second one, and a compact form fits inside a list.
//!
//! [`EmptyStatePanel::of`] fills in plain words for a reason when the caller
//! has none of its own. A failure can carry a detail line (an error code or
//! a reference, in mono) and show its retry as running; it is announced as an
//! alert. A bare panel drops the dashed outline to sit inside a card or a
//! table that already has an edge.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::Tone;

/// Why there's nothing to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyReason {
    /// Nothing has been added yet.
    Fresh,
    /// A search or filter hides everything.
    NoMatches,
    /// Loading failed.
    Failed,
    /// The person isn't allowed to see it.
    Locked,
}

impl EmptyReason {
    /// Plain words for the reason, for a caller with none of its own: a
    /// title and a line under it.
    pub fn default_words(self) -> (&'static str, &'static str) {
        match self {
            Self::Fresh => ("Nothing here yet", "Items you add will show up here."),
            Self::NoMatches => (
                "No results",
                "Nothing matches the search or filters. Try fewer words or clear the filters.",
            ),
            Self::Failed => (
                "This didn't load",
                "Something went wrong on the way. Try again in a moment.",
            ),
            Self::Locked => (
                "You don't have access",
                "Ask the owner to share it with you.",
            ),
        }
    }

    fn look(self) -> (IconName, Tone) {
        match self {
            Self::Fresh => (IconName::Inbox, Tone::Info),
            Self::NoMatches => (IconName::Search, Tone::Neutral),
            Self::Failed => (IconName::CircleAlert, Tone::Danger),
            Self::Locked => (IconName::Ban, Tone::Warning),
        }
    }
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// What a screen reader hears: the title, the body, then the detail.
pub fn spoken_words(title: &str, body: &str, detail: Option<&str>, busy: bool) -> String {
    let mut out = format!("{title}. {body}");
    if let Some(detail) = detail.filter(|d| !d.is_empty()) {
        out.push_str(&format!(" {detail}."));
    }
    if busy {
        out.push_str(" Trying again.");
    }
    out
}

/// What to show in place of empty content.
#[derive(IntoElement)]
pub struct EmptyStatePanel {
    id: ElementId,
    style: StyleRefinement,
    reason: EmptyReason,
    title: SharedString,
    body: SharedString,
    primary: Option<(SharedString, Handler)>,
    secondary: Option<(SharedString, Handler)>,
    compact: bool,
    detail: Option<SharedString>,
    busy: bool,
    bare: bool,
}

impl EmptyStatePanel {
    /// Create a panel.
    pub fn new(
        id: impl Into<ElementId>,
        reason: EmptyReason,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            reason,
            title: title.into(),
            body: body.into(),
            primary: None,
            secondary: None,
            compact: false,
            detail: None,
            busy: false,
            bare: false,
        }
    }

    /// Create a panel with the reason's own plain words.
    pub fn of(id: impl Into<ElementId>, reason: EmptyReason) -> Self {
        let (title, body) = reason.default_words();
        Self::new(id, reason, title, body)
    }

    /// Add a detail line under the body, such as an error code or a request
    /// reference, set in mono.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Show the main action as running (a retry in flight): it spins and
    /// can't be pressed again.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Drop the dashed outline, to sit inside something that has an edge.
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }

    /// Offer a main action.
    pub fn primary(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.primary = Some((label.into(), Rc::new(handler)));
        self
    }

    /// Offer a second action.
    pub fn secondary(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.secondary = Some((label.into(), Rc::new(handler)));
        self
    }

    /// Fit it on one line, inside a list.
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Styled for EmptyStatePanel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EmptyStatePanel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let (icon, tone) = self.reason.look();
        let color = tone.color(cx);
        let tint = tone.tint(cx);
        let busy = self.busy;
        let button = |which: &'static str, action: Option<(SharedString, Handler)>, main: bool| {
            action.map(|(label, handler)| {
                Button::new(ElementId::NamedChild(self.id.clone().into(), which.into()))
                    .small()
                    .map(|b| if main { b.primary() } else { b.ghost() })
                    .label(label)
                    .when(main, |b| b.loading(busy))
                    .on_click(move |_, window, cx| handler(window, cx))
            })
        };
        let role = if self.reason == EmptyReason::Failed {
            Role::Alert
        } else {
            Role::Group
        };
        let spoken = spoken_words(&self.title, &self.body, self.detail.as_deref(), busy);
        let bare = self.bare;
        let detail = self.detail.clone().map(|detail| {
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_caption()
                .text_color(isonomic_ui::component::look::ink::subtle(cx))
                .child(detail)
        });
        let primary = button("primary", self.primary.clone(), true);
        let secondary = button("secondary", self.secondary.clone(), false);
        if self.compact {
            return h_flex()
                .id(self.id.clone())
                .gap_3()
                .px_3()
                .py_2p5()
                .rounded(theme.radius)
                .when(!bare, |this| {
                    this.border_1().border_dashed().border_color(line)
                })
                .text_body()
                .role(role)
                .aria_label(spoken.clone())
                .child(Icon::new(icon).small().text_color(color))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().font_medium().truncate().child(self.title.clone()))
                        .child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .truncate()
                                .child(self.body.clone()),
                        )
                        .children(detail.map(|d| d.truncate())),
                )
                .children(secondary)
                .children(primary)
                .refine_style(&self.style)
                .into_any_element();
        }
        v_flex()
            .id(self.id.clone())
            .w_full()
            .items_center()
            .gap_3()
            .px_6()
            .py_8()
            .rounded(theme.radius_lg)
            .when(!bare, |this| {
                this.border_1().border_dashed().border_color(line)
            })
            .text_body()
            .role(role)
            .aria_label(spoken)
            .child(
                div()
                    .size(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(tint)
                    .child(Icon::new(icon).text_color(color)),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_1()
                    .max_w(px(340.))
                    .child(
                        div()
                            .text_base()
                            .font_medium()
                            .text_center()
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_center()
                            .text_color(quiet)
                            .line_height(relative(1.45))
                            .child(self.body.clone()),
                    )
                    .children(detail.map(|d| d.text_center().mt_1())),
            )
            .when(primary.is_some() || secondary.is_some(), |this| {
                this.child(h_flex().gap_2().children(secondary).children(primary))
            })
            .refine_style(&self.style)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_reason_has_its_own_look() {
        let looks: Vec<_> = [
            EmptyReason::Fresh,
            EmptyReason::NoMatches,
            EmptyReason::Failed,
            EmptyReason::Locked,
        ]
        .iter()
        .map(|r| r.look().1)
        .collect();
        assert_eq!(
            looks,
            [Tone::Info, Tone::Neutral, Tone::Danger, Tone::Warning]
        );
    }

    #[test]
    fn every_reason_has_plain_words_of_its_own() {
        let words: Vec<_> = [
            EmptyReason::Fresh,
            EmptyReason::NoMatches,
            EmptyReason::Failed,
            EmptyReason::Locked,
        ]
        .iter()
        .map(|r| r.default_words())
        .collect();
        assert_eq!(words[1].0, "No results");
        for (ix, (title, body)) in words.iter().enumerate() {
            assert!(!title.is_empty() && body.ends_with('.'));
            assert!(words.iter().skip(ix + 1).all(|(other, _)| other != title));
        }
    }

    #[test]
    fn spoken_words_add_the_detail_and_a_running_retry() {
        assert_eq!(
            spoken_words("Feed failed", "It timed out.", None, false),
            "Feed failed. It timed out."
        );
        assert_eq!(
            spoken_words("Feed failed", "It timed out.", Some("Error 504"), true),
            "Feed failed. It timed out. Error 504. Trying again."
        );
        assert_eq!(
            spoken_words("Feed failed", "It timed out.", Some(""), false),
            "Feed failed. It timed out."
        );
    }
}
