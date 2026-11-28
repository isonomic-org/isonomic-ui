// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the file change review.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::file_change_review::{
    ChangeKind, FileChange, FileChangeReview, Hunk, HunkLine,
};

/// A made-up change to a pottery studio's booking site.
fn studio_change() -> Vec<FileChange> {
    vec![
        FileChange::new("prices", "classes/prices.toml", ChangeKind::Modified)
            .hunk(Hunk::new("wheel", 3, 3).lines([
                HunkLine::context("[wheel_throwing]"),
                HunkLine::removed("price = 48"),
                HunkLine::added("price = 52"),
                HunkLine::added("includes_clay = true"),
                HunkLine::context("seats = 8"),
            ]))
            .hunk(Hunk::new("glazing", 14, 15).lines([
                HunkLine::context("[glazing_evening]"),
                HunkLine::removed("seats = 12"),
                HunkLine::added("seats = 10"),
                HunkLine::context("kiln = \"north\""),
            ])),
        FileChange::new("booking", "classes/booking.rs", ChangeKind::Modified).hunk(
            Hunk::new("waitlist", 41, 41).lines([
                HunkLine::context("fn seats_left(class: &Class) -> u32 {"),
                HunkLine::removed("    class.seats - class.booked"),
                HunkLine::added("    class.seats.saturating_sub(class.booked + class.held)"),
                HunkLine::context("}"),
                HunkLine::context(""),
                HunkLine::added("/// Seats held for ten minutes while someone pays, so two people never book the last wheel."),
                HunkLine::added("fn hold(class: &mut Class) { class.held += 1; }"),
            ]),
        ),
        FileChange::new("firing", "kiln/firing_log.md", ChangeKind::Added).hunk(
            Hunk::new("log", 0, 1).lines([
                HunkLine::added("# Firing log"),
                HunkLine::added(""),
                HunkLine::added("| Date | Kiln | Cone | Shelf load |"),
                HunkLine::added("| ---- | ---- | ---- | ---------- |"),
            ]),
        ),
        FileChange::new("schedule", "kiln/schedule.md", ChangeKind::Renamed)
            .from("docs/kiln-times.md")
            .hunk(Hunk::new("title", 1, 1).lines([
                HunkLine::removed("# Kiln times"),
                HunkLine::added("# Kiln schedule"),
                HunkLine::context(""),
                HunkLine::context("Bisque loads go in on Tuesdays."),
            ])),
        FileChange::new("flyer", "old_flyer.txt", ChangeKind::Deleted).hunk(
            Hunk::new("all", 1, 0).lines([
                HunkLine::removed("Spring open day: bring a mug, leave with a glaze."),
                HunkLine::removed("Doors at ten."),
            ]),
        ),
    ]
}

/// A change reviewed in a tree and in tabs, plus the empty review.
pub struct FileChangeReviewStory {
    applying: bool,
    said: SharedString,
}

impl FileChangeReviewStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            applying: false,
            said: "Nothing applied yet.".into(),
        })
    }
}

impl Story for FileChangeReviewStory {
    fn title() -> &'static str {
        "File change review"
    }

    fn description() -> &'static str {
        "Changed files beside their hunks: keep or revert each hunk, then apply what is kept."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FileChangeReviewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let apply = cx.entity();
        let discard = cx.entity();
        let quiet = cx.theme().muted_foreground;

        v_flex()
            .gap_6()
            .child(
                FileChangeReview::new("review", studio_change())
                    .reverted("prices", "glazing")
                    .applying(self.applying)
                    .on_apply(move |kept, _, cx| {
                        let hunks: usize = kept.iter().map(|file| file.hunks.len()).sum();
                        apply.update(cx, |this, cx| {
                            this.applying = true;
                            this.said = format!("Applying {hunks} hunks.").into();
                            cx.notify();
                        })
                    })
                    .on_discard(move |_, cx| {
                        discard.update(cx, |this, cx| {
                            this.applying = false;
                            this.said = "Discarded.".into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(quiet)
                    .child(self.said.clone()),
            )
            .child(
                FileChangeReview::new("stacked", studio_change())
                    .stacked()
                    .default_selected("schedule")
                    .on_apply(|_, _, _| {}),
            )
            .child(FileChangeReview::new("empty", Vec::new()))
    }
}
