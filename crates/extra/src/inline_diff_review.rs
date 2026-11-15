//! Inline diff review: word-level changes to a text, accepted or rejected one at a time.
//!
//! The text reads as one paragraph with every change marked in place: what
//! goes is struck through in red, what comes is tinted green. One change is
//! current. A bar under the text says what it does and holds Reject and
//! Accept. Decided changes settle into plain text, so the paragraph turns
//! into the result as the review goes on.
//!
//! The review is a Tab stop. Left and Right (or K and J) move between the
//! changes still open, A or Enter accepts, R or Backspace rejects and U
//! reopens the current change.

use std::{ops::Range, rc::Rc};

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, FocusHandle, HighlightStyle, Hsla, InteractiveElement as _, InteractiveText,
    IntoElement, KeyDownEvent, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StrikethroughStyle, StyleRefinement, Styled, StyledText,
    UnderlineStyle, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One piece of a word-level diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffPart {
    /// Text both versions share.
    Same(String),
    /// Text that changes: `removed` goes and `added` comes. Either may be
    /// empty for a pure insertion or deletion.
    Change {
        /// What the first version says.
        removed: String,
        /// What the second version says.
        added: String,
    },
}

/// Split text into words and the spaces between them, keeping both.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut space = None;
    for (ix, ch) in text.char_indices() {
        let is_space = ch.is_whitespace();
        if space.is_some_and(|was| was != is_space) {
            out.push(&text[start..ix]);
            start = ix;
        }
        space = Some(is_space);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Compare two texts word by word.
///
/// Changes only separated by spaces are joined, so "within thirty" to "in
/// 30" is one change rather than two.
pub fn word_diff(before: &str, after: &str) -> Vec<DiffPart> {
    let a = tokens(before);
    let b = tokens(after);
    // Longest common subsequence over the tokens, filled from the end.
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut parts: Vec<DiffPart> = Vec::new();
    let push_change = |parts: &mut Vec<DiffPart>, removed: &str, added: &str| {
        if let Some(DiffPart::Change {
            removed: r,
            added: d,
        }) = parts.last_mut()
        {
            r.push_str(removed);
            d.push_str(added);
        } else {
            parts.push(DiffPart::Change {
                removed: removed.into(),
                added: added.into(),
            });
        }
    };
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            match parts.last_mut() {
                Some(DiffPart::Same(text)) => text.push_str(a[i]),
                _ => parts.push(DiffPart::Same(a[i].into())),
            }
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            push_change(&mut parts, "", b[j]);
            j += 1;
        } else {
            push_change(&mut parts, a[i], "");
            i += 1;
        }
    }

    // Join changes that only a space keeps apart.
    let mut joined: Vec<DiffPart> = Vec::new();
    for part in parts {
        let gap = match joined.as_slice() {
            [.., DiffPart::Change { .. }, DiffPart::Same(gap)]
                if gap.chars().all(char::is_whitespace)
                    && matches!(part, DiffPart::Change { .. }) =>
            {
                Some(gap.clone())
            }
            _ => None,
        };
        match (gap, part) {
            (Some(gap), DiffPart::Change { removed, added }) => {
                joined.pop();
                if let Some(DiffPart::Change {
                    removed: r,
                    added: d,
                }) = joined.last_mut()
                {
                    r.push_str(&gap);
                    r.push_str(&removed);
                    d.push_str(&gap);
                    d.push_str(&added);
                }
            }
            (_, part) => joined.push(part),
        }
    }
    joined
}

/// How many changes a diff has.
pub fn change_count(parts: &[DiffPart]) -> usize {
    parts
        .iter()
        .filter(|part| matches!(part, DiffPart::Change { .. }))
        .count()
}

/// The text once the review is applied: accepted changes take the new words,
/// rejected and open ones keep the old.
pub fn resolved_text(parts: &[DiffPart], decisions: &[Option<bool>]) -> String {
    let mut out = String::new();
    let mut change = 0;
    for part in parts {
        match part {
            DiffPart::Same(text) => out.push_str(text),
            DiffPart::Change { removed, added } => {
                let accepted = decisions.get(change).copied().flatten() == Some(true);
                out.push_str(if accepted { added } else { removed });
                change += 1;
            }
        }
    }
    out
}

/// The next open change after `from` going `forward`, wrapping around.
fn next_open(decisions: &[Option<bool>], from: usize, forward: bool) -> Option<usize> {
    let count = decisions.len();
    (1..=count)
        .map(|step| {
            if forward {
                (from + step) % count
            } else {
                (from + count - step % count) % count
            }
        })
        .find(|ix| decisions[*ix].is_none())
}

