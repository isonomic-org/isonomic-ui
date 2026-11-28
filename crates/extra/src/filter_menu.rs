// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filter menu: a few groups of options to filter a list, table or board by.
//!
//! What is picked sits on top as removable chips. Below, each group folds open
//! to its options as checkboxes and says how many of them are picked. The whole
//! list is one stop for the Tab key and is worked with the arrow keys.

use std::rc::Rc;

use std::time::Duration;

use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::base::{
    Easing, MotionReveal, Presence, PressableExt as _, Transition, transition,
};
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, look, v_flex,
};
use isonomic_ui::{
    AnyElement, App, Bounds, ElementId, Entity, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, percentage,
    point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, resting_shadow, shell};

/// A group of options that filter by the same thing, such as "Stage".
#[derive(Debug, Clone)]
pub struct FilterGroup {
    id: SharedString,
    label: SharedString,
    options: Vec<(SharedString, SharedString)>,
    chip_prefix: Option<SharedString>,
}

impl FilterGroup {
    /// Create a group with no options.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options: Vec::new(),
            chip_prefix: None,
        }
    }

    /// Add an option after the ones already added.
    pub fn option(mut self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.options.push((id.into(), label.into()));
        self
    }

    /// Put this in front of an option on its chip, for options that do not
    /// explain themselves: "Score: " turns "80 to 90" into "Score: 80 to 90".
    pub fn chip_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.chip_prefix = Some(prefix.into());
        self
    }
}

/// The picked options, in the order they were picked. The menu shows them
/// in group and option order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterSelection(Vec<(SharedString, SharedString)>);

impl FilterSelection {
    /// Create an empty selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an option of a group, by their ids.
    pub fn with(mut self, group: impl Into<SharedString>, option: impl Into<SharedString>) -> Self {
        let (group, option) = (group.into(), option.into());
        if !self.contains(&group, &option) {
            self.0.push((group, option));
        }
        self
    }

    /// Whether an option of a group is picked.
    pub fn contains(&self, group: &str, option: &str) -> bool {
        self.iter().any(|(picked_group, picked)| {
            picked_group.as_ref() == group && picked.as_ref() == option
        })
    }

    /// The picked options of one group.
    pub fn in_group<'a>(&'a self, group: &'a str) -> impl Iterator<Item = &'a SharedString> {
        self.iter()
            .filter(move |(picked_group, _)| picked_group.as_ref() == group)
            .map(|(_, option)| option)
    }

    /// Every picked option with its group, as `(group, option)` ids.
    pub fn iter(&self) -> impl Iterator<Item = &(SharedString, SharedString)> {
        self.0.iter()
    }

    /// How many options are picked.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing is picked.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn toggle(&mut self, group: &SharedString, option: &SharedString) {
        match self
            .0
            .iter()
            .position(|(picked_group, picked)| picked_group == group && picked == option)
        {
            Some(ix) => {
                self.0.remove(ix);
            }
            None => self.0.push((group.clone(), option.clone())),
        }
    }
}

type ChangeHandler = Rc<dyn Fn(&FilterSelection, &mut Window, &mut App)>;
type Dispatch = Rc<dyn Fn(Action, &mut Window, &mut App)>;

/// Grouped filters with chips for what is picked.
#[derive(IntoElement)]
pub struct FilterMenu {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    groups: Vec<FilterGroup>,
    value: Option<FilterSelection>,
    default_value: FilterSelection,
    default_open: Option<Vec<SharedString>>,
    hints: bool,
    on_change: Option<ChangeHandler>,
}

