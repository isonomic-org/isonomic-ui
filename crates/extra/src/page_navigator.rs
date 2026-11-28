// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page navigator: moves through a long list a page at a time, showing which rows are on screen and the pages around the current one.
//!
//! The first and last pages always show, with the current page and its
//! neighbours between them. A gap of one page shows that page; a longer gap
//! shows "…". Previous and next turn off at the ends, and an optional size
//! menu changes how many rows a page holds.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use isonomic_ui::{
    App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
};

/// How many pages hold `total` rows, never fewer than one.
pub fn page_count(total: usize, per_page: usize) -> usize {
    total.div_ceil(per_page.max(1)).max(1)
}

/// Pages to show around `current`, with `None` where a gap folds.
pub fn page_window(current: usize, pages: usize, siblings: usize) -> Vec<Option<usize>> {
    let pages = pages.max(1);
    let current = current.clamp(1, pages);
    let mut shown = vec![1, pages];
    shown.extend(current.saturating_sub(siblings).max(1)..=(current + siblings).min(pages));
    shown.sort_unstable();
    shown.dedup();
    let mut out = Vec::new();
    let mut prev = 0;
    for page in shown {
        if prev != 0 && page > prev + 1 {
            out.push(if page == prev + 2 {
                Some(prev + 1)
            } else {
                None
            });
        }
        out.push(Some(page));
        prev = page;
    }
    out
}

/// Which rows a page holds, such as "21–40 of 312".
pub fn range_label(page: usize, per_page: usize, total: usize) -> String {
    if total == 0 {
        return "No rows".into();
    }
    let per_page = per_page.max(1);
    let page = page.clamp(1, page_count(total, per_page));
    let first = (page - 1) * per_page + 1;
    let last = (page * per_page).min(total);
    format!("{first}\u{2013}{last} of {total}")
}

type PickHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Moves through pages of a list.
#[derive(IntoElement)]
pub struct PageNavigator {
    id: ElementId,
    style: StyleRefinement,
    page: usize,
    total: usize,
    per_page: usize,
    sizes: Vec<usize>,
    siblings: usize,
    on_page: Option<PickHandler>,
    on_per_page: Option<PickHandler>,
}

impl PageNavigator {
    /// Create a navigator on `page` (from 1) of a list with `total` rows.
    pub fn new(id: impl Into<ElementId>, page: usize, total: usize) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            page,
            total,
            per_page: 20,
            sizes: Vec::new(),
            siblings: 1,
            on_page: None,
            on_per_page: None,
        }
    }

    /// Set how many rows a page holds. Twenty by default.
    pub fn per_page(mut self, per_page: usize) -> Self {
        self.per_page = per_page.max(1);
        self
    }

    /// Offer these page sizes in a menu.
    pub fn sizes(mut self, sizes: impl IntoIterator<Item = usize>) -> Self {
        self.sizes = sizes.into_iter().filter(|s| *s > 0).collect();
        self
    }

    /// Show this many pages on each side of the current one. One by default.
    pub fn siblings(mut self, siblings: usize) -> Self {
        self.siblings = siblings;
        self
    }

    /// Run with the page picked.
    pub fn on_page(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_page = Some(Rc::new(handler));
        self
    }

    /// Run with the page size picked.
    pub fn on_per_page(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_per_page = Some(Rc::new(handler));
        self
    }
}

impl Styled for PageNavigator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn go(
    handler: &Option<PickHandler>,
    page: usize,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |_, window, cx| {
        if let Some(handler) = &handler {
            handler(page, window, cx);
        }
    }
}

impl RenderOnce for PageNavigator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let pages = page_count(self.total, self.per_page);
        let page = self.page.clamp(1, pages);
        let child = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());

        let mut buttons: Vec<isonomic_ui::AnyElement> = Vec::new();
        buttons.push(
            Button::new(child("prev".into()))
                .xsmall()
                .ghost()
                .icon(IconName::ChevronLeft)
                .accessibility_label("Previous page")
                .disabled(page == 1)
                .on_click(go(&self.on_page, page.saturating_sub(1).max(1)))
                .into_any_element(),
        );
        for (n, slot) in page_window(page, pages, self.siblings)
            .into_iter()
            .enumerate()
        {
            match slot {
                Some(p) => {
                    let button = Button::new(child(format!("page-{p}")))
                        .xsmall()
                        .label(p.to_string());
                    let button = if p == page {
                        button
                            .primary()
                            .accessibility_label(format!("Page {p}, current"))
                    } else {
                        button
                            .ghost()
                            .accessibility_label(format!("Page {p}"))
                            .on_click(go(&self.on_page, p))
                    };
                    buttons.push(button.into_any_element());
                }
                None => buttons.push(
                    div()
                        .id(child(format!("gap-{n}")))
                        .px_1()
                        .text_body()
                        .text_color(quiet)
                        .role(Role::Label)
                        .aria_label("More pages")
                        .child("\u{2026}")
                        .into_any_element(),
                ),
            }
        }
        buttons.push(
            Button::new(child("next".into()))
                .xsmall()
                .ghost()
                .icon(IconName::ChevronRight)
                .accessibility_label("Next page")
                .disabled(page == pages)
                .on_click(go(&self.on_page, (page + 1).min(pages)))
                .into_any_element(),
        );

        let size_menu = (!self.sizes.is_empty()).then(|| {
            let sizes = self.sizes.clone();
            let current = self.per_page;
            let pick = self.on_per_page.clone();
            Button::new(child("size".into()))
                .xsmall()
                .ghost()
                .label(format!("{current} per page"))
                .accessibility_label(format!("Rows per page: {current}"))
                .dropdown_menu(move |menu, _, _| {
                    sizes.iter().fold(menu, |menu, size| {
                        let pick = pick.clone();
                        let size = *size;
                        menu.item(
                            PopupMenuItem::new(format!("{size} per page"))
                                .checked(size == current)
                                .on_click(move |_, window, cx| {
                                    if let Some(pick) = &pick {
                                        pick(size, window, cx);
                                    }
                                }),
                        )
                    })
                })
        });

        h_flex()
            .id(self.id.clone())
            .gap_3()
            .justify_between()
            .flex_wrap()
            .role(Role::Navigation)
            .aria_label(format!("Pages, on page {page} of {pages}"))
            .child(
                h_flex()
                    .gap_2()
                    .text_body()
                    .text_color(quiet)
                    .child(range_label(page, self.per_page, self.total))
                    .children(size_menu),
            )
            .child(h_flex().gap_0p5().children(buttons))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_fold_long_gaps_only() {
        assert_eq!(page_window(1, 1, 1), [Some(1)]);
        assert_eq!(
            page_window(5, 10, 1),
            [Some(1), None, Some(4), Some(5), Some(6), None, Some(10)]
        );
        assert_eq!(
            page_window(4, 10, 1),
            [Some(1), Some(2), Some(3), Some(4), Some(5), None, Some(10)]
        );
        assert_eq!(page_window(99, 3, 1), [Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn ranges_cover_the_last_partial_page() {
        assert_eq!(page_count(312, 20), 16);
        assert_eq!(page_count(0, 20), 1);
        assert_eq!(range_label(2, 20, 312), "21\u{2013}40 of 312");
        assert_eq!(range_label(16, 20, 312), "301\u{2013}312 of 312");
        assert_eq!(range_label(1, 20, 0), "No rows");
    }
}
