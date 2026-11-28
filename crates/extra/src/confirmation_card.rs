// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Confirmation card: what an assistant is about to do, itemised, to confirm
//! or change before it acts.
//!
//! Line items can have a quantity stepper and a remove button. Under them sit
//! the subtotal, any fees and the total, which roll as amounts change, and
//! any warnings. The footer cancels, goes back to edit, or confirms for the
//! total. Once done the card folds to one row with a reference and, for a
//! while, an undo with the time left.
//!
//! The host owns the items: the card reports a new quantity or a removal and
//! is rendered again with the result.

use std::rc::Rc;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    TextRole as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    progress::Progress,
    rolling_number::RollingNumber,
    tag::Tag,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::{icon_tile, inset, panel, shell, shell_header};

/// One thing in the list.
#[derive(Clone)]
pub struct ConfirmationItem {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    amount: f64,
    quantity: u32,
    editable: bool,
    removable: bool,
}

impl ConfirmationItem {
    /// An item with the price of one.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, amount: f64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            amount,
            quantity: 1,
            editable: false,
            removable: false,
        }
    }

    /// A quieter second line.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// How many. One by default.
    pub fn quantity(mut self, quantity: u32) -> Self {
        self.quantity = quantity.max(1);
        self
    }

    /// Show a stepper for the quantity.
    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Show a remove button.
    pub fn removable(mut self) -> Self {
        self.removable = true;
        self
    }

    /// The price of this line: one times how many.
    pub fn line_total(&self) -> f64 {
        self.amount * f64::from(self.quantity)
    }
}

/// Where the card stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConfirmationStatus {
    /// Waiting for the person.
    #[default]
    Review,
    /// Confirmed, and being carried out.
    Confirming,
    /// Carried out.
    Done,
}

/// An amount of money: `symbol`, digits grouped in threes, two decimals.
pub fn money(amount: f64, symbol: &str) -> String {
    let cents = (amount.abs() * 100.).round() as u64;
    let whole = (cents / 100).to_string();
    let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
    for (ix, digit) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let sign = if amount < 0. && cents > 0 { "-" } else { "" };
    format!("{sign}{symbol}{grouped}.{:02}", cents % 100)
}

/// The sum of every line.
pub fn subtotal(items: &[ConfirmationItem]) -> f64 {
    items.iter().map(ConfirmationItem::line_total).sum()
}

type QuantityHandler = Rc<dyn Fn(&SharedString, u32, &mut Window, &mut App)>;
type ItemHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// An itemised action to confirm.
#[derive(IntoElement)]
pub struct ConfirmationCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    items: Vec<ConfirmationItem>,
    fees: Vec<(SharedString, f64)>,
    warnings: Vec<SharedString>,
    reference: Option<SharedString>,
    done_note: Option<SharedString>,
    currency: SharedString,
    status: ConfirmationStatus,
    confirm_label: SharedString,
    done_label: SharedString,
    undo: Option<(u32, u32)>,
    on_quantity: Option<QuantityHandler>,
    on_remove: Option<ItemHandler>,
    on_confirm: Option<Handler>,
    on_edit: Option<Handler>,
    on_cancel: Option<Handler>,
    on_undo: Option<Handler>,
}

impl ConfirmationCard {
    /// Create a card in review, priced in dollars.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            items: Vec::new(),
            fees: Vec::new(),
            warnings: Vec::new(),
            reference: None,
            done_note: None,
            currency: "$".into(),
            status: ConfirmationStatus::Review,
            confirm_label: "Confirm".into(),
            done_label: "Done".into(),
            undo: None,
            on_quantity: None,
            on_remove: None,
            on_confirm: None,
            on_edit: None,
            on_cancel: None,
            on_undo: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The line items.
    pub fn items(mut self, items: Vec<ConfirmationItem>) -> Self {
        self.items = items;
        self
    }

    /// A fee or tax added to the subtotal.
    pub fn fee(mut self, label: impl Into<SharedString>, amount: f64) -> Self {
        self.fees.push((label.into(), amount));
        self
    }

