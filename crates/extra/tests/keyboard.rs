// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard reach for extra components, checked in a headless window.

use isonomic_ui::Entity;
use isonomic_ui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Point, Render, Role,
    Styled as _, TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions,
    base::Root,
    div, px, size,
    test::{TestQueryExt as _, TestWindowExt as _},
};
use isonomic_ui_extra::{
    inline_diff_review::InlineDiffReview,
    sortable_table::{SortableTable, TableColumn, TableRow},
};

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
    open_view(cx, |cx| cx.new(|_| DiffHarness))
}

fn open_view<V: Render>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut isonomic_ui::App) -> Entity<V> + 'static,
) -> WindowHandle<Root> {
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
            |_, cx| build(cx),
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

/// Owns the sort, as a page would, and flips it when a header asks.
struct TableHarness {
    sort: Option<(usize, bool)>,
}

impl Render for TableHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().size_full().p_4().child(
            SortableTable::new(
                "bills",
                vec![TableColumn::text("Supplier"), TableColumn::number("Amount")],
                vec![
                    TableRow::new("a", vec!["Paper Co", "40.00"]),
                    TableRow::new("b", vec!["Ink Ltd", "12.50"]),
                ],
            )
            .sort(self.sort)
            .on_sort(move |col, _, cx| {
                this.update(cx, |this, cx| {
                    this.sort = match this.sort {
                        Some((c, asc)) if c == col => Some((col, !asc)),
                        _ => Some((col, true)),
                    };
                    cx.notify();
                })
            }),
        )
    }
}

#[isonomic_ui::test]
fn tab_reaches_a_column_header_and_enter_sorts_it(cx: &mut TestAppContext) {
    let window = open_view(cx, |cx| cx.new(|_| TableHarness { sort: None }));
    cx.update_window(window.into(), |_, window, cx| window.focus_next(cx))
        .unwrap();
    let mut reached = false;
    for _ in 0..6 {
        reached = cx
            .update_window(window.into(), |_, window, cx| {
                window.render_frame(cx);
                window
                    .try_find_by_label("Supplier, not sorted")
                    .and_then(|e| e.focused())
                    == Some(true)
            })
            .unwrap();
        if reached {
            break;
        }
        cx.update_window(window.into(), |_, window, cx| window.press("tab", cx))
            .unwrap();
        cx.run_until_parked();
    }
    assert!(reached, "Tab never focused the Supplier header");

    for (key, expected) in [
        ("enter", "Supplier, sorted up"),
        ("space", "Supplier, sorted down"),
    ] {
        cx.update_window(window.into(), |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            let header = window.find_by_label(expected);
            assert_eq!(header.role(), Some(Role::ColumnHeader));
            assert_eq!(
                header.focused(),
                Some(true),
                "{key} keeps focus on the header"
            );
        })
        .unwrap();
    }
}

struct FinderHarness {
    finder: Entity<isonomic_ui_extra::agent_finder::AgentFinder>,
}

impl Render for FinderHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(self.finder.clone())
    }
}

#[isonomic_ui::test]
fn the_finder_opens_folders_and_quick_looks_documents_from_the_keyboard(cx: &mut TestAppContext) {
    use isonomic_ui::Focusable as _;
    use isonomic_ui_extra::agent_finder::{AgentFinder, FinderAgent, FinderFile};

    let finder_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot = finder_slot.clone();
    let window = open_view(cx, move |cx| {
        let finder = cx.new(|cx| {
            AgentFinder::new(
                "Q3 close",
                vec![
                    FinderAgent::new("planner", "Planner")
                        .child(FinderAgent::new("researcher", "Researcher"))
                        .file(
                            FinderFile::new("checklist", "Close checklist.pdf")
                                .exchange("{}", "Created"),
                        ),
                    FinderAgent::new("ledger", "Ledger"),
                ],
                cx,
            )
        });
        *slot.borrow_mut() = Some(finder.clone());
        cx.new(|_| FinderHarness { finder })
    });
    let finder = finder_slot.borrow().clone().expect("finder built");
    let press = |cx: &mut TestAppContext, key: &str| {
        cx.update_window(window.into(), |_, window, cx| {
            window.press(key, cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        finder.read(cx).focus_handle(cx).focus(window, cx);
    })
    .unwrap();

    // The first arrow picks the first folder; Enter steps into it.
    press(cx, "right");
    press(cx, "enter");
    let found = |cx: &mut TestAppContext, label: &str| {
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.try_find_by_label(label).is_some()
        })
        .unwrap()
    };
    assert!(found(cx, "Close checklist.pdf, PDF document"));
    assert!(!found(cx, "Ledger"));

    // The next item is the document; Space shows it, Escape hides it again.
    press(cx, "right");
    press(cx, "space");
    assert!(found(cx, "Quick look: Close checklist.pdf"));
    press(cx, "escape");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(400));
    press(cx, "escape");
    assert!(!found(cx, "Quick look: Close checklist.pdf"));

    // Backspace goes back up to the top.
    press(cx, "backspace");
    assert!(found(cx, "Ledger"));
}

struct FleetHarness {
    fleet: Entity<isonomic_ui_extra::agent_fleet::AgentFleet>,
}

impl Render for FleetHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(self.fleet.clone())
    }
}

#[isonomic_ui::test]
fn the_fleet_moves_between_leads_and_opens_one_with_enter(cx: &mut TestAppContext) {
    use isonomic_ui::Focusable as _;
    use isonomic_ui_extra::agent_fleet::{AgentFleet, FleetAgent};

    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = slot.clone();
    let window = open_view(cx, move |cx| {
        let fleet = cx.new(|cx| {
            AgentFleet::new(
                vec![
                    FleetAgent::new("books", "Books lead", "Close the books")
                        .child(FleetAgent::new("matcher", "Matcher", "Match lines")),
                    FleetAgent::new("support", "Support lead", "Answer tickets")
                        .child(FleetAgent::new("triage", "Triage", "Sort tickets")),
                ],
                cx,
            )
        });
        *keep.borrow_mut() = Some(fleet.clone());
        cx.new(|_| FleetHarness { fleet })
    });
    let fleet = slot.borrow().clone().expect("fleet built");
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        fleet.read(cx).focus_handle(cx).focus(window, cx);
    })
    .unwrap();
    let opened = |cx: &mut TestAppContext| cx.update(|cx| fleet.read(cx).opened().cloned());
    assert_eq!(opened(cx).as_deref(), Some("books"));
    for key in ["down", "enter"] {
        cx.update_window(window.into(), |_, window, cx| {
            window.press(key, cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    assert_eq!(opened(cx).as_deref(), Some("support"));
    cx.update_window(window.into(), |_, window, cx| {
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(opened(cx), None);
}
