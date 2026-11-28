// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::common;
use isonomic_ui::component::{
    Root, WindowExt,
    button::Button,
    dialog::{DialogAction, DialogClose, DialogFooter},
    input::{Input, InputState},
    notification::Notification,
};
use isonomic_ui::test::{TestAppContextExt, TestSupportExt as _, TestWindowExt};
use isonomic_ui::{
    AppContext, Bounds, Context, Entity, Focusable, TestAppContext, Window, div, point, prelude::*,
    px, size,
};
use std::time::Duration;

struct Workspace {
    saved: Entity<InputState>,
    draft: Entity<InputState>,
}
impl Render for Workspace {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let draft = self.draft.clone();
        let saved = self.saved.clone();
        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(Input::new(&self.saved).id("name"))
            .child(
                Button::new("edit")
                    .label("Edit…")
                    .on_click(move |_, window, cx| {
                        let draft = draft.clone();
                        let saved = saved.clone();
                        window.open_dialog(cx, move |dialog, _, _| {
                            let draft = draft.clone();
                            let saved = saved.clone();
                            dialog
                                .title("Edit profile")
                                .child(Input::new(&draft).id("name"))
                                .footer(DialogFooter::new().child(
                                    DialogAction::new().child(Button::new("ok").label("Save")),
                                ))
                                .on_ok(move |_, window, cx| {
                                    if draft.read(cx).value().is_empty() {
                                        return false;
                                    }
                                    let value = draft.read(cx).value();
                                    saved
                                        .update(cx, |input, cx| input.set_value(value, window, cx));
                                    window.push_notification(
                                        Notification::new()
                                            .message("Profile saved")
                                            .autohide(false),
                                        cx,
                                    );
                                    true
                                })
                        });
                    }),
            )
            .child(
                Button::new("inspect")
                    .label("Inspect…")
                    .on_click(|_, window, cx| {
                        window.open_sheet(cx, |sheet, _, _| {
                            sheet.title("Inspector").child("Details")
                        });
                    }),
            )
            .child(
                Button::new("notify")
                    .label("Notify")
                    .on_click(|_, window, cx| {
                        window.push_notification(
                            Notification::new().message("Updated").autohide(true),
                            cx,
                        );
                    }),
            )
    }
}

#[isonomic_ui::test]
async fn dialog_validates_scoped_input_saves_and_dismisses_notification(cx: &mut TestAppContext) {
    cx.update(|cx| {
        isonomic_ui::init(cx);
        // Validation and scoped input should not depend on animation progress.
        cx.set_reduce_motion(true);
    });
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), |window, cx| {
        let view = cx.new(|cx| Workspace {
            saved: cx.new(|cx| InputState::new(window, cx)),
            draft: cx.new(|cx| InputState::new(window, cx)),
        });
        view
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("name", cx);
        window.click("edit", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let dialog = window.within("dialog").find("surface").bounds();
        assert!(dialog.left() >= px(0.) && dialog.right() <= window.viewport_size().width);
        assert!(dialog.top() >= px(0.) && dialog.bottom() <= window.viewport_size().height);
        window.within("dialog").click("ok", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("notification").is_none());
        assert!(
            window.find("dialog").visible(),
            "empty input must prevent confirmation"
        );
        window.within("dialog").click("name", cx);
        window.within("dialog").input("Ada 中文", cx);
        assert_eq!(
            window.within("dialog").find("name").value(),
            Some("Ada 中文")
        );
        window.within("dialog").click("ok", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none() && window.try_find("notification").is_some()
    })
    .await;
    // Toast entrance (400 ms) runs on the test clock.
    cx.executor().advance_clock(Duration::from_millis(410));
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("name").value(), Some("Ada 中文"));
        assert!(window.find("notification").visible());
        window.hover("notification", cx);
        window.within("notification").click("close", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("notification").is_none()
    })
    .await;
}

