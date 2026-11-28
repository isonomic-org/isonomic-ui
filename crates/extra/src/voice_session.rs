// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Voice session: a live spoken conversation, with a meter per speaker and a
//! transcript that writes itself a word at a time.
//!
//! Each speaker has a row of level bars that rise with their voice and fall
//! back to dots in silence. Hand the session a [`SpeechState`] with
//! [`VoiceSession::listen`] and the first speaker (you) is the microphone: its
//! bars become the speech module's own waveform and what the recognizer hears
//! so far appears as your turn while you talk. The transcript shows each turn
//! up to the words that have arrived, each new word fading in. When you talk
//! over the other speaker the session shows the interruption: a notice, your
//! row marked as talking over, and their turn cut where it stopped. Mute and
//! End sit at the foot; both are buttons, so Tab and Enter reach them. Under
//! reduced motion words appear without the fade.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    speech::{SpeechState, SpeechWaveform},
    tag::Tag,
};
use isonomic_ui::{
    App, ElementId, Entity, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    PathBuilder, RenderOnce, Role, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// How many level bars a speaker's meter shows.
const BARS: usize = 28;
/// The height of a speaker's meter.
const METER_HEIGHT: f32 = 24.;
/// How long one word takes to fade in.
const WORD_FADE: Duration = Duration::from_millis(180);
/// The tallest the transcript grows before it scrolls.
const TRANSCRIPT_HEIGHT: f32 = 220.;

/// One person or agent in the session.
#[derive(Clone)]
pub struct VoiceSpeaker {
    name: SharedString,
    levels: Vec<f32>,
    speaking: bool,
    color: Option<Hsla>,
}

impl VoiceSpeaker {
    /// A speaker, silent until given levels.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            levels: Vec::new(),
            speaking: false,
            color: None,
        }
    }

    /// Recent loudness readings between 0 and 1, oldest first.
    pub fn levels(mut self, levels: impl IntoIterator<Item = f32>) -> Self {
        self.levels = levels
            .into_iter()
            .map(|level| level.clamp(0., 1.))
            .collect();
        self
    }

    /// Whether the speaker is talking now.
    pub fn speaking(mut self, speaking: bool) -> Self {
        self.speaking = speaking;
        self
    }

    /// Their colour. By default you are the ink and the others the data
    /// colours.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// One turn of the conversation.
#[derive(Clone)]
pub struct VoiceTurn {
    speaker: usize,
    text: SharedString,
    arrived: Option<usize>,
    cut: bool,
}

impl VoiceTurn {
    /// What speaker `speaker` (an index into the speakers) said.
    pub fn new(speaker: usize, text: impl Into<SharedString>) -> Self {
        Self {
            speaker,
            text: text.into(),
            arrived: None,
            cut: false,
        }
    }

    /// How many of its words have arrived so far. All of them by default.
    pub fn arrived(mut self, words: usize) -> Self {
        self.arrived = Some(words);
        self
    }

    /// The turn was cut off by an interruption where its words stop.
    pub fn cut(mut self) -> Self {
        self.cut = true;
        self
    }
}

/// Where the session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceStatus {
    /// Joining.
    Connecting,
    /// Under way.
    #[default]
    Live,
    /// Hung up.
    Ended,
}

/// The words of `text` that have arrived: the first `arrived`, or all.
pub fn visible_words(text: &str, arrived: Option<usize>) -> Vec<&str> {
    let words = text.split_whitespace();
    match arrived {
        Some(count) => words.take(count).collect(),
        None => words.collect(),
    }
}

/// How opaque word `ix` is while `shown` words have faded in (fractional
/// while a word is fading).
pub fn word_opacity(shown: f32, ix: usize) -> f32 {
    (shown - ix as f32).clamp(0., 1.)
}

