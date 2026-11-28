// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The permission prompt by keyboard, checked in a headless window: which
//! keys allow, which choices are offered and announced, and that a long
//! preview is shown whole.

use std::{cell::RefCell, rc::Rc};

use isonomic_ui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Point, Render, Role,
    Styled as _, TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions,
    base::Root,
    div, px, size,
    test::{TestQueryExt as _, TestWindowExt as _},
};
use isonomic_ui_extra::permission_prompt::{
    PermissionDecision, PermissionPreview, PermissionPrompt, PermissionRisk, PermissionScope,
};

const LABEL: &str = "Build assistant wants to run a command";

type Answers = Rc<RefCell<Vec<Option<PermissionDecision>>>>;

struct Harness {
    risk: PermissionRisk,
    deliberate: Option<bool>,
    preview: Option<String>,
    answers: Answers,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let answers = self.answers.clone();
        let mut prompt = PermissionPrompt::new("prompt", "Build assistant", "run a command")
            .target("Project folder")
            .risk(self.risk)
            .on_decision_change(move |decision, _, _| {
                answers.borrow_mut().push(decision.copied());
            });
        if let Some(deliberate) = self.deliberate {
            prompt = prompt.deliberate_allow(deliberate);
        }
        if let Some(text) = &self.preview {
            prompt = prompt.preview(PermissionPreview::new(text.clone()).title("Command"));
        }
        div().size_full().p_4().child(prompt)
    }
}

fn open(
    cx: &mut TestAppContext,
    risk: PermissionRisk,
    deliberate: Option<bool>,
    preview: Option<String>,
) -> (WindowHandle<Root>, Answers) {
    let answers: Answers = Rc::default();
    let shared = answers.clone();
    cx.update(isonomic_ui::init);
    let window = cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(800.), px(900.)),
        };
        isonomic_ui::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            move |_, cx| {
                cx.new(|_| Harness {
                    risk,
                    deliberate,
                    preview,
                    answers: shared,
                })
            },
        )
        .expect("open test window")
        .0
        .downcast::<Root>()
        .expect("Base Root")
    });
    (window, answers)
}

/// Tab until the element with `label` has focus.
fn tab_to(cx: &mut TestAppContext, window: WindowHandle<Root>, label: &str) {
    cx.update_window(window.into(), |_, window, cx| window.focus_next(cx))
        .unwrap();
    for _ in 0..12 {
        let reached = cx
            .update_window(window.into(), |_, window, cx| {
                window.render_frame(cx);
                window.try_find_by_label(label).and_then(|e| e.focused()) == Some(true)
            })
            .unwrap();
        if reached {
            return;
        }
        cx.update_window(window.into(), |_, window, cx| window.press("tab", cx))
            .unwrap();
        cx.run_until_parked();
    }
    panic!("Tab never focused {label:?}");
}

fn press(cx: &mut TestAppContext, window: WindowHandle<Root>, key: &str) {
    cx.update_window(window.into(), |_, window, cx| window.press(key, cx))
        .unwrap();
    cx.run_until_parked();
}

fn allowed(answers: &Answers) -> Vec<(bool, PermissionScope)> {
    answers
        .borrow()
        .iter()
        .flatten()
        .map(|decision| (decision.allow, decision.scope))
        .collect()
}

#[isonomic_ui::test]
fn on_a_risky_prompt_plain_enter_does_nothing_and_the_modified_enter_allows(
    cx: &mut TestAppContext,
) {
    let (window, answers) = open(cx, PermissionRisk::Risky, None, None);
    tab_to(cx, window, LABEL);
    press(cx, window, "enter");
    assert!(answers.borrow().is_empty(), "a plain Enter answered");
    press(cx, window, "secondary-enter");
    assert_eq!(allowed(&answers), [(true, PermissionScope::Once)]);
}

#[isonomic_ui::test]
fn on_a_destructive_prompt_escape_denies_after_a_plain_enter(cx: &mut TestAppContext) {
    let (window, answers) = open(cx, PermissionRisk::Destructive, None, None);
    tab_to(cx, window, LABEL);
    press(cx, window, "enter");
    press(cx, window, "space");
    assert!(answers.borrow().is_empty(), "a plain key answered");
    press(cx, window, "escape");
    assert_eq!(allowed(&answers), [(false, PermissionScope::Once)]);
}

