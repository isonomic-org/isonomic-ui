// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Billing phase editor: the phases of a contract, each with its dates, line items, discount and minimum.
//!
//! Each phase is a card: when it starts and for how many months, the line
//! items with quantity and unit price, a percentage discount and a monthly
//! minimum. Totals update as values change: per month, per phase and for
//! the whole contract. An amount that does not read as money is marked, and
//! the totals keep its last good value until it does.
//!
//! The editor reports every change with the whole plan, so its owner can
//! keep it, check it or save it.

use std::collections::HashMap;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, Entity, EventEmitter, Hsla, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
    relative,
};

use crate::common::resting_shadow;

/// One thing billed in a phase.
#[derive(Debug, Clone, PartialEq)]
pub struct LineItem {
    /// Identifies the item within its phase.
    pub id: SharedString,
    /// What it is.
    pub name: SharedString,
    /// How many.
    pub quantity: u32,
    /// The price of one, in cents, per month.
    pub unit_cents: i64,
}

impl LineItem {
    /// Create an item.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        quantity: u32,
        unit_cents: i64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            quantity,
            unit_cents,
        }
    }
}

/// One phase of a contract.
#[derive(Debug, Clone, PartialEq)]
pub struct BillingPhase {
    /// Identifies the phase.
    pub id: SharedString,
    /// Its name, such as "Ramp-up".
    pub name: SharedString,
    /// When it starts, as written, such as "2027-01-01".
    pub start: SharedString,
    /// How many months it runs.
    pub months: u32,
    /// What it bills each month.
    pub items: Vec<LineItem>,
    /// A discount on the items, in percent.
    pub discount_percent: u32,
    /// The least it bills each month, in cents. Zero for none.
    pub minimum_cents: i64,
}

impl BillingPhase {
    /// Create an empty phase.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        start: impl Into<SharedString>,
        months: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            start: start.into(),
            months: months.max(1),
            items: Vec::new(),
            discount_percent: 0,
            minimum_cents: 0,
        }
    }

    /// Add a line item.
    pub fn item(mut self, item: LineItem) -> Self {
        self.items.push(item);
        self
    }

    /// Set the discount, in percent.
    pub fn discount(mut self, percent: u32) -> Self {
        self.discount_percent = percent.min(100);
        self
    }

    /// Set the monthly minimum, in cents.
    pub fn minimum(mut self, cents: i64) -> Self {
        self.minimum_cents = cents.max(0);
        self
    }

    /// What the phase bills each month and over its length.
    pub fn totals(&self) -> PhaseTotals {
        let subtotal: i64 = self
            .items
            .iter()
            .map(|item| item.unit_cents * item.quantity as i64)
            .sum();
        let discount = subtotal * self.discount_percent.min(100) as i64 / 100;
        let after = subtotal - discount;
        let top_up = (self.minimum_cents - after).max(0);
        let monthly = after + top_up;
        PhaseTotals {
            subtotal,
            discount,
            top_up,
            monthly,
            total: monthly * self.months as i64,
        }
    }
}

/// The sums for one phase, in cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseTotals {
    /// The items before the discount.
    pub subtotal: i64,
    /// What the discount takes off.
    pub discount: i64,
    /// What brings the month up to the minimum.
    pub top_up: i64,
    /// What one month bills.
    pub monthly: i64,
    /// What the whole phase bills.
    pub total: i64,
}

/// Read money as written, such as "1,250", "1250.5" or "$1,250.00", into cents.
pub fn parse_cents(text: &str) -> Option<i64> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, ',' | ' ' | '$' | '\u{20ac}' | '\u{a3}'))
        .collect();
    if cleaned.is_empty() {
        return Some(0);
    }
    let (whole, fraction) = match cleaned.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (cleaned.as_str(), ""),
    };
    if fraction.len() > 2
        || !whole.chars().all(|ch| ch.is_ascii_digit())
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
        || whole.is_empty() && fraction.is_empty()
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let cents: i64 = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse().ok()?,
    };
    whole.checked_mul(100)?.checked_add(cents)
}

/// Write cents as money with thousands separators: "1,250.00".
pub fn money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    let whole = (cents / 100).to_string();
    let mut grouped = String::new();
    for (ix, digit) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{sign}{grouped}.{:02}", cents % 100)
}

/// What happened in the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum BillingPhaseEvent {
    /// The plan changed. Carries every phase as it now stands.
    Changed(Vec<BillingPhase>),
}

