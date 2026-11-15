//! Empty state panel: what to show where content would be, told apart by why it's empty, with the next thing to do.
//!
//! "Nothing yet", "nothing matches", "couldn't load" and "not allowed" each
//! get their own mark and wording, so a person can tell a fresh start from
//! a filter that's too narrow, a failure, or a lock. Each can offer a main
//! action and a second one, and a compact form fits inside a list.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
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
}

impl EmptyStatePanel {
    /// Create a panel.
    pub fn new(id: impl Into<ElementId>, reason: EmptyReason, title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            reason,
            title: title.into(),
            body: body.into(),
            primary: None,
            secondary: None,
            compact: false,
        }
    }

    /// Offer a main action.
    pub fn primary(mut self, label: impl Into<SharedString>, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.primary = Some((label.into(), Rc::new(handler)));
        self
    }

    /// Offer a second action.
    pub fn secondary(mut self, label: impl Into<SharedString>, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
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
        let button = |which: &'static str, action: Option<(SharedString, Handler)>, main: bool| {
            action.map(|(label, handler)| {
                Button::new(ElementId::NamedChild(self.id.clone().into(), which.into()))
                    .small()
                    .map(|b| if main { b.primary() } else { b.ghost() })
                    .label(label)
                    .on_click(move |_, window, cx| handler(window, cx))
            })
        };
        let primary = button("primary", self.primary.clone(), true);
        let secondary = button("secondary", self.secondary.clone(), false);
        if self.compact {
            return h_flex()
                .id(self.id.clone())
                .gap_3()
                .px_3()
                .py_2p5()
                .rounded(theme.radius)
                .border_1()
                .border_dashed()
                .border_color(line)
                .text_sm()
                .aria_label(format!("{}. {}", self.title, self.body))
                .child(Icon::new(icon).small().text_color(color))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().font_medium().truncate().child(self.title.clone()))
                        .child(div().text_xs().text_color(quiet).truncate().child(self.body.clone())),
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
            .border_1()
            .border_dashed()
            .border_color(line)
            .text_sm()
            .aria_label(format!("{}. {}", self.title, self.body))
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
                    .child(div().text_base().font_semibold().text_center().child(self.title.clone()))
                    .child(div().text_center().text_color(quiet).line_height(relative(1.45)).child(self.body.clone())),
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
        let looks: Vec<_> = [EmptyReason::Fresh, EmptyReason::NoMatches, EmptyReason::Failed, EmptyReason::Locked]
            .iter()
            .map(|r| r.look().1)
            .collect();
        assert_eq!(looks, [Tone::Info, Tone::Neutral, Tone::Danger, Tone::Warning]);
    }
}