#[isonomic_ui::test]
fn a_focused_allow_button_ignores_enter_and_space_when_the_key_is_deliberate(
    cx: &mut TestAppContext,
) {
    let (window, answers) = open(cx, PermissionRisk::Risky, None, None);
    tab_to(cx, window, "Allow this once");
    press(cx, window, "enter");
    press(cx, window, "space");
    assert!(answers.borrow().is_empty(), "the Allow button took a key");
    press(cx, window, "secondary-enter");
    assert_eq!(allowed(&answers), [(true, PermissionScope::Once)]);
}

#[isonomic_ui::test]
fn with_the_option_off_plain_enter_allows_on_the_prompt(cx: &mut TestAppContext) {
    let (window, answers) = open(cx, PermissionRisk::Risky, Some(false), None);
    tab_to(cx, window, LABEL);
    press(cx, window, "right");
    press(cx, window, "enter");
    assert_eq!(allowed(&answers), [(true, PermissionScope::Chat)]);
}

#[isonomic_ui::test]
fn a_safe_prompt_keeps_plain_enter(cx: &mut TestAppContext) {
    let (window, answers) = open(cx, PermissionRisk::Safe, None, None);
    tab_to(cx, window, LABEL);
    press(cx, window, "enter");
    assert_eq!(allowed(&answers), [(true, PermissionScope::Once)]);
}

fn choices(cx: &mut TestAppContext, window: WindowHandle<Root>) -> Vec<(String, bool)> {
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        window
            .find_all_by_role(Role::RadioButton)
            .into_iter()
            .map(|choice| {
                (
                    choice.label().unwrap_or_default().to_string(),
                    choice.checked() == Some(true),
                )
            })
            .collect()
    })
    .unwrap()
}

#[isonomic_ui::test]
fn the_choices_are_announced_and_arrows_move_the_pick(cx: &mut TestAppContext) {
    let (window, answers) = open(cx, PermissionRisk::Risky, None, None);
    let names = |list: &[(String, bool)]| {
        list.iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };
    let picked = |list: &[(String, bool)]| {
        list.iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };
    let start = choices(cx, window);
    assert_eq!(
        names(&start),
        ["This once", "This chat", "1 hour", "For good"]
    );
    assert_eq!(picked(&start), ["This once"]);

    tab_to(cx, window, LABEL);
    let description = cx
        .update_window(window.into(), |_, window, _| {
            window
                .find_by_label(LABEL)
                .description()
                .map(str::to_string)
        })
        .unwrap()
        .unwrap_or_default();
    assert!(description.starts_with("Allow this once."), "{description}");
    assert!(description.contains("Escape denies"), "{description}");
    assert!(description.contains("Enter allows"), "{description}");

    press(cx, window, "right");
    assert_eq!(picked(&choices(cx, window)), ["This chat"]);
    press(cx, window, "secondary-enter");
    assert_eq!(allowed(&answers), [(true, PermissionScope::Chat)]);
}

#[isonomic_ui::test]
fn a_destructive_prompt_never_offers_for_good(cx: &mut TestAppContext) {
    let (window, _) = open(cx, PermissionRisk::Destructive, None, None);
    let list = choices(cx, window);
    let names: Vec<_> = list.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["This once", "This chat", "1 hour"]);
}

#[isonomic_ui::test]
fn a_two_thousand_character_preview_is_shown_whole_and_wrapped(cx: &mut TestAppContext) {
    // A long command on one line, as a shell tool would send it.
    let text: String = "./scripts/publish --mirror ./build/ web-01:/srv/site/ && "
        .chars()
        .cycle()
        .take(2000)
        .collect();
    assert_eq!(text.chars().count(), 2000);
    let (window, _) = open(cx, PermissionRisk::Destructive, None, Some(text));
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let preview = window.find_by_label("Command");
        assert_eq!(preview.role(), Some(Role::Document));
        assert_eq!(
            preview.description(),
            Some("2,000 characters. Up and down arrows scroll.")
        );
        let body = window.within("prompt").find("preview-text");
        let shown = preview.bounds();
        let whole = body.bounds();
        // Wrapped: the text is no wider than its box and runs to many lines.
        assert!(
            whole.size.width <= shown.size.width,
            "{whole:?} in {shown:?}"
        );
        assert!(
            whole.size.height > shown.size.height,
            "the text fits in {shown:?}, so it was cut: {whole:?}"
        );
    })
    .unwrap();

    tab_to(cx, window, "Command");
    press(cx, window, "pagedown");
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let preview = window.find_by_label("Command");
        let body = window.within("prompt").find("preview-text");
        assert!(
            body.bounds().origin.y < preview.bounds().origin.y,
            "Page Down did not scroll the preview"
        );
    })
    .unwrap();
}
