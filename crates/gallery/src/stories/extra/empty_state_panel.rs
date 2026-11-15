//! Gallery story for the empty state panel.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::empty_state_panel::{EmptyReason, EmptyStatePanel};

/// The four reasons a list can be empty, large and compact. The story says
/// which action was clicked.
pub struct EmptyStatePanelStory {
    clicked: Option<&'static str>,
}

impl EmptyStatePanelStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { clicked: None })
    }
}

impl Story for EmptyStatePanelStory {
    fn title() -> &'static str {
        "Empty state panel"
    }

    fn description() -> &'static str {
        "What to show where content would be, told apart by why it's empty, with the next thing to do."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EmptyStatePanelStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let click = |what: &'static str, cx: &mut Context<Self>| {
            let story = cx.entity();
            move |_: &mut Window, cx: &mut App| {
                story.update(cx, |this, cx| {
                    this.clicked = Some(what);
                    cx.notify();
                })
            }
        };
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        div().w(px(320.)).child(
                            EmptyStatePanel::new(
                                "fresh",
                                EmptyReason::Fresh,
                                "No invoices yet",
                                "Send your first invoice and it will show here with its status.",
                            )
                            .primary("New invoice", click("New invoice", cx))
                            .secondary("Import from a file", click("Import", cx)),
                        ),
                    )
                    .child(
                        div().w(px(320.)).child(
                            EmptyStatePanel::new(
                                "failed",
                                EmptyReason::Failed,
                                "The bank feed didn't load",
                                "The connection timed out. Your matched lines are safe.",
                            )
                            .primary("Try again", click("Try again", cx)),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .w(px(520.))
                    .gap_2()
                    .child(
                        EmptyStatePanel::new(
                            "nomatch",
                            EmptyReason::NoMatches,
                            "No bills match \u{201c}fuel, over $5,000\u{201d}",
                            "Try a wider amount or clear the filters.",
                        )
                        .compact(true)
                        .primary("Clear filters", click("Clear filters", cx)),
                    )
                    .child(
                        EmptyStatePanel::new(
                            "locked",
                            EmptyReason::Locked,
                            "Payroll is private",
                            "Ask Maya for access to see this month's payroll.",
                        )
                        .compact(true)
                        .secondary("Ask for access", click("Ask for access", cx)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match self.clicked {
                        Some(what) => format!("Clicked: {what}"),
                        None => "Each reason has its own mark and words.".into(),
                    }),
            )
    }
}
