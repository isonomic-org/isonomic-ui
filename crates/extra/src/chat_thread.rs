// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Chat thread: the turns of a conversation, read from the top down.
//!
//! Turns from the same side within five minutes share one name and avatar.
//! A pill marks each new day, and another marks where the unread turns
//! begin. The thread follows the newest turn until the reader scrolls up;
//! then a button offers the way back and counts what arrived meanwhile. A
//! turn that is still being written ends in a pulsing bar. The time of a
//! turn shows beside its name under the pointer.

use std::time::Duration;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    avatar::Avatar,
    bubble::{Bubble, BubbleContent, BubbleVariant},
    button::{Button, ButtonVariants as _},
    h_flex,
    message::MessageAlignment,
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell};

/// Turns from the same side this close together share one header, in seconds.
const GROUP_WITHIN: i64 = 5 * 60;
/// How close to the end still counts as following the conversation.
const FOLLOW_WITHIN: f32 = 48.;
/// How long a new turn and the jump button take to appear.
const ENTER: Duration = Duration::from_millis(200);
/// One beat of the bar at the end of a turn that is being written.
const PULSE: Duration = Duration::from_millis(1000);
const DAY: i64 = 86_400;
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Which side a turn is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    /// The person reading the thread: turns sit at the far edge in a bubble.
    Person,
    /// The assistant: turns sit at the near edge and take the column's width.
    Assistant,
}

/// How much room the turns get.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Density {
    /// Avatars and generous spacing.
    #[default]
    Comfortable,
    /// No avatars and tighter spacing.
    Dense,
}

/// One turn of the conversation.
pub struct ThreadMessage {
    id: SharedString,
    speaker: Speaker,
    text: SharedString,
    at: i64,
    author: Option<SharedString>,
    streaming: bool,
    rich: Option<AnyElement>,
}

impl ThreadMessage {
    /// A turn of plain text, said at `at` (seconds since the epoch, UTC).
    pub fn new(
        id: impl Into<SharedString>,
        speaker: Speaker,
        text: impl Into<SharedString>,
        at: i64,
    ) -> Self {
        Self {
            id: id.into(),
            speaker,
            text: text.into(),
            at,
            author: None,
            streaming: false,
            rich: None,
        }
    }

    /// The name on the turn, instead of the thread's name for that side.
    pub fn author(mut self, author: impl Into<SharedString>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// The turn is still being written: it ends in a pulsing bar.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Show this element instead of the text: a card, a list, a table.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.rich = Some(content.into_any_element());
        self
    }
}

/// The day a moment falls on where the reader is, counted from the epoch.
pub fn day_number(at: i64, utc_offset_minutes: i32) -> i64 {
    (at + i64::from(utc_offset_minutes) * 60).div_euclid(DAY)
}

/// The month (1 to 12) and day of the month of a day counted from the epoch.
pub fn month_and_day(day_number: i64) -> (u32, u32) {
    // Count from 1 March 0000 so the leap day is the last of its year, then
    // walk down through 400-year eras, years and five-month runs of 153 days.
    let shifted = day_number + 719_468;
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    (month as u32, day as u32)
}

/// What the pill above a day's first turn says: "Today", "Yesterday", "March 3".
pub fn day_label(at: i64, now: i64, utc_offset_minutes: i32) -> String {
    let day = day_number(at, utc_offset_minutes);
    match day_number(now, utc_offset_minutes) - day {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        _ => {
            let (month, date) = month_and_day(day);
            format!("{} {date}", MONTHS[(month - 1) as usize])
        }
    }
}

/// The time of a turn where the reader is: "9:05 AM", or "09:05" on a 24-hour clock.
pub fn time_label(at: i64, utc_offset_minutes: i32, twenty_four_hour: bool) -> String {
    let seconds = (at + i64::from(utc_offset_minutes) * 60).rem_euclid(DAY);
    let (hour, minute) = (seconds / 3600, seconds % 3600 / 60);
    if twenty_four_hour {
        return format!("{hour:02}:{minute:02}");
    }
    let half = if hour < 12 { "AM" } else { "PM" };
    let hour = match hour % 12 {
        0 => 12,
        hour => hour,
    };
    format!("{hour}:{minute:02} {half}")
}

/// What surrounds one turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnPlace {
    /// The first turn of its day: a day pill goes above it.
    pub new_day: bool,
    /// The first unread turn: the unread pill goes above it.
    pub unread: bool,
    /// It continues the turn above: no name, no avatar, no extra space.
    pub grouped: bool,
}

