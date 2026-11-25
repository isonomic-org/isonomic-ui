// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Sourced answer: a finished answer wired to the sources behind it.
//!
//! Every cited sentence ends in numbered markers. Pointing at a marker tints
//! its sentence and lights up its source; pointing at a source tints every
//! sentence that cites it. Pressing either unfolds the quoted passage, with
//! the words it shares with the answer marked.

use std::{ops::Range, rc::Rc, time::Duration};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ClipboardItem, ElementId, FocusHandle, HighlightStyle, InteractiveElement as _,
    InteractiveText, IntoElement, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, StyledText, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How long the status line says that the quote was copied.
const COPIED_FOR: Duration = Duration::from_secs(2);

/// A source the answer draws on.
#[derive(Debug, Clone)]
pub struct AnswerSource {
    id: SharedString,
    name: SharedString,
    page_title: Option<SharedString>,
    site: Option<SharedString>,
    date: Option<SharedString>,
    url: Option<SharedString>,
    quote: Option<SharedString>,
    marks: Vec<SharedString>,
}

impl AnswerSource {
    /// Create a source with the id segments cite it by and the name on its chip.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            page_title: None,
            site: None,
            date: None,
            url: None,
            quote: None,
            marks: Vec::new(),
        }
    }

    /// The title of the page or document, shown above the passage.
    pub fn page_title(mut self, title: impl Into<SharedString>) -> Self {
        self.page_title = Some(title.into());
        self
    }

    /// Where the source lives, such as a host name.
    pub fn site(mut self, site: impl Into<SharedString>) -> Self {
        self.site = Some(site.into());
        self
    }

    /// When the source was written, already formatted.
    pub fn date(mut self, date: impl Into<SharedString>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// The address "Open source" goes to.
    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// The passage the answer relied on.
    pub fn quote(mut self, quote: impl Into<SharedString>) -> Self {
        self.quote = Some(quote.into());
        self
    }

    /// Mark this exact phrase in the passage. Without any, the longer words
    /// the passage shares with the sentences that cite it are marked.
    pub fn mark(mut self, phrase: impl Into<SharedString>) -> Self {
        self.marks.push(phrase.into());
        self
    }
}

/// A stretch of the answer, usually one sentence.
#[derive(Debug, Clone)]
pub struct AnswerSegment {
    text: SharedString,
    cites: Vec<SharedString>,
    new_paragraph: bool,
}

impl AnswerSegment {
    /// Create a segment. Keep the spaces between sentences in the text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            cites: Vec::new(),
            new_paragraph: false,
        }
    }

    /// Cite a source by id. Each one adds a marker; unknown ids are skipped.
    pub fn cite(mut self, source: impl Into<SharedString>) -> Self {
        self.cites.push(source.into());
        self
    }

    /// Start a new paragraph with this segment.
    pub fn new_paragraph(mut self) -> Self {
        self.new_paragraph = true;
        self
    }
}

type SourceHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type PreviewHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type ShowHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// An answer whose sentences can be checked against their sources.
#[derive(IntoElement)]
pub struct SourcedAnswer {
    id: ElementId,
    style: StyleRefinement,
    segments: Vec<AnswerSegment>,
    sources: Vec<AnswerSource>,
    pages_read: Option<usize>,
    default_open: Option<SharedString>,
    on_open_source: Option<SourceHandler>,
    on_preview_change: Option<PreviewHandler>,
    action: Option<(SharedString, ActionHandler)>,
}

impl SourcedAnswer {
    /// Create an empty answer.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            segments: Vec::new(),
            sources: Vec::new(),
            pages_read: None,
            default_open: None,
            on_open_source: None,
            on_preview_change: None,
            action: None,
        }
    }

    /// Add a segment after the ones already added.
    pub fn segment(mut self, segment: AnswerSegment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Add a source. Sources are numbered in the order they are added.
    pub fn source(mut self, source: AnswerSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Show how many pages were read while searching.
    pub fn pages_read(mut self, pages: usize) -> Self {
        self.pages_read = Some(pages);
        self
    }

    /// Start with this source's passage unfolded.
    pub fn default_open(mut self, source: impl Into<SharedString>) -> Self {
        self.default_open = Some(source.into());
        self
    }

    /// Handle "Open source" yourself, with the source's id. Without this the
    /// source's address is opened.
    pub fn on_open_source(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_source = Some(Rc::new(handler));
        self
    }

    /// Run when a passage opens, switches or closes, with the open source's id.
    pub fn on_preview_change(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_preview_change = Some(Rc::new(handler));
        self
    }

    /// Add a button to the last row, such as "Follow up".
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }
}