#[isonomic_ui::test]
async fn find_all_counts_stacked_notifications(cx: &mut TestAppContext) {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), |window, cx| {
        cx.new(|cx| Workspace {
            saved: cx.new(|cx| InputState::new(window, cx)),
            draft: cx.new(|cx| InputState::new(window, cx)),
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("notify", cx);
        window.click("notify", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.find_all("notification").len() == 2
    })
    .await;
    cx.update_window(handle.into(), |_, window, _| {
        let toasts = window.find_all("notification");
        assert_ne!(toasts[0].path(), toasts[1].path());
    })
    .unwrap();
}

#[isonomic_ui::test]
async fn escape_dismisses_dialog_and_sheet_and_restores_focus(cx: &mut TestAppContext) {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), |window, cx| {
        let view = cx.new(|cx| Workspace {
            saved: cx.new(|cx| InputState::new(window, cx)),
            draft: cx.new(|cx| InputState::new(window, cx)),
        });
        view
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("name", cx);
        window.click("edit", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("escape", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none() && window.find("name").focused() == Some(true)
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("name").focused(), Some(true));
        window.click("inspect", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("sheet-content").is_some()
    })
    .await;
    // GPUI Animation runs on the test clock; advance it past
    // the 200 ms entrance before asserting the final geometry.
    cx.executor().advance_clock(Duration::from_millis(240));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let sheet = window.find("sheet-content").bounds();
        assert!(
            sheet.size.width > px(0.) && sheet.right() <= window.viewport_size().width,
            "{sheet:?}"
        );
        window.press("escape", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("sheet").is_none() && window.try_find("sheet-content").is_none()
    })
    .await;
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("name").focused(), Some(true));
    })
    .unwrap();
}

#[isonomic_ui::test]
async fn sheet_and_notifications_stay_in_the_fully_visible_area(cx: &mut TestAppContext) {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(400.), px(800.))), |window, cx| {
        cx.new(|cx| Workspace {
            saved: cx.new(|cx| InputState::new(window, cx)),
            draft: cx.new(|cx| InputState::new(window, cx)),
        })
    });
    // As on a phone: a status bar above, the on-screen keyboard below.
    let visible = Bounds::new(point(px(0.), px(40.)), size(px(400.), px(500.)));
    cx.simulate_window_visual_viewport_change(handle.into(), visible);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.fully_visible_bounds(), visible);
        window.click("notify", cx);
        window.click("inspect", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("sheet-content").is_some() && window.try_find("notification").is_some()
    })
    .await;
    // Let the sheet's 200 ms entrance finish (see above).
    cx.executor().advance_clock(Duration::from_millis(240));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let sheet = window.find("sheet-content").bounds();
        assert!(
            sheet.top() >= visible.top() && sheet.bottom() <= visible.bottom(),
            "sheet {sheet:?} outside {visible:?}"
        );
        let toast = window.find("notification").bounds();
        assert!(
            toast.top() >= visible.top() && toast.bottom() <= visible.bottom(),
            "notification {toast:?} outside {visible:?}"
        );
    })
    .unwrap();
}

#[isonomic_ui::test]
async fn notification_auto_dismisses_after_its_timer(cx: &mut TestAppContext) {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), |window, cx| {
        let view = cx.new(|cx| Workspace {
            saved: cx.new(|cx| InputState::new(window, cx)),
            draft: cx.new(|cx| InputState::new(window, cx)),
        });
        view
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("notify", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("notification").is_some()
    })
    .await;
    // The default timeout is five seconds; the notification must remain
    // mounted before that deadline, not merely disappear eventually.
    for _ in 0..40 {
        cx.background_executor
            .advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("notification").is_some());
        })
        .unwrap();
    }
    cx.wait_for(handle.into(), Duration::from_secs(10), |window, _| {
        window.try_find("notification").is_none()
    })
    .await;
}

/// A view with one focusable control behind a dialog whose footer carries
/// both the confirm action and a `DialogClose` trigger.
struct Stealer {
    outside: Entity<InputState>,
}
impl Render for Stealer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child(Input::new(&self.outside).id("outside"))
            .child(
                Button::new("open")
                    .label("Open…")
                    .on_click(|_, window, cx| {
                        window.open_dialog(cx, |dialog, _, _| {
                            dialog.title("Stolen focus").footer(
                                DialogFooter::new()
                                    .child(div().id("footer-close").child(
                                        DialogClose::new().trigger(|button| button.label("Cancel")),
                                    ))
                                    .child(
                                        DialogAction::new().child(Button::new("ok").label("OK")),
                                    ),
                            )
                        });
                    }),
            )
            .child(
                Button::new("open-alert")
                    .label("Alert…")
                    .on_click(|_, window, cx| {
                        window.open_alert_dialog(cx, |dialog, _, _| {
                            dialog.title("Stolen focus").confirm()
                        });
                    }),
            )
    }
}

/// Opens the dialog, then moves focus back to the control behind it — what a
/// native web view or an always-on-top window does on its own.
async fn open_dialog_and_steal_focus(
    cx: &mut TestAppContext,
    open: &'static str,
) -> isonomic_ui::AnyWindowHandle {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), |window, cx| {
        let view = cx.new(|cx| Stealer {
            outside: cx.new(|cx| InputState::new(window, cx)),
        });
        view
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(open, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_some()
    })
    .await;
    cx.update_window(handle.into(), |root, window, cx| {
        let root = root.downcast::<Root>().unwrap().read(cx).view().clone();
        let outside = root.downcast::<Stealer>().unwrap().read(cx).outside.clone();
        let outside = outside.read(cx).focus_handle(cx);
        outside.focus(window, cx);
        window.render_frame(cx);
        assert!(
            outside.is_focused(window),
            "focus must sit outside the dialog"
        );
    })
    .unwrap();
    handle.into()
}