/// Where the day pills, the unread pill and the shared headers fall.
///
/// `turns` holds each turn's side and time in order; `unread_from` is the
/// index of the first unread turn.
pub fn place_turns(
    turns: &[(Speaker, i64)],
    unread_from: Option<usize>,
    utc_offset_minutes: i32,
) -> Vec<TurnPlace> {
    turns
        .iter()
        .enumerate()
        .map(|(ix, (speaker, at))| {
            let before = ix.checked_sub(1).map(|ix| turns[ix]);
            let new_day = before.is_none_or(|(_, earlier)| {
                day_number(earlier, utc_offset_minutes) != day_number(*at, utc_offset_minutes)
            });
            let unread = unread_from == Some(ix);
            let grouped = !new_day
                && !unread
                && before
                    .is_some_and(|(side, earlier)| side == *speaker && at - earlier < GROUP_WITHIN);
            TurnPlace {
                new_day,
                unread,
                grouped,
            }
        })
        .collect()
}

/// What following the conversation looks like from one frame to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Follow {
    /// The view is kept at the end as turns arrive.
    stick: bool,
    /// How many turns there were when the reader scrolled away.
    seen: usize,
}

impl Follow {
    /// Take in this frame's scroll position and turn count.
    fn step(self, at_end: bool, turns: usize) -> Self {
        let seen = if self.stick && !at_end {
            turns
        } else {
            self.seen
        };
        Self {
            stick: at_end,
            seen,
        }
    }

    /// Turns that arrived since the reader scrolled away.
    fn fresh(self, turns: usize) -> usize {
        if self.stick {
            0
        } else {
            turns.saturating_sub(self.seen)
        }
    }
}

/// What the jump button says: "Jump to latest", then the count of new turns.
pub fn jump_label(fresh: usize) -> String {
    if fresh == 0 {
        "Jump to latest".to_string()
    } else {
        format!("Jump to latest · {fresh} new")
    }
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// A conversation's turns.
#[derive(IntoElement)]
pub struct ChatThread {
    id: ElementId,
    style: StyleRefinement,
    messages: Vec<ThreadMessage>,
    unread_from: Option<SharedString>,
    density: Density,
    assistant_name: SharedString,
    person_name: SharedString,
    height: Option<Pixels>,
    bare: bool,
    footer: Option<AnyElement>,
    bottom_inset: Pixels,
    now: i64,
    utc_offset_minutes: i32,
    twenty_four_hour: bool,
    scroll: Option<ScrollHandle>,
}

impl ChatThread {
    /// Create a thread of these turns, oldest first.
    pub fn new(id: impl Into<ElementId>, messages: Vec<ThreadMessage>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            messages,
            unread_from: None,
            density: Density::Comfortable,
            assistant_name: "Assistant".into(),
            person_name: "You".into(),
            height: None,
            bare: false,
            footer: None,
            bottom_inset: px(0.),
            now: 0,
            utc_offset_minutes: 0,
            twenty_four_hour: false,
            scroll: None,
        }
    }

    /// The id of the first unread turn: the unread pill goes above it.
    pub fn unread_from(mut self, id: impl Into<SharedString>) -> Self {
        self.unread_from = Some(id.into());
        self
    }

    /// How much room the turns get.
    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// The name on the assistant's turns. "Assistant" by default.
    pub fn assistant_name(mut self, name: impl Into<SharedString>) -> Self {
        self.assistant_name = name.into();
        self
    }

    /// The name on the person's turns. "You" by default.
    pub fn person_name(mut self, name: impl Into<SharedString>) -> Self {
        self.person_name = name.into();
        self
    }

    /// The height of the scrolling area. 420px by default, the parent's
    /// height when bare.
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// No frame: the thread is only the scrolling area and fills its parent.
    pub fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    /// Shown after the last turn, inside the scrolling area.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Space kept free at the end for a composer that floats over the thread.
    pub fn bottom_inset(mut self, inset: impl Into<Pixels>) -> Self {
        self.bottom_inset = inset.into();
        self
    }

    /// The present moment (seconds since the epoch, UTC) and how far the
    /// reader's clock is from UTC, in minutes. Days and times follow them.
    pub fn now(mut self, now: i64, utc_offset_minutes: i32) -> Self {
        self.now = now;
        self.utc_offset_minutes = utc_offset_minutes;
        self
    }

    /// Show times on a 24-hour clock.
    pub fn twenty_four_hour(mut self, twenty_four_hour: bool) -> Self {
        self.twenty_four_hour = twenty_four_hour;
        self
    }

    /// Scroll with this handle, so the owner can move the thread itself.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }
}