    /// Something the person should know before confirming.
    pub fn warning(mut self, warning: impl Into<SharedString>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// A reference shown once done, such as an order number.
    pub fn reference(mut self, reference: impl Into<SharedString>) -> Self {
        self.reference = Some(reference.into());
        self
    }

    /// A note shown once done in place of the total, such as when it happened.
    pub fn done_note(mut self, note: impl Into<SharedString>) -> Self {
        self.done_note = Some(note.into());
        self
    }

    /// The currency symbol. "$" by default.
    pub fn currency(mut self, symbol: impl Into<SharedString>) -> Self {
        self.currency = symbol.into();
        self
    }

    /// Where the card stands.
    pub fn status(mut self, status: ConfirmationStatus) -> Self {
        self.status = status;
        self
    }

    /// The confirm button's verb. "Confirm" by default.
    pub fn confirm_label(mut self, label: impl Into<SharedString>) -> Self {
        self.confirm_label = label.into();
        self
    }

    /// The word in the tag once done. "Done" by default.
    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        self.done_label = label.into();
        self
    }

    /// Once done, offer undo: seconds left of the seconds allowed. The host
    /// counts down and stops passing this when time is up.
    pub fn undo_window(mut self, seconds_left: u32, seconds_allowed: u32) -> Self {
        self.undo = Some((seconds_left, seconds_allowed.max(1)));
        self
    }

