// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The one cursor rule for pressable controls.
//!
//! Everything a person can press — buttons, links, checkboxes, switches,
//! radios, tabs, menu items, select triggers, accordion headers, pagination
//! items, rows that open something — shows the pointing hand while it can be
//! pressed. A disabled control shows the arrow. Text fields keep the text
//! cursor, and elements that cannot be pressed are left alone.
//!
//! Base controls call [`PressableExt::pressable`] before they apply the
//! caller's style, so a caller can still refine the cursor for one instance.

use gpui::{CursorStyle, Styled};

/// Applies the pressable cursor rule to an element.
pub trait PressableExt: Styled + Sized {
    /// Shows the pointing hand while `enabled`, and the arrow otherwise.
    ///
    /// The arrow is set explicitly when disabled, so a disabled control inside
    /// a pressable row or card does not inherit the row's pointing hand.
    fn pressable(self, enabled: bool) -> Self {
        self.cursor(pressable_cursor(enabled))
    }
}

impl<T: Styled> PressableExt for T {}

/// The cursor a pressable control shows in the given state.
pub fn pressable_cursor(enabled: bool) -> CursorStyle {
    if enabled {
        CursorStyle::PointingHand
    } else {
        CursorStyle::Arrow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{InteractiveElement as _, div};

    #[test]
    fn enabled_controls_show_the_pointing_hand() {
        let mut element = div().id("pressable").pressable(true);
        assert_eq!(
            element.interactivity().base_style.mouse_cursor,
            Some(CursorStyle::PointingHand)
        );
    }

    #[test]
    fn disabled_controls_show_the_arrow() {
        let mut element = div().id("pressable").pressable(false);
        assert_eq!(
            element.interactivity().base_style.mouse_cursor,
            Some(CursorStyle::Arrow)
        );
    }
}
