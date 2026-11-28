// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! File change review: the files a change touches beside their hunks, each hunk kept or reverted, and one action to apply what is kept.
//!
//! The split view groups files by folder in a tree: Up and Down move through
//! it, Left and Right close and open a folder, Enter opens a file. The
//! stacked view puts the files in a row of tabs instead. Every hunk starts
//! kept; reverting one dims it and takes it out of the count in the footer.
//! Apply reports the hunks still kept, file by file.

use std::{cell::Cell, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    rolling_number::RollingNumber,
    tab::{Tab, TabBar},
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, RenderOnce, Role,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _, px,
};

use crate::common::{inset, panel, resting_shadow, shell};

/// How long a file's hunks take to ease in, and a reverted hunk to dim.
const EASE: Duration = Duration::from_millis(200);

/// What happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// A new file.
    Added,
    /// An existing file with edits.
    Modified,
    /// A file that goes away.
    Deleted,
    /// A file that moved.
    Renamed,
}

impl ChangeKind {
    /// The letter in the file's badge.
    pub fn letter(self) -> &'static str {
        match self {
            ChangeKind::Added => "A",
            ChangeKind::Modified => "M",
            ChangeKind::Deleted => "D",
            ChangeKind::Renamed => "R",
        }
    }

    /// The word a screen reader hears for the badge.
    pub fn word(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Modified => "modified",
            ChangeKind::Deleted => "deleted",
            ChangeKind::Renamed => "renamed",
        }
    }
}

/// Which side of the change a line is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Only in the new file.
    Added,
    /// Only in the old file.
    Removed,
    /// In both.
    Context,
}

/// One line of a hunk.
#[derive(Debug, Clone)]
pub struct HunkLine {
    kind: LineKind,
    text: SharedString,
}

impl HunkLine {
    /// A line the change adds.
    pub fn added(text: impl Into<SharedString>) -> Self {
        Self {
            kind: LineKind::Added,
            text: text.into(),
        }
    }

    /// A line the change removes.
    pub fn removed(text: impl Into<SharedString>) -> Self {
        Self {
            kind: LineKind::Removed,
            text: text.into(),
        }
    }

    /// A line the change leaves alone.
    pub fn context(text: impl Into<SharedString>) -> Self {
        Self {
            kind: LineKind::Context,
            text: text.into(),
        }
    }
}

/// A run of changed lines with the lines around them.
#[derive(Debug, Clone)]
pub struct Hunk {
    id: SharedString,
    old_start: u32,
    new_start: u32,
    lines: Vec<HunkLine>,
}

impl Hunk {
    /// A hunk starting at these line numbers of the old and the new file.
    pub fn new(id: impl Into<SharedString>, old_start: u32, new_start: u32) -> Self {
        Self {
            id: id.into(),
            old_start,
            new_start,
            lines: Vec::new(),
        }
    }

    /// Its lines, top to bottom.
    pub fn lines(mut self, lines: impl IntoIterator<Item = HunkLine>) -> Self {
        self.lines.extend(lines);
        self
    }
}

/// One file of the change.
#[derive(Debug, Clone)]
pub struct FileChange {
    id: SharedString,
    path: SharedString,
    from: Option<SharedString>,
    kind: ChangeKind,
    hunks: Vec<Hunk>,
}

impl FileChange {
    /// A file at `path`, folders separated by `/`.
    pub fn new(
        id: impl Into<SharedString>,
        path: impl Into<SharedString>,
        kind: ChangeKind,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            from: None,
            kind,
            hunks: Vec::new(),
        }
    }

    /// Where a renamed file used to be.
    pub fn from(mut self, path: impl Into<SharedString>) -> Self {
        self.from = Some(path.into());
        self
    }

    /// Add a hunk.
    pub fn hunk(mut self, hunk: Hunk) -> Self {
        self.hunks.push(hunk);
        self
    }
}

/// The hunks of one file that are still kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptHunks {
    /// The file's id.
    pub file: SharedString,
    /// Ids of the hunks to apply. Empty when every hunk was reverted.
    pub hunks: Vec<SharedString>,
}

