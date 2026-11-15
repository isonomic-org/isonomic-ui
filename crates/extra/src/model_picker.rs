//! Model picker: choose which model answers, and the settings that go with it.
//!
//! A small trigger for a composer toolbar opens a menu. Each model has a short
//! line and a row of five-step meters under headings the owner names, and the
//! footer carries a note for the model the pointer or the arrows are on, so
//! what a choice costs is on screen before it is made. Under the models sit
//! switches for whatever travels with the model; a switch can unfold a choice
//! of levels while it is on.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    kbd::Kbd,
    popover::{Popover, PopoverState},
    switch::Switch,
    v_flex,
};
use isonomic_ui::{
    Anchor, AnyElement, App, Context, ElementId, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, Keystroke, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

/// How many steps a meter has.
const STEPS: usize = 5;
/// The width of one meter column.
const COLUMN: f32 = 46.;

/// A model on offer.
#[derive(Clone)]
pub struct ModelOption {
    id: SharedString,
    name: SharedString,
    tagline: Option<SharedString>,
    icon: Option<IconName>,
    ratings: Vec<u8>,
    note: Option<SharedString>,
}

impl ModelOption {
    /// Create a model.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tagline: None,
            icon: None,
            ratings: Vec::new(),
            note: None,
        }
    }

    /// Two or three words on what the model is for.
    pub fn tagline(mut self, tagline: impl Into<SharedString>) -> Self {
        self.tagline = Some(tagline.into());
        self
    }

    /// An icon for the tile. Without one the tile shows the first letter.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Rate the model from 0 to 5 under each heading of the picker, in order.
    /// A model with no ratings, such as one that routes to others, shows
    /// "Not fixed" in place of the meters.
    pub fn ratings(mut self, ratings: impl IntoIterator<Item = u8>) -> Self {
        self.ratings = ratings.into_iter().collect();
        self
    }

    /// A line for the footer while this model is pointed at, such as how soon
    /// it starts and what it costs.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// A switch that travels with the model, with an optional choice of levels.
#[derive(Clone)]
pub struct PickerSetting {
    id: SharedString,
    label: SharedString,
    icon: Option<IconName>,
    on: Option<bool>,
    default_on: bool,
    levels: Vec<SharedString>,
    levels_label: SharedString,
    level: Option<usize>,
    default_level: usize,
}

impl PickerSetting {
    /// Create a setting that starts switched off.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            on: None,
            default_on: false,
            levels: Vec::new(),
            levels_label: "Level".into(),
            level: None,
            default_level: 0,
        }
    }

    /// An icon in front of the label. It also shows on the trigger while the
    /// setting is on.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Control the switch. Pair it with `on_setting_change`.
    pub fn on(mut self, on: bool) -> Self {
        self.on = Some(on);
        self
    }

    /// Set the switch at first, when the picker keeps it itself.
    pub fn default_on(mut self, on: bool) -> Self {
        self.default_on = on;
        self
    }

    /// Offer these levels while the switch is on.
    pub fn levels(mut self, levels: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.levels = levels.into_iter().map(Into::into).collect();
        self
    }

    /// Set the label in front of the levels.
    pub fn levels_label(mut self, label: impl Into<SharedString>) -> Self {
        self.levels_label = label.into();
        self
    }

    /// Control the level, by position. Pair it with `on_setting_change`.
    pub fn level(mut self, level: usize) -> Self {
        self.level = Some(level);
        self
    }

    /// Set the level at first, when the picker keeps it itself.
    pub fn default_level(mut self, level: usize) -> Self {
        self.default_level = level;
        self
    }
}

/// What changed about a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingChange {
    /// The switch was set.
    Switched(bool),
    /// A level was picked, by position.
    Level(usize),
}

type ChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type SettingHandler = Rc<dyn Fn(&SharedString, SettingChange, &mut Window, &mut App)>;
type SetHandler = Rc<dyn Fn(usize, SettingChange, &mut Window, &mut App)>;

