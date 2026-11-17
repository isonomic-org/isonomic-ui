// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Activity recap: a day's work as a stacked bar chart over time, then lanes of what needs review, what failed and what's done.
//!
//! Each bar is one stretch of time, split into done, failed and needing
//! review, with its label under it and its counts in its name.
//! Below, three lanes list the items themselves; review items carry an
//! Open button. Pointing at a bar picks it out; the owner gets clicks.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, button::Button, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// One stretch of time on the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct RecapBucket {
    /// Its label, such as "9 am".
    pub label: SharedString,
    /// Items finished.
    pub done: u32,
    /// Items that failed.
    pub failed: u32,
    /// Items waiting for review.
    pub review: u32,
}

impl RecapBucket {
    /// Create a bucket.
    pub fn new(label: impl Into<SharedString>, done: u32, failed: u32, review: u32) -> Self {
        Self {
            label: label.into(),
            done,
            failed,
            review,
        }
    }

    /// All its items.
    pub fn total(&self) -> u32 {
        self.done + self.failed + self.review
    }
}

/// Which lane an item sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecapLane {
    /// Waits on a person.
    Review,
    /// Went wrong.
    Failed,
    /// Finished.
    Done,
}

/// One item in a lane.
#[derive(Debug, Clone, PartialEq)]
pub struct RecapItem {
    /// Identifies it.
    pub id: SharedString,
    /// Its lane.
    pub lane: RecapLane,
    /// What it is.
    pub title: SharedString,
    /// Who did it and when, as written.
    pub meta: SharedString,
}

impl RecapItem {
    /// Create an item.
    pub fn new(
        id: impl Into<SharedString>,
        lane: RecapLane,
        title: impl Into<SharedString>,
        meta: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            lane,
            title: title.into(),
            meta: meta.into(),
        }
    }
}

/// Totals across buckets: done, failed, review.
pub fn recap_totals(buckets: &[RecapBucket]) -> (u32, u32, u32) {
    buckets.iter().fold((0, 0, 0), |(d, f, r), b| {
        (d + b.done, f + b.failed, r + b.review)
    })
}

type ItemHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type BucketHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// A day's work over time, then by outcome.
#[derive(IntoElement)]
pub struct ActivityRecap {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    buckets: Vec<RecapBucket>,
    items: Vec<RecapItem>,
    picked: Option<usize>,
    on_pick: Option<BucketHandler>,
    on_open: Option<ItemHandler>,
}

