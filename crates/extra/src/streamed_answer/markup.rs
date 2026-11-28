// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The answer's short markup: paragraphs, list items, bold and citations,
//! how many steps each part takes to stream in, and the plain text a copy
//! writes.

use super::AnswerSource;

/// A piece of a line of the answer.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Inline {
    Text {
        text: String,
        bold: bool,
    },
    /// Cited sources, counted from 0.
    Cite(Vec<usize>),
}

/// A paragraph or a list item.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Block {
    pub(super) item: bool,
    pub(super) inlines: Vec<Inline>,
}

/// Read a line's bold markers and citations.
pub(super) fn parse_line(line: &str) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let mut bold = false;
    let mut text = String::new();
    let flush = |text: &mut String, bold: bool, out: &mut Vec<Inline>| {
        if !text.is_empty() {
            out.push(Inline::Text {
                text: std::mem::take(text),
                bold,
            });
        }
    };
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            flush(&mut text, bold, &mut out);
            bold = !bold;
            rest = after;
            continue;
        }
        if rest.starts_with('[')
            && let Some(close) = rest.find(']')
        {
            let inside = &rest[1..close];
            let numbers: Option<Vec<usize>> = inside
                .split(',')
                .map(|part| part.trim().parse::<usize>().ok().filter(|n| *n > 0))
                .collect();
            if let Some(numbers) = numbers.filter(|n| !n.is_empty()) {
                // A citation hugs the word before it, so the space goes.
                let trimmed = text.trim_end().len();
                text.truncate(trimmed);
                flush(&mut text, bold, &mut out);
                out.push(Inline::Cite(numbers.into_iter().map(|n| n - 1).collect()));
                rest = &rest[close + 1..];
                continue;
            }
        }
        let next = rest.chars().next().map_or(1, char::len_utf8);
        text.push_str(&rest[..next]);
        rest = &rest[next..];
    }
    flush(&mut text, bold, &mut out);
    out
}

/// Split the answer into paragraphs and list items.
pub(super) fn parse_answer(answer: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para = String::new();
    let end_para = |para: &mut String, blocks: &mut Vec<Block>| {
        if !para.trim().is_empty() {
            blocks.push(Block {
                item: false,
                inlines: parse_line(para.trim()),
            });
        }
        para.clear();
    };
    for line in answer.lines() {
        let line = line.trim();
        if line.is_empty() {
            end_para(&mut para, &mut blocks);
        } else if let Some(item) = line.strip_prefix("- ") {
            end_para(&mut para, &mut blocks);
            blocks.push(Block {
                item: true,
                inlines: parse_line(item.trim()),
            });
        } else {
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(line);
        }
    }
    end_para(&mut para, &mut blocks);
    blocks
}

/// The steps an inline takes to stream in: a word each, or one for a chip.
pub(super) fn steps(inline: &Inline) -> usize {
    match inline {
        Inline::Text { text, .. } => text.split_inclusive(' ').count(),
        Inline::Cite(_) => 1,
    }
}

pub(super) fn total_steps(blocks: &[Block]) -> usize {
    blocks.iter().flat_map(|b| &b.inlines).map(steps).sum()
}

/// Add a word or chip to the runs that wrap as one. A part joins the run
/// before it when no space came between them, so a citation never wraps
/// away from its word or the full stop after it.
pub(super) fn place(
    runs: &mut Vec<Vec<isonomic_ui::AnyElement>>,
    glue: &mut bool,
    part: isonomic_ui::AnyElement,
    open_end: bool,
) {
    match runs.last_mut() {
        Some(run) if *glue => run.push(part),
        _ => runs.push(vec![part]),
    }
    *glue = open_end;
}

/// The answer as plain text, with a numbered list of its sources.
pub(super) fn plain_text(blocks: &[Block], sources: &[AnswerSource]) -> String {
    let mut out = String::new();
    for (ix, block) in blocks.iter().enumerate() {
        if ix > 0 {
            out.push_str(if block.item && blocks[ix - 1].item {
                "\n"
            } else {
                "\n\n"
            });
        }
        if block.item {
            out.push_str("- ");
        }
        for inline in &block.inlines {
            match inline {
                Inline::Text { text, .. } => out.push_str(text),
                Inline::Cite(numbers) => {
                    let list: Vec<String> = numbers.iter().map(|n| (n + 1).to_string()).collect();
                    out.push_str(&format!(" [{}]", list.join(",")));
                }
            }
        }
    }
    if !sources.is_empty() {
        out.push_str("\n\nSources:");
        for (ix, source) in sources.iter().enumerate() {
            out.push_str(&format!("\n{}. {}", ix + 1, source.title));
            if let Some(domain) = &source.domain {
                out.push_str(&format!(" ({domain})"));
            }
        }
    }
    out
}

