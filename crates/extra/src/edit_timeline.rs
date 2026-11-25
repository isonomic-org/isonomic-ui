// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Edit timeline: each change to each file on a time axis, to watch live or replay.
//!
//! Every file gets a lane, and every edit is a mark on it, sized by how
//! many lines it touched and coloured by who made it. A playhead sweeps
//! across; edits past it are faded, so replaying shows the work build up.
//! Below, a scrubber, play and pause, and the edit at the playhead or the
//! one selected, with its lines added and removed.

use std::{cell::Cell, rc::Rc};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// One edit to one file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileEdit {
    /// Identifies it.
    pub id: SharedString,
    /// The file's id.
    pub file: SharedString,
    /// When, in seconds from the start.
    pub at: u32,
    /// Lines added.
    pub added: u32,
    /// Lines removed.
    pub removed: u32,
    /// Who made it, by an author id.
    pub by: SharedString,
    /// What it did, in a few words.
    pub summary: SharedString,
}

impl FileEdit {
    /// Create an edit.
    pub fn new(
        id: impl Into<SharedString>,
        file: impl Into<SharedString>,
        at: u32,
        by: impl Into<SharedString>,
        summary: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            file: file.into(),
            at,
            added: 0,
            removed: 0,
            by: by.into(),
            summary: summary.into(),
        }
    }

    /// Set the lines added and removed.
    pub fn lines(mut self, added: u32, removed: u32) -> Self {
        self.added = added;
        self.removed = removed;
        self
    }
}

/// The last edit at or before `at`, if any.
pub fn edit_at(edits: &[FileEdit], at: u32) -> Option<&FileEdit> {
    edits.iter().filter(|e| e.at <= at).max_by_key(|e| e.at)
}

/// Lines added and removed up to `at`.
pub fn totals_until(edits: &[FileEdit], at: u32) -> (u32, u32) {
    edits
        .iter()
        .filter(|e| e.at <= at)
        .fold((0, 0), |(a, r), e| (a + e.added, r + e.removed))
}

fn mmss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

type SeekHandler = Rc<dyn Fn(u32, &mut Window, &mut App)>;
type PlayHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type EditHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Files' edits on a time axis.
#[derive(IntoElement)]
pub struct EditTimeline {
    id: ElementId,
    style: StyleRefinement,
    files: Vec<(SharedString, SharedString)>,
    authors: Vec<(SharedString, SharedString)>,
    edits: Vec<FileEdit>,
    length: u32,
    playhead: u32,
    playing: bool,
    selected: Option<SharedString>,
    on_seek: Option<SeekHandler>,
    on_play: Option<PlayHandler>,
    on_select: Option<EditHandler>,
}