impl FilterMenu {
    /// Create a menu titled "Filters" with no groups.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Filters".into(),
            groups: Vec::new(),
            value: None,
            default_value: FilterSelection::new(),
            default_open: None,
            hints: true,
            on_change: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Add a group after the ones already added.
    pub fn group(mut self, group: FilterGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Control what is picked. Pair it with `on_change`.
    pub fn value(mut self, value: FilterSelection) -> Self {
        self.value = Some(value);
        self
    }

    /// Set what is picked at first, when the menu keeps the selection itself.
    pub fn default_value(mut self, value: FilterSelection) -> Self {
        self.default_value = value;
        self
    }

    /// Set the groups that start open, by id. By default the first one does.
    pub fn default_open(
        mut self,
        groups: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_open = Some(groups.into_iter().map(Into::into).collect());
        self
    }

    /// Whether the keyboard hints show as the last row. On by default.
    pub fn hints(mut self, hints: bool) -> Self {
        self.hints = hints;
        self
    }

    /// Run after every change, with the new selection.
    pub fn on_change(
        mut self,
        handler: impl Fn(&FilterSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for FilterMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A line of the list: a group's header, or one option of an open group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Group(usize),
    Option(usize, usize),
}

/// The lines the arrow keys move through: every header, and the options of the
/// groups that are open.
fn visible_rows(groups: &[FilterGroup], open: &[SharedString]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        rows.push(Row::Group(g));
        if open.contains(&group.id) {
            rows.extend((0..group.options.len()).map(|o| Row::Option(g, o)));
        }
    }
    rows
}

/// Where a key moves the cursor to among `count` rows. It stops at the ends.
fn move_cursor(cursor: usize, count: usize, key: &str) -> Option<usize> {
    let last = count.checked_sub(1)?;
    match key {
        "down" => Some((cursor + 1).min(last)),
        "up" => Some(cursor.saturating_sub(1)),
        "home" => Some(0),
        "end" => Some(last),
        _ => None,
    }
}

/// Where each picked option's chip goes: in group and option order, whatever
/// order they were picked in, as `(group, option)` positions.
fn chip_order(groups: &[FilterGroup], value: &FilterSelection) -> Vec<(usize, usize)> {
    let mut picked: Vec<(usize, usize)> = value
        .iter()
        .filter_map(|(group_id, option_id)| {
            let g = groups.iter().position(|group| &group.id == group_id)?;
            let o = groups[g]
                .options
                .iter()
                .position(|(id, _)| id == option_id)?;
            Some((g, o))
        })
        .collect();
    picked.sort_unstable();
    picked
}

/// How many filters a group has picked, in words. Nothing for none.
fn picked_label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 filter".into()),
        n => Some(format!("{n} filters")),
    }
}

/// Something done to the menu, by pointer or by key.
#[derive(Debug, Clone, Copy)]
enum Action {
    /// Put the keyboard cursor on a row.
    Cursor(usize),
    ToggleOpen(usize),
    SetOpen(usize, bool),
    /// Pick or unpick an option of a group.
    Toggle(usize, usize),
    Clear,
}

#[derive(Default)]
struct MenuState {
    seeded: bool,
    value: FilterSelection,
    /// Ids of the groups that are open.
    open: Vec<SharedString>,
    cursor: usize,
}

/// The reference's ease-out for heights, turns and strokes.
fn ease_out() -> Easing {
    Easing::cubic_bezier(0.22, 1.0, 0.36, 1.0).expect("static curve is valid")
}

/// The plain `ease` curve fills, fades and hovers ride on.
fn ease() -> Easing {
    Easing::cubic_bezier(0.25, 0.1, 0.25, 1.0).expect("static curve is valid")
}

fn over(ms: u64, easing: Easing) -> Transition {
    Transition::new(Duration::from_millis(ms)).easing(easing)
}

/// Where a key hint's cap and words go.
fn key_cap(icon: AssetIcon, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(20.))
        .rounded(px(6.))
        .bg(cx.theme().background)
        .shadow(resting_shadow(cx))
        .text_color(look::ink::icon(cx))
        .child(Icon::new(icon).size(px(12.)))
}

