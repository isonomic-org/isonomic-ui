//! Keyboard reach for extra components, checked in a headless window.

use isonomic_ui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Point, Render, Role,
    Styled as _, TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions,
    base::Root,
    div, px, size,
    test::{TestQueryExt as _, TestWindowExt as _},
};
use isonomic_ui_extra::inline_diff_review::InlineDiffReview;

struct DiffHarness;

impl Render for DiffHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            InlineDiffReview::new("review", "pay in thirty days", "pay in 30 days")
                .title("Edits to the reminder"),
        )
    }
}

fn open(cx: &mut TestAppContext) -> WindowHandle<Root> {
    cx.update(isonomic_ui::init);
    cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(800.), px(400.)),
        };
        isonomic_ui::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| DiffHarness),
        )
        .expect("open test window")
        .0
        .downcast::<Root>()
        .expect("Base Root")
    })
}

#[isonomic_ui::test]
fn tab_reaches_the_diff_paragraph_and_it_is_named(cx: &mut TestAppContext) {
    let window = open(cx);
    let label = "Edits to the reminder, 1 of 1 changes left";
    // Start inside the window's focus chain, as a click on the window would.
    cx.update_window(window.into(), |_, window, cx| window.focus_next(cx))
        .unwrap();
    let mut reached = false;
    for _ in 0..6 {
        cx.update_window(window.into(), |_, window, cx| window.press("tab", cx))
            .unwrap();
        cx.run_until_parked();
        reached = cx
            .update_window(window.into(), |_, window, cx| {
                window.render_frame(cx);
                window.try_find_by_label(label).and_then(|e| e.focused()) == Some(true)
            })
            .unwrap();
        if reached {
            break;
        }
    }
    assert!(reached, "Tab never focused the paragraph");

    cx.update_window(window.into(), |_, window, cx| {
        let paragraph = window.find_by_label(label);
        assert_eq!(paragraph.role(), Some(Role::Group));
        window.press("a", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find_by_label("Edits to the reminder, 0 of 1 changes left")
                .is_some(),
            "A accepts the current change"
        );
    })
    .unwrap();
}
