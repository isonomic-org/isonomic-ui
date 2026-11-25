// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! A vertical list of dated events joined by a line, such as an order's
//! history or a document's changes.

use gpui::{
    AnyElement, App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme as _, Icon, Sizable as _, StyledExt as _, h_flex, v_flex};

/// What an event in a [`Timeline`] marks, which sets its marker's colour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimelineTone {
    /// An ordinary event.
    #[default]
    Neutral,
    /// The event in progress or the latest one.
    Current,
    /// Something that went well, such as a payment received.
    Success,
    /// Something that needs attention.
    Warning,
    /// Something that failed.
    Danger,
}

/// One event in a [`Timeline`].
pub struct TimelineItem {
    title: SharedString,
    time: Option<SharedString>,
    description: Option<SharedString>,
    icon: Option<Icon>,
    tone: TimelineTone,
    content: Option<AnyElement>,
}

impl TimelineItem {
    /// An event with a title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            time: None,
            description: None,
            icon: None,
            tone: TimelineTone::Neutral,
            content: None,
        }
    }

    /// When it happened, as people should read it, such as "Today, 09:14".
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// A line of detail under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// An icon inside the marker, from the component set or the whole
    /// catalog. Without one the marker is a dot.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// What the event marks.
    pub fn tone(mut self, tone: TimelineTone) -> Self {
        self.tone = tone;
        self
    }

    /// Any element under the description, such as a quoted comment or a
    /// button.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

/// Events in order, each with a marker on a line that joins them.
///
/// Screen readers get a list, each event read as its title, time and
/// description. The newest event comes first by convention; the timeline
/// draws them in the order given.
#[derive(IntoElement)]
pub struct Timeline {
    id: ElementId,
    label: Option<SharedString>,
    items: Vec<TimelineItem>,
    style: StyleRefinement,
}

impl Timeline {
    /// An empty timeline.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            items: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Name the timeline for screen readers, such as "Order history".
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Add an event.
    pub fn item(mut self, item: TimelineItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add events.
    pub fn items(mut self, items: impl IntoIterator<Item = TimelineItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// How many events the timeline holds.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the timeline has no events.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Styled for Timeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The text a screen reader hears for an event.
fn spoken(item: &TimelineItem) -> String {
    let mut parts = vec![item.title.to_string()];
    parts.extend(item.time.iter().map(|t| t.to_string()));
    parts.extend(item.description.iter().map(|d| d.to_string()));
    parts.join(", ")
}

impl RenderOnce for Timeline {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let count = self.items.len();
        let marker_size = px(20.);
        let tone_color = |tone: TimelineTone| -> Hsla {
            match tone {
                TimelineTone::Neutral => theme.muted_foreground,
                TimelineTone::Current => theme.foreground,
                TimelineTone::Success => theme.success,
                TimelineTone::Warning => theme.warning,
                TimelineTone::Danger => theme.danger,
            }
        };

        v_flex()
            .id(self.id)
            .role(Role::List)
            .when_some(self.label, |this, label| this.aria_label(label))
            .w_full()
            .refine_style(&self.style)
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let last = ix + 1 == count;
                let color = tone_color(item.tone);
                let current = item.tone == TimelineTone::Current;
                let spoken_text = spoken(&item);
                let marker = div()
                    .size(marker_size)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(theme.background)
                    .map(|this| match item.icon {
                        Some(icon) => this
                            .border_1()
                            .border_color(color.opacity(0.4))
                            .child(icon.xsmall().text_color(color)),
                        None => this.child(
                            div()
                                .size(px(if current { 10. } else { 8. }))
                                .rounded_full()
                                .bg(color)
                                .when(current, |this| {
                                    this.border_2().border_color(color.opacity(0.25))
                                }),
                        ),
                    });

                h_flex()
                    .id(ix)
                    .role(Role::ListItem)
                    .aria_label(spoken_text)
                    .aria_position_in_set(ix + 1)
                    .aria_size_of_set(count)
                    .items_start()
                    .gap_3()
                    .child(
                        v_flex()
                            .items_center()
                            .self_stretch()
                            .child(marker)
                            .when(!last, |this| {
                                this.child(div().w(px(1.)).flex_1().min_h(px(12.)).bg(theme.border))
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .pb(if last { px(0.) } else { px(16.) })
                            .child(
                                h_flex()
                                    .min_h(marker_size)
                                    .gap_2()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.foreground)
                                            .when(current, |this| this.font_medium())
                                            .child(item.title),
                                    )
                                    .when_some(item.time, |this, time| {
                                        this.child(
                                            div()
                                                .flex_shrink_0()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(time),
                                        )
                                    }),
                            )
                            .when_some(item.description, |this, description| {
                                this.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(description),
                                )
                            })
                            .when_some(item.content, |this, content| {
                                this.child(h_flex().pt_1().child(content))
                            }),
                    )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_text_joins_title_time_and_description() {
        let item = TimelineItem::new("Payment received")
            .time("Today, 09:14")
            .description("Visa ending 4417");
        assert_eq!(
            spoken(&item),
            "Payment received, Today, 09:14, Visa ending 4417"
        );
        assert_eq!(spoken(&TimelineItem::new("Created")), "Created");
    }

    #[test]
    fn items_accumulate_in_order() {
        let timeline = Timeline::new("t")
            .item(TimelineItem::new("One"))
            .items([TimelineItem::new("Two"), TimelineItem::new("Three")]);
        assert_eq!(timeline.len(), 3);
        assert!(!timeline.is_empty());
        assert_eq!(timeline.items[2].title.as_ref(), "Three");
        assert!(Timeline::new("e").is_empty());
    }
}