/// What a text field edits.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Field {
    Start(SharedString),
    Minimum(SharedString),
    ItemName(SharedString, SharedString),
    ItemPrice(SharedString, SharedString),
}

/// The phases of a contract, edited in place with live totals.
pub struct BillingPhaseEditor {
    phases: Vec<BillingPhase>,
    currency: SharedString,
    inputs: HashMap<Field, Entity<InputState>>,
    invalid: Vec<Field>,
    made: usize,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<BillingPhaseEvent> for BillingPhaseEditor {}

impl BillingPhaseEditor {
    /// Create an editor for these phases.
    pub fn new(phases: Vec<BillingPhase>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut editor = Self {
            phases: Vec::new(),
            currency: "$".into(),
            inputs: HashMap::new(),
            invalid: Vec::new(),
            made: 0,
            _subscriptions: Vec::new(),
        };
        editor.set_phases(phases, window, cx);
        editor
    }

    /// Show this currency sign before amounts. "$" by default.
    pub fn set_currency(&mut self, sign: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.currency = sign.into();
        cx.notify();
    }

    /// Replace the phases, such as after loading a saved plan.
    pub fn set_phases(
        &mut self,
        phases: Vec<BillingPhase>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.phases = phases;
        self.inputs.clear();
        self.invalid.clear();
        self._subscriptions.clear();
        let phases = self.phases.clone();
        for phase in &phases {
            self.field(
                Field::Start(phase.id.clone()),
                &phase.start,
                "YYYY-MM-DD",
                window,
                cx,
            );
            let minimum = if phase.minimum_cents == 0 {
                String::new()
            } else {
                money(phase.minimum_cents)
            };
            self.field(
                Field::Minimum(phase.id.clone()),
                &minimum,
                "None",
                window,
                cx,
            );
            for item in &phase.items {
                self.item_fields(&phase.id, item, window, cx);
            }
        }
        cx.notify();
    }

    /// The phases as they now stand.
    pub fn phases(&self) -> &[BillingPhase] {
        &self.phases
    }

    fn item_fields(
        &mut self,
        phase: &SharedString,
        item: &LineItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = Field::ItemName(phase.clone(), item.id.clone());
        let price = Field::ItemPrice(phase.clone(), item.id.clone());
        self.field(name, &item.name, "What it is", window, cx);
        self.field(price, &money(item.unit_cents), "0.00", window, cx);
    }

