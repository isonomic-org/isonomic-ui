// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the reaction bar.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::reaction_bar::{Reaction, ReactionBar};

/// Shows a bar with counts large and small and reports the last press.
pub struct ReactionBarStory {
    last: SharedString,
}

impl ReactionBarStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing pressed yet".into(),
        })
    }
}

impl Story for ReactionBarStory {
    fn title() -> &'static str {
        "Reaction bar"
    }

    fn description() -> &'static str {
        "A row of small toggles with public counts, such as likes or saves."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ReactionBarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        v_flex()
            .gap_3()
            .items_start()
            .child(
                ReactionBar::new("post")
                    .reaction(Reaction::like().count(1_299))
                    .reaction(Reaction::save().count(9).active(true))
                    .reaction(
                        Reaction::new("agree", IconName::ThumbsUp, "Agree")
                            .noun("vote", "votes")
                            .count(41),
                    )
                    .reaction(Reaction::new("open", IconName::ExternalLink, "Open"))
                    .on_react(move |key, state, _, cx| {
                        let line = format!(
                            "{key}: {} at {}",
                            if state.active { "on" } else { "off" },
                            state.count
                        );
                        view.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
