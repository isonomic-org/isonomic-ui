// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Rows that a scripted test of a real app has to find by a stable id.

use isonomic_ui::component::sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem};
use isonomic_ui::component::{
    button::Button,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use isonomic_ui::{
    AppContext as _, Bounds, Context, ElementId, Entity, IntoElement, ParentElement as _, Point,
    Render, Role, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size,
    test::TestWindowExt as _,
};
use isonomic_ui_extra::command_palette::{CommandPalette, PaletteCommand};
use isonomic_ui_extra::model_picker::{self, ModelOption, ModelPicker};

struct Harness {
    palette: Entity<CommandPalette>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(
                Sidebar::new("chats").collapsible(false).w(px(240.)).child(
                    SidebarGroup::new("Chats").child(
                        SidebarMenu::new()
                            .child(SidebarMenuItem::new("New chat").id("row-new-chat"))
                            .child(
                                SidebarMenuItem::new("March close")
                                    .id("row-chat-42")
                                    .active(true),
                            ),
                    ),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .child(
                        ModelPicker::new("picker")
                            .model(ModelOption::new("small", "Small"))
                            .model(ModelOption::new("medium", "Medium").tagline("Balanced"))
                            .default_value("medium"),
                    )
                    .child(
                        Button::new("more")
                            .label("More")
                            .dropdown_menu(|menu, _, _| {
                                menu.item(PopupMenuItem::new("Rename").id("menu-rename"))
                                    .item(
                                        PopupMenuItem::new("Delete")
                                            .id("menu-delete")
                                            .destructive(),
                                    )
                            }),
                    )
                    .child(self.palette.clone()),
            )
    }
}

#[isonomic_ui::test]
fn rows_are_found_by_their_own_ids(cx: &mut TestAppContext) {
    cx.update(isonomic_ui::init);
    let window = cx.update(|cx| {
        isonomic_ui::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(900.), px(600.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| {
                let palette = cx.new(|cx| {
                    let mut palette = CommandPalette::new(
                        vec![
                            PaletteCommand::new("new", "Actions", "New chat"),
                            PaletteCommand::new("chat-42", "Today", "March close").meta("active"),
                        ],
                        window,
                        cx,
                    );
                    // The same command shows twice: under Recent and under its group.
                    palette.set_recent(vec!["chat-42".into()], cx);
                    palette
                });
                cx.new(|_| Harness { palette })
            },
        )
        .expect("open test window")
        .0
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);

        // Sidebar rows, whatever their place in the menu.
        let row = window.find("row-chat-42");
        assert_eq!(row.role(), Some(Role::TreeItem));
        assert_eq!(row.label(), Some("March close"));
        assert_eq!(row.selected(), Some(true));
        assert_eq!(window.find("row-new-chat").selected(), Some(false));

        // Palette rows by command id, with no index; the meta text on its own.
        let command = window.find(ElementId::Name("command-chat-42".into()));
        assert_eq!(command.role(), Some(Role::ListBoxOption));
        assert_eq!(command.label(), Some("March close"));
        assert_eq!(
            window
                .find(ElementId::Name("command-chat-42-meta".into()))
                .label(),
            Some("active")
        );
        assert!(
            window
                .try_find(ElementId::Name("command-new".into()))
                .is_some()
        );
    })
    .unwrap();

    // Model picker rows by model id, once its menu is open.
    cx.update_window(window, |_, window, cx| {
        window.click(
            ElementId::NamedChild(ElementId::from("picker").into(), "trigger".into()),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let list = window.find(model_picker::list_id("picker"));
        assert_eq!(list.role(), Some(Role::ListBox));
        let medium = window.find(model_picker::row_id("picker", "medium"));
        assert_eq!(medium.label(), Some("Medium, Balanced"));
        assert_eq!(medium.selected(), Some(true));
        assert_eq!(
            window
                .find(model_picker::row_id("picker", "small"))
                .selected(),
            Some(false)
        );
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();

    // Menu items by their own ids, once the menu is open.
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("more", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let delete = window.find("menu-delete");
        assert_eq!(delete.role(), Some(Role::MenuItem));
        assert_eq!(delete.label(), Some("Delete"));
        assert!(window.try_find("menu-rename").is_some());
    })
    .unwrap();
}
