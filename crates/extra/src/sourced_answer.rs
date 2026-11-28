// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sourced answer: a finished answer wired to the sources behind it.
//!
//! Every cited sentence ends in numbered markers. Pointing at a marker tints
//! its sentence and lights up its source; pointing at a source tints every
//! sentence that cites it. Pressing either unfolds the quoted passage, with
//! the words it shares with the answer marked.
//!
//! Opt in to a preview card that floats by the pointer while it rests on a
//! marker, and to a panel that lists every source with what it is and how
//! often it is cited. Up and Down move through the panel; Enter opens one.

use std::{ops::Range, rc::Rc, time::Duration};

use isonomic_ui::component::look;

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ClipboardItem, ElementId, FocusHandle, HighlightStyle,
    InteractiveElement as _, InteractiveText, IntoElement, ParentElement as _, Pixels, Point,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    StyledText, Task, Window, anchored, deferred, div, point, prelude::FluentBuilder as _, px,
    relative,
};

use crate::common::resting_shadow;

#[cfg(test)]
mod tests;
mod text;

use text::{
    cited_words, initial, neighbour, panel_row_words, panel_step, paragraphs, quote_marks, snippet,
};

/// How long the status line says that the quote was copied.
const COPIED_FOR: Duration = Duration::from_secs(2);
/// How long the hover preview takes to fade in.
const PEEK_IN: Duration = Duration::from_millis(160);
/// How much of a passage the hover preview shows.
const PEEK_CHARS: usize = 150;

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
    hover_previews: bool,
    sources_panel: bool,
    default_panel_open: bool,
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
            hover_previews: false,
            sources_panel: false,
            default_panel_open: false,
        }
    }

    /// Float a preview of the source by the pointer while it rests on a
    /// numbered marker: its name, where it is from and the start of the
    /// passage. Pressing the marker still unfolds the whole passage.
    pub fn hover_previews(mut self, hover_previews: bool) -> Self {
        self.hover_previews = hover_previews;
        self
    }

    /// Offer a panel of all sources behind a button in the header: each one
    /// numbered, with its title, where it is from and how often it is cited.
    pub fn sources_panel(mut self, sources_panel: bool) -> Self {
        self.sources_panel = sources_panel;
        self
    }

    /// Start with the panel of all sources open.
    pub fn default_panel_open(mut self, open: bool) -> Self {
        self.default_panel_open = open;
        self
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
    /// Where the pointer came onto the marker the preview floats for.
    peek_at: Option<Point<Pixels>>,
    /// Whether the panel of all sources is open.
    panel_open: bool,
    /// Which panel row the pointer is over.
    panel_hovered: Option<usize>,
}

