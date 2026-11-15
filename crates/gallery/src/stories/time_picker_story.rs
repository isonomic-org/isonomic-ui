use chrono::NaiveTime;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, Size, StyledExt,
    time_field::{HourCycle, TimePrecision},
    time_picker::{TimePicker, TimePickerEvent, TimePickerState},
    v_flex,
};
use isonomic_ui::*;

use crate::{ChangeStorySize, section, story_toolbar};

pub struct TimePickerStory {
    default: Entity<TimePickerState>,
    twelve_hour: Entity<TimePickerState>,
    seconds: Entity<TimePickerState>,
    disabled: Entity<TimePickerState>,
    invalid: Entity<TimePickerState>,
    value: NaiveTime,
    size: Size,
    _subscriptions: Vec<Subscription>,
}

impl super::Story for TimePickerStory {
    fn title() -> &'static str {
        "TimePicker"
    }

    fn description() -> &'static str {
        "Type a time, or pick hours and minutes from columns."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl TimePickerStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let value = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        let picker = |build: fn(TimePickerState) -> TimePickerState,
                      window: &mut Window,
                      cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut state = build(TimePickerState::new(window, cx));
                state.set_time(value, window, cx);
                state
            })
        };
        let default = picker(|state| state, window, cx);
        let twelve_hour = picker(
            |state| state.hour_cycle(HourCycle::H12).minute_step(15),
            window,
            cx,
        );
        let seconds = picker(
            |state| state.precision(TimePrecision::Second).second_step(5),
            window,
            cx,
        );
        let disabled = picker(|state| state, window, cx);
        let invalid = picker(|state| state, window, cx);

        let _subscriptions = vec![cx.subscribe(&default, |this, _, event, cx| match event {
            TimePickerEvent::Change(time) => {
                this.value = *time;
                cx.notify();
            }
        })];

        Self {
            default,
            twelve_hour,
            seconds,
            disabled,
            invalid,
            value,
            size: Size::Medium,
            _subscriptions,
        }
    }
}

impl Focusable for TimePickerStory {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.default.focus_handle(cx)
    }
}

impl Render for TimePickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                        "Type in the field, or open the columns with the clock button or Alt-Down.",
                    )
                    .v_flex()
                    .gap_3()
                    .child(TimePicker::new(&self.default).with_size(self.size))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Value: {}", self.value)),
                    ),
            )
            .child(
                section("12-hour clock")
                    .description(
                        "An AM/PM column follows; minutes are listed every 15, and any minute can still be typed.",
                    )
                    .child(TimePicker::new(&self.twelve_hour).with_size(self.size)),
            )
            .child(
                section("With seconds")
                    .description("Set the precision to pick seconds as well.")
                    .child(TimePicker::new(&self.seconds).with_size(self.size)),
            )
            .child(
                section("Disabled").child(
                    TimePicker::new(&self.disabled)
                        .with_size(self.size)
                        .disabled(true),
                ),
            )
            .child(
                section("Invalid")
                    .description("Show a validation result from the owner.")
                    .child(
                        TimePicker::new(&self.invalid)
                            .with_size(self.size)
                            .invalid(true),
                    ),
            )
    }
}
