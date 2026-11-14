#[cfg(feature = "gpui-fast")]
extern crate gpui_fast as gpui;

use gpui::App;
use std::ops::Deref;
use std::sync::LazyLock;

mod component_traits;
mod element_ext;
pub mod global_state;
mod icon;
mod index_path;
mod root;
mod sizing;
mod styled;
mod time;
mod title_bar;
mod touch_selection;
mod virtual_list;
mod window_border;
mod window_ext;

pub(crate) mod actions {
    pub use isonomic_ui_base::actions::*;
}

pub mod badge;
pub mod bubble;
pub mod button;
pub mod command;
pub mod description_list;
pub mod dialog;
pub mod highlighter;
pub mod history;
pub mod input;
pub mod kbd;
pub mod label;
pub mod list;
pub mod menu;
pub mod message;
pub mod native_menu;
pub mod notification;
pub mod plot;
pub mod popover;
pub mod progress;
pub mod scroll;
pub mod searchable_list;
pub mod select;
pub mod separator;
pub mod sheet;
pub mod skeleton;
pub mod spinner;
pub mod text;
pub mod theme;
pub mod tooltip;

pub use crate::Disableable;
pub use element_ext::*;
pub use global_state::GlobalState;
pub use icon::*;
pub use index_path::IndexPath;
pub use input::{Rope, RopeExt, RopeLines};
pub use isonomic_ui_base::Root;
pub use isonomic_ui_base::animation;
pub use isonomic_ui_base::{
    AxisExt, Edges, FocusTrapElement, InteractiveElementExt, LengthExt, Measure, OngoingScrollExt,
    Placement, Side, measure, measure_if,
};
pub use isonomic_ui_macros::icon_named;
pub use styled::*;
pub use theme::*;
pub use title_bar::*;
pub use virtual_list::{VirtualList, VirtualListScrollHandle, h_virtual_list, v_virtual_list};
pub use window_border::{WindowBorder, window_border, window_corner_radii, window_paddings};
pub use window_ext::WindowExt;

rust_i18n::i18n!("locales", fallback = "en");

/// Initialize the components.
///
/// You must initialize the components at your application's entry point.
pub fn init(cx: &mut App) {
    theme::init(cx);
    global_state::init(cx);
    root::init(cx);
    isonomic_ui_base::init(cx);
    input::init(cx);
    sheet::init(cx);
    list::init(cx);
    command::init(cx);
    notification::init(cx);
    popover::init(cx);
    menu::init(cx);
    tooltip::init(cx);
}

#[inline]
pub fn locale() -> impl Deref<Target = str> {
    rust_i18n::locale()
}

#[inline]
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale)
}

/// Whether measurement logging is enabled, read once per process.
///
/// Measurement is a startup debug flag, and the table queries it for every
/// rendered cell, so cache it instead of reading the environment each call.
#[inline]
pub(crate) fn measure_enable() -> bool {
    static MEASURE_ENABLED: LazyLock<bool> = LazyLock::new(isonomic_ui_base::measurement_enabled);
    *MEASURE_ENABLED
}

/// Draws a frame that re-renders every view. GPUI Fast replays retained
/// subtrees when nothing was invalidated, so a bare `window.draw` in a test
/// would skip `render`/`paint` and any side-channel recording they do.
#[cfg(test)]
pub(crate) fn test_redraw(window: &mut gpui::Window, cx: &mut gpui::App) {
    window.refresh();
    window.draw(cx).clear(cx);
}
