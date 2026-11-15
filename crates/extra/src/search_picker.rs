//! Search picker: a field that filters a list as you type, highlights the matching letters, picks one or many as chips, and offers to create what isn't there.
//!
//! Matching is forgiving: the typed letters only need to appear in order,
//! so "nwd" finds "Northwind Dental". Up, Down and Enter choose; Backspace
//! in an empty field removes the last chip. When nothing matches exactly,
//! the last row offers to create the typed name.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _,
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    HighlightStyle, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledText, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
};

use crate::common::resting_shadow;

/// Where the typed letters fall in `text`, in order, ignoring case; `None` if they don't all appear.
pub fn fuzzy_positions(text: &str, typed: &str) -> Option<Vec<usize>> {
    let mut positions = Vec::new();
    let mut letters = typed.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).peekable();
    for (at, c) in text.char_indices() {
        let Some(want) = letters.peek() else { break };
        if c.to_lowercase().next() == Some(*want) {
            positions.push(at);
            letters.next();
        }
    }
    letters.peek().is_none().then_some(positions)
}

/// A score for ordering matches: earlier and closer letters score better.
pub fn fuzzy_score(positions: &[usize]) -> usize {
    match (positions.first(), positions.last()) {
        (Some(first), Some(last)) => first * 2 + (last - first),
        _ => 0,
    }
}

/// What the picker reports.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchPickerEvent {
    /// The picked options changed.
    Picked(Vec<SharedString>),
    /// Asked to create a new option with this name.
    Create(SharedString),
}

/// A searchable picker.
pub struct SearchPicker {
    options: Vec<SharedString>,
    picked: Vec<SharedString>,
    multiple: bool,
    field: Entity<InputState>,
    cursor: usize,
    _events: Subscription,
}

impl EventEmitter<SearchPickerEvent> for SearchPicker {}

impl Focusable for SearchPicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl SearchPicker {
    /// Create a picker over these options; `multiple` lets it keep more than one.
    pub fn new(options: Vec<SharedString>, multiple: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder("Type to search"));
        let events = cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            InputEvent::Change => {
                this.cursor = 0;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.choose(window, cx),
            _ => {}
        });
        Self {
            options,
            picked: Vec::new(),
            multiple,
            field,
            cursor: 0,
            _events: events,
        }
    }

    /// Set what's picked.
    pub fn set_picked(&mut self, picked: Vec<SharedString>, cx: &mut Context<Self>) {
        self.picked = picked;
        cx.notify();
    }

    /// Set the options.
    pub fn set_options(&mut self, options: Vec<SharedString>, cx: &mut Context<Self>) {
        self.options = options;
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<(SharedString, Vec<usize>)> {
        let typed = self.field.read(cx).value().to_string();
        let mut found: Vec<_> = self
            .options
            .iter()
            .filter(|o| !self.multiple || !self.picked.contains(o))
            .filter_map(|o| fuzzy_positions(o, &typed).map(|p| (o.clone(), p)))
            .collect();
        found.sort_by_key(|(_, p)| fuzzy_score(p));
        found
    }

    fn can_create(&self, cx: &App) -> Option<SharedString> {
        let typed = self.field.read(cx).value().trim().to_string();
        (!typed.is_empty() && !self.options.iter().any(|o| o.eq_ignore_ascii_case(&typed))).then(|| typed.into())
    }

    fn pick(&mut self, option: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiple {
            if !self.picked.contains(&option) {
                self.picked.push(option);
            }
        } else {
            self.picked = vec![option];
        }
        cx.emit(SearchPickerEvent::Picked(self.picked.clone()));
        self.field.update(cx, |f, cx| f.set_value("", window, cx));
        self.cursor = 0;
        cx.notify();
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let found = self.matches(cx);
        if let Some((option, _)) = found.get(self.cursor) {
            let option = option.clone();
            self.pick(option, window, cx);
        } else if let Some(name) = self.can_create(cx) {
            cx.emit(SearchPickerEvent::Create(name));
            self.field.update(cx, |f, cx| f.set_value("", window, cx));
        }
    }

    fn unpick(&mut self, option: &SharedString, cx: &mut Context<Self>) {
        self.picked.retain(|p| p != option);
        cx.emit(SearchPickerEvent::Picked(self.picked.clone()));
        cx.notify();
    }
}