impl Styled for ChatThread {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A centred pill between turns: a day, or where the unread turns begin.
fn divider(id: ElementId, label: String, unread: bool, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let (fill, ink) = if unread {
        (theme.info.opacity(0.15), theme.info)
    } else {
        (theme.foreground.opacity(0.05), theme.muted_foreground)
    };
    h_flex()
        .id(id)
        .test_support()
        .justify_center()
        .my_3()
        .role(Role::Splitter)
        .aria_label(label.clone())
        .child(
            div()
                .rounded_full()
                .px(px(10.))
                .py(px(2.))
                .bg(fill)
                .text_color(ink)
                .text_size(px(11.))
                .line_height(px(16.))
                .font_medium()
                .child(label),
        )
}

impl RenderOnce for ChatThread {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let accent = theme.info;
        let ink = theme.foreground;
        let still = cx.reduce_motion();
        let dense = self.density == Density::Dense;
        let count = self.messages.len();
        let streaming = self.messages.iter().any(|message| message.streaming);

        let own_handle = window
            .use_keyed_state(part("scroll"), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let handle = self.scroll.clone().unwrap_or(own_handle);
        let follow = window.use_keyed_state(part("follow"), cx, |_, _| Follow {
            stick: true,
            seen: 0,
        });
        // Turns that were there on the first frame arrive without movement.
        let first: Vec<SharedString> = self.messages.iter().map(|m| m.id.clone()).collect();
        let settled = window
            .use_keyed_state(part("settled"), cx, |_, _| first)
            .read(cx)
            .clone();

        let scrolled = -f32::from(handle.offset().y);
        let most = f32::from(handle.max_offset().y);
        let at_end = most - scrolled < FOLLOW_WITHIN;
        let before = *follow.read(cx);
        let now = before.step(at_end, count);
        if now != before {
            follow.update(cx, |state, _| *state = now);
        }
        if now.stick {
            handle.scroll_to_bottom();
        }
        let away = !now.stick;
        let fresh = now.fresh(count);

        let stamps: Vec<(Speaker, i64)> = self.messages.iter().map(|m| (m.speaker, m.at)).collect();
        let unread_at = self
            .unread_from
            .as_ref()
            .and_then(|id| self.messages.iter().position(|m| &m.id == id));
        let places = place_turns(&stamps, unread_at, self.utc_offset_minutes);

        let mut rows: Vec<AnyElement> = Vec::new();
        for (ix, (message, place)) in self.messages.into_iter().zip(places).enumerate() {
            let turn = |name: &'static str| {
                ElementId::NamedChild(
                    ElementId::NamedChild(self.id.clone().into(), message.id.clone()).into(),
                    name.into(),
                )
            };
            let person = message.speaker == Speaker::Person;
            let name = message.author.clone().unwrap_or_else(|| {
                if person {
                    self.person_name.clone()
                } else {
                    self.assistant_name.clone()
                }
            });
            let time = time_label(message.at, self.utc_offset_minutes, self.twenty_four_hour);

            if place.new_day {
                rows.push(
                    divider(
                        turn("day"),
                        day_label(message.at, self.now, self.utc_offset_minutes),
                        false,
                        cx,
                    )
                    .into_any_element(),
                );
            }
            if place.unread {
                rows.push(
                    divider(part("unread"), "Unread".to_string(), true, cx).into_any_element(),
                );
            }

            let avatar = (!dense).then(|| {
                if place.grouped {
                    div().w(px(28.)).flex_none().into_any_element()
                } else if person {
                    Avatar::new()
                        .name(name.clone())
                        .with_size(px(28.))
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded_full()
                        .bg(accent.opacity(0.15))
                        .text_color(accent)
                        .child(Icon::new(CatalogIcon::Sparkles).size(px(14.)))
                        .into_any_element()
                }
            });

            let hover = SharedString::from(format!("turn-{}", message.id));
            let header = (!place.grouped).then(|| {
                h_flex()
                    .items_baseline()
                    .gap_2()
                    .when(person, |this| this.flex_row_reverse())
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .child(div().font_medium().child(name.clone()))
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(quiet)
                            .opacity(0.)
                            .group_hover(hover.clone(), |this| this.opacity(1.))
                            .child(time.clone()),
                    )
            });

