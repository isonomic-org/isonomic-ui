// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the message action bar.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, Size, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use isonomic_ui_extra::message_action_bar::{
    MessageActionBar, MessageActionLabel, SwapDirection, Vote, swap_direction, swap_in,
};

const FIRST: &str = "March closed 4% under budget. Freight was the only line that ran over.";
/// Four takes on one answer, for the branch map.
const BRANCHES: [(&str, &str); 4] = [
    (
        "First answer",
        "Payroll ran 2% over in March, all of it overtime in the warehouse.",
    ),
    ("Shorter", "Payroll: 2% over, from warehouse overtime."),
    (
        "With a cause",
        "Payroll ran 2% over: the warehouse worked two extra Saturdays for the quarter-end rush.",
    ),
    (
        "With a fix",
        "Payroll ran 2% over. Moving the quarter-end count to a weekday would have saved most of it.",
    ),
];
const MORE: [&str; 2] = [
    "March came in 4% under budget; only freight overshot, by about 1,200.",
    "Under budget by 4% in March. Freight ran over; everything else held.",
];

/// Shows a message whose bar adds a version on Regenerate and reports votes.
pub struct MessageActionBarStory {
    versions: Vec<SharedString>,
    version: usize,
    writing: bool,
    last: SharedString,
    /// The branch shown in the branch map's message, and the way it moved.
    branch: usize,
    branch_direction: SwapDirection,
    branch_swaps: usize,
    feedback: SharedString,
}

impl MessageActionBarStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            versions: vec![FIRST.into()],
            version: 0,
            writing: false,
            last: "Point at the message to light the bar up".into(),
            branch: 3,
            branch_direction: SwapDirection::Still,
            branch_swaps: 0,
            feedback: "Vote to see the reasons".into(),
        })
    }

    fn regenerate(&mut self, cx: &mut Context<Self>) {
        if self.writing {
            return;
        }
        self.writing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1400))
                .await;
            this.update(cx, |this, cx| {
                let next = MORE[(this.versions.len() - 1) % MORE.len()];
                this.versions.push(next.into());
                this.version = this.versions.len() - 1;
                this.writing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Story for MessageActionBarStory {
    fn title() -> &'static str {
        "Message action bar"
    }

    fn description() -> &'static str {
        "The quiet row of actions under an assistant's message."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MessageActionBarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let radius = theme.radius_lg;
        let text = self.versions[self.version].clone();
        let story = cx.entity();
        let picked = story.clone();
        let voted = story.clone();
        let branched = story.clone();
        let detailed = story.clone();
        let still = cx.reduce_motion();
        let (branch_title, branch_text) = BRANCHES[self.branch];

        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .group("message")
                    .max_w(px(420.))
                    .gap_2()
                    .p_4()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .child(div().text_sm().child(text.clone()))
                    .child(
                        MessageActionBar::new("actions")
                            .text(text)
                            .hover_group("message")
                            .versions(self.versions.len())
                            .version(self.version)
                            .regenerating(self.writing)
                            .on_version_change(move |version, _, cx| {
                                picked.update(cx, |this, cx| {
                                    this.version = version;
                                    cx.notify();
                                })
                            })
                            .on_regenerate(move |_, cx| {
                                story.update(cx, |this, cx| this.regenerate(cx))
                            })
                            .on_feedback(move |vote, reason, _, cx| {
                                let line = match (vote, reason) {
                                    (Some(Vote::Down), Some(reason)) => format!("Down: {reason}"),
                                    (Some(Vote::Down), None) => "Down".to_string(),
                                    (Some(Vote::Up), _) => "Up".to_string(),
                                    (None, _) => "Vote taken back".to_string(),
                                };
                                voted.update(cx, |this, cx| {
                                    this.last = line.into();
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(div().text_xs().text_color(quiet).child(self.last.clone()))
            .child(div().pt_4().text_xs().text_color(quiet).child(
                "28 px buttons with 16 px icons, the switcher first and quiet (12 px muted arrows), and Regenerate reworded",
            ))
            .child(
                MessageActionBar::new("actions-reworded")
                    .text("The March close is ready to review.")
                    .quiet(false)
                    .versions(3)
                    .default_version(1)
                    .button_size(Size::Size(px(28.)))
                    .icon_size(px(16.))
                    .pager_first(true)
                    .pager_quiet(true)
                    .label(MessageActionLabel::Regenerate, "Try again"),
            )
            .child(div().pt_4().text_xs().text_color(quiet).child(
                "Branches: a map of every version, and the text slides in from the side the switch came from",
            ))
            .child(
                v_flex()
                    .max_w(px(420.))
                    .gap_2()
                    .p_4()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .child(div().text_xs().text_color(quiet).child(branch_title))
                    .child(swap_in(
                        ("branch-text", self.branch_swaps),
                        self.branch_direction,
                        still,
                        div().text_sm().child(branch_text),
                    ))
                    .child(
                        MessageActionBar::new("actions-branches")
                            .text(branch_text)
                            .quiet(false)
                            .actions([
                                isonomic_ui_extra::message_action_bar::MessageAction::Copy,
                                isonomic_ui_extra::message_action_bar::MessageAction::Regenerate,
                            ])
                            .versions(BRANCHES.len())
                            .version(self.branch)
                            .branch_map(true)
                            .branch_labels(BRANCHES.map(|(title, _)| title))
                            .on_version_change(move |version, _, cx| {
                                branched.update(cx, |this, cx| {
                                    this.branch_direction = swap_direction(this.branch, version);
                                    this.branch = version;
                                    this.branch_swaps += 1;
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                MessageActionBar::new("actions-many-branches")
                    .text("A long thread of retries.")
                    .quiet(false)
                    .actions([isonomic_ui_extra::message_action_bar::MessageAction::Regenerate])
                    .versions(14)
                    .default_version(6)
                    .branch_map(true),
            )
            .child(div().pt_4().text_xs().text_color(quiet).child(
                "Feedback: several reasons with a note, reasons for a vote up too, and a thank-you that stays",
            ))
            .child(
                MessageActionBar::new("actions-feedback")
                    .text("Freight ran over by about 1,200.")
                    .quiet(false)
                    .actions([isonomic_ui_extra::message_action_bar::MessageAction::Feedback])
                    .reasons(["Wrong numbers", "Too long", "Missed the question", "Out of date"])
                    .up_reasons(["Accurate", "Easy to follow", "Saved me time"])
                    .multi_reason(true)
                    .feedback_note(true)
                    .keep_thanks(true)
                    .thanks_message("Thanks. We read every note.")
                    .on_feedback_detail(move |detail, _, cx| {
                        let mut line = format!(
                            "{}: {}",
                            match detail.vote {
                                Vote::Up => "Up",
                                Vote::Down => "Down",
                            },
                            detail.reasons.join(", ")
                        );
                        if let Some(note) = &detail.note {
                            line.push_str(&format!(" (note: {note})"));
                        }
                        detailed.update(cx, |this, cx| {
                            this.feedback = line.into();
                            cx.notify();
                        })
                    }),
            )
            .child(div().text_xs().text_color(quiet).child(self.feedback.clone()))
            .child(
                MessageActionBar::new("actions-thank")
                    .text("Rent is due on the 1st.")
                    .quiet(false)
                    .actions([isonomic_ui_extra::message_action_bar::MessageAction::Feedback])
                    .thank_on_vote(true),
            )
    }
}
