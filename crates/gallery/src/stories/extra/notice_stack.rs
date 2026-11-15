//! Gallery story for the notice stack.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::notice_stack::{Notice, NoticeStack};

/// Shows three notices and reports what was dismissed or opened.
pub struct NoticeStackStory {
    last: SharedString,
}

impl NoticeStackStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing dismissed yet".into(),
        })
    }
}

impl Story for NoticeStackStory {
    fn title() -> &'static str {
        "Notice stack"
    }

    fn description() -> &'static str {
        "Announcements stacked behind each other, read one at a time."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for NoticeStackStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let opened = view.clone();

        v_flex()
            .gap_4()
            .child(
                NoticeStack::new("notices")
                    .notice(
                        Notice::new("export", "Exports now include attachments")
                            .badge("New")
                            .text("Every export bundles the files linked to its records.")
                            .action("Export now", move |_, _, cx| {
                                opened.update(cx, |this, cx| {
                                    this.last = "Opened the export".into();
                                    cx.notify();
                                })
                            }),
                    )
                    .notice(
                        Notice::new("billing", "Card expires next month")
                            .badge("Billing")
                            .text("Update the card on file to keep the workspace active.")
                            .external_action("Open billing", |_, _, _| {}),
                    )
                    .notice(Notice::new("seats", "Two seats are unassigned"))
                    .on_dismiss(move |id, _, cx| {
                        let line = format!("Dismissed {id}");
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
