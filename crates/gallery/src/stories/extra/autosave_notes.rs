//! Gallery story for the autosave notes.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, Sizable as _, button::Button, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::autosave_notes::{AutosaveNotes, NotesEvent};

/// Notes on a client file. Saves take half a second; turn the connection
/// off to watch a save fail.
pub struct AutosaveNotesStory {
    notes: Entity<AutosaveNotes>,
    offline: bool,
    saves: usize,
    _events: Subscription,
}

impl AutosaveNotesStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let notes = cx.new(|cx| {
                AutosaveNotes::new(
                    "Notes on Harbor Dental",
                    "Prefers invoices on the 1st. Ask Dana before changing the fuel card limit.",
                    window,
                    cx,
                )
                .limit(280)
            });
            let events = cx.subscribe(&notes, |this: &mut Self, notes, event: &NotesEvent, cx| {
                let NotesEvent::Save(_) = event;
                let offline = this.offline;
                cx.spawn(async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                    cx.background_executor().timer(Duration::from_millis(500)).await;
                    this.update(cx, |this, cx| {
                        notes.update(cx, |n, cx| if offline { n.save_failed("no connection", cx) } else { n.saved(cx) });
                        if !offline {
                            this.saves += 1;
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            });
            Self { notes, offline: false, saves: 0, _events: events }
        })
    }
}

impl Story for AutosaveNotesStory {
    fn title() -> &'static str {
        "Autosave notes"
    }

    fn description() -> &'static str {
        "A growing multi-line field that asks to be saved a moment after typing stops, and says whether its text is saved."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AutosaveNotesStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .w(px(440.))
            .gap_3()
            .child(self.notes.clone())
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("connection")
                            .small()
                            .outline()
                            .label(if self.offline { "Go back online" } else { "Go offline" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.offline = !this.offline;
                                cx.notify();
                            })),
                    )
                    .child(div().text_xs().text_color(quiet).child(format!("Saved {} times. Cmd or Ctrl and S saves now.", self.saves))),
            )
    }
}