    fn field(
        &mut self,
        field: Field,
        value: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.to_string();
        let placeholder = SharedString::from(placeholder.to_string());
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(placeholder);
            input.set_value(value, window, cx);
            input
        });
        let key = field.clone();
        let subscription = cx.subscribe_in(&input, window, move |this, input, event, _, cx| {
            if let InputEvent::Change = event {
                let text = input.read(cx).value().to_string();
                this.edit(&key, &text, cx);
            }
        });
        self.inputs.insert(field, input);
        self._subscriptions.push(subscription);
    }

    fn phase_mut(&mut self, id: &SharedString) -> Option<&mut BillingPhase> {
        self.phases.iter_mut().find(|phase| &phase.id == id)
    }

    fn edit(&mut self, field: &Field, text: &str, cx: &mut Context<Self>) {
        let mut valid = true;
        match field {
            Field::Start(phase) => {
                if let Some(phase) = self.phase_mut(phase) {
                    phase.start = text.trim().to_string().into();
                }
            }
            Field::Minimum(phase) => match parse_cents(text) {
                Some(cents) => {
                    if let Some(phase) = self.phase_mut(phase) {
                        phase.minimum_cents = cents;
                    }
                }
                None => valid = false,
            },
            Field::ItemName(phase, item) => {
                if let Some(item) = self
                    .phase_mut(phase)
                    .and_then(|phase| phase.items.iter_mut().find(|i| &i.id == item))
                {
                    item.name = text.trim().to_string().into();
                }
            }
            Field::ItemPrice(phase, item) => match parse_cents(text) {
                Some(cents) => {
                    if let Some(item) = self
                        .phase_mut(phase)
                        .and_then(|phase| phase.items.iter_mut().find(|i| &i.id == item))
                    {
                        item.unit_cents = cents;
                    }
                }
                None => valid = false,
            },
        }
        self.invalid.retain(|bad| bad != field);
        if !valid {
            self.invalid.push(field.clone());
        }
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(BillingPhaseEvent::Changed(self.phases.clone()));
        cx.notify();
    }

    fn step(&mut self, phase: &SharedString, what: Step, up: bool, cx: &mut Context<Self>) {
        if let Some(phase) = self.phase_mut(phase) {
            match what {
                Step::Months => {
                    phase.months = if up {
                        phase.months.saturating_add(1).min(120)
                    } else {
                        phase.months.saturating_sub(1).max(1)
                    }
                }
                Step::Discount => {
                    phase.discount_percent = if up {
                        (phase.discount_percent + 5).min(100)
                    } else {
                        phase.discount_percent.saturating_sub(5)
                    }
                }
                Step::Quantity(ref item) => {
                    if let Some(item) = phase.items.iter_mut().find(|i| &i.id == item) {
                        item.quantity = if up {
                            item.quantity.saturating_add(1).min(9_999)
                        } else {
                            item.quantity.saturating_sub(1)
                        }
                    }
                }
            }
        }
        self.changed(cx);
    }

    fn add_item(&mut self, phase: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.made += 1;
        let item = LineItem::new(format!("new-{}", self.made), "", 1, 0);
        if let Some(target) = self.phase_mut(phase) {
            target.items.push(item.clone());
        }
        self.item_fields(phase, &item, window, cx);
        if let Some(input) = self
            .inputs
            .get(&Field::ItemName(phase.clone(), item.id.clone()))
        {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        self.changed(cx);
    }

    fn remove_item(&mut self, phase: &SharedString, item: &SharedString, cx: &mut Context<Self>) {
        if let Some(target) = self.phase_mut(phase) {
            target.items.retain(|i| &i.id != item);
        }
        self.inputs
            .remove(&Field::ItemName(phase.clone(), item.clone()));
        self.inputs
            .remove(&Field::ItemPrice(phase.clone(), item.clone()));
        self.invalid
            .retain(|bad| bad != &Field::ItemPrice(phase.clone(), item.clone()));
        self.changed(cx);
    }

    fn add_phase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.made += 1;
        let number = self.phases.len() + 1;
        let phase = BillingPhase::new(
            format!("phase-{}", self.made),
            format!("Phase {number}"),
            "",
            12,
        );
        self.field(Field::Start(phase.id.clone()), "", "YYYY-MM-DD", window, cx);
        self.field(Field::Minimum(phase.id.clone()), "", "None", window, cx);
        self.phases.push(phase);
        self.changed(cx);
    }

    fn remove_phase(&mut self, phase: &SharedString, cx: &mut Context<Self>) {
        self.phases.retain(|p| &p.id != phase);
        self.inputs.retain(|field, _| match field {
            Field::Start(id) | Field::Minimum(id) => id != phase,
            Field::ItemName(id, _) | Field::ItemPrice(id, _) => id != phase,
        });
        self.changed(cx);
    }

    fn input(&self, field: &Field, danger: Hsla) -> Option<Input> {
        let bad = self.invalid.contains(field);
        self.inputs.get(field).map(|input| {
            Input::new(input)
                .small()
                .when(bad, |this| this.border_color(danger))
        })
    }
}

/// What a stepper changes.
#[derive(Clone)]
enum Step {
    Months,
    Discount,
    Quantity(SharedString),
}

