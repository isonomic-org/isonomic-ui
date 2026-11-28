// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration settings card: the settings of one connected service.
//!
//! A switch turns the service on or off at once. Below it, an API key can be
//! made (and copied once), the site address is checked as it is typed, and a
//! time zone is picked. Those changes only count once Save is pressed: a bar
//! with Discard and Save shows as soon as anything differs.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    rc::Rc,
};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    switch::Switch,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// The values the card saves.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntegrationValues {
    /// Whether the service is switched on.
    pub enabled: bool,
    /// The site address, without the scheme.
    pub site: String,
    /// The picked time zone.
    pub time_zone: Option<SharedString>,
    /// The API key, once one has been made.
    pub api_key: Option<String>,
}

/// What happened in the card.
#[derive(Debug, Clone)]
pub enum IntegrationSettingsEvent {
    /// The switch changed. This counts at once, without Save.
    Toggled(bool),
    /// Save was pressed, with every value.
    Saved(IntegrationValues),
    /// A new API key was made. It is saved with the rest.
    KeyMade,
}

type KeyMaker = Rc<dyn Fn() -> String>;

/// Whether a site address looks like a domain: letters, digits and hyphens
/// in at least two dot-separated parts, with no part empty.
fn valid_site(site: &str) -> bool {
    let host = site.trim().trim_end_matches('/');
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && parts
            .last()
            .is_some_and(|tld| tld.chars().all(|c| c.is_ascii_alphabetic()) && tld.len() >= 2)
}

/// Drop a scheme someone pasted in, since the field shows its own.
fn without_scheme(site: &str) -> &str {
    site.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
}

/// A key with only its last characters showing: "••••••••3f9a".
fn masked(key: &str) -> String {
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}{tail}", "\u{2022}".repeat(8))
}

/// A random key, for when the owner gives no way to make one.
fn local_key() -> String {
    let mut out = String::from("sk_");
    for _ in 0..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u8(0);
        out.push_str(&format!("{:016x}", hasher.finish()));
    }
    out
}

/// Settings for one connected service that save only on confirm.
pub struct IntegrationSettingsCard {
    name: SharedString,
    icon: Option<IconName>,
    count: Option<usize>,
    meta: Option<SharedString>,
    site_hint: Option<SharedString>,
    time_zones: Vec<SharedString>,
    key_maker: Option<KeyMaker>,
    saved: IntegrationValues,
    values: IntegrationValues,
    fresh_key: bool,
    site: Entity<InputState>,
    focus: FocusHandle,
    _site_events: Subscription,
}

impl EventEmitter<IntegrationSettingsEvent> for IntegrationSettingsCard {}

impl Focusable for IntegrationSettingsCard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl IntegrationSettingsCard {
    /// Create a card for the service called `name`, starting from `values`.
    pub fn new(
        name: impl Into<SharedString>,
        values: IntegrationValues,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let start = values.site.clone();
        let site = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("yourcompany.com");
            state.set_value(start, window, cx);
            state
        });
        let site_events = cx.subscribe_in(&site, window, |this, input, event, window, cx| {
            if let InputEvent::Change = event {
                let typed = input.read(cx).value().to_string();
                let site = without_scheme(&typed).to_string();
                // The field shows its own scheme, so a pasted one is dropped.
                if site.len() != typed.trim().len() {
                    let shown = site.clone();
                    input.update(cx, |input, cx| input.set_value(shown, window, cx));
                }
                this.values.site = site;
                cx.notify();
            }
        });
        Self {
            name: name.into(),
            icon: None,
            count: None,
            meta: None,
            site_hint: None,
            time_zones: Vec::new(),
            key_maker: None,
            saved: values.clone(),
            values,
            fresh_key: false,
            site,
            focus: cx.focus_handle(),
            _site_events: site_events,
        }
    }

    /// Show an icon in the header tile.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Show a small number next to the name, such as how many items sync.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Show a line under the name, such as when it was connected.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Show help under the address field.
    pub fn site_hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.site_hint = Some(hint.into());
        self
    }

    /// Set the time zones to pick from.
    pub fn time_zones(mut self, zones: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.time_zones = zones.into_iter().map(Into::into).collect();
        self
    }

    /// Make API keys your way. A random key is made otherwise.
    pub fn key_maker(mut self, maker: impl Fn() -> String + 'static) -> Self {
        self.key_maker = Some(Rc::new(maker));
        self
    }

    /// The values as they stand, saved or not.
    pub fn values(&self) -> &IntegrationValues {
        &self.values
    }

    fn dirty(&self) -> bool {
        IntegrationValues {
            enabled: self.saved.enabled,
            ..self.values.clone()
        } != self.saved
    }

    fn toggle(&mut self, on: bool, cx: &mut Context<Self>) {
        self.values.enabled = on;
        self.saved.enabled = on;
        cx.emit(IntegrationSettingsEvent::Toggled(on));
        cx.notify();
    }

    fn make_key(&mut self, cx: &mut Context<Self>) {
        let key = match &self.key_maker {
            Some(maker) => maker(),
            None => local_key(),
        };
        self.values.api_key = Some(key);
        self.fresh_key = true;
        cx.emit(IntegrationSettingsEvent::KeyMade);
        cx.notify();
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.values = self.saved.clone();
        self.fresh_key = false;
        let site = self.saved.site.clone();
        self.site
            .update(cx, |input, cx| input.set_value(site, window, cx));
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if !valid_site(&self.values.site) {
            return;
        }
        self.saved = self.values.clone();
        self.fresh_key = false;
        cx.emit(IntegrationSettingsEvent::Saved(self.values.clone()));
        cx.notify();
    }
}

