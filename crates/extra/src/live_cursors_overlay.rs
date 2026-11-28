// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Live cursors overlay: who else is in a shared document, where their cursors are, what they've selected and what they suggest.
//!
//! The document is laid out word by word. Each collaborator, person or
//! assistant, has a colour: their selection is tinted in it, their cursor
//! is a bar of it with their name on a flag, and their suggestions show the
//! words they'd remove struck through beside the words they'd add. A list
//! at the side names everyone present and every open suggestion, with
//! accept and reject buttons.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
    px, relative,
};

use crate::common::resting_shadow;

/// Someone in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct Collaborator {
    /// Identifies them.
    pub id: SharedString,
    /// Their name.
    pub name: SharedString,
    /// Whether they're an assistant.
    pub agent: bool,
}

impl Collaborator {
    /// Create a person.
    pub fn person(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            agent: false,
        }
    }

    /// Create an assistant.
    pub fn agent(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            agent: true,
        }
    }
}

/// Where someone is: after a word, with an optional selection, in a paragraph.
#[derive(Debug, Clone, PartialEq)]
pub struct Presence {
    /// Who.
    pub who: SharedString,
    /// The paragraph.
    pub paragraph: usize,
    /// The word their cursor sits after.
    pub after_word: usize,
    /// The words they've selected, first to last, if any.
    pub selection: Option<(usize, usize)>,
}

/// A suggested change to some words.
#[derive(Debug, Clone, PartialEq)]
pub struct WordSuggestion {
    /// Identifies it.
    pub id: SharedString,
    /// Who suggests it.
    pub who: SharedString,
    /// The paragraph.
    pub paragraph: usize,
    /// The words it replaces, first to last.
    pub words: (usize, usize),
    /// What it would put there.
    pub replacement: SharedString,
}

/// What a word is covered by.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WordMarks {
    /// The collaborator whose selection covers it.
    pub selected_by: Option<usize>,
    /// The suggestion that would replace it.
    pub suggested: Option<usize>,
    /// Collaborators whose cursor sits right after it.
    pub cursors_after: Vec<usize>,
}

/// Marks for each word of a paragraph.
pub fn word_marks(
    words: usize,
    paragraph: usize,
    people: &[Collaborator],
    presence: &[Presence],
    suggestions: &[WordSuggestion],
) -> Vec<WordMarks> {
    let mut marks = vec![WordMarks::default(); words];
    for p in presence.iter().filter(|p| p.paragraph == paragraph) {
        let Some(who) = people.iter().position(|c| c.id == p.who) else {
            continue;
        };
        if let Some((a, b)) = p.selection {
            for mark in marks
                .iter_mut()
                .take(b.min(words.saturating_sub(1)) + 1)
                .skip(a)
            {
                mark.selected_by = Some(who);
            }
        }
        if let Some(mark) = marks.get_mut(p.after_word.min(words.saturating_sub(1))) {
            mark.cursors_after.push(who);
        }
    }
    for (ix, s) in suggestions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.paragraph == paragraph)
    {
        for mark in marks
            .iter_mut()
            .take(s.words.1.min(words.saturating_sub(1)) + 1)
            .skip(s.words.0)
        {
            mark.suggested = Some(ix);
        }
    }
    marks
}

type SuggestionHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;

/// A shared document with everyone's cursors, selections and suggestions.
#[derive(IntoElement)]
pub struct LiveCursorsOverlay {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    paragraphs: Vec<SharedString>,
    people: Vec<Collaborator>,
    presence: Vec<Presence>,
    suggestions: Vec<WordSuggestion>,
    on_resolve: Option<SuggestionHandler>,
}

impl LiveCursorsOverlay {
    /// Create an overlay on a document's paragraphs.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        paragraphs: Vec<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            paragraphs,
            people: Vec::new(),
            presence: Vec::new(),
            suggestions: Vec::new(),
            on_resolve: None,
        }
    }

    /// Set who's here and where they are.
    pub fn people(mut self, people: Vec<Collaborator>, presence: Vec<Presence>) -> Self {
        self.people = people;
        self.presence = presence;
        self
    }

    /// Set the open suggestions.
    pub fn suggestions(mut self, suggestions: Vec<WordSuggestion>) -> Self {
        self.suggestions = suggestions;
        self
    }

    /// Run when a suggestion is accepted (`true`) or rejected.
    pub fn on_resolve(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolve = Some(Rc::new(handler));
        self
    }
}