impl Render for SearchPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let found = self.matches(cx);
        let create = self.can_create(cx);
        let rows = found.len() + usize::from(create.is_some());
        let cursor = self.cursor.min(rows.saturating_sub(1));

        let chips: Vec<_> = if self.multiple {
            self.picked
                .iter()
                .map(|p| {
                    let option = p.clone();
                    h_flex()
                        .gap_0p5()
                        .pl_2()
                        .h(px(22.))
                        .rounded(px(5.))
                        .bg(chip)
                        .text_xs()
                        .child(p.clone())
                        .child(
                            div()
                                .id(ElementId::Name(format!("unpick-{p}").into()))
                                .size(px(18.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(|s| s.bg(line))
                                .aria_label(format!("Remove {p}"))
                                .on_click(cx.listener(move |this, _, _, cx| this.unpick(&option, cx)))
                                .child(Icon::new(IconName::Close).xsmall()),
                        )
                })
                .collect()
        } else {
            Vec::new()
        };

        let list: Vec<_> = found
            .iter()
            .enumerate()
            .map(|(ix, (option, positions))| {
                let text = StyledText::new(option.clone()).with_highlights(
                    positions
                        .iter()
                        .map(|at| {
                            let len = option[*at..].chars().next().map(char::len_utf8).unwrap_or(1);
                            (*at..*at + len, HighlightStyle { color: Some(accent), font_weight: Some(isonomic_ui::FontWeight::SEMIBOLD), ..Default::default() })
                        })
                        .collect::<Vec<_>>(),
                );
                let pick = option.clone();
                let is_picked = !self.multiple && self.picked.contains(option);
                h_flex()
                    .id(ElementId::Name(format!("option-{option}").into()))
                    .gap_2()
                    .px_2()
                    .h(px(30.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(ix == cursor, |this| this.bg(chip))
                    .aria_selected(ix == cursor)
                    .on_click(cx.listener(move |this, _, window, cx| this.pick(pick.clone(), window, cx)))
                    .child(div().flex_1().min_w_0().truncate().child(text))
                    .when(is_picked, |this| this.child(Icon::new(IconName::Check).xsmall().text_color(accent)))
            })
            .collect();

        let create_row = create.map(|name| {
            let at_cursor = cursor == found.len();
            let value = name.clone();
            h_flex()
                .id("option-create")
                .gap_2()
                .px_2()
                .h(px(30.))
                .rounded(px(5.))
                .cursor_pointer()
                .when(at_cursor, |this| this.bg(chip))
                .text_color(accent)
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.emit(SearchPickerEvent::Create(value.clone()));
                    this.field.update(cx, |f, cx| f.set_value("", window, cx));
                }))
                .child(Icon::new(IconName::Plus).xsmall())
                .child(format!("Create \u{201c}{name}\u{201d}"))
        });

        v_flex()
            .w_full()
            .max_w(px(360.))
            .gap_1p5()
            .p_1p5()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| match event.keystroke.key.as_str() {
                "down" if rows > 0 => {
                    this.cursor = (cursor + 1) % rows;
                    cx.notify();
                }
                "up" if rows > 0 => {
                    this.cursor = (cursor + rows - 1) % rows;
                    cx.notify();
                }
                "backspace" if this.multiple && this.field.read(cx).value().is_empty() => {
                    if let Some(last) = this.picked.last().cloned() {
                        this.unpick(&last, cx);
                    }
                }
                _ => {}
            }))
            .when(!chips.is_empty(), |this| this.child(h_flex().flex_wrap().gap_1().px_0p5().children(chips)))
            .child(Input::new(&self.field).small().prefix(Icon::new(IconName::Search).xsmall().text_color(quiet)).cleanable(true))
            .child(
                v_flex()
                    .id("picker-list")
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .children(list)
                    .children(create_row)
                    .when(rows == 0, |this| this.child(div().px_2().py_2().text_xs().text_color(quiet).child("Nothing left to pick."))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_match_in_order_ignoring_case() {
        assert_eq!(fuzzy_positions("Northwind Dental", "nwd"), Some(vec![0, 5, 10]));
        assert_eq!(fuzzy_positions("Northwind Dental", "zx"), None);
        assert_eq!(fuzzy_positions("Kestrel", ""), Some(vec![]));
        assert!(fuzzy_score(&[0, 1]) < fuzzy_score(&[4, 9]));
    }
}