/// A hunk that was just kept or reverted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkChoice {
    /// The file's id.
    pub file: SharedString,
    /// The hunk's id.
    pub hunk: SharedString,
    /// Kept, or reverted.
    pub keep: bool,
}

/// A row of the file tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRow {
    /// A folder, by its path.
    Folder(SharedString),
    /// A file, by its id.
    File(SharedString),
}

/// A path as its folder and its file name. A file at the top is in ".".
pub fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(at) => (&path[..at], &path[at + 1..]),
        None => (".", path),
    }
}

/// How many lines the hunks add and remove.
pub fn line_counts(hunks: &[Hunk]) -> (usize, usize) {
    let count = |kind| {
        hunks
            .iter()
            .flat_map(|hunk| &hunk.lines)
            .filter(|line| line.kind == kind)
            .count()
    };
    (count(LineKind::Added), count(LineKind::Removed))
}

/// The old and new line number beside each line of a hunk.
pub fn line_numbers(hunk: &Hunk) -> Vec<(Option<u32>, Option<u32>)> {
    let (mut old, mut new) = (hunk.old_start, hunk.new_start);
    hunk.lines
        .iter()
        .map(|line| {
            let pair = (
                (line.kind != LineKind::Added).then_some(old),
                (line.kind != LineKind::Removed).then_some(new),
            );
            old += u32::from(pair.0.is_some());
            new += u32::from(pair.1.is_some());
            pair
        })
        .collect()
}

/// The line a hunk is named by: where it starts in the new file, or in the
/// old one when nothing of it is left in the new.
pub fn shown_start(hunk: &Hunk) -> u32 {
    if hunk.new_start == 0 {
        hunk.old_start
    } else {
        hunk.new_start
    }
}

/// Files grouped by folder, folders in the order they first appear.
pub fn folders(files: &[FileChange]) -> Vec<(SharedString, Vec<usize>)> {
    let mut groups: Vec<(SharedString, Vec<usize>)> = Vec::new();
    for (ix, file) in files.iter().enumerate() {
        let dir = split_path(&file.path).0;
        match groups.iter_mut().find(|(name, _)| name.as_ref() == dir) {
            Some((_, members)) => members.push(ix),
            None => groups.push((SharedString::from(dir.to_string()), vec![ix])),
        }
    }
    groups
}

/// The tree rows that show, top to bottom, with these folders closed.
pub fn visible_rows(files: &[FileChange], closed: &[SharedString]) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for (dir, members) in folders(files) {
        let open = !closed.contains(&dir);
        rows.push(TreeRow::Folder(dir));
        if open {
            rows.extend(
                members
                    .iter()
                    .map(|&ix| TreeRow::File(files[ix].id.clone())),
            );
        }
    }
    rows
}

/// What Apply reports: for every file, the hunks that were not reverted.
pub fn kept_hunks(
    files: &[FileChange],
    reverted: &[(SharedString, SharedString)],
) -> Vec<KeptHunks> {
    files
        .iter()
        .map(|file| KeptHunks {
            file: file.id.clone(),
            hunks: file
                .hunks
                .iter()
                .filter(|hunk| !is_reverted(reverted, &file.id, &hunk.id))
                .map(|hunk| hunk.id.clone())
                .collect(),
        })
        .collect()
}

fn is_reverted(reverted: &[(SharedString, SharedString)], file: &str, hunk: &str) -> bool {
    reverted
        .iter()
        .any(|(a, b)| a.as_ref() == file && b.as_ref() == hunk)
}

/// The footer's sentence, as it is spoken.
pub fn summary(kept: usize, total: usize, files: usize) -> String {
    let noun = if files == 1 { "file" } else { "files" };
    format!("Applying {kept} of {total} hunks across {files} {noun}")
}

/// What the review remembers between frames.
#[derive(Clone)]
struct Review {
    selected: Option<SharedString>,
    reverted: Vec<(SharedString, SharedString)>,
    closed: Vec<SharedString>,
    cursor: Option<TreeRow>,
    /// The open file was picked with the pointer, so its hunks ease in.
    eased: bool,
    /// The cursor was last moved with the keys, so it shows a ring.
    keys: bool,
}

