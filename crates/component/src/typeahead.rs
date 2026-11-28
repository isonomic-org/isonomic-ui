// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typeahead: typing letters in an open list moves to the item whose label
//! begins with them.
//!
//! Letters typed within a second of each other build one query, matched from
//! the current item onward. Pressing the same letter again steps through the
//! items that start with it. A pause longer than [`TYPEAHEAD_RESET`] starts a new query.
//! Menus and selects share this, so they answer the keyboard the same way.

use std::time::Duration;

use instant::Instant;

use gpui::Keystroke;

/// How long a pause starts a new query.
pub const TYPEAHEAD_RESET: Duration = Duration::from_millis(1000);

/// The letters typed so far, and when the last one came.
#[derive(Clone, Debug, Default)]
pub struct Typeahead {
    query: String,
    last: Option<Instant>,
}

impl Typeahead {
    /// Add a typed character at `now`, starting over after a pause, and
    /// return the query to match.
    pub fn push(&mut self, ch: char, now: Instant) -> &str {
        let stale = self
            .last
            .is_none_or(|last| now.saturating_duration_since(last) > TYPEAHEAD_RESET);
        if stale {
            self.query.clear();
        }
        self.query.push(ch);
        self.last = Some(now);
        &self.query
    }

    /// Forget the query, such as when the list closes.
    pub fn clear(&mut self) {
        self.query.clear();
        self.last = None;
    }

    /// Whether letters typed now would add to a query in progress, so that
    /// a space belongs to it instead of choosing the item.
    pub fn in_progress(&self, now: Instant) -> bool {
        !self.query.is_empty()
            && self
                .last
                .is_some_and(|last| now.saturating_duration_since(last) <= TYPEAHEAD_RESET)
    }
}

/// The character a key press types for typeahead, or `None` for a key that
/// is a command: anything held with control, alt, the platform key or the
/// function key, and keys that type nothing or only whitespace. A space is
/// typed only while `in_progress`.
pub fn typeahead_char(keystroke: &Keystroke, in_progress: bool) -> Option<char> {
    let modifiers = &keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    let typed = keystroke.key_char.as_deref().or_else(|| {
        // A test keystroke may carry only its key.
        match keystroke.key.as_str() {
            "space" => Some(" "),
            key => (key.chars().count() == 1).then_some(key),
        }
    })?;
    let mut chars = typed.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_control() {
        return None;
    }
    if ch.is_whitespace() && !(ch == ' ' && in_progress) {
        return None;
    }
    Some(ch)
}

/// The item a query lands on. `labels` holds one entry per item, `None` for
/// one that cannot be chosen (a separator, a heading, a disabled item).
///
/// The search starts at the current item, so typing more letters keeps the
/// match where it is while it still fits. A query that is one letter typed
/// again and again ("ccc") steps on to the next item starting with it
/// instead, wrapping at the end. Matching ignores case.
pub fn typeahead_match(
    labels: &[Option<&str>],
    current: Option<usize>,
    query: &str,
) -> Option<usize> {
    let total = labels.len();
    if total == 0 || query.is_empty() {
        return None;
    }
    let query = query.to_lowercase();
    let mut chars = query.chars();
    let first = chars.next()?;
    let repeated = query.chars().count() > 1 && chars.all(|ch| ch == first);
    let (needle, start) = if repeated || query.chars().count() == 1 {
        // A fresh letter, or the same one again: move past the current item.
        (first.to_string(), current.map_or(0, |ix| ix + 1))
    } else {
        (query.clone(), current.unwrap_or(0))
    };
    (0..total)
        .map(|offset| (start + offset) % total)
        .find(|ix| {
            labels[*ix].is_some_and(|label| label.trim_start().to_lowercase().starts_with(&needle))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABELS: [Option<&str>; 6] = [
        Some("Rename"),
        Some("Refresh"),
        None,
        Some("Copy link"),
        Some("Copy as text"),
        Some("Delete"),
    ];

    #[test]
    fn a_letter_moves_to_the_next_item_starting_with_it() {
        assert_eq!(typeahead_match(&LABELS, None, "c"), Some(3));
        assert_eq!(typeahead_match(&LABELS, Some(3), "c"), Some(4));
        // Wraps round.
        assert_eq!(typeahead_match(&LABELS, Some(4), "c"), Some(3));
        assert_eq!(typeahead_match(&LABELS, Some(0), "D"), Some(5));
        assert_eq!(typeahead_match(&LABELS, None, "x"), None);
    }

    #[test]
    fn repeating_a_letter_cycles_through_its_items() {
        assert_eq!(typeahead_match(&LABELS, Some(0), "rr"), Some(1));
        assert_eq!(typeahead_match(&LABELS, Some(1), "rrr"), Some(0));
    }

    #[test]
    fn more_letters_narrow_from_where_the_match_is() {
        assert_eq!(typeahead_match(&LABELS, Some(0), "ref"), Some(1));
        assert_eq!(typeahead_match(&LABELS, Some(3), "copy a"), Some(4));
        assert_eq!(typeahead_match(&LABELS, Some(3), "copy l"), Some(3));
    }

    #[test]
    fn items_that_cannot_be_chosen_are_skipped() {
        assert_eq!(
            typeahead_match(&[None, Some("Archive")], None, "a"),
            Some(1)
        );
        assert_eq!(typeahead_match(&[], None, "a"), None);
        assert_eq!(typeahead_match(&LABELS, None, ""), None);
    }

    #[test]
    fn a_pause_starts_a_new_query() {
        let start = Instant::now();
        let mut typeahead = Typeahead::default();
        assert_eq!(typeahead.push('c', start), "c");
        assert_eq!(
            typeahead.push('o', start + Duration::from_millis(300)),
            "co"
        );
        assert!(typeahead.in_progress(start + Duration::from_millis(900)));
        assert_eq!(
            typeahead.push('d', start + Duration::from_millis(1500)),
            "d"
        );
        typeahead.clear();
        assert!(!typeahead.in_progress(start + Duration::from_millis(1500)));
    }

    #[test]
    fn only_plain_typing_counts() {
        let key = |text: &'static str| Keystroke::parse(text).unwrap();
        assert_eq!(typeahead_char(&key("a"), false), Some('a'));
        assert_eq!(typeahead_char(&key("ctrl-a"), false), None);
        assert_eq!(typeahead_char(&key("cmd-a"), false), None);
        assert_eq!(typeahead_char(&key("alt-a"), false), None);
        assert_eq!(typeahead_char(&key("enter"), false), None);
        assert_eq!(typeahead_char(&key("down"), false), None);
        assert_eq!(typeahead_char(&key("space"), false), None);
        assert_eq!(typeahead_char(&key("space"), true), Some(' '));
    }
}
