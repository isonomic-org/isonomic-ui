// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(feature = "gpui-fast")]
extern crate gpui_fast as gpui;

use gpui::{
    Axis, Div, InteractiveElement as _, Length, ParentElement as _, Pixels, Stateful,
    StatefulInteractiveElement as _, Styled as _, blue, green, prelude::FluentBuilder as _, px,
    red,
};
use isonomic_ui_component::{
    AxisExt as _, FocusTrapElement, InteractiveElementExt, LengthExt as _, Placement, Side,
    animation,
};

#[test]
fn legacy_foundation_exports_remain_available() {
    assert!(Axis::Horizontal.is_horizontal());
    assert!(Axis::Vertical.is_vertical());
    assert_eq!(Placement::Top.axis(), Axis::Vertical);
    assert!(Side::Left.is_left());

    let length = Length::Definite(px(12.).into());
    let pixels: Option<Pixels> = length.to_pixels(px(16.).into(), px(16.));
    assert_eq!(pixels, Some(px(12.)));

    assert_eq!(animation::ease_in_cubic(0.5), 0.125);

    fn requires_interaction_extensions<T: FocusTrapElement + InteractiveElementExt>() {}
    requires_interaction_extensions::<Stateful<Div>>();
}

#[test]
fn focus_ring_api_exposes_component_state_without_visual_options() {
    use isonomic_ui_component::FocusableExt as _;

    let button = isonomic_ui_component::button::Button::new("focus-ring-state").focus_ring(false);
    assert!(!button.is_focus_ring_enabled());
}

#[test]
fn base_crate_exports_the_same_foundation_types() {
    let legacy = isonomic_ui_component::Edges::all(1_u8);
    let base: isonomic_ui_base::Edges<u8> = legacy;

    assert_eq!(base.top, 1);
    assert_eq!(base.right, 1);
    assert_eq!(base.bottom, 1);
    assert_eq!(base.left, 1);
}

#[test]
fn motion_core_types_are_available_from_the_base_facade() {
    let timing = isonomic_ui_base::Timing::new(std::time::Duration::from_millis(100))
        .ease(isonomic_ui_base::Easing::Linear);
    assert_eq!(
        timing
            .sample(std::time::Duration::from_millis(50))
            .directed_progress,
        0.5
    );
}

#[test]
fn base_avatar_uses_application_owned_image_and_fallback_slots() {
    let _ = isonomic_ui_base::Avatar::new()
        .image(isonomic_ui_base::AvatarImage::new(gpui::ImageSource::from(
            "avatar.png",
        )))
        .fallback(isonomic_ui_base::AvatarFallback::new().child("JL"));
}

#[test]
fn base_sheet_accepts_application_owned_overlay_and_surface() {
    fn build(cx: &mut gpui::App) {
        let _ = isonomic_ui_base::Sheet::new(cx)
            .overlay(gpui::div())
            .surface(gpui::div())
            .overlay_closable(false)
            .on_close(|_, _, _| {});
    }

    let _ = build;
}

#[test]
fn base_dialog_owns_modal_actions_and_alert_defaults() {
    fn dialog(cx: &mut gpui::App) {
        let handle = isonomic_ui_base::DialogHandle::new(true);
        let _ = isonomic_ui_base::Dialog::new(cx)
            .handle(handle.clone())
            .on_open_change(|_, _, _, _| {})
            .backdrop(isonomic_ui_base::DialogBackdrop::new())
            .popup(isonomic_ui_base::DialogPopup::new())
            .on_ok(|_, _, _| true)
            .on_cancel(|_, _, _| true)
            .on_close(|_, _, _| {});
        let _ = isonomic_ui_base::AlertDialog::new(cx)
            .handle(handle)
            .backdrop(isonomic_ui_base::AlertDialogBackdrop::new())
            .popup(isonomic_ui_base::AlertDialogPopup::new());
    }

    let _ = dialog;
    let _ = isonomic_ui_base::DialogTitle::new().child("Title");
    let _ = isonomic_ui_base::DialogDescription::new().child("Description");
    let _ = isonomic_ui_base::DialogClose::new().child(gpui::div());
    let _ = isonomic_ui_base::AlertDialogTrigger::new(gpui::div());
    let _ = isonomic_ui_base::AlertDialogTitle::new().child("Title");
    let _ = isonomic_ui_base::AlertDialogDescription::new().child("Description");
    let _ = isonomic_ui_base::AlertDialogAction::new().child(gpui::div());
    let _ = isonomic_ui_base::AlertDialogCancel::new().child(gpui::div());
    let _: isonomic_ui_base::actions::Cancel = isonomic_ui_component::dialog::Cancel;
    let _: isonomic_ui_base::actions::Confirm =
        isonomic_ui_component::dialog::Confirm { secondary: false };
}