/// A trigger and menu for choosing a model and its settings.
#[derive(IntoElement)]
pub struct ModelPicker {
    id: ElementId,
    style: StyleRefinement,
    models: Vec<ModelOption>,
    columns: Vec<SharedString>,
    settings: Vec<PickerSetting>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    above: bool,
    align_end: bool,
    on_change: Option<ChangeHandler>,
    on_setting_change: Option<SettingHandler>,
}

impl ModelPicker {
    /// Create a picker with no models.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            models: Vec::new(),
            columns: Vec::new(),
            settings: Vec::new(),
            value: None,
            default_value: None,
            above: false,
            align_end: false,
            on_change: None,
            on_setting_change: None,
        }
    }

    /// Add a model after the ones already added.
    pub fn model(mut self, model: ModelOption) -> Self {
        self.models.push(model);
        self
    }

    /// Name what the meters measure, such as speed and price. Each model's
    /// ratings follow this order.
    pub fn columns(mut self, columns: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.columns = columns.into_iter().map(Into::into).collect();
        self
    }

    /// Add a setting under the models.
    pub fn setting(mut self, setting: PickerSetting) -> Self {
        self.settings.push(setting);
        self
    }

    /// Control the chosen model, by id. Pair it with `on_change`.
    pub fn value(mut self, id: impl Into<SharedString>) -> Self {
        self.value = Some(id.into());
        self
    }

    /// Set the chosen model at first, when the picker keeps it itself. The
    /// first model is chosen otherwise.
    pub fn default_value(mut self, id: impl Into<SharedString>) -> Self {
        self.default_value = Some(id.into());
        self
    }

    /// Open the menu above the trigger, for a composer at the bottom.
    pub fn above(mut self, above: bool) -> Self {
        self.above = above;
        self
    }

    /// Line the menu up with the end of the trigger, for a trigger on the far
    /// side of its toolbar.
    pub fn align_end(mut self, align_end: bool) -> Self {
        self.align_end = align_end;
        self
    }

    /// Run with the id of a newly chosen model.
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Run with the id of a setting and what changed about it.
    pub fn on_setting_change(
        mut self,
        handler: impl Fn(&SharedString, SettingChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_setting_change = Some(Rc::new(handler));
        self
    }
}

/// Styles the menu surface.
impl Styled for ModelPicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The corner of the menu that meets the trigger.
fn menu_anchor(above: bool, align_end: bool) -> Anchor {
    match (above, align_end) {
        (false, false) => Anchor::TopLeft,
        (false, true) => Anchor::TopRight,
        (true, false) => Anchor::BottomLeft,
        (true, true) => Anchor::BottomRight,
    }
}

/// The row an arrow key moves to. It stops at the ends.
fn step_active(active: Option<usize>, rows: usize, key: &str) -> Option<usize> {
    let last = rows.checked_sub(1)?;
    match (key, active) {
        ("down", None) | ("home", _) => Some(0),
        ("up", None) | ("end", _) => Some(last),
        ("down", Some(ix)) => Some((ix + 1).min(last)),
        ("up", Some(ix)) => Some(ix.saturating_sub(1)),
        _ => None,
    }
}

/// How many steps of a meter are lit.
fn lit_steps(rating: u8) -> usize {
    (rating as usize).min(STEPS)
}

/// The letter that stands for a model without an icon.
fn initial(name: &str) -> String {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}

/// The small square in front of a model's name.
fn tile(model: &ModelOption, cx: &App) -> isonomic_ui::Div {
    let theme = cx.theme();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.))
        .rounded(px(5.))
        .border_1()
        .border_color(theme.border)
        .text_xs()
        .text_color(theme.muted_foreground)
        .map(|this| match model.icon.clone() {
            Some(icon) => this.child(Icon::new(icon).xsmall()),
            None => this.child(initial(&model.name)),
        })
}

struct PickerState {
    value: Option<SharedString>,
    /// The row the pointer or the arrows are on.
    active: Option<usize>,
    /// Each setting's switch and level, in order.
    settings: Vec<(bool, usize)>,
    focus: FocusHandle,
}

/// Everything the open menu draws from.
struct Menu {
    models: Vec<ModelOption>,
    columns: Vec<SharedString>,
    settings: Vec<PickerSetting>,
    switched: Vec<(bool, usize)>,
    chosen: Option<usize>,
    active: Option<usize>,
    state: Entity<PickerState>,
    focus: FocusHandle,
    pick: ChangeHandler,
    set: SetHandler,
}