impl RenderOnce for SourcedAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let popover = theme.popover;
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
        let (open, pointed, copied, peek_at, panel_open, panel_hovered) =
            state.update(cx, |state, _| {
                if !state.seeded {
                    state.seeded = true;
                    state.open = default_open;
                    state.panel_open = self.default_panel_open;
                }
                // A source that went away cannot stay open or pointed at.
                let count = self.sources.len();
                state.open = state.open.filter(|source| *source < count);
                state.pointed = state.pointed.filter(|(source, _)| *source < count);
                if !self.sources_panel {
                    state.panel_open = false;
                }
                (
                    state.open,
                    state.pointed,
                    state.copied,
                    state.peek_at,
                    state.panel_open,
                    state.panel_hovered,
                )
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
                    move |ix, event, _, cx| {
                        let pointed = ix
                            .and_then(|ix| markers.iter().find(|marker| marker.range.contains(&ix)))
                            .map(|marker| (marker.source, Some(marker.segment)));
                        state.update(cx, |state, cx| {
                            if state.pointed != pointed {
                                state.pointed = pointed;
                                // The preview stays where the pointer came onto
                                // the marker, so it does not chase the pointer.
                                state.peek_at = pointed.map(|_| event.position);
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

        // The floating preview of the marker under the pointer.
        let peek = pointed
            .filter(|(_, segment)| self.hover_previews && segment.is_some())
            .zip(peek_at)
            .and_then(|((n, _), at)| self.sources.get(n).map(|source| (n, source, at)))
            .map(|(n, source, at)| {
                let about = [source.site.clone(), source.date.clone()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let card = v_flex()
                    .id(child_id(format!("peek-{n}")))
                    .role(Role::Tooltip)
                    .aria_label(format!("Source {}: {}", n + 1, source.name))
                    .w(px(280.))
                    .gap_1p5()
                    .p_3()
                    .rounded(radius_lg)
                    .bg(popover)
                    .shadow(look::surface::floating(cx))
                    .text_caption()
                    .line_height(px(16.))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(badge(&source.name))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_medium()
                                    .text_color(ink)
                                    .child(source.name.clone()),
                            )
                            .child(div().text_color(quiet).child((n + 1).to_string())),
                    )
                    .when(!about.is_empty(), |this| {
                        this.child(div().truncate().text_color(quiet).child(about))
                    })
                    .when_some(source.page_title.clone(), |this, title| {
                        this.child(div().font_medium().text_color(ink).child(title))
                    })
                    .when_some(source.quote.clone(), |this, quote| {
                        this.child(
                            div()
                                .pl_2()
                                .border_l_2()
                                .border_color(line)
                                .text_color(quiet)
                                .child(snippet(&quote, PEEK_CHARS)),
                        )
                    })
                    .child(div().text_color(quiet).child("Press to read the passage"));
                let card = if cx.reduce_motion() {
                    card.into_any_element()
                } else {
                    card.with_animation(
                        child_id(format!("peek-in-{n}")),
                        Animation::new(PEEK_IN),
                        |this, time| {
                            let shown = 1. - (1. - time).powi(3);
                            this.opacity(shown).mt(px(4. * (1. - shown)))
                        },
                    )
                    .into_any_element()
                };
                deferred(
                    anchored()
                        .position(at)
                        .offset(point(px(-12.), px(18.)))
                        .snap_to_window_with_margin(px(8.))
                        .child(card),
                )
                .with_priority(1)
            });

        // The panel of all sources, one pressable row each.
        let mut panel_handles: Vec<FocusHandle> = Vec::new();
        if panel_open {
            for n in 0..self.sources.len() {
                let handle = window
                    .use_keyed_state(child_id(format!("panel-focus-{n}")), cx, |_, cx| {
                        cx.focus_handle()
                    })
                    .read(cx)
                    .clone();
                panel_handles.push(handle);
            }
        }
        let panel_handles = Rc::new(panel_handles);
        let panel = panel_open.then(|| {
            let rows: Vec<_> = self
                .sources
                .iter()
                .enumerate()
                .map(|(n, source)| {
                    let about = [source.site.clone(), source.date.clone()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let cited = self
                        .segments
                        .iter()
                        .filter(|segment| segment.cites.contains(&source.id))
                        .count();
                    let is_open = open == Some(n);
                    let hover = look::hover_fade(
                        child_id(format!("panel-row-hover-{n}")),
                        panel_hovered == Some(n) || is_open,
                        window,
                        cx,
                    );
                    let focused = panel_handles[n].is_focused(window);
                    div()
                        .id(child_id(format!("panel-row-area-{n}")))
                        .on_hover({
                            let state = state.clone();
                            move |hovered, _, cx| {
                                state.update(cx, |state, cx| {
                                    let next = if *hovered {
                                        Some(n)
                                    } else if state.panel_hovered == Some(n) {
                                        None
                                    } else {
                                        state.panel_hovered
                                    };
                                    if state.panel_hovered != next {
                                        state.panel_hovered = next;
                                        cx.notify();
                                    }
                                })
                            }
                        })
                        .child(
                            Pressable::new(child_id(format!("panel-row-{n}")))
                                .track_focus(&panel_handles[n])
                                .selected(is_open)
                                .accessibility_label(panel_row_words(
                                    n + 1,
                                    &source.name,
                                    &about,
                                    cited,
                                    is_open,
                                ))
                                .on_click({
                                    let show = show.clone();
                                    move |_, window, cx| show(Some(n), window, cx)
                                })
                                .w_full()
                                .justify_start()
                                .items_start()
                                .gap_2p5()
                                .px_2()
                                .py_2()
                                .rounded(radius)
                                .bg(look::mix(fill, chip, hover))
                                .cursor_pointer()
                                .when(focused, |this| this.focus_ring_style(window, cx))
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(16.))
                                        .text_caption()
                                        .text_color(quiet)
                                        .text_right()
                                        .child((n + 1).to_string()),
                                )
                                .child(badge(&source.name))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div().truncate().font_medium().text_caption().child(
                                                source
                                                    .page_title
                                                    .clone()
                                                    .unwrap_or_else(|| source.name.clone()),
                                            ),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_caption()
                                                .text_color(quiet)
                                                .child(if about.is_empty() {
                                                    source.name.to_string()
                                                } else {
                                                    format!("{} · {about}", source.name)
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(cited_words(cited)),
                                ),
                        )
                })
                .collect();
            let count = rows.len();
            v_flex()
                .id(child_id("panel".into()))
                .role(Role::List)
                .aria_label(format!("All sources, {count}"))
                .gap_0p5()
                .p_1()
                .rounded(radius)
                .border_1()
                .border_color(line)
                .on_key_down({
                    let handles = panel_handles.clone();
                    move |event, window, cx| {
                        let Some(current) =
                            handles.iter().position(|handle| handle.is_focused(window))
                        else {
                            return;
                        };
                        let key = event.keystroke.key.as_str();
                        if let Some(next) = panel_step(current, count, key) {
                            handles[next].focus(window, cx);
                            cx.stop_propagation();
                        }
                    }
                })
                .children(rows)
        });
        let panel_toggle = self.sources_panel.then(|| {
            let state = state.clone();
            let count = self.sources.len();
            Button::new(child_id("panel-toggle".into()))
                .ghost()
                .xsmall()
                .icon(if panel_open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .label(if panel_open {
                    "Hide sources".to_string()
                } else {
                    format!("All {count}")
                })
                .accessibility_label(if panel_open {
                    "All sources, shown".to_string()
                } else {
                    format!("All sources, {count}, hidden")
                })
                .on_click(move |_, _, cx| {
                    state.update(cx, |state, cx| {
                        state.panel_open = !state.panel_open;
                        cx.notify();
                    })
                })
        });

        let status: SharedString = if copied {
            "Quote copied".into()
        } else if let Some(source) = subject.and_then(|n| self.sources.get(n)) {
            let cited = self
                .segments
                .iter()
                .filter(|segment| segment.cites.contains(&source.id))
                .count();
            format!("{} · {}", source.name, cited_words(cited)).into()
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
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_caption()
                            .text_color(quiet)
                            .child(summary),
                    )
                    .children(panel_toggle),
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
            .children(panel)
            .children(preview)
            .children(peek)
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