impl ActivityRecap {
    /// Create a recap.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        buckets: Vec<RecapBucket>,
        items: Vec<RecapItem>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            buckets,
            items,
            picked: None,
            on_pick: None,
            on_open: None,
        }
    }

    /// The bar picked out, if any.
    pub fn picked(mut self, ix: Option<usize>) -> Self {
        self.picked = ix;
        self
    }

    /// Run when the pointer moves onto a bar, or off the chart with `None`.
    pub fn on_pick(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// Run when a review item's Open button is clicked.
    pub fn on_open(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl Styled for ActivityRecap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ActivityRecap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let done_c = Tone::Success.color(cx);
        let failed_c = Tone::Danger.color(cx);
        let review_c = Tone::Warning.color(cx);

        let (done, failed, review) = recap_totals(&self.buckets);
        let peak = self
            .buckets
            .iter()
            .map(RecapBucket::total)
            .max()
            .unwrap_or(1)
            .max(1) as f32;

        let bars: Vec<_> = self
            .buckets
            .iter()
            .enumerate()
            .map(|(ix, bucket)| {
                let pick = self.on_pick.clone();
                let picked = self.picked == Some(ix);
                let dim = self.picked.is_some() && !picked;
                let seg =
                    |n: u32, color: Hsla| div().w_full().h(relative(n as f32 / peak)).bg(color);
                v_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("bar-{ix}").into(),
                    ))
                    .flex_1()
                    .min_w(px(14.))
                    .h_full()
                    .gap_1()
                    .role(Role::Image)
                    .aria_label(format!(
                        "{}: {} done, {} failed, {} to review",
                        bucket.label, bucket.done, bucket.failed, bucket.review
                    ))
                    .on_hover(move |hovered, window, cx| {
                        if let Some(pick) = &pick {
                            pick(hovered.then_some(ix), window, cx);
                        }
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .justify_end()
                            .when(dim, |this| this.opacity(0.4))
                            .child(
                                v_flex()
                                    .h_full()
                                    .justify_end()
                                    .overflow_hidden()
                                    .rounded_t(px(3.))
                                    .child(seg(bucket.review, review_c))
                                    .child(seg(bucket.failed, failed_c))
                                    .child(seg(bucket.done, done_c.opacity(0.75))),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_center()
                            .truncate()
                            .text_color(if picked { ink } else { quiet })
                            .child(bucket.label.clone()),
                    )
            })
            .collect();

        let picked_line = self
            .picked
            .and_then(|ix| self.buckets.get(ix))
            .map(|b| {
                format!(
                    "{}: {} done, {} failed, {} to review",
                    b.label, b.done, b.failed, b.review
                )
            })
            .unwrap_or_else(|| "Point at a bar for its counts.".into());

        let lane = |which: RecapLane, title: &'static str, color: Hsla, icon: IconName| {
            let items: Vec<_> = self.items.iter().filter(|i| i.lane == which).collect();
            let count = items.len();
            let cards: Vec<_> =
                items
                    .into_iter()
                    .map(|item| {
                        let open = self.on_open.clone();
                        let id = item.id.clone();
                        v_flex()
                            .gap_1()
                            .p_2()
                            .rounded(radius)
                            .border_1()
                            .border_color(line)
                            .bg(fill)
                            .child(div().line_height(relative(1.35)).child(item.title.clone()))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(quiet)
                                            .child(item.meta.clone()),
                                    )
                                    .when(which == RecapLane::Review, |this| {
                                        this.child(
                                            Button::new(ElementId::NamedChild(
                                                self.id.clone().into(),
                                                format!("open-{}", item.id).into(),
                                            ))
                                            .xsmall()
                                            .outline()
                                            .label("Open")
                                            .accessibility_label(format!("Open {}", item.title))
                                            .on_click(move |_, window, cx| {
                                                if let Some(open) = &open {
                                                    open(&id, window, cx);
                                                }
                                            }),
                                        )
                                    }),
                            )
                    })
                    .collect();
            v_flex()
                .flex_1()
                .min_w(px(200.))
                .gap_2()
                .p_2()
                .rounded(radius_lg)
                .bg(chip.opacity(0.4))
                .child(
                    h_flex()
                        .gap_1p5()
                        .px_1()
                        .text_xs()
                        .font_medium()
                        .child(Icon::new(icon).xsmall().text_color(color))
                        .child(title)
                        .child(div().text_color(quiet).child(count.to_string())),
                )
                .children(cards)
                .when(count == 0, |this| {
                    this.child(
                        div()
                            .px_1()
                            .py_2()
                            .text_xs()
                            .text_color(quiet)
                            .child("Nothing here."),
                    )
                })
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(900.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .child(div().flex_1().font_semibold().child(self.title.clone()))
                    .child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .child(div().size(px(8.)).rounded(px(2.)).bg(done_c))
                            .child(format!("{done} done")),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .child(div().size(px(8.)).rounded(px(2.)).bg(failed_c))
                            .child(format!("{failed} failed")),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .child(div().size(px(8.)).rounded(px(2.)).bg(review_c))
                            .child(format!("{review} to review")),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .px_4()
                    .pt_3()
                    .child(h_flex().h(px(140.)).items_end().gap_1().children(bars))
                    .child(div().text_xs().text_color(quiet).child(picked_line)),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap_3()
                    .p_4()
                    .child(lane(
                        RecapLane::Review,
                        "Needs review",
                        review_c,
                        IconName::Bell,
                    ))
                    .child(lane(
                        RecapLane::Failed,
                        "Failed",
                        failed_c,
                        IconName::CircleX,
                    ))
                    .child(lane(RecapLane::Done, "Done", done_c, IconName::CircleCheck)),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_across_buckets() {
        let buckets = vec![
            RecapBucket::new("9", 4, 1, 0),
            RecapBucket::new("10", 2, 0, 3),
        ];
        assert_eq!(recap_totals(&buckets), (6, 1, 3));
        assert_eq!(buckets[1].total(), 5);
    }
}