/// A call's length: "0:42", "12:05", "1:02:05".
pub fn call_time(time: Duration) -> String {
    let seconds = time.as_secs();
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// The heights of a meter's bars, oldest first, between `floor` and 1: the
/// newest `count` levels, with silence before the first reading.
pub fn bar_heights(levels: &[f32], count: usize, floor: f32) -> Vec<f32> {
    let start = levels.len().saturating_sub(count);
    let recent = &levels[start..];
    std::iter::repeat_n(0., count - recent.len())
        .chain(recent.iter().copied())
        .map(|level| level.clamp(0., 1.).max(floor))
        .collect()
}

/// What a speaker's row says, in words.
pub fn spoken_speaker(name: &str, speaking: bool, muted: bool, talking_over: bool) -> String {
    let state = if muted {
        "muted"
    } else if talking_over {
        "talking over"
    } else if speaking {
        "speaking"
    } else {
        "quiet"
    };
    format!("{name}, {state}")
}

/// What one turn says, in words.
pub fn spoken_turn(name: &str, words: &[&str], whole: bool, cut: bool) -> String {
    let mut line = format!("{name}: {}", words.join(" "));
    if cut {
        line.push_str(" (cut off)");
    } else if !whole {
        line.push_str(" …");
    }
    line
}

type MuteHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type EndHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A live voice conversation.
#[derive(IntoElement)]
pub struct VoiceSession {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    speakers: Vec<VoiceSpeaker>,
    turns: Vec<VoiceTurn>,
    speech: Option<Entity<SpeechState>>,
    status: VoiceStatus,
    elapsed: Duration,
    muted: bool,
    interrupted: bool,
    on_mute: Option<MuteHandler>,
    on_end: Option<EndHandler>,
}

impl VoiceSession {
    /// Create a session. Add yourself as the first speaker.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            speakers: Vec::new(),
            turns: Vec::new(),
            speech: None,
            status: VoiceStatus::Live,
            elapsed: Duration::ZERO,
            muted: false,
            interrupted: false,
            on_mute: None,
            on_end: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a speaker. The first is you.
    pub fn speaker(mut self, speaker: VoiceSpeaker) -> Self {
        self.speakers.push(speaker);
        self
    }

    /// Add a finished or arriving turn.
    pub fn turn(mut self, turn: VoiceTurn) -> Self {
        self.turns.push(turn);
        self
    }

    /// Add several turns.
    pub fn turns(mut self, turns: impl IntoIterator<Item = VoiceTurn>) -> Self {
        self.turns.extend(turns);
        self
    }

    /// Take your meter and live words from a speech input: while it
    /// captures, your bars are its waveform and its transcript so far is
    /// your turn.
    pub fn listen(mut self, speech: &Entity<SpeechState>) -> Self {
        self.speech = Some(speech.clone());
        self
    }

    /// Where the session is. Live by default.
    pub fn status(mut self, status: VoiceStatus) -> Self {
        self.status = status;
        self
    }

    /// How long the session has run.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = elapsed;
        self
    }

    /// Whether your microphone is muted.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Whether you are talking over another speaker.
    pub fn interrupted(mut self, interrupted: bool) -> Self {
        self.interrupted = interrupted;
        self
    }

    /// Run with the new muted state when Mute or Unmute is pressed.
    pub fn on_mute(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_mute = Some(Rc::new(handler));
        self
    }

    /// Run when End is pressed.
    pub fn on_end(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_end = Some(Rc::new(handler));
        self
    }

    /// The speaker you are talking over: the other one speaking, else the
    /// first other.
    fn talked_over(&self) -> Option<usize> {
        if !self.interrupted {
            return None;
        }
        (1..self.speakers.len())
            .find(|ix| self.speakers[*ix].speaking)
            .or((self.speakers.len() > 1).then_some(1))
    }

    /// What a screen reader hears for the session as a whole.
    fn spoken(&self) -> String {
        let others: Vec<&str> = self
            .speakers
            .iter()
            .skip(1)
            .map(|speaker| speaker.name.as_ref())
            .collect();
        let mut spoken = if others.is_empty() {
            format!("{}: ", self.title)
        } else {
            format!("{} with {}: ", self.title, others.join(" and "))
        };
        spoken.push_str(match self.status {
            VoiceStatus::Connecting => "connecting",
            VoiceStatus::Live => "live",
            VoiceStatus::Ended => "ended",
        });
        if self.status != VoiceStatus::Connecting {
            spoken.push_str(&format!(", {}", call_time(self.elapsed)));
        }
        if self.status == VoiceStatus::Live {
            match self.talked_over() {
                Some(ix) => spoken.push_str(&format!(
                    ", you are talking over {}",
                    self.speakers[ix].name
                )),
                None => {
                    if let Some(speaker) = self.speakers.iter().skip(1).find(|one| one.speaking) {
                        spoken.push_str(&format!(", {} speaking", speaker.name));
                    }
                }
            }
            spoken.push_str(if self.muted {
                ", microphone muted"
            } else {
                ", microphone on"
            });
        }
        spoken
    }
}

