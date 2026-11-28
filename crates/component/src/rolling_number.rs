// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rolling number: a figure whose digits roll to their new values.
//!
//! Each digit sits on its own wheel of 0 to 9 and turns to the new value, the
//! rightmost first. Places are counted from the right, so a unit or separator
//! ("k", "%", ",") keeps its place while the digits beside it change. Under
//! reduced motion the new figure is shown at once.

use std::time::Duration;

use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, px,
};
use isonomic_ui_base::{SignedDuration, Transition, transition};

use crate::{ActiveTheme as _, StyledExt as _};

/// The delay between neighbouring digits, right to left.
const STAGGER: Duration = Duration::from_millis(24);

/// One character of the figure, with its place counted from the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// A digit, which rolls.
    Digit {
        /// 0 for the last character.
        from_right: usize,
        /// 0 to 9.
        digit: u8,
    },
    /// Anything else, which stays put.
    Mark {
        /// 0 for the last character.
        from_right: usize,
        /// The character shown.
        mark: char,
    },
}

/// Split a figure into its places.
pub fn places(text: &str) -> Vec<Place> {
    let count = text.chars().count();
    text.chars()
        .enumerate()
        .map(|(ix, mark)| {
            let from_right = count - 1 - ix;
            match mark.to_digit(10) {
                Some(digit) if mark.is_ascii_digit() => Place::Digit {
                    from_right,
                    digit: digit as u8,
                },
                _ => Place::Mark { from_right, mark },
            }
        })
        .collect()
}

/// Write a whole number with a comma between each group of three digits.
pub fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A figure whose digits roll when it changes.
#[derive(IntoElement)]
pub struct RollingNumber {
    id: ElementId,
    style: StyleRefinement,
    text: SharedString,
    line_height: Pixels,
    stagger: Duration,
}

impl RollingNumber {
    /// Show a figure that is already written out, such as "87.2k" or "64%".
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            text: text.into(),
            line_height: px(20.),
            stagger: STAGGER,
        }
    }

    /// Show a whole number, grouped in thousands.
    pub fn grouped(id: impl Into<ElementId>, value: u64) -> Self {
        Self::new(id, group_thousands(value))
    }

    /// The height of one line of the figure. Set it with the text size, so
    /// each wheel is exactly one line tall. 20px by default.
    pub fn line_height(mut self, line_height: impl Into<Pixels>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// The delay between neighbouring digits, right to left. 24ms by default.
    pub fn stagger(mut self, stagger: Duration) -> Self {
        self.stagger = stagger;
        self
    }
}

impl Styled for RollingNumber {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RollingNumber {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let line = self.line_height;
        let motion = cx.theme().motion_tokens();
        let roll = motion.duration_long;
        let easing = motion.easing_enter.clone();

        let mut cells = Vec::new();
        for place in places(&self.text) {
            match place {
                Place::Mark { mark, .. } => {
                    cells.push(div().h(line).child(mark.to_string()).into_any_element());
                }
                Place::Digit { from_right, digit } => {
                    // The wheel's offset in lines. Keyed by the place from the
                    // right, so a figure that gains a digit keeps its others.
                    let turned = transition(
                        (self.id.clone(), format!("place-{from_right}")),
                        f32::from(digit),
                        Transition::new(roll)
                            .easing(easing.clone())
                            .delay(SignedDuration::positive(self.stagger * from_right as u32)),
                        window,
                        cx,
                    );
                    cells.push(
                        div()
                            .relative()
                            .h(line)
                            .overflow_hidden()
                            // Sizes the wheel to one digit.
                            .child(div().invisible().child("0"))
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .top(-line * turned)
                                    .v_flex()
                                    .items_center()
                                    .children(
                                        (0..10u8)
                                            .map(|digit| div().h(line).child(digit.to_string())),
                                    ),
                            )
                            .into_any_element(),
                    );
                }
            }
        }

        div()
            .id(self.id)
            .flex()
            .flex_row()
            .flex_none()
            .line_height(line)
            .role(Role::Label)
            .aria_label(self.text)
            .children(cells)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_count_from_the_right() {
        assert_eq!(
            places("7k"),
            vec![
                Place::Digit {
                    from_right: 1,
                    digit: 7
                },
                Place::Mark {
                    from_right: 0,
                    mark: 'k'
                },
            ]
        );
    }

    #[test]
    fn a_longer_figure_keeps_the_places_it_had() {
        let before = places("98%");
        let after = places("100%");
        // The unit and the two digits beside it keep their places.
        assert_eq!(before[2], after[3]);
        assert!(matches!(before[1], Place::Digit { from_right: 1, .. }));
        assert!(matches!(after[2], Place::Digit { from_right: 1, .. }));
        assert!(matches!(after[0], Place::Digit { from_right: 3, .. }));
    }

    #[test]
    fn only_ascii_digits_roll() {
        assert!(matches!(places("٣")[0], Place::Mark { .. }));
        assert!(places("").is_empty());
    }

    #[test]
    fn whole_numbers_are_grouped_in_threes() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_000), "1,000");
        assert_eq!(group_thousands(87_240), "87,240");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
    }
}
