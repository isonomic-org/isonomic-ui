// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tag input: several short values in one field, as chips.
//!
//! Enter, a comma or Tab turns what was typed into a chip, and a pasted list
//! becomes many chips at once. A value that fails the check stays in the field
//! with a message; a duplicate flashes the chip it repeats. Backspace on an
//! empty field marks the last chip and then removes it, and the arrow keys walk
//! the chips.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};

/// How long a repeated chip stays lit.
const FLASH: Duration = Duration::from_millis(700);

type Validate = Rc<dyn Fn(&str) -> Result<(), SharedString>>;
type Normalize = Rc<dyn Fn(&str) -> String>;

/// What the field tells its owner.
#[derive(Debug, Clone)]
pub enum TagInputEvent {
    /// A chip was added or removed. Carries all chips, in order.
    Change(Vec<SharedString>),
}

/// A text field that collects several values as chips.
pub struct TagInput {
    tags: Vec<SharedString>,
    label: Option<SharedString>,
    hint: Option<SharedString>,
    error: Option<SharedString>,
    optional: bool,
    split_on_space: bool,
    max: Option<usize>,
    avatar: bool,
    disabled: bool,
    validate: Option<Validate>,
    normalize: Option<Normalize>,
    /// Why the value in the field was not taken.
    message: Option<SharedString>,
    /// The chip the keyboard is on.
    marked: Option<usize>,
    /// The chip a duplicate just pointed at.
    flashing: Option<usize>,
    /// The text the field was left with after values were taken out of it.
    held: String,
    input: Entity<InputState>,
    _input_events: Subscription,
    _flash: Option<Task<()>>,
}

impl EventEmitter<TagInputEvent> for TagInput {}

impl TagInput {
    /// Create an empty field.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Add…"));
        let input_events = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
            let text = input.read(cx).value().to_string();
            match event {
                InputEvent::PressEnter { .. } => this.take(&text, true, window, cx),
                InputEvent::Change => this.take(&text, false, window, cx),
                _ => {}
            }
        });

        Self {
            tags: Vec::new(),
            label: None,
            hint: None,
            error: None,
            optional: false,
            split_on_space: false,
            max: None,
            avatar: false,
            disabled: false,
            validate: None,
            normalize: None,
            message: None,
            marked: None,
            flashing: None,
            held: String::new(),
            input,
            _input_events: input_events,
            _flash: None,
        }
    }

    /// Start with these chips.
    pub fn with_tags(
        mut self,
        tags: impl IntoIterator<Item = impl Into<SharedString>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self.sync_placeholder(window, cx);
        self
    }

    /// Show a label above the field.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Show a quiet line under the field. A message takes its place while
    /// there is one.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Say "Optional" next to the label.
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// Also split on spaces. Right for addresses and single words, wrong for
    /// values that contain spaces.
    pub fn split_on_space(mut self, split: bool) -> Self {
        self.split_on_space = split;
        self
    }

    /// Take at most this many chips, and show the count under the field.
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    /// Put a small circle with the first letter on every chip, for people.
    pub fn avatar(mut self, avatar: bool) -> Self {
        self.avatar = avatar;
        self
    }

    /// Dim the field and ignore input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Check a value before it becomes a chip. An error keeps the value in the
    /// field and shows its message under it.
    pub fn validate(mut self, check: impl Fn(&str) -> Result<(), SharedString> + 'static) -> Self {
        self.validate = Some(Rc::new(check));
        self
    }

    /// Clean a value before it is checked, such as lowercasing an address.
    /// Values are always trimmed first.
    pub fn normalize(mut self, clean: impl Fn(&str) -> String + 'static) -> Self {
        self.normalize = Some(Rc::new(clean));
        self
    }

    /// The chips, in order.
    pub fn tags(&self) -> &[SharedString] {
        &self.tags
    }

    /// Replace the chips from outside.
    pub fn set_tags(
        &mut self,
        tags: impl IntoIterator<Item = impl Into<SharedString>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tags = tags.into_iter().map(Into::into).collect();
        self.marked = None;
        self.sync_placeholder(window, cx);
        cx.notify();
    }

    /// Mark the field invalid from outside with a message, or clear it.
    pub fn set_error(&mut self, error: Option<SharedString>, cx: &mut Context<Self>) {
        self.error = error;
        cx.notify();
    }

    fn sync_placeholder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = if self.tags.is_empty() {
            "Add…"
        } else {
            "Add more…"
        };
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
    }

    fn set_text(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.held = text.clone();
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
    }

    /// Take finished values out of the field's text and turn them into chips.
    /// With `all`, what is still being typed counts as finished too.
    fn take(&mut self, text: &str, all: bool, window: &mut Window, cx: &mut Context<Self>) {
        // The field settling on the text we just gave it is not typing.
        if !all && text == self.held {
            return;
        }
        let (finished, rest) = split(text, self.split_on_space);
        self.marked = None;
        if finished.is_empty() && !all {
            self.held.clear();
            if self.message.take().is_some() {
                cx.notify();
            }
            return;
        }

        let mut kept: Vec<String> = Vec::new();
        let mut message = None;
        let mut repeated = None;
        let mut changed = false;
        for value in finished.into_iter().chain(all.then_some(rest)) {
            let value = match &self.normalize {
                Some(normalize) => normalize(value.trim()),
                None => value.trim().to_string(),
            };
            if value.is_empty() {
                continue;
            }
            match admit(&self.tags, &value, self.max, self.validate.as_ref()) {
                Admission::Added => {
                    self.tags.push(value.into());
                    changed = true;
                }
                Admission::Repeats(ix) => repeated = Some(ix),
                Admission::Refused(why) => {
                    message.get_or_insert(why);
                    kept.push(value);
                }
            }
        }
        if !all && !rest.is_empty() {
            kept.push(rest.to_string());
        }

        // What was refused, and what is still being typed, stays in the field.
        let left = kept.join(", ");
        if left != text {
            self.set_text(left, window, cx);
        }
        self.message = message;
        if let Some(ix) = repeated {
            self.flashing = Some(ix);
            self._flash = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(FLASH).await;
                this.update(cx, |this, cx| {
                    this.flashing = None;
                    cx.notify();
                })
                .ok();
            }));
        }
        if changed {
            self.sync_placeholder(window, cx);
            cx.emit(TagInputEvent::Change(self.tags.clone()));
        }
        cx.notify();
    }

    fn remove(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tags.len() {
            return;
        }
        self.tags.remove(ix);
        self.marked = None;
        self.flashing = None;
        self.sync_placeholder(window, cx);
        cx.emit(TagInputEvent::Change(self.tags.clone()));
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || event.keystroke.modifiers.modified() {
            return;
        }
        let key = event.keystroke.key.as_str();
        let text = self.input.read(cx).value().to_string();
        match key {
            // Tab adds what was typed and then moves on as usual.
            "tab" => {
                if !text.trim().is_empty() {
                    self.take(&text, true, window, cx);
                }
                return;
            }
            "escape" if !text.is_empty() || self.marked.is_some() => {
                self.set_text(String::new(), window, cx);
                self.message = None;
                self.marked = None;
            }
            // The keys below work on the chips, and only from an empty field.
            _ if !text.is_empty() => return,
            "backspace" | "delete" => match self.marked {
                Some(ix) => self.remove(ix, window, cx),
                None if key == "backspace" && !self.tags.is_empty() => {
                    self.marked = Some(self.tags.len() - 1);
                }
                None => return,
            },
            "left" | "right" => {
                let next = walk(self.marked, self.tags.len(), key);
                if next == self.marked {
                    return;
                }
                self.marked = next;
            }
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }
}

