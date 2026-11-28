// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

#[test]
fn risks_and_decisions_have_words() {
    assert_eq!(risk_tone(RequestRisk::High).1, "High risk");
    assert_eq!(decision_words(StackDecision::Edit), "Sent back to edit");
    assert!(RequestRisk::High > RequestRisk::Low);
}

fn queue() -> Vec<PendingRequest> {
    vec![
        PendingRequest::new("a", "x", "Read a file", RequestRisk::Low),
        PendingRequest::new("b", "x", "Pay a bill", RequestRisk::High),
        PendingRequest::new("c", "x", "Send mail", RequestRisk::Medium),
        PendingRequest::new("d", "x", "Delete a branch", RequestRisk::High),
    ]
}

#[test]
fn triage_puts_the_riskiest_first_and_keeps_arrival_order() {
    assert_eq!(triage(&queue(), None), [1, 3, 2, 0]);
    assert_eq!(triage(&queue(), Some(RequestRisk::High)), [1, 3]);
    assert_eq!(triage(&queue(), Some(RequestRisk::Low)), [0]);
    assert_eq!(risk_counts(&queue()), [2, 1, 1]);
}

#[test]
fn the_filter_steps_through_every_risk_and_back() {
    let mut filter = None;
    let mut seen = Vec::new();
    for _ in 0..4 {
        filter = next_filter(filter);
        seen.push(filter);
    }
    assert_eq!(
        seen,
        [
            Some(RequestRisk::High),
            Some(RequestRisk::Medium),
            Some(RequestRisk::Low),
            None
        ]
    );
}

#[test]
fn the_cursor_stops_at_the_ends() {
    assert_eq!(queue_step(None, 3, "down"), Some(0));
    assert_eq!(queue_step(Some(0), 3, "down"), Some(1));
    assert_eq!(queue_step(Some(2), 3, "down"), Some(2));
    assert_eq!(queue_step(Some(0), 3, "up"), Some(0));
    assert_eq!(queue_step(Some(1), 3, "end"), Some(2));
    assert_eq!(queue_step(Some(1), 3, "x"), None);
    assert_eq!(queue_step(None, 0, "down"), None);
}

#[test]
fn picking_toggles_one_or_all() {
    let shown: Vec<SharedString> = vec!["b".into(), "d".into()];
    let mut picked = Vec::new();
    toggle_pick(&mut picked, &"b".into());
    assert_eq!(picked, ["b"]);
    toggle_all(&mut picked, &shown);
    assert_eq!(picked, ["b", "d"]);
    // All shown were picked, so the second press clears them.
    toggle_all(&mut picked, &shown);
    assert!(picked.is_empty());
    toggle_pick(&mut picked, &"b".into());
    toggle_pick(&mut picked, &"b".into());
    assert!(picked.is_empty());
}

#[test]
fn actions_apply_to_the_picked_rows_or_the_cursor() {
    let shown: Vec<SharedString> = vec!["b".into(), "d".into(), "c".into()];
    let picked: Vec<SharedString> = vec!["c".into(), "b".into(), "gone".into()];
    // Picked rows in the order shown; ones not shown are left alone.
    assert_eq!(targets(&picked, &shown, Some(&"d".into())), ["b", "c"]);
    assert_eq!(targets(&[], &shown, Some(&"d".into())), ["d"]);
    assert!(targets(&[], &shown, None).is_empty());
}

#[test]
fn grants_and_rows_read_in_words() {
    let one: Vec<SharedString> = vec!["Pay a bill".into()];
    let three: Vec<SharedString> = vec!["a".into(), "b".into(), "c".into()];
    assert_eq!(
        grant_note(&one, GrantScope::Session),
        "Allowed for this session: Pay a bill"
    );
    assert_eq!(
        grant_note(&three, GrantScope::Always),
        "Allowed 3 requests always"
    );
    assert_eq!(reject_note(&three), "Rejected 3 requests");
    assert_eq!(GrantScope::of_key("w"), Some(GrantScope::Always));
    assert_eq!(GrantScope::of_key("q"), None);
    let request =
        PendingRequest::new("b", "Payables", "Pay a bill", RequestRisk::High).asked("2 min ago");
    assert_eq!(
        row_words(&request, true),
        "Pay a bill, high risk, from Payables, 2 min ago, picked"
    );
}

#[test]
fn requests_collect_details() {
    let request = PendingRequest::new("a", "Payables", "Pay a bill", RequestRisk::High)
        .detail("Amount", "$1,240.50")
        .detail("To", "Harbour Supply")
        .asked("3 min ago");
    assert_eq!(request.details.len(), 2);
    assert_eq!(request.asked.as_ref(), "3 min ago");
}
