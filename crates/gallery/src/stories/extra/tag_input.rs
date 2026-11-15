//! Gallery story for the tag input.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::tag_input::{TagInput, TagInputEvent};

/// A loose check, enough for a demo: something before the @ and a dot after it.
fn looks_like_an_address(value: &str) -> bool {
    value.split_once('@').is_some_and(|(name, host)| {
        !name.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
    })
}

/// Shows a field for addresses with a check and a limit, and a plain one for labels.
pub struct TagInputStory {
    people: Entity<TagInput>,
    labels: Entity<TagInput>,
    last: SharedString,
    _events: Subscription,
}

impl TagInputStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let people = cx.new(|cx| {
                TagInput::new(window, cx)
                    .label("Share the report with")
                    .hint("Paste a whole list: commas, spaces or new lines.")
                    .avatar(true)
                    .split_on_space(true)
                    .max(6)
                    .normalize(|value| value.to_lowercase())
                    .validate(|value| {
                        if looks_like_an_address(value) {
                            Ok(())
                        } else {
                            Err(format!("\"{value}\" does not look like an address.").into())
                        }
                    })
                    .with_tags(["imani@books.example", "tomas@books.example"], window, cx)
            });
            let labels = cx.new(|cx| {
                TagInput::new(window, cx)
                    .label("Labels")
                    .optional(true)
                    .hint("Press Enter or a comma after each one.")
                    .with_tags(["Year end", "VAT"], window, cx)
            });
            let events = cx.subscribe(&people, |this: &mut Self, _, event, cx| {
                let TagInputEvent::Change(tags) = event;
                this.last = format!("Shared with {}", tags.len()).into();
                cx.notify();
            });

            Self {
                people,
                labels,
                last: "Shared with 2".into(),
                _events: events,
            }
        })
    }
}

impl Story for TagInputStory {
    fn title() -> &'static str {
        "Tag input"
    }

    fn description() -> &'static str {
        "Several short values in one field, as chips."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for TagInputStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_5()
            .max_w(px(400.))
            .child(self.people.clone())
            .child(self.labels.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