impl Styled for VoiceSession {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A meter of level bars, drawn as rounded bars from the middle line.
fn meter(heights: Vec<f32>, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let count = heights.len().max(1) as f32;
            let pitch = f32::from(bounds.size.width) / count;
            let bar = (pitch * 0.45).clamp(2., 4.);
            let middle = f32::from(bounds.size.height) / 2.;
            for (ix, height) in heights.iter().enumerate() {
                let half = (height * middle).max(bar / 2.);
                let x = f32::from(bounds.left()) + pitch * (ix as f32 + 0.5);
                let top = f32::from(bounds.top()) + middle - half + bar / 2.;
                let bottom = f32::from(bounds.top()) + middle + half - bar / 2.;
                let mut path = PathBuilder::stroke(px(bar));
                path.move_to(point(px(x), px(top)));
                path.line_to(point(px(x), px(bottom.max(top + 0.01))));
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
                // Round the ends with a dot at each.
                for y in [top, bottom] {
                    let ring: Vec<_> = (0..10)
                        .map(|step| {
                            let angle = step as f32 / 10. * std::f32::consts::TAU;
                            point(
                                px(x + bar / 2. * angle.cos()),
                                px(y + bar / 2. * angle.sin()),
                            )
                        })
                        .collect();
                    let mut dot = PathBuilder::fill();
                    dot.add_polygon(&ring, true);
                    if let Ok(dot) = dot.build() {
                        window.paint_path(dot, color);
                    }
                }
            }
        },
    )
    .w_full()
    .h(px(METER_HEIGHT))
}

