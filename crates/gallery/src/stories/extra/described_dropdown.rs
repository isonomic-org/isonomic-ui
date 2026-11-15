//! Gallery story for the described dropdown.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::described_dropdown::{DescribedChoice, DescribedDropdown};

/// How often the profit and loss report is emailed, and who sees it. The
/// story keeps both choices.
pub struct DescribedDropdownStory {
    schedule: Option<SharedString>,
    audience: Option<SharedString>,
}

impl DescribedDropdownStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            schedule: Some("weekly".into()),
            audience: None,
        })
    }
}

impl Story for DescribedDropdownStory {
    fn title() -> &'static str {
        "Described dropdown"
    }

    fn description() -> &'static str {
        "A button whose menu explains each choice in a second line, marks the current one, and keeps destructive choices apart."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DescribedDropdownStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (a, b) = (cx.entity(), cx.entity());
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        DescribedDropdown::new(
                            "schedule",
                            "Send",
                            vec![
                                DescribedChoice::new("daily", "Daily", "Every weekday at 8 am"),
                                DescribedChoice::new("weekly", "Weekly", "Every Monday at 9 am"),
                                DescribedChoice::new(
                                    "monthly",
                                    "Monthly",
                                    "On the 1st, after the close",
                                ),
                                DescribedChoice::new(
                                    "off",
                                    "Stop sending",
                                    "Nobody gets it any more",
                                )
                                .destructive(),
                            ],
                        )
                        .current(self.schedule.clone())
                        .on_pick(move |id, _, cx| {
                            a.update(cx, |this, cx| {
                                this.schedule = Some(id.clone());
                                cx.notify();
                            })
                        }),
                    )
                    .child(
                        DescribedDropdown::new(
                            "audience",
                            "Share with",
                            vec![
                                DescribedChoice::new("me", "Just me", "Only you see it"),
                                DescribedChoice::new(
                                    "team",
                                    "The team",
                                    "Everyone in the workspace",
                                ),
                                DescribedChoice::new(
                                    "accountant",
                                    "The accountant",
                                    "Read-only, by email link",
                                ),
                            ],
                        )
                        .current(self.audience.clone())
                        .on_pick(move |id, _, cx| {
                            b.update(cx, |this, cx| {
                                this.audience = Some(id.clone());
                                cx.notify();
                            })
                        }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("The profit and loss report, emailed."),
            )
    }
}