            let caret = message.streaming.then(|| {
                let bar = div()
                    .ml(px(2.))
                    .w(px(2.))
                    .h(px(15.))
                    .rounded_full()
                    .bg(accent);
                if still {
                    bar.into_any_element()
                } else {
                    bar.with_animation(
                        turn("caret"),
                        Animation::new(PULSE).repeat(),
                        |this, time| this.opacity(0.4 + 0.6 * (1. - (time * 2. - 1.).abs())),
                    )
                    .into_any_element()
                }
            });
            let body = match message.rich {
                Some(rich) => v_flex().w_full().child(rich).children(caret),
                None => {
                    // While a turn is written its last line is laid out word by
                    // word, so the bar can sit right after the newest word.
                    let mut lines: Vec<&str> = message.text.split('\n').collect();
                    let last = caret.as_ref().and_then(|_| lines.pop());
                    v_flex()
                        .w_full()
                        .children(lines.into_iter().map(|line| {
                            div()
                                .min_h(px(22.))
                                .child(SharedString::from(line.to_string()))
                        }))
                        .when_some(last, |this, last| {
                            let mut words: Vec<String> = last
                                .split(' ')
                                .filter(|word| !word.is_empty())
                                .map(|word| format!("{word} "))
                                .collect();
                            if let Some(newest) = words.last_mut() {
                                newest.pop();
                            }
                            this.child(
                                h_flex()
                                    .flex_wrap()
                                    .min_h(px(22.))
                                    .children(words.into_iter().map(|word| {
                                        div().max_w_full().whitespace_nowrap().child(word)
                                    }))
                                    .children(caret),
                            )
                        })
                }
            };
            let content = if person {
                Bubble::new()
                    .with_variant(BubbleVariant::Secondary)
                    .alignment(MessageAlignment::End)
                    .max_w_full()
                    .content(
                        BubbleContent::new()
                            .rounded(px(16.))
                            .when(!place.grouped, |this| this.rounded_tr(px(10.)))
                            .px(px(14.))
                            .py_2()
                            .text_size(px(14.))
                            .line_height(px(22.))
                            .text_color(ink)
                            .child(body),
                    )
            } else {
                Bubble::new().with_variant(BubbleVariant::Ghost).content(
                    BubbleContent::new()
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .child(body),
                )
            };

            let article = h_flex()
                .id(turn("turn"))
                .test_support()
                .group(hover)
                .relative()
                .items_start()
                .gap(px(10.))
                .when(person, |this| this.flex_row_reverse())
                .when(!place.grouped && ix > 0, |this| {
                    this.mt(if dense { px(8.) } else { px(16.) })
                })
                .role(Role::Group)
                .aria_label(format!("{name}, {time}"))
                .children(avatar)
                .child(
                    v_flex()
                        .min_w_0()
                        .gap_1()
                        .map(|this| {
                            if person {
                                this.items_end().max_w(relative(0.8))
                            } else {
                                this.flex_1().max_w(relative(0.92))
                            }
                        })
                        .children(header)
                        .child(content),
                );
            if still || settled.contains(&message.id) {
                rows.push(article.into_any_element());
            } else {
                rows.push(
                    article
                        .with_animation(turn("enter"), Animation::new(ENTER), |this, time| {
                            let shown = ease_out(time);
                            this.opacity(shown).top(px(6. * (1. - shown)))
                        })
                        .into_any_element(),
                );
            }
        }

        let jump_handle = handle.clone();
        let jump_follow = follow.clone();
        let jump = away.then(|| {
            let button = h_flex()
                .absolute()
                .left_0()
                .right_0()
                .bottom(self.bottom_inset + px(12.))
                .justify_center()
                .child(
                    Button::new(part("jump"))
                        .secondary()
                        .small()
                        .icon(IconName::ArrowDown)
                        .label(jump_label(fresh))
                        .h(px(32.))
                        .px(px(14.))
                        .rounded_full()
                        .text_size(px(13.))
                        .on_click(move |_, window, cx| {
                            jump_handle.scroll_to_bottom();
                            jump_follow.update(cx, |state, _| state.stick = true);
                            window.refresh();
                        }),
                );
            if still {
                button.into_any_element()
            } else {
                let rest = f32::from(self.bottom_inset) + 12.;
                button
                    .with_animation(part("jump-in"), Animation::new(ENTER), move |this, time| {
                        let shown = ease_out(time);
                        this.opacity(shown).bottom(px(rest - 8. * (1. - shown)))
                    })
                    .into_any_element()
            }
        });

