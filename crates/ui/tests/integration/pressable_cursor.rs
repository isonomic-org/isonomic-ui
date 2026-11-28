// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Every pressable control shows the pointing hand while it can be pressed,
//! and the arrow once it is disabled.

use crate::common;
use isonomic_ui::component::{
    Disableable, IndexPath,
    accordion::Accordion,
    button::{Button, ButtonVariants as _, Toggle},
    checkbox::Checkbox,
    link::Link,
    radio::Radio,
    select::{SearchableVec, Select, SelectState},
    sidebar::{Sidebar, SidebarMenu, SidebarMenuItem},
    switch::Switch,
    tab::{Tab, TabBar},
};
use isonomic_ui::test::{TestQueryExt, TestWindowExt};
use isonomic_ui::{
    Context, CursorStyle, Entity, TestAppContext, Window, div, prelude::*, px, size,
};

struct Controls {
    language: Entity<SelectState<SearchableVec<&'static str>>>,
    locked_language: Entity<SelectState<SearchableVec<&'static str>>>,
}

impl Render for Controls {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(Button::new("save").primary().label("Save"))
            .child(Button::new("more").ghost().label("More"))
            .child(
                Button::new("delete")
                    .danger()
                    .label("Delete")
                    .disabled(true),
            )
            .child(Button::new("saving").label("Saving").loading(true))
            .child(
                Link::new("learn-more")
                    .href("https://example.com")
                    .child("Learn more"),
            )
            .child(
                Link::new("learn-more-off")
                    .href("https://example.com")
                    .disabled(true)
                    .child("Unavailable"),
            )
            .child(Checkbox::new("agree").label("Agree"))
            .child(Checkbox::new("locked").label("Locked").disabled(true))
            .child(Switch::new("wifi"))
            .child(Switch::new("airplane").disabled(true))
            .child(Radio::new("alpha").label("Alpha"))
            .child(Radio::new("beta").label("Beta").disabled(true))
            .child(Toggle::new("bold").label("Bold"))
            .child(Toggle::new("italic").label("Italic").disabled(true))
            .child(
                TabBar::new("sections")
                    .segmented()
                    .selected_index(0)
                    .children([
                        Tab::new().label("General"),
                        Tab::new().label("Advanced").disabled(true),
                    ]),
            )
            .child(
                Accordion::new("faq")
                    .item(|item| item.title("Shipping"))
                    .item(|item| item.title("Billing").disabled(true)),
            )
            .child(
                div().w(px(240.)).h(px(160.)).child(
                    Sidebar::new("nav").child(
                        SidebarMenu::new()
                            .child(SidebarMenuItem::new("Inbox"))
                            .child(SidebarMenuItem::new("Archive").disable(true)),
                    ),
                ),
            )
            .child(Select::new(&self.language).id("language").w(px(200.)))
            .child(
                Select::new(&self.locked_language)
                    .id("locked-language")
                    .disabled(true)
                    .w(px(200.)),
            )
    }
}

fn open(cx: &mut TestAppContext) -> isonomic_ui::WindowHandle<isonomic_ui::base::Root> {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(640.), px(1200.))), |window, cx| {
        let languages = || SearchableVec::new(vec!["Rust", "Go"]);
        cx.new(|cx| Controls {
            language: cx
                .new(|cx| SelectState::new(languages(), Some(IndexPath::new(0)), window, cx)),
            locked_language: cx
                .new(|cx| SelectState::new(languages(), Some(IndexPath::new(0)), window, cx)),
        })
    });
    handle
}

/// The cursor an observed element shows on hover.
fn cursor_by_label(window: &Window, label: &str) -> Option<CursorStyle> {
    window.find_by_label(label).cursor()
}

#[isonomic_ui::test]
fn buttons_show_the_pointer_only_while_pressable(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            cursor_by_label(window, "Save"),
            Some(CursorStyle::PointingHand)
        );
        assert_eq!(
            cursor_by_label(window, "More"),
            Some(CursorStyle::PointingHand)
        );
        assert_eq!(cursor_by_label(window, "Delete"), Some(CursorStyle::Arrow));
        assert_eq!(cursor_by_label(window, "Saving"), Some(CursorStyle::Arrow));
    })
    .unwrap();
}

#[isonomic_ui::test]
fn links_show_the_pointer_only_while_enabled(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("learn-more").shows_pointer());
        assert_eq!(
            window.find("learn-more-off").cursor(),
            Some(CursorStyle::Arrow)
        );
    })
    .unwrap();
}

#[isonomic_ui::test]
fn choice_controls_show_the_pointer_only_while_enabled(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        for enabled in ["agree", "wifi", "alpha", "bold"] {
            assert!(
                window.find(enabled).shows_pointer(),
                "{enabled} shows the pointing hand"
            );
        }
        for disabled in ["locked", "airplane", "beta", "italic"] {
            assert_eq!(
                window.find(disabled).cursor(),
                Some(CursorStyle::Arrow),
                "{disabled} shows the arrow"
            );
        }
    })
    .unwrap();
}

#[isonomic_ui::test]
fn tabs_and_accordion_headers_show_the_pointer_only_while_enabled(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.within("sections").find(0usize).shows_pointer());
        assert_eq!(
            window.within("sections").find(1usize).cursor(),
            Some(CursorStyle::Arrow)
        );
        assert!(window.find_by_label("Shipping").shows_pointer());
        assert_eq!(cursor_by_label(window, "Billing"), Some(CursorStyle::Arrow));
    })
    .unwrap();
}

#[isonomic_ui::test]
fn select_triggers_show_the_pointer_only_while_enabled(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // The trigger is the select's input-styled box inside the root.
        assert!(window.within("language").find("input").shows_pointer());
        assert_eq!(
            window.within("locked-language").find("input").cursor(),
            Some(CursorStyle::Arrow)
        );
    })
    .unwrap();
}

#[isonomic_ui::test]
fn sidebar_rows_show_the_pointer_only_while_enabled(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let rows = |label| {
            window
                .find_all_by_label(label)
                .into_iter()
                .filter_map(|row| row.cursor())
                .collect::<Vec<_>>()
        };
        assert_eq!(rows("Inbox"), vec![CursorStyle::PointingHand]);
        assert_eq!(rows("Archive"), vec![CursorStyle::Arrow]);
    })
    .unwrap();
}
