// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Streamed answer: an assistant's reply with the sources it used.
//!
//! The question sits on top, then a row of source cards, then the answer as
//! it streams in word by word. Short markup shapes the answer: blank lines
//! make paragraphs, lines starting with "- " make a list, `**bold**` is
//! bold, and `[1]` or `[1,3]` cite sources. Each citation is a small chip
//! that previews its sources on hover and lights up their cards. Once it is
//! done, follow-up questions and actions for copy, regenerate and feedback
//! show underneath.
//!
//! An answer can carry the reasoning behind it. The reasoning streams in
//! first, open under a "Thinking" toggle, and folds away once the answer
//! starts, its toggle then saying how long it took; the toggle opens and
//! closes it from the keyboard like any button. A stopped answer offers to
//! try again.

use std::time::{Duration, Instant};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ClipboardItem, Context, EventEmitter, FocusHandle,
    Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, Window, anchored, deferred, div,
    ease_in_out, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

mod markup;

use markup::{
    Block, Inline, parse_answer, place, plain_text, reasoning_cut, reasoning_steps, thought_words,
    total_steps,
};

/// A source the answer draws on.
#[derive(Clone)]
pub struct AnswerSource {
    title: SharedString,
    name: SharedString,
    domain: Option<SharedString>,
    icon: Option<IconName>,
    snippet: Option<SharedString>,
}

impl AnswerSource {
    /// Create a source. `name` is the short label on citation chips.
    pub fn new(title: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            name: name.into(),
            domain: None,
            icon: None,
            snippet: None,
        }
    }

    /// Show where it lives in the preview.
    pub fn domain(mut self, domain: impl Into<SharedString>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Mark it with an icon instead of its first letter.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Quote a line from it in the preview.
    pub fn snippet(mut self, snippet: impl Into<SharedString>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }
}

/// What people did with the answer.
#[derive(Debug, Clone)]
pub enum StreamedAnswerEvent {
    /// A follow-up question was picked.
    FollowUp(SharedString),
    /// Regenerate was pressed. Give the answer its new text with
    /// [`StreamedAnswer::set_answer`], or call [`StreamedAnswer::start`] to
    /// stream the same text in again.
    Regenerate,
    /// Stop was pressed while it was writing.
    Stop,
    /// Thumbs up, or thumbs down with the reason that was picked.
    Feedback {
        /// Whether it was thumbs up.
        up: bool,
        /// The reason given with a thumbs down.
        reason: Option<SharedString>,
    },
    /// The answer was copied: plain text and a numbered source list.
    Copied(String),
    /// A source card was clicked. The index counts from 0.
    OpenSource(usize),
}

/// Where the answer is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// The reasoning is streaming in, before any of the answer.
    Thinking,
    Reading,
    Writing,
    Done,
    Stopped,
}

/// An assistant's answer with its sources, streamed in.
pub struct StreamedAnswer {
    question: SharedString,
    sources: Vec<AnswerSource>,
    answer: SharedString,
    blocks: Vec<Block>,
    streaming: bool,
    animate: bool,
    speed: f32,
    follow_ups: Vec<SharedString>,
    reasons: Vec<SharedString>,
    framed: bool,
    shown: usize,
    phase: Phase,
    started: Instant,
    took: Option<Duration>,
    lit_chip: Option<usize>,
    lit_sources: Vec<usize>,
    all_sources: bool,
    thumbs: Option<bool>,
    reason: Option<SharedString>,
    copied: bool,
    reasoning: SharedString,
    reasoning_shown: usize,
    /// Opened or closed by hand; until then it is open while thinking.
    reasoning_open: Option<bool>,
    thought_for: Option<Duration>,
    hovered_follow_up: Option<usize>,
    more_hovered: bool,
    focus: FocusHandle,
    _ticker: Option<Task<()>>,
}

impl EventEmitter<StreamedAnswerEvent> for StreamedAnswer {}

