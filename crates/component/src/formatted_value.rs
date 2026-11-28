// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Numbers, sizes, durations and times written the way people read them,
//! and an element that shows one.
//!
//! Every formatter is a plain function, so the same text can go into a
//! label, a table cell or a tooltip. [`FormattedValue`] shows the short form
//! and gives screen readers the full one: "1.2K" is read as "1,234".

use std::time::Duration;

use chrono::{DateTime, Utc};
use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
};

use crate::StyledExt as _;

/// Writes `value` with thousands separators and `decimals` places:
/// `format_number(1234567.891, 2)` is "1,234,567.89".
pub fn format_number(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    let fixed = format!("{:.*}", decimals, value.abs());
    let (whole, fraction) = match fixed.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (fixed.as_str(), None),
    };
    let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
    for (ix, ch) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let negative = value < 0. && fixed.chars().any(|c| c.is_ascii_digit() && c != '0');
    let sign = if negative { "−" } else { "" };
    match fraction {
        Some(fraction) => format!("{sign}{grouped}.{fraction}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Writes large numbers short: 950, 1.2K, 3.4M, 7.1B. Below 1,000 the value
/// is written whole.
pub fn format_compact(value: f64) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    let abs = value.abs();
    let sign = if value < 0. { "−" } else { "" };
    const UNITS: [(f64, &str); 4] = [(1e3, "K"), (1e6, "M"), (1e9, "B"), (1e12, "T")];
    if abs.round() < 1e3 {
        return format!("{sign}{}", format_number(abs.round(), 0));
    }
    // The largest unit the value reaches once rounded, so 999,999 reads
    // "1M" and not "1000K".
    let mut unit = 0;
    while unit + 1 < UNITS.len() && abs >= UNITS[unit + 1].0 * 0.9995 {
        unit += 1;
    }
    let (scaled, suffix) = (abs / UNITS[unit].0, UNITS[unit].1);
    // One decimal below 100 (1.2K, 12.5K), none above (125K).
    let text = if scaled < 100. {
        let one = format!("{:.1}", scaled);
        one.trim_end_matches(".0").to_string()
    } else {
        format!("{:.0}", scaled)
    };
    format!("{sign}{text}{suffix}")
}

/// Writes a byte count in binary units: "512 B", "1.5 KB", "3.2 GB".
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit + 1 < UNITS.len() {
        value /= 1024.;
        unit += 1;
    }
    let text = if value < 10. {
        format!("{:.1}", value).trim_end_matches(".0").to_string()
    } else {
        format!("{:.0}", value)
    };
    format!("{text} {}", UNITS[unit])
}

/// Writes a fraction as a percentage: `format_percent(0.4251, 1)` is "42.5%".
pub fn format_percent(fraction: f64, decimals: usize) -> String {
    if !fraction.is_finite() {
        return "—".into();
    }
    format!("{}%", format_number(fraction * 100., decimals))
}

/// Writes a duration in its two largest units: "45s", "4m 05s", "1h 04m",
/// "2d 3h".
pub fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let (days, hours, minutes, seconds) = (
        total / 86_400,
        (total % 86_400) / 3_600,
        (total % 3_600) / 60,
        total % 60,
    );
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else if total > 0 {
        format!("{seconds}s")
    } else {
        format!("{}ms", duration.as_millis())
    }
}

/// Writes how long ago `then` was, seen from `now`: "Just now", "5 min ago",
/// "3 h ago", "Yesterday", "4 days ago", and the date after a week. A time
/// in the future reads "In 5 min".
pub fn format_relative_time(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - then).num_seconds();
    let future = seconds < 0;
    let abs = seconds.unsigned_abs();
    let phrase = |amount: u64, unit: &str| {
        if future {
            format!("In {amount} {unit}")
        } else {
            format!("{amount} {unit} ago")
        }
    };
    match abs {
        0..=44 => "Just now".into(),
        45..=3_599 => phrase((abs / 60).max(1), "min"),
        3_600..=86_399 => phrase(abs / 3_600, "h"),
        86_400..=172_799 => (if future { "Tomorrow" } else { "Yesterday" }).into(),
        172_800..=604_799 => phrase(abs / 86_400, "days"),
        _ => then.format("%-d %b %Y").to_string(),
    }
}