/// The tick in a 20px box, drawn up to `drawn` (0 to 1) of its length.
fn paint_tick(bounds: Bounds<Pixels>, drawn: f32, color: Hsla, window: &mut Window) {
    if drawn <= 0.01 {
        return;
    }
    let unit = bounds.size.width / 20.;
    let at = |x: f32, y: f32| point(bounds.origin.x + unit * x, bounds.origin.y + unit * y);
    let (a, b, c): ((f32, f32), (f32, f32), (f32, f32)) = ((5.5, 10.5), (8.5, 13.5), (14.5, 6.5));
    let first = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let second = ((c.0 - b.0).powi(2) + (c.1 - b.1).powi(2)).sqrt();
    let length = (first + second) * drawn.clamp(0., 1.);
    let mut line = PathBuilder::stroke(unit * 1.75);
    line.move_to(at(a.0, a.1));
    if length <= first {
        let t = length / first;
        line.line_to(at(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
    } else {
        let t = (length - first) / second;
        line.line_to(at(b.0, b.1));
        line.line_to(at(b.0 + (c.0 - b.0) * t, b.1 + (c.1 - b.1) * t));
    }
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }
}

/// Whether the pointer is over `id`, kept between frames.
fn hover_state(id: ElementId, window: &mut Window, cx: &mut App) -> Entity<bool> {
    window.use_keyed_state(id, cx, |_, _| false)
}

fn track_hover(state: Entity<bool>) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    move |hovered, window, cx| {
        let hovered = *hovered;
        state.update(cx, |state, cx| {
            if *state != hovered {
                *state = hovered;
                cx.notify();
            }
        });
        window.refresh();
    }
}