#[test]
fn base_button_accepts_application_owned_state_styles() {
    let _button = isonomic_ui_base::Button::new("save")
        .accessibility_label("Save")
        .disabled(false)
        .on_click(|_, _, _| {})
        .child("Save")
        .bg(red())
        .hover(|style| style.bg(green()))
        .active(|style| style.bg(blue()))
        .focus_visible(|style| style.border_1());
}

#[test]
fn base_controls_expose_typed_semantic_style_contexts() {
    let _ = isonomic_ui_base::Button::new("button")
        .styles(|styles| styles.disabled(|style| style.opacity(0.5)));
    let _ = isonomic_ui_base::Checkbox::new("checkbox").styles(|styles| {
        styles
            .checked(|style| style.bg(green()))
            .indeterminate(|style| style.bg(blue()))
            .disabled(|style| style.when(true, |style| style.opacity(0.5)))
    });
    let _ = isonomic_ui_base::Radio::new("radio")
        .styles(|styles| styles.checked(|style| style.bg(green())));
    let _ = isonomic_ui_base::Switch::new("switch")
        .styles(|styles| styles.checked(|style| style.bg(green())));
    let _ = isonomic_ui_base::Toggle::new("toggle")
        .styles(|styles| styles.pressed(|style| style.bg(green())));
    let _ = isonomic_ui_base::Link::new("link")
        .styles(|styles| styles.disabled(|style| style.opacity(0.5)));
    let _ = isonomic_ui_base::Tab::new("tab").styles(|styles| {
        styles
            .selected(|style| style.bg(green()))
            .disabled(|style| style.opacity(0.5))
    });
    let _ = isonomic_ui_base::Tabs::new("tabs").child(
        isonomic_ui_base::Tab::new("first")
            .selected(true)
            .on_click(|_, _, _| {}),
    );
}

#[test]
fn base_collapsible_uses_normal_child_and_content_slots() {
    let _ = isonomic_ui_base::Collapsible::new()
        .open(true)
        .child("Trigger")
        .content("Content");
    let _ = isonomic_ui_component::collapsible::Collapsible::new()
        .open(true)
        .child("Trigger")
        .content("Content");
}

#[test]
fn base_accordion_uses_normal_parts_and_children() {
    let trigger = isonomic_ui_base::AccordionTrigger::new("trigger")
        .open(true)
        .disabled(false)
        .child("Title");
    let header = isonomic_ui_base::AccordionHeader::new(trigger).level(3);
    let panel = isonomic_ui_base::AccordionPanel::new()
        .open(true)
        .child("Content");
    let item = isonomic_ui_base::AccordionItem::new()
        .open(true)
        .header(header)
        .panel(panel);
    let _ = isonomic_ui_base::Accordion::new("accordion").child(item);
}

#[test]
fn legacy_behavior_traits_are_the_base_traits() {
    fn selectable<T: isonomic_ui_base::Selectable>(value: T) -> T {
        value
    }
    fn disableable<T: isonomic_ui_base::Disableable>(value: T) -> T {
        value
    }

    let _ = selectable(isonomic_ui_component::button::Button::new("selectable"));
    let _ = disableable(isonomic_ui_component::button::Button::new("disableable"));
}

#[test]
fn legacy_measure_type_is_the_base_type() {
    fn accepts_base(_: isonomic_ui_base::Measure) {}
    accepts_base(isonomic_ui_component::Measure::new("compat"));
}

#[test]
fn base_progress_accepts_application_owned_indicator_content() {
    let _ = isonomic_ui_base::Progress::new("progress")
        .value(42.)
        .child(isonomic_ui_base::ProgressTrack::new())
        .child(isonomic_ui_base::ProgressIndicator::new().child(gpui::div()));
}

