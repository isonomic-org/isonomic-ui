// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the voice session.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{
    speech::{
        AudioFormat, AudioInput, AudioSink, RecognitionSession, SpeechError, SpeechEvent,
        SpeechRecognizer, SpeechSink, SpeechState,
    },
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window,
};
use isonomic_ui_extra::voice_session::{VoiceSession, VoiceSpeaker, VoiceStatus, VoiceTurn};

/// How often the made-up conversation moves on.
const TICK: Duration = Duration::from_millis(220);

/// What the made-up guide says, a word per tick.
const GUIDE: [&str; 2] = [
    "Morning. The ferry to the island leaves at nine forty and the crossing takes about an hour.",
    "If you would rather stay dry, the coast path is open and the cafe at the far end serves lunch until three.",
];

/// What you "say" into the made-up microphone, a word per 0.3s of audio.
const YOU: &str = "Could we take the coast path and come back on the ferry";

/// Audio for the story's microphone: a tone that rises and falls like speech.
struct SpokenTone;

impl AudioInput for SpokenTone {
    fn start(
        &self,
        format: AudioFormat,
        sink: AudioSink,
        cx: &mut App,
    ) -> Result<Subscription, SpeechError> {
        let chunk = Duration::from_millis(80);
        let len = (format.sample_rate() as usize / 1000 * 80) * format.channels() as usize;
        let task = cx.spawn(async move |cx| {
            let mut step = 0u32;
            loop {
                cx.background_executor().timer(chunk).await;
                step = step.wrapping_add(1);
                let syllable = ((step as f32) * 1.3).sin().abs();
                let loudness = 0.04 + 0.3 * syllable * syllable;
                let samples = (0..len)
                    .map(|ix| ((ix as f32 * 0.09).sin() * loudness * i16::MAX as f32) as i16)
                    .collect();
                cx.update(|cx| sink.push(samples, cx));
            }
        });
        Ok(Subscription::new(move || drop(task)))
    }
}

/// A recognizer that hears [`YOU`] one word per 0.3s of audio.
struct ScriptedEar;

impl SpeechRecognizer for ScriptedEar {
    fn start(
        &self,
        sink: SpeechSink,
        cx: &mut App,
    ) -> Result<Box<dyn RecognitionSession>, SpeechError> {
        sink.ready(cx);
        Ok(Box::new(Hearing {
            sink,
            words: 0,
            samples: 0,
        }))
    }
}

struct Hearing {
    sink: SpeechSink,
    words: usize,
    samples: usize,
}

impl Hearing {
    fn heard(&self) -> String {
        YOU.split(' ')
            .take(self.words)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl RecognitionSession for Hearing {
    fn push_audio(&mut self, samples: &[i16], cx: &mut App) {
        self.samples += samples.len();
        while self.samples >= 4_800 {
            self.samples -= 4_800;
            if self.words < YOU.split(' ').count() {
                self.words += 1;
                self.sink.hypothesis(self.heard(), cx);
            }
        }
    }

    fn finish(&mut self, cx: &mut App) {
        if self.words > 0 {
            self.sink.phrase(self.heard(), cx);
        }
        self.sink.finish(cx);
    }
}

/// A live trip-planning call where you talk through the microphone, and a
/// call caught at the moment you talk over the other side.
pub struct VoiceSessionStory {
    speech: Entity<SpeechState>,
    ticks: u32,
    muted: bool,
    ended: bool,
    said: Vec<String>,
    _subscription: Subscription,
}

impl VoiceSessionStory {
    /// Create the story view and start the call.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let speech = cx.new(|cx| {
                SpeechState::new(cx)
                    .recognizer(ScriptedEar)
                    .input(SpokenTone)
                    .system_fallback(false)
            });
            let subscription = cx.subscribe_in(
                &speech,
                window,
                |this: &mut Self, _, event: &SpeechEvent, _, cx| {
                    if let SpeechEvent::Final(text) = event
                        && !text.is_empty()
                    {
                        this.said.push(text.to_string());
                        cx.notify();
                    }
                },
            );
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let going = this
                        .update(cx, |this: &mut Self, cx| {
                            if this.ended {
                                return false;
                            }
                            this.ticks += 1;
                            // You start talking once the guide's first line
                            // is out, and stop a little later.
                            let first = GUIDE[0].split(' ').count() as u32;
                            if this.ticks == first + 2 && !this.muted {
                                this.speech.update(cx, |speech, cx| speech.start(cx));
                            }
                            if this.ticks == first + 22 {
                                this.speech.update(cx, |speech, cx| speech.stop(cx));
                            }
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !going {
                        break;
                    }
                }
            })
            .detach();
            Self {
                speech,
                ticks: 10,
                muted: false,
                ended: false,
                said: Vec::new(),
                _subscription: subscription,
            }
        })
    }

    /// The guide's words so far: a word a tick, the second line once you
    /// have finished.
    fn guide_turns(&self) -> Vec<(usize, VoiceTurn)> {
        let first = GUIDE[0].split(' ').count();
        let mut turns = vec![(0, VoiceTurn::new(1, GUIDE[0]).arrived(self.ticks as usize))];
        let second_from = first as u32 + 26;
        if self.ticks > second_from {
            turns.push((
                1,
                VoiceTurn::new(1, GUIDE[1]).arrived((self.ticks - second_from) as usize),
            ));
        }
        turns
    }

    /// Whether a guide line is still coming out.
    fn guide_speaking(&self) -> bool {
        let first = GUIDE[0].split(' ').count();
        let second_from = first + 26;
        let ticks = self.ticks as usize;
        (ticks > 0 && ticks < first)
            || (ticks > second_from && ticks - second_from < GUIDE[1].split(' ').count())
    }

    fn guide_levels(&self, speaking: bool) -> Vec<f32> {
        (0..28)
            .map(|ix| {
                if !speaking {
                    return 0.;
                }
                let t = (self.ticks * 2 + ix) as f32;
                (0.25 + 0.55 * ((t * 0.9).sin() * (t * 0.37).cos()).abs()).min(1.)
            })
            .collect()
    }
}

