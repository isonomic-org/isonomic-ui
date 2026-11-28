// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Segmented switcher: a short row of choices joined into one control, where exactly one is on, each with an optional icon and count.
//!
//! It is the tab bar's segmented variant: the chosen segment's raised pill
//! slides between segments when one is clicked, and jumps at once when the
//! keyboard moves it. Arrow keys, Home and End pick the next segment that is
//! on, wrapping at the ends; the row is one tab stop. Segments can carry a
//! count, such as how many invoices are unpaid, and can be turned off one at
//! a time. With `fill` the segments share the full width equally.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable, Size, StyledExt as _, h_flex,
    tab::{Tab, TabBar},
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

/// One choice.
#[derive(Clone)]
pub struct Segment {
    label: SharedString,
    icon: Option<IconName>,
    count: Option<usize>,
    disabled: bool,
}

impl Segment {
    /// Create a choice with a label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            count: None,
            disabled: false,
        }
    }

    /// Lead with an icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Show a count after the label.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Turn this choice off.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// What a screen reader hears for a segment, such as "Unpaid, 12".
pub fn segment_label(label: &str, count: Option<usize>) -> String {
    match count {
        Some(count) => format!("{label}, {count}"),
        None => label.to_string(),
    }
}

/// Compact counts for a narrow segment: 1,200 shows as "1.2k".
pub fn short_count(count: usize) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=9_999 => format!("{:.1}k", count as f32 / 1000.0).replace(".0k", "k"),
        10_000..=999_999 => format!("{}k", count / 1000),
        _ => format!("{}m", count / 1_000_000),
    }
}

/// The segment an arrow, Home or End key moves to from `selected`, skipping
/// segments that are off and wrapping at the ends.
pub fn step_segment(selected: usize, enabled: &[bool], key: &str) -> Option<usize> {
    let count = enabled.len();
    if count == 0 || !enabled.iter().any(|e| *e) {
        return None;
    }
    let first = enabled.iter().position(|e| *e)?;
    let last = enabled.iter().rposition(|e| *e)?;
    let walk = |forward: bool| {
        let mut ix = selected.min(count - 1);
        for _ in 0..count {
            ix = if forward {
                (ix + 1) % count
            } else {
                (ix + count - 1) % count
            };
            if enabled[ix] {
                return Some(ix);
            }
        }
        None
    };
    match key {
        "right" | "down" => walk(true),
        "left" | "up" => walk(false),
        "home" => Some(first),
        "end" => Some(last),
        _ => None,
    }
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A row of joined choices.
#[derive(IntoElement)]
pub struct SegmentedSwitcher {
    id: ElementId,
    style: StyleRefinement,
    segments: Vec<Segment>,
    selected: usize,
    fill: bool,
    size: Size,
    label: SharedString,
    on_select: Option<SelectHandler>,
}

impl SegmentedSwitcher {
    /// Create a switcher with `selected` on.
    pub fn new(id: impl Into<ElementId>, segments: Vec<Segment>, selected: usize) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            segments,
            selected,
            fill: false,
            size: Size::default(),
            label: "Choices".into(),
            on_select: None,
        }
    }

    /// Share the full width equally.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Name the group for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Run with the index picked.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Sizable for SegmentedSwitcher {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for SegmentedSwitcher {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SegmentedSwitcher {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let selected = self.selected;
        let small = matches!(self.size, Size::XSmall | Size::Small);
        let on_chip = cx.theme().muted;
        let off_chip = cx.theme().popover.opacity(0.6);
        let fill = self.fill;

        let tabs = self.segments.into_iter().enumerate().map(|(ix, segment)| {
            let on = ix == selected;
            Tab::new()
                .aria_label(segment_label(&segment.label, segment.count))
                .disabled(segment.disabled)
                .when(fill, |tab| tab.flex_1())
                .child(
                    h_flex()
                        .gap(if small { px(4.) } else { px(6.) })
                        .when_some(segment.icon, |this, icon| {
                            this.child(Icon::new(icon).with_size(if small {
                                Size::XSmall
                            } else {
                                Size::Small
                            }))
                        })
                        .child(segment.label.clone())
                        .when_some(segment.count, |this, count| {
                            this.child(
                                div()
                                    .px_1()
                                    .min_w(px(18.))
                                    .rounded(px(4.))
                                    .bg(if on { on_chip } else { off_chip })
                                    .text_badge()
                                    .text_center()
                                    .child(short_count(count)),
                            )
                        }),
                )
        });

        let pick = self.on_select.clone();
        let bar = TabBar::new(self.id.clone())
            .segmented()
            .activate_on_focus(true)
            .with_size(self.size)
            .selected_index(selected)
            .when(fill, |bar| bar.w_full())
            .on_click(move |ix, window, cx| {
                if *ix != selected {
                    if let Some(pick) = &pick {
                        pick(*ix, window, cx);
                    }
                }
            })
            .children(tabs);

        div()
            .id(ElementId::NamedChild(
                self.id.clone().into(),
                "group".into(),
            ))
            .flex()
            .when(!fill, |this| this.flex_none())
            .when(fill, |this| this.w_full())
            .role(Role::Group)
            .aria_label(self.label.clone())
            .child(bar)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_shorten_and_read_aloud() {
        assert_eq!(segment_label("Unpaid", Some(12)), "Unpaid, 12");
        assert_eq!(segment_label("All", None), "All");
        assert_eq!(short_count(999), "999");
        assert_eq!(short_count(1000), "1k");
        assert_eq!(short_count(1250), "1.2k");
        assert_eq!(short_count(48_200), "48k");
        assert_eq!(short_count(3_400_000), "3m");
        assert_eq!(
            step_segment(0, &[true, true, false, true], "right"),
            Some(1)
        );
        assert_eq!(
            step_segment(1, &[true, true, false, true], "right"),
            Some(3)
        );
        assert_eq!(
            step_segment(3, &[true, true, false, true], "right"),
            Some(0)
        );
        assert_eq!(step_segment(0, &[true, true, false, true], "left"), Some(3));
        assert_eq!(
            step_segment(2, &[false, true, true, false], "home"),
            Some(1)
        );
        assert_eq!(step_segment(0, &[true, true], "end"), Some(1));
        assert_eq!(step_segment(0, &[false, false], "right"), None);
        let segment = Segment::new("Draft")
            .icon(IconName::File)
            .count(2)
            .disabled(true);
        assert!(segment.disabled && segment.count == Some(2));
    }
}