impl BillingPhaseEditor {
    fn stepper(
        &self,
        id: SharedString,
        name: &str,
        value: String,
        phase: &SharedString,
        what: Step,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (down_phase, up_phase) = (phase.clone(), phase.clone());
        let (down_what, up_what) = (what.clone(), what);
        h_flex()
            .flex_none()
            .gap_0p5()
            .child(
                Button::new(SharedString::from(format!("{id}-down")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Minus)
                    .accessibility_label(format!("Less {name}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.step(&down_phase, down_what.clone(), false, cx)
                    })),
            )
            .child(
                div()
                    .min_w(px(28.))
                    .text_center()
                    .font_medium()
                    .child(value),
            )
            .child(
                Button::new(SharedString::from(format!("{id}-up")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .accessibility_label(format!("More {name}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.step(&up_phase, up_what.clone(), true, cx)
                    })),
            )
    }

    fn phase_card(
        &self,
        ix: usize,
        phase: &BillingPhase,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let success = theme.success;
        let danger = theme.danger;
        let ink = theme.foreground;
        let muted = theme.muted;
        let fill = theme.background;
        let radius = theme.radius_lg;
        let sign = self.currency.clone();
        let totals = phase.totals();
        let id = phase.id.clone();
        let can_remove = self.phases.len() > 1;

        let rows =
            phase.items.iter().map(|item| {
                let amount = item.unit_cents * item.quantity as i64;
                let remove_phase = id.clone();
                let remove_item = item.id.clone();
                h_flex()
                    .gap_2()
                    .h(px(34.))
                    .child(div().flex_1().min_w_0().children(
                        self.input(&Field::ItemName(id.clone(), item.id.clone()), danger),
                    ))
                    .child(self.stepper(
                        format!("{id}-{}-qty", item.id).into(),
                        "quantity",
                        item.quantity.to_string(),
                        &id,
                        Step::Quantity(item.id.clone()),
                        cx,
                    ))
                    .child(div().w(px(96.)).flex_none().children(
                        self.input(&Field::ItemPrice(id.clone(), item.id.clone()), danger),
                    ))
                    .child(
                        div()
                            .w(px(84.))
                            .flex_none()
                            .text_right()
                            .font_medium()
                            .child(money(amount)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{id}-{}-remove", item.id)))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .accessibility_label(if item.name.is_empty() {
                                "Remove item".to_string()
                            } else {
                                format!("Remove {}", item.name)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_item(&remove_phase, &remove_item, cx)
                            })),
                    )
            });
        let rows: Vec<_> = rows.collect();

        let sum_row = |label: String, value: String, strong: bool| {
            h_flex()
                .justify_between()
                .text_color(if strong { ink } else { quiet })
                .when(strong, |this| this.font_medium())
                .child(label)
                .child(value)
        };
        let add_phase = id.clone();
        let remove_phase = id.clone();

        v_flex()
            .gap_3()
            .p_4()
            .rounded(radius)
            .border_1()
            .border_color(line)
            .bg(fill)
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        div()
                            .flex_none()
                            .size(px(22.))
                            .rounded_full()
                            .bg(muted)
                            .text_xs()
                            .font_medium()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child((ix + 1).to_string()),
                    )
                    .child(div().font_medium().child(phase.name.clone()))
                    .child(div().text_color(quiet).child("from"))
                    .child(
                        div()
                            .w(px(116.))
                            .children(self.input(&Field::Start(id.clone()), danger)),
                    )
                    .child(div().text_color(quiet).child("for"))
                    .child(self.stepper(
                        format!("{id}-months").into(),
                        "months",
                        phase.months.to_string(),
                        &id,
                        Step::Months,
                        cx,
                    ))
                    .child(div().text_color(quiet).child(if phase.months == 1 {
                        "month"
                    } else {
                        "months"
                    }))
                    .child(div().flex_1())
                    .when(can_remove, |this| {
                        this.child(
                            Button::new(SharedString::from(format!("{id}-remove")))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Delete)
                                .accessibility_label(format!("Remove {}", phase.name))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_phase(&remove_phase, cx)
                                })),
                        )
                    }),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .text_color(quiet)
                            .child(div().flex_1().child("Item"))
                            .child(div().w(px(80.)).text_center().child("Qty"))
                            .child(div().w(px(96.)).child(format!("Unit, {sign}/mo")))
                            .child(div().w(px(84.)).text_right().child("Amount"))
                            .child(div().w(px(24.))),
                    )
                    .children(rows)
                    .when(phase.items.is_empty(), |this| {
                        this.child(
                            div()
                                .py_2()
                                .text_color(quiet)
                                .child("Nothing billed in this phase yet"),
                        )
                    })
                    .child(
                        h_flex().child(
                            Button::new(SharedString::from(format!("{id}-add-item")))
                                .ghost()
                                .small()
                                .icon(IconName::Plus)
                                .label("Add item")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.add_item(&add_phase, window, cx)
                                })),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_x_6()
                    .gap_y_2()
                    .pt_3()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().text_color(quiet).child("Discount"))
                            .child(self.stepper(
                                format!("{id}-discount").into(),
                                "discount",
                                format!("{}%", phase.discount_percent),
                                &id,
                                Step::Discount,
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().text_color(quiet).child(format!("Minimum, {sign}/mo")))
                            .child(
                                div()
                                    .w(px(96.))
                                    .children(self.input(&Field::Minimum(id.clone()), danger)),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_0p5()
                    .text_sm()
                    .child(sum_row("Items".into(), money(totals.subtotal), false))
                    .when(totals.discount > 0, |this| {
                        this.child(sum_row(
                            format!("Discount {}%", phase.discount_percent),
                            format!("-{}", money(totals.discount)),
                            false,
                        ))
                    })
                    .when(totals.top_up > 0, |this| {
                        this.child(sum_row(
                            "Up to the minimum".into(),
                            format!("+{}", money(totals.top_up)),
                            false,
                        ))
                    })
                    .child(sum_row(
                        "Per month".into(),
                        format!("{sign}{}", money(totals.monthly)),
                        true,
                    ))
                    .child(
                        h_flex()
                            .justify_between()
                            .text_color(success)
                            .font_medium()
                            .child(format!("{} \u{d7} {} months", phase.name, phase.months))
                            .child(format!("{sign}{}", money(totals.total))),
                    ),
            )
    }
}

