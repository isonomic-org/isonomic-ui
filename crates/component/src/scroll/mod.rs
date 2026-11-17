// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

mod scrollable;

pub use isonomic_ui_base::AutoScroll;
pub use isonomic_ui_base::ScrollableMask;
pub use isonomic_ui_base::{
    Scrollbar, ScrollbarAxis, ScrollbarEntrance, ScrollbarHandle, ScrollbarMode, ScrollbarMotion,
    ScrollbarStyles, ScrollbarThumbStyle, ScrollbarTrackStyle,
};
pub use scrollable::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scrollbar_is_the_base_type() {
        fn accepts_base(_: isonomic_ui_base::Scrollbar) {}
        fn accepts_handle(_: impl isonomic_ui_base::ScrollbarHandle) {}

        let handle = gpui::ScrollHandle::default();
        let scrollbar: crate::scroll::Scrollbar = Scrollbar::vertical(&handle).styles(|styles| {
            styles
                .track(|style| style.bg(gpui::transparent_black()))
                .thumb(|style| style.bg(gpui::transparent_black()))
                .thumb_hover(|style| style.bg(gpui::transparent_black()))
                .thumb_active(|style| style.bg(gpui::transparent_black()))
        });
        accepts_base(scrollbar);
        accepts_handle(handle);
    }
}
