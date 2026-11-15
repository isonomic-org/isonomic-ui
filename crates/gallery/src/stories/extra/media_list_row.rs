//! Gallery story for the media list row.

use crate::Story;
use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::media_list_row::{MediaListRow, RowMedia};

/// What's in scope for a conversation with a bookkeeping assistant: files,
/// connections and tools, one compact row each. Click a row to pick it, or
/// Tab to it and press Enter.
pub struct MediaListRowStory {
    picked: usize,
    removed: Vec<usize>,
}

impl MediaListRowStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: 0,
            removed: Vec::new(),
        })
    }
}

impl Story for MediaListRowStory {
    fn title() -> &'static str {
        "Media list row"
    }

    fn description() -> &'static str {
        "One compact row with a small picture or icon, a title over a quieter line, and an optional control at the end."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MediaListRowStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (line, quiet, success) = (theme.border, theme.muted_foreground, theme.success);
        let rows = [
            (
                IconName::FileText,
                "March bank statement for the operating account.csv",
                "18 KB \u{b7} added by you",
            ),
            (
                IconName::File,
                "Receipts Q1.zip",
                "2.4 MB \u{b7} 214 receipts",
            ),
            (
                IconName::Globe,
                "Bank feed",
                "Read only \u{b7} synced 4 minutes ago",
            ),
            (IconName::Inbox, "Mail", "Drafts only"),
        ];
        let picked = self.picked;
        v_flex()
            .w(px(300.))
            .gap_1()
            .p_1()
            .rounded(px(8.))
            .border_1()
            .border_color(line)
            .children(
                rows.into_iter()
                    .enumerate()
                    .filter(|(ix, _)| !self.removed.contains(ix))
                    .map(|(ix, (icon, title, note))| {
                        let pick = cx.entity();
                        let remove = cx.entity();
                        let row = MediaListRow::new(("context-row", ix), title)
                            .media(RowMedia::Icon(icon))
                            .secondary(note)
                            .picked(ix == picked)
                            .on_click(move |_, _, cx| {
                                pick.update(cx, |this, cx| {
                                    this.picked = ix;
                                    cx.notify();
                                })
                            });
                        if ix < 2 {
                            row.trailing(
                                Button::new(("remove-row", ix))
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .accessibility_label(format!("Remove {title}"))
                                    .on_click(move |_, _, cx| {
                                        remove.update(cx, |this, cx| {
                                            this.removed.push(ix);
                                            cx.notify();
                                        })
                                    }),
                            )
                        } else {
                            row.trailing(div().text_xs().text_color(success).child("On"))
                        }
                    }),
            )
            .child(
                MediaListRow::new("payroll-row", "Payroll")
                    .media(RowMedia::Icon(IconName::User))
                    .secondary("Not on the Starter plan")
                    .disabled(true)
                    .compact(),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(quiet)
                    .child("Long names end in an ellipsis; a screen reader hears them in full."),
            )
    }
}
