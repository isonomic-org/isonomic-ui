use chrono::NaiveDate;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, Size, StyledExt,
    date_field::{DateField, DateFieldEvent, DateFieldState, DateOrder},
    v_flex,
};
use isonomic_ui::*;

use crate::{ChangeStorySize, section, story_toolbar};

pub struct DateFieldStory {
    year_first: Entity<DateFieldState>,
    month_first: Entity<DateFieldState>,
    day_first: Entity<DateFieldState>,
    ranged: Entity<DateFieldState>,
    disabled: Entity<DateFieldState>,
    value: NaiveDate,
    ranged_value: NaiveDate,
    size: Size,
    _subscriptions: Vec<Subscription>,
}

impl super::Story for DateFieldStory {
    fn title() -> &'static str {
        "DateField"
    }

    fn description() -> &'static str {
        "Type a date segment by segment, in year, month or day order."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl DateFieldStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let value = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap();
        let field = |order: DateOrder, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut state = DateFieldState::new(window, cx).order(order);
                state.set_date(value, window, cx);
                state
            })
        };
        let year_first = field(DateOrder::YearMonthDay, window, cx);
        let month_first = field(DateOrder::MonthDayYear, window, cx);
        let day_first = field(DateOrder::DayMonthYear, window, cx);
        let disabled = field(DateOrder::YearMonthDay, window, cx);
        let ranged_value = NaiveDate::from_ymd_opt(2027, 1, 4).unwrap();
        let ranged = cx.new(|cx| {
            let mut state = DateFieldState::new(window, cx)
                .min_date(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
                .max_date(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
            state.set_date(ranged_value, window, cx);
            state
        });

        let _subscriptions = vec![
            cx.subscribe(&year_first, |this, _, event, cx| match event {
                DateFieldEvent::Change(date) => {
                    this.value = *date;
                    cx.notify();
                }
            }),
            cx.subscribe(&ranged, |this, _, event, cx| match event {
                DateFieldEvent::Change(date) => {
                    this.ranged_value = *date;
                    cx.notify();
                }
            }),
        ];

        Self {
            year_first,
            month_first,
            day_first,
            ranged,
            disabled,
            value,
            ranged_value,
            size: Size::Medium,
            _subscriptions,
        }
    }
}

impl Focusable for DateFieldStory {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.year_first.focus_handle(cx)
    }
}

impl Render for DateFieldStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let in_range = self.ranged.read(cx).is_in_range();
        let note = |text: String, cx: &App| {
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(text)
        };

        v_flex()
            .gap_3()
            .on_action(cx.listener(|this, action: &ChangeStorySize, _, cx| {
                this.size = action.0;
                cx.notify();
            }))
            .child(story_toolbar(self.size))
            .child(
                section("Default")
                    .description(
                        "Up/Down change the selected segment; digits type it and move to the next.",
                    )
                    .v_flex()
                    .gap_3()
                    .child(
                        DateField::new(&self.year_first)
                            .aria_label("Start date")
                            .with_size(self.size),
                    )
                    .child(note(format!("Value: {}", self.value), cx)),
            )
            .child(
                section("Month first")
                    .description("Set the order to match how the reader writes dates.")
                    .child(
                        DateField::new(&self.month_first)
                            .aria_label("Due date")
                            .with_size(self.size),
                    ),
            )
            .child(
                section("Day first")
                    .description("Any separator can stand between the segments.")
                    .child(
                        DateField::new(&self.day_first)
                            .aria_label("Date of birth")
                            .separator(".")
                            .with_size(self.size),
                    ),
            )
            .child(
                section("Within a range")
                    .description(
                        "A date outside the range shows as invalid; typing is never blocked.",
                    )
                    .v_flex()
                    .gap_3()
                    .child(
                        DateField::new(&self.ranged)
                            .aria_label("Report date")
                            .with_size(self.size),
                    )
                    .child(note(
                        if in_range {
                            format!("{} is within 2026.", self.ranged_value)
                        } else {
                            format!("{} is outside 2026.", self.ranged_value)
                        },
                        cx,
                    )),
            )
            .child(
                section("Disabled").child(
                    DateField::new(&self.disabled)
                        .aria_label("Start date")
                        .with_size(self.size)
                        .disabled(true),
                ),
            )
    }
}
