//! Gallery story for the file dropzone.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::file_dropzone::{FileDropzone, FileDropzoneEvent};

const MB: u64 = 1024 * 1024;

/// Shows a dropzone whose uploads are simulated. One file fails on its first
/// try, and one is too large to be taken.
pub struct FileDropzoneStory {
    dropzone: Entity<FileDropzone>,
    /// Files that had a first try, so that a retry can go through.
    tried: Vec<usize>,
    _events: Subscription,
}

impl FileDropzoneStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let dropzone = cx
                .new(|cx| FileDropzone::new(cx).accept([".pdf", ".png", ".csv", ".xlsx", ".mov"]));
            let events = cx.subscribe(&dropzone, |this: &mut Self, _, event, cx| {
                if let FileDropzoneEvent::Upload(ids) = event {
                    for id in ids {
                        this.simulate(*id, cx);
                    }
                }
            });
            dropzone.update(cx, |dropzone, cx| {
                dropzone.add_file("Year-end-accounts.pdf", 4 * MB + MB / 5, cx);
                dropzone.add_file("Receipt-scan.png", 2 * MB + MB / 2, cx);
                dropzone.add_file("Ledger-export.xlsx", 9 * MB, cx);
                dropzone.add_file("Walkthrough.mov", 80 * MB, cx);
            });

            Self {
                dropzone,
                tried: Vec::new(),
                _events: events,
            }
        })
    }

    fn simulate(&mut self, id: usize, cx: &mut Context<Self>) {
        let first_try = !self.tried.contains(&id);
        self.tried.push(id);
        let name = self
            .dropzone
            .read(cx)
            .items()
            .iter()
            .find(|item| item.id() == id)
            .map(|item| item.name().clone())
            .unwrap_or_default();
        let fails = first_try && name.starts_with("Ledger");

        let dropzone = self.dropzone.clone();
        cx.spawn(async move |_, cx| {
            for step in 1..=10 {
                cx.background_executor()
                    .timer(Duration::from_millis(350))
                    .await;
                let stopped = dropzone.update(cx, |dropzone, cx| {
                    if fails && step == 6 {
                        dropzone.fail(id, "Connection lost", cx);
                        return true;
                    }
                    dropzone.set_progress(id, step as f32 / 10., cx);
                    false
                });
                if stopped {
                    return;
                }
            }
            dropzone.update(cx, |dropzone, cx| dropzone.finish(id, cx));
        })
        .detach();
    }
}

impl Render for FileDropzoneStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.dropzone.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Uploads here are simulated. Retry the failed file to see it go through."),
        )
    }
}
