// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the progress ring.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Task, Window, div,
};
use isonomic_ui_extra::progress_ring::{ProgressRing, RingSize};

/// Receipts uploading one by one, a bank sync of unknown length, and the three sizes.
pub struct ProgressRingStory {
    uploaded: usize,
    _ticker: Task<()>,
}

const RECEIPTS: usize = 24;

impl ProgressRingStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let ticker = cx.spawn(
                async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(350))
                            .await;
                        let alive = this.update(cx, |this, cx| {
                            this.uploaded = if this.uploaded >= RECEIPTS {
                                0
                            } else {
                                this.uploaded + 1
                            };
                            cx.notify();
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                },
            );
            Self {
                uploaded: 0,
                _ticker: ticker,
            }
        })
    }
}

impl Story for ProgressRingStory {
    fn title() -> &'static str {
        "Progress ring"
    }

    fn description() -> &'static str {
        "A circular indicator that fills as work completes, or circles while the amount is unknown."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ProgressRingStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let share = self.uploaded as f32 / RECEIPTS as f32;
        v_flex()
            .gap_6()
            .child(ProgressRing::new("upload").share(share).label(format!(
                "Uploading receipts, {} of {RECEIPTS}",
                self.uploaded
            )))
            .child(ProgressRing::new("sync").label("Syncing the bank feed"))
            .child(
                div()
                    .text_xs()
                    .text_color(quiet)
                    .child("Small, medium and large"),
            )
            .child(
                h_flex()
                    .gap_6()
                    .child(ProgressRing::new("small").size(RingSize::Small).share(0.3))
                    .child(ProgressRing::new("medium").share(0.65))
                    .child(
                        ProgressRing::new("large")
                            .size(RingSize::Large)
                            .share(share),
                    )
                    .child(
                        ProgressRing::new("finished")
                            .size(RingSize::Large)
                            .share(1.0),
                    ),
            )
    }
}