impl Render for IntegrationSettingsCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let success = theme.success;
        let danger = theme.danger;
        let warning = theme.warning;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();

        let on = self.values.enabled;
        let site_ok = valid_site(&self.values.site);
        let site_empty = self.values.site.trim().is_empty();
        let dirty = self.dirty();
        let this = cx.entity();

        let label = |text: &'static str| div().font_medium().child(text);
        let section = || {
            v_flex()
                .gap_1p5()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
        };

        let key_row = match (&self.values.api_key, self.fresh_key) {
            (Some(key), true) => {
                let copy = key.clone();
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .px_3()
                            .h(px(34.))
                            .rounded(radius)
                            .border_1()
                            .border_color(line)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(mono)
                                    .text_caption()
                                    .child(key.clone()),
                            )
                            .child(
                                Button::new("settings-copy-key")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .label("Copy")
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copy.clone(),
                                        ))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_caption()
                            .text_color(warning)
                            .child("Copy it now. Once saved, only its last characters show."),
                    )
                    .into_any_element()
            }
            (Some(key), false) => h_flex()
                .gap_2()
                .px_3()
                .h(px(34.))
                .rounded(radius)
                .border_1()
                .border_color(line)
                .child(
                    div()
                        .flex_1()
                        .font_family(mono)
                        .text_caption()
                        .text_color(quiet)
                        .child(masked(key)),
                )
                .child(
                    Button::new("settings-new-key")
                        .ghost()
                        .xsmall()
                        .label("Make a new one")
                        .on_click(cx.listener(|this, _, _, cx| this.make_key(cx))),
                )
                .into_any_element(),
            (None, _) => h_flex()
                .id("settings-make-key")
                .gap_2()
                .px_3()
                .h(px(34.))
                .rounded(radius)
                .bg(chip)
                .cursor_pointer()
                .hover(|style| style.bg(chip.opacity(0.7)))
                .on_click(cx.listener(|this, _, _, cx| this.make_key(cx)))
                .child(div().flex_1().text_color(quiet).child("Make an API key"))
                .child(Icon::new(IconName::Plus).small().text_color(quiet))
                .into_any_element(),
        };

        let zone_words = self
            .values
            .time_zone
            .clone()
            .unwrap_or_else(|| "Pick a time zone".into());
        let zones = self.time_zones.clone();
        let current_zone = self.values.time_zone.clone();
        let zone_picker = Button::new("settings-zone")
            .outline()
            .w_full()
            .accessibility_label(zone_words.clone())
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(div().truncate().child(zone_words))
                    .child(Icon::new(IconName::ChevronDown).small().text_color(quiet)),
            )
            .dropdown_menu(move |mut menu, _, _| {
                for zone in &zones {
                    let this = this.clone();
                    let pick = zone.clone();
                    menu = menu.item(
                        PopupMenuItem::new(zone.clone())
                            .checked(current_zone.as_ref() == Some(zone))
                            .on_click(move |_, _, cx| {
                                this.update(cx, |this, cx| {
                                    this.values.time_zone = Some(pick.clone());
                                    cx.notify();
                                })
                            }),
                    );
                }
                menu
            });

        let body =
            on.then(|| {
                v_flex()
                    .child(section().child(label("API key")).child(key_row))
                    .child(
                        section()
                            .child(label("Site address"))
                            .child(
                                h_flex()
                                    .h(px(34.))
                                    .pl_3()
                                    .pr_2()
                                    .rounded(radius)
                                    .border_1()
                                    .border_color(if !site_ok && !site_empty {
                                        danger
                                    } else {
                                        line
                                    })
                                    .child(div().text_color(quiet).child("https://"))
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&self.site).appearance(false)),
                                    )
                                    .when(site_ok, |this| {
                                        this.child(
                                            Icon::new(IconName::Check).small().text_color(success),
                                        )
                                    }),
                            )
                            .map(|this| {
                                if !site_ok && !site_empty {
                                    this.child(
                                        div()
                                            .text_caption()
                                            .text_color(danger)
                                            .child("Enter a domain such as yourcompany.com"),
                                    )
                                } else {
                                    this.children(self.site_hint.clone().map(|hint| {
                                        div().text_caption().text_color(quiet).child(hint)
                                    }))
                                }
                            }),
                    )
                    .child(section().child(label("Time zone")).child(zone_picker))
            });

        let bar = (on && dirty).then(|| {
            h_flex()
                .gap_2()
                .px_4()
                .py_2p5()
                .border_t_1()
                .border_color(line)
                .bg(chip.opacity(0.5))
                .child(
                    h_flex()
                        .flex_1()
                        .gap_1p5()
                        .text_caption()
                        .text_color(quiet)
                        .child(div().size(px(6.)).rounded_full().bg(warning))
                        .child("Unsaved changes"),
                )
                .child(
                    Button::new("settings-discard")
                        .ghost()
                        .small()
                        .label("Discard")
                        .on_click(cx.listener(|this, _, window, cx| this.discard(window, cx))),
                )
                .child(
                    Button::new("settings-save")
                        .primary()
                        .small()
                        .label("Save")
                        .disabled(!site_ok)
                        .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                )
        });

        v_flex()
            .track_focus(&self.focus)
            .w_full()
            .max_w(px(460.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.35))
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .flex_none()
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(radius)
                            .border_1()
                            .border_color(line)
                            .child(
                                Icon::new(self.icon.clone().unwrap_or(IconName::Globe))
                                    .text_color(if on { ink } else { quiet }),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(div().font_medium().truncate().child(self.name.clone()))
                                    .children(self.count.map(|count| {
                                        div()
                                            .px_1p5()
                                            .rounded(px(4.))
                                            .bg(chip)
                                            .text_caption()
                                            .text_color(quiet)
                                            .child(count.to_string())
                                    })),
                            )
                            .child(div().text_caption().text_color(quiet).child(if on {
                                self.meta.clone().unwrap_or_default()
                            } else {
                                SharedString::from(
                                    "Paused. Nothing syncs until it is switched on again.",
                                )
                            })),
                    )
                    .child(
                        Switch::new("settings-switch")
                            .checked(on)
                            .accessibility_label(self.name.clone())
                            .on_click(cx.listener(|this, on: &bool, _, cx| this.toggle(*on, cx))),
                    ),
            )
            .children(body)
            .children(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_are_checked() {
        assert!(valid_site("ledger-and-co.com"));
        assert!(valid_site("shop.example.co.uk/"));
        assert!(!valid_site("localhost"));
        assert!(!valid_site("bad domain.com"));
        assert!(!valid_site("-start.com"));
        assert!(!valid_site("example.c"));
        assert!(!valid_site("example..com"));
    }

    #[test]
    fn a_pasted_scheme_is_dropped() {
        assert_eq!(without_scheme("https://ledger.example"), "ledger.example");
        assert_eq!(without_scheme(" http://a.io"), "a.io");
    }

    #[test]
    fn keys_show_only_their_end() {
        assert_eq!(
            masked("sk_0123456789abcd"),
            format!("{}abcd", "\u{2022}".repeat(8))
        );
        let key = local_key();
        assert!(key.starts_with("sk_") && key.len() == 35);
        assert_ne!(key, local_key());
    }
}
