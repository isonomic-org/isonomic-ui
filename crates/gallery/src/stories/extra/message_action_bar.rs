//! Gallery story for the message action bar.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use isonomic_ui_extra::message_action_bar::{MessageActionBar, Vote};

const FIRST: &str = "March closed 4% under budget. Freight was the only line that ran over.";
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
}

impl MessageActionBarStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            versions: vec![FIRST.into()],
            version: 0,
            writing: false,
            last: "Point at the message to light the bar up".into(),
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
    }
}
