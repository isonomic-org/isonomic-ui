// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Status badge: a labelled pill in a tone, with an optional live dot, a capped count and a remove button.
//!
//! The kit's badge marks a count or dot on something else; this one stands
//! on its own as a word: "Running", "Needs review", "Paid". A live dot
//! pulses while something is in progress. A count is capped ("99+"), and a
//! removable badge has a named close button, for filters and tags.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::Tone;

/// Write a count with a cap: 7, 99, "99+".
pub fn capped(count: u32, max: u32) -> String {
    if count > max {
        format!("{max}+")
    } else {
        count.to_string()
    }
}

/// A labelled status pill.
#[derive(IntoElement)]
pub struct StatusBadge {
    id: ElementId,
    style: StyleRefinement,
    label: SharedString,
    tone: Tone,
    live: bool,
    count: Option<u32>,
    max: u32,
    solid: bool,
    on_remove: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl StatusBadge {
    /// Create a neutral badge.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            label: label.into(),
            tone: Tone::Neutral,
            live: false,
            count: None,
            max: 99,
            solid: false,
            on_remove: None,
        }
    }

    /// Set its tone.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Show a pulsing dot for something in progress.
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    /// Show a count, capped at `max`.
    pub fn count(mut self, count: u32, max: u32) -> Self {
        self.count = Some(count);
        self.max = max.max(1);
        self
    }

    /// Fill it with the tone instead of tinting it.
    pub fn solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    /// Give it a remove button.
    pub fn on_remove(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

impl Styled for StatusBadge {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let remove_focus = self.on_remove.is_some().then(|| {
            window
                .use_keyed_state(
                    ElementId::NamedChild(self.id.clone().into(), "remove-focus".into()),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone()
        });
        let theme = cx.theme();
        let ring = theme.ring;
        let color = match self.tone {
            Tone::Neutral => theme.foreground,
            tone => tone.color(cx),
        };
        let (bg, fg) = if self.solid {
            (color, theme.background)
        } else {
            (self.tone.tint(cx), color)
        };
        let label = self.label.clone();
        h_flex()
            .id(self.id.clone())
            .flex_none()
            .gap_1()
            .h(px(20.))
            .px_2()
            .when(self.on_remove.is_some(), |this| this.pr_0p5())
            .rounded_full()
            .bg(bg)
            .text_color(fg)
            .text_label_small()
            .role(Role::Status)
            .aria_label(match self.count {
                Some(n) => format!(
                    "{}, {n}{}",
                    self.label,
                    if self.live { ", in progress" } else { "" }
                ),
                None => format!(
                    "{}{}",
                    self.label,
                    if self.live { ", in progress" } else { "" }
                ),
            })
            .when(self.live, |this| {
                this.child(div().size(px(6.)).rounded_full().bg(fg).with_animation(
                    ElementId::NamedChild(self.id.clone().into(), "pulse".into()),
                    Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                    |dot, delta| dot.opacity(0.35 + 0.65 * (1. - (delta * 2. - 1.).abs())),
                ))
            })
            .child(label)
            .children(self.count.map(|n| {
                div()
                    .px_1()
                    .rounded_full()
                    .bg(fg.opacity(0.15))
                    .child(capped(n, self.max))
            }))
            .children(self.on_remove.zip(remove_focus).map(|(remove, focus)| {
                let on_key = remove.clone();
                let focused = focus.is_focused(window);
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "remove".into(),
                    ))
                    .role(Role::Button)
                    .track_focus(&focus)
                    .when(focused, |this| this.border_1().border_color(ring))
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        // Enter and Space already click a focused element.
                        if matches!(event.keystroke.key.as_str(), "backspace" | "delete") {
                            cx.stop_propagation();
                            on_key(window, cx);
                        }
                    })
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(fg.opacity(0.15)))
                    .aria_label(format!("Remove {}", self.label))
                    .on_click(move |_, window, cx| remove(window, cx))
                    .child(Icon::new(IconName::Close).xsmall())
            }))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_capped() {
        assert_eq!(capped(7, 99), "7");
        assert_eq!(capped(99, 99), "99");
        assert_eq!(capped(140, 99), "99+");
    }
}
