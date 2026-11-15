//! Gallery story for the amount stepper.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::amount_stepper::{AmountEvent, AmountStepper, format_amount};

/// An invoice line: hours at a rate, with the total worked out as either changes.
pub struct AmountStepperStory {
    hours: Entity<AmountStepper>,
    rate: Entity<AmountStepper>,
    seats: Entity<AmountStepper>,
    _events: Vec<Subscription>,
}

impl AmountStepperStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let hours = cx.new(|cx| {
                AmountStepper::new("Hours", 6.5, window, cx).range(0.0, 80.0, window, cx).step(0.25).suffix_unit("hrs")
            });
            let rate = cx.new(|cx| AmountStepper::new("Hourly rate", 145.0, window, cx).range(0.0, 10_000.0, window, cx).step(5.0).prefix_unit("$"));
            let seats = cx.new(|cx| {
                AmountStepper::new("Seats", 3.0, window, cx).range(1.0, 25.0, window, cx).decimals(0, window, cx)
            });
            let events = vec![
                cx.subscribe(&hours, |_, _, _: &AmountEvent, cx| cx.notify()),
                cx.subscribe(&rate, |_, _, _: &AmountEvent, cx| cx.notify()),
                cx.subscribe(&seats, |_, _, _: &AmountEvent, cx| cx.notify()),
            ];
            Self { hours, rate, seats, _events: events }
        })
    }
}

impl Story for AmountStepperStory {
    fn title() -> &'static str {
        "Amount stepper"
    }

    fn description() -> &'static str {
        "A number field for money, hours or quantities that reads what people paste, tidies it on leaving, and steps."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AmountStepperStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let total = self.hours.read(cx).value() * self.rate.read(cx).value();
        let row = |label: &'static str| div().w(px(120.)).text_sm().child(label);
        v_flex()
            .gap_3()
            .child(h_flex().gap_3().child(row("Hours")).child(self.hours.clone()))
            .child(h_flex().gap_3().child(row("Rate")).child(self.rate.clone()))
            .child(h_flex().gap_3().child(row("Line total")).child(div().text_sm().font_weight(isonomic_ui::FontWeight::MEDIUM).child(format!("${}", format_amount(total, 2)))))
            .child(h_flex().gap_3().child(row("Seats")).child(self.seats.clone()))
            .child(div().text_xs().text_color(quiet).child("Try pasting \"$1,250\" into the rate, or Up and Down with Shift."))
    }
}
