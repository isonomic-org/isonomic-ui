// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Comments panel: a sidebar feed of comments on one piece of work.
//!
//! Tabs narrow the feed to questions or to edits. A comment that starts with
//! `/edit` is a request an assistant can carry out, so it gets a Run button
//! that shows the edit running and then applied. Comments can be resolved,
//! and the composer at the bottom posts new ones.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, HighlightStyle,
    Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, Subscription, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often the "5min ago" times redraw.
const TICK: Duration = Duration::from_secs(20);
/// The prefix that turns a comment into an edit request.
const EDIT_PREFIX: &str = "/edit";

/// Who wrote a comment.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentAuthor {
    name: SharedString,
    tint: Option<Hsla>,
}

impl CommentAuthor {
    /// Create an author. Their initials stand in for a picture.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            tint: None,
        }
    }

    /// Colour their initials circle.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }
}

/// One comment in the feed.
#[derive(Debug, Clone)]
pub struct PanelComment {
    id: SharedString,
    author: CommentAuthor,
    posted: Instant,
    text: SharedString,
    resolved: bool,
}

impl PanelComment {
    /// Create a comment. Text starting with `/edit` becomes a runnable edit.
    pub fn new(
        id: impl Into<SharedString>,
        author: CommentAuthor,
        posted: Instant,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            author,
            posted,
            text: text.into(),
            resolved: false,
        }
    }

    /// Start it resolved.
    pub fn resolved(mut self, resolved: bool) -> Self {
        self.resolved = resolved;
        self
    }

    /// The comment's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// What it says.
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    /// The edit it asks for, without the prefix, if it is an edit.
    pub fn edit_request(&self) -> Option<&str> {
        edit_request(&self.text)
    }
}

/// What people did in the panel.
#[derive(Debug, Clone)]
pub enum CommentsPanelEvent {
    /// A comment was posted from the composer.
    Added(PanelComment),
    /// Run was pressed on an edit. Call [`CommentsPanel::finish_edit`] when
    /// it is done.
    RunEdit(PanelComment),
    /// A comment was resolved, or opened again.
    Resolved(SharedString, bool),
    /// The close button was pressed.
    Close,
}

/// Which comments the tabs show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Questions,
    Edits,
}

/// Where an edit stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditState {
    Running,
    Applied,
}

/// The edit a comment asks for, if it starts with `/edit`.
fn edit_request(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let head = trimmed.get(..EDIT_PREFIX.len())?;
    head.eq_ignore_ascii_case(EDIT_PREFIX)
        .then(|| trimmed[EDIT_PREFIX.len()..].trim_start())
}

/// Whether a comment reads as a question.
fn is_question(text: &str) -> bool {
    edit_request(text).is_none() && text.trim_end().ends_with('?')
}

/// "now", "5min ago", "2h ago", "3d ago".
fn ago_words(ago: Duration) -> String {
    let mins = ago.as_secs() / 60;
    if mins == 0 {
        "now".into()
    } else if mins < 60 {
        format!("{mins}min ago")
    } else if mins < 60 * 24 {
        format!("{}h ago", mins / 60)
    } else {
        format!("{}d ago", mins / (60 * 24))
    }
}

/// Up to two initials: "Mara Lind" gives "ML".
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// A comments sidebar with filters, runnable edits, resolve and a composer.
pub struct CommentsPanel {
    title: SharedString,
    me: CommentAuthor,
    comments: Vec<PanelComment>,
    filter: Filter,
    newest_first: bool,
    show_resolved: bool,
    closable: bool,
    edits: HashMap<SharedString, EditState>,
    written: usize,
    input: Entity<InputState>,
    focus: FocusHandle,
    _ticker: Task<()>,
    _input_events: Subscription,
}

impl EventEmitter<CommentsPanelEvent> for CommentsPanel {}

impl Focusable for CommentsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CommentsPanel {
    /// Create an empty panel. New comments are written by `me`.
    pub fn new(me: CommentAuthor, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Add a comment or /edit"));
        let input_events =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.send(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            });
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            title: "Comments".into(),
            me,
            comments: Vec::new(),
            filter: Filter::All,
            newest_first: true,
            show_resolved: false,
            closable: false,
            edits: HashMap::new(),
            written: 0,
            input,
            focus: cx.focus_handle(),
            _ticker: ticker,
            _input_events: input_events,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Start with a comment.
    pub fn comment(mut self, comment: PanelComment) -> Self {
        self.comments.push(comment);
        self
    }

    /// Show a close button in the heading.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Add a comment, such as one that arrived from someone else.
    pub fn push(&mut self, comment: PanelComment, cx: &mut Context<Self>) {
        self.comments.push(comment);
        cx.notify();
    }

    /// Say that an edit has been applied. Its button shows that it is done.
    pub fn finish_edit(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(state) = self.edits.get_mut(id) {
            *state = EditState::Applied;
        }
        cx.notify();
    }