impl Story for VoiceSessionStory {
    fn title() -> &'static str {
        "Voice session"
    }

    fn description() -> &'static str {
        "A live voice conversation: level bars per speaker, a transcript written word by word, interruptions, mute and end."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for VoiceSessionStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let guide = self.guide_turns();
        let guide_speaking = self.guide_speaking();
        let mut turns: Vec<VoiceTurn> = vec![guide[0].1.clone()];
        turns.extend(self.said.iter().map(|text| VoiceTurn::new(0, text.clone())));
        turns.extend(guide.iter().skip(1).map(|(_, turn)| turn.clone()));

        let mute = cx.entity();
        let end = cx.entity();
        v_flex()
            .gap_6()
            .child(
                VoiceSession::new("trip", "Trip planning")
                    .subtitle("Talking with the travel guide")
                    .speaker(VoiceSpeaker::new("You"))
                    .speaker(
                        VoiceSpeaker::new("Guide")
                            .levels(self.guide_levels(guide_speaking && !self.ended))
                            .speaking(guide_speaking),
                    )
                    .listen(&self.speech)
                    .turns(turns)
                    .status(if self.ended {
                        VoiceStatus::Ended
                    } else {
                        VoiceStatus::Live
                    })
                    .elapsed(TICK * self.ticks + Duration::from_secs(12))
                    .muted(self.muted)
                    .on_mute(move |muted, _, cx| {
                        mute.update(cx, |this, cx| {
                            this.muted = muted;
                            this.speech.update(cx, |speech, cx| {
                                if muted {
                                    speech.stop(cx)
                                } else {
                                    speech.start(cx)
                                }
                            });
                            cx.notify();
                        })
                    })
                    .on_end(move |_, cx| {
                        end.update(cx, |this, cx| {
                            this.ended = true;
                            this.speech.update(cx, |speech, cx| speech.stop(cx));
                            cx.notify();
                        })
                    }),
            )
            .child(
                VoiceSession::new("support", "Billing question")
                    .subtitle("Talking with the help desk")
                    .speaker(VoiceSpeaker::new("You").levels(
                        [0.1, 0.3, 0.6, 0.8, 0.7, 0.9, 0.6, 0.75, 0.5, 0.65, 0.8, 0.7]
                            .into_iter()
                            .cycle()
                            .take(20),
                    ))
                    .speaker(
                        VoiceSpeaker::new("Help desk")
                            .levels([0.5, 0.6, 0.4, 0.3, 0.2, 0.1, 0.05, 0., 0., 0.])
                            .speaking(true),
                    )
                    .turn(VoiceTurn::new(
                        0,
                        "Why was I charged twice for the March invoice?",
                    ))
                    .turn(
                        VoiceTurn::new(
                            1,
                            "I can see two payments on the fourth. The second one was a retry after the card",
                        )
                        .arrived(11)
                        .cut(),
                    )
                    .turn(VoiceTurn::new(0, "Sorry, can you refund the").arrived(5))
                    .interrupted(true)
                    .elapsed(Duration::from_secs(137)),
            )
    }
}
