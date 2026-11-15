//! Explained tooltip: a hover note on any element with a title, a line or two of explanation and its keyboard shortcut, or the reason a control is off.
//!
//! Wrap a control and give it a title; add a body for what it does and a
//! shortcut to show as key caps. When the control is turned off, pass the
//! reason instead and the note leads with it, so people learn why rather
//! than guessing. Long notes wrap at a readable width.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, tooltip::Tooltip,
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::shortcut_keys::ShortcutKeys;

/// How wide the note grows for a body of `chars` characters: snug for short notes, capped for long ones.
pub fn note_width(chars: usize) -> Pixels {
    px((120.0 + chars as f32 * 3.2).clamp(160.0, 300.0))
}

/// Everything the note says, in reading order, for a screen reader.
pub fn spoken_note(title: &str, body: Option<&str>, off_because: Option<&str>) -> String {
    let mut parts = vec![title.to_string()];
    if let Some(reason) = off_because {
        parts.push(format!("Unavailable: {reason}"));
    }
    if let Some(body) = body {
        parts.push(body.to_string());
    }
    parts.join(". ")
}

/// An element with an explaining hover note.
#[derive(IntoElement)]
pub struct ExplainedTooltip {
    id: ElementId,
    style: StyleRefinement,
    child: AnyElement,
    title: SharedString,
    body: Option<SharedString>,
    shortcut: Option<SharedString>,
    off_because: Option<SharedString>,
}

impl ExplainedTooltip {
    /// Wrap `child` with a note titled `title`.
    pub fn new(
        id: impl Into<ElementId>,
        child: impl IntoElement,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            child: child.into_any_element(),
            title: title.into(),
            body: None,
            shortcut: None,
            off_because: None,
        }
    }

    /// Explain what it does.
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Show a shortcut, written as for the shortcut keys, such as "mod-shift-e".
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Say why the control is off.
    pub fn off_because(mut self, reason: impl Into<SharedString>) -> Self {
        self.off_because = Some(reason.into());
        self
    }
}

impl Styled for ExplainedTooltip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ExplainedTooltip {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let title = self.title.clone();
        let body = self.body.clone();
        let shortcut = self.shortcut.clone();
        let reason = self.off_because.clone();
        let length = body.as_ref().map_or(0, |b| b.len()) + reason.as_ref().map_or(0, |r| r.len());
        let width = note_width(length);
        let shortcut_id = ElementId::NamedChild(self.id.clone().into(), "keys".into());

        div()
            .id(self.id.clone())
            .flex_none()
            .tooltip(move |window, cx| {
                let title = title.clone();
                let body = body.clone();
                let shortcut = shortcut.clone();
                let reason = reason.clone();
                let shortcut_id = shortcut_id.clone();
                Tooltip::element(move |_, cx| {
                    let quiet = cx.theme().muted_foreground;
                    let warning = cx.theme().warning;
                    v_flex()
                        .max_w(width)
                        .gap_1()
                        .py_0p5()
                        .child(
                            h_flex()
                                .gap_3()
                                .justify_between()
                                .child(div().text_sm().font_medium().child(title.clone()))
                                .when_some(shortcut.clone(), |this, keys| {
                                    this.child(ShortcutKeys::new(shortcut_id.clone(), keys))
                                }),
                        )
                        .when_some(reason.clone(), |this, reason| {
                            this.child(
                                h_flex()
                                    .items_start()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(warning)
                                    .child(Icon::new(IconName::Info).xsmall())
                                    .child(div().flex_1().child(reason)),
                            )
                        })
                        .when_some(body.clone(), |this, body| {
                            this.child(div().text_xs().text_color(quiet).child(body))
                        })
                })
                .build(window, cx)
            })
            .role(Role::Group)
            .aria_label(spoken_note(
                &self.title,
                self.body.as_deref(),
                self.off_because.as_deref(),
            ))
            .child(self.child)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_size_and_read_in_order() {
        assert_eq!(note_width(0), px(160.));
        assert_eq!(note_width(50), px(280.));
        assert_eq!(note_width(400), px(300.));
        assert_eq!(spoken_note("Export", None, None), "Export");
        assert_eq!(
            spoken_note(
                "Close the month",
                Some("Locks every entry before 1 Dec."),
                Some("2 bank feeds are unreconciled")
            ),
            "Close the month. Unavailable: 2 bank feeds are unreconciled. Locks every entry before 1 Dec."
        );
    }
}
