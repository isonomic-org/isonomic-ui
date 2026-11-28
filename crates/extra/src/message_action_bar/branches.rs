// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Branches of a message: which way a switch between versions goes, the
//! slide that follows it, and the small map of every version.

use std::time::Duration;

use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, ElementId, IntoElement, ParentElement as _,
    Styled as _, div, px,
};

/// How far a swapped-in part starts from its place, in pixels.
const SWAP_TRAVEL: f32 = 8.;
/// How long a swap slides for.
pub(super) const SWAP_FOR: Duration = Duration::from_millis(180);
/// How many versions the map shows before it starts leaving some out.
pub(super) const MAP_SLOTS: usize = 9;

/// Which way the shown version moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwapDirection {
    /// To a newer version: the new one comes in from the end.
    Forward,
    /// To an older version: the new one comes in from the start.
    Back,
    /// Nothing moved.
    #[default]
    Still,
}

/// Which way a switch from version `from` to version `to` goes.
pub fn swap_direction(from: usize, to: usize) -> SwapDirection {
    match to.cmp(&from) {
        std::cmp::Ordering::Greater => SwapDirection::Forward,
        std::cmp::Ordering::Less => SwapDirection::Back,
        std::cmp::Ordering::Equal => SwapDirection::Still,
    }
}

/// Where a swapped-in part sits, in pixels from its place, `progress` of
/// the way through its slide: it starts on the side it comes from and eases
/// home.
pub fn swap_offset(direction: SwapDirection, progress: f32) -> f32 {
    let left = 1. - progress.clamp(0., 1.);
    let eased = left * left * left;
    match direction {
        SwapDirection::Forward => SWAP_TRAVEL * eased,
        SwapDirection::Back => -SWAP_TRAVEL * eased,
        SwapDirection::Still => 0.,
    }
}

/// Slide and fade `element` in from the side `direction` comes from, under
/// `id`, which should change with each version so the slide plays again.
/// With reduced motion, or nothing moved, it shows in place at once. An
/// owner can wrap the message's text in the same slide.
pub fn swap_in(
    id: impl Into<ElementId>,
    direction: SwapDirection,
    still: bool,
    element: impl IntoElement,
) -> AnyElement {
    let slot = div().relative().child(element);
    if still || direction == SwapDirection::Still {
        return slot.into_any_element();
    }
    slot.with_animation(id, Animation::new(SWAP_FOR), move |this, progress| {
        this.left(px(swap_offset(direction, progress)))
            .opacity(0.35 + 0.65 * progress)
    })
    .into_any_element()
}

/// One place in the branch map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    /// A version, counted from zero.
    Version(usize),
    /// Versions left out between two shown ones.
    Gap,
}

/// The places of a map of `count` versions with `current` showing. Up to
/// `room` versions show one each; past that the first, the last and the
/// ones around the current one show, with gaps for the rest.
pub(super) fn map_slots(count: usize, current: usize, room: usize) -> Vec<Slot> {
    if count <= room.max(5) {
        return (0..count).map(Slot::Version).collect();
    }
    let current = current.min(count - 1);
    let last = count - 1;
    let mut shown: Vec<usize> = vec![0, last];
    for near in current.saturating_sub(1)..=(current + 1).min(last) {
        shown.push(near);
    }
    shown.sort_unstable();
    shown.dedup();
    let mut slots = Vec::new();
    for (ix, version) in shown.iter().enumerate() {
        if ix > 0 && *version > shown[ix - 1] + 1 {
            slots.push(Slot::Gap);
        }
        slots.push(Slot::Version(*version));
    }
    slots
}

/// The version an arrow key moves the map to, or `None` for another key.
pub(super) fn map_step(current: usize, count: usize, key: &str) -> Option<usize> {
    let last = count.checked_sub(1)?;
    match key {
        "left" | "up" => Some(current.saturating_sub(1)),
        "right" | "down" => Some((current + 1).min(last)),
        "home" => Some(0),
        "end" => Some(last),
        _ => None,
    }
}

/// What a screen reader hears for the map: where it is and how to move.
pub(super) fn map_words(current: usize, count: usize, label: Option<&str>) -> String {
    match label {
        Some(label) => format!(
            "Versions: {} of {count}, {label}. Left and Right switch.",
            current + 1
        ),
        None => format!(
            "Versions: {} of {count}. Left and Right switch.",
            current + 1
        ),
    }
}