impl Render for BillingPhaseEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let ink = theme.foreground;
        let danger = theme.danger;
        let sign = self.currency.clone();
        let total: i64 = self.phases.iter().map(|phase| phase.totals().total).sum();
        let months: u32 = self.phases.iter().map(|phase| phase.months).sum();
        let average = if months == 0 {
            0
        } else {
            total / months as i64
        };
        let bad = self.invalid.len();

        let phases = self.phases.clone();
        let cards: Vec<_> = phases
            .iter()
            .enumerate()
            .map(|(ix, phase)| self.phase_card(ix, phase, cx).into_any_element())
            .collect();

        v_flex()
            .w_full()
            .max_w(px(640.))
            .gap_3()
            .text_sm()
            .text_color(ink)
            .line_height(relative(1.45))
            .children(cards)
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        Button::new("add-phase")
                            .outline()
                            .small()
                            .icon(IconName::Plus)
                            .label("Add phase")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_phase(window, cx)),
                            ),
                    )
                    .when(bad > 0, |this| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .text_xs()
                                .text_color(danger)
                                .child(Icon::new(IconName::TriangleAlert).xsmall())
                                .child(if bad == 1 {
                                    "One amount isn't a number; its last good value is used".to_string()
                                } else {
                                    format!("{bad} amounts aren't numbers; their last good values are used")
                                }),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        v_flex()
                            .items_end()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .child(format!("{sign}{}", money(total))),
                            )
                            .child(div().text_xs().text_color(quiet).child(format!(
                                "over {months} months, {sign}{} a month on average",
                                money(average)
                            ))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_reads_the_usual_ways() {
        assert_eq!(parse_cents("1250"), Some(125_000));
        assert_eq!(parse_cents("1,250.5"), Some(125_050));
        assert_eq!(parse_cents("$1,250.05"), Some(125_005));
        assert_eq!(parse_cents(".5"), Some(50));
        assert_eq!(parse_cents(""), Some(0));
        assert_eq!(parse_cents("12.345"), None);
        assert_eq!(parse_cents("twelve"), None);
    }

    #[test]
    fn money_writes_with_separators() {
        assert_eq!(money(0), "0.00");
        assert_eq!(money(125_005), "1,250.05");
        assert_eq!(money(123_456_789), "1,234,567.89");
        assert_eq!(money(-500), "-5.00");
    }

    #[test]
    fn a_phase_adds_up() {
        let phase = BillingPhase::new("p", "Ramp-up", "2027-01-01", 3)
            .item(LineItem::new("seats", "Seats", 10, 4_000))
            .item(LineItem::new("support", "Support", 1, 15_000))
            .discount(10);
        let totals = phase.totals();
        assert_eq!(totals.subtotal, 55_000);
        assert_eq!(totals.discount, 5_500);
        assert_eq!(totals.top_up, 0);
        assert_eq!(totals.monthly, 49_500);
        assert_eq!(totals.total, 148_500);
    }

    #[test]
    fn the_minimum_tops_a_light_month_up() {
        let phase = BillingPhase::new("p", "Pilot", "2027-01-01", 2)
            .item(LineItem::new("seats", "Seats", 2, 4_000))
            .minimum(20_000);
        let totals = phase.totals();
        assert_eq!(totals.top_up, 12_000);
        assert_eq!(totals.monthly, 20_000);
        assert_eq!(totals.total, 40_000);
    }
}
