//! Gallery story for the path breadcrumb.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::path_breadcrumb::PathBreadcrumb;

/// A deep folder path that folds, and a short one that doesn't. Clicking a
/// place moves there.
pub struct PathBreadcrumbStory {
    path: Vec<SharedString>,
}

fn full() -> Vec<SharedString> {
    [
        "Bookkeeping",
        "Clients",
        "Northwind Dental",
        "2026",
        "March",
        "Receipts",
        "Fuel",
    ]
    .into_iter()
    .map(SharedString::from)
    .collect()
}

impl PathBreadcrumbStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { path: full() })
    }
}

impl Story for PathBreadcrumbStory {
    fn title() -> &'static str {
        "Path breadcrumb"
    }

    fn description() -> &'static str {
        "Where you are as a trail of places, folding the middle into a menu when it's long."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PathBreadcrumbStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let go = cx.entity();
        v_flex()
            .gap_4()
            .child(
                PathBreadcrumb::new("receipts-path", self.path.clone()).on_go(move |ix, _, cx| {
                    go.update(cx, |this, cx| {
                        this.path.truncate(ix + 1);
                        cx.notify();
                    })
                }),
            )
            .child(PathBreadcrumb::new(
                "short-path",
                vec!["Reports".into(), "Profit and loss".into()],
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.path.len() < 7 {
                        "Moved up. Reopen the story to start again."
                    } else {
                        "Click a place, or the \u{2026} for the ones folded away."
                    }),
            )
    }
}