type ApplyHandler = Rc<dyn Fn(&[KeptHunks], &mut Window, &mut App)>;
type HunkHandler = Rc<dyn Fn(&HunkChoice, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type DiscardHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A change to review hunk by hunk.
#[derive(IntoElement)]
pub struct FileChangeReview {
    id: ElementId,
    style: StyleRefinement,
    files: Vec<FileChange>,
    stacked: bool,
    selected: Option<SharedString>,
    default_selected: Option<SharedString>,
    reverted: Vec<(SharedString, SharedString)>,
    applying: bool,
    on_apply: Option<ApplyHandler>,
    on_discard: Option<DiscardHandler>,
    on_hunk_change: Option<HunkHandler>,
    on_select: Option<SelectHandler>,
}

impl FileChangeReview {
    /// Create the review of these files, with the tree beside the hunks.
    pub fn new(id: impl Into<ElementId>, files: Vec<FileChange>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            files,
            stacked: false,
            selected: None,
            default_selected: None,
            reverted: Vec::new(),
            applying: false,
            on_apply: None,
            on_discard: None,
            on_hunk_change: None,
            on_select: None,
        }
    }

    /// Put the files in a row of tabs above the hunks instead of a tree.
    pub fn stacked(mut self) -> Self {
        self.stacked = true;
        self
    }

    /// The file that is open at first. Without it, the first one.
    pub fn default_selected(mut self, file: impl Into<SharedString>) -> Self {
        self.default_selected = Some(file.into());
        self
    }

    /// Hold the open file from outside. Pair with [`Self::on_select`].
    pub fn selected(mut self, file: impl Into<SharedString>) -> Self {
        self.selected = Some(file.into());
        self
    }

    /// A hunk that starts reverted.
    pub fn reverted(
        mut self,
        file: impl Into<SharedString>,
        hunk: impl Into<SharedString>,
    ) -> Self {
        self.reverted.push((file.into(), hunk.into()));
        self
    }

    /// The change is being applied: Apply spins and Discard is off.
    pub fn applying(mut self, applying: bool) -> Self {
        self.applying = applying;
        self
    }

    /// Run with the kept hunks of every file when Apply is pressed.
    pub fn on_apply(
        mut self,
        handler: impl Fn(&[KeptHunks], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_apply = Some(Rc::new(handler));
        self
    }

    /// Show Discard all, and run this when it is pressed.
    pub fn on_discard(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_discard = Some(Rc::new(handler));
        self
    }

    /// Run when a hunk is kept or reverted.
    pub fn on_hunk_change(
        mut self,
        handler: impl Fn(&HunkChoice, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hunk_change = Some(Rc::new(handler));
        self
    }

    /// Run with the file's id when a file is opened.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for FileChangeReview {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The colours a review draws with, copied out of the theme once.
#[derive(Clone)]
struct Inks {
    ink: Hsla,
    quiet: Hsla,
    added: Hsla,
    removed: Hsla,
    warning: Hsla,
    modified: Hsla,
    renamed: Hsla,
    mono: SharedString,
}

impl Inks {
    fn of(kind: ChangeKind, inks: &Inks) -> Hsla {
        match kind {
            ChangeKind::Added => inks.added,
            ChangeKind::Modified => inks.modified,
            ChangeKind::Deleted => inks.removed,
            ChangeKind::Renamed => inks.renamed,
        }
    }
}

/// The lettered square in front of a file name.
fn badge(kind: ChangeKind, inks: &Inks) -> impl IntoElement {
    let color = Inks::of(kind, inks);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(px(5.))
        .bg(color.opacity(0.15))
        .text_color(color)
        .font_family(inks.mono.clone())
        .text_size(px(10.))
        .line_height(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(kind.letter())
}

/// "+4 −2" beside a file name.
fn counts(hunks: &[Hunk], inks: &Inks) -> impl IntoElement {
    let (added, removed) = line_counts(hunks);
    h_flex()
        .flex_none()
        .gap_1()
        .font_family(inks.mono.clone())
        .text_size(px(11.))
        .line_height(px(16.))
        .when(added > 0, |this| {
            this.child(div().text_color(inks.added).child(format!("+{added}")))
        })
        .when(removed > 0, |this| {
            this.child(div().text_color(inks.removed).child(format!("−{removed}")))
        })
}

/// The code of one hunk: both line numbers, a sign, the line.
fn hunk_lines(hunk: &Hunk, inks: &Inks) -> impl IntoElement {
    let number = |value: Option<u32>| {
        div()
            .w(px(36.))
            .flex_none()
            .pr_2()
            .text_right()
            .text_color(inks.quiet.opacity(0.7))
            .child(value.map(|n| n.to_string()).unwrap_or_default())
    };
    let rows = hunk
        .lines
        .iter()
        .zip(line_numbers(hunk))
        .map(|(line, (old, new))| {
            let (sign, tone) = match line.kind {
                LineKind::Added => ("+", Some(inks.added)),
                LineKind::Removed => ("−", Some(inks.removed)),
                LineKind::Context => ("", None),
            };
            h_flex()
                .items_start()
                .pr_4()
                .when_some(tone, |this, tone| this.bg(tone.opacity(0.1)))
                .child(number(old))
                .child(number(new))
                .child(
                    div()
                        .w(px(16.))
                        .flex_none()
                        .when_some(tone, |this, tone| this.text_color(tone))
                        .child(sign),
                )
                .child(div().min_w_0().flex_1().child(if line.text.is_empty() {
                    SharedString::from(" ")
                } else {
                    line.text.clone()
                }))
        })
        .collect::<Vec<_>>();
    v_flex()
        .pb(px(6.))
        .font_family(inks.mono.clone())
        .text_size(px(12.))
        .line_height(px(20.))
        .children(rows)
}

impl RenderOnce for FileChangeReview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let part = |name: &str| ElementId::NamedChild(id.clone().into(), name.to_string().into());
        let files = Rc::new(self.files);
        let still = cx.reduce_motion();

        let first = self
            .default_selected
            .clone()
            .or_else(|| files.first().map(|file| file.id.clone()));
        let start_reverted = self.reverted.clone();
        let state = window.use_keyed_state(part("state"), cx, |_, _| Review {
            selected: first,
            reverted: start_reverted,
            closed: Vec::new(),
            cursor: None,
            eased: false,
            keys: true,
        });
        let now = state.read(cx).clone();
        let focus = window
            .use_keyed_state(part("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let ringed = now.keys && focus.is_focused(window);

        let open = self
            .selected
            .clone()
            .or(now.selected.clone())
            .and_then(|id| files.iter().position(|file| file.id == id))
            .or(if files.is_empty() { None } else { Some(0) });
        let total: usize = files.iter().map(|file| file.hunks.len()).sum();
        let selection = kept_hunks(&files, &now.reverted);
        let kept: usize = selection.iter().map(|file| file.hunks.len()).sum();
        let kept_files = selection
            .iter()
            .filter(|file| !file.hunks.is_empty())
            .count();

        let theme = cx.theme();
        let inks = Inks {
            ink: theme.foreground,
            quiet: theme.muted_foreground,
            added: theme.success,
            removed: theme.danger,
            warning: theme.warning,
            modified: theme.info,
            renamed: theme.magenta,
            mono: theme.mono_font_family.clone(),
        };
        let raised = theme.background;
        let wash = look::ink::wash(cx);

        let pick = {
            let state = state.clone();
            let select = self.on_select.clone();
            Rc::new(
                move |file: &SharedString, eased: bool, window: &mut Window, cx: &mut App| {
                    state.update(cx, |review, cx| {
                        review.selected = Some(file.clone());
                        review.cursor = Some(TreeRow::File(file.clone()));
                        review.eased = eased;
                        review.keys = !eased;
                        cx.notify();
                    });
                    if let Some(select) = &select {
                        select(file, window, cx);
                    }
                },
            )
        };

        // The files: a tree by folder, or a row of tabs.
        let rows = visible_rows(&files, &now.closed);
        let cursor = now
            .cursor
            .clone()
            .filter(|row| rows.contains(row))
            .or_else(|| open.map(|ix| TreeRow::File(files[ix].id.clone())));
        // In the tab strip, the open file's tab is brought into view when it changes.
        let strip = window
            .use_keyed_state(part("strip"), cx, |_, _| {
                (ScrollHandle::new(), Rc::new(Cell::new(None::<usize>)))
            })
            .read(cx)
            .clone();
        if strip.1.replace(open) != open
            && let Some(ix) = open
        {
            strip.0.scroll_to_item(ix);
        }
        let file_row = |ix: usize, tab: bool| {
            let file = &files[ix];
            let name = SharedString::from(split_path(&file.path).1.to_string());
            let chosen = open == Some(ix);
            let at_cursor = !tab && ringed && cursor == Some(TreeRow::File(file.id.clone()));
            let (added, removed) = line_counts(&file.hunks);
            let pick = pick.clone();
            let file_id = file.id.clone();
            h_flex()
                .id(part(&format!("file-{}", file.id)))
                .test_support()
                .role(if tab { Role::Tab } else { Role::TreeItem })
                .aria_label(format!(
                    "{name}, {}, {added} added, {removed} removed",
                    file.kind.word()
                ))
                .aria_selected(chosen)
                .when(tab, |this| {
                    this.tab_index(0)
                        .h(px(32.))
                        .flex_none()
                        .rounded_full()
                        .px_3()
                        .gap(px(6.))
                })
                .when(!tab, |this| this.rounded(px(12.)).px_2().py(px(6.)).gap_2())
                .cursor_pointer()
                .map(|this| {
                    if chosen {
                        this.bg(raised).shadow(resting_shadow(cx))
                    } else {
                        this.when(tab, |this| this.text_color(inks.quiet))
                            .hover(|this| this.bg(wash))
                    }
                })
                .when(at_cursor, |this| this.focus_ring_style(window, cx))
                .child(badge(file.kind, &inks))
                .child(
                    div()
                        .when(!tab, |this| this.min_w_0().flex_1().truncate())
                        .when(tab, |this| this.flex_none().whitespace_nowrap())
                        .when(file.kind == ChangeKind::Deleted, |this| this.line_through())
                        .child(name),
                )
                .child(counts(&file.hunks, &inks))
                .on_click(move |_, window, cx| pick(&file_id, true, window, cx))
        };

        let list: AnyElement = if self.stacked {
            h_flex()
                .id(part("tabs"))
                .track_scroll(&strip.0)
                .role(Role::TabList)
                .aria_label("Changed files")
                .overflow_x_scroll()
                .gap_1()
                .px_1()
                .pb(px(6.))
                .text_size(px(13.))
                .line_height(px(20.))
                .children((0..files.len()).map(|ix| file_row(ix, true)))
                .into_any_element()
        } else {
            let groups = folders(&files).into_iter().map(|(dir, members)| {
                let is_open = !now.closed.contains(&dir);
                let at_cursor = ringed && cursor == Some(TreeRow::Folder(dir.clone()));
                let label: SharedString = if dir.as_ref() == "." {
                    "root".into()
                } else {
                    dir.clone()
                };
                let toggle_state = state.clone();
                let toggle_dir = dir.clone();
                v_flex()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .id(part(&format!("folder-{dir}")))
                            .test_support()
                            .role(Role::TreeItem)
                            .aria_label(format!("Folder {label}"))
                            .aria_expanded(is_open)
                            .gap(px(6.))
                            .rounded(px(12.))
                            .px_2()
                            .py(px(6.))
                            .cursor_pointer()
                            .text_color(inks.quiet)
                            .hover(|this| this.bg(wash))
                            .when(at_cursor, |this| this.focus_ring_style(window, cx))
                            .child(
                                Icon::new(if is_open {
                                    CatalogIcon::ChevronDown
                                } else {
                                    CatalogIcon::ChevronRight
                                })
                                .size(px(14.)),
                            )
                            .child(Icon::new(CatalogIcon::Folder).size(px(14.)))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(inks.mono.clone())
                                    .text_size(px(12.))
                                    .child(label),
                            )
                            .on_click(move |_, _, cx| {
                                toggle_state.update(cx, |review, cx| {
                                    toggle(&mut review.closed, &toggle_dir, is_open);
                                    review.cursor = Some(TreeRow::Folder(toggle_dir.clone()));
                                    review.keys = false;
                                    cx.notify();
                                });
                            }),
                    )
                    .when(is_open, |this| {
                        this.child(
                            v_flex()
                                .gap(px(2.))
                                .pl_4()
                                .children(members.iter().map(|&ix| file_row(ix, false))),
                        )
                    })
            });
            let key_state = state.clone();
            let key_pick = pick.clone();
            let key_rows = rows;
            let key_cursor = cursor.clone();
            v_flex()
                .id(part("tree"))
                .test_support()
                .track_focus(&focus)
                .role(Role::Tree)
                .aria_label("Changed files")
                .w(px(208.))
                .flex_none()
                .p(px(6.))
                .gap(px(2.))
                .text_size(px(13.))
                .line_height(px(20.))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let at = key_cursor
                        .as_ref()
                        .and_then(|row| key_rows.iter().position(|other| other == row));
                    let last = key_rows.len().saturating_sub(1);
                    let key = event.keystroke.key.as_str();
                    let go = match key {
                        "down" => Some(at.map_or(0, |at| (at + 1).min(last))),
                        "up" => Some(at.map_or(0, |at| at.saturating_sub(1))),
                        "home" => Some(0),
                        "end" => Some(last),
                        _ => None,
                    };
                    match (go, key_cursor.as_ref(), key) {
                        (Some(to), _, _) => {
                            if let Some(row) = key_rows.get(to).cloned() {
                                key_state.update(cx, |review, cx| {
                                    review.cursor = Some(row);
                                    review.keys = true;
                                    cx.notify();
                                });
                            }
                        }
                        (None, Some(TreeRow::Folder(dir)), "left" | "right") => {
                            let dir = dir.clone();
                            key_state.update(cx, |review, cx| {
                                toggle(&mut review.closed, &dir, key == "left");
                                review.cursor = Some(TreeRow::Folder(dir.clone()));
                                review.keys = true;
                                cx.notify();
                            });
                        }
                        (None, Some(TreeRow::File(file)), "enter" | "space") => {
                            key_pick(file, false, window, cx);
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                })
                .children(groups)
                .into_any_element()
        };

        // The open file's hunks.
        let hunks = open.map(|ix| {
            let file = &files[ix];
            let mut cards: Vec<AnyElement> = Vec::new();
            for hunk in &file.hunks {
                let keep = !is_reverted(&now.reverted, &file.id, &hunk.id);
                let key = format!("{}-{}", file.id, hunk.id);
                let shown = transition(
                    part(&format!("dim-{key}")),
                    if keep { 1. } else { 0.4 },
                    Transition::new(EASE),
                    window,
                    cx,
                );
                let choice_state = state.clone();
                let choice = HunkChoice {
                    file: file.id.clone(),
                    hunk: hunk.id.clone(),
                    keep,
                };
                let changed = self.on_hunk_change.clone();
                cards.push(
                    inset(cx)
                        .id(part(&format!("hunk-{key}")))
                        .role(Role::Group)
                        .aria_label(format!(
                            "Lines from {}, {}",
                            shown_start(hunk),
                            if keep { "kept" } else { "reverted" }
                        ))
                        .overflow_hidden()
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_3()
                                .py(px(6.))
                                .pl_3()
                                .pr(px(6.))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .font_family(inks.mono.clone())
                                        .text_size(px(11.))
                                        .line_height(px(16.))
                                        .text_color(inks.quiet)
                                        .child(format!("Line {}", shown_start(hunk)))
                                        .when(!keep, |this| {
                                            this.child(
                                                div().text_color(inks.warning).child("· Reverted"),
                                            )
                                        }),
                                )
                                .child(
                                    TabBar::new(part(&format!("choice-{key}")))
                                        .segmented()
                                        .small()
                                        .selected_index(usize::from(!keep))
                                        .on_click(move |ix: &usize, window, cx| {
                                            let keep = *ix == 0;
                                            if keep == choice.keep {
                                                return;
                                            }
                                            let next = HunkChoice {
                                                keep,
                                                ..choice.clone()
                                            };
                                            choice_state.update(cx, |review, cx| {
                                                review.reverted.retain(|(file, hunk)| {
                                                    !(*file == next.file && *hunk == next.hunk)
                                                });
                                                if !keep {
                                                    review.reverted.push((
                                                        next.file.clone(),
                                                        next.hunk.clone(),
                                                    ));
                                                }
                                                cx.notify();
                                            });
                                            if let Some(changed) = &changed {
                                                changed(&next, window, cx);
                                            }
                                        })
                                        .child(Tab::new().label("Keep"))
                                        .child(Tab::new().label("Revert")),
                                ),
                        )
                        .child(div().opacity(shown).child(hunk_lines(hunk, &inks)))
                        .into_any_element(),
                );
            }
            let cards = v_flex().relative().gap_2().children(cards);
            let cards = if now.eased && !still {
                cards
                    .with_animation(
                        part(&format!("open-{}", file.id)),
                        Animation::new(EASE),
                        |this, time| {
                            let shown = 1. - (1. - time).powi(3);
                            this.opacity(shown).top(px(6. * (1. - shown)))
                        },
                    )
                    .into_any_element()
            } else {
                cards.into_any_element()
            };
            panel(cx)
                .min_w_0()
                .flex_1()
                .p_2()
                .child(
                    h_flex()
                        .gap_2()
                        .px_2()
                        .pt_1()
                        .pb(px(10.))
                        .font_family(inks.mono.clone())
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(inks.quiet)
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(inks.ink)
                                .child(file.path.clone()),
                        )
                        .when_some(file.from.clone(), |this, from| {
                            this.child(div().min_w_0().truncate().child(format!("from {from}")))
                        }),
                )
                .child(cards)
        });

        let apply = self.on_apply.clone();
        let apply_state = state;
        let apply_files = files.clone();
        let footer = h_flex()
            .flex_wrap()
            .gap_3()
            .py(px(6.))
            .pl_3()
            .pr(px(2.))
            .text_size(px(13.))
            .line_height(px(20.))
            .child(
                h_flex()
                    .flex_1()
                    .gap_1()
                    .text_color(inks.quiet)
                    .child("Applying")
                    .child(
                        div()
                            .font_family(inks.mono.clone())
                            .text_color(inks.ink)
                            .child(
                                RollingNumber::new(part("kept"), format!("{kept} of {total}"))
                                    .line_height(px(20.)),
                            ),
                    )
                    .child("hunks across")
                    .child(
                        RollingNumber::new(part("files"), kept_files.to_string())
                            .line_height(px(20.)),
                    )
                    .child(if kept_files == 1 { "file" } else { "files" }),
            )
            .when_some(self.on_discard.clone(), |this, discard| {
                this.child(
                    Button::new(part("discard"))
                        .ghost()
                        .label("Discard all")
                        .disabled(self.applying)
                        .on_click(move |_, window, cx| discard(window, cx)),
                )
            })
            .child(
                Button::new(part("apply"))
                    .primary()
                    .icon(Icon::new(CatalogIcon::Checkmark))
                    .label(if self.applying {
                        "Applying…"
                    } else {
                        "Apply"
                    })
                    .loading(self.applying)
                    .disabled(kept == 0)
                    .px_4()
                    .on_click(move |_, window, cx| {
                        if let Some(apply) = &apply {
                            let reverted = apply_state.read(cx).reverted.clone();
                            apply(&kept_hunks(&apply_files, &reverted), window, cx);
                        }
                    }),
            );

        shell(cx)
            .id(self.id.clone())
            .max_w(px(if self.stacked { 576. } else { 768. }))
            .role(Role::Region)
            .aria_label(format!(
                "File changes. {}",
                summary(kept, total, kept_files)
            ))
            .map(|this| match hunks {
                None => this.child(
                    panel(cx)
                        .px_4()
                        .py_10()
                        .text_center()
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(inks.quiet)
                        .child("No changes to review"),
                ),
                Some(hunks) if self.stacked => this.child(list).child(hunks),
                Some(hunks) => {
                    this.child(h_flex().items_start().gap(px(6.)).child(list).child(hunks))
                }
            })
            .child(footer)
            .refine_style(&self.style)
    }
}

