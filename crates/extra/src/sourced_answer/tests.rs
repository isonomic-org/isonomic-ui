// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

fn sources() -> Vec<AnswerSource> {
    vec![
        AnswerSource::new("ledger", "Ledger"),
        AnswerSource::new("contract", "Contract"),
    ]
}

#[test]
fn markers_are_numbered_by_source_order() {
    let segments = [
        AnswerSegment::new("Freight went up.").cite("contract"),
        AnswerSegment::new(" Sales held.")
            .cite("ledger")
            .cite("contract"),
    ];
    let laid_out = paragraphs(&segments, &sources());
    assert_eq!(laid_out.len(), 1);
    let paragraph = &laid_out[0];
    assert_eq!(
        paragraph.text,
        "Freight went up.\u{a0}\u{a0}2\u{a0} Sales held.\u{a0}\u{a0}1\u{a0}\u{a0}\u{a0}2\u{a0}"
    );
    let numbers: Vec<_> = paragraph
        .markers
        .iter()
        .map(|marker| (marker.segment, marker.source))
        .collect();
    assert_eq!(numbers, [(0, 1), (1, 0), (1, 1)]);
    // The spaces at a segment's edges stay outside its sentence.
    let (range, _) = &paragraph.sentences[1];
    assert_eq!(&paragraph.text[range.clone()], "Sales held.");
}

#[test]
fn unknown_citations_leave_no_marker() {
    let segments = [AnswerSegment::new("Margins fell.").cite("nowhere")];
    let laid_out = paragraphs(&segments, &sources());
    assert_eq!(laid_out[0].text, "Margins fell.");
    assert!(laid_out[0].markers.is_empty());
}

#[test]
fn a_segment_can_start_a_paragraph() {
    let segments = [
        AnswerSegment::new("One."),
        AnswerSegment::new("Two.").new_paragraph(),
    ];
    let laid_out = paragraphs(&segments, &sources());
    assert_eq!(laid_out.len(), 2);
    assert_eq!(laid_out[1].sentences, [(0..4, 1)]);
}

#[test]
fn shared_telling_words_are_marked() {
    let quote = "The carrier raised its rate by 12 percent in March.";
    let citing = "Freight cost more because the carrier put its RATE up 12 percent.";
    let marked: Vec<_> = quote_marks(quote, &[], citing)
        .into_iter()
        .map(|range| &quote[range])
        .collect();
    // "rate" is too short to count on its own; "12 percent" joins up.
    assert_eq!(marked, ["carrier", "12 percent"]);
}

#[test]
fn given_phrases_win_over_shared_words() {
    let quote = "The carrier raised its rate by 12 percent in March.";
    let phrases = ["raised its rate".into()];
    let marked: Vec<_> = quote_marks(quote, &phrases, "carrier percent")
        .into_iter()
        .map(|range| &quote[range])
        .collect();
    assert_eq!(marked, ["raised its rate"]);
}

#[test]
fn arrows_wrap_around_the_sources() {
    assert_eq!(neighbour(2, 3, "right"), Some(0));
    assert_eq!(neighbour(0, 3, "left"), Some(2));
    assert_eq!(neighbour(1, 3, "home"), Some(0));
    assert_eq!(neighbour(1, 3, "end"), Some(2));
    assert_eq!(neighbour(1, 3, "a"), None);
    assert_eq!(neighbour(0, 0, "right"), None);
}

#[test]
fn cited_counts_read_in_words() {
    assert_eq!(cited_words(0), "not cited");
    assert_eq!(cited_words(1), "cited in 1 sentence");
    assert_eq!(cited_words(4), "cited in 4 sentences");
}

#[test]
fn a_snippet_cuts_at_a_whole_word() {
    assert_eq!(snippet("Short   passage.", 40), "Short passage.");
    let cut = snippet(
        "The carrier raised its rate by twelve percent in March.",
        30,
    );
    assert_eq!(cut, "The carrier raised its rate by\u{2026}");
    // One long word is cut where it has to be.
    assert_eq!(snippet("abcdefghij", 4), "abcd\u{2026}");
}

#[test]
fn panel_rows_stop_at_the_ends() {
    assert_eq!(panel_step(0, 3, "up"), Some(0));
    assert_eq!(panel_step(2, 3, "down"), Some(2));
    assert_eq!(panel_step(1, 3, "down"), Some(2));
    assert_eq!(panel_step(1, 3, "end"), Some(2));
    assert_eq!(panel_step(1, 3, "left"), None);
    assert_eq!(panel_step(0, 0, "down"), None);
}

#[test]
fn a_panel_row_says_what_and_how_often() {
    assert_eq!(
        panel_row_words(2, "Contract", "example.test · May 2", 3, true),
        "Source 2: Contract, example.test · May 2, cited in 3 sentences, passage open"
    );
    assert_eq!(
        panel_row_words(1, "Ledger", "", 0, false),
        "Source 1: Ledger, not cited"
    );
}

#[test]
fn a_badge_shows_the_first_letter() {
    assert_eq!(initial("ledger"), "L");
    assert_eq!(initial("  9 lives"), "9");
    assert_eq!(initial(""), "");
}