/// What a dot in the map says: its number, its name when it has one, and
/// whether it is the one showing.
pub(super) fn dot_words(version: usize, label: Option<&str>, current: bool) -> String {
    let mut words = format!("Version {}", version + 1);
    if let Some(label) = label {
        words.push_str(&format!(": {label}"));
    }
    if current {
        words.push_str(", showing");
    }
    words
}

/// A dot's width: the one showing is a short pill.
pub(super) fn dot_width(current: bool) -> f32 {
    if current { 12. } else { 6. }
}

/// The small marks for versions left out of the map.
pub(super) fn gap_mark(color: isonomic_ui::Hsla) -> AnyElement {
    div()
        .flex()
        .gap(px(1.5))
        .mx(px(1.))
        .children((0..3).map(|_| div().size(px(2.)).rounded_full().bg(color)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_goes_the_way_the_number_moves() {
        assert_eq!(swap_direction(1, 2), SwapDirection::Forward);
        assert_eq!(swap_direction(2, 0), SwapDirection::Back);
        assert_eq!(swap_direction(1, 1), SwapDirection::Still);
    }

    #[test]
    fn a_swap_starts_on_its_side_and_ends_in_place() {
        assert_eq!(swap_offset(SwapDirection::Forward, 0.), SWAP_TRAVEL);
        assert_eq!(swap_offset(SwapDirection::Back, 0.), -SWAP_TRAVEL);
        assert_eq!(swap_offset(SwapDirection::Forward, 1.), 0.);
        assert_eq!(swap_offset(SwapDirection::Still, 0.), 0.);
        // Eased: most of the way home by the middle.
        assert!(swap_offset(SwapDirection::Forward, 0.5) < SWAP_TRAVEL / 4.);
    }

    #[test]
    fn a_short_map_shows_every_version() {
        assert_eq!(
            map_slots(3, 1, MAP_SLOTS),
            [Slot::Version(0), Slot::Version(1), Slot::Version(2)]
        );
        assert_eq!(map_slots(0, 0, MAP_SLOTS), []);
    }

    #[test]
    fn a_long_map_keeps_the_ends_and_the_neighbours() {
        assert_eq!(
            map_slots(20, 10, MAP_SLOTS),
            [
                Slot::Version(0),
                Slot::Gap,
                Slot::Version(9),
                Slot::Version(10),
                Slot::Version(11),
                Slot::Gap,
                Slot::Version(19),
            ]
        );
        assert_eq!(
            map_slots(20, 0, MAP_SLOTS),
            [
                Slot::Version(0),
                Slot::Version(1),
                Slot::Gap,
                Slot::Version(19)
            ]
        );
        assert_eq!(
            map_slots(20, 18, MAP_SLOTS),
            [
                Slot::Version(0),
                Slot::Gap,
                Slot::Version(17),
                Slot::Version(18),
                Slot::Version(19)
            ]
        );
    }

    #[test]
    fn arrows_step_the_map_and_stop_at_the_ends() {
        assert_eq!(map_step(1, 3, "right"), Some(2));
        assert_eq!(map_step(2, 3, "right"), Some(2));
        assert_eq!(map_step(0, 3, "left"), Some(0));
        assert_eq!(map_step(1, 3, "home"), Some(0));
        assert_eq!(map_step(0, 3, "end"), Some(2));
        assert_eq!(map_step(0, 3, "a"), None);
        assert_eq!(map_step(0, 0, "right"), None);
    }

    #[test]
    fn the_map_and_its_dots_say_where_they_are() {
        assert_eq!(
            map_words(1, 3, None),
            "Versions: 2 of 3. Left and Right switch."
        );
        assert_eq!(
            map_words(0, 2, Some("Shorter")),
            "Versions: 1 of 2, Shorter. Left and Right switch."
        );
        assert_eq!(dot_words(0, None, false), "Version 1");
        assert_eq!(
            dot_words(2, Some("With a table"), true),
            "Version 3: With a table, showing"
        );
        assert!(dot_width(true) > dot_width(false));
    }
}