impl Styled for SourcedAnswer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct AnswerState {
    seeded: bool,
    /// The source whose passage is unfolded.
    open: Option<usize>,
    /// The source being pointed at, with the segment when it is pointed at
    /// through one of that segment's markers.
    pointed: Option<(usize, Option<usize>)>,
    copied: bool,
    _copied_reset: Option<Task<()>>,
}

/// A numbered marker inside a paragraph.
#[derive(Debug, PartialEq)]
struct Marker {
    range: Range<usize>,
    segment: usize,
    source: usize,
}

/// One paragraph as a single run of text, with the places that matter in it.
#[derive(Debug, Default, PartialEq)]
struct Paragraph {
    text: String,
    /// Each segment's words, without the spaces at their edges.
    sentences: Vec<(Range<usize>, usize)>,
    markers: Vec<Marker>,
}

/// Lay the segments out as paragraphs and put a marker after each citation.
/// A marker is tied to the last word before it by non-breaking spaces, so it
/// never starts a line on its own.
fn paragraphs(segments: &[AnswerSegment], sources: &[AnswerSource]) -> Vec<Paragraph> {
    let mut out: Vec<Paragraph> = Vec::new();
    for (ix, segment) in segments.iter().enumerate() {
        if segment.new_paragraph || out.is_empty() {
            out.push(Paragraph::default());
        }
        let Some(paragraph) = out.last_mut() else {
            continue;
        };
        let text = segment.text.as_ref();
        let words = text.trim();
        if words.is_empty() {
            paragraph.text.push_str(text);
            continue;
        }
        paragraph
            .text
            .push_str(&text[..text.len() - text.trim_start().len()]);
        let start = paragraph.text.len();
        paragraph.text.push_str(words);
        paragraph.sentences.push((start..paragraph.text.len(), ix));
        for cite in &segment.cites {
            let Some(source) = sources.iter().position(|source| &source.id == cite) else {
                continue;
            };
            paragraph.text.push('\u{a0}');
            let start = paragraph.text.len();
            paragraph
                .text
                .push_str(&format!("\u{a0}{}\u{a0}", source + 1));
            paragraph.markers.push(Marker {
                range: start..paragraph.text.len(),
                segment: ix,
                source,
            });
        }
        paragraph.text.push_str(&text[text.trim_end().len()..]);
    }
    out
}

/// The byte ranges of the words in a text.
fn words(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (ix, ch) in text.char_indices() {
        if ch.is_alphanumeric() {
            start.get_or_insert(ix);
        } else if let Some(from) = start.take() {
            out.push(from..ix);
        }
    }
    if let Some(from) = start {
        out.push(from..text.len());
    }
    out
}

/// Whether a word says enough to be worth marking: a longer word or a number.
fn is_telling(word: &str) -> bool {
    word.chars().count() >= 5 || word.chars().all(|ch| ch.is_ascii_digit())
}

/// The parts of a passage to mark: the given phrases, or failing those the
/// telling words it shares with the sentences that cite it.
fn quote_marks(quote: &str, phrases: &[SharedString], citing: &str) -> Vec<Range<usize>> {
    let mut marks: Vec<Range<usize>> = if phrases.is_empty() {
        let known: Vec<String> = words(citing)
            .into_iter()
            .map(|range| citing[range].to_lowercase())
            .filter(|word| is_telling(word))
            .collect();
        words(quote)
            .into_iter()
            .filter(|range| {
                let word = quote[range.clone()].to_lowercase();
                is_telling(&word) && known.contains(&word)
            })
            .collect()
    } else {
        phrases
            .iter()
            .filter(|phrase| !phrase.is_empty())
            .flat_map(|phrase| {
                quote
                    .match_indices(phrase.as_ref())
                    .map(|(at, found)| at..at + found.len())
            })
            .collect()
    };
    marks.sort_by_key(|range| range.start);

    // Join marks that overlap or have nothing but spaces between them.
    let mut joined: Vec<Range<usize>> = Vec::new();
    for mark in marks {
        match joined.last_mut() {
            Some(last)
                if mark.start <= last.end || quote[last.end..mark.start].trim().is_empty() =>
            {
                last.end = last.end.max(mark.end);
            }
            _ => joined.push(mark),
        }
    }
    joined
}