    /// The comments as they stand.
    pub fn comments(&self) -> &[PanelComment] {
        &self.comments
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.written += 1;
        let comment = PanelComment::new(
            format!("mine-{}", self.written),
            self.me.clone(),
            Instant::now(),
            text,
        );
        self.comments.push(comment.clone());
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.emit(CommentsPanelEvent::Added(comment));
        cx.notify();
    }

    fn run_edit(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(comment) = self.comments.iter().find(|comment| &comment.id == id) {
            self.edits.insert(id.clone(), EditState::Running);
            cx.emit(CommentsPanelEvent::RunEdit(comment.clone()));
            cx.notify();
        }
    }

    fn toggle_resolved(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(comment) = self.comments.iter_mut().find(|comment| &comment.id == id) {
            comment.resolved = !comment.resolved;
            cx.emit(CommentsPanelEvent::Resolved(id.clone(), comment.resolved));
            cx.notify();
        }
    }

    fn shown(&self) -> Vec<&PanelComment> {
        let mut shown: Vec<&PanelComment> = self
            .comments
            .iter()
            .filter(|comment| self.show_resolved || !comment.resolved)
            .filter(|comment| match self.filter {
                Filter::All => true,
                Filter::Questions => is_question(&comment.text),
                Filter::Edits => edit_request(&comment.text).is_some(),
            })
            .collect();
        shown.sort_by_key(|comment| comment.posted);
        if self.newest_first {
            shown.reverse();
        }
        shown
    }
}