impl EditTimeline {
    /// Create a timeline for files and authors, each given as id and name.
    pub fn new(
        id: impl Into<ElementId>,
        files: Vec<(SharedString, SharedString)>,
        authors: Vec<(SharedString, SharedString)>,
        edits: Vec<FileEdit>,
    ) -> Self {
        let length = edits.iter().map(|e| e.at).max().unwrap_or(0).max(1);
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            files,
            authors,
            edits,
            length,
            playhead: length,
            playing: false,
            selected: None,
            on_seek: None,
            on_play: None,
            on_select: None,
        }
    }

    /// Where the playhead is, in seconds.
    pub fn playhead(mut self, at: u32) -> Self {
        self.playhead = at.min(self.length);
        self
    }

    /// Whether it's replaying.
    pub fn playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    /// The selected edit.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when the scrubber is clicked, with the time picked.
    pub fn on_seek(mut self, handler: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_seek = Some(Rc::new(handler));
        self
    }

    /// Run when play or pause is clicked, with whether to play.
    pub fn on_play(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_play = Some(Rc::new(handler));
        self
    }

    /// Run when an edit's mark is clicked.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for EditTimeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EditTimeline {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius_lg = theme.radius_lg;
        let palette: [Hsla; 4] = [theme.primary, theme.success, theme.warning, theme.info];
        let added_color = Tone::Success.color(cx);
        let removed_color = Tone::Danger.color(cx);

        let length = self.length as f32;
        let playhead = self.playhead;
        let head_share = playhead as f32 / length;
        let author_color = |by: &SharedString| {
            self.authors
                .iter()
                .position(|(id, _)| id == by)
                .map(|ix| palette[ix % palette.len()])
                .unwrap_or(quiet)
        };
        let author_name = |by: &SharedString| {
            self.authors
                .iter()
                .find(|(id, _)| id == by)
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| by.clone())
        };
        let biggest = self
            .edits
            .iter()
            .map(|e| e.added + e.removed)
            .max()
            .unwrap_or(1)
            .max(1);

        let lanes: Vec<_> = self
            .files
            .iter()
            .map(|(file_id, name)| {
                let marks: Vec<_> = self
                    .edits
                    .iter()
                    .filter(|e| &e.file == file_id)
                    .map(|edit| {
                        let size =
                            8. + 10. * ((edit.added + edit.removed) as f32 / biggest as f32).sqrt();
                        let future = edit.at > playhead;
                        let is_selected = self.selected.as_ref() == Some(&edit.id);
                        let select = self.on_select.clone();
                        let id = edit.id.clone();
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("edit-{}", edit.id).into(),
                            ))
                            .absolute()
                            .top(relative(0.5))
                            .left(relative(edit.at as f32 / length))
                            .ml(px(-size / 2.))
                            .mt(px(-size / 2.))
                            .size(px(size))
                            .rounded_full()
                            .bg(author_color(&edit.by))
                            .when(future, |this| this.opacity(0.2))
                            .when(is_selected, |this| this.border_2().border_color(ink))
                            .cursor_pointer()
                            .role(Role::Button)
                            .aria_label(format!(
                                "{} at {}: {}, +{} -{}",
                                author_name(&edit.by),
                                mmss(edit.at),
                                edit.summary,
                                edit.added,
                                edit.removed
                            ))
                            .aria_selected(is_selected)
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(&id, window, cx);
                                }
                            })
                    })
                    .collect();
                h_flex()
                    .h(px(30.))
                    .gap_2()
                    .child(
                        div()
                            .w(px(140.))
                            .flex_none()
                            .truncate()
                            .text_caption()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h_full()
                            .child(
                                div()
                                    .absolute()
                                    .top(relative(0.5))
                                    .left_0()
                                    .right_0()
                                    .h(px(1.))
                                    .bg(line),
                            )
                            .children(marks),
                    )
            })
            .collect();

        let seek = self.on_seek.clone();
        let length_u = self.length;
        let track = Rc::new(Cell::new(Bounds::default()));
        let measured = track.clone();
        let scrubber = div()
            .id(ElementId::NamedChild(
                self.id.clone().into(),
                "scrubber".into(),
            ))
            .relative()
            .flex_1()
            .h(px(18.))
            .cursor_pointer()
            .role(Role::Slider)
            .aria_label(format!(
                "Playhead at {} of {}",
                mmss(playhead),
                mmss(length_u)
            ))
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    let bounds = track.get();
                    if let Some(seek) = &seek
                        && bounds.size.width > px(0.)
                    {
                        let share = ((event.position.x - bounds.origin.x) / bounds.size.width)
                            .clamp(0., 1.);
                        seek((share * length_u as f32).round() as u32, window, cx);
                    }
                },
            )
            .child(
                canvas(move |bounds, _, _| measured.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .inset_0()
                    .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top(px(7.))
                    .left_0()
                    .right_0()
                    .h(px(4.))
                    .rounded_full()
                    .bg(chip),
            )
            .child(
                div()
                    .absolute()
                    .top(px(7.))
                    .left_0()
                    .h(px(4.))
                    .rounded_full()
                    .bg(accent)
                    .w(relative(head_share)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(relative(head_share))
                    .ml(px(-7.))
                    .size(px(14.))
                    .rounded_full()
                    .bg(fill)
                    .border_2()
                    .border_color(accent),
            );

        let play = {
            let on_play = self.on_play.clone();
            let playing = self.playing;
            Button::new(ElementId::NamedChild(self.id.clone().into(), "play".into()))
                .small()
                .ghost()
                .icon(if playing {
                    IconName::Pause
                } else {
                    IconName::Play
                })
                .accessibility_label(if playing {
                    "Pause replay"
                } else {
                    "Replay edits"
                })
                .on_click(move |_, window, cx| {
                    if let Some(on_play) = &on_play {
                        on_play(!playing, window, cx);
                    }
                })
        };

        let focus = self
            .selected
            .as_ref()
            .and_then(|id| self.edits.iter().find(|e| &e.id == id))
            .or_else(|| edit_at(&self.edits, playhead));
        let (added, removed) = totals_until(&self.edits, playhead);
        let done = self.edits.iter().filter(|e| e.at <= playhead).count();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(800.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .font_medium()
                            .child(format!("{done} of {} edits", self.edits.len())),
                    )
                    .child(
                        div()
                            .text_caption()
                            .text_color(added_color)
                            .child(format!("+{added}")),
                    )
                    .child(
                        div()
                            .text_caption()
                            .text_color(removed_color)
                            .child(format!("\u{2212}{removed}")),
                    )
                    .children(self.authors.iter().enumerate().map(|(ix, (_, name))| {
                        h_flex()
                            .gap_1()
                            .text_caption()
                            .text_color(quiet)
                            .child(
                                div()
                                    .size(px(8.))
                                    .rounded_full()
                                    .bg(palette[ix % palette.len()]),
                            )
                            .child(name.clone())
                    })),
            )
            .child(
                div().relative().px_4().py_2().children(lanes).child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(16. + 140. + 8.))
                        .right(px(16.))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(relative(head_share))
                                .w(px(1.))
                                .bg(accent.opacity(0.7)),
                        ),
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .child(play)
                    .child(
                        div()
                            .w(px(40.))
                            .text_caption()
                            .text_color(quiet)
                            .child(mmss(playhead)),
                    )
                    .child(scrubber)
                    .child(
                        div()
                            .w(px(40.))
                            .text_right()
                            .text_caption()
                            .text_color(quiet)
                            .child(mmss(length_u)),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .text_caption()
                    .map(|this| match focus {
                        Some(edit) => this
                            .child(div().size(px(8.)).rounded_full().bg(author_color(&edit.by)))
                            .child(div().font_medium().child(author_name(&edit.by)))
                            .child(div().text_color(quiet).child(mmss(edit.at)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(edit.summary.clone()),
                            )
                            .child(
                                div()
                                    .text_color(added_color)
                                    .child(format!("+{}", edit.added)),
                            )
                            .child(
                                div()
                                    .text_color(removed_color)
                                    .child(format!("\u{2212}{}", edit.removed)),
                            ),
                        None => this
                            .text_color(quiet)
                            .child(Icon::new(IconName::Time).xsmall())
                            .child("No edits yet at this point."),
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edits() -> Vec<FileEdit> {
        vec![
            FileEdit::new("a", "f1", 10, "x", "one").lines(5, 1),
            FileEdit::new("b", "f2", 30, "y", "two").lines(2, 0),
            FileEdit::new("c", "f1", 50, "x", "three").lines(0, 4),
        ]
    }

    #[test]
    fn the_playhead_picks_the_last_edit_before_it() {
        assert_eq!(edit_at(&edits(), 35).map(|e| e.id.as_ref()), Some("b"));
        assert!(edit_at(&edits(), 5).is_none());
    }

    #[test]
    fn totals_count_only_what_has_happened() {
        assert_eq!(totals_until(&edits(), 30), (7, 1));
        assert_eq!(totals_until(&edits(), 60), (7, 5));
    }
}
