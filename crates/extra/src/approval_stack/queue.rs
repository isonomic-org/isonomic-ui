// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The triage queue's pure parts: the order by risk, picking, the cursor's
//! steps, the words a screen reader hears, and handing decisions to the owner.

use isonomic_ui::{App, SharedString, Window};

use super::{
    DecideHandler, GrantHandler, GrantScope, ManyHandler, PendingRequest, RequestRisk,
    StackDecision,
};
use crate::common::Tone;

pub(super) fn risk_tone(risk: RequestRisk) -> (Tone, &'static str) {
    match risk {
        RequestRisk::Low => (Tone::Success, "Low risk"),
        RequestRisk::Medium => (Tone::Warning, "Medium risk"),
        RequestRisk::High => (Tone::Danger, "High risk"),
    }
}

/// The requests a queue shows: those at the filter's risk, or all of them,
/// riskiest first and otherwise in the order they came.
pub(super) fn triage(requests: &[PendingRequest], filter: Option<RequestRisk>) -> Vec<usize> {
    let mut shown: Vec<usize> = (0..requests.len())
        .filter(|ix| filter.is_none_or(|risk| requests[*ix].risk == risk))
        .collect();
    shown.sort_by_key(|ix| std::cmp::Reverse(requests[*ix].risk));
    shown
}

/// How many requests wait at each risk, high first.
pub(super) fn risk_counts(requests: &[PendingRequest]) -> [usize; 3] {
    let count = |risk| requests.iter().filter(|r| r.risk == risk).count();
    [
        count(RequestRisk::High),
        count(RequestRisk::Medium),
        count(RequestRisk::Low),
    ]
}

/// The filter after the current one: all, then high, medium and low.
pub(super) fn next_filter(filter: Option<RequestRisk>) -> Option<RequestRisk> {
    match filter {
        None => Some(RequestRisk::High),
        Some(RequestRisk::High) => Some(RequestRisk::Medium),
        Some(RequestRisk::Medium) => Some(RequestRisk::Low),
        Some(RequestRisk::Low) => None,
    }
}

/// The row an arrow key moves the queue's cursor to; it stops at the ends.
pub(super) fn queue_step(current: Option<usize>, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let at = current.unwrap_or(0).min(count - 1);
    match key {
        "down" => Some(if current.is_none() {
            0
        } else {
            (at + 1).min(count - 1)
        }),
        "up" => Some(at.saturating_sub(1)),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

/// Pick a request, or put it back.
pub(super) fn toggle_pick(picked: &mut Vec<SharedString>, id: &SharedString) {
    match picked.iter().position(|p| p == id) {
        Some(at) => {
            picked.remove(at);
        }
        None => picked.push(id.clone()),
    }
}

/// Pick every shown request, or none when all of them are picked already.
pub(super) fn toggle_all(picked: &mut Vec<SharedString>, shown: &[SharedString]) {
    if !shown.is_empty() && shown.iter().all(|id| picked.contains(id)) {
        picked.retain(|id| !shown.contains(id));
    } else {
        for id in shown {
            if !picked.contains(id) {
                picked.push(id.clone());
            }
        }
    }
}

/// What a queue action applies to: the picked requests that are shown, in
/// the order shown, or else the row under the cursor.
pub(super) fn targets(
    picked: &[SharedString],
    shown: &[SharedString],
    cursor: Option<&SharedString>,
) -> Vec<SharedString> {
    let chosen: Vec<SharedString> = shown
        .iter()
        .filter(|id| picked.contains(id))
        .cloned()
        .collect();
    if chosen.is_empty() {
        cursor.into_iter().cloned().collect()
    } else {
        chosen
    }
}

/// What a screen reader hears for a queue row.
pub(super) fn row_words(request: &PendingRequest, picked: bool) -> String {
    let mut words = format!(
        "{}, {}, from {}",
        request.action,
        risk_tone(request.risk).1.to_lowercase(),
        request.from
    );
    if !request.asked.is_empty() {
        words.push_str(&format!(", {}", request.asked));
    }
    words.push_str(if picked { ", picked" } else { ", not picked" });
    words
}

/// Hand a grant to the owner: in one call when it takes them together, or
/// as approvals one by one.
pub(super) fn send_grant(
    ids: &[SharedString],
    scope: GrantScope,
    on_grant: &Option<GrantHandler>,
    on_decide: &Option<DecideHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if ids.is_empty() {
        return;
    }
    match (on_grant, on_decide) {
        (Some(grant), _) => grant(ids, scope, window, cx),
        (None, Some(decide)) => {
            for id in ids {
                decide(id, StackDecision::Approve, window, cx);
            }
        }
        (None, None) => {}
    }
}

/// Hand a rejection of many to the owner.
pub(super) fn send_reject(
    ids: &[SharedString],
    on_reject_many: &Option<ManyHandler>,
    on_decide: &Option<DecideHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if ids.is_empty() {
        return;
    }
    match (on_reject_many, on_decide) {
        (Some(reject), _) => reject(ids, window, cx),
        (None, Some(decide)) => {
            for id in ids {
                decide(id, StackDecision::Reject, window, cx);
            }
        }
        (None, None) => {}
    }
}

/// "1 request" or "3 requests".
pub(super) fn count_words(count: usize) -> String {
    match count {
        1 => "1 request".to_string(),
        n => format!("{n} requests"),
    }
}
