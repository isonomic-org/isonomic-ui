//! Gallery story for the search picker.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::search_picker::{SearchPicker, SearchPickerEvent};

/// A single customer picker and a multi tag picker that can create tags.
pub struct SearchPickerStory {
    customer: Entity<SearchPicker>,
    tags: Entity<SearchPicker>,
    note: String,
    _events: Vec<Subscription>,
}

fn customers() -> Vec<SharedString> {
    [
        "Northwind Dental",
        "Harbour Supply",
        "Kestrel Print",
        "Bayview Rentals",
        "Oak & Iron",
        "Northline Fuel",
        "Juniper Cafe",
    ]
    .into_iter()
    .map(SharedString::from)
    .collect()
}

impl SearchPickerStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let customer = cx.new(|cx| SearchPicker::new(customers(), false, window, cx));
            let tags = cx.new(|cx| {
                let mut picker = SearchPicker::new(
                    ["Fuel", "Rent", "Software", "Travel", "Meals", "Office"]
                        .into_iter()
                        .map(SharedString::from)
                        .collect(),
                    true,
                    window,
                    cx,
                );
                picker.set_picked(vec!["Fuel".into()], cx);
                picker
            });
            let a = cx.subscribe(&customer, |this: &mut Self, _, event, cx| {
                if let SearchPickerEvent::Picked(picked) = event {
                    this.note = format!(
                        "Customer: {}",
                        picked.first().map(|p| p.to_string()).unwrap_or_default()
                    );
                    cx.notify();
                }
            });
            let b = cx.subscribe(&tags, |this: &mut Self, tags, event, cx| {
                match event {
                    SearchPickerEvent::Create(name) => {
                        let name = name.clone();
                        tags.update(cx, |picker, cx| {
                            let mut options = picker_options();
                            options.push(name.clone());
                            picker.set_options(options, cx);
                        });
                        this.note = format!("Created the tag {name}");
                    }
                    SearchPickerEvent::Picked(picked) => {
                        this.note = format!(
                            "Tags: {}",
                            picked
                                .iter()
                                .map(|p| p.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
                cx.notify();
            });
            Self {
                customer,
                tags,
                note: String::new(),
                _events: vec![a, b],
            }
        })
    }
}

fn picker_options() -> Vec<SharedString> {
    ["Fuel", "Rent", "Software", "Travel", "Meals", "Office"]
        .into_iter()
        .map(SharedString::from)
        .collect()
}

impl Story for SearchPickerStory {
    fn title() -> &'static str {
        "Search picker"
    }

    fn description() -> &'static str {
        "A field that filters a list as you type, highlights the matching letters, picks one or many as chips, and offers to create what isn't there."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SearchPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(quiet)
                                    .child("Customer (try \u{201c}nwd\u{201d})"),
                            )
                            .child(self.customer.clone()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(quiet)
                                    .child("Tags (type a new one to create it)"),
                            )
                            .child(self.tags.clone()),
                    ),
            )
            .child(div().text_xs().text_color(quiet).child(self.note.clone()))
    }
}