/// Split typed or pasted text into the values that are finished and what is
/// still being typed. A comma, a tab or a line break finishes a value; a space
/// does too with `on_space`. Text without any of them is all still being typed.
fn split(text: &str, on_space: bool) -> (Vec<&str>, &str) {
    let ends_value = |ch: char| matches!(ch, ',' | '\n' | '\r' | '\t') || (on_space && ch == ' ');
    let Some(last) = text.rfind(ends_value) else {
        return (Vec::new(), text);
    };
    let finished = text[..last]
        .split(ends_value)
        .filter(|value| !value.trim().is_empty())
        .collect();
    // Separators are one byte each, so the rest starts right after the last one.
    (finished, &text[last + 1..])
}

/// What happens to a value that asks to become a chip.
#[derive(Debug, PartialEq, Eq)]
enum Admission {
    Added,
    /// It repeats the chip at this position.
    Repeats(usize),
    /// It was not taken, for this reason.
    Refused(SharedString),
}

fn admit(
    tags: &[SharedString],
    value: &str,
    max: Option<usize>,
    validate: Option<&Validate>,
) -> Admission {
    if let Some(ix) = tags.iter().position(|tag| tag.as_ref() == value) {
        return Admission::Repeats(ix);
    }
    if let Some(max) = max.filter(|max| tags.len() >= *max) {
        return Admission::Refused(format!("No more than {max} fit here.").into());
    }
    match validate.map(|validate| validate(value)) {
        Some(Err(why)) => Admission::Refused(why),
        _ => Admission::Added,
    }
}

/// The chip an arrow key marks next, among `count` chips. `None` leaves the
/// chips and returns to the field.
fn walk(marked: Option<usize>, count: usize, key: &str) -> Option<usize> {
    let last = count.checked_sub(1)?;
    match (key, marked) {
        ("left", None) => Some(last),
        ("left", Some(ix)) => Some(ix.saturating_sub(1)),
        ("right", Some(ix)) if ix < last => Some(ix + 1),
        _ => None,
    }
}

/// The letter on a chip's small circle.
fn initial(value: &str) -> String {
    value
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}