impl Menu {
    fn key_down(
        self: &Rc<Self>,
        popover: &mut PopoverState,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<PopoverState>,
    ) {
        // Keys pressed on a switch or a level belong to that control.
        if !self.focus.is_focused(window) || event.keystroke.modifiers.modified() {
            return;
        }
        let at = self.active.or(self.chosen);
        match event.keystroke.key.as_str() {
            "enter" | "space" => {
                if let Some(model) = at.and_then(|ix| self.models.get(ix)) {
                    (self.pick)(&model.id, window, cx);
                }
                popover.dismiss(window, cx);
            }
            key => match step_active(at, self.models.len(), key) {
                Some(next) => self.state.update(cx, |state, cx| {
                    state.active = Some(next);
                    cx.notify();
                }),
                None => return,
            },
        }
        cx.stop_propagation();
    }

    fn render(self: &Rc<Self>, cx: &mut Context<PopoverState>) -> AnyElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let radius = theme.radius;

        let lit_row = self.active.or(self.chosen);
        let columns_width = px(COLUMN * self.columns.len() as f32);
        let meter = |rating: u8, is_lit: bool| {
            h_flex()
                .flex_none()
                .w(px(COLUMN))
                .justify_center()
                .gap(px(2.))
                .children((0..STEPS).map(|step| {
                    div().w(px(6.)).h(px(3.)).rounded(px(1.)).bg(
                        match (step < lit_steps(rating), is_lit) {
                            (true, true) => accent,
                            (true, false) => quiet.opacity(0.6),
                            (false, _) => line,
                        },
                    )
                }))
        };

        let rows = self.models.iter().enumerate().map(|(ix, model)| {
            let is_lit = lit_row == Some(ix);
            h_flex()
                .id(("model", ix))
                .gap_2p5()
                .min_h(px(44.))
                .px_2()
                .rounded(radius)
                .cursor_pointer()
                .when(self.active == Some(ix), |this| this.bg(chip))
                .on_hover({
                    let state = self.state.clone();
                    move |hovered, _, cx| {
                        if !*hovered {
                            return;
                        }
                        state.update(cx, |state, cx| {
                            if state.active != Some(ix) {
                                state.active = Some(ix);
                                cx.notify();
                            }
                        })
                    }
                })
                .on_click(cx.listener({
                    let menu = self.clone();
                    move |popover, _, window, cx| {
                        if let Some(model) = menu.models.get(ix) {
                            (menu.pick)(&model.id, window, cx);
                        }
                        popover.dismiss(window, cx);
                    }
                }))
                .child(tile(model, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(div().truncate().font_medium().child(model.name.clone()))
                                .when(self.chosen == Some(ix), |this| {
                                    this.child(
                                        Icon::new(IconName::Check).xsmall().text_color(accent),
                                    )
                                }),
                        )
                        .children(model.tagline.clone().map(|tagline| {
                            div().truncate().text_xs().text_color(quiet).child(tagline)
                        })),
                )
                .when(!self.columns.is_empty(), |this| {
                    if model.ratings.is_empty() {
                        this.child(
                            div()
                                .flex_none()
                                .w(columns_width)
                                .text_center()
                                .text_xs()
                                .text_color(quiet)
                                .child("Not fixed"),
                        )
                    } else {
                        this.child(h_flex().flex_none().children((0..self.columns.len()).map(
                            |column| meter(model.ratings.get(column).copied().unwrap_or(0), is_lit),
                        )))
                    }
                })
        });
        let rows: Vec<_> = rows.collect();