impl Focusable for StreamedAnswer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StreamedAnswer {
    /// Create an answer to `question`. Call [`StreamedAnswer::start`] once it
    /// is set up.
    pub fn new(question: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            question: question.into(),
            sources: Vec::new(),
            answer: SharedString::default(),
            blocks: Vec::new(),
            streaming: false,
            animate: true,
            speed: 38.,
            follow_ups: Vec::new(),
            reasons: ["Not accurate", "Wrong source", "Too long", "Out of date"]
                .map(SharedString::from)
                .to_vec(),
            framed: false,
            shown: 0,
            phase: Phase::Reading,
            started: Instant::now(),
            took: None,
            lit_chip: None,
            lit_sources: Vec::new(),
            all_sources: false,
            thumbs: None,
            reason: None,
            copied: false,
            reasoning: SharedString::default(),
            reasoning_shown: 0,
            reasoning_open: None,
            thought_for: None,
            hovered_follow_up: None,
            more_hovered: false,
            focus: cx.focus_handle(),
            _ticker: None,
        }
    }

    /// Add a source. Citations count them from 1.
    pub fn source(mut self, source: AnswerSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Set the answer's text.
    pub fn answer(mut self, answer: impl Into<SharedString>) -> Self {
        self.answer = answer.into();
        self.blocks = parse_answer(&self.answer);
        self
    }

    /// Give the reasoning behind the answer. It streams in before the
    /// answer, word by word, and folds away once the answer starts.
    pub fn reasoning(mut self, reasoning: impl Into<SharedString>) -> Self {
        self.reasoning = reasoning.into();
        self
    }

    /// Say how long the reasoning took, for an answer shown at once or one
    /// whose owner keeps the time. Without it the time is measured.
    pub fn reasoning_took(mut self, took: Duration) -> Self {
        self.thought_for = Some(took);
        self
    }

    /// Say that more text is still coming, so it keeps writing once caught up.
    /// Until the first words come in, the header says the sources are read.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Stream the text in, or show it at once with `false`.
    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }

    /// Set how many words a second stream in.
    pub fn speed(mut self, words_per_second: f32) -> Self {
        self.speed = words_per_second.max(1.);
        self
    }

    /// Offer a related question once the answer is done.
    pub fn follow_up(mut self, question: impl Into<SharedString>) -> Self {
        self.follow_ups.push(question.into());
        self
    }

    /// Change the reasons offered after a thumbs down.
    pub fn feedback_reasons(
        mut self,
        reasons: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.reasons = reasons.into_iter().map(Into::into).collect();
        self
    }

    /// Put the answer on its own card instead of the surface it sits on.
    pub fn framed(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }

    /// Start streaming the answer in from the top.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.shown = 0;
        self.took = None;
        self.thumbs = None;
        self.reason = None;
        self.copied = false;
        self.reasoning_open = None;
        self.started = Instant::now();
        if !self.animate {
            self.reasoning_shown = reasoning_steps(&self.reasoning);
            self.shown = total_steps(&self.blocks);
            self.phase = if self.streaming {
                Phase::Writing
            } else {
                Phase::Done
            };
            self.took = (!self.streaming).then(|| self.started.elapsed());
            self._ticker = None;
            cx.notify();
            return;
        }
        self.reasoning_shown = 0;
        self.thought_for = None;
        self.phase = if self.reasoning.is_empty() {
            Phase::Reading
        } else {
            Phase::Thinking
        };
        self.tick(Duration::ZERO, cx);
        cx.notify();
    }

    /// Replace the answer's text. Text that carries on from what was there
    /// keeps streaming from where it was; anything else starts over.
    pub fn set_answer(&mut self, answer: impl Into<SharedString>, cx: &mut Context<Self>) {
        let answer = answer.into();
        let carries_on = answer.starts_with(self.answer.as_ref());
        self.answer = answer;
        self.blocks = parse_answer(&self.answer);
        if !carries_on {
            self.start(cx);
            return;
        }
        if !self.animate {
            self.shown = total_steps(&self.blocks);
        } else if self.phase == Phase::Done {
            self.phase = Phase::Writing;
            self.took = None;
            self.tick(Duration::ZERO, cx);
        }
        cx.notify();
    }

    /// Add streamed text to the end of the answer, such as the next few
    /// tokens from a model. It keeps streaming from where it was.
    pub fn push_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let next = format!("{}{text}", self.answer);
        self.set_answer(next, cx);
    }

    /// Replace the reasoning. While it is still being thought, the new words
    /// stream in after the ones shown.
    pub fn set_reasoning(&mut self, reasoning: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.reasoning = reasoning.into();
        if !self.animate {
            self.reasoning_shown = reasoning_steps(&self.reasoning);
        }
        cx.notify();
    }

    /// Say whether more text is still coming.
    pub fn set_streaming(&mut self, streaming: bool, cx: &mut Context<Self>) {
        self.streaming = streaming;
        if !streaming && !self.animate && self.phase == Phase::Writing {
            self.finish();
        }
        cx.notify();
    }

    fn finish(&mut self) {
        self.phase = Phase::Done;
        self.took = Some(self.started.elapsed());
        self._ticker = None;
    }

    fn tick(&mut self, first_wait: Duration, cx: &mut Context<Self>) {
        let step = Duration::from_secs_f32(1. / self.speed);
        self._ticker = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(first_wait).await;
            loop {
                let more = this
                    .update(cx, |this, cx| {
                        let total = total_steps(&this.blocks);
                        if this.reasoning_shown < reasoning_steps(&this.reasoning) {
                            // The reasoning streams in before the answer.
                            this.phase = Phase::Thinking;
                            this.reasoning_shown += 1;
                        } else if this.shown < total {
                            if this.phase == Phase::Thinking && this.thought_for.is_none() {
                                this.thought_for = Some(this.started.elapsed());
                            }
                            // Words are coming in: the answer is being written.
                            this.phase = Phase::Writing;
                            this.shown += 1;
                        } else if !this.streaming {
                            this.finish();
                            cx.notify();
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
                cx.background_executor().timer(step).await;
            }
        }));
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Stopped;
        self.took = Some(self.started.elapsed());
        self._ticker = None;
        cx.emit(StreamedAnswerEvent::Stop);
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let text = plain_text(&self.blocks, &self.sources);
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        cx.emit(StreamedAnswerEvent::Copied(text));
        self.copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn light(&mut self, chip: Option<usize>, sources: Vec<usize>, cx: &mut Context<Self>) {
        if self.lit_chip != chip || self.lit_sources != sources {
            self.lit_chip = chip;
            self.lit_sources = sources;
            cx.notify();
        }
    }
}