impl Render for TagInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let danger = theme.danger;
        let radius = theme.radius;

        let is_focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let trouble = self.message.clone().or_else(|| self.error.clone());
        let edge = if trouble.is_some() {
            danger
        } else if is_focused {
            accent
        } else {
            line
        };
        let disabled = self.disabled;

        let chips = self.tags.iter().enumerate().map(|(ix, tag)| {
            h_flex()
                .flex_none()
                .max_w_full()
                .gap_1()
                .h(px(24.))
                .pl_1p5()
                .pr_0p5()
                .rounded(px(6.))
                .bg(if self.flashing == Some(ix) {
                    accent.opacity(0.25)
                } else {
                    chip
                })
                .when(self.marked == Some(ix), |this| {
                    this.focus_ring_style(window, cx)
                })
                .when(self.avatar, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size_4()
                            .rounded_full_style(cx)
                            .bg(fill)
                            .text_size(px(10.))
                            .text_color(quiet)
                            .child(initial(tag)),
                    )
                })
                .child(
                    div()
                        .min_w_0()
                        .max_w(px(220.))
                        .truncate()
                        .child(tag.clone()),
                )
                .child(
                    Button::new(("remove", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .disabled(disabled)
                        .accessibility_label(format!("Remove {tag}"))
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.remove(ix, window, cx)),
                        ),
                )
        });
        let chips: Vec<_> = chips.collect();

        let below = trouble
            .map(|text| (text, danger))
            .or_else(|| self.hint.clone().map(|text| (text, quiet)));
        let counter = self.max.map(|max| format!("{}/{max}", self.tags.len()));

        v_flex()
            .w_full()
            .gap_1p5()
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .when(disabled, |this| this.opacity(0.6))
            .on_key_down(cx.listener(Self::key_down))
            .when_some(self.label.clone(), |this, label| {
                this.child(
                    h_flex()
                        .gap_2()
                        .child(div().font_medium().child(label))
                        .when(self.optional, |this| {
                            this.child(div().text_caption().text_color(quiet).child("Optional"))
                        }),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .min_h(px(36.))
                    .p_1()
                    .rounded(radius)
                    .border_1()
                    .border_color(edge)
                    .bg(fill)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            // A press anywhere in the field lands in the text.
                            this.input.read(cx).focus_handle(cx).focus(window, cx)
                        }),
                    )
                    .children(chips)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(120.))
                            .child(Input::new(&self.input).appearance(false).disabled(disabled)),
                    ),
            )
            .when(below.is_some() || counter.is_some(), |this| {
                this.child(
                    h_flex()
                        .items_start()
                        .gap_3()
                        .text_caption()
                        .child(div().flex_1().min_w_0().children(
                            below.map(|(text, color)| div().text_color(color).child(text)),
                        ))
                        .children(
                            counter
                                .map(|counter| div().flex_none().text_color(quiet).child(counter)),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_without_a_separator_finishes_nothing() {
        assert_eq!(split("quarter", false), (vec![], "quarter"));
        assert_eq!(split("year end", false), (vec![], "year end"));
    }

    #[test]
    fn a_separator_finishes_what_came_before_it() {
        assert_eq!(split("tax,", false), (vec!["tax"], ""));
        assert_eq!(split("tax, payr", false), (vec!["tax"], " payr"));
        assert_eq!(
            split("tax,\npayroll\r\nyear end,", false),
            (vec!["tax", "payroll", "year end"], "")
        );
        // Empty values between separators are dropped.
        assert_eq!(split(",, ,vat,", false), (vec!["vat"], ""));
    }

    #[test]
    fn spaces_split_only_when_asked() {
        assert_eq!(
            split("a@x.example b@x.example ", true),
            (vec!["a@x.example", "b@x.example"], "")
        );
        assert_eq!(split("a@x.example b@x", true), (vec!["a@x.example"], "b@x"));
    }

    #[test]
    fn a_value_is_added_repeats_a_chip_or_is_refused() {
        let tags: Vec<SharedString> = vec!["tax".into(), "payroll".into()];
        let no_dots: Validate = Rc::new(|value: &str| {
            if value.contains('.') {
                Err("No dots.".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(admit(&tags, "vat", None, None), Admission::Added);
        assert_eq!(admit(&tags, "payroll", None, None), Admission::Repeats(1));
        assert_eq!(
            admit(&tags, "vat", Some(2), None),
            Admission::Refused("No more than 2 fit here.".into())
        );
        assert_eq!(
            admit(&tags, "v.a.t", None, Some(&no_dots)),
            Admission::Refused("No dots.".into())
        );
        // A repeat is pointed out even when the field is full.
        assert_eq!(
            admit(&tags, "tax", Some(2), Some(&no_dots)),
            Admission::Repeats(0)
        );
    }

    #[test]
    fn arrows_walk_the_chips_from_the_field() {
        assert_eq!(walk(None, 3, "left"), Some(2));
        assert_eq!(walk(Some(2), 3, "left"), Some(1));
        assert_eq!(walk(Some(0), 3, "left"), Some(0));
        assert_eq!(walk(Some(1), 3, "right"), Some(2));
        // Right from the last chip goes back to the field.
        assert_eq!(walk(Some(2), 3, "right"), None);
        assert_eq!(walk(None, 3, "right"), None);
        assert_eq!(walk(None, 0, "left"), None);
    }

    #[test]
    fn a_chip_shows_the_first_letter() {
        assert_eq!(initial("maya@books.example"), "M");
        assert_eq!(initial(""), "");
    }
}