impl RenderOnce for VoiceSession {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let numbered = |name: &'static str, ix: usize| {
            ElementId::NamedChild(self.id.clone().into(), format!("{name}-{ix}").into())
        };
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let warning = theme.warning;
        let palette = [theme.chart_1, theme.chart_2, theme.chart_5, theme.chart_4];
        let colors: Vec<Hsla> = self
            .speakers
            .iter()
            .enumerate()
            .map(|(ix, speaker)| {
                speaker.color.unwrap_or(if ix == 0 {
                    ink
                } else {
                    palette[(ix - 1) % palette.len()]
                })
            })
            .collect();
        let live = self.status == VoiceStatus::Live;
        let talked_over = self.talked_over().filter(|_| live);

        // What the speech input hears, when there is one.
        let (capturing, heard) = match &self.speech {
            Some(speech) => {
                let state = speech.read(cx);
                let capturing = state.status().is_capturing() && !self.muted && live;
                let heard = state.transcript();
                (
                    capturing,
                    (state.status().is_active() && !heard.trim().is_empty()).then_some(heard),
                )
            }
            None => (false, None),
        };

        let header_tag = match self.status {
            VoiceStatus::Connecting => Tag::secondary().small().child("Connecting"),
            VoiceStatus::Live => Tag::success()
                .small()
                .child(format!("Live · {}", call_time(self.elapsed))),
            VoiceStatus::Ended => Tag::secondary()
                .small()
                .child(format!("Ended · {}", call_time(self.elapsed))),
        };

        let notice = talked_over.map(|ix| {
            h_flex()
                .id(part("notice"))
                .role(Role::Status)
                .aria_label(format!(
                    "You are talking over {}. They have stopped to listen.",
                    self.speakers[ix].name
                ))
                .gap_2()
                .px_3()
                .py_2()
                .rounded(px(10.))
                .bg(warning.opacity(0.12))
                .text_color(warning)
                .child(Icon::new(CatalogIcon::VoiceActivate).size(px(16.)))
                .child(div().text_caption().font_medium().child(format!(
                    "You're talking over {}. They stopped to listen.",
                    self.speakers[ix].name
                )))
        });

        let rows = self.speakers.iter().enumerate().map(|(ix, speaker)| {
            let you = ix == 0;
            let muted = you && self.muted;
            let over = you && talked_over.is_some();
            let cut_off = talked_over == Some(ix);
            let active =
                live && !muted && !cut_off && (speaker.speaking || over || (you && capturing));
            let color = if active {
                colors[ix]
            } else {
                ink.opacity(0.22)
            };
            let state: Option<(SharedString, Hsla)> = if muted {
                Some(("Muted".into(), quiet))
            } else if over {
                Some(("Talking over".into(), warning))
            } else if cut_off {
                Some(("Stopped".into(), warning))
            } else if active {
                Some(("Speaking".into(), colors[ix]))
            } else {
                None
            };
            let bars = match (&self.speech, you && capturing) {
                (Some(speech), true) => SpeechWaveform::new(speech)
                    .w_full()
                    .h(px(METER_HEIGHT))
                    .into_any_element(),
                _ => {
                    let floor = 0.;
                    let heights = if muted || !live {
                        vec![0.; BARS]
                    } else {
                        bar_heights(&speaker.levels, BARS, floor)
                    };
                    meter(heights, color).into_any_element()
                }
            };
            h_flex()
                .id(numbered("speaker", ix))
                .role(Role::Group)
                .aria_label(spoken_speaker(&speaker.name, active, muted, over))
                .gap_3()
                .child(
                    h_flex()
                        .w(px(112.))
                        .flex_none()
                        .gap_2()
                        .child(div().flex_none().size(px(8.)).rounded_full().bg(colors[ix]))
                        .child(
                            div()
                                .truncate()
                                .text_label_small()
                                .child(speaker.name.clone()),
                        ),
                )
                .child(div().flex_1().min_w_0().child(bars))
                .child(
                    div()
                        .w(px(84.))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .text_caption()
                        .when_some(state, |this, (word, color)| {
                            this.text_color(color).child(word)
                        }),
                )
        });

        // The transcript: every turn, then what you are saying now.
        let mut turns = self.turns.clone();
        if let Some(heard) = heard.filter(|_| !self.speakers.is_empty()) {
            turns.push(VoiceTurn::new(0, heard));
        }
        let total_words: usize = turns
            .iter()
            .map(|turn| visible_words(&turn.text, turn.arrived).len())
            .sum();
        let handle = window
            .use_keyed_state(part("scroll"), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let seen = window.use_keyed_state(part("seen"), cx, |_, _| 0usize);
        if *seen.read(cx) != total_words {
            seen.update(cx, |state, _| *state = total_words);
            handle.scroll_to_bottom();
        }

        let mut lines = Vec::with_capacity(turns.len());
        for (ix, turn) in turns.iter().enumerate() {
            let Some(speaker) = self.speakers.get(turn.speaker) else {
                continue;
            };
            let words = visible_words(&turn.text, turn.arrived);
            let whole = words.len() == visible_words(&turn.text, None).len();
            let shown = transition(
                numbered("words", ix),
                words.len() as f32,
                Transition::new(WORD_FADE),
                window,
                cx,
            );
            let arriving = !whole && !turn.cut && live;
            lines.push(
                div()
                    .id(numbered("turn", ix))
                    .role(Role::Group)
                    .aria_label(spoken_turn(&speaker.name, &words, whole, turn.cut))
                    .v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_eyebrow()
                            .text_color(quiet)
                            .child(speaker.name.to_uppercase()),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_baseline()
                            .gap_x(px(4.))
                            .text_body()
                            .when(turn.cut, |this| this.text_color(quiet))
                            .children(words.iter().enumerate().map(|(word_ix, word)| {
                                div()
                                    .opacity(word_opacity(shown, word_ix))
                                    .child(word.to_string())
                            }))
                            .when(arriving, |this| {
                                this.child(
                                    div().size(px(6.)).rounded_full().bg(colors[turn.speaker]),
                                )
                            })
                            .when(turn.cut, |this| {
                                this.child(
                                    div()
                                        .text_caption()
                                        .text_color(warning)
                                        .child("\u{2014} cut off"),
                                )
                            }),
                    ),
            );
        }
        let transcript = div()
            .id(part("transcript"))
            .test_support()
            .role(Role::Log)
            .aria_label("Transcript")
            .max_h(px(TRANSCRIPT_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&handle)
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .when(lines.is_empty(), |this| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child("Words appear here as they are spoken."),
                        )
                    })
                    .children(lines),
            );

        let ended = self.status == VoiceStatus::Ended;
        let muted = self.muted;
        let on_mute = self.on_mute.clone();
        let on_end = self.on_end.clone();
        let controls = h_flex()
            .gap_2()
            .justify_end()
            .child(
                Button::new(part("mute"))
                    .secondary()
                    .icon(Icon::new(if muted {
                        CatalogIcon::MicrophoneOff
                    } else {
                        CatalogIcon::Microphone
                    }))
                    .label(if muted { "Unmute" } else { "Mute" })
                    .toggled(muted)
                    .disabled(ended)
                    .on_click(move |_, window, cx| {
                        if let Some(on_mute) = &on_mute {
                            on_mute(!muted, window, cx);
                        }
                    }),
            )
            .child(
                Button::new(part("end"))
                    .danger()
                    .icon(Icon::new(CatalogIcon::PhoneOff))
                    .label("End")
                    .disabled(ended)
                    .on_click(move |_, window, cx| {
                        if let Some(on_end) = &on_end {
                            on_end(window, cx);
                        }
                    }),
            );

        shell(cx)
            .id(self.id.clone())
            .role(Role::Region)
            .aria_label(self.spoken())
            .max_w(px(520.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Headset)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(header_tag.into_any_element()),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .v_flex()
                    .gap_4()
                    .children(notice)
                    .child(div().v_flex().gap_2().children(rows))
                    .child(div().h(px(1.)).bg(ink.opacity(0.08)))
                    .child(transcript),
            )
            .child(div().px(px(10.)).pt(px(10.)).pb(px(4.)).child(controls))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_arrive_one_at_a_time() {
        let text = "The forecast for  Friday is rain";
        assert_eq!(visible_words(text, Some(0)), Vec::<&str>::new());
        assert_eq!(visible_words(text, Some(3)), ["The", "forecast", "for"]);
        assert_eq!(visible_words(text, None).len(), 6);
        assert_eq!(visible_words(text, Some(40)).len(), 6);
    }

    #[test]
    fn each_new_word_fades_in_after_the_last() {
        assert_eq!(word_opacity(3., 0), 1.);
        assert_eq!(word_opacity(2.5, 2), 0.5);
        assert_eq!(word_opacity(2.5, 3), 0.);
    }

    #[test]
    fn call_time_reads_like_a_phone() {
        assert_eq!(call_time(Duration::from_secs(42)), "0:42");
        assert_eq!(call_time(Duration::from_secs(725)), "12:05");
        assert_eq!(call_time(Duration::from_secs(3_725)), "1:02:05");
    }

    #[test]
    fn the_meter_shows_the_newest_levels_after_silence() {
        assert_eq!(bar_heights(&[0.5, 0.8], 4, 0.), [0., 0., 0.5, 0.8]);
        assert_eq!(bar_heights(&[0.1, 0.2, 0.3], 2, 0.), [0.2, 0.3]);
        assert_eq!(bar_heights(&[0.01, 2.], 2, 0.05), [0.05, 1.]);
    }

    #[test]
    fn rows_and_turns_say_their_state() {
        assert_eq!(spoken_speaker("You", true, true, false), "You, muted");
        assert_eq!(
            spoken_speaker("You", true, false, true),
            "You, talking over"
        );
        assert_eq!(
            spoken_speaker("Guide", true, false, false),
            "Guide, speaking"
        );
        assert_eq!(spoken_speaker("Guide", false, false, false), "Guide, quiet");
        assert_eq!(
            spoken_turn("Guide", &["Rain", "is"], false, true),
            "Guide: Rain is (cut off)"
        );
        assert_eq!(
            spoken_turn("Guide", &["Rain", "is"], false, false),
            "Guide: Rain is …"
        );
        assert_eq!(spoken_turn("You", &["Hi"], true, false), "You: Hi");
    }

    #[test]
    fn the_session_line_names_who_speaks_and_the_interruption() {
        let session = VoiceSession::new("v", "Trip planning")
            .speaker(VoiceSpeaker::new("You"))
            .speaker(VoiceSpeaker::new("Guide").speaking(true))
            .elapsed(Duration::from_secs(84));
        assert_eq!(
            session.spoken(),
            "Trip planning with Guide: live, 1:24, Guide speaking, microphone on"
        );
        let over = session.interrupted(true).muted(false);
        assert_eq!(over.talked_over(), Some(1));
        assert_eq!(
            over.spoken(),
            "Trip planning with Guide: live, 1:24, you are talking over Guide, microphone on"
        );
        let ended = VoiceSession::new("v", "Call")
            .speaker(VoiceSpeaker::new("You"))
            .status(VoiceStatus::Ended)
            .elapsed(Duration::from_secs(5));
        assert_eq!(ended.spoken(), "Call: ended, 0:05");
        assert_eq!(ended.talked_over(), None);
    }
}
