//! Gallery story for the filter menu.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::filter_menu::{FilterGroup, FilterMenu, FilterSelection};

/// Shows a menu that starts with three filters and reports every change.
pub struct FilterMenuStory {
    last: SharedString,
}

impl FilterMenuStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "3 filters at the start".into(),
        })
    }
}

impl Story for FilterMenuStory {
    fn title() -> &'static str {
        "Filter menu"
    }

    fn description() -> &'static str {
        "A few groups of options to filter a list, table or board by."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FilterMenuStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();

        v_flex()
            .gap_3()
            .child(
                FilterMenu::new("invoices")
                    .group(
                        FilterGroup::new("status", "Status")
                            .option("draft", "Draft")
                            .option("sent", "Sent")
                            .option("overdue", "Overdue")
                            .option("paid", "Paid"),
                    )
                    .group(
                        FilterGroup::new("amount", "Amount")
                            .chip_prefix("Amount: ")
                            .option("small", "Under 500")
                            .option("medium", "500 to 5,000")
                            .option("large", "Over 5,000"),
                    )
                    .group(
                        FilterGroup::new("customer", "Customer type")
                            .option("retail", "Retail")
                            .option("wholesale", "Wholesale")
                            .option("public", "Public sector"),
                    )
                    .default_value(
                        FilterSelection::new()
                            .with("status", "overdue")
                            .with("amount", "medium")
                            .with("customer", "wholesale"),
                    )
                    .on_change(move |picked, _, cx| {
                        let line = match picked.len() {
                            0 => "No filters".to_string(),
                            1 => "1 filter".to_string(),
                            n => format!("{n} filters"),
                        };
                        story.update(cx, |this, cx| {
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
