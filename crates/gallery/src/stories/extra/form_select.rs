//! Gallery story for the form select.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, Sizable as _, button::{Button, ButtonVariants as _}, v_flex};
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, div, px};
use isonomic_ui_extra::form_select::{FormSelect, SelectGroup, SelectOption};

/// Coding a bank transaction to an account, grouped by type, with a
/// required check on save.
pub struct FormSelectStory {
    account: Option<SharedString>,
    tax: Option<SharedString>,
    error: Option<SharedString>,
    saved: bool,
}

impl FormSelectStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { account: None, tax: Some("standard".into()), error: None, saved: false })
    }
}

fn accounts() -> Vec<SelectGroup> {
    vec![
        SelectGroup::new("Income", vec![SelectOption::new("4000", "Sales"), SelectOption::new("4100", "Interest earned")]),
        SelectGroup::new(
            "Expenses",
            vec![
                SelectOption::new("6100", "Fuel and travel"),
                SelectOption::new("6200", "Software"),
                SelectOption::new("6300", "Meals"),
                SelectOption::new("6900", "Miscellaneous (closed)").disabled(),
            ],
        ),
        SelectGroup::new("Transfers", vec![SelectOption::new("1100", "Savings account")]),
    ]
}

impl Story for FormSelectStory {
    fn title() -> &'static str {
        "Form select"
    }

    fn description() -> &'static str {
        "A form field that picks one value from grouped options, with a placeholder, required mark, error line and clear."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FormSelectStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let success = cx.theme().success;
        let account = cx.entity();
        let tax = cx.entity();
        v_flex()
            .w(px(320.))
            .gap_4()
            .child(div().text_sm().child("Shell #4412 \u{b7} $86.40 \u{b7} 12 Nov"))
            .child(
                FormSelect::new("account", "Account", accounts())
                    .placeholder("Pick an account")
                    .required()
                    .clearable()
                    .value(self.account.clone())
                    .error(self.error.clone())
                    .on_pick(move |value, _, cx| {
                        account.update(cx, |this, cx| {
                            this.account = value;
                            this.error = None;
                            this.saved = false;
                            cx.notify();
                        })
                    }),
            )
            .child(
                FormSelect::new(
                    "tax",
                    "Tax rate",
                    vec![SelectGroup::new(
                        "",
                        vec![SelectOption::new("standard", "Standard 20%"), SelectOption::new("reduced", "Reduced 5%"), SelectOption::new("none", "No tax")],
                    )],
                )
                .value(self.tax.clone())
                .on_pick(move |value, _, cx| {
                    tax.update(cx, |this, cx| {
                        this.tax = value;
                        cx.notify();
                    })
                }),
            )
            .child(Button::new("code-it").small().primary().label("Save coding").on_click(cx.listener(|this, _, _, cx| {
                if this.account.is_none() {
                    this.error = Some("Pick an account to code this to.".into());
                } else {
                    this.saved = true;
                }
                cx.notify();
            })))
            .children(self.saved.then(|| div().text_sm().text_color(success).child("Coded.")))
    }
}
