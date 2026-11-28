// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration picker: find a service and switch it on, without leaving.
//!
//! A search field sits over a list of services, grouped into connected and
//! available. The picked one shows on the right: the two products side by
//! side, what the connection does, and a switch. The keyboard does it all:
//! arrows move, Enter switches, Escape clears and then closes.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    switch::Switch,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One service that can be connected.
#[derive(Clone)]
pub struct Integration {
    id: SharedString,
    name: SharedString,
    domain: Option<SharedString>,
    category: SharedString,
    description: Option<SharedString>,
    features: Vec<SharedString>,
    icon: Option<IconName>,
    color: Option<Hsla>,
    connected: bool,
}

impl Integration {
    /// Create a service in a category, not yet connected.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        category: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            domain: None,
            category: category.into(),
            description: None,
            features: Vec::new(),
            icon: None,
            color: None,
            connected: false,
        }
    }

    /// Show where it lives, such as its web address.
    pub fn domain(mut self, domain: impl Into<SharedString>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Say what the connection does, in a sentence.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add something the connection can do.
    pub fn feature(mut self, feature: impl Into<SharedString>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Mark it with an icon instead of its first letter.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Colour its tile.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Start it connected.
    pub fn connected(mut self, connected: bool) -> Self {
        self.connected = connected;
        self
    }

    /// The service's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Whether it is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

/// What people did in the picker.
#[derive(Debug, Clone)]
pub enum IntegrationPickerEvent {
    /// A service was switched on or off.
    Toggled(SharedString, bool),
    /// "Learn more" was pressed for a service.
    Explore(SharedString),
    /// The back button was pressed, or Left in an empty search.
    Back,
    /// Escape was pressed in an empty search.
    Close,
}

/// The rows the list shows: connected first, then available, each matching
/// the search. Returns indexes into `all`.
fn visible(all: &[Integration], query: &str) -> (Vec<usize>, Vec<usize>) {
    let query = query.trim().to_lowercase();
    let matches = |service: &Integration| {
        query.is_empty()
            || [
                Some(&service.name),
                Some(&service.category),
                service.domain.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|text| text.to_lowercase().contains(&query))
    };
    let pick = |connected: bool| {
        all.iter()
            .enumerate()
            .filter(|(_, service)| service.connected == connected && matches(service))
            .map(|(ix, _)| ix)
            .collect()
    };
    (pick(true), pick(false))
}

/// The first letter of a name, for its tile.
fn mark(name: &str) -> String {
    name.chars()
        .next()
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default()
}

/// A command menu page for finding services and switching them on.
pub struct IntegrationPicker {
    app_name: SharedString,
    services: Vec<Integration>,
    picked: Option<SharedString>,
    with_back: bool,
    search: Entity<InputState>,
    focus: FocusHandle,
    _search_events: Subscription,
}

impl EventEmitter<IntegrationPickerEvent> for IntegrationPicker {}

impl Focusable for IntegrationPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl IntegrationPicker {
    /// Create a picker for the product called `app_name`.
    pub fn new(
        app_name: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search integrations"));
        let search_events = cx.subscribe_in(&search, window, |_, _, event, _, cx| {
            // Render moves the pick to the first match if the search hid it.
            if let InputEvent::Change = event {
                cx.notify();
            }
        });
        Self {
            app_name: app_name.into(),
            services: Vec::new(),
            picked: None,
            with_back: false,
            search,
            focus: cx.focus_handle(),
            _search_events: search_events,
        }
    }

    /// Add a service.
    pub fn service(mut self, service: Integration) -> Self {
        if self.picked.is_none() {
            self.picked = Some(service.id.clone());
        }
        self.services.push(service);
        self
    }

    /// Pick a service to start with. The first one otherwise.
    pub fn picked(mut self, id: impl Into<SharedString>) -> Self {
        self.picked = Some(id.into());
        self
    }

    /// Show a back button before the search.
    pub fn with_back(mut self, with_back: bool) -> Self {
        self.with_back = with_back;
        self
    }

    /// Put the keyboard in the search field.
    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
    }

    /// The services as they stand.
    pub fn services(&self) -> &[Integration] {
        &self.services
    }

    fn order(&self, cx: &App) -> Vec<usize> {
        let query = self.search.read(cx).value();
        let (connected, available) = visible(&self.services, &query);
        connected.into_iter().chain(available).collect()
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let order = self.order(cx);
        if order.is_empty() {
            return;
        }
        let at = self
            .picked
            .as_ref()
            .and_then(|id| order.iter().position(|ix| &self.services[*ix].id == id));
        let next = match at {
            Some(at) => (at as isize + by).clamp(0, order.len() as isize - 1) as usize,
            None => 0,
        };
        self.picked = Some(self.services[order[next]].id.clone());
        cx.notify();
    }

    fn toggle(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(service) = self.services.iter_mut().find(|service| &service.id == id) {
            service.connected = !service.connected;
            cx.emit(IntegrationPickerEvent::Toggled(
                id.clone(),
                service.connected,
            ));
            cx.notify();
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let empty = self.search.read(cx).value().is_empty();
        let modifiers = &event.keystroke.modifiers;
        match event.keystroke.key.as_str() {
            "up" => self.step(-1, cx),
            "down" => self.step(1, cx),
            "enter" => {
                if let Some(id) = self.picked.clone() {
                    self.toggle(&id, cx);
                }
            }
            "escape" if !empty => {
                self.search
                    .update(cx, |search, cx| search.set_value("", window, cx));
                cx.notify();
            }
            "escape" => cx.emit(IntegrationPickerEvent::Close),
            "left" if empty && self.with_back => cx.emit(IntegrationPickerEvent::Back),
            "/" if modifiers.control || modifiers.platform => self.focus_search(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for IntegrationPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette = [
            theme.blue,
            theme.magenta,
            theme.green,
            theme.cyan,
            theme.yellow,
            theme.red,
        ];

        let query = self.search.read(cx).value();
        let (connected, available) = visible(&self.services, &query);
        let order: Vec<usize> = connected.iter().chain(available.iter()).copied().collect();
        // If the search hid the pick, the first visible row takes over.
        if !order
            .iter()
            .any(|ix| self.picked.as_ref() == Some(&self.services[*ix].id))
        {
            self.picked = order.first().map(|ix| self.services[*ix].id.clone());
        }
        let picked = self
            .picked
            .as_ref()
            .and_then(|id| self.services.iter().find(|service| &service.id == id))
            .cloned();
        let total = self.services.len();
        let on = self
            .services
            .iter()
            .filter(|service| service.connected)
            .count();

        let tile = |service: &Integration, size: f32, ix: usize| {
            let color = service.color.unwrap_or(palette[ix % palette.len()]);
            div()
                .flex_none()
                .size(px(size))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(size * 0.24))
                .bg(color)
                .text_color(isonomic_ui::white())
                .font_medium()
                .map(|this| match &service.icon {
                    Some(icon) => this.child(
                        Icon::new(icon.clone())
                            .size(px(size * 0.55))
                            .text_color(isonomic_ui::white()),
                    ),
                    None => this.text_size(px(size * 0.5)).child(mark(&service.name)),
                })
        };

        let mut row = |ix: usize, cx: &mut Context<Self>| {
            let service = &self.services[ix];
            let is_picked = self.picked.as_ref() == Some(&service.id);
            let id = service.id.clone();
            let (hover_fade, hover_on_hover) = isonomic_ui::component::look::tracked_hover(
                ("integration-hover", ix),
                true,
                window,
                cx,
            );
            h_flex()
                .id(("integration", ix))
                .gap_2p5()
                .px_2()
                .py_1p5()
                .rounded(radius)
                .cursor_pointer()
                .when(is_picked, |this| this.bg(chip))
                .when(!is_picked, |this| {
                    this.bg(crate::common::faded(chip.opacity(0.6), hover_fade))
                })
                .on_hover(hover_on_hover)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.picked = Some(id.clone());
                    cx.notify();
                }))
                .child(tile(service, 26., ix))
                .child(
                    v_flex()
                        .min_w_0()
                        .child(div().truncate().child(service.name.clone()))
                        .child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .truncate()
                                .child(service.category.clone()),
                        ),
                )
        };
        let heading = |label: &'static str| {
            div()
                .px_2()
                .pt_2()
                .pb_1()
                .text_label_small()
                .text_color(quiet)
                .child(label)
        };

        let list = v_flex()
            .id("integration-list")
            .w(px(230.))
            .flex_none()
            .p_1p5()
            .border_r_1()
            .border_color(line)
            .overflow_y_scroll()
            .when(!connected.is_empty(), |this| {
                this.child(heading("Connected"))
            })
            .children(connected.iter().map(|ix| row(*ix, cx)))
            .when(!available.is_empty(), |this| {
                this.child(heading("Available"))
            })
            .children(available.iter().map(|ix| row(*ix, cx)))
            .when(order.is_empty(), |this| {
                this.child(
                    div()
                        .p_3()
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("Nothing matches \"{query}\"")),
                )
            });

        let detail = picked.map(|service| {
            let ix = self
                .services
                .iter()
                .position(|candidate| candidate.id == service.id)
                .unwrap_or(0);
            let toggle_id = service.id.clone();
            let explore_id = service.id.clone();
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_3()
                .p_3()
                .child(
                    h_flex()
                        .justify_center()
                        .gap_4()
                        .py_5()
                        .rounded(radius_lg)
                        .bg(chip.opacity(0.5))
                        .when(service.connected, |this| {
                            this.border_1().border_color(accent.opacity(0.35))
                        })
                        .child(
                            div()
                                .size(px(48.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(12.))
                                .bg(ink)
                                .text_color(fill)
                                .text_size(px(22.))
                                .font_medium()
                                .child(mark(&self.app_name)),
                        )
                        .child(
                            Icon::new(IconName::ChevronsUpDown)
                                .small()
                                .rotate(isonomic_ui::Radians(std::f32::consts::FRAC_PI_2))
                                .text_color(if service.connected { accent } else { quiet }),
                        )
                        .child(tile(&service, 48., ix)),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .font_medium()
                                        .child(format!("{} + {}", self.app_name, service.name)),
                                )
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .text_caption()
                                        .text_color(quiet)
                                        .children(service.domain.clone())
                                        .when(service.connected, |this| {
                                            this.child(
                                                div()
                                                    .px_1()
                                                    .rounded(px(4.))
                                                    .bg(success.opacity(0.14))
                                                    .text_color(success)
                                                    .child("Connected"),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            Switch::new("integration-switch")
                                .checked(service.connected)
                                .accessibility_label(format!("Connect {}", service.name))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.toggle(&toggle_id, cx)),
                                ),
                        ),
                )
                .children(service.description.clone().map(|description| {
                    div()
                        .text_color(quiet)
                        .line_height(relative(1.45))
                        .child(description)
                }))
                .child(
                    v_flex()
                        .gap_1()
                        .children(service.features.iter().map(|feature| {
                            h_flex()
                                .gap_2()
                                .child(Icon::new(IconName::Check).xsmall().text_color(quiet))
                                .child(feature.clone())
                        })),
                )
                .child(
                    h_flex().pt_1().child(
                        Button::new("integration-explore")
                            .outline()
                            .small()
                            .label("Learn more")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(IntegrationPickerEvent::Explore(explore_id.clone()))
                            })),
                    ),
                )
        });

        let key = |label: &'static str| {
            div()
                .px_1()
                .rounded(px(4.))
                .border_1()
                .border_color(line)
                .child(label)
        };

        v_flex()
            .key_context("IntegrationPicker")
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::key_down))
            .w_full()
            .max_w(px(680.))
            .h(px(440.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(line)
                    .when(self.with_back, |this| {
                        this.child(
                            Button::new("integration-back")
                                .ghost()
                                .xsmall()
                                .icon(IconName::ArrowLeft)
                                .accessibility_label("Back")
                                .on_click(
                                    cx.listener(|_, _, _, cx| {
                                        cx.emit(IntegrationPickerEvent::Back)
                                    }),
                                ),
                        )
                    })
                    .child(Icon::new(IconName::Search).small().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.search).appearance(false)),
                    )
                    .child(div().text_caption().text_color(quiet).child(key("Ctrl /"))),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(list)
                    .children(detail),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .text_caption()
                    .text_color(quiet)
                    .child(div().size(px(6.)).rounded_full().bg(if on > 0 {
                        success
                    } else {
                        quiet
                    }))
                    .child(div().flex_1().child(format!("{on} of {total} connected")))
                    .child(key("Up"))
                    .child(key("Down"))
                    .child("to move")
                    .child(div().w_2())
                    .child(key("Enter"))
                    .child("to switch"),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn services() -> Vec<Integration> {
        vec![
            Integration::new("a", "Bankfeed", "Banking").connected(true),
            Integration::new("b", "Taxbox", "Tax").domain("taxbox.example"),
            Integration::new("c", "Paystream", "Payments").connected(true),
            Integration::new("d", "Payroller", "Payroll"),
        ]
    }

    #[test]
    fn connected_come_first_and_search_narrows() {
        assert_eq!(visible(&services(), ""), (vec![0, 2], vec![1, 3]));
        assert_eq!(visible(&services(), "pay"), (vec![2], vec![3]));
        assert_eq!(visible(&services(), "EXAMPLE"), (vec![], vec![1]));
        assert_eq!(visible(&services(), "nothing"), (vec![], vec![]));
    }

    #[test]
    fn tiles_use_the_first_letter() {
        assert_eq!(mark("ledgerline"), "L");
        assert_eq!(mark(""), "");
    }
}