/// Close or open a folder in the list of closed ones.
fn toggle(closed: &mut Vec<SharedString>, dir: &SharedString, close: bool) {
    closed.retain(|other| other != dir);
    if close {
        closed.push(dir.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<FileChange> {
        vec![
            FileChange::new("glaze", "recipes/glaze.md", ChangeKind::Modified).hunk(
                Hunk::new("a", 4, 4).lines([
                    HunkLine::context("## Celadon"),
                    HunkLine::removed("- 40 feldspar"),
                    HunkLine::added("- 42 feldspar"),
                    HunkLine::added("- 2 bentonite"),
                    HunkLine::context("- 20 silica"),
                ]),
            ),
            FileChange::new("kiln", "kiln.toml", ChangeKind::Added)
                .hunk(Hunk::new("a", 0, 1).lines([HunkLine::added("cone = 6")])),
            FileChange::new("notes", "recipes/notes.md", ChangeKind::Deleted)
                .hunk(Hunk::new("a", 1, 0).lines([HunkLine::removed("try ash glaze")]))
                .hunk(Hunk::new("b", 9, 0).lines([HunkLine::removed("done")])),
        ]
    }

    #[test]
    fn a_path_splits_into_folder_and_name() {
        assert_eq!(split_path("recipes/glaze.md"), ("recipes", "glaze.md"));
        assert_eq!(split_path("a/b/c.rs"), ("a/b", "c.rs"));
        assert_eq!(split_path("kiln.toml"), (".", "kiln.toml"));
    }

    #[test]
    fn lines_are_counted_and_numbered_on_both_sides() {
        let files = sample();
        assert_eq!(line_counts(&files[0].hunks), (2, 1));
        assert_eq!(line_counts(&files[2].hunks), (0, 2));
        assert_eq!(shown_start(&files[0].hunks[0]), 4);
        assert_eq!(shown_start(&files[2].hunks[1]), 9);
        assert_eq!(
            line_numbers(&files[0].hunks[0]),
            vec![
                (Some(4), Some(4)),
                (Some(5), None),
                (None, Some(5)),
                (None, Some(6)),
                (Some(6), Some(7)),
            ]
        );
    }

    #[test]
    fn files_group_by_folder_in_first_seen_order() {
        let files = sample();
        let groups = folders(&files);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], (SharedString::from("recipes"), vec![0, 2]));
        assert_eq!(groups[1], (SharedString::from("."), vec![1]));
    }

    #[test]
    fn closed_folders_hide_their_files_from_the_rows() {
        let files = sample();
        assert_eq!(visible_rows(&files, &[]).len(), 5);
        let rows = visible_rows(&files, &["recipes".into()]);
        assert_eq!(
            rows,
            vec![
                TreeRow::Folder("recipes".into()),
                TreeRow::Folder(".".into()),
                TreeRow::File("kiln".into()),
            ]
        );
    }

    #[test]
    fn apply_reports_what_was_not_reverted() {
        let files = sample();
        let reverted = vec![("notes".into(), "a".into()), ("kiln".into(), "a".into())];
        let kept = kept_hunks(&files, &reverted);
        assert_eq!(kept[0].hunks, vec![SharedString::from("a")]);
        assert!(kept[1].hunks.is_empty());
        assert_eq!(kept[2].hunks, vec![SharedString::from("b")]);
        assert_eq!(summary(2, 4, 2), "Applying 2 of 4 hunks across 2 files");
        assert_eq!(summary(1, 4, 1), "Applying 1 of 4 hunks across 1 file");
    }
}