#[test]
fn base_table_and_toast_accept_application_owned_composition() {
    let row = isonomic_ui_base::TableRow::new("row", 1)
        .child(isonomic_ui_base::TableHead::new("head", 1).child("Name"))
        .child(isonomic_ui_base::TableCell::new("cell", 1).child("Ada"));
    let _ = isonomic_ui_base::Table::new("table")
        .child(isonomic_ui_base::TableHeader::new("header").child(row))
        .child(isonomic_ui_base::TableBody::new("body"))
        .child(isonomic_ui_base::TableCaption::new("caption").child("People"));

    let _ =
        isonomic_ui_base::ToastStack::new("toasts", isonomic_ui_base::ToastStackState::default())
            .child(isonomic_ui_base::Toast::new("toast").child("Saved"));
}

#[test]
fn legacy_tree_models_are_base_types() {
    fn accepts_base(_: isonomic_ui_base::TreeItem) {}
    accepts_base(isonomic_ui_component::tree::TreeItem::new("id", "Label"));
}

#[test]
fn base_combobox_accepts_application_owned_content() {
    fn build(trigger: gpui::FocusHandle, content: gpui::FocusHandle) {
        let _ = isonomic_ui_base::Combobox::new("language")
            .open(true)
            .disabled(false)
            .focus_handle(&trigger)
            .content_focus_handle(&content)
            .on_open_change(|_, _, _| {})
            .on_confirm(|_, _| {})
            .child(gpui::div());
    }

    let _ = build;
}

#[test]
fn base_input_frames_accept_application_owned_content() {
    let _ = isonomic_ui_base::InputBase::new("input").child(gpui::div());
    fn build_number(state: &gpui::Entity<isonomic_ui_base::input::InputState>) {
        let _ = isonomic_ui_base::NumberInput::new(state)
            .on_step(|_, _, _| {})
            .decrement_button(|button| button.child("-"))
            .increment_button(|button| button.child("+"))
            .input(gpui::div())
            .child(gpui::div());
    }
    let _ = build_number;
    let _ = isonomic_ui_base::NumberInputText::new().child(gpui::div());

    let _: isonomic_ui_base::StepAction = isonomic_ui_component::input::StepAction::Increment;
}

#[test]
fn base_input_accepts_application_owned_highlighters() {
    struct CustomHighlighter;

    impl isonomic_ui_base::input::InputHighlighter for CustomHighlighter {
        fn language(&self) -> gpui::SharedString {
            "custom".into()
        }

        fn update(
            &mut self,
            _: Option<isonomic_ui_base::input::InputEdit>,
            _: &isonomic_ui_base::input::Rope,
            _: bool,
            _: &mut gpui::Window,
            _: &mut gpui::Context<isonomic_ui_base::input::EditorState>,
        ) {
        }

        fn styles(
            &self,
            _: &std::ops::Range<usize>,
            _: &dyn isonomic_ui_base::input::HighlightStyleResolver,
        ) -> Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> {
            Vec::new()
        }

        fn fold_ranges(
            &self,
            _: &isonomic_ui_base::input::Rope,
        ) -> Vec<isonomic_ui_base::input::FoldRange> {
            Vec::new()
        }
    }

    let factory: isonomic_ui_base::input::InputHighlighterFactory =
        std::rc::Rc::new(|_| Some(Box::new(CustomHighlighter)));
    let _ = factory;
}

#[test]
fn legacy_list_settings_are_the_base_type() {
    fn accepts_base(_: isonomic_ui_base::ListSettings) {}
    accepts_base(isonomic_ui_component::list::ListSettings::default());
}

#[test]
fn legacy_global_state_is_the_base_type() {
    fn accepts_base(_: &isonomic_ui_base::GlobalState) {}
    fn through_legacy(state: &isonomic_ui_component::GlobalState) {
        accepts_base(state);
    }

    let _ = through_legacy;
}

#[test]
fn legacy_popover_state_is_the_base_type() {
    fn accepts_base(_: &isonomic_ui_base::PopoverState) {}
    fn through_legacy(state: &isonomic_ui_component::popover::PopoverState) {
        accepts_base(state);
    }

    let _ = through_legacy;
}

