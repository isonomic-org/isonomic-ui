//! Gallery story for the sequence diagram.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use isonomic_ui_extra::sequence_diagram::{SequenceDiagram, SequenceMessage};

fn script() -> Vec<SequenceMessage> {
    vec![
        SequenceMessage::new("maya", "lead", "Close March"),
        SequenceMessage::new("lead", "recon", "Match cards, then bank"),
        SequenceMessage::new("recon", "ledger", "List unmatched lines"),
        SequenceMessage::new("ledger", "recon", "214 lines").reply(),
        SequenceMessage::new("recon", "recon", "Match by amount and date"),
        SequenceMessage::new("recon", "ledger", "Post 201 matches"),
        SequenceMessage::new("ledger", "recon", "Period is locked").reply(),
        SequenceMessage::new("recon", "lead", "Need March unlocked").reply(),
        SequenceMessage::new("lead", "maya", "Unlock March or post to April?"),
    ]
}

/// A close run's messages arriving one every second and a half, then
/// starting over. The story keeps the selection.
pub struct SequenceDiagramStory {
    shown: usize,
    selected: Option<usize>,
    _tick: Task<()>,
}

impl SequenceDiagramStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tick = cx.spawn(async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                loop {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    let alive = this.update(cx, |this, cx| {
                        this.shown = if this.shown >= script().len() + 2 { 1 } else { this.shown + 1 };
                        cx.notify();
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            });
            Self {
                shown: 1,
                selected: None,
                _tick: tick,
            }
        })
    }
}

impl Story for SequenceDiagramStory {
    fn title() -> &'static str {
        "Sequence diagram"
    }

    fn description() -> &'static str {
        "Messages between participants along their lifelines, newest one lit as it arrives."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SequenceDiagramStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let people: Vec<(SharedString, SharedString)> = vec![
            ("maya".into(), "Maya".into()),
            ("lead".into(), "Coordinator".into()),
            ("recon".into(), "Reconciler".into()),
            ("ledger".into(), "Ledger".into()),
        ];
        let messages: Vec<_> = script().into_iter().take(self.shown).collect();
        let note = self
            .selected
            .and_then(|ix| messages.get(ix))
            .map(|m| format!("Selected: {} \u{2192} {}, \u{201c}{}\u{201d}", m.from, m.to, m.label))
            .unwrap_or_else(|| "Messages arrive one by one. Click one to select it.".into());
        v_flex()
            .w_full()
            .gap_3()
            .child(
                SequenceDiagram::new("close-sequence", people, messages)
                    .selected(self.selected)
                    .on_select(move |ix, _, cx| {
                        select.update(cx, |this, cx| {
                            this.selected = Some(ix);
                            cx.notify();
                        })
                    }),
            )
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(note))
    }
}
