// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::common;
use isonomic_ui::base::test_support::{snapshots, texts};
use isonomic_ui::test::TestWindowExt;
use isonomic_ui::{
    AppContext, Context, Keystroke, TestAppContext, Window, div, prelude::*, px, size,
};
use isonomic_ui_component::{
    avatar::Avatar, button::Button, kbd::Kbd, label::Label, text::TextView,
};

struct Painted;
impl Render for Painted {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("painted")
            .flex()
            .flex_col()
            .child(Label::new("Hello"))
            .child(Kbd::new(Keystroke::parse("ctrl-s").unwrap()))
            .child(Avatar::new().name("Jane Doe"))
            .child(
                Button::new("save")
                    .label("Save")
                    .tooltip("Write the file to disk"),
            )
            .child(TextView::markdown("doc", "A painted paragraph."))
    }
}

#[isonomic_ui::test]
fn painted_text_and_alternative_text_are_observed(cx: &mut TestAppContext) {
    cx.update(isonomic_ui_component::init);
    let (handle, _) = common::open_window(cx, Some(size(px(600.), px(500.))), |_, cx| {
        cx.new(|_| Painted)
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let texts = texts(window);
        let find = |text: &str| {
            texts
                .iter()
                .find(|snapshot| snapshot.text() == text)
                .unwrap_or_else(|| panic!("{text:?} was not painted: {texts:?}"))
        };

        let label = find("Hello");
        assert!(label.visible());
        assert!(!label.is_alternative());
        assert!(label.path().contains(&"painted".into()));
        find(&Kbd::format(&Keystroke::parse("ctrl-s").unwrap()));
        find("A painted paragraph.");
        assert!(find("Jane Doe").is_alternative());

        let save = window.find("save");
        assert_eq!(save.description(), Some("Write the file to disk"));
    })
    .unwrap();
}

#[isonomic_ui::test]
fn each_frame_replaces_the_text_of_the_last(cx: &mut TestAppContext) {
    cx.update(isonomic_ui_component::init);
    let (handle, _) = common::open_window(cx, Some(size(px(600.), px(500.))), |_, cx| {
        cx.new(|_| Painted)
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        // GPUI Fast draws an unchanged Root again from its last frame without
        // rendering it, so it would not start the frame. Like a cached view,
        // a retained one records no text.
        #[cfg(feature = "gpui-fast")]
        window.set_view_retention(false);
        window.draw(cx).clear(cx);
        let first = texts(window).len();
        assert!(first > 0);
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(texts(window).len(), first);
    })
    .unwrap();
}

// The second item's paragraph after a paragraph is the reference: it already
// takes no marker and lines up with its item's text.
const SUB_LIST_THEN_PARAGRAPH: &str = "\
1. Prepare:
   - check

   Then tag.
2. Publish.

   Then announce.
";

struct NestedList;
impl Render for NestedList {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(TextView::markdown("list", SUB_LIST_THEN_PARAGRAPH))
    }
}

#[isonomic_ui::test]
fn a_paragraph_after_a_sub_list_takes_no_marker_of_its_own(cx: &mut TestAppContext) {
    cx.update(isonomic_ui_component::init);
    let (handle, _) = common::open_window(cx, Some(size(px(600.), px(500.))), |_, cx| {
        cx.new(|_| NestedList)
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let texts = texts(window);
        let left = |text: &str| {
            texts
                .iter()
                .find(|snapshot| snapshot.text() == text)
                .unwrap_or_else(|| panic!("{text:?} was not painted: {texts:?}"))
                .bounds()
                .left()
        };
        // After a marker, it would start where the item's own text does.
        assert_eq!(left("Then tag."), left("Then announce."));
        assert_ne!(left("Then tag."), left("Prepare:"));
    })
    .unwrap();
}

struct Labels;
impl Render for Labels {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("labels")
            .flex()
            .flex_col()
            .child(Label::new("Account details"))
            .child(Label::new("CPU").secondary("12%").id("cpu"))
            .child(Label::new("hunter2").masked(true).id("password"))
            .child(Label::new("").id("empty"))
            .children((0..2usize).map(|ix| Label::new(format!("Row {ix}")).id(("row", ix))))
    }
}

/// Plain text is not in the accessibility tree, so a label is a `Label` node
/// whose value is its text. A masked label keeps its text out of the tree, and
/// labels built by one line in a loop are told apart by their ids.
#[isonomic_ui::test]
fn labels_expose_their_text_to_assistive_technology(cx: &mut TestAppContext) {
    cx.update(isonomic_ui_component::init);
    let (handle, _) = common::open_window(cx, Some(size(px(600.), px(500.))), |_, cx| {
        cx.new(|_| Labels)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);

        let cpu = window.find("cpu");
        assert_eq!(cpu.role(), Some(isonomic_ui::Role::Label));
        assert_eq!(cpu.value(), Some("CPU 12%"));
        for (ix, text) in ["Row 0", "Row 1"].into_iter().enumerate() {
            assert_eq!(window.find(("row", ix)).value(), Some(text));
        }
        // Without an id of its own, a label is identified by where it was built.
        assert!(snapshots(window).iter().any(|label| {
            label.role() == Some(isonomic_ui::Role::Label)
                && label.value() == Some("Account details")
        }));

        for hidden in ["password", "empty"] {
            let label = window.find(hidden);
            assert_eq!(label.role(), None, "{hidden}");
            assert_eq!(label.value(), None, "{hidden}");
        }
    })
    .unwrap();
}