#[test]
fn legacy_hover_card_state_is_the_base_type() {
    fn accepts_base(_: &isonomic_ui_base::HoverCardState) {}
    fn through_legacy(state: &isonomic_ui_component::hover_card::HoverCardState) {
        accepts_base(state);
    }

    let _ = through_legacy;
}

#[test]
fn base_tooltip_positioner_uses_normal_children() {
    let _ = isonomic_ui_base::Tooltip::new("tooltip-popup")
        .child("Tooltip")
        .child("⌘K");
    let _ = isonomic_ui_base::TooltipPositioner::new(gpui::Bounds::default())
        .placement(isonomic_ui_component::Placement::Right)
        .child("Tooltip");
}

#[test]
fn transition_ids_accept_strings_and_named_channels() {
    let _: isonomic_ui_base::TransitionId = "opacity".into();
    let _: isonomic_ui_base::TransitionId = ("checkbox", "fill").into();

    fn requires_interpolation<T: isonomic_ui_base::Interpolate>() {}
    requires_interpolation::<f32>();
}

#[test]
fn legacy_styled_and_sizing_exports_remain_available() {
    use isonomic_ui_component::Sizable as _;

    let _: isonomic_ui_component::Size = isonomic_ui_component::Size::Medium;
    let _ = isonomic_ui_component::StyledExt::font_medium(gpui::div());
    let _ = isonomic_ui_component::h_flex();
    let _ = isonomic_ui_component::v_flex();
    let _ = isonomic_ui_component::box_shadow(0., 0., 0., 0., gpui::hsla(0., 0., 0., 0.));

    struct SizedValue;
    impl isonomic_ui_component::Sizable for SizedValue {
        fn with_size(self, _: impl Into<isonomic_ui_component::Size>) -> Self {
            self
        }
    }
    let _ = SizedValue.small();
}

#[test]
fn element_ext_is_available_from_base_and_the_legacy_root() {
    use isonomic_ui_component::ElementExt as _;

    fn requires_base<T: isonomic_ui_base::ElementExt>() {}
    fn requires_legacy<T: isonomic_ui_component::ElementExt>() {}
    requires_base::<gpui::Div>();
    requires_legacy::<gpui::Div>();

    let _ = gpui::div().on_prepaint(|_, _, _| {});
}

#[test]
fn legacy_history_path_reexports_the_base_type() {
    let _: isonomic_ui_component::history::History<u8> = isonomic_ui_base::History::new();
    let _: isonomic_ui_component::history::UndoHistory<u8> = isonomic_ui_base::UndoHistory::new();
}

#[test]
fn legacy_auto_scroll_path_reexports_the_base_type() {
    fn through_legacy_path(
        scroll: isonomic_ui_base::AutoScroll,
    ) -> isonomic_ui_component::scroll::AutoScroll {
        scroll
    }

    let _ = through_legacy_path(isonomic_ui_base::AutoScroll::default());
}

#[test]
fn root_resizable_paths_reexport_base_types() {
    fn state_through_facade(
        state: isonomic_ui_base::ResizableState,
    ) -> isonomic_ui_component::ResizableState {
        state
    }

    fn group_through_facade(
        group: isonomic_ui_base::ResizablePanelGroup,
    ) -> isonomic_ui_component::ResizablePanelGroup {
        group
    }

    fn panel_through_facade(
        panel: isonomic_ui_base::ResizablePanel,
    ) -> isonomic_ui_component::ResizablePanel {
        panel
    }

    let _ = state_through_facade(isonomic_ui_base::ResizableState::default());
    let _ = group_through_facade(isonomic_ui_base::ResizablePanelGroup::new("group"));
    let _ = panel_through_facade(isonomic_ui_base::resizable_panel());
    let _ = isonomic_ui_component::h_resizable("horizontal");
    let _ = isonomic_ui_component::v_resizable("vertical");
}

#[test]
fn legacy_resizable_module_paths_remain_available() {
    fn panel_through_legacy(
        panel: isonomic_ui_base::ResizablePanel,
    ) -> isonomic_ui_component::resizable::ResizablePanel {
        panel
    }

    let _ = panel_through_legacy(isonomic_ui_base::resizable_panel());
    let _ = isonomic_ui_component::resizable::h_resizable("horizontal");
    let _ = isonomic_ui_component::resizable::v_resizable("vertical");
}
