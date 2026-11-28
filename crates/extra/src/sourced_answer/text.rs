// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The answer as text: paragraphs with their markers, the words a quote
//! shares with the answer, and the words the panel and keys use.

use std::ops::Range;

use isonomic_ui::SharedString;

use super::{AnswerSegment, AnswerSource};

/// A numbered marker inside a paragraph.
#[derive(Debug, PartialEq)]
pub(super) struct Marker {
    pub(super) range: Range<usize>,
    pub(super) segment: usize,
    pub(super) source: usize,
}

/// One paragraph as a single run of text, with the places that matter in it.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Paragraph {
    pub(super) text: String,
    /// Each segment's words, without the spaces at their edges.
    pub(super) sentences: Vec<(Range<usize>, usize)>,
    pub(super) markers: Vec<Marker>,
}

/// Lay the segments out as paragraphs and put a marker after each citation.
/// A marker is tied to the last word before it by non-breaking spaces, so it
/// never starts a line on its own.
pub(super) fn paragraphs(segments: &[AnswerSegment], sources: &[AnswerSource]) -> Vec<Paragraph> {
    let mut out: Vec<Paragraph> = Vec::new();
    for (ix, segment) in segments.iter().enumerate() {
        if segment.new_paragraph || out.is_empty() {
            out.push(Paragraph::default());
        }
        let Some(paragraph) = out.last_mut() else {
            continue;
        };
        let text = segment.text.as_ref();
        let words = text.trim();
        if words.is_empty() {
            paragraph.text.push_str(text);
            continue;
        }
        paragraph
            .text
            .push_str(&text[..text.len() - text.trim_start().len()]);
        let start = paragraph.text.len();
        paragraph.text.push_str(words);
        paragraph.sentences.push((start..paragraph.text.len(), ix));
        for cite in &segment.cites {
            let Some(source) = sources.iter().position(|source| &source.id == cite) else {
                continue;
            };
            paragraph.text.push('\u{a0}');
            let start = paragraph.text.len();
            paragraph
                .text
                .push_str(&format!("\u{a0}{}\u{a0}", source + 1));
            paragraph.markers.push(Marker {
                range: start..paragraph.text.len(),
                segment: ix,
                source,
            });
        }
        paragraph.text.push_str(&text[text.trim_end().len()..]);
    }
    out
}

/// The byte ranges of the words in a text.
fn words(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (ix, ch) in text.char_indices() {
        if ch.is_alphanumeric() {
            start.get_or_insert(ix);
        } else if let Some(from) = start.take() {
            out.push(from..ix);
        }
    }
    if let Some(from) = start {
        out.push(from..text.len());
    }
    out
}

/// Whether a word says enough to be worth marking: a longer word or a number.
fn is_telling(word: &str) -> bool {
    word.chars().count() >= 5 || word.chars().all(|ch| ch.is_ascii_digit())
}

/// The parts of a passage to mark: the given phrases, or failing those the
/// telling words it shares with the sentences that cite it.
pub(super) fn quote_marks(
    quote: &str,
    phrases: &[SharedString],
    citing: &str,
) -> Vec<Range<usize>> {
    let mut marks: Vec<Range<usize>> = if phrases.is_empty() {
        let known: Vec<String> = words(citing)
            .into_iter()
            .map(|range| citing[range].to_lowercase())
            .filter(|word| is_telling(word))
            .collect();
        words(quote)
            .into_iter()
            .filter(|range| {
                let word = quote[range.clone()].to_lowercase();
                is_telling(&word) && known.contains(&word)
            })
            .collect()
    } else {
        phrases
            .iter()
            .filter(|phrase| !phrase.is_empty())
            .flat_map(|phrase| {
                quote
                    .match_indices(phrase.as_ref())
                    .map(|(at, found)| at..at + found.len())
            })
            .collect()
    };
    marks.sort_by_key(|range| range.start);

    // Join marks that overlap or have nothing but spaces between them.
    let mut joined: Vec<Range<usize>> = Vec::new();
    for mark in marks {
        match joined.last_mut() {
            Some(last)
                if mark.start <= last.end || quote[last.end..mark.start].trim().is_empty() =>
            {
                last.end = last.end.max(mark.end);
            }
            _ => joined.push(mark),
        }
    }
    joined
}

/// The chip an arrow key moves to from `current`, among `count` chips.
pub(super) fn neighbour(current: usize, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match key {
        "right" => Some((current + 1) % count),
        "left" => Some((current + count - 1) % count),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

/// How many sentences cite a source, in words.
pub(super) fn cited_words(count: usize) -> String {
    match count {
        0 => "not cited".to_string(),
        1 => "cited in 1 sentence".to_string(),
        n => format!("cited in {n} sentences"),
    }
}

/// The start of a passage for a preview: at most `max` characters, cut at
/// the last whole word, with an ellipsis when anything was left out.
pub(super) fn snippet(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    // A cut that lands on a space already ends a whole word.
    let ends_word = text.chars().nth(max).is_some_and(char::is_whitespace);
    let cut = match cut.rfind(' ') {
        Some(at) if !ends_word && at > max / 2 => &cut[..at],
        _ => cut.as_str(),
    };
    format!(
        "{}\u{2026}",
        cut.trim_end_matches([',', ';', ':', '.', ' '])
    )
}

/// The panel row an arrow key moves to: Up and Down stop at the ends.
pub(super) fn panel_step(current: usize, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match key {
        "down" => Some((current + 1).min(count - 1)),
        "up" => Some(current.saturating_sub(1)),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

/// What a screen reader hears for a panel row.
pub(super) fn panel_row_words(
    number: usize,
    name: &str,
    about: &str,
    cited: usize,
    open: bool,
) -> String {
    let mut words = format!("Source {number}: {name}");
    if !about.is_empty() {
        words.push_str(", ");
        words.push_str(about);
    }
    words.push_str(", ");
    words.push_str(&cited_words(cited));
    if open {
        words.push_str(", passage open");
    }
    words
}

/// The letter that stands for a source on its badge.
pub(super) fn initial(name: &str) -> String {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}