/// The chip an arrow key moves to from `current`, among `count` chips.
fn neighbour(current: usize, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match key {
        "right" => Some((current + 1) % count),
        "left" => Some((current + count - 1) % count),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

/// The letter that stands for a source on its badge.
fn initial(name: &str) -> String {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}

impl RenderOnce for SourcedAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let tint = accent.opacity(0.14);
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| AnswerState::default());
        let default_open = self
            .default_open
            .as_ref()
            .and_then(|id| self.sources.iter().position(|source| &source.id == id));
        let (open, pointed, copied) = state.update(cx, |state, _| {
            if !state.seeded {
                state.seeded = true;
                state.open = default_open;
            }
            // A source that went away cannot stay open or pointed at.
            let count = self.sources.len();
            state.open = state.open.filter(|source| *source < count);
            state.pointed = state.pointed.filter(|(source, _)| *source < count);
            (state.open, state.pointed, state.copied)
        });

        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let mut handles: Vec<FocusHandle> = Vec::new();
        for n in 0..self.sources.len() {
            let handle = window
                .use_keyed_state(child_id(format!("chip-focus-{n}")), cx, |_, cx| {
                    cx.focus_handle()
                })
                .read(cx)
                .clone();
            handles.push(handle);
        }
        let handles = Rc::new(handles);
        let focused = handles.iter().position(|handle| handle.is_focused(window));
        // The source everything lights up for: pointed at, focused, or open.
        let subject = pointed.map(|(source, _)| source).or(focused).or(open);
        let pointed_segment = pointed.and_then(|(_, segment)| segment);

        let ids: Rc<Vec<SharedString>> = Rc::new(
            self.sources
                .iter()
                .map(|source| source.id.clone())
                .collect(),
        );
        // Unfold a source's passage; asking for the open one folds it away.
        let show: ShowHandler = {
            let state = state.clone();
            let on_preview_change = self.on_preview_change.clone();
            Rc::new(move |source, window, cx| {
                let open = state.update(cx, |state, cx| {
                    state.open = source.filter(|source| state.open != Some(*source));
                    state.copied = false;
                    cx.notify();
                    state.open
                });
                if let Some(handler) = &on_preview_change {
                    handler(open.and_then(|ix| ids.get(ix)), window, cx);
                }
            })
        };

        let badge = |name: &SharedString| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(16.))
                .rounded(px(4.))
                .border_1()
                .border_color(line)
                .bg(fill)
                .text_size(px(10.))
                .text_color(quiet)
                .font_medium()
                .child(initial(name))
        };

        let body = paragraphs(&self.segments, &self.sources)
            .into_iter()
            .enumerate()
            .map(|(n, paragraph)| {
                let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
                for (range, segment) in &paragraph.sentences {
                    let cites_subject = subject.is_some_and(|subject| {
                        paragraph
                            .markers
                            .iter()
                            .any(|marker| marker.segment == *segment && marker.source == subject)
                    });
                    let tinted = match pointed_segment {
                        Some(pointed) => pointed == *segment,
                        None => cites_subject,
                    };
                    let faded = pointed_segment.is_none() && subject.is_some() && !cites_subject;
                    if tinted || faded {
                        highlights.push((
                            range.clone(),
                            HighlightStyle {
                                color: faded.then_some(quiet),
                                background_color: tinted.then_some(tint),
                                ..Default::default()
                            },
                        ));
                    }
                }
                for marker in &paragraph.markers {
                    let lit = subject == Some(marker.source);
                    highlights.push((
                        marker.range.clone(),
                        HighlightStyle {
                            color: Some(if lit { ink } else { quiet }),
                            background_color: Some(if lit { accent.opacity(0.28) } else { chip }),
                            ..Default::default()
                        },
                    ));
                }
                highlights.sort_by_key(|(range, _)| range.start);

                let ranges: Vec<Range<usize>> = paragraph
                    .markers
                    .iter()
                    .map(|marker| marker.range.clone())
                    .collect();
                let markers = Rc::new(paragraph.markers);
                InteractiveText::new(
                    child_id(format!("paragraph-{n}")),
                    StyledText::new(paragraph.text).with_highlights(highlights),
                )
                .on_click(ranges, {
                    let show = show.clone();
                    let markers = markers.clone();
                    move |ix, window, cx| {
                        if let Some(marker) = markers.get(ix) {
                            show(Some(marker.source), window, cx);
                        }
                    }
                })
                .on_hover({
                    let state = state.clone();
                    move |ix, _, _, cx| {
                        let pointed = ix
                            .and_then(|ix| markers.iter().find(|marker| marker.range.contains(&ix)))
                            .map(|marker| (marker.source, Some(marker.segment)));
                        state.update(cx, |state, cx| {
                            if state.pointed != pointed {
                                state.pointed = pointed;
                                cx.notify();
                            }
                        })
                    }
                })
            });
        let body: Vec<_> = body.collect();

        let chips = self.sources.iter().enumerate().map(|(n, source)| {
            let lit = subject == Some(n);
            div()
                .id(child_id(format!("chip-area-{n}")))
                .flex_none()
                .on_hover({
                    let state = state.clone();
                    move |hovered, _, cx| {
                        state.update(cx, |state, cx| {
                            let pointed = if *hovered {
                                Some((n, None))
                            } else if state.pointed == Some((n, None)) {
                                None
                            } else {
                                state.pointed
                            };
                            if state.pointed != pointed {
                                state.pointed = pointed;
                                cx.notify();
                            }
                        })
                    }
                })
                .child(
                    Pressable::new(child_id(format!("chip-{n}")))
                        .track_focus(&handles[n])
                        .selected(open == Some(n))
                        .accessibility_label(format!("Source {}: {}", n + 1, source.name))
                        .on_click({
                            let show = show.clone();
                            move |_, window, cx| show(Some(n), window, cx)
                        })
                        .gap_1p5()
                        .h(px(28.))
                        .px_2()
                        .rounded(radius)
                        .border_1()
                        .border_color(if lit { accent } else { line })
                        .when(open == Some(n), |this| this.bg(chip))
                        .text_caption()
                        .cursor_pointer()
                        .when(subject.is_some() && !lit, |this| this.opacity(0.55))
                        .when(focused == Some(n), |this| this.focus_ring_style(window, cx))
                        .child(badge(&source.name))
                        .child(
                            div()
                                .max_w(px(140.))
                                .truncate()
                                .font_medium()
                                .child(source.name.clone()),
                        )
                        .child(div().text_color(quiet).child((n + 1).to_string())),
                )
        });
        let chips: Vec<_> = chips.collect();

        let preview = open
            .and_then(|n| self.sources.get(n).map(|source| (n, source)))
            .map(|(n, source)| {
                let citing = self
                    .segments
                    .iter()
                    .filter(|segment| segment.cites.contains(&source.id))
                    .map(|segment| segment.text.as_ref())
                    .collect::<Vec<_>>()
                    .join(" ");
                let about = [source.site.clone(), source.date.clone()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let quote = source.quote.clone().map(|quote| {
                    let marks =
                        quote_marks(&quote, &source.marks, &citing)
                            .into_iter()
                            .map(|range| {
                                let style = HighlightStyle {
                                    color: Some(ink),
                                    background_color: Some(tint),
                                    ..Default::default()
                                };
                                (range, style)
                            });
                    div()
                        .pl_3()
                        .border_l_2()
                        .border_color(line)
                        .text_color(quiet)
                        .child(StyledText::new(quote).with_highlights(marks))
                });
                let can_open = source.url.is_some() || self.on_open_source.is_some();

                v_flex()
                    .gap_2()
                    .p_3()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(badge(&source.name))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(about),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .px_1()
                                    .rounded(px(4.))
                                    .bg(chip)
                                    .text_caption()
                                    .text_color(quiet)
                                    .child((n + 1).to_string()),
                            )
                            .child(
                                Button::new(child_id("close-passage".into()))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .accessibility_label("Close the passage")
                                    .on_click({
                                        let show = show.clone();
                                        move |_, window, cx| show(None, window, cx)
                                    }),
                            ),
                    )
                    .when_some(source.page_title.clone(), |this, title| {
                        this.child(div().font_medium().child(title))
                    })
                    .children(quote)
                    .child(
                        h_flex()
                            .gap_1()
                            .when(can_open, |this| {
                                this.child(
                                    Button::new(child_id("open-source".into()))
                                        .outline()
                                        .small()
                                        .icon(IconName::ExternalLink)
                                        .label("Open source")
                                        .on_click({
                                            let on_open = self.on_open_source.clone();
                                            let id = source.id.clone();
                                            let url = source.url.clone();
                                            move |_, window, cx| match (&on_open, &url) {
                                                (Some(on_open), _) => on_open(&id, window, cx),
                                                (None, Some(url)) => cx.open_url(url),
                                                (None, None) => {}
                                            }
                                        }),
                                )
                            })
                            .when_some(source.quote.clone(), |this, quote| {
                                this.child(
                                    Button::new(child_id("copy-quote".into()))
                                        .ghost()
                                        .small()
                                        .icon(IconName::Copy)
                                        .label("Copy quote")
                                        .on_click({
                                            let state = state.clone();
                                            move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    quote.to_string(),
                                                ));
                                                state.update(cx, |state, cx| {
                                                    state.copied = true;
                                                    state._copied_reset =
                                                        Some(cx.spawn(async move |this, cx| {
                                                            cx.background_executor()
                                                                .timer(COPIED_FOR)
                                                                .await;
                                                            this.update(cx, |state, cx| {
                                                                state.copied = false;
                                                                cx.notify();
                                                            })
                                                            .ok();
                                                        }));
                                                    cx.notify();
                                                })
                                            }
                                        }),
                                )
                            }),
                    )
            });

        let status: SharedString = if copied {
            "Quote copied".into()
        } else if let Some(source) = subject.and_then(|n| self.sources.get(n)) {
            let cited = self
                .segments
                .iter()
                .filter(|segment| segment.cites.contains(&source.id))
                .count();
            let cited = match cited {
                1 => "1 sentence".to_string(),
                n => format!("{n} sentences"),
            };
            format!("{} · cited in {cited}", source.name).into()
        } else {
            "Press a number to read its source".into()
        };
        let summary = match self.sources.len() {
            1 => "1 source".to_string(),
            n => format!("{n} sources"),
        };
        let summary = match self.pages_read {
            Some(pages) => format!("{summary} · {pages} pages read"),
            None => summary,
        };
        let source_count = self.sources.len();

        v_flex()
            .id(self.id.clone())
            .on_key_down({
                let show = show.clone();
                let handles = handles.clone();
                move |event, window, cx| {
                    if event.keystroke.key == "escape"
                        && let Some(open) = open
                    {
                        show(None, window, cx);
                        if let Some(handle) = handles.get(open) {
                            handle.focus(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }
            })
            .w_full()
            .max_w(px(460.))
            .gap_3()
            .p_4()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.6))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        h_flex().gap(px(2.)).children(
                            self.sources
                                .iter()
                                .take(3)
                                .map(|source| badge(&source.name)),
                        ),
                    )
                    .child(div().text_caption().text_color(quiet).child(summary)),
            )
            .child(
                div()
                    .id(child_id("answer".into()))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .on_hover({
                        let state = state.clone();
                        move |hovered, _, cx| {
                            if !hovered {
                                state.update(cx, |state, cx| {
                                    // Markers stop being pointed at when the pointer leaves the text.
                                    if state.pointed.is_some_and(|(_, segment)| segment.is_some()) {
                                        state.pointed = None;
                                        cx.notify();
                                    }
                                })
                            }
                        }
                    })
                    .children(body),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1p5()
                    .on_key_down({
                        let handles = handles.clone();
                        move |event, window, cx| {
                            let Some(current) =
                                handles.iter().position(|handle| handle.is_focused(window))
                            else {
                                return;
                            };
                            let key = event.keystroke.key.as_str();
                            if let Some(next) = neighbour(current, source_count, key) {
                                handles[next].focus(window, cx);
                                cx.stop_propagation();
                            }
                        }
                    })
                    .children(chips),
            )
            .children(preview)
            .child(
                h_flex()
                    .gap_2()
                    .pt_3()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_caption()
                            .text_color(quiet)
                            .child(status),
                    )
                    .when_some(self.action, |this, (label, handler)| {
                        this.child(
                            Button::new(child_id("action".into()))
                                .outline()
                                .small()
                                .label(label)
                                .on_click(move |_, window, cx| handler(window, cx)),
                        )
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Vec<AnswerSource> {
        vec![
            AnswerSource::new("ledger", "Ledger"),
            AnswerSource::new("contract", "Contract"),
        ]
    }

    #[test]
    fn markers_are_numbered_by_source_order() {
        let segments = [
            AnswerSegment::new("Freight went up.").cite("contract"),
            AnswerSegment::new(" Sales held.")
                .cite("ledger")
                .cite("contract"),
        ];
        let laid_out = paragraphs(&segments, &sources());
        assert_eq!(laid_out.len(), 1);
        let paragraph = &laid_out[0];
        assert_eq!(
            paragraph.text,
            "Freight went up.\u{a0}\u{a0}2\u{a0} Sales held.\u{a0}\u{a0}1\u{a0}\u{a0}\u{a0}2\u{a0}"
        );
        let numbers: Vec<_> = paragraph
            .markers
            .iter()
            .map(|marker| (marker.segment, marker.source))
            .collect();
        assert_eq!(numbers, [(0, 1), (1, 0), (1, 1)]);
        // The spaces at a segment's edges stay outside its sentence.
        let (range, _) = &paragraph.sentences[1];
        assert_eq!(&paragraph.text[range.clone()], "Sales held.");
    }

    #[test]
    fn unknown_citations_leave_no_marker() {
        let segments = [AnswerSegment::new("Margins fell.").cite("nowhere")];
        let laid_out = paragraphs(&segments, &sources());
        assert_eq!(laid_out[0].text, "Margins fell.");
        assert!(laid_out[0].markers.is_empty());
    }

    #[test]
    fn a_segment_can_start_a_paragraph() {
        let segments = [
            AnswerSegment::new("One."),
            AnswerSegment::new("Two.").new_paragraph(),
        ];
        let laid_out = paragraphs(&segments, &sources());
        assert_eq!(laid_out.len(), 2);
        assert_eq!(laid_out[1].sentences, [(0..4, 1)]);
    }

    #[test]
    fn shared_telling_words_are_marked() {
        let quote = "The carrier raised its rate by 12 percent in March.";
        let citing = "Freight cost more because the carrier put its RATE up 12 percent.";
        let marked: Vec<_> = quote_marks(quote, &[], citing)
            .into_iter()
            .map(|range| &quote[range])
            .collect();
        // "rate" is too short to count on its own; "12 percent" joins up.
        assert_eq!(marked, ["carrier", "12 percent"]);
    }

    #[test]
    fn given_phrases_win_over_shared_words() {
        let quote = "The carrier raised its rate by 12 percent in March.";
        let phrases = ["raised its rate".into()];
        let marked: Vec<_> = quote_marks(quote, &phrases, "carrier percent")
            .into_iter()
            .map(|range| &quote[range])
            .collect();
        assert_eq!(marked, ["raised its rate"]);
    }

    #[test]
    fn arrows_wrap_around_the_sources() {
        assert_eq!(neighbour(2, 3, "right"), Some(0));
        assert_eq!(neighbour(0, 3, "left"), Some(2));
        assert_eq!(neighbour(1, 3, "home"), Some(0));
        assert_eq!(neighbour(1, 3, "end"), Some(2));
        assert_eq!(neighbour(1, 3, "a"), None);
        assert_eq!(neighbour(0, 0, "right"), None);
    }

    #[test]
    fn a_badge_shows_the_first_letter() {
        assert_eq!(initial("ledger"), "L");
        assert_eq!(initial("  9 lives"), "9");
        assert_eq!(initial(""), "");
    }
}