        let settings = self.settings.iter().enumerate().map(|(ix, setting)| {
            let (on, level) = self.switched[ix];
            let set = self.set.clone();
            v_flex()
                .child(
                    h_flex()
                        .gap_2p5()
                        .h(px(32.))
                        .px_2()
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(18.))
                                .children(
                                    setting
                                        .icon
                                        .clone()
                                        .map(|icon| Icon::new(icon).small().text_color(quiet)),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(setting.label.clone()),
                        )
                        .child(
                            Switch::new(("switch", ix))
                                .small()
                                .checked(on)
                                .accessibility_label(setting.label.clone())
                                .on_change(move |on, window, cx| {
                                    set(ix, SettingChange::Switched(*on), window, cx)
                                }),
                        ),
                )
                .when(on && !setting.levels.is_empty(), |this| {
                    this.child(
                        h_flex()
                            .gap_2p5()
                            .h(px(32.))
                            .pl(px(36.))
                            .pr_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(quiet)
                                    .child(setting.levels_label.clone()),
                            )
                            .child(
                                h_flex()
                                    .flex_none()
                                    .gap_0p5()
                                    .p_0p5()
                                    .rounded(radius)
                                    .bg(chip)
                                    .children(setting.levels.iter().enumerate().map(
                                        |(n, name)| {
                                            let set = self.set.clone();
                                            div()
                                                .id(SharedString::from(format!("level-{ix}-{n}")))
                                                .flex()
                                                .items_center()
                                                .h(px(24.))
                                                .px_2()
                                                .rounded(radius)
                                                .cursor_pointer()
                                                .text_xs()
                                                .map(|this| {
                                                    if n == level {
                                                        this.bg(fill).font_medium()
                                                    } else {
                                                        this.text_color(quiet)
                                                    }
                                                })
                                                .on_click(move |_, window, cx| {
                                                    set(ix, SettingChange::Level(n), window, cx)
                                                })
                                                .child(name.clone())
                                        },
                                    )),
                            ),
                    )
                })
        });
        let settings: Vec<_> = settings.collect();

        let note = lit_row.and_then(|ix| self.models.get(ix)?.note.clone());
        let hint = |keys: &'static str| Keystroke::parse(keys).ok().map(Kbd::new);

        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener({
                let menu = self.clone();
                move |popover, event, window, cx| menu.key_down(popover, event, window, cx)
            }))
            .w_full()
            .text_sm()
            .line_height(relative(1.4))
            .text_color(ink)
            .child(
                v_flex()
                    .p_1()
                    .when(!self.columns.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .h(px(24.))
                                .px_2()
                                .text_xs()
                                .text_color(quiet)
                                .child(div().flex_1().child("Model"))
                                .children(self.columns.iter().map(|column| {
                                    div()
                                        .flex_none()
                                        .w(px(COLUMN))
                                        .truncate()
                                        .text_center()
                                        .child(column.clone())
                                })),
                        )
                    })
                    .children(rows),
            )
            .when(!settings.is_empty(), |this| {
                this.child(
                    v_flex()
                        .p_1()
                        .border_t_1()
                        .border_color(line)
                        .children(settings),
                )
            })
            .child(
                h_flex()
                    .gap_1()
                    .h(px(34.))
                    .px_3()
                    .border_t_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
                    .child(div().flex_1().min_w_0().truncate().children(note))
                    .children(hint("up"))
                    .children(hint("down"))
                    .children(hint("enter")),
            )
            .into_any_element()
    }
}

