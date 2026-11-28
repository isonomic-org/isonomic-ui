// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filter menu: a few groups of options to filter a list, table or board by.
//!
//! What is picked sits on top as removable chips. Below, each group folds open
//! to its options as checkboxes and says how many of them are picked. The whole
//! list is one stop for the Tab key and is worked with the arrow keys.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    kbd::Kbd,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, Keystroke, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

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

/// The picked options, in the order they were picked.
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

impl RenderOnce for FilterMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let on_accent = theme.info_foreground;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

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

        let chips = value.iter().filter_map(|(group_id, option_id)| {
            let g = self.groups.iter().position(|group| &group.id == group_id)?;
            let group = &self.groups[g];
            let o = group.options.iter().position(|(id, _)| id == option_id)?;
            let label: SharedString = match &group.chip_prefix {
                Some(prefix) => format!("{prefix}{}", group.options[o].1).into(),
                None => group.options[o].1.clone(),
            };
            Some(
                h_flex()
                    .flex_none()
                    .gap_0p5()
                    .h(px(26.))
                    .pl_2()
                    .pr_0p5()
                    .rounded(radius)
                    .bg(chip)
                    .font_medium()
                    .child(label.clone())
                    .child(
                        Button::new(child_id(format!("remove-{g}-{o}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .accessibility_label(format!("Remove filter {label}"))
                            .on_click({
                                let act = act.clone();
                                move |_, window, cx| act(Action::Toggle(g, o), window, cx)
                            }),
                    ),
            )
        });
        let chips: Vec<_> = chips.collect();

        let row_elements = rows.iter().enumerate().map(|(ix, row)| {
            let (key, action) = match *row {
                Row::Group(g) => (format!("group-{g}"), Action::ToggleOpen(g)),
                Row::Option(g, o) => (format!("option-{g}-{o}"), Action::Toggle(g, o)),
            };
            let base = h_flex()
                .id(child_id(key))
                .gap_2()
                .h(px(34.))
                .pr_3()
                .rounded(radius)
                .cursor_pointer()
                .hover(|this| this.bg(chip))
                .when(ix == cursor && is_focused, |this| {
                    this.bg(chip).focus_ring_style(window, cx)
                })
                .on_click({
                    let act = act.clone();
                    let focus = focus.clone();
                    move |_, window, cx| {
                        focus.focus(window, cx);
                        act(Action::Cursor(ix), window, cx);
                        act(action, window, cx);
                    }
                });

            match *row {
                Row::Group(g) => {
                    let group = &self.groups[g];
                    let picked = picked_label(value.in_group(&group.id).count());
                    base.pl_2()
                        .role(Role::ListBoxOption)
                        .aria_label(match &picked {
                            Some(picked) => format!(
                                "{}, {} options, {picked} picked",
                                group.label,
                                group.options.len()
                            ),
                            None => format!("{}, {} options", group.label, group.options.len()),
                        })
                        .aria_expanded(open.contains(&group.id))
                        .child(
                            Icon::new(if open.contains(&group.id) {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .xsmall()
                            .flex_none()
                            .text_color(quiet),
                        )
                        .child(div().font_medium().child(group.label.clone()))
                        .child(
                            div()
                                .flex_1()
                                .text_color(quiet)
                                .child(group.options.len().to_string()),
                        )
                        .children(picked.map(|picked| div().text_color(quiet).child(picked)))
                }
                Row::Option(g, o) => {
                    let group = &self.groups[g];
                    let (option_id, label) = &group.options[o];
                    let is_picked = value.contains(&group.id, option_id);
                    base.pl(px(30.))
                        .role(Role::ListBoxOption)
                        .aria_label(format!("{}: {label}", group.label))
                        .aria_selected(is_picked)
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size_4()
                                .rounded(px(4.))
                                .border_1()
                                .map(|this| {
                                    if is_picked {
                                        this.border_color(accent)
                                            .bg(accent)
                                            .text_color(on_accent)
                                            .child(Icon::new(IconName::Check).size_3())
                                    } else {
                                        this.border_color(line)
                                    }
                                }),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                }
            }
        });
        let row_elements: Vec<_> = row_elements.collect();

        let key = |name: &'static str| Keystroke::parse(name).ok().map(Kbd::new);
        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(400.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_2()
                    .px_4()
                    .pt_3()
                    .pb_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .min_h(px(26.))
                            .child(div().text_base().font_medium().child(self.title))
                            .when(!value.is_empty(), |this| {
                                this.child(
                                    Button::new(child_id("clear".into()))
                                        .ghost()
                                        .xsmall()
                                        .label("Clear all")
                                        .on_click({
                                            let act = act.clone();
                                            move |_, window, cx| act(Action::Clear, window, cx)
                                        }),
                                )
                            }),
                    )
                    .when(!chips.is_empty(), |this| {
                        this.child(h_flex().flex_wrap().gap_1().children(chips))
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
                        let rows = rows.clone();
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
                                    let header = rows.iter().position(|row| *row == Row::Group(g));
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
                    .border_t_1()
                    .border_color(line)
                    .children(row_elements),
            )
            .when(self.hints, |this| {
                this.child(
                    h_flex()
                        .justify_between()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(line)
                        .text_caption()
                        .text_color(quiet)
                        .child(
                            h_flex()
                                .gap_1()
                                .children(key("up"))
                                .children(key("down"))
                                .child("to navigate"),
                        )
                        .child(h_flex().gap_1().children(key("enter")).child("to select")),
                )
            })
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
    fn a_group_says_how_many_filters_it_has() {
        assert_eq!(picked_label(0), None);
        assert_eq!(picked_label(1).as_deref(), Some("1 filter"));
        assert_eq!(picked_label(3).as_deref(), Some("3 filters"));
    }
}
