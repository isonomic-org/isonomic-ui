// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Autosave notes: a growing multi-line field that asks to be saved a moment after typing stops, and says whether its text is saved.
//!
//! After a pause in typing it reports the text once; the owner saves it and
//! calls `saved` or `save_failed`. The footer reads "Unsaved changes",
//! "Saving…", "Saved" or the failure, beside a word count and, with a limit,
//! how many characters are left. Cmd or Ctrl and S saves at once.

use std::time::Duration;

use isonomic_ui::component::{
    ActiveTheme as _, StyledExt as _, h_flex,
    input::{InputEvent, Textarea, TextareaState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window, div, px,
};

/// Where the notes stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStatus {
    /// Nothing typed since the last save.
    Saved,
    /// Typed but not yet asked to save.
    Unsaved,
    /// Asked to save; waiting on the owner.
    Saving,
    /// The owner couldn't save, and why.
    Failed(SharedString),
}

/// The footer's words for a status.
pub fn status_label(status: &SaveStatus) -> String {
    match status {
        SaveStatus::Saved => "Saved".into(),
        SaveStatus::Unsaved => "Unsaved changes".into(),
        SaveStatus::Saving => "Saving\u{2026}".into(),
        SaveStatus::Failed(why) => format!("Not saved: {why}"),
    }
}

/// Words in `text`, counting runs of non-space characters.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// The count shown in the footer, such as "42 words" or "42 words · 18 left".
pub fn count_label(text: &str, limit: Option<usize>) -> String {
    let words = word_count(text);
    let words = if words == 1 {
        "1 word".to_string()
    } else {
        format!("{words} words")
    };
    match limit {
        Some(limit) => {
            let used = text.chars().count();
            if used > limit {
                format!("{words} \u{b7} {} over", used - limit)
            } else {
                format!("{words} \u{b7} {} left", limit - used)
            }
        }
        None => words,
    }
}

/// What the notes report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotesEvent {
    /// Save this text.
    Save(SharedString),
}

/// A notes field that saves itself.
pub struct AutosaveNotes {
    field: Entity<TextareaState>,
    status: SaveStatus,
    limit: Option<usize>,
    pause: Duration,
    label: SharedString,
    _pending: Option<Task<()>>,
    _events: Subscription,
}

impl EventEmitter<NotesEvent> for AutosaveNotes {}

impl Focusable for AutosaveNotes {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl AutosaveNotes {
    /// Create notes holding `text`, named `label` for screen readers.
    pub fn new(
        label: impl Into<SharedString>,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text: SharedString = text.into();
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx).auto_grow(3, 12);
            field.set_value(text.clone(), window, cx);
            field
        });
        let events = cx.subscribe(&field, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.status = SaveStatus::Unsaved;
                this.schedule(cx);
                cx.notify();
            }
        });
        Self {
            field,
            status: SaveStatus::Saved,
            limit: None,
            pause: Duration::from_millis(900),
            label: label.into(),
            _pending: None,
            _events: events,
        }
    }

    /// Warn past this many characters, and don't ask to save past it.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Wait this long after typing stops before asking to save.
    pub fn pause(mut self, pause: Duration) -> Self {
        self.pause = pause;
        self
    }

    /// The text in the field.
    pub fn text(&self, cx: &App) -> SharedString {
        self.field.read(cx).value()
    }

    /// Where the notes stand.
    pub fn status(&self) -> &SaveStatus {
        &self.status
    }

    /// The owner saved the text.
    pub fn saved(&mut self, cx: &mut Context<Self>) {
        if self.status == SaveStatus::Saving {
            self.status = SaveStatus::Saved;
            cx.notify();
        }
    }

    /// The owner couldn't save, and why. Typing again tries again.
    pub fn save_failed(&mut self, why: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = SaveStatus::Failed(why.into());
        cx.notify();
    }

    fn over_limit(&self, cx: &App) -> bool {
        self.limit
            .is_some_and(|limit| self.text(cx).chars().count() > limit)
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        let pause = self.pause;
        self._pending = Some(cx.spawn(
            async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                cx.background_executor().timer(pause).await;
                this.update(cx, |this, cx| this.save_now(cx)).ok();
            },
        ));
    }

    fn save_now(&mut self, cx: &mut Context<Self>) {
        self._pending = None;
        if matches!(self.status, SaveStatus::Saved | SaveStatus::Saving) || self.over_limit(cx) {
            return;
        }
        self.status = SaveStatus::Saving;
        cx.emit(NotesEvent::Save(self.text(cx)));
        cx.notify();
    }
}

impl Render for AutosaveNotes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let text = self.text(cx);
        let over = self.over_limit(cx);
        let status_color = match self.status {
            SaveStatus::Failed(_) => danger,
            _ => quiet,
        };
        v_flex()
            .gap_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let keys = &event.keystroke;
                if (keys.modifiers.platform || keys.modifiers.control) && keys.key == "s" {
                    if this.status == SaveStatus::Unsaved
                        || matches!(this.status, SaveStatus::Failed(_))
                    {
                        if matches!(this.status, SaveStatus::Failed(_)) {
                            this.status = SaveStatus::Unsaved;
                        }
                        this.save_now(cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .child(Textarea::new(&self.field).aria_label(self.label.clone()))
            .child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .text_xs()
                    .child(
                        div()
                            .text_color(status_color)
                            .child(status_label(&self.status)),
                    )
                    .child(
                        div()
                            .text_color(if over { danger } else { quiet })
                            .font_medium()
                            .min_w(px(80.))
                            .text_right()
                            .child(count_label(&text, self.limit)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footers_read_status_and_counts() {
        assert_eq!(status_label(&SaveStatus::Saving), "Saving\u{2026}");
        assert_eq!(
            status_label(&SaveStatus::Failed("offline".into())),
            "Not saved: offline"
        );
        assert_eq!(word_count("  paid in full,  thanks "), 4);
        assert_eq!(count_label("one", None), "1 word");
        assert_eq!(count_label("two words", Some(12)), "2 words \u{b7} 3 left");
        assert_eq!(count_label("two words", Some(5)), "2 words \u{b7} 4 over");
    }
}