/// How many steps the reasoning takes to stream in: a word each.
pub(super) fn reasoning_steps(text: &str) -> usize {
    text.split_whitespace().count()
}

/// The first `words` words of the reasoning, its paragraphs kept apart.
pub(super) fn reasoning_cut(text: &str, words: usize) -> Vec<String> {
    let mut left = words;
    let mut out = Vec::new();
    for para in text.split("\n\n") {
        if left == 0 {
            break;
        }
        let kept: Vec<&str> = para.split_whitespace().take(left).collect();
        if kept.is_empty() {
            continue;
        }
        left -= kept.len();
        out.push(kept.join(" "));
    }
    out
}

/// What the reasoning's toggle says: "Thinking" while it streams, then how
/// long it took, or plain "Reasoning" when that is not known.
pub(super) fn thought_words(took: Option<std::time::Duration>, thinking: bool) -> String {
    match (thinking, took) {
        (true, _) => "Thinking".into(),
        (false, Some(took)) if took.as_secs_f32() < 60. => {
            format!("Thought for {:.0}s", took.as_secs_f32().max(1.))
        }
        (false, Some(took)) => {
            let secs = took.as_secs();
            format!("Thought for {}m {}s", secs / 60, secs % 60)
        }
        (false, None) => "Reasoning".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read_bold_and_citations() {
        assert_eq!(
            parse_line("**Rent** rose [2] and fees [1, 3]. See [x] or [0]."),
            vec![
                Inline::Text {
                    text: "Rent".into(),
                    bold: true
                },
                Inline::Text {
                    text: " rose".into(),
                    bold: false
                },
                Inline::Cite(vec![1]),
                Inline::Text {
                    text: " and fees".into(),
                    bold: false
                },
                Inline::Cite(vec![0, 2]),
                Inline::Text {
                    text: ". See [x] or [0].".into(),
                    bold: false
                },
            ]
        );
    }

    #[test]
    fn answers_split_into_paragraphs_and_items() {
        let blocks = parse_answer("One\ntwo.\n\n- first\n- second\nAfter");
        assert_eq!(blocks.len(), 4);
        assert!(!blocks[0].item && blocks[1].item && blocks[2].item && !blocks[3].item);
        assert_eq!(
            blocks[0].inlines,
            vec![Inline::Text {
                text: "One two.".into(),
                bold: false
            }]
        );
        assert_eq!(total_steps(&blocks), 2 + 1 + 1 + 1);
    }

    #[test]
    fn copies_as_plain_text_with_sources() {
        let blocks = parse_answer("**Yes** [1].\n\n- a\n- b");
        let sources = [AnswerSource::new("Ledger export", "Ledger").domain("ledger.example")];
        assert_eq!(
            plain_text(&blocks, &sources),
            "Yes [1].\n\n- a\n- b\n\nSources:\n1. Ledger export (ledger.example)"
        );
    }

    #[test]
    fn reasoning_streams_by_the_word_and_keeps_its_paragraphs() {
        let text = "Check the rent.\n\nThen the  fees and the tax.";
        assert_eq!(reasoning_steps(text), 9);
        assert_eq!(reasoning_cut(text, 0), Vec::<String>::new());
        assert_eq!(reasoning_cut(text, 2), ["Check the"]);
        assert_eq!(reasoning_cut(text, 5), ["Check the rent.", "Then the"]);
        assert_eq!(reasoning_cut(text, 99).len(), 2);
    }

    #[test]
    fn the_reasoning_toggle_says_how_long_it_thought() {
        use std::time::Duration;
        assert_eq!(thought_words(None, true), "Thinking");
        assert_eq!(
            thought_words(Some(Duration::from_millis(300)), false),
            "Thought for 1s"
        );
        assert_eq!(
            thought_words(Some(Duration::from_secs(12)), false),
            "Thought for 12s"
        );
        assert_eq!(
            thought_words(Some(Duration::from_secs(75)), false),
            "Thought for 1m 15s"
        );
        assert_eq!(thought_words(None, false), "Reasoning");
    }
}
