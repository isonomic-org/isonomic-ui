// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the live cursors overlay.

use std::time::Duration;

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Task, Window,
};
use isonomic_ui_extra::live_cursors_overlay::{
    Collaborator, LiveCursorsOverlay, Presence, WordSuggestion,
};

fn paragraphs() -> Vec<SharedString> {
    vec![
        "March closed with revenue of $184,200, up 6% on February, and operating costs of $121,900.".into(),
        "Card payouts are fully matched except 13 refunds, which wait on the processor's statement.".into(),
        "The ledger stays locked until Maya signs off, after which the close note goes to the board.".into(),
    ]
}

/// Three collaborators in a close note; the reconciler's cursor walks
/// along the second paragraph. The story resolves suggestions.
pub struct LiveCursorsOverlayStory {
    step: usize,
    suggestions: Vec<WordSuggestion>,
    _tick: Task<()>,
}

impl LiveCursorsOverlayStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tick = cx.spawn(
                async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(700))
                            .await;
                        let alive = this.update(cx, |this, cx| {
                            this.step = (this.step + 1) % 14;
                            cx.notify();
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                },
            );
            Self {
                step: 0,
                suggestions: vec![
                    WordSuggestion {
                        id: "s1".into(),
                        who: "lead".into(),
                        paragraph: 0,
                        words: (8, 9),
                        replacement: "compared with".into(),
                    },
                    WordSuggestion {
                        id: "s2".into(),
                        who: "recon".into(),
                        paragraph: 1,
                        words: (10, 13),
                        replacement: "the processor's March statement.".into(),
                    },
                ],
                _tick: tick,
            }
        })
    }
}

impl Story for LiveCursorsOverlayStory {
    fn title() -> &'static str {
        "Live cursors overlay"
    }

    fn description() -> &'static str {
        "Who else is in a shared document, where their cursors are, what they've selected and what they suggest."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for LiveCursorsOverlayStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let resolve = cx.entity();
        LiveCursorsOverlay::new("close-note", "March close note", paragraphs())
            .people(
                vec![
                    Collaborator::person("maya", "Maya"),
                    Collaborator::agent("recon", "Reconciler"),
                    Collaborator::agent("lead", "Coordinator"),
                ],
                vec![
                    Presence {
                        who: "maya".into(),
                        paragraph: 0,
                        after_word: 6,
                        selection: Some((4, 6)),
                    },
                    Presence {
                        who: "recon".into(),
                        paragraph: 1,
                        after_word: self.step.min(13),
                        selection: None,
                    },
                    Presence {
                        who: "lead".into(),
                        paragraph: 2,
                        after_word: 9,
                        selection: None,
                    },
                ],
            )
            .suggestions(self.suggestions.clone())
            .on_resolve(move |id, _, _, cx| {
                resolve.update(cx, |this, cx| {
                    this.suggestions.retain(|s| &s.id != id);
                    cx.notify();
                })
            })
    }
}