/// Shorten a change's text for the bar.
fn excerpt(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= 40 {
        text.to_string()
    } else {
        let cut: String = text.chars().take(38).collect();
        format!("{}\u{2026}", cut.trim_end())
    }
}

type DecideHandler = Rc<dyn Fn(usize, Option<bool>, &mut Window, &mut App)>;
type DecideAllHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A text with its proposed changes, reviewed one at a time.
#[derive(IntoElement)]
pub struct InlineDiffReview {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    parts: Vec<DiffPart>,
    decisions: Option<Vec<Option<bool>>>,
    on_decide: Option<DecideHandler>,
    on_decide_all: Option<DecideAllHandler>,
}

impl InlineDiffReview {
    /// Review the changes from `before` to `after`.
    pub fn new(id: impl Into<ElementId>, before: impl AsRef<str>, after: impl AsRef<str>) -> Self {
        Self::from_parts(id, word_diff(before.as_ref(), after.as_ref()))
    }

    /// Review a diff made elsewhere.
    pub fn from_parts(id: impl Into<ElementId>, parts: Vec<DiffPart>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Suggested edits".into(),
            parts,
            decisions: None,
            on_decide: None,
            on_decide_all: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Control the decisions, one per change: `Some(true)` accepted,
    /// `Some(false)` rejected, `None` open. Pair it with `on_decide`.
    pub fn decisions(mut self, decisions: Vec<Option<bool>>) -> Self {
        self.decisions = Some(decisions);
        self
    }

    /// Run when a change is accepted, rejected or reopened (`None`).
    pub fn on_decide(
        mut self,
        handler: impl Fn(usize, Option<bool>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }

    /// Run when Accept all or Reject all is pressed.
    pub fn on_decide_all(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide_all = Some(Rc::new(handler));
        self
    }
}

impl Styled for InlineDiffReview {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct ReviewState {
    current: usize,
    decisions: Vec<Option<bool>>,
    focus: FocusHandle,
}

/// The paragraph, its highlights and the range each change covers.
fn paint(
    parts: &[DiffPart],
    decisions: &[Option<bool>],
    current: usize,
    colors: (Hsla, Hsla, Hsla),
) -> (
    String,
    Vec<(Range<usize>, HighlightStyle)>,
    Vec<Range<usize>>,
) {
    let (danger, success, quiet) = colors;
    let mut text = String::new();
    let mut highlights = Vec::new();
    let mut ranges = Vec::new();
    let mut change = 0;
    for part in parts {
        match part {
            DiffPart::Same(same) => text.push_str(same),
            DiffPart::Change { removed, added } => {
                let start = text.len();
                let is_current = change == current;
                match decisions.get(change).copied().flatten() {
                    None => {
                        let lift = if is_current { 0.24 } else { 0.12 };
                        if !removed.is_empty() {
                            let from = text.len();
                            text.push_str(removed);
                            highlights.push((
                                from..text.len(),
                                HighlightStyle {
                                    color: Some(danger),
                                    background_color: Some(danger.opacity(lift)),
                                    strikethrough: Some(StrikethroughStyle {
                                        thickness: px(1.),
                                        color: Some(danger),
                                    }),
                                    ..Default::default()
                                },
                            ));
                        }
                        if !added.is_empty() {
                            // Keep a gap between the two, so the old and the
                            // new words don't read as one.
                            if !removed.is_empty() && !removed.ends_with(char::is_whitespace) {
                                text.push(' ');
                            }
                            let from = text.len();
                            text.push_str(added);
                            highlights.push((
                                from..text.len(),
                                HighlightStyle {
                                    color: Some(success),
                                    background_color: Some(success.opacity(lift)),
                                    underline: is_current.then_some(UnderlineStyle {
                                        thickness: px(1.),
                                        color: Some(success),
                                        wavy: false,
                                    }),
                                    ..Default::default()
                                },
                            ));
                        }
                    }
                    Some(accepted) => {
                        let kept = if accepted { added } else { removed };
                        let from = text.len();
                        text.push_str(kept);
                        if !kept.is_empty() {
                            highlights.push((
                                from..text.len(),
                                HighlightStyle {
                                    underline: Some(UnderlineStyle {
                                        thickness: px(1.),
                                        color: Some(if accepted {
                                            success.opacity(0.5)
                                        } else {
                                            quiet.opacity(0.5)
                                        }),
                                        wavy: false,
                                    }),
                                    ..Default::default()
                                },
                            ));
                        }
                    }
                }
                ranges.push(start..text.len());
                change += 1;
            }
        }
    }
    (text, highlights, ranges)
}

impl RenderOnce for InlineDiffReview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let danger = theme.danger;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let count = change_count(&self.parts);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| ReviewState {
            current: 0,
            decisions: vec![None; count],
            focus: cx.focus_handle().tab_stop(true),
        });
        if let Some(given) = &self.decisions {
            state.update(cx, |state, _| state.decisions = given.clone());
        }
        state.update(cx, |state, _| {
            state.decisions.resize(count, None);
            if state.current >= count {
                state.current = count.saturating_sub(1);
            }
        });
        let (current, decisions, focus) = {
            let state = state.read(cx);
            (state.current, state.decisions.clone(), state.focus.clone())
        };
        let open = decisions.iter().filter(|d| d.is_none()).count();
        let is_focused = focus.is_focused(window);

        // Deciding updates the kept state, moves to the next open change and
        // tells the owner.
        let decide: Rc<dyn Fn(usize, Option<bool>, &mut Window, &mut App)> = {
            let state = state.clone();
            let on_decide = self.on_decide.clone();
            Rc::new(move |ix, decision, window, cx| {
                state.update(cx, |state, cx| {
                    if let Some(slot) = state.decisions.get_mut(ix) {
                        *slot = decision;
                    }
                    if decision.is_some()
                        && let Some(next) = next_open(&state.decisions, ix, true)
                    {
                        state.current = next;
                    }
                    cx.notify();
                });
                if let Some(handler) = &on_decide {
                    handler(ix, decision, window, cx);
                }
            })
        };
        let decide_all: Rc<dyn Fn(bool, &mut Window, &mut App)> = {
            let state = state.clone();
            let on_all = self.on_decide_all.clone();
            Rc::new(move |accept, window, cx| {
                state.update(cx, |state, cx| {
                    for slot in &mut state.decisions {
                        if slot.is_none() {
                            *slot = Some(accept);
                        }
                    }
                    cx.notify();
                });
                if let Some(handler) = &on_all {
                    handler(accept, window, cx);
                }
            })
        };

        let (text, highlights, ranges) =
            paint(&self.parts, &decisions, current, (danger, success, quiet));
        let paragraph = InteractiveText::new(
            (self.id.clone(), "text"),
            StyledText::new(text).with_highlights(highlights),
        )
        .on_click(ranges, {
            let state = state.clone();
            let focus = focus.clone();
            move |ix, window, cx| {
                focus.focus(window, cx);
                state.update(cx, |state, cx| {
                    state.current = ix;
                    cx.notify();
                })
            }
        });

        let current_part = self
            .parts
            .iter()
            .filter_map(|part| match part {
                DiffPart::Change { removed, added } => Some((removed, added)),
                DiffPart::Same(_) => None,
            })
            .nth(current);
        let describe = current_part.map(|(removed, added)| match (removed.trim(), added.trim()) {
            ("", added) => format!("Add \u{201c}{}\u{201d}", excerpt(added)),
            (removed, "") => format!("Remove \u{201c}{}\u{201d}", excerpt(removed)),
            (removed, added) => format!(
                "\u{201c}{}\u{201d} \u{2192} \u{201c}{}\u{201d}",
                excerpt(removed),
                excerpt(added)
            ),
        });
        let current_decision = decisions.get(current).copied().flatten();

        let key_down = {
            let state = state.clone();
            let decide = decide.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.modifiers.modified() || count == 0 {
                    return;
                }
                let (current, decisions) = {
                    let state = state.read(cx);
                    (state.current, state.decisions.clone())
                };
                let step = |forward: bool, cx: &mut App| {
                    let next = next_open(&decisions, current, forward).unwrap_or_else(|| {
                        if forward {
                            (current + 1).min(count - 1)
                        } else {
                            current.saturating_sub(1)
                        }
                    });
                    state.update(cx, |state, cx| {
                        state.current = next;
                        cx.notify();
                    });
                };
                match event.keystroke.key.as_str() {
                    "right" | "down" | "j" => step(true, cx),
                    "left" | "up" | "k" => step(false, cx),
                    "a" | "enter" => decide(current, Some(true), window, cx),
                    "r" | "backspace" => decide(current, Some(false), window, cx),
                    "u" => decide(current, None, window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }
        };

        let bar = (count > 0).then(|| {
            let decide_reject = decide.clone();
            let decide_accept = decide.clone();
            let decide_undo = decide.clone();
            h_flex()
                .gap_2()
                .px_4()
                .py_2()
                .border_t_1()
                .border_color(line)
                .bg(chip.opacity(0.4))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(quiet)
                        .child(format!("{} of {count}", current + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .children(describe.clone()),
                )
                .map(|this| match current_decision {
                    None => this
                        .child(
                            Button::new((self.id.clone(), "reject"))
                                .outline()
                                .small()
                                .icon(IconName::Close)
                                .label("Reject")
                                .accessibility_label(format!(
                                    "Reject change {} of {count}",
                                    current + 1
                                ))
                                .on_click(move |_, window, cx| {
                                    decide_reject(current, Some(false), window, cx)
                                }),
                        )
                        .child(
                            Button::new((self.id.clone(), "accept"))
                                .primary()
                                .small()
                                .icon(IconName::Check)
                                .label("Accept")
                                .accessibility_label(format!(
                                    "Accept change {} of {count}",
                                    current + 1
                                ))
                                .on_click(move |_, window, cx| {
                                    decide_accept(current, Some(true), window, cx)
                                }),
                        ),
                    Some(accepted) => this
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(if accepted { success } else { quiet })
                                .child(if accepted { "Accepted" } else { "Rejected" }),
                        )
                        .child(
                            Button::new((self.id.clone(), "undo"))
                                .ghost()
                                .small()
                                .icon(IconName::Undo)
                                .label("Undo")
                                .on_click(move |_, window, cx| {
                                    decide_undo(current, None, window, cx)
                                }),
                        ),
                })
        });

        let label = format!("{}, {open} of {count} changes left", self.title);
        let reject_all = decide_all.clone();
        let accept_all = decide_all;
        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(560.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.6))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .pt_3()
                    .pb_1()
                    .child(Icon::new(IconName::Replace).small().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .child(self.title),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(quiet)
                            .child(if open == 0 {
                                "All reviewed".to_string()
                            } else {
                                format!("{open} left")
                            }),
                    )
                    .child(
                        Button::new((self.id.clone(), "reject-all"))
                            .ghost()
                            .xsmall()
                            .label("Reject all")
                            .disabled(open == 0)
                            .on_click(move |_, window, cx| reject_all(false, window, cx)),
                    )
                    .child(
                        Button::new((self.id.clone(), "accept-all"))
                            .ghost()
                            .xsmall()
                            .label("Accept all")
                            .disabled(open == 0)
                            .on_click(move |_, window, cx| accept_all(true, window, cx)),
                    ),
            )
            .child(
                div()
                    .id((self.id.clone(), "paragraph"))
                    .track_focus(&focus)
                    .aria_label(label)
                    .on_key_down(key_down)
                    .mx_2()
                    .mb_2()
                    .px_2()
                    .py_1p5()
                    .rounded(radius)
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    .child(paragraph),
            )
            .children(bar)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_spaces_split_apart() {
        assert_eq!(tokens("pay  in 30"), ["pay", "  ", "in", " ", "30"]);
    }

