// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Path breadcrumb: where you are as a trail of places, folding the middle into a menu when it's long.
//!
//! The first place and the last few always show; the ones between fold into
//! a "…" button whose menu lists them. Every place but the current one is a
//! link. A copy button puts the whole path on the clipboard as text.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use isonomic_ui::{
    App, ClipboardItem, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

/// Which places show and which fold: indexes shown, with `None` where the fold goes.
pub fn trail_layout(count: usize, keep_last: usize) -> Vec<Option<usize>> {
    let keep_last = keep_last.max(1);
    if count <= keep_last + 2 {
        return (0..count).map(Some).collect();
    }
    let mut out = vec![Some(0), None];
    out.extend((count - keep_last..count).map(Some));
    out
}

type GoHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A trail of places.
#[derive(IntoElement)]
pub struct PathBreadcrumb {
    id: ElementId,
    style: StyleRefinement,
    places: Vec<SharedString>,
    keep_last: usize,
    separator: SharedString,
    on_go: Option<GoHandler>,
}

impl PathBreadcrumb {
    /// Create a trail from the first place to the current one.
    pub fn new(id: impl Into<ElementId>, places: Vec<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            places,
            keep_last: 2,
            separator: "/".into(),
            on_go: None,
        }
    }

    /// Keep this many places at the end before folding. Two by default.
    pub fn keep_last(mut self, count: usize) -> Self {
        self.keep_last = count.max(1);
        self
    }

    /// Set the text the copy button joins places with.
    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Run with the index of the place clicked.
    pub fn on_go(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_go = Some(Rc::new(handler));
        self
    }
}

impl Styled for PathBreadcrumb {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for PathBreadcrumb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let chip = theme.muted;
        let count = self.places.len();
        let layout = trail_layout(count, self.keep_last);
        let folded: Vec<usize> = if layout.contains(&None) {
            (1..count - self.keep_last).collect()
        } else {
            Vec::new()
        };
        let joined = self
            .places
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(&format!(" {} ", self.separator));

        let mut items: Vec<isonomic_ui::AnyElement> = Vec::new();
        for (n, slot) in layout.iter().enumerate() {
            if n > 0 {
                items.push(
                    Icon::new(IconName::ChevronRight)
                        .xsmall()
                        .text_color(quiet)
                        .into_any_element(),
                );
            }
            match slot {
                Some(ix) => {
                    let ix = *ix;
                    let current = ix + 1 == count;
                    let go = self.on_go.clone();
                    let (fade, on_hover) = look::tracked_hover(
                        ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("place-hover-{ix}").into(),
                        ),
                        !current,
                        window,
                        cx,
                    );
                    items.push(
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("place-{ix}").into(),
                            ))
                            .max_w(px(180.))
                            .truncate()
                            .px_1()
                            .rounded(px(4.))
                            .when(current, |this| this.text_color(ink).font_medium())
                            .when(!current, |this| {
                                this.text_color(look::mix(quiet, ink, fade))
                                    .bg(crate::common::faded(chip, fade))
                                    .cursor_pointer()
                                    .on_hover(on_hover)
                                    .on_click(move |_, window, cx| {
                                        if let Some(go) = &go {
                                            go(ix, window, cx);
                                        }
                                    })
                            })
                            .role(Role::Link)
                            .aria_label(if current {
                                format!("{}, current", self.places[ix])
                            } else {
                                self.places[ix].to_string()
                            })
                            .child(self.places[ix].clone())
                            .into_any_element(),
                    );
                }
                None => {
                    let go = self.on_go.clone();
                    let entries: Vec<(usize, SharedString)> = folded
                        .iter()
                        .map(|ix| (*ix, self.places[*ix].clone()))
                        .collect();
                    items.push(
                        Button::new(ElementId::NamedChild(self.id.clone().into(), "fold".into()))
                            .xsmall()
                            .ghost()
                            .label("\u{2026}")
                            .accessibility_label(format!("{} more places", entries.len()))
                            .dropdown_menu(move |menu, _, _| {
                                entries.iter().fold(menu, |menu, (ix, name)| {
                                    let go = go.clone();
                                    let ix = *ix;
                                    menu.item(PopupMenuItem::new(name.clone()).on_click(
                                        move |_, window, cx| {
                                            if let Some(go) = &go {
                                                go(ix, window, cx);
                                            }
                                        },
                                    ))
                                })
                            })
                            .into_any_element(),
                    );
                }
            }
        }

        h_flex()
            .id(self.id.clone())
            .gap_0p5()
            .min_w_0()
            .text_body()
            .role(Role::Navigation)
            .aria_label(format!("Path: {joined}"))
            .children(items)
            .child(
                Button::new(ElementId::NamedChild(self.id.clone().into(), "copy".into()))
                    .xsmall()
                    .ghost()
                    .icon(IconName::Copy)
                    .accessibility_label("Copy the path")
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(joined.clone()))
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_trails_fold_the_middle() {
        assert_eq!(trail_layout(3, 2), [Some(0), Some(1), Some(2)]);
        assert_eq!(trail_layout(4, 2), [Some(0), Some(1), Some(2), Some(3)]);
        assert_eq!(trail_layout(6, 2), [Some(0), None, Some(4), Some(5)]);
    }
}
