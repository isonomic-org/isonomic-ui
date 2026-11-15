//! Gallery story for the tips grid.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, kbd::Kbd, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Keystroke, ParentElement as _, Render,
    SharedString, Styled as _, Window, div, px,
};
use isonomic_ui_extra::tips_grid::{Tip, TipsGrid};

/// Shows six tips in three columns, one of them with a key cap, and a close row.
pub struct TipsGridStory {
    last: SharedString,
}

impl TipsGridStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing taken yet".into(),
        })
    }
}

impl Story for TipsGridStory {
    fn title() -> &'static str {
        "Tips grid"
    }

    fn description() -> &'static str {
        "A panel of short tips, each with one action."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TipsGridStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let report = move |line: &'static str| {
            let view = view.clone();
            move |_: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| {
                    this.last = line.into();
                    cx.notify();
                })
            }
        };
        let find = Keystroke::parse("cmd-f").ok().map(Kbd::new);

        v_flex()
            .w_full()
            .max_w(px(680.))
            .gap_3()
            .child(
                TipsGrid::new("tips")
                    .title("Worth a try")
                    .max_height(px(300.))
                    .tip(
                        Tip::new("find", "Find in ledger")
                            .description(
                                h_flex()
                                    .flex_wrap()
                                    .gap_1()
                                    .child("Press")
                                    .children(find)
                                    .child("to search every entry."),
                            )
                            .on_action(report("Find in ledger")),
                    )
                    .tip(
                        Tip::new("rules", "Matching rules")
                            .description(
                                "Let recurring payments match their invoices by themselves.",
                            )
                            .action("Set up")
                            .done_label("Set up")
                            .on_action(report("Matching rules")),
                    )
                    .tip(
                        Tip::new("digest", "Friday digest")
                            .description("A short mail with what is still unmatched.")
                            .action("Turn on")
                            .done_label("Turned on")
                            .on_action(report("Friday digest")),
                    )
                    .tip(
                        Tip::new("export", "Export for your accountant")
                            .description("One file with the year's entries and receipts.")
                            .action("Export")
                            .on_action(report("Export")),
                    )
                    .tip(
                        Tip::new("phone", "Receipts from your phone")
                            .description("Photograph a receipt and it lands on the right entry.")
                            .action("Get the app"),
                    )
                    .tip(
                        Tip::new("plan", "More than one company?")
                            .description("Keep separate ledgers under one sign-in.")
                            .action("See plans")
                            .primary(true),
                    )
                    .on_close(report("Closed")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
