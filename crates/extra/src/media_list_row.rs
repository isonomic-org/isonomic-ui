// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Media list row: one compact row with a small picture or icon, a title over a quieter line, and an optional control at the end, for lists of files, connections or settings.
//!
//! The title and the line under it each end in an ellipsis when space runs
//! out, and screen readers hear both in full. A row can be clicked, can be
//! the picked one, or can be turned off; a clickable row is a Tab stop that
//! Enter or Space opens. The compact form drops the second line's spacing
//! for dense lists.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, img, prelude::FluentBuilder as _, px,
};

/// What leads the row.
#[derive(Clone)]
pub enum RowMedia {
    /// An icon.
    Icon(IconName),
    /// A picture, by path or address.
    Image(SharedString),
}

/// What a screen reader hears for a row, such as "Q3 report.pdf, 2.4 MB, picked".
pub fn spoken_row(title: &str, secondary: Option<&str>, picked: bool, disabled: bool) -> String {
    let mut out = title.to_string();
    if let Some(line) = secondary.filter(|l| !l.is_empty()) {
        out.push_str(", ");
        out.push_str(line);
    }
    if picked {
        out.push_str(", picked");
    }
    if disabled {
        out.push_str(", unavailable");
    }
    out
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One compact row.
#[derive(IntoElement)]
pub struct MediaListRow {
    id: ElementId,
    style: StyleRefinement,
    media: Option<RowMedia>,
    title: SharedString,
    secondary: Option<SharedString>,
    trailing: Option<AnyElement>,
    picked: bool,
    disabled: bool,
    compact: bool,
    on_click: Option<ClickHandler>,
}

impl MediaListRow {
    /// Create a row with a title.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            media: None,
            title: title.into(),
            secondary: None,
            trailing: None,
            picked: false,
            disabled: false,
            compact: false,
            on_click: None,
        }
    }

    /// Lead with an icon or a picture.
    pub fn media(mut self, media: RowMedia) -> Self {
        self.media = Some(media);
        self
    }

    /// Add a quieter line under the title.
    pub fn secondary(mut self, line: impl Into<SharedString>) -> Self {
        self.secondary = Some(line.into());
        self
    }

    /// End the row with a control, such as a remove button, a badge or a menu.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Mark it as the picked row.
    pub fn picked(mut self, picked: bool) -> Self {
        self.picked = picked;
        self
    }

    /// Turn it off.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Use the dense layout.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Run when the row is clicked, or Enter or Space is pressed on it.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for MediaListRow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MediaListRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let clickable = self.on_click.is_some() && !self.disabled;
        let focus = clickable.then(|| {
            window
                .use_keyed_state(
                    ElementId::NamedChild(self.id.clone().into(), "focus".into()),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone()
        });
        let focused = focus.as_ref().is_some_and(|f| f.is_focused(window));
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let picked_bg = theme.accent;
        let hover = theme.muted.opacity(0.6);
        let ring = theme.ring;
        let radius = theme.radius;
        let spoken = spoken_row(
            &self.title,
            self.secondary.as_deref(),
            self.picked,
            self.disabled,
        );
        let click = self.on_click.clone();

        let media = self.media.map(|media| match media {
            RowMedia::Icon(icon) => Icon::new(icon).small().text_color(quiet).into_any_element(),
            RowMedia::Image(src) => img(src).size(px(20.)).rounded(px(4.)).into_any_element(),
        });

        h_flex()
            .id(self.id.clone())
            .role(Role::ListItem)
            .aria_label(spoken)
            .aria_selected(self.picked)
            .w_full()
            .min_w_0()
            .gap_2()
            .px_2()
            .py(px(if self.compact { 4. } else { 6. }))
            .rounded(radius)
            .border_1()
            .border_color(if focused {
                ring
            } else {
                isonomic_ui::transparent_black()
            })
            .when(self.picked, |this| this.bg(picked_bg))
            .when(self.disabled, |this| this.opacity(0.5))
            .when_some(focus, |this, focus| this.track_focus(&focus))
            .when(clickable, |this| {
                this.cursor_pointer()
                    .when(!self.picked, |this| this.hover(move |s| s.bg(hover)))
                    .on_click(move |event, window, cx| {
                        if let Some(click) = &click {
                            click(event, window, cx);
                        }
                    })
            })
            .children(media.map(|m| {
                div()
                    .size(px(20.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(m)
            }))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(if self.compact { 0. } else { 1. }))
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_label()
                            .child(self.title.clone()),
                    )
                    .when_some(self.secondary, |this, line| {
                        this.child(
                            div()
                                .w_full()
                                .truncate()
                                .text_caption()
                                .text_color(quiet)
                                .child(line),
                        )
                    }),
            )
            .children(self.trailing.map(|t| div().flex_none().child(t)))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_read_in_full() {
        assert_eq!(
            spoken_row("Q3 report.pdf", Some("2.4 MB"), true, false),
            "Q3 report.pdf, 2.4 MB, picked"
        );
        assert_eq!(
            spoken_row("Bank feed", None, false, true),
            "Bank feed, unavailable"
        );
        assert_eq!(spoken_row("Mail", Some(""), false, false), "Mail");
        let row = MediaListRow::new("r", "Mail")
            .media(RowMedia::Image("mail.png".into()))
            .compact();
        assert!(row.compact && row.media.is_some());
    }
}