    /// Run with an item's id and its new quantity.
    pub fn on_quantity(
        mut self,
        handler: impl Fn(&SharedString, u32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_quantity = Some(Rc::new(handler));
        self
    }

    /// Run with the id of an item to remove.
    pub fn on_remove(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    /// Run when the person confirms.
    pub fn on_confirm(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }

    /// Show an Edit button.
    pub fn on_edit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// Show a Cancel button.
    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// Run when the person undoes a finished action.
    pub fn on_undo(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }

    /// Subtotal plus fees.
    pub fn total(&self) -> f64 {
        subtotal(&self.items) + self.fees.iter().map(|(_, amount)| amount).sum::<f64>()
    }
}

impl Styled for ConfirmationCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ConfirmationCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let good = theme.success;
        let caution = theme.warning;
        let mono = theme.mono_font_family.clone();
        let symbol = self.currency.clone();
        let total = money(self.total(), &symbol);
        let in_review = self.status == ConfirmationStatus::Review;
        let frame = shell(cx)
            .id(self.id.clone())
            .max_w(px(448.))
            .role(Role::Region)
            .aria_label(match self.status {
                ConfirmationStatus::Review => self.title.to_string(),
                ConfirmationStatus::Confirming => format!("{}, confirming", self.title),
                ConfirmationStatus::Done => {
                    format!("{}, {}, {total}", self.title, self.done_label)
                }
            });

        if self.status == ConfirmationStatus::Done {
            let note = self
                .done_note
                .clone()
                .unwrap_or_else(|| total.clone().into());
            let undo = self
                .on_undo
                .clone()
                .zip(self.undo)
                .filter(|(_, (left, _))| *left > 0);
            return frame
                .child(
                    panel(cx)
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_3()
                        .child(
                            icon_tile(Icon::new(CatalogIcon::CheckmarkFilled), cx)
                                .bg(good.opacity(0.15))
                                .text_color(good),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_title().child(self.title.clone()))
                                .child(
                                    h_flex()
                                        .text_body()
                                        .text_color(quiet)
                                        .when_some(self.reference.clone(), |this, reference| {
                                            this.child(
                                                div()
                                                    .font_family(mono.clone())
                                                    .child(format!("{reference} · ")),
                                            )
                                        })
                                        .child(div().min_w_0().truncate().child(note)),
                                ),
                        )
                        .child(
                            Tag::success()
                                .rounded_full()
                                .gap(px(6.))
                                .child(Icon::new(CatalogIcon::CheckmarkFilled).size(px(14.)))
                                .child(self.done_label.clone()),
                        ),
                )
                .when_some(undo, |this, (undo, (left, allowed))| {
                    this.child(
                        h_flex()
                            .gap_3()
                            .pl_3()
                            .pt(px(6.))
                            .child(
                                div().flex_1().child(
                                    Progress::new(part("undo-time".into()))
                                        .small()
                                        .value(left as f32 / allowed as f32 * 100.)
                                        .color(quiet)
                                        .accessibility_label(format!(
                                            "Time left to undo: {left} seconds"
                                        )),
                                ),
                            )
                            .child(
                                Button::new(part("undo".into()))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Undo)
                                    .accessibility_label(format!("Undo, {left} seconds left"))
                                    .child("Undo ·")
                                    .child(
                                        RollingNumber::new(
                                            part("undo-left".into()),
                                            format!("{left}s"),
                                        )
                                        .line_height(px(16.)),
                                    )
                                    .on_click(move |_, window, cx| undo(window, cx)),
                            ),
                    )
                })
                .refine_style(&self.style);
        }

        let rows = self.items.iter().map(|item| {
            let stepper = item.editable.then(|| {
                let less = self.on_quantity.clone();
                let more = self.on_quantity.clone();
                let (less_id, more_id) = (item.id.clone(), item.id.clone());
                let quantity = item.quantity;
                h_flex()
                    .id(part(format!("quantity-{}", item.id)))
                    .gap_1()
                    .p(px(2.))
                    .rounded(px(16.))
                    .bg(look::ink::wash_strong(cx))
                    .role(Role::Group)
                    .aria_label(format!("{} quantity, {quantity}", item.label))
                    .child(
                        Button::new(part(format!("less-{}", item.id)))
                            .ghost()
                            .small()
                            .icon(Icon::new(CatalogIcon::Subtract))
                            .rounded(px(14.))
                            .size(px(28.))
                            .accessibility_label("Decrease")
                            .disabled(quantity <= 1 || !in_review)
                            .on_click(move |_, window, cx| {
                                if let Some(less) = &less {
                                    less(&less_id, quantity - 1, window, cx);
                                }
                            }),
                    )
                    .child(
                        div()
                            .w(px(20.))
                            .flex()
                            .justify_center()
                            .font_family(mono.clone())
                            .child(
                                RollingNumber::new(
                                    part(format!("count-{}", item.id)),
                                    quantity.to_string(),
                                )
                                .text_size(px(12.))
                                .line_height(px(16.)),
                            ),
                    )
                    .child(
                        Button::new(part(format!("more-{}", item.id)))
                            .ghost()
                            .small()
                            .icon(Icon::new(CatalogIcon::Add))
                            .rounded(px(14.))
                            .size(px(28.))
                            .accessibility_label("Increase")
                            .disabled(!in_review)
                            .on_click(move |_, window, cx| {
                                if let Some(more) = &more {
                                    more(&more_id, quantity + 1, window, cx);
                                }
                            }),
                    )
            });
            let remove = item.removable.then(|| {
                let remove = self.on_remove.clone();
                let id = item.id.clone();
                Button::new(part(format!("remove-{}", item.id)))
                    .ghost()
                    .small()
                    .icon(Icon::new(CatalogIcon::TrashCan))
                    .accessibility_label(format!("Remove {}", item.label))
                    .disabled(!in_review)
                    .on_click(move |_, window, cx| {
                        if let Some(remove) = &remove {
                            remove(&id, window, cx);
                        }
                    })
            });
            h_flex()
                .gap_3()
                .px_2()
                .py(px(10.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .child(item.label.clone()),
                        )
                        .when_some(item.detail.clone(), |this, detail| {
                            this.child(
                                div()
                                    .truncate()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(detail),
                            )
                        }),
                )
                .children(stepper)
                .child(
                    div()
                        .min_w(px(80.))
                        .flex()
                        .justify_end()
                        .font_family(mono.clone())
                        .child(
                            RollingNumber::new(
                                part(format!("amount-{}", item.id)),
                                money(item.line_total(), &symbol),
                            )
                            .text_size(px(13.))
                            .line_height(px(20.)),
                        ),
                )
                .children(remove)
        });

        let mut sums = vec![("Subtotal".into(), subtotal(&self.items))];
        sums.extend(self.fees.iter().cloned());
        let sums = sums.into_iter().enumerate().map(
            |(ix, (label, amount)): (usize, (SharedString, f64))| {
                h_flex()
                    .justify_between()
                    .text_color(quiet)
                    .child(label)
                    .child(
                        div().font_family(mono.clone()).child(
                            RollingNumber::new(part(format!("sum-{ix}")), money(amount, &symbol))
                                .line_height(px(20.)),
                        ),
                    )
            },
        );

        let warnings = (!self.warnings.is_empty()).then(|| {
            v_flex()
                .gap(px(6.))
                .px_2()
                .pb_2()
                .children(self.warnings.iter().enumerate().map(|(ix, warning)| {
                    inset(cx)
                        .id(part(format!("warning-{ix}")))
                        .flex()
                        .items_start()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .bg(caution.opacity(0.1))
                        .text_body()
                        .text_color(caution)
                        .role(Role::Note)
                        .aria_label(format!("Warning: {warning}"))
                        .child(
                            div()
                                .mt(px(3.))
                                .flex_none()
                                .child(Icon::new(CatalogIcon::WarningFilled).size(px(14.))),
                        )
                        .child(div().min_w_0().flex_1().child(warning.clone()))
                }))
        });

        let cancel = self.on_cancel.clone().map(|cancel| {
            Button::new(part("cancel".into()))
                .ghost()
                .small()
                .label("Cancel")
                .disabled(!in_review)
                .on_click(move |_, window, cx| cancel(window, cx))
        });
        let edit = self.on_edit.clone().map(|edit| {
            Button::new(part("edit".into()))
                .secondary()
                .small()
                .label("Edit")
                .disabled(!in_review)
                .on_click(move |_, window, cx| edit(window, cx))
        });
        let confirm = self.on_confirm.clone();
        let confirm = Button::new(part("confirm".into()))
            .primary()
            .small()
            .loading(self.status == ConfirmationStatus::Confirming)
            .disabled(self.items.is_empty())
            .label(if self.status == ConfirmationStatus::Confirming {
                SharedString::from("Confirming…")
            } else {
                format!("{} · {total}", self.confirm_label).into()
            })
            .on_click(move |_, window, cx| {
                if let Some(confirm) = &confirm {
                    confirm(window, cx);
                }
            });

        frame
            .child(shell_header(
                Some(Icon::new(CatalogIcon::TaskApproved)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                None,
                cx,
            ))
            .child(
                panel(cx)
                    .child(v_flex().px_2().pt_2().children(rows))
                    .child(
                        v_flex()
                            .mx_4()
                            .gap(px(6.))
                            .py_3()
                            .text_body()
                            .children(sums)
                            .child(
                                h_flex()
                                    .items_baseline()
                                    .justify_between()
                                    .pt(px(6.))
                                    .child(div().font_medium().child("Total"))
                                    .child(
                                        div().font_family(mono.clone()).child(
                                            RollingNumber::new(part("total".into()), total)
                                                .text_size(px(18.))
                                                .line_height(px(28.))
                                                .font_medium(),
                                        ),
                                    ),
                            ),
                    )
                    .children(warnings),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap(px(6.))
                    .pt(px(6.))
                    .children(cancel)
                    .children(edit)
                    .child(confirm),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_grouped_with_two_decimals() {
        assert_eq!(money(0., "$"), "$0.00");
        assert_eq!(money(48.5, "$"), "$48.50");
        assert_eq!(money(1234.567, "€"), "€1,234.57");
        assert_eq!(money(1_000_000., "$"), "$1,000,000.00");
        assert_eq!(money(-12.3, "$"), "-$12.30");
        assert_eq!(money(-0.001, "$"), "$0.00");
    }

    #[test]
    fn the_total_is_every_line_plus_fees() {
        let card = ConfirmationCard::new("order", "Order")
            .items(vec![
                ConfirmationItem::new("a", "Oak shelf", 120.).quantity(2),
                ConfirmationItem::new("b", "Brackets", 9.5).quantity(4),
            ])
            .fee("Delivery", 18.)
            .fee("Discount", -10.);
        assert_eq!(subtotal(&card.items), 278.);
        assert_eq!(card.total(), 286.);
        assert_eq!(card.items[1].line_total(), 38.);
    }

    #[test]
    fn a_quantity_is_never_less_than_one() {
        assert_eq!(
            ConfirmationItem::new("a", "Shelf", 1.).quantity(0).quantity,
            1
        );
        assert_eq!(
            ConfirmationCard::new("c", "Order").undo_window(5, 0).undo,
            Some((5, 1))
        );
    }
}
