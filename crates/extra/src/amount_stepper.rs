//! Amount stepper: a number field for money, hours or quantities that reads what people paste, tidies it on leaving, and steps with buttons or arrow keys.
//!
//! It accepts "1,250.50", "$1 250" or "(40)" for minus forty, keeps the value
//! between a floor and a ceiling, and shows it with grouped thousands and a
//! fixed number of decimals once the field loses focus. Up and Down step
//! by one step, or ten with Shift. It reports each settled value.

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};

/// Read an amount as people write it, or `None` when it isn't one.
pub fn parse_amount(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let negative = trimmed.starts_with('-') || (trimmed.starts_with('(') && trimmed.ends_with(')'));
    let digits: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if digits.is_empty() || digits.matches('.').count() > 1 {
        return None;
    }
    let value: f64 = digits.parse().ok()?;
    Some(if negative { -value } else { value })
}

/// Write an amount with grouped thousands and `decimals` places, such as "-1,250.50".
pub fn format_amount(value: f64, decimals: usize) -> String {
    let fixed = format!("{:.*}", decimals, value.abs());
    let (whole, fraction) = match fixed.split_once('.') {
        Some((w, f)) => (w.to_string(), Some(f.to_string())),
        None => (fixed.clone(), None),
    };
    let mut grouped = String::new();
    for (ix, ch) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if value < 0.0 && fixed.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "-"
    } else {
        ""
    };
    match fraction {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Move `value` by `steps` steps and keep it between `min` and `max`.
pub fn step_amount(value: f64, step: f64, steps: i32, min: f64, max: f64) -> f64 {
    let moved = value + step * f64::from(steps);
    let snapped = if step > 0.0 {
        (moved / step).round() * step
    } else {
        moved
    };
    snapped.clamp(min, max)
}

/// What the stepper reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmountEvent {
    /// The value settled on, after a step or leaving the field.
    Change(f64),
}

/// A number field with steps.
pub struct AmountStepper {
    field: Entity<InputState>,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    decimals: usize,
    unit: Option<SharedString>,
    unit_first: bool,
    label: SharedString,
    disabled: bool,
    _events: Subscription,
}

impl EventEmitter<AmountEvent> for AmountStepper {}

impl Focusable for AmountStepper {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl AmountStepper {
    /// Create a stepper holding `value`, named `label` for screen readers.
    pub fn new(
        label: impl Into<SharedString>,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| {
            InputState::new(window, cx).validate(|text, _| {
                text.chars().all(|c| {
                    c.is_ascii_digit()
                        || matches!(
                            c,
                            '.' | ',' | '-' | '(' | ')' | ' ' | '$' | '\u{20ac}' | '\u{a3}'
                        )
                })
            })
        });
        let events = cx.subscribe_in(
            &field,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                InputEvent::Blur | InputEvent::PressEnter { .. } => this.settle(window, cx),
                _ => {}
            },
        );
        let mut this = Self {
            field,
            value,
            min: f64::MIN,
            max: f64::MAX,
            step: 1.0,
            decimals: 2,
            unit: None,
            unit_first: true,
            label: label.into(),
            disabled: false,
            _events: events,
        };
        this.show(window, cx);
        this
    }

    /// Keep the value between these.
    pub fn range(
        mut self,
        min: f64,
        max: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self.value = self.value.clamp(self.min, self.max);
        self.show(window, cx);
        self
    }

    /// Step by this much. One by default.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step.abs();
        self
    }

    /// Show this many decimal places. Two by default.
    pub fn decimals(
        mut self,
        decimals: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        self.decimals = decimals;
        self.show(window, cx);
        self
    }

    /// Show a unit before the number, such as "$".
    pub fn prefix_unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self.unit_first = true;
        self
    }

    /// Show a unit after the number, such as "hrs".
    pub fn suffix_unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self.unit_first = false;
        self
    }

    /// The settled value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Replace the value without reporting it.
    pub fn set_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value.clamp(self.min, self.max);
        self.show(window, cx);
    }

    /// Turn the stepper off or on.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = format_amount(self.value, self.decimals);
        self.field
            .update(cx, |field, cx| field.set_value(text, window, cx));
        cx.notify();
    }

    fn settle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.field.read(cx).value().to_string();
        if let Some(value) = parse_amount(&typed) {
            let value = value.clamp(self.min, self.max);
            if (value - self.value).abs() > f64::EPSILON {
                self.value = value;
                cx.emit(AmountEvent::Change(value));
            }
        }
        self.show(window, cx);
    }

    fn nudge(&mut self, steps: i32, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let base = parse_amount(&self.field.read(cx).value()).unwrap_or(self.value);
        let value = step_amount(base, self.step, steps, self.min, self.max);
        self.value = value;
        cx.emit(AmountEvent::Change(value));
        self.show(window, cx);
    }
}

impl Render for AmountStepper {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let unit = self.unit.clone();
        let at_min = self.value <= self.min;
        let at_max = self.value >= self.max;
        let disabled = self.disabled;
        h_flex()
            .gap_1()
            .w(px(200.))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let big = if event.keystroke.modifiers.shift {
                    10
                } else {
                    1
                };
                match event.keystroke.key.as_str() {
                    "up" => {
                        this.nudge(big, window, cx);
                        cx.stop_propagation();
                    }
                    "down" => {
                        this.nudge(-big, window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(
                Button::new(SharedString::from(format!("{}-decrease", self.label)))
                    .small()
                    .outline()
                    .icon(IconName::Minus)
                    .accessibility_label(format!("Decrease {}", self.label))
                    .disabled(disabled || at_min)
                    .on_click(cx.listener(|this, _, window, cx| this.nudge(-1, window, cx))),
            )
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.field)
                        .small()
                        .disabled(disabled)
                        .aria_label(self.label.clone())
                        .when_some(unit.clone().filter(|_| self.unit_first), |this, unit| {
                            this.prefix(div().text_sm().text_color(quiet).child(unit))
                        })
                        .when_some(unit.filter(|_| !self.unit_first), |this, unit| {
                            this.suffix(div().text_sm().text_color(quiet).child(unit))
                        }),
                ),
            )
            .child(
                Button::new(SharedString::from(format!("{}-increase", self.label)))
                    .small()
                    .outline()
                    .icon(IconName::Plus)
                    .accessibility_label(format!("Increase {}", self.label))
                    .disabled(disabled || at_max)
                    .on_click(cx.listener(|this, _, window, cx| this.nudge(1, window, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_read_write_and_step() {
        assert_eq!(parse_amount("1,250.50"), Some(1250.5));
        assert_eq!(parse_amount("$1 250"), Some(1250.0));
        assert_eq!(parse_amount("(40)"), Some(-40.0));
        assert_eq!(parse_amount("-3"), Some(-3.0));
        assert_eq!(parse_amount("abc"), None);
        assert_eq!(parse_amount("1.2.3"), None);
        assert_eq!(format_amount(1250.5, 2), "1,250.50");
        assert_eq!(format_amount(-1234567.0, 0), "-1,234,567");
        assert_eq!(format_amount(999.0, 1), "999.0");
        assert_eq!(format_amount(-0.001, 2), "0.00");
        assert_eq!(step_amount(9.5, 0.25, 2, 0.0, 10.0), 10.0);
        assert_eq!(step_amount(9.5, 0.25, 4, 0.0, 10.0), 10.0);
        assert_eq!(step_amount(1.0, 1.0, -10, 0.0, 10.0), 0.0);
        assert_eq!(step_amount(2.3, 1.0, 1, 0.0, 10.0), 3.0);
    }
}
