// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

#[test]
fn a_call_header_reads_tool_summary_and_state() {
    assert_eq!(
        call_name(
            "ledger.search",
            Some("Search March"),
            ToolCallStatus::Success,
            Some("1.4s")
        ),
        "ledger.search, Search March, done in 1.4s"
    );
    assert_eq!(
        call_name("rates.fetch", None, ToolCallStatus::Failed, Some("Failed")),
        "rates.fetch, failed"
    );
    assert_eq!(
        call_name("post", None, ToolCallStatus::Running, None),
        "post, running"
    );
}

#[test]
fn durations_read_in_the_right_unit() {
    assert_eq!(format_duration(Duration::from_millis(380), false), "380ms");
    assert_eq!(format_duration(Duration::from_millis(380), true), "0.3s");
    assert_eq!(format_duration(Duration::ZERO, false), "1ms");
    assert_eq!(
        format_duration(Duration::from_millis(12_340), false),
        "12.3s"
    );
    assert_eq!(format_duration(Duration::from_secs(125), false), "2m 05s");
    assert_eq!(
        format_duration(Duration::from_secs(3 * 3600 + 12 * 60), false),
        "3h 12m"
    );
    assert_eq!(
        format_duration(Duration::from_secs(52 * 3600), false),
        "2d 4h"
    );
}

#[test]
fn json_is_laid_out_one_value_per_line() {
    let pretty = pretty_json(r#"{"query":"vat, 2026","limit":5,"tags":[],"deep":{"a":[1,2]}}"#);
    let expected = [
        "{",
        "  \"query\": \"vat, 2026\",",
        "  \"limit\": 5,",
        "  \"tags\": [],",
        "  \"deep\": {",
        "    \"a\": [",
        "      1,",
        "      2",
        "    ]",
        "  }",
        "}",
    ]
    .join("\n");
    assert_eq!(pretty.as_deref(), Some(expected.as_str()));
}

#[test]
fn text_that_is_not_json_is_left_alone() {
    assert_eq!(pretty_json("3 rows updated"), None);
    assert_eq!(pretty_json("{\"open\": [1, 2"), None);
    assert_eq!(pretty_json("]"), None);
    // Brackets and quotes inside strings do not count.
    assert_eq!(
        pretty_json(r#"["a \"}\" b"]"#).as_deref(),
        Some("[\n  \"a \\\"}\\\" b\"\n]")
    );
}

#[test]
fn a_preview_is_one_compact_line() {
    assert_eq!(
        one_line(r#"{"posted":true,"tags":["a","b"]}"#, 80),
        r#"{"posted": true, "tags": ["a", "b"]}"#
    );
    assert_eq!(one_line("3 rows\n  updated", 80), "3 rows updated");
    assert_eq!(one_line("abcdefghij", 4), "abcd\u{2026}");
}

#[test]
fn a_result_says_how_much_came_back() {
    assert_eq!(
        result_size(r#"{"a":1,"b":{"c":2,"d":3}}"#).as_deref(),
        Some("2 fields")
    );
    assert_eq!(
        result_size(r#"[1,[2,3],"x,y"]"#).as_deref(),
        Some("3 items")
    );
    assert_eq!(result_size("[]").as_deref(), Some("0 items"));
    assert_eq!(result_size(r#"{"only":1}"#).as_deref(), Some("1 field"));
    assert_eq!(result_size("one\ntwo").as_deref(), Some("2 lines"));
    assert_eq!(result_size("0.9214"), None);
}

#[test]
fn folded_arguments_name_their_fields() {
    assert_eq!(
        field_names(r#"{"period":"2026-03","filter":{"status":"x"},"limit":50}"#),
        ["period", "filter", "limit"]
    );
    assert_eq!(field_names(r#"{"say \"hi\"":1}"#), ["say \"hi\""]);
    assert!(field_names("[1, 2]").is_empty());
    assert!(field_names("plain").is_empty());
    assert_eq!(arguments_words(false, 3), "Arguments, 3 fields, collapsed");
    assert_eq!(arguments_words(true, 0), "Arguments, expanded");
}

#[test]
fn a_failed_call_opens_and_closes_by_itself() {
    use ToolCallStatus::{Failed, Running, Success};
    // Failing opens a closed card and remembers that it did.
    assert_eq!(
        after_status_change(Running, Failed, false, false),
        (true, true)
    );
    // The next run closes it again.
    assert_eq!(
        after_status_change(Failed, Running, true, true),
        (false, false)
    );
    // A card someone opened by hand stays open.
    assert_eq!(
        after_status_change(Failed, Running, true, false),
        (true, false)
    );
    assert_eq!(
        after_status_change(Running, Success, false, false),
        (false, false)
    );
}

#[test]
fn a_group_adds_up_only_complete_durations() {
    let second = Some(Duration::from_secs(1));
    assert_eq!(
        total_duration(&[second, second]),
        Some(Duration::from_secs(2))
    );
    assert_eq!(total_duration(&[second, None]), None);
    assert_eq!(total_duration(&[]), None);
    assert_eq!(count_label(1), "Used 1 tool");
    assert_eq!(count_label(3), "Used 3 tools");
}