impl Render for StreamedAnswer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let card_fades: Vec<f32> = (0..self.sources.len())
            .map(|ix| {
                look::hover_fade(
                    ("answer-source", ix),
                    self.lit_sources.contains(&ix),
                    window,
                    cx,
                )
            })
            .collect();
        let more_fade = look::hover_fade("answer-more-sources", self.more_hovered, window, cx);
        let follow_fades: Vec<f32> = (0..self.follow_ups.len())
            .map(|ix| {
                look::hover_fade(
                    ("answer-follow-up", ix),
                    self.hovered_follow_up == Some(ix),
                    window,
                    cx,
                )
            })
            .collect();
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let popover = theme.popover;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette = [
            theme.blue,
            theme.magenta,
            theme.green,
            theme.cyan,
            theme.yellow,
            theme.red,
        ];

        let mark = |source: &AnswerSource, ix: usize, size: f32| {
            div()
                .flex_none()
                .size(px(size))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .bg(palette[ix % palette.len()])
                .text_color(isonomic_ui::white())
                .text_size(px(size * 0.62))
                .font_medium()
                .map(|this| match &source.icon {
                    Some(icon) => this.child(
                        Icon::new(icon.clone())
                            .size(px(size * 0.7))
                            .text_color(isonomic_ui::white()),
                    ),
                    None => this.child(
                        source
                            .name
                            .chars()
                            .next()
                            .map(|c| c.to_uppercase().to_string())
                            .unwrap_or_default(),
                    ),
                })
        };

        // Source cards: three and a "+N" tile, or all of them once opened.
        let limit = if self.all_sources || self.sources.len() <= 3 {
            self.sources.len()
        } else {
            3
        };
        let hidden = self.sources.len() - limit;
        let card = |ix: usize, source: &AnswerSource, cx: &mut Context<Self>| {
            let lit = self.lit_sources.contains(&ix);
            v_flex()
                .id(("answer-source", ix))
                .flex_1()
                .min_w(px(140.))
                .gap_1()
                .p_2()
                .rounded(radius)
                .border_1()
                .border_color(if lit { accent } else { line })
                .bg(look::mix(
                    accent.opacity(0.),
                    accent.opacity(0.06),
                    card_fades[ix],
                ))
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.light(None, if *hovered { vec![ix] } else { Vec::new() }, cx)
                }))
                .on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(StreamedAnswerEvent::OpenSource(ix))),
                )
                .child(
                    h_flex()
                        .gap_1p5()
                        .text_caption()
                        .text_color(quiet)
                        .child(mark(source, ix, 14.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(source.name.clone()),
                        )
                        .child((ix + 1).to_string()),
                )
                .child(
                    div()
                        .text_label_small()
                        .line_height(relative(1.35))
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(source.title.clone()),
                )
        };
        let cards = (!self.sources.is_empty()).then(|| {
            let tile_lit = self.lit_sources.iter().any(|ix| *ix >= limit);
            h_flex()
                .flex_wrap()
                .items_stretch()
                .gap_1p5()
                .children(
                    self.sources
                        .iter()
                        .take(limit)
                        .enumerate()
                        .map(|(ix, source)| card(ix, source, cx))
                        .collect::<Vec<_>>(),
                )
                .when(hidden > 0, |this| {
                    this.child(
                        v_flex()
                            .id("answer-more-sources")
                            .flex_none()
                            .w(px(56.))
                            .justify_between()
                            .p_2()
                            .rounded(radius)
                            .border_1()
                            .border_color(if tile_lit { accent } else { line })
                            .bg(look::mix(
                                if tile_lit {
                                    accent.opacity(0.06)
                                } else {
                                    chip.opacity(0.)
                                },
                                chip.opacity(0.5),
                                more_fade,
                            ))
                            .cursor_pointer()
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if this.more_hovered != *hovered {
                                    this.more_hovered = *hovered;
                                    cx.notify();
                                }
                                let rest = if *hovered {
                                    (limit..limit + hidden).collect()
                                } else {
                                    Vec::new()
                                };
                                this.light(None, rest, cx)
                            }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.all_sources = true;
                                cx.notify();
                            }))
                            .child(div().font_medium().child(format!("+{hidden}")))
                            .child(div().text_caption().text_color(quiet).child("more")),
                    )
                })
        });

        // The answer, cut at the number of steps streamed in so far.
        let mut budget = self.shown;
        let mut chip_ix = 0;
        let mut blocks = Vec::new();
        let writing = matches!(
            self.phase,
            Phase::Thinking | Phase::Reading | Phase::Writing
        );
        for block in &self.blocks {
            if budget == 0 {
                break;
            }
            let mut runs: Vec<Vec<isonomic_ui::AnyElement>> = Vec::new();
            let mut glue = false;
            for inline in &block.inlines {
                if budget == 0 {
                    break;
                }
                match inline {
                    Inline::Text { text, bold } => {
                        for word in text.split_inclusive(' ') {
                            if budget == 0 {
                                break;
                            }
                            budget -= 1;
                            place(
                                &mut runs,
                                &mut glue,
                                div()
                                    .whitespace_nowrap()
                                    .when(*bold, |this| this.font_medium().text_color(ink))
                                    .child(word.to_string())
                                    .into_any_element(),
                                !word.ends_with(' '),
                            );
                        }
                    }
                    Inline::Cite(numbers) => {
                        budget -= 1;
                        let this_chip = chip_ix;
                        chip_ix += 1;
                        let known: Vec<usize> = numbers
                            .iter()
                            .copied()
                            .filter(|n| *n < self.sources.len())
                            .collect();
                        let Some(first) = known.first().copied() else {
                            continue;
                        };
                        let label = if known.len() > 1 {
                            format!("{} +{}", self.sources[first].name, known.len() - 1)
                        } else {
                            self.sources[first].name.to_string()
                        };
                        let open = self.lit_chip == Some(this_chip);
                        let lit = open || known.iter().any(|ix| self.lit_sources.contains(ix));
                        let preview = open.then(|| {
                            v_flex()
                                .w(px(300.))
                                .p_1()
                                .rounded(radius)
                                .bg(popover)
                                .shadow(resting_shadow(cx))
                                .children(known.iter().map(|ix| {
                                    let source = &self.sources[*ix];
                                    v_flex()
                                        .gap_0p5()
                                        .px_2()
                                        .py_1p5()
                                        .child(
                                            h_flex()
                                                .gap_1p5()
                                                .text_caption()
                                                .text_color(quiet)
                                                .child(mark(source, *ix, 12.))
                                                .child(
                                                    div().flex_1().child(
                                                        source
                                                            .domain
                                                            .clone()
                                                            .unwrap_or(source.name.clone()),
                                                    ),
                                                )
                                                .child((ix + 1).to_string()),
                                        )
                                        .child(
                                            div()
                                                .text_label_small()
                                                .text_color(ink)
                                                .child(source.title.clone()),
                                        )
                                        .children(source.snippet.clone().map(|snippet| {
                                            div()
                                                .text_caption()
                                                .text_color(quiet)
                                                .line_clamp(2)
                                                .text_ellipsis()
                                                .child(snippet)
                                        }))
                                }))
                        });
                        place(
                            &mut runs,
                            &mut glue,
                            div()
                                .relative()
                                .ml_1()
                                .child(
                                    div()
                                        .id(("answer-cite", this_chip))
                                        .px_1p5()
                                        .rounded(px(4.))
                                        .text_caption()
                                        .line_height(relative(1.5))
                                        .bg(if lit { accent.opacity(0.16) } else { chip })
                                        .text_color(if lit { accent } else { quiet })
                                        .cursor_pointer()
                                        .on_hover(cx.listener(
                                            move |this, hovered: &bool, _, cx| {
                                                let (chip, lit) = if *hovered {
                                                    (Some(this_chip), known.clone())
                                                } else {
                                                    (None, Vec::new())
                                                };
                                                this.light(chip, lit, cx)
                                            },
                                        ))
                                        .child(label),
                                )
                                .children(preview.map(|preview| {
                                    deferred(
                                        anchored()
                                            .offset(point(px(0.), px(22.)))
                                            .snap_to_window_with_margin(px(8.))
                                            .child(preview),
                                    )
                                    .with_priority(1)
                                }))
                                .into_any_element(),
                            true,
                        );
                    }
                }
            }
            blocks.push((block.item, runs));
        }
        let last = blocks.len().saturating_sub(1);
        let answer = v_flex()
            .gap_2()
            .children(
                blocks
                    .into_iter()
                    .enumerate()
                    .map(|(ix, (item, mut runs))| {
                        if ix == last && writing {
                            place(
                                &mut runs,
                                &mut true,
                                div()
                                    .ml_1()
                                    .size(px(7.))
                                    .rounded_full()
                                    .bg(accent)
                                    .with_animation(
                                        "answer-cursor",
                                        Animation::new(Duration::from_millis(900))
                                            .repeat()
                                            .with_easing(ease_in_out),
                                        |this, delta| {
                                            this.opacity(
                                                0.35 + 0.65 * (1. - (delta * 2. - 1.).abs()),
                                            )
                                        },
                                    )
                                    .into_any_element(),
                                true,
                            );
                        }
                        let words = h_flex()
                            .flex_1()
                            .min_w_0()
                            .flex_wrap()
                            .items_center()
                            .children(
                                runs.into_iter()
                                    .map(|run| h_flex().flex_none().items_center().children(run)),
                            );
                        if item {
                            h_flex()
                                .items_start()
                                .gap_2()
                                .pl_1()
                                .child(
                                    div()
                                        .mt(px(8.))
                                        .size(px(4.))
                                        .rounded_full()
                                        .bg(quiet)
                                        .flex_none(),
                                )
                                .child(words)
                                .into_any_element()
                        } else {
                            words.into_any_element()
                        }
                    }),
            );

        let status = match self.phase {
            // The reasoning's own toggle says it is thinking.
            Phase::Thinking if self.sources.is_empty() => "Working".to_string(),
            Phase::Thinking => format!("Reading {} sources", self.sources.len()),
            Phase::Reading => format!("Reading {} sources", self.sources.len()),
            Phase::Writing => "Writing".to_string(),
            Phase::Done => "Answer".to_string(),
            Phase::Stopped => "Stopped".to_string(),
        };
        let done = matches!(self.phase, Phase::Done | Phase::Stopped);

        let thinking = self.phase == Phase::Thinking;
        let reasoning = (!self.reasoning.is_empty()).then(|| {
            let open = self.reasoning_open.unwrap_or(thinking);
            let label = thought_words(self.thought_for, thinking);
            v_flex()
                .gap_1()
                .child(
                    h_flex().child(
                        Button::new("answer-reasoning")
                            .ghost()
                            .xsmall()
                            .icon(if open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .label(label.clone())
                            .text_color(quiet)
                            .accessibility_label(format!(
                                "{label}, reasoning {}",
                                if open { "shown" } else { "hidden" }
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reasoning_open = Some(!open);
                                cx.notify();
                            })),
                    ),
                )
                .when(open, |this| {
                    this.child(
                        v_flex()
                            .id("answer-reasoning-text")
                            .gap_1p5()
                            .ml(px(9.))
                            .pl_3()
                            .border_l_2()
                            .border_color(line)
                            .text_caption()
                            .line_height(px(18.))
                            .text_color(quiet)
                            .children(
                                reasoning_cut(&self.reasoning, self.reasoning_shown)
                                    .into_iter()
                                    .map(|para| div().child(para)),
                            ),
                    )
                })
        });

        let follow_ups = (self.phase == Phase::Done && !self.follow_ups.is_empty()).then(|| {
            v_flex()
                .child(
                    div()
                        .pb_1()
                        .text_label_small()
                        .text_color(quiet)
                        .child("RELATED"),
                )
                .children(self.follow_ups.iter().enumerate().map(|(ix, question)| {
                    let asked = question.clone();
                    h_flex()
                        .id(("answer-follow-up", ix))
                        .py_1p5()
                        .border_t_1()
                        .border_color(line)
                        .cursor_pointer()
                        .text_color(look::mix(ink, accent, follow_fades[ix]))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            let next = if *hovered {
                                Some(ix)
                            } else if this.hovered_follow_up == Some(ix) {
                                None
                            } else {
                                this.hovered_follow_up
                            };
                            if next != this.hovered_follow_up {
                                this.hovered_follow_up = next;
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(StreamedAnswerEvent::FollowUp(asked.clone()))
                        }))
                        .child(div().flex_1().child(question.clone()))
                        .child(Icon::new(IconName::ChevronRight).xsmall().text_color(quiet))
                }))
        });

        let took = self
            .took
            .map(|took| took.as_secs_f32())
            .map(|secs| match self.phase {
                Phase::Stopped => format!("Stopped after {secs:.1}s"),
                _ => format!("Answered in {secs:.1}s"),
            })
            .unwrap_or_default();
        let thumbs = self.thumbs;
        let actions =
            if done {
                h_flex()
                    .gap_0p5()
                    .pt_2()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        Button::new("answer-copy")
                            .ghost()
                            .xsmall()
                            .icon(if self.copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .accessibility_label("Copy the answer")
                            .tooltip(if self.copied { "Copied" } else { "Copy" })
                            .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                    )
                    .child(
                        Button::new("answer-regenerate")
                            .ghost()
                            .xsmall()
                            .icon(IconName::RotateCw)
                            .accessibility_label("Regenerate")
                            .tooltip("Regenerate")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(StreamedAnswerEvent::Regenerate);
                            })),
                    )
                    .child(
                        Button::new("answer-up")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ThumbsUp)
                            .accessibility_label("Good answer")
                            .tooltip("Good answer")
                            .when(thumbs == Some(true), |this| this.text_color(accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.thumbs = Some(true);
                                this.reason = None;
                                cx.emit(StreamedAnswerEvent::Feedback {
                                    up: true,
                                    reason: None,
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("answer-down")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ThumbsDown)
                            .accessibility_label("Bad answer")
                            .tooltip("Bad answer")
                            .when(thumbs == Some(false), |this| this.text_color(accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.thumbs = Some(false);
                                this.reason = None;
                                cx.notify();
                            })),
                    )
                    .when(self.phase == Phase::Stopped, |this| {
                        this.child(
                            Button::new("answer-retry")
                                .outline()
                                .xsmall()
                                .ml_1()
                                .icon(IconName::RotateCw)
                                .label("Retry")
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.emit(StreamedAnswerEvent::Regenerate);
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(div().text_caption().text_color(quiet).child(
                        match (thumbs, &self.reason) {
                            (Some(false), None) => "What went wrong?".to_string(),
                            (Some(false), Some(reason)) => format!("Thanks, noted: {reason}"),
                            (Some(true), _) => "Thanks for the feedback".to_string(),
                            _ => took,
                        },
                    ))
                    .into_any_element()
            } else {
                h_flex()
                    .pt_2()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        Button::new("answer-stop")
                            .outline()
                            .xsmall()
                            .icon(IconName::Square)
                            .label("Stop")
                            .on_click(cx.listener(|this, _, _, cx| this.stop(cx))),
                    )
                    .into_any_element()
            };
        let reasons = (thumbs == Some(false) && self.reason.is_none()).then(|| {
            h_flex()
                .flex_wrap()
                .gap_1p5()
                .children(self.reasons.iter().enumerate().map(|(ix, reason)| {
                    let picked = reason.clone();
                    Button::new(("answer-reason", ix))
                        .outline()
                        .xsmall()
                        .label(reason.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.reason = Some(picked.clone());
                            cx.emit(StreamedAnswerEvent::Feedback {
                                up: false,
                                reason: Some(picked.clone()),
                            });
                            cx.notify();
                        }))
                }))
        });

        v_flex()
            .track_focus(&self.focus)
            .w_full()
            .max_w(px(640.))
            .gap_3()
            .text_color(ink)
            .text_body()
            .line_height(relative(1.55))
            .when(self.framed, |this| {
                this.p_4()
                    .rounded(radius_lg)
                    .bg(fill)
                    .shadow(resting_shadow(cx))
            })
            .child(
                div()
                    .text_base()
                    .font_medium()
                    .line_height(relative(1.35))
                    .child(self.question.clone()),
            )
            .children(cards)
            .child(
                h_flex()
                    .gap_1p5()
                    .text_caption()
                    .text_color(quiet)
                    .map(|this| {
                        if done {
                            this.child(Icon::new(IconName::Bot).xsmall().text_color(accent))
                        } else {
                            this.child(Spinner::new().xsmall().color(accent))
                        }
                    })
                    .child(status),
            )
            .children(reasoning)
            .child(answer)
            .children(follow_ups)
            .child(actions)
            .children(reasons)
    }
}