impl RenderOnce for ModelPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| PickerState {
            value: self
                .default_value
                .clone()
                .or_else(|| self.models.first().map(|model| model.id.clone())),
            active: None,
            settings: Vec::new(),
            focus: cx.focus_handle(),
        });
        let (kept_value, active, kept_settings, focus) = state.update(cx, |state, _| {
            // Settings added since the last draw start from their defaults.
            for setting in self.settings.iter().skip(state.settings.len()) {
                state
                    .settings
                    .push((setting.default_on, setting.default_level));
            }
            (
                state.value.clone(),
                state.active,
                state.settings.clone(),
                state.focus.clone(),
            )
        });
        let value = self.value.clone().or(kept_value);
        let chosen = self
            .models
            .iter()
            .position(|model| Some(&model.id) == value.as_ref());
        let switched: Vec<(bool, usize)> = self
            .settings
            .iter()
            .zip(kept_settings)
            .map(|(setting, (on, level))| {
                (
                    setting.on.unwrap_or(on),
                    setting
                        .level
                        .unwrap_or(level)
                        .min(setting.levels.len().saturating_sub(1)),
                )
            })
            .collect();

        let pick: ChangeHandler = {
            let state = state.clone();
            let on_change = self.on_change.clone();
            Rc::new(move |id, window, cx| {
                if value.as_ref() == Some(id) {
                    return;
                }
                state.update(cx, |state, cx| {
                    state.value = Some(id.clone());
                    cx.notify();
                });
                if let Some(handler) = &on_change {
                    handler(id, window, cx);
                }
            })
        };
        let set: SetHandler = {
            let state = state.clone();
            let on_setting_change = self.on_setting_change.clone();
            let ids: Vec<SharedString> = self
                .settings
                .iter()
                .map(|setting| setting.id.clone())
                .collect();
            Rc::new(move |ix, change, window, cx| {
                state.update(cx, |state, cx| {
                    if let Some(kept) = state.settings.get_mut(ix) {
                        match change {
                            SettingChange::Switched(on) => kept.0 = on,
                            SettingChange::Level(level) => kept.1 = level,
                        }
                    }
                    cx.notify();
                });
                if let (Some(handler), Some(id)) = (&on_setting_change, ids.get(ix)) {
                    handler(id, change, window, cx);
                }
            })
        };

        let trigger = Button::new((self.id.clone(), "trigger"))
            .ghost()
            .small()
            .map(|this| match chosen.and_then(|ix| self.models.get(ix)) {
                Some(model) => this
                    .accessibility_label(format!("Model: {}", model.name))
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(tile(model, cx))
                            .child(model.name.clone())
                            .children(self.settings.iter().zip(&switched).filter_map(
                                |(setting, (on, _))| {
                                    let icon = setting.icon.clone().filter(|_| *on)?;
                                    Some(Icon::new(icon).xsmall().text_color(quiet))
                                },
                            ))
                            .child(Icon::new(IconName::ChevronDown).xsmall().text_color(quiet)),
                    ),
                None => this.label("Choose a model"),
            });

        let menu = Rc::new(Menu {
            models: self.models,
            columns: self.columns,
            settings: self.settings,
            switched,
            chosen,
            active,
            state: state.clone(),
            focus: focus.clone(),
            pick,
            set,
        });

        Popover::new((self.id, "menu"))
            .anchor(menu_anchor(self.above, self.align_end))
            .trigger(trigger)
            .on_open_change(move |open, window, cx| {
                if !*open {
                    return;
                }
                // Start on the chosen model, with the keyboard on the list.
                state.update(cx, |state, cx| {
                    state.active = chosen;
                    cx.notify();
                });
                focus.focus(window, cx);
            })
            .content(move |_, _, cx| menu.render(cx))
            .p_0()
            .w(px(320.))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_move_through_the_models_and_stop_at_the_ends() {
        assert_eq!(step_active(None, 4, "down"), Some(0));
        assert_eq!(step_active(None, 4, "up"), Some(3));
        assert_eq!(step_active(Some(1), 4, "down"), Some(2));
        assert_eq!(step_active(Some(3), 4, "down"), Some(3));
        assert_eq!(step_active(Some(0), 4, "up"), Some(0));
        assert_eq!(step_active(Some(2), 4, "home"), Some(0));
        assert_eq!(step_active(Some(0), 4, "end"), Some(3));
        assert_eq!(step_active(Some(0), 4, "left"), None);
        assert_eq!(step_active(None, 0, "down"), None);
    }

    #[test]
    fn a_meter_lights_at_most_all_of_its_steps() {
        assert_eq!(lit_steps(0), 0);
        assert_eq!(lit_steps(3), 3);
        assert_eq!(lit_steps(9), STEPS);
    }

    #[test]
    fn the_tile_shows_the_first_letter() {
        assert_eq!(initial("tally"), "T");
        assert_eq!(initial(" 4-way router"), "4");
        assert_eq!(initial(""), "");
    }

    #[test]
    fn the_menu_meets_the_trigger_at_the_asked_corner() {
        assert!(matches!(menu_anchor(false, false), Anchor::TopLeft));
        assert!(matches!(menu_anchor(false, true), Anchor::TopRight));
        assert!(matches!(menu_anchor(true, false), Anchor::BottomLeft));
        assert!(matches!(menu_anchor(true, true), Anchor::BottomRight));
    }
}
