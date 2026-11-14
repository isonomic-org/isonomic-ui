//! Streamed answer: an assistant's reply with the sources it used.
//!
//! The question sits on top, then a row of source cards, then the answer as
//! it streams in word by word. Short markup shapes the answer: blank lines
//! make paragraphs, lines starting with "- " make a list, `**bold**` is
//! bold, and `[1]` or `[1,3]` cite sources. Each citation is a small chip
//! that previews its sources on hover and lights up their cards. Once it is
//! done, follow-up questions and actions for copy, regenerate and feedback
//! show underneath.

use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt as _, App, ClipboardItem, Context, EventEmitter, FocusHandle,
    Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, Window, anchored, deferred, div,
    ease_in_out, point, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    spinner::Spinner,
    v_flex,
};

use crate::common::resting_shadow;

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
    /// Regenerate was pressed. The answer streams in again.
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

/// A piece of a line of the answer.
#[derive(Debug, Clone, PartialEq)]
enum Inline {
    Text {
        text: String,
        bold: bool,
    },
    /// Cited sources, counted from 0.
    Cite(Vec<usize>),
}

/// A paragraph or a list item.
#[derive(Debug, Clone, PartialEq)]
struct Block {
    item: bool,
    inlines: Vec<Inline>,
}

/// Read a line's bold markers and citations.
fn parse_line(line: &str) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let mut bold = false;
    let mut text = String::new();
    let flush = |text: &mut String, bold: bool, out: &mut Vec<Inline>| {
        if !text.is_empty() {
            out.push(Inline::Text {
                text: std::mem::take(text),
                bold,
            });
        }
    };
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            flush(&mut text, bold, &mut out);
            bold = !bold;
            rest = after;
            continue;
        }
        if rest.starts_with('[')
            && let Some(close) = rest.find(']')
        {
            let inside = &rest[1..close];
            let numbers: Option<Vec<usize>> = inside
                .split(',')
                .map(|part| part.trim().parse::<usize>().ok().filter(|n| *n > 0))
                .collect();
            if let Some(numbers) = numbers.filter(|n| !n.is_empty()) {
                // A citation hugs the word before it, so the space goes.
                let trimmed = text.trim_end().len();
                text.truncate(trimmed);
                flush(&mut text, bold, &mut out);
                out.push(Inline::Cite(numbers.into_iter().map(|n| n - 1).collect()));
                rest = &rest[close + 1..];
                continue;
            }
        }
        let next = rest.chars().next().map_or(1, char::len_utf8);
        text.push_str(&rest[..next]);
        rest = &rest[next..];
    }
    flush(&mut text, bold, &mut out);
    out
}

/// Split the answer into paragraphs and list items.
fn parse_answer(answer: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para = String::new();
    let end_para = |para: &mut String, blocks: &mut Vec<Block>| {
        if !para.trim().is_empty() {
            blocks.push(Block {
                item: false,
                inlines: parse_line(para.trim()),
            });
        }
        para.clear();
    };
    for line in answer.lines() {
        let line = line.trim();
        if line.is_empty() {
            end_para(&mut para, &mut blocks);
        } else if let Some(item) = line.strip_prefix("- ") {
            end_para(&mut para, &mut blocks);
            blocks.push(Block {
                item: true,
                inlines: parse_line(item.trim()),
            });
        } else {
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(line);
        }
    }
    end_para(&mut para, &mut blocks);
    blocks
}

/// The steps an inline takes to stream in: a word each, or one for a chip.
fn steps(inline: &Inline) -> usize {
    match inline {
        Inline::Text { text, .. } => text.split_inclusive(' ').count(),
        Inline::Cite(_) => 1,
    }
}

fn total_steps(blocks: &[Block]) -> usize {
    blocks.iter().flat_map(|b| &b.inlines).map(steps).sum()
}

/// Add a word or chip to the runs that wrap as one. A part joins the run
/// before it when no space came between them, so a citation never wraps
/// away from its word or the full stop after it.
fn place(
    runs: &mut Vec<Vec<gpui::AnyElement>>,
    glue: &mut bool,
    part: gpui::AnyElement,
    open_end: bool,
) {
    match runs.last_mut() {
        Some(run) if *glue => run.push(part),
        _ => runs.push(vec![part]),
    }
    *glue = open_end;
}

/// The answer as plain text, with a numbered list of its sources.
fn plain_text(blocks: &[Block], sources: &[AnswerSource]) -> String {
    let mut out = String::new();
    for (ix, block) in blocks.iter().enumerate() {
        if ix > 0 {
            out.push_str(if block.item && blocks[ix - 1].item {
                "\n"
            } else {
                "\n\n"
            });
        }
        if block.item {
            out.push_str("- ");
        }
        for inline in &block.inlines {
            match inline {
                Inline::Text { text, .. } => out.push_str(text),
                Inline::Cite(numbers) => {
                    let list: Vec<String> = numbers.iter().map(|n| (n + 1).to_string()).collect();
                    out.push_str(&format!(" [{}]", list.join(",")));
                }
            }
        }
    }
    if !sources.is_empty() {
        out.push_str("\n\nSources:");
        for (ix, source) in sources.iter().enumerate() {
            out.push_str(&format!("\n{}. {}", ix + 1, source.title));
            if let Some(domain) = &source.domain {
                out.push_str(&format!(" ({domain})"));
            }
        }
    }
    out
}

