// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::component::{
    ActiveTheme as _, StyledExt as _,
    inline_edit::{InlineEdit, InlineEditEvent},
    v_flex,
};
use isonomic_ui::*;

use crate::section;

pub struct InlineEditStory {
    title: Entity<InlineEdit>,
    rows: Vec<Entity<InlineEdit>>,
    last: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl super::Story for InlineEditStory {
    fn title() -> &'static str {
        "Inline Edit"
    }

    fn description() -> &'static str {
        "Text that turns into a field where it stands."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl InlineEditStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| {
            InlineEdit::new("doc-title", "Q3 launch plan", "Document title", window, cx)
                .placeholder("Untitled")
        });
        let rows = ["Design review", "Pricing page", "Launch email"]
            .into_iter()
            .enumerate()
            .map(|(ix, name)| {
                cx.new(|cx| {
                    InlineEdit::new(
                        SharedString::from(format!("task-{ix}")),
                        name,
                        "Task name",
                        window,
                        cx,
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut subscriptions = vec![cx.subscribe(&title, Self::report)];
        subscriptions.extend(rows.iter().map(|row| cx.subscribe(row, Self::report)));
        Self {
            title,
            rows,
            last: "Nothing yet".into(),
            _subscriptions: subscriptions,
        }
    }

    fn report(&mut self, _: Entity<InlineEdit>, event: &InlineEditEvent, cx: &mut Context<Self>) {
        self.last = match event {
            InlineEditEvent::Commit(value) => format!("Saved “{value}”").into(),
            InlineEditEvent::Cancel => "Cancelled".into(),
        };
        cx.notify();
    }
}

impl Focusable for InlineEditStory {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title.focus_handle(cx)
    }
}

impl Render for InlineEditStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                section("Title")
                    .description(
                        "Press the title, or Tab to it and press Enter. Enter saves, Escape cancels.",
                    )
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(360.))
                            .text_lg()
                            .font_semibold()
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Last change: {}", self.last)),
                    ),
            )
            .child(
                section("In a list")
                    .description("Each row is renamed where it stands.")
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(360.))
                            .gap_1()
                            .text_sm()
                            .children(self.rows.iter().cloned()),
                    ),
            )
    }
}