#[isonomic_ui::test]
async fn dialog_action_confirms_its_dialog_when_focus_was_stolen(cx: &mut TestAppContext) {
    let handle = open_dialog_and_steal_focus(cx, "open").await;
    cx.update_window(handle, |_, window, cx| {
        window.within("dialog").click("ok", cx);
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none()
    })
    .await;
}

#[isonomic_ui::test]
async fn dialog_close_trigger_cancels_its_dialog_when_focus_was_stolen(cx: &mut TestAppContext) {
    let handle = open_dialog_and_steal_focus(cx, "open").await;
    cx.update_window(handle, |_, window, cx| {
        window
            .within("dialog")
            .within("footer-close")
            .click("close", cx);
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none()
    })
    .await;
}

#[isonomic_ui::test]
async fn alert_dialog_default_buttons_reach_their_dialog_when_focus_was_stolen(
    cx: &mut TestAppContext,
) {
    let handle = open_dialog_and_steal_focus(cx, "open-alert").await;
    cx.update_window(handle, |_, window, cx| {
        window.within("dialog").click("ok", cx);
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none()
    })
    .await;
}

struct LongPage {
    scroll: isonomic_ui::ScrollHandle,
}
impl Render for LongPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("page")
                .test_support()
                .w(px(200.))
                .h(px(300.))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .flex()
                .flex_col()
                .children((0..60usize).map(|ix| {
                    div()
                        .id(("page-row", ix))
                        .h(px(20.))
                        .flex_shrink_0()
                        .child(format!("Row {ix}"))
                })),
        )
    }
}

#[isonomic_ui::test]
async fn an_open_sheet_or_dialog_locks_the_page_scroll(cx: &mut TestAppContext) {
    use isonomic_ui::ScrollDelta;
    cx.update(|cx| {
        isonomic_ui::init(cx);
        cx.set_reduce_motion(true);
    });
    let scroll = isonomic_ui::ScrollHandle::new();
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), {
        let scroll = scroll.clone();
        move |_, cx| cx.new(|_| LongPage { scroll })
    });
    let wheel = ScrollDelta::Pixels(point(px(0.), px(-100.)));
    // With nothing open the page scrolls, so the checks below can fail.
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.scroll("page", wheel, cx);
    })
    .unwrap();
    let scrolled = scroll.offset().y;
    assert!(scrolled < px(0.), "{scrolled:?}");

    cx.update_window(handle.into(), |_, window, cx| {
        window.open_sheet(cx, |sheet, _, _| sheet.title("Filters"));
        window.render_frame(cx);
        window.scroll("page", wheel, cx);
    })
    .unwrap();
    assert_eq!(
        scroll.offset().y,
        scrolled,
        "the sheet's backdrop lets the page scroll"
    );

    cx.update_window(handle.into(), |_, window, cx| {
        window.close_sheet(cx);
        window.render_frame(cx);
        window.open_dialog(cx, |dialog, _, _| dialog.title("Rename"));
        window.render_frame(cx);
        window.scroll("page", wheel, cx);
    })
    .unwrap();
    assert_eq!(
        scroll.offset().y,
        scrolled,
        "the dialog's backdrop lets the page scroll"
    );
}

#[isonomic_ui::test]
async fn a_closed_sheet_slides_out_without_taking_input(cx: &mut TestAppContext) {
    cx.update(isonomic_ui::init);
    let (handle, _) = common::open_window(cx, Some(size(px(800.), px(700.))), |window, cx| {
        cx.new(|cx| Workspace {
            saved: cx.new(|cx| InputState::new(window, cx)),
            draft: cx.new(|cx| InputState::new(window, cx)),
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("inspect", cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(400));
    let rest = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let rest = window.find("sheet-content").bounds();
            window.close_sheet(cx);
            assert!(!window.has_active_sheet(cx));
            rest
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // Partway out: still drawn, further right than at rest.
        let leaving = window.find("sheet-content").bounds();
        assert!(leaving.left() > rest.left(), "{leaving:?} vs {rest:?}");
        // The page under the leaving backdrop already takes a press.
        window.click("notify", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("sheet-content").is_none() && window.try_find("notification").is_some()
    })
    .await;
}