impl Styled for LiveCursorsOverlay {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LiveCursorsOverlay {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette: [Hsla; 5] = [
            theme.primary,
            theme.success,
            theme.warning,
            theme.info,
            theme.danger,
        ];
        let color = |who: usize| palette[who % palette.len()];

        let paragraphs: Vec<AnyElement> = self
            .paragraphs
            .iter()
            .enumerate()
            .map(|(px_ix, text)| {
                let words: Vec<&str> = text.split_whitespace().collect();
                let marks = word_marks(
                    words.len(),
                    px_ix,
                    &self.people,
                    &self.presence,
                    &self.suggestions,
                );
                let mut pieces: Vec<AnyElement> = Vec::new();
                let mut ix = 0;
                while ix < words.len() {
                    if let Some(s) = marks[ix].suggested {
                        let suggestion = &self.suggestions[s];
                        let who = self
                            .people
                            .iter()
                            .position(|c| c.id == suggestion.who)
                            .unwrap_or(0);
                        let end = suggestion.words.1.min(words.len() - 1);
                        let old = words[ix..=end].join(" ");
                        pieces.push(
                            h_flex()
                                .gap_1()
                                .px_0p5()
                                .rounded(px(3.))
                                .bg(color(who).opacity(0.08))
                                .border_b_1()
                                .border_color(color(who))
                                .child(div().line_through().text_color(quiet).child(old))
                                .child(
                                    div()
                                        .text_color(color(who))
                                        .child(suggestion.replacement.clone()),
                                )
                                .into_any_element(),
                        );
                        for after in marks[ix..=end].iter().flat_map(|m| m.cursors_after.iter()) {
                            pieces.push(cursor(*after, &self.people, color(*after), fill));
                        }
                        ix = end + 1;
                        continue;
                    }
                    let mark = &marks[ix];
                    pieces.push(
                        div()
                            .px_px()
                            .rounded(px(2.))
                            .when_some(mark.selected_by, |this, who| {
                                this.bg(color(who).opacity(0.22))
                            })
                            .child(words[ix].to_string())
                            .into_any_element(),
                    );
                    for after in &mark.cursors_after {
                        pieces.push(cursor(*after, &self.people, color(*after), fill));
                    }
                    ix += 1;
                }
                h_flex()
                    .flex_wrap()
                    .items_end()
                    .gap_x(px(4.))
                    .gap_y(px(6.))
                    .line_height(relative(1.6))
                    .children(pieces)
                    .into_any_element()
            })
            .collect();

        let people_rows: Vec<_> = self
            .people
            .iter()
            .enumerate()
            .map(|(ix, person)| {
                let place = self
                    .presence
                    .iter()
                    .find(|p| p.who == person.id)
                    .map(|p| match p.selection {
                        Some((a, b)) => format!(
                            "selecting {} words in paragraph {}",
                            b + 1 - a,
                            p.paragraph + 1
                        ),
                        None => format!("in paragraph {}", p.paragraph + 1),
                    })
                    .unwrap_or_else(|| "away".into());
                h_flex()
                    .gap_2()
                    .text_caption()
                    .child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(color(ix))
                            .child(
                                Icon::new(if person.agent {
                                    IconName::Bot
                                } else {
                                    IconName::User
                                })
                                .xsmall()
                                .text_color(fill),
                            ),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(div().font_medium().truncate().child(person.name.clone()))
                            .child(div().text_color(quiet).truncate().child(place)),
                    )
            })
            .collect();