impl RenderOnce for FilterMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = look::ink::strong(cx);
        let quiet = cx.theme().muted_foreground;
        let wash = look::ink::wash(cx);
        let box_rest = look::ink::wash_strong(cx);
        let on_ink = cx.theme().background;
        let radius = cx.theme().radius;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| MenuState::default());
        let (kept, open, cursor) = state.update(cx, |state, _| {
            if !state.seeded {
                state.seeded = true;
                state.value = self.default_value.clone();
                state.open = match &self.default_open {
                    Some(open) => open.clone(),
                    None => self
                        .groups
                        .first()
                        .map(|group| group.id.clone())
                        .into_iter()
                        .collect(),
                };
            }
            (state.value.clone(), state.open.clone(), state.cursor)
        });
        let value = self.value.clone().unwrap_or(kept);
        let list_name = format!("{} options", self.title);
        let rows = Rc::new(visible_rows(&self.groups, &open));
        let cursor = cursor.min(rows.len().saturating_sub(1));

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);
        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());

        // The ids alone, for the handlers to look things up by position.
        let shape: Vec<(SharedString, Vec<SharedString>)> = self
            .groups
            .iter()
            .map(|group| {
                let options = group.options.iter().map(|(id, _)| id.clone()).collect();
                (group.id.clone(), options)
            })
            .collect();
        let act: Dispatch = {
            let state = state.clone();
            let controlled = self.value.clone();
            let on_change = self.on_change.clone();
            Rc::new(move |action, window, cx| {
                let changed = state.update(cx, |state, cx| {
                    let before = controlled.clone().unwrap_or_else(|| state.value.clone());
                    let mut value = before.clone();
                    match action {
                        Action::Cursor(ix) => state.cursor = ix,
                        Action::ToggleOpen(g) | Action::SetOpen(g, _) => {
                            let (group, _) = shape.get(g)?;
                            let is_open = state.open.contains(group);
                            let wanted = match action {
                                Action::SetOpen(_, open) => open,
                                _ => !is_open,
                            };
                            if wanted && !is_open {
                                state.open.push(group.clone());
                            } else if !wanted {
                                state.open.retain(|open| open != group);
                            }
                        }
                        Action::Toggle(g, o) => {
                            let option = shape.get(g).and_then(|(group, options)| {
                                options.get(o).map(|option| (group, option))
                            });
                            if let Some((group, option)) = option {
                                value.toggle(group, option);
                            }
                        }
                        Action::Clear => value = FilterSelection::new(),
                    }
                    cx.notify();
                    (value != before).then(|| {
                        state.value = value.clone();
                        value
                    })
                });
                if let (Some(value), Some(on_change)) = (changed, &on_change) {
                    on_change(&value, window, cx);
                }
            })
        };

        let picked = chip_order(&self.groups, &value);
        let chips: Vec<_> = picked
            .into_iter()
            .map(|(g, o)| {
                let group = &self.groups[g];
                let label: SharedString = match &group.chip_prefix {
                    Some(prefix) => format!("{prefix}{}", group.options[o].1).into(),
                    None => group.options[o].1.clone(),
                };
                let enter = Presence::new(child_id(format!("chip-{g}-{o}")), true)
                    .transition(over(260, Easing::Linear))
                    .sample(window, cx);
                h_flex()
                    .flex_none()
                    .gap_0p5()
                    .h(px(26.))
                    .pl_2()
                    .pr_0p5()
                    .rounded(radius)
                    .bg(wash)
                    .opacity(enter.progress)
                    .child(label.clone())
                    .child(
                        Button::new(child_id(format!("remove-{g}-{o}")))
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(IconName::Close).text_color(quiet))
                            .accessibility_label(format!("Remove filter {label}"))
                            .on_click({
                                let act = act.clone();
                                move |_, window, cx| act(Action::Toggle(g, o), window, cx)
                            }),
                    )
            })
            .collect();
        let chip_row = Presence::new(child_id("chips".into()), !chips.is_empty())
            .transition(over(320, ease_out()))
            .sample(window, cx);
        let clear = Presence::new(child_id("clear-shown".into()), !value.is_empty())
            .transition(over(200, ease()))
            .sample(window, cx);

        // One line of the list: a group's head or an option, with its hover
        // fading in and the keyboard cursor drawn on it.
        let line =
            |key: String, ix: Option<usize>, action: Action, window: &mut Window, cx: &mut App| {
                let id = child_id(key);
                let hovered = hover_state(
                    ElementId::NamedChild(id.clone().into(), "hover".into()),
                    window,
                    cx,
                );
                let lit = *hovered.read(cx) || (Some(cursor) == ix && is_focused);
                let fade = transition(
                    ElementId::NamedChild(id.clone().into(), "hover-fade".into()),
                    if lit { 1.0_f32 } else { 0. },
                    over(160, ease()),
                    window,
                    cx,
                );
                h_flex()
                    .id(id)
                    .gap_2()
                    .h(px(35.))
                    .pr_3()
                    .rounded(radius)
                    .bg(look::mix(wash.opacity(0.), wash, fade))
                    .pressable(true)
                    .on_hover(track_hover(hovered))
                    .when(ix.is_some() && ix == Some(cursor) && is_focused, |this| {
                        this.focus_ring_style(window, cx)
                    })
                    .on_click({
                        let act = act.clone();
                        let focus = focus.clone();
                        move |_, window, cx| {
                            focus.focus(window, cx);
                            if let Some(ix) = ix {
                                act(Action::Cursor(ix), window, cx);
                            }
                            act(action, window, cx);
                        }
                    })
            };

        let mut list_children: Vec<AnyElement> = Vec::new();
        for (g, group) in self.groups.iter().enumerate() {
            let is_open = open.contains(&group.id);
            let head_ix = rows.iter().position(|row| *row == Row::Group(g));
            let turn = transition(
                child_id(format!("chevron-{g}")),
                if is_open { 0.25_f32 } else { 0. },
                over(260, ease_out()),
                window,
                cx,
            );
            let count = value.in_group(&group.id).count();
            let picked = picked_label(count);
            list_children.push(
                line(
                    format!("group-{g}"),
                    head_ix,
                    Action::ToggleOpen(g),
                    window,
                    cx,
                )
                .pl_2()
                .role(Role::ListBoxOption)
                .aria_label(match &picked {
                    Some(picked) => format!(
                        "{}, {} options, {picked} picked",
                        group.label,
                        group.options.len()
                    ),
                    None => format!("{}, {} options", group.label, group.options.len()),
                })
                .aria_expanded(is_open)
                .child(
                    Icon::new(IconName::ChevronRight)
                        .xsmall()
                        .flex_none()
                        .text_color(quiet)
                        .rotate(percentage(turn)),
                )
                .child(div().font_medium().child(group.label.clone()))
                .child(
                    div()
                        .flex_1()
                        .text_color(quiet)
                        .child(group.options.len().to_string()),
                )
                .children(picked.map(|picked| div().text_color(quiet).child(picked)))
                .into_any_element(),
            );

            let height = transition(
                child_id(format!("panel-{g}")),
                if is_open { 1.0_f32 } else { 0. },
                over(320, ease_out()),
                window,
                cx,
            );
            let shown = transition(
                child_id(format!("panel-fade-{g}")),
                if is_open { 1.0_f32 } else { 0. },
                over(220, ease()),
                window,
                cx,
            );
            if !is_open && height == 0. {
                continue;
            }
            let mut options: Vec<AnyElement> = Vec::new();
            for (o, (option_id, label)) in group.options.iter().enumerate() {
                let is_picked = value.contains(&group.id, option_id);
                let ix = rows.iter().position(|row| *row == Row::Option(g, o));
                let filled = transition(
                    child_id(format!("box-{g}-{o}")),
                    if is_picked { 1.0_f32 } else { 0. },
                    over(180, ease()),
                    window,
                    cx,
                );
                let drawn = transition(
                    child_id(format!("tick-{g}-{o}")),
                    if is_picked { 1.0_f32 } else { 0. },
                    over(260, ease_out()),
                    window,
                    cx,
                );
                options.push(
                    line(
                        format!("option-{g}-{o}"),
                        ix,
                        Action::Toggle(g, o),
                        window,
                        cx,
                    )
                    .pl(px(30.))
                    .role(Role::ListBoxOption)
                    .aria_label(format!("{}: {label}", group.label))
                    .aria_selected(is_picked)
                    .child(
                        div()
                            .flex_none()
                            .size(px(20.))
                            .rounded(px(6.))
                            .bg(look::mix(box_rest, ink, filled))
                            .child(
                                canvas(
                                    |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        paint_tick(bounds, drawn, on_ink, window)
                                    },
                                )
                                .size_full(),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(15.))
                            .line_height(px(20.))
                            .child(label.clone()),
                    )
                    .into_any_element(),
                );
            }
            list_children.push(
                MotionReveal::new(
                    child_id(format!("reveal-{g}")),
                    height,
                    v_flex().opacity(shown).children(options).into_any_element(),
                )
                .into_any_element(),
            );
        }

        let hints = self.hints.then(|| {
            h_flex()
                .justify_between()
                .px_3()
                .pt_2()
                .pb_1()
                .text_caption()
                .text_color(quiet)
                .child(
                    h_flex()
                        .gap_1()
                        .child(key_cap(AssetIcon::ArrowDown, cx))
                        .child(key_cap(AssetIcon::ArrowUp, cx))
                        .child(div().pl_1().child("To navigate")),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(key_cap(AssetIcon::Return, cx))
                        .child(div().pl_1().child("To select")),
                )
        });

        shell(cx)
            .id(self.id.clone())
            .max_w(px(432.))
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .child(
                panel(cx)
                    .v_flex()
                    .child(
                        v_flex()
                            .gap_2()
                            .px_4()
                            .pt_3()
                            .pb_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .min_h(px(26.))
                                    .child(div().text_base().font_medium().child(self.title))
                                    .when(clear.should_render(), |this| {
                                        this.child(
                                            Button::new(child_id("clear".into()))
                                                .ghost()
                                                .xsmall()
                                                .label("Clear all")
                                                .text_base()
                                                .font_normal()
                                                .text_color(quiet)
                                                .opacity(clear.progress)
                                                .when(!value.is_empty(), |this| {
                                                    this.on_click({
                                                        let act = act.clone();
                                                        move |_, window, cx| {
                                                            act(Action::Clear, window, cx)
                                                        }
                                                    })
                                                }),
                                        )
                                    }),
                            )
                            .when(chip_row.should_render(), |this| {
                                this.child(MotionReveal::new(
                                    child_id("chip-row".into()),
                                    chip_row.progress,
                                    h_flex()
                                        .flex_wrap()
                                        .gap_1()
                                        .children(chips)
                                        .into_any_element(),
                                ))
                            }),
                    )
                    .child(
                        v_flex()
                            .id(child_id("list".into()))
                            .track_focus(&focus)
                            .role(Role::ListBox)
                            .aria_label(list_name)
                            .on_key_down({
                                let act = act.clone();
                                move |event, window, cx| {
                                    if event.keystroke.modifiers.modified() {
                                        return;
                                    }
                                    let Some(row) = rows.get(cursor).copied() else {
                                        return;
                                    };
                                    let key = event.keystroke.key.as_str();
                                    match (key, row) {
                                        ("enter" | "space", Row::Group(g)) => {
                                            act(Action::ToggleOpen(g), window, cx)
                                        }
                                        ("enter" | "space", Row::Option(g, o)) => {
                                            act(Action::Toggle(g, o), window, cx)
                                        }
                                        ("right", Row::Group(g)) => {
                                            act(Action::SetOpen(g, true), window, cx)
                                        }
                                        ("left", Row::Group(g)) => {
                                            act(Action::SetOpen(g, false), window, cx)
                                        }
                                        ("left", Row::Option(g, _)) => {
                                            // Fold the group and go back to its header.
                                            let header =
                                                rows.iter().position(|row| *row == Row::Group(g));
                                            act(Action::SetOpen(g, false), window, cx);
                                            if let Some(header) = header {
                                                act(Action::Cursor(header), window, cx);
                                            }
                                        }
                                        _ => match move_cursor(cursor, rows.len(), key) {
                                            Some(next) => act(Action::Cursor(next), window, cx),
                                            None => return,
                                        },
                                    }
                                    cx.stop_propagation();
                                }
                            })
                            .p_1()
                            .children(list_children),
                    ),
            )
            .children(hints)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<FilterGroup> {
        vec![
            FilterGroup::new("stage", "Stage")
                .option("new", "New")
                .option("won", "Won"),
            FilterGroup::new("region", "Region").option("north", "North"),
        ]
    }

    #[test]
    fn a_selection_keeps_the_order_things_were_picked_in() {
        let mut picked = FilterSelection::new()
            .with("region", "north")
            .with("stage", "won")
            .with("stage", "won");
        assert_eq!(picked.len(), 2);
        assert!(picked.contains("stage", "won"));
        assert!(!picked.contains("stage", "new"));
        assert_eq!(picked.in_group("stage").count(), 1);

        picked.toggle(&"stage".into(), &"new".into());
        picked.toggle(&"region".into(), &"north".into());
        let order: Vec<_> = picked.iter().map(|(_, option)| option.as_ref()).collect();
        assert_eq!(order, ["won", "new"]);
    }

    #[test]
    fn only_open_groups_show_their_options() {
        let open = ["region".into()];
        assert_eq!(
            visible_rows(&groups(), &open),
            [Row::Group(0), Row::Group(1), Row::Option(1, 0)]
        );
        assert_eq!(visible_rows(&groups(), &[]), [Row::Group(0), Row::Group(1)]);
    }

    #[test]
    fn the_cursor_stops_at_the_ends() {
        assert_eq!(move_cursor(0, 3, "down"), Some(1));
        assert_eq!(move_cursor(2, 3, "down"), Some(2));
        assert_eq!(move_cursor(0, 3, "up"), Some(0));
        assert_eq!(move_cursor(1, 3, "end"), Some(2));
        assert_eq!(move_cursor(2, 3, "home"), Some(0));
        assert_eq!(move_cursor(0, 3, "tab"), None);
        assert_eq!(move_cursor(0, 0, "down"), None);
    }

    #[test]
    fn chips_follow_group_and_option_order() {
        let picked = FilterSelection::new()
            .with("region", "north")
            .with("stage", "won")
            .with("stage", "new")
            .with("stage", "gone");
        assert_eq!(chip_order(&groups(), &picked), [(0, 0), (0, 1), (1, 0)]);
    }

    #[test]
    fn a_group_says_how_many_filters_it_has() {
        assert_eq!(picked_label(0), None);
        assert_eq!(picked_label(1).as_deref(), Some("1 filter"));
        assert_eq!(picked_label(3).as_deref(), Some("3 filters"));
    }
}