/// Where the answer is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
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

    /// Say that more text is still coming, so it keeps writing once caught up.
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
        self.started = Instant::now();
        if !self.animate {
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
        self.phase = Phase::Reading;
        self.tick(Duration::from_millis(700), cx);
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
                        if this.phase == Phase::Reading {
                            this.phase = Phase::Writing;
                        }
                        let total = total_steps(&this.blocks);
                        if this.shown < total {
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .text_color(gpui::white())
                .text_size(px(size * 0.62))
                .font_semibold()
                .map(|this| match &source.icon {
                    Some(icon) => this.child(
                        Icon::new(icon.clone())
                            .size(px(size * 0.7))
                            .text_color(gpui::white()),
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
                .when(lit, |this| this.bg(accent.opacity(0.06)))
                .cursor_pointer()
                .hover(|style| style.bg(chip.opacity(0.5)))
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.light(None, if *hovered { vec![ix] } else { Vec::new() }, cx)
                }))
                .on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(StreamedAnswerEvent::OpenSource(ix))),
                )
                .child(
                    h_flex()
                        .gap_1p5()
                        .text_xs()
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
                        .text_xs()
                        .font_medium()
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
                            .when(tile_lit, |this| this.bg(accent.opacity(0.06)))
                            .cursor_pointer()
                            .hover(|style| style.bg(chip.opacity(0.5)))
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
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
                            .child(div().font_semibold().child(format!("+{hidden}")))
                            .child(div().text_xs().text_color(quiet).child("more")),
                    )
                })
        });

        // The answer, cut at the number of steps streamed in so far.
        let mut budget = self.shown;
        let mut chip_ix = 0;
        let mut blocks = Vec::new();
        let writing = matches!(self.phase, Phase::Reading | Phase::Writing);
        for block in &self.blocks {
            if budget == 0 {
                break;
            }
            let mut runs: Vec<Vec<gpui::AnyElement>> = Vec::new();
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
                                    .when(*bold, |this| this.font_semibold().text_color(ink))
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
                                .border_1()
                                .border_color(line)
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
                                                .text_xs()
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
                                                .text_xs()
                                                .font_semibold()
                                                .text_color(ink)
                                                .child(source.title.clone()),
                                        )
                                        .children(source.snippet.clone().map(|snippet| {
                                            div()
                                                .text_xs()
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
                                        .text_xs()
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
            Phase::Reading => format!("Reading {} sources", self.sources.len()),
            Phase::Writing => "Writing".to_string(),
            Phase::Done => "Answer".to_string(),
            Phase::Stopped => "Stopped".to_string(),
        };
        let done = matches!(self.phase, Phase::Done | Phase::Stopped);

        let follow_ups = (self.phase == Phase::Done && !self.follow_ups.is_empty()).then(|| {
            v_flex()
                .child(
                    div()
                        .pb_1()
                        .text_xs()
                        .font_medium()
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
                        .hover(|style| style.text_color(accent))
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
        let actions = if done {
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
                        .tooltip(if self.copied { "Copied" } else { "Copy" })
                        .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                )
                .child(
                    Button::new("answer-regenerate")
                        .ghost()
                        .xsmall()
                        .icon(IconName::RotateCw)
                        .tooltip("Regenerate")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.emit(StreamedAnswerEvent::Regenerate);
                            this.start(cx);
                        })),
                )
                .child(
                    Button::new("answer-up")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ThumbsUp)
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
                        .tooltip("Bad answer")
                        .when(thumbs == Some(false), |this| this.text_color(accent))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.thumbs = Some(false);
                            this.reason = None;
                            cx.notify();
                        })),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child(match (thumbs, &self.reason) {
                            (Some(false), None) => "What went wrong?".to_string(),
                            (Some(false), Some(reason)) => format!("Thanks, noted: {reason}"),
                            (Some(true), _) => "Thanks for the feedback".to_string(),
                            _ => took,
                        }),
                )
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
            .text_sm()
            .line_height(relative(1.55))
            .when(self.framed, |this| {
                this.p_4()
                    .rounded(radius_lg)
                    .border_1()
                    .border_color(line)
                    .bg(fill)
                    .shadow(resting_shadow(cx))
            })
            .child(
                div()
                    .text_base()
                    .font_semibold()
                    .line_height(relative(1.35))
                    .child(self.question.clone()),
            )
            .children(cards)
            .child(
                h_flex()
                    .gap_1p5()
                    .text_xs()
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
            .child(answer)
            .children(follow_ups)
            .child(actions)
            .children(reasons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read_bold_and_citations() {
        assert_eq!(
            parse_line("**Rent** rose [2] and fees [1, 3]. See [x] or [0]."),
            vec![
                Inline::Text {
                    text: "Rent".into(),
                    bold: true
                },
                Inline::Text {
                    text: " rose".into(),
                    bold: false
                },
                Inline::Cite(vec![1]),
                Inline::Text {
                    text: " and fees".into(),
                    bold: false
                },
                Inline::Cite(vec![0, 2]),
                Inline::Text {
                    text: ". See [x] or [0].".into(),
                    bold: false
                },
            ]
        );
    }

    #[test]
    fn answers_split_into_paragraphs_and_items() {
        let blocks = parse_answer("One\ntwo.\n\n- first\n- second\nAfter");
        assert_eq!(blocks.len(), 4);
        assert!(!blocks[0].item && blocks[1].item && blocks[2].item && !blocks[3].item);
        assert_eq!(
            blocks[0].inlines,
            vec![Inline::Text {
                text: "One two.".into(),
                bold: false
            }]
        );
        assert_eq!(total_steps(&blocks), 2 + 1 + 1 + 1);
    }

    #[test]
    fn copies_as_plain_text_with_sources() {
        let blocks = parse_answer("**Yes** [1].\n\n- a\n- b");
        let sources = [AnswerSource::new("Ledger export", "Ledger").domain("ledger.example")];
        assert_eq!(
            plain_text(&blocks, &sources),
            "Yes [1].\n\n- a\n- b\n\nSources:\n1. Ledger export (ledger.example)"
        );
    }
}