        let height = self.height;
        let log = v_flex()
            .id(part("log"))
            .test_support()
            .map(|this| match height {
                Some(height) => this.h(height),
                None if self.bare => this.h_full(),
                None => this.h(px(420.)),
            })
            .overflow_y_scroll()
            .track_scroll(&handle)
            .on_scroll(|_, window, _| window.refresh())
            .tab_index(0)
            .role(Role::Log)
            .aria_label(if streaming {
                "Conversation, a reply is being written"
            } else {
                "Conversation"
            })
            .child(
                v_flex()
                    .map(|this| {
                        if dense {
                            this.gap_1().p_3()
                        } else {
                            this.gap(px(6.)).px_4().py_5()
                        }
                    })
                    .when(self.bottom_inset > px(0.), |this| {
                        this.pb(self.bottom_inset + px(20.))
                    })
                    .children(rows)
                    .children(self.footer),
            );

        if self.bare {
            div()
                .id(self.id.clone())
                .relative()
                .size_full()
                .child(log)
                .children(jump)
                .refine_style(&self.style)
        } else {
            shell(cx)
                .id(self.id.clone())
                .max_w(px(672.))
                .child(
                    panel(cx)
                        .relative()
                        .overflow_hidden()
                        .child(log)
                        .children(jump),
                )
                .refine_style(&self.style)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 12 March 2026, 09:00 UTC.
    const MORNING: i64 = 1_773_306_000;

    #[test]
    fn days_are_named_from_where_the_reader_is() {
        assert_eq!(month_and_day(0), (1, 1));
        assert_eq!(month_and_day(day_number(MORNING, 0)), (3, 12));
        // 29 February 2024, and the day after.
        assert_eq!(month_and_day(19_782), (2, 29));
        assert_eq!(month_and_day(19_783), (3, 1));
        assert_eq!(day_label(MORNING, MORNING + 3600, 0), "Today");
        assert_eq!(day_label(MORNING - DAY, MORNING, 0), "Yesterday");
        assert_eq!(day_label(MORNING - 9 * DAY, MORNING, 0), "March 3");
        // Ten at night in UTC is already tomorrow eight hours to the east.
        let late = MORNING + 13 * 3600;
        assert_eq!(day_label(MORNING, late, 480), "Yesterday");
        assert_eq!(day_label(MORNING, late, 0), "Today");
    }

    #[test]
    fn times_read_on_either_clock() {
        assert_eq!(time_label(MORNING, 0, false), "9:00 AM");
        assert_eq!(time_label(MORNING + 5 * 60, 0, true), "09:05");
        assert_eq!(time_label(MORNING, 330, false), "2:30 PM");
        assert_eq!(time_label(MORNING - 9 * 3600, 0, false), "12:00 AM");
        assert_eq!(time_label(MORNING + 3 * 3600, 0, false), "12:00 PM");
    }

    #[test]
    fn turns_share_a_header_within_five_minutes_on_one_side() {
        let turns = [
            (Speaker::Person, MORNING),
            (Speaker::Person, MORNING + 60),
            (Speaker::Assistant, MORNING + 90),
            (Speaker::Assistant, MORNING + 120),
            (Speaker::Assistant, MORNING + 120 + GROUP_WITHIN),
        ];
        let places = place_turns(&turns, None, 0);
        let grouped: Vec<bool> = places.iter().map(|place| place.grouped).collect();
        assert_eq!(grouped, [false, true, false, true, false]);
        assert!(places[0].new_day);
        assert!(places[1..].iter().all(|place| !place.new_day));
    }

    #[test]
    fn a_new_day_and_the_unread_pill_both_start_a_group() {
        let turns = [
            (Speaker::Assistant, MORNING - 60),
            (Speaker::Assistant, MORNING),
            (Speaker::Assistant, MORNING + 30),
        ];
        // The second turn falls on the next day nine hours to the west of UTC.
        let places = place_turns(&turns, Some(2), -540);
        assert!(places[1].new_day && !places[1].grouped);
        assert!(places[2].unread && !places[2].grouped && !places[2].new_day);
        assert_eq!(place_turns(&[], None, 0), []);
    }

    #[test]
    fn the_thread_follows_until_the_reader_scrolls_away() {
        let start = Follow {
            stick: true,
            seen: 0,
        };
        let following = start.step(true, 6);
        assert_eq!(following.fresh(6), 0);
        let away = following.step(false, 6);
        assert!(!away.stick && away.fresh(6) == 0);
        let more = away.step(false, 8);
        assert_eq!(more.fresh(8), 2);
        assert_eq!(jump_label(more.fresh(8)), "Jump to latest · 2 new");
        assert_eq!(jump_label(0), "Jump to latest");
        assert_eq!(more.step(true, 8).fresh(8), 0);
    }
}
