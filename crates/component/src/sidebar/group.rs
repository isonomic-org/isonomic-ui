// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{ActiveTheme, Collapsible, Icon, IconName, h_flex, sidebar::SidebarItem, v_flex};
use gpui::{
    App, Edges, ElementId, InteractiveElement as _, IntoElement, ParentElement, Pixels, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use isonomic_ui_base::{LogicalStyledExt as _, PressableExt as _};

/// A group of items in the [`super::Sidebar`].
#[derive(Clone)]
pub struct SidebarGroup<E: SidebarItem + 'static> {
    label: SharedString,
    collapsed: bool,
    children: Vec<E>,
    padding: Option<Edges<Pixels>>,
    foldable: bool,
    default_open: bool,
}

impl<E: SidebarItem> SidebarGroup<E> {
    /// Create a new [`SidebarGroup`] with the given label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            collapsed: false,
            children: Vec::new(),
            padding: None,
            foldable: false,
            default_open: true,
        }
    }

    /// Add a child to the sidebar group, the child should implement [`SidebarItem`].
    pub fn child(mut self, child: E) -> Self {
        self.children.push(child);
        self
    }

    /// Add multiple children to the sidebar group.
    ///
    /// See also [`SidebarGroup::child`].
    pub fn children(mut self, children: impl IntoIterator<Item = E>) -> Self {
        self.children.extend(children);
        self
    }

    /// Set the padding around the group. The default is none.
    pub fn padding(mut self, padding: Edges<Pixels>) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Let the group fold away under its label. Off by default.
    ///
    /// The label becomes a button with a chevron that shows or hides the
    /// group's children. It is a Tab stop, and Enter and Space press it.
    pub fn foldable(mut self, foldable: bool) -> Self {
        self.foldable = foldable;
        self
    }

    /// Whether a foldable group starts open. The default is open.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }
}

impl<E: SidebarItem> Collapsible for SidebarGroup<E> {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl<E: SidebarItem> SidebarItem for SidebarGroup<E> {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();
        let default_open = self.default_open;
        let open_state = self.foldable.then(|| {
            window.use_keyed_state(
                SharedString::from(format!("{id}-group-open")),
                cx,
                |_, _| default_open,
            )
        });
        // Collapsed to icons there is no label to press, so the children show.
        let is_open = self.collapsed || open_state.as_ref().is_none_or(|open| *open.read(cx));
        let ring = cx.theme().ring;
        let label = self.label.clone();

        v_flex()
            .relative()
            .when_some(self.padding.filter(|_| !self.collapsed), |this, edges| {
                this.pt(edges.top)
                    .pr(edges.right)
                    .pb(edges.bottom)
                    .pl(edges.left)
            })
            .when(!self.collapsed, |this| {
                this.child(
                    h_flex()
                        .id(SharedString::from(format!("{id}-group-label")))
                        .flex_row_dir(cx)
                        .flex_shrink_0()
                        .px_2()
                        .rounded(cx.theme().radius)
                        .text_xs()
                        .text_color(cx.theme().sidebar_foreground.opacity(0.7))
                        .h_8()
                        .when_some(open_state, |this, open_state| {
                            this.gap_1()
                                .pressable(true)
                                .role(Role::Button)
                                .aria_label(label)
                                .aria_expanded(is_open)
                                .tab_index(0)
                                .focus_visible(move |style| style.border_1().border_color(ring))
                                .on_key_down(super::menu::arrow_keys_move_focus)
                                .on_click(move |_, _, cx| {
                                    open_state.update(cx, |open, cx| {
                                        *open = !*open;
                                        cx.notify();
                                    })
                                })
                                .child(
                                    Icon::new(if is_open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .size_3(),
                                )
                        })
                        .child(self.label),
                )
            })
            .when(is_open, |this| {
                this.child(div().gap_2().flex_col().children(
                    self.children.into_iter().enumerate().map(|(ix, child)| {
                        child
                            .collapsed(self.collapsed)
                            .render(format!("{}-{}", id, ix), window, cx)
                            .into_any_element()
                    }),
                ))
            })
    }
}