        let suggestion_rows: Vec<_> = self
            .suggestions
            .iter()
            .map(|s| {
                let who = self.people.iter().position(|c| c.id == s.who).unwrap_or(0);
                let name = self
                    .people
                    .get(who)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let words: Vec<&str> = self
                    .paragraphs
                    .get(s.paragraph)
                    .map(|p| p.split_whitespace().collect())
                    .unwrap_or_default();
                let old = words
                    .get(s.words.0..=s.words.1.min(words.len().saturating_sub(1)))
                    .map(|w| w.join(" "))
                    .unwrap_or_default();
                let (accept, reject) = (self.on_resolve.clone(), self.on_resolve.clone());
                let (a_id, r_id) = (s.id.clone(), s.id.clone());
                v_flex()
                    .gap_1()
                    .p_2()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .border_l_2()
                    .border_color(color(who))
                    .text_caption()
                    .child(div().font_medium().child(name))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(div().line_through().text_color(quiet).child(old))
                            .child("\u{2192}")
                            .child(div().text_color(ink).child(s.replacement.clone())),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .justify_end()
                            .child(
                                Button::new(ElementId::NamedChild(
                                    self.id.clone().into(),
                                    format!("reject-{}", s.id).into(),
                                ))
                                .xsmall()
                                .ghost()
                                .label("Reject")
                                .on_click(move |_, window, cx| {
                                    if let Some(reject) = &reject {
                                        reject(&r_id, false, window, cx);
                                    }
                                }),
                            )
                            .child(
                                Button::new(ElementId::NamedChild(
                                    self.id.clone().into(),
                                    format!("accept-{}", s.id).into(),
                                ))
                                .xsmall()
                                .primary()
                                .label("Accept")
                                .on_click(move |_, window, cx| {
                                    if let Some(accept) = &accept {
                                        accept(&a_id, true, window, cx);
                                    }
                                }),
                            ),
                    )
            })
            .collect();

        h_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(900.))
            .items_start()
            .flex_wrap()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .flex_grow(3.)
                    .w(px(420.))
                    .max_w_full()
                    .gap_4()
                    .p_5()
                    .child(div().text_lg().font_medium().child(self.title.clone()))
                    .children(paragraphs),
            )
            .child(
                v_flex()
                    .flex_grow(1.)
                    .w(px(220.))
                    .max_w_full()
                    .gap_3()
                    .p_4()
                    .border_l_1()
                    .border_color(line)
                    .child(
                        div()
                            .text_label_small()
                            .text_color(quiet)
                            .child(format!("{} here", self.people.len())),
                    )
                    .children(people_rows)
                    .when(!self.suggestions.is_empty(), |this| {
                        this.child(
                            div()
                                .pt_2()
                                .text_label_small()
                                .text_color(quiet)
                                .child(format!("{} suggestions", self.suggestions.len())),
                        )
                        .children(suggestion_rows)
                    }),
            )
            .refine_style(&self.style)
    }
}

fn cursor(who: usize, people: &[Collaborator], color: Hsla, fill: Hsla) -> AnyElement {
    let name = people.get(who).map(|p| p.name.clone()).unwrap_or_default();
    div()
        .relative()
        .w(px(2.))
        .h(px(20.))
        .bg(color)
        .child(
            div()
                .absolute()
                .bottom(px(18.))
                .left_0()
                .px_1()
                .rounded(px(3.))
                .bg(color)
                .text_color(fill)
                .text_label_small()
                .whitespace_nowrap()
                .child(name),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_cover_selections_suggestions_and_cursors() {
        let people = vec![
            Collaborator::person("m", "Maya"),
            Collaborator::agent("r", "Reconciler"),
        ];
        let presence = vec![
            Presence {
                who: "m".into(),
                paragraph: 0,
                after_word: 3,
                selection: Some((1, 3)),
            },
            Presence {
                who: "r".into(),
                paragraph: 1,
                after_word: 0,
                selection: None,
            },
        ];
        let suggestions = vec![WordSuggestion {
            id: "s".into(),
            who: "r".into(),
            paragraph: 0,
            words: (5, 6),
            replacement: "x".into(),
        }];
        let marks = word_marks(8, 0, &people, &presence, &suggestions);
        assert_eq!(marks[0].selected_by, None);
        assert_eq!(marks[2].selected_by, Some(0));
        assert_eq!(marks[3].cursors_after, [0]);
        assert_eq!(marks[6].suggested, Some(0));
        assert!(marks[4].suggested.is_none());
    }
}
