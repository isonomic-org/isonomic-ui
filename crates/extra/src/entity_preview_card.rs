// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Entity preview card: a name in running text that previews what it stands for on hover: a customer, an invoice, an assistant.
//!
//! The kit's hover card does the opening and placing. This gives it a
//! consistent inside: a kind mark, the name and a line under it, a status
//! badge, a few label and value rows, and an Open button. The trigger reads
//! as a dotted-underlined link so people know there's more behind it.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, button::Button, h_flex,
    hover_card::HoverCard, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::Tone;

/// What kind of thing a preview is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    /// A person.
    Person,
    /// A company or customer.
    Company,
    /// A document such as an invoice.
    Document,
    /// An assistant.
    Agent,
}

impl EntityKind {
    fn icon(self) -> IconName {
        match self {
            Self::Person => IconName::User,
            Self::Company => IconName::Building2,
            Self::Document => IconName::FileText,
            Self::Agent => IconName::Bot,
        }
    }
}

/// What a preview shows.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityPreview {
    /// What kind of thing it is.
    pub kind: EntityKind,
    /// Its name.
    pub name: SharedString,
    /// A line under the name.
    pub subtitle: SharedString,
    /// A status and its tone, if any.
    pub status: Option<(SharedString, Tone)>,
    /// Label and value rows.
    pub facts: Vec<(SharedString, SharedString)>,
}

impl EntityPreview {
    /// Create a preview.
    pub fn new(
        kind: EntityKind,
        name: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            subtitle: subtitle.into(),
            status: None,
            facts: Vec::new(),
        }
    }

    /// Show a status.
    pub fn status(mut self, label: impl Into<SharedString>, tone: Tone) -> Self {
        self.status = Some((label.into(), tone));
        self
    }

    /// Add a label and value row.
    pub fn fact(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.facts.push((label.into(), value.into()));
        self
    }
}

/// Everything a preview shows, in words: "Harbor Dental, Customer since 2021, Active. Owner: Dana".
pub fn preview_words(preview: &EntityPreview) -> String {
    let mut out = format!("{}, {}", preview.name, preview.subtitle);
    if let Some((status, _)) = &preview.status {
        out.push_str(&format!(", {status}"));
    }
    if !preview.facts.is_empty() {
        let facts = preview
            .facts
            .iter()
            .map(|(l, v)| format!("{l}: {v}"))
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!(". {facts}"));
    }
    out
}

/// A name that previews its entity on hover.
#[derive(IntoElement)]
pub struct EntityPreviewCard {
    id: ElementId,
    label: SharedString,
    preview: EntityPreview,
    on_open: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl EntityPreviewCard {
    /// Create a trigger with its text and the preview behind it.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        preview: EntityPreview,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            preview,
            on_open: None,
        }
    }

    /// Give the preview an Open button.
    pub fn on_open(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for EntityPreviewCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(
                ElementId::NamedChild(self.id.clone().into(), "focus".into()),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let words = preview_words(&self.preview);
        let key_open = self.on_open.clone();
        let theme = cx.theme();
        let accent = theme.primary;
        let ring = theme.ring;
        let preview = self.preview.clone();
        let on_open = self.on_open.clone();
        let button_id = ElementId::NamedChild(self.id.clone().into(), "open".into());
        HoverCard::new(self.id.clone())
            .trigger(
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "trigger".into(),
                    ))
                    .role(Role::Link)
                    .aria_label(self.label.clone())
                    .aria_description(words.clone())
                    .track_focus(&focus)
                    .when(focused, |this| this.rounded(px(2.)).bg(ring.opacity(0.15)))
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "enter"
                            && let Some(open) = &key_open
                        {
                            cx.stop_propagation();
                            open(window, cx);
                        }
                    })
                    .text_color(accent)
                    .border_b_1()
                    .border_dashed()
                    .border_color(accent.opacity(0.6))
                    .cursor_pointer()
                    .child(self.label.clone()),
            )
            .content(move |_, _, cx| {
                let theme = cx.theme();
                let quiet = theme.muted_foreground;
                let chip = theme.muted;
                let status = preview.status.clone().map(|(label, tone)| {
                    div()
                        .px_1p5()
                        .rounded(px(4.))
                        .bg(tone.tint(cx))
                        .text_caption()
                        .text_color(tone.color(cx))
                        .child(label)
                });
                v_flex()
                    .id("preview")
                    .role(Role::Group)
                    .aria_label(words.clone())
                    .w(px(280.))
                    .gap_3()
                    .text_body()
                    .child(
                        h_flex()
                            .gap_2p5()
                            .child(
                                div()
                                    .size(px(34.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.))
                                    .bg(chip)
                                    .child(
                                        Icon::new(preview.kind.icon()).small().text_color(quiet),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div().font_medium().truncate().child(preview.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_caption()
                                            .text_color(quiet)
                                            .truncate()
                                            .child(preview.subtitle.clone()),
                                    ),
                            )
                            .children(status),
                    )
                    .children(preview.facts.iter().map(|(label, value)| {
                        h_flex()
                            .gap_3()
                            .text_caption()
                            .child(
                                div()
                                    .w(px(84.))
                                    .flex_none()
                                    .text_color(quiet)
                                    .child(label.clone()),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(value.clone()))
                    }))
                    .children(on_open.clone().map(|open| {
                        Button::new(button_id.clone())
                            .xsmall()
                            .outline()
                            .label(format!("Open {}", preview.name))
                            .on_click(move |_, window, cx| open(window, cx))
                    }))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_collect_facts() {
        let p = EntityPreview::new(EntityKind::Company, "Northwind", "Dental clinic")
            .status("Overdue", Tone::Warning)
            .fact("Owes", "$3,120.00")
            .fact("Since", "2019");
        assert_eq!(p.facts.len(), 2);
        assert_eq!(p.status.as_ref().map(|s| s.0.as_ref()), Some("Overdue"));
    }
}