/// Shows a short formatted value and gives screen readers the full one.
///
/// ```ignore
/// FormattedValue::new("followers", format_compact(12_345.))
///     .full_text("12,345 followers")
/// ```
#[derive(IntoElement)]
pub struct FormattedValue {
    id: ElementId,
    text: SharedString,
    full_text: Option<SharedString>,
    style: StyleRefinement,
}

impl FormattedValue {
    /// Show `text`, such as the output of one of this module's formatters.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            full_text: None,
            style: StyleRefinement::default(),
        }
    }

    /// What screen readers hear in place of the short text, such as
    /// "12,345 followers" for "12K".
    pub fn full_text(mut self, text: impl Into<SharedString>) -> Self {
        self.full_text = Some(text.into());
        self
    }
}

impl Styled for FormattedValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FormattedValue {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let spoken = self.full_text.unwrap_or_else(|| self.text.clone());
        div()
            .id(self.id)
            .aria_label(spoken)
            .refine_style(&self.style)
            .child(self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    #[test]
    fn numbers_group_thousands() {
        assert_eq!(format_number(1_234_567.891, 2), "1,234,567.89");
        assert_eq!(format_number(999., 0), "999");
        assert_eq!(format_number(1000., 0), "1,000");
        assert_eq!(format_number(-2400., 2), "−2,400.00");
        assert_eq!(format_number(-0.001, 2), "0.00");
        assert_eq!(format_number(f64::NAN, 2), "—");
    }

    #[test]
    fn compact_numbers() {
        assert_eq!(format_compact(950.), "950");
        assert_eq!(format_compact(1_234.), "1.2K");
        assert_eq!(format_compact(1_000.), "1K");
        assert_eq!(format_compact(12_500.), "12.5K");
        assert_eq!(format_compact(125_000.), "125K");
        assert_eq!(format_compact(3_400_000.), "3.4M");
        assert_eq!(format_compact(-7_100_000_000.), "−7.1B");
        assert_eq!(format_compact(999_999.), "1M");
        assert_eq!(format_compact(999.6), "1K");
    }

    #[test]
    fn bytes_use_binary_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1 MB");
        assert_eq!(format_bytes(150 * 1024 * 1024), "150 MB");
        assert_eq!(format_bytes(3_435_973_837), "3.2 GB");
    }

    #[test]
    fn percentages() {
        assert_eq!(format_percent(0.4251, 1), "42.5%");
        assert_eq!(format_percent(1., 0), "100%");
    }

    #[test]
    fn durations_show_two_units() {
        assert_eq!(format_duration(Duration::from_millis(250)), "250ms");
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(245)), "4m 05s");
        assert_eq!(format_duration(Duration::from_secs(3_840)), "1h 04m");
        assert_eq!(format_duration(Duration::from_secs(183_600)), "2d 3h");
    }

    #[test]
    fn relative_times() {
        let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
        let ago = |secs: i64| format_relative_time(now - chrono::Duration::seconds(secs), now);
        assert_eq!(ago(10), "Just now");
        assert_eq!(ago(60 * 5), "5 min ago");
        assert_eq!(ago(3_600 * 3), "3 h ago");
        assert_eq!(ago(86_400 + 60), "Yesterday");
        assert_eq!(ago(86_400 * 4), "4 days ago");
        assert_eq!(ago(86_400 * 30), "25 Oct 2026");
        assert_eq!(ago(-60 * 5), "In 5 min");
        assert_eq!(ago(-(86_400 + 60)), "Tomorrow");
    }
}