impl Render for CommentsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let success = theme.success;
        let radius_lg = theme.radius_lg;
        let tints = [
            theme.blue,
            theme.magenta,
            theme.green,
            theme.cyan,
            theme.yellow,
        ];
        let now = Instant::now();
        let this = cx.entity();

        let open_edits = self
            .comments
            .iter()
            .filter(|comment| !comment.resolved && edit_request(&comment.text).is_some())
            .filter(|comment| self.edits.get(&comment.id) != Some(&EditState::Applied))
            .count();
        let tab =
            |label: &'static str, which: Filter, count: Option<usize>, cx: &mut Context<Self>| {
                let picked = self.filter == which;
                Button::new(label)
                    .small()
                    .label(label)
                    .map(|this| if picked { this.outline() } else { this.ghost() })
                    .when_some(count.filter(|count| *count > 0), |this, count| {
                        this.child(
                            div()
                                .ml_1()
                                .px_1()
                                .rounded(px(4.))
                                .bg(chip)
                                .text_xs()
                                .text_color(quiet)
                                .child(count.to_string()),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter = which;
                        cx.notify();
                    }))
            };

        let sort = {
            let (newest, show_resolved) = (self.newest_first, self.show_resolved);
            let (a, b, c) = (this.clone(), this.clone(), this);
            Button::new("comments-sort")
                .ghost()
                .small()
                .icon(IconName::SortDescending)
                .accessibility_label("Sort and show")
                .dropdown_menu(move |menu, _, _| {
                    let (a, b, c) = (a.clone(), b.clone(), c.clone());
                    menu.item(PopupMenuItem::new("Newest first").checked(newest).on_click(
                        move |_, _, cx| {
                            a.update(cx, |this, cx| {
                                this.newest_first = true;
                                cx.notify();
                            })
                        },
                    ))
                    .item(
                        PopupMenuItem::new("Oldest first")
                            .checked(!newest)
                            .on_click(move |_, _, cx| {
                                b.update(cx, |this, cx| {
                                    this.newest_first = false;
                                    cx.notify();
                                })
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Show resolved")
                            .checked(show_resolved)
                            .on_click(move |_, _, cx| {
                                c.update(cx, |this, cx| {
                                    this.show_resolved = !this.show_resolved;
                                    cx.notify();
                                })
                            }),
                    )
                })
        };

        let rows: Vec<_> =
            self.shown()
                .into_iter()
                .enumerate()
                .map(|(ix, comment)| {
                    let tint = comment.author.tint.unwrap_or_else(|| {
                        let sum: usize = comment.author.name.bytes().map(usize::from).sum();
                        tints[sum % tints.len()]
                    });
                    let edit = edit_request(&comment.text);
                    let edit_state = self.edits.get(&comment.id).copied();
                    let id = comment.id.clone();
                    let resolve_id = comment.id.clone();
                    let text = match edit {
                        // One run of text, with the prefix quieter.
                        Some(request) => StyledText::new(format!("{EDIT_PREFIX} {request}"))
                            .with_highlights([(
                                0..EDIT_PREFIX.len(),
                                HighlightStyle {
                                    color: Some(quiet),
                                    ..Default::default()
                                },
                            )])
                            .into_any_element(),
                        None => div().child(comment.text.clone()).into_any_element(),
                    };
                    let run = edit.map(|_| match edit_state {
                        None => Button::new(("comment-run", ix))
                            .outline()
                            .xsmall()
                            .icon(IconName::Play)
                            .label("Run edit")
                            .disabled(comment.resolved)
                            .on_click(cx.listener(move |this, _, _, cx| this.run_edit(&id, cx)))
                            .into_any_element(),
                        Some(EditState::Running) => h_flex()
                            .gap_1p5()
                            .text_xs()
                            .text_color(quiet)
                            .child(Spinner::new().xsmall())
                            .child("Running")
                            .into_any_element(),
                        Some(EditState::Applied) => h_flex()
                            .gap_1()
                            .text_xs()
                            .text_color(success)
                            .child(Icon::new(IconName::CircleCheck).xsmall())
                            .child("Applied")
                            .into_any_element(),
                    });
                    v_flex()
                        .id(("comment", ix))
                        .gap_1()
                        .px_4()
                        .py_2p5()
                        .border_b_1()
                        .border_color(line)
                        .when(comment.resolved, |this| this.opacity(0.55))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_none()
                                        .size(px(22.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(tint.opacity(0.18))
                                        .text_color(tint)
                                        .text_xs()
                                        .font_semibold()
                                        .child(initials(&comment.author.name)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_medium()
                                        .child(comment.author.name.clone()),
                                )
                                .child(div().flex_none().text_xs().text_color(quiet).child(
                                    ago_words(now.saturating_duration_since(comment.posted)),
                                ))
                                .child(
                                    Button::new(("comment-resolve", ix))
                                        .ghost()
                                        .xsmall()
                                        .icon(if comment.resolved {
                                            IconName::Undo2
                                        } else {
                                            IconName::Check
                                        })
                                        .accessibility_label(if comment.resolved {
                                            "Open again"
                                        } else {
                                            "Resolve"
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_resolved(&resolve_id, cx)
                                        })),
                                ),
                        )
                        .child(div().pl(px(30.)).line_height(relative(1.45)).child(text))
                        .children(run.map(|run| h_flex().pl(px(30.)).pt_0p5().child(run)))
                })
                .collect();
        let empty = rows.is_empty();
        let typed = !self.input.read(cx).value().trim().is_empty();
        let me_tint = self.me.tint.unwrap_or(accent);

        v_flex()
            .key_context("CommentsPanel")
            .track_focus(&self.focus)
            .w_full()
            .max_w(px(420.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_2()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().font_semibold().child(self.title.clone()))
                            .when(self.closable, |this| {
                                this.child(
                                    Button::new("comments-close")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Close)
                                        .accessibility_label("Close")
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.emit(CommentsPanelEvent::Close)
                                        })),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(tab("All", Filter::All, None, cx))
                            .child(tab("Questions", Filter::Questions, None, cx))
                            .child(tab("Edits", Filter::Edits, Some(open_edits), cx))
                            .child(div().flex_1())
                            .child(sort),
                    ),
            )
            .child(
                v_flex()
                    .id("comments-feed")
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .children(rows)
                    .when(empty, |this| {
                        this.child(div().px_4().py_6().text_center().text_color(quiet).child(
                            match self.filter {
                                Filter::All => "No comments yet",
                                Filter::Questions => "No open questions",
                                Filter::Edits => "No edits to run",
                            },
                        ))
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        div()
                            .flex_none()
                            .size(px(22.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(me_tint.opacity(0.18))
                            .text_color(me_tint)
                            .text_xs()
                            .font_semibold()
                            .child(initials(&self.me.name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.input).small().appearance(false)),
                    )
                    .child(
                        Button::new("comments-send")
                            .primary()
                            .xsmall()
                            .icon(IconName::ArrowUp)
                            .accessibility_label("Send")
                            .disabled(!typed)
                            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_are_found_by_their_prefix() {
        assert_eq!(
            edit_request("/edit Round to whole numbers"),
            Some("Round to whole numbers")
        );
        assert_eq!(edit_request("  /Edit fix the dates"), Some("fix the dates"));
        assert_eq!(edit_request("Please edit this"), None);
        assert_eq!(edit_request("/ed"), None);
    }

    #[test]
    fn questions_end_with_a_question_mark() {
        assert!(is_question("Can we close March on Friday?"));
        assert!(!is_question("/edit Why is this here?"));
        assert!(!is_question("Looks right."));
    }

    #[test]
    fn small_words_for_people_and_times() {
        assert_eq!(initials("Mara Lind"), "ML");
        assert_eq!(initials("ana"), "A");
        assert_eq!(ago_words(Duration::from_secs(30)), "now");
        assert_eq!(ago_words(Duration::from_secs(300)), "5min ago");
        assert_eq!(ago_words(Duration::from_secs(7300)), "2h ago");
    }
}
