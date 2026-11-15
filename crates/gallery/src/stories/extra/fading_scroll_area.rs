//! Gallery story for the fading scroll area.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px};
use isonomic_ui_extra::fading_scroll_area::FadingScrollArea;

/// A long list of bank transactions in a short box, and a short list that fits.
pub struct FadingScrollAreaStory;

impl FadingScrollAreaStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for FadingScrollAreaStory {
    fn title() -> &'static str {
        "Fading scroll area"
    }

    fn description() -> &'static str {
        "A box that scrolls its content and fades the edge where more is hidden, with a way back to the top."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

const PAYEES: [&str; 6] = ["Harbor Dental", "Fuel card", "Brightline Cafe", "Office lease", "Payroll", "Software"];

impl Render for FadingScrollAreaStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (line, quiet) = (theme.border, theme.muted_foreground);
        let row = move |ix: usize| {
            let amount = 40 + (ix * 137) % 2900;
            h_flex()
                .justify_between()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(line)
                .text_sm()
                .child(v_flex().child(PAYEES[ix % PAYEES.len()]).child(div().text_xs().text_color(quiet).child(format!("Nov {}", 30 - ix % 30))))
                .child(format!("${amount}.00"))
        };
        h_flex()
            .items_start()
            .gap_6()
            .child(
                div().w(px(320.)).rounded(px(8.)).border_1().border_color(line).overflow_hidden().child(
                    FadingScrollArea::new("transactions", px(260.)).label("Bank transactions").children((0..60).map(row)),
                ),
            )
            .child(
                div().w(px(260.)).rounded(px(8.)).border_1().border_color(line).overflow_hidden().child(
                    FadingScrollArea::new("short", px(260.)).label("Pending").children((0..3).map(row)),
                ),
            )
    }
}