    #[test]
    fn a_swapped_word_is_one_change() {
        let parts = word_diff("pay in thirty days", "pay in 30 days");
        assert_eq!(
            parts,
            [
                DiffPart::Same("pay in ".into()),
                DiffPart::Change {
                    removed: "thirty".into(),
                    added: "30".into()
                },
                DiffPart::Same(" days".into()),
            ]
        );
    }

    #[test]
    fn neighbouring_changes_join() {
        let parts = word_diff("pay within thirty days", "pay in 30 days");
        assert_eq!(change_count(&parts), 1);
        assert_eq!(resolved_text(&parts, &[Some(true)]), "pay in 30 days");
        assert_eq!(
            resolved_text(&parts, &[Some(false)]),
            "pay within thirty days"
        );
    }

    #[test]
    fn insertions_and_deletions_resolve_both_ways() {
        let parts = word_diff("Thanks for the payment.", "Thanks again for the payment.");
        assert_eq!(change_count(&parts), 1);
        assert_eq!(resolved_text(&parts, &[None]), "Thanks for the payment.");
        assert_eq!(
            resolved_text(&parts, &[Some(true)]),
            "Thanks again for the payment."
        );
    }

    #[test]
    fn moving_skips_decided_changes() {
        let decisions = [Some(true), None, Some(false), None];
        assert_eq!(next_open(&decisions, 1, true), Some(3));
        assert_eq!(next_open(&decisions, 3, true), Some(1));
        assert_eq!(next_open(&decisions, 1, false), Some(3));
        assert_eq!(next_open(&[Some(true)], 0, true), None);
    }

    #[test]
    fn long_changes_are_cut_short() {
        assert_eq!(excerpt("short"), "short");
        assert!(excerpt(&"word ".repeat(20)).ends_with('\u{2026}'));
    }
}
