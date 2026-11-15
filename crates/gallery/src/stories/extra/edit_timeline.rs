//! Gallery story for the edit timeline.

use std::time::Duration;

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Task, Window,
};
use isonomic_ui_extra::edit_timeline::{EditTimeline, FileEdit};

fn edits() -> Vec<FileEdit> {
    vec![
        FileEdit::new("e1", "rules", 12, "rec", "Add the card-matching rule").lines(18, 0),
        FileEdit::new("e2", "ledger", 40, "rec", "Mark 61 bank lines matched").lines(61, 61),
        FileEdit::new("e3", "rules", 66, "maya", "Widen the date window to 3 days").lines(2, 2),
        FileEdit::new("e4", "ledger", 95, "card", "Mark 201 card lines matched").lines(201, 201),
        FileEdit::new("e5", "note", 120, "lead", "Draft the close note").lines(24, 0),
        FileEdit::new("e6", "ledger", 150, "card", "Undo 3 matches on refunds").lines(3, 3),
        FileEdit::new("e7", "note", 172, "maya", "Tighten the summary").lines(6, 11),
        FileEdit::new(
            "e8",
            "rules",
            190,
            "lead",
            "Note the locked-period exception",
        )
        .lines(5, 0),
    ]
}

/// A close's edits across three files, replayable. The story keeps the
/// playhead, the replay and the selection.
pub struct EditTimelineStory {
    playhead: u32,
    playing: bool,
    selected: Option<SharedString>,
    _tick: Task<()>,
}

impl EditTimelineStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tick = cx.spawn(
                async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        let alive = this.update(cx, |this, cx| {
                            if this.playing {
                                this.playhead += 2;
                                if this.playhead >= 190 {
                                    this.playhead = 190;
                                    this.playing = false;
                                }
                                cx.notify();
                            }
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                },
            );
            Self {
                playhead: 190,
                playing: false,
                selected: None,
                _tick: tick,
            }
        })
    }
}

impl Story for EditTimelineStory {
    fn title() -> &'static str {
        "Edit timeline"
    }

    fn description() -> &'static str {
        "Each change to each file on a time axis, to watch live or replay."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EditTimelineStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let seek = cx.entity();
        let play = cx.entity();
        let select = cx.entity();
        EditTimeline::new(
            "close-edits",
            vec![
                ("rules".into(), "matching-rules.toml".into()),
                ("ledger".into(), "march-ledger.csv".into()),
                ("note".into(), "close-note.md".into()),
            ],
            vec![
                ("lead".into(), "Coordinator".into()),
                ("rec".into(), "Reconciler".into()),
                ("card".into(), "Card matcher".into()),
                ("maya".into(), "Maya".into()),
            ],
            edits(),
        )
        .playhead(self.playhead)
        .playing(self.playing)
        .selected(self.selected.clone())
        .on_seek(move |at, _, cx| {
            seek.update(cx, |this, cx| {
                this.playhead = at;
                this.selected = None;
                cx.notify();
            })
        })
        .on_play(move |playing, _, cx| {
            play.update(cx, |this, cx| {
                if playing && this.playhead >= 190 {
                    this.playhead = 0;
                }
                this.playing = playing;
                this.selected = None;
                cx.notify();
            })
        })
        .on_select(move |id, _, cx| {
            select.update(cx, |this, cx| {
                this.selected = Some(id.clone());
                cx.notify();
            })
        })
    }
}
