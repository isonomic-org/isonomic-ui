// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! File dropzone: drop or pick files and watch each one upload.
//!
//! The dropzone owns the list. It checks every file against the accepted types
//! and the size limit, then tells its owner which files to upload through
//! [`FileDropzoneEvent`]. The owner reports back with
//! [`FileDropzone::set_progress`], [`FileDropzone::finish`] and
//! [`FileDropzone::fail`].
//!
//! Accepted types are extensions (".pdf") or whole families ("image/*").
//! Besides the size limit a dropzone can ask for a smallest size (turning away
//! empty files), cap how many files it holds and turn away a file already in
//! the list. Each turned-away row says why in words, and the owner can read
//! the reason as a [`RejectReason`]. While files are dragged over it, the area
//! turns the danger colour when none of them would be taken.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Context, EventEmitter, ExternalPaths, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// The size limit when none is set: 25 MB.
const DEFAULT_MAX_SIZE: u64 = 25 * 1024 * 1024;

/// Where a file stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadStatus {
    /// Being uploaded.
    Uploading,
    /// Uploaded.
    Done,
    /// The upload failed and can be retried.
    Failed,
    /// Turned away before uploading: see [`UploadItem::rejection`].
    Rejected,
}

/// One file in the list.
#[derive(Debug, Clone)]
pub struct UploadItem {
    id: usize,
    name: SharedString,
    path: Option<PathBuf>,
    size: u64,
    status: UploadStatus,
    progress: f32,
    error: Option<SharedString>,
    rejection: Option<RejectReason>,
    started: Instant,
}

/// Why a file was turned away before uploading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// Its type is not one the dropzone takes; `extension` is the file's own,
    /// lower case, if it has one.
    WrongType {
        /// The file's extension.
        extension: Option<String>,
    },
    /// Larger than the limit, in bytes.
    TooLarge {
        /// The largest size taken.
        limit: u64,
    },
    /// Smaller than the minimum, in bytes; a minimum of one byte turns away
    /// empty files.
    TooSmall {
        /// The smallest size taken.
        minimum: u64,
    },
    /// The list already holds as many files as it takes.
    TooMany {
        /// How many files the list holds at most.
        limit: usize,
    },
    /// A file of the same name and size is already in the list.
    Duplicate,
}

impl RejectReason {
    /// The reason in words, as the row shows it.
    pub fn message(&self) -> String {
        match self {
            Self::WrongType {
                extension: Some(extension),
            } => format!("{} files are not accepted", extension.to_uppercase()),
            Self::WrongType { extension: None } => {
                "Files without an extension are not accepted".into()
            }
            Self::TooLarge { limit } => format!("Too large, the limit is {}", format_size(*limit)),
            Self::TooSmall { minimum } if *minimum <= 1 => "This file is empty".into(),
            Self::TooSmall { minimum } => {
                format!("Too small, the minimum is {}", format_size(*minimum))
            }
            Self::TooMany { limit: 1 } => "Only one file at a time".into(),
            Self::TooMany { limit } => format!("Only {limit} files at a time"),
            Self::Duplicate => "Already in the list".into(),
        }
    }
}

/// The checks a file must pass.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rules {
    /// Accepted extensions, lower case and without the dot, or families
    /// such as "image/*". Empty accepts all.
    accept: Vec<SharedString>,
    max_size: u64,
    min_size: u64,
    max_files: Option<usize>,
    reject_duplicates: bool,
}

/// The extensions a family such as "image/*" stands for.
pub fn family_extensions(family: &str) -> &'static [&'static str] {
    match family {
        "image/*" => &[
            "png", "jpg", "jpeg", "gif", "webp", "heic", "svg", "tif", "tiff",
        ],
        "video/*" => &["mp4", "mov", "webm", "mkv", "avi", "m4v"],
        "audio/*" => &["mp3", "wav", "m4a", "aac", "flac", "ogg"],
        "text/*" => &["txt", "md", "csv", "tsv", "json", "log"],
        _ => &[],
    }
}

/// Whether a file with `extension` is taken by the accepted list.
fn accepts(accept: &[SharedString], extension: Option<&str>) -> bool {
    if accept.is_empty() {
        return true;
    }
    let Some(extension) = extension else {
        return false;
    };
    accept.iter().any(|accepted| {
        accepted.as_ref() == extension || family_extensions(accepted).contains(&extension)
    })
}

impl Rules {
    /// Whether a file may join a list holding `held` files (not counting
    /// turned-away rows), `existing` being the names and sizes already in it.
    fn check(
        &self,
        name: &str,
        size: u64,
        held: usize,
        existing: &[(SharedString, u64)],
    ) -> Result<(), RejectReason> {
        let extension = extension(name);
        if !accepts(&self.accept, extension.as_deref()) {
            return Err(RejectReason::WrongType { extension });
        }
        if size > self.max_size {
            return Err(RejectReason::TooLarge {
                limit: self.max_size,
            });
        }
        if size < self.min_size {
            return Err(RejectReason::TooSmall {
                minimum: self.min_size,
            });
        }
        if self.reject_duplicates
            && existing
                .iter()
                .any(|(other, other_size)| other.as_ref() == name && *other_size == size)
        {
            return Err(RejectReason::Duplicate);
        }
        if let Some(limit) = self.max_files
            && held >= limit
        {
            return Err(RejectReason::TooMany { limit });
        }
        Ok(())
    }
}

impl UploadItem {
    /// The id the dropzone gave this file.
    pub fn id(&self) -> usize {
        self.id
    }

    /// The file name.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// Where the file is on disk, when it came from a drop or the picker.
    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// The size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Where the file stands.
    pub fn status(&self) -> UploadStatus {
        self.status
    }

    /// How much is uploaded, from 0 to 1.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Why the file was turned away, for a [`UploadStatus::Rejected`] row.
    pub fn rejection(&self) -> Option<&RejectReason> {
        self.rejection.as_ref()
    }

    /// Why the upload failed or the file was turned away, in words.
    pub fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }
}

/// What the dropzone tells its owner.
#[derive(Debug, Clone)]
pub enum FileDropzoneEvent {
    /// Start uploading these files: they were just added, or one is retried.
    Upload(Vec<usize>),
    /// A file was taken off the list. Cancel its upload if it is running.
    Removed(usize),
    /// The list was cleared.
    Cleared,
}

/// A drop area with the list of files dropped on it.
pub struct FileDropzone {
    rules: Rules,
    hint: Option<SharedString>,
    items: Vec<UploadItem>,
    next_id: usize,
    focus: FocusHandle,
}

impl EventEmitter<FileDropzoneEvent> for FileDropzone {}

impl FileDropzone {
    /// Create a dropzone that takes any file up to 25 MB.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            rules: Rules {
                accept: Vec::new(),
                max_size: DEFAULT_MAX_SIZE,
                min_size: 0,
                max_files: None,
                reject_duplicates: false,
            },
            hint: None,
            items: Vec::new(),
            next_id: 0,
            focus: cx.focus_handle(),
        }
    }

    /// Only take files with these extensions, such as `[".pdf", ".png"]`,
    /// or of these families: `"image/*"`, `"video/*"`, `"audio/*"` and
    /// `"text/*"`. Other files get a row that says why they were turned away.
    pub fn accept(mut self, extensions: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.rules.accept = extensions
            .into_iter()
            .map(|extension| normalize(extension.as_ref()).into())
            .collect();
        self
    }

    /// Set the largest file taken, in bytes.
    pub fn max_size(mut self, bytes: u64) -> Self {
        self.rules.max_size = bytes;
        self
    }

    /// Set the smallest file taken, in bytes. One byte turns away empty
    /// files. None by default.
    pub fn min_size(mut self, bytes: u64) -> Self {
        self.rules.min_size = bytes;
        self
    }

    /// Hold at most this many files; more are turned away until one is
    /// removed. Turned-away rows don't count. No limit by default.
    pub fn max_files(mut self, count: usize) -> Self {
        self.rules.max_files = Some(count.max(1));
        self
    }

    /// Turn away a file with the same name and size as one already in the
    /// list. Off by default.
    pub fn reject_duplicates(mut self, reject: bool) -> Self {
        self.rules.reject_duplicates = reject;
        self
    }

    /// Replace the line under the title, which otherwise lists the accepted
    /// types and the size limit.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The files in the list, oldest first.
    pub fn items(&self) -> &[UploadItem] {
        &self.items
    }

    /// Add files from disk, as a drop or the picker does. Folders are skipped.
    pub fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>, cx: &mut Context<Self>) {
        let mut accepted = Vec::new();
        for path in paths {
            if path.is_dir() {
                continue;
            }
            let Some(name) = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
            else {
                continue;
            };
            let size = std::fs::metadata(&path).map_or(0, |meta| meta.len());
            let (id, ok) = self.push(name.into(), Some(path), size);
            if ok {
                accepted.push(id);
            }
        }
        if !accepted.is_empty() {
            cx.emit(FileDropzoneEvent::Upload(accepted));
        }
        cx.notify();
    }

    /// Add a file that does not live on disk, such as a pasted image.
    /// Returns its id.
    pub fn add_file(
        &mut self,
        name: impl Into<SharedString>,
        size: u64,
        cx: &mut Context<Self>,
    ) -> usize {
        let (id, ok) = self.push(name.into(), None, size);
        if ok {
            cx.emit(FileDropzoneEvent::Upload(vec![id]));
        }
        cx.notify();
        id
    }

    /// Report how much of a file is uploaded, from 0 to 1.
    pub fn set_progress(&mut self, id: usize, progress: f32, cx: &mut Context<Self>) {
        if let Some(item) = self.uploading(id) {
            item.progress = progress.clamp(0., 1.);
            cx.notify();
        }
    }

    /// Mark a file as uploaded.
    pub fn finish(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.uploading(id) {
            item.status = UploadStatus::Done;
            item.progress = 1.;
            cx.notify();
        }
    }

    /// Mark a file as failed, with a short reason. It can then be retried.
    pub fn fail(&mut self, id: usize, reason: impl Into<SharedString>, cx: &mut Context<Self>) {
        if let Some(item) = self.uploading(id) {
            item.status = UploadStatus::Failed;
            item.error = Some(reason.into());
            cx.notify();
        }
    }

    /// Upload a failed file again, from the start.
    pub fn retry(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return;
        };
        if item.status != UploadStatus::Failed {
            return;
        }
        item.status = UploadStatus::Uploading;
        item.progress = 0.;
        item.error = None;
        item.started = Instant::now();
        cx.emit(FileDropzoneEvent::Upload(vec![id]));
        cx.notify();
    }

    /// Take a file off the list.
    pub fn remove(&mut self, id: usize, cx: &mut Context<Self>) {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        if self.items.len() != before {
            cx.emit(FileDropzoneEvent::Removed(id));
            cx.notify();
        }
    }

    /// Empty the list.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.items.is_empty() {
            self.items.clear();
            cx.emit(FileDropzoneEvent::Cleared);
            cx.notify();
        }
    }

    fn uploading(&mut self, id: usize) -> Option<&mut UploadItem> {
        self.items
            .iter_mut()
            .find(|item| item.id == id && item.status == UploadStatus::Uploading)
    }

    /// Add a row. Returns its id and whether the file passed the checks.
    fn push(&mut self, name: SharedString, path: Option<PathBuf>, size: u64) -> (usize, bool) {
        let id = self.next_id;
        self.next_id += 1;
        let held = self
            .items
            .iter()
            .filter(|item| item.status != UploadStatus::Rejected)
            .count();
        let existing: Vec<(SharedString, u64)> = self
            .items
            .iter()
            .filter(|item| item.status != UploadStatus::Rejected)
            .map(|item| (item.name.clone(), item.size))
            .collect();
        let verdict = self.rules.check(&name, size, held, &existing);
        let ok = verdict.is_ok();
        self.items.push(UploadItem {
            id,
            name,
            path,
            size,
            status: if ok {
                UploadStatus::Uploading
            } else {
                UploadStatus::Rejected
            },
            progress: 0.,
            error: verdict.as_ref().err().map(|reason| reason.message().into()),
            rejection: verdict.err(),
            started: Instant::now(),
        });
        (id, ok)
    }

    fn browse(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                this.update(cx, |this, cx| this.add_paths(paths, cx)).ok();
            }
        })
        .detach();
    }
}

/// An extension as the checks compare it: lower case, without the dot. A
/// family such as "Image/*" stays a family.
fn normalize(extension: &str) -> String {
    extension.trim().trim_start_matches('.').to_lowercase()
}

/// Whether any of the dragged files would be taken. Sizes are read from disk;
/// a file that can't be read counts as empty.
fn any_taken(rules: &Rules, paths: &[PathBuf], held: usize) -> bool {
    paths.iter().filter(|path| !path.is_dir()).any(|path| {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let size = std::fs::metadata(path).map_or(0, |meta| meta.len());
        rules.check(&name, size, held, &[]).is_ok()
    })
}

/// The extension of a file name, lower case. Dot files have none.
fn extension(name: &str) -> Option<String> {
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty()).then(|| extension.to_lowercase())
}

/// Whether a file may be uploaded by type and size alone, or why not.
#[cfg(test)]
fn check(name: &str, size: u64, accept: &[SharedString], max_size: u64) -> Result<(), String> {
    Rules {
        accept: accept.to_vec(),
        max_size,
        min_size: 0,
        max_files: None,
        reject_duplicates: false,
    }
    .check(name, size, 0, &[])
    .map_err(|reason| reason.message())
}

/// A size in the largest unit that keeps it readable: "850 KB", "4.2 MB".
fn format_size(bytes: u64) -> String {
    const STEP: f64 = 1024.;
    let one_decimal = |value: f64| {
        let text = format!("{value:.1}");
        text.strip_suffix(".0").map(str::to_string).unwrap_or(text)
    };
    let size = bytes as f64;
    if size < STEP {
        format!("{bytes} B")
    } else if size < STEP * STEP {
        format!("{:.0} KB", size / STEP)
    } else if size < STEP * STEP * STEP {
        format!("{} MB", one_decimal(size / (STEP * STEP)))
    } else {
        format!("{} GB", one_decimal(size / (STEP * STEP * STEP)))
    }
}

/// How the hint names an accepted entry: "PDF", or "Images" for a family.
fn accepted_name(entry: &str) -> String {
    match entry {
        "image/*" => "Images".into(),
        "video/*" => "Videos".into(),
        "audio/*" => "Audio".into(),
        "text/*" => "Text files".into(),
        other => other.to_uppercase(),
    }
}

/// The line under the title when no hint is given.
fn default_hint(accept: &[SharedString], max_size: u64) -> String {
    let limit = format!("up to {} each", format_size(max_size));
    if accept.is_empty() {
        return format!("Any file · {limit}");
    }
    let listed = accept
        .iter()
        .take(3)
        .map(|extension| accepted_name(extension))
        .collect::<Vec<_>>()
        .join(", ");
    if accept.len() > 3 {
        format!("{listed} and more · {limit}")
    } else {
        format!("{listed} · {limit}")
    }
}

/// How long the rest of an upload should take, judged by how long it took to
/// get this far. Too early to say below two percent.
fn time_left(elapsed: Duration, progress: f32) -> Option<Duration> {
    (0.02..1.)
        .contains(&progress)
        .then(|| elapsed.mul_f32((1. - progress) / progress))
}

fn time_left_label(left: Duration) -> String {
    let seconds = left.as_secs();
    if seconds < 5 {
        "a few seconds left".into()
    } else if seconds < 60 {
        format!("about {} s left", (seconds + 2) / 5 * 5)
    } else {
        format!("about {} min left", (seconds + 30) / 60)
    }
}

/// The line in the last row of the card.
fn summary(items: &[UploadItem]) -> String {
    let count = |status| items.iter().filter(|item| item.status == status).count();
    let total = items.len();
    match count(UploadStatus::Uploading) {
        0 => format!("{} of {total} uploaded", count(UploadStatus::Done)),
        running => format!("Uploading {running} of {total}"),
    }
}

impl Render for FileDropzone {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let success = theme.success;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let hint: SharedString = match &self.hint {
            Some(hint) => hint.clone(),
            None => {
                let mut hint = default_hint(&self.rules.accept, self.rules.max_size);
                if let Some(limit) = self.rules.max_files {
                    hint.push_str(&format!(
                        " · at most {limit} file{}",
                        if limit == 1 { "" } else { "s" }
                    ));
                }
                hint.into()
            }
        };
        let rules = self.rules.clone();
        let held = self
            .items
            .iter()
            .filter(|item| item.status != UploadStatus::Rejected)
            .count();
        let is_focused = self.focus.is_focused(window);

        let rows = self.items.iter().map(|item| {
            let id = item.id;
            let (detail, detail_color) = match item.status {
                UploadStatus::Uploading => {
                    let percent = (item.progress * 100.).round() as u32;
                    let size = format_size(item.size);
                    let detail = match time_left(item.started.elapsed(), item.progress) {
                        Some(left) => format!("{percent}% of {size} · {}", time_left_label(left)),
                        None => format!("{percent}% of {size}"),
                    };
                    (SharedString::from(detail), quiet)
                }
                UploadStatus::Done => (format_size(item.size).into(), quiet),
                UploadStatus::Failed | UploadStatus::Rejected => {
                    let reason = item.error.clone();
                    (reason.unwrap_or_else(|| "Upload failed".into()), danger)
                }
            };
            let bar = match item.status {
                UploadStatus::Uploading => Some(accent),
                UploadStatus::Failed => Some(danger),
                UploadStatus::Done | UploadStatus::Rejected => None,
            };

            h_flex()
                .items_start()
                .gap_3()
                .px_3()
                .py_2p5()
                .border_t_1()
                .border_color(line)
                .child(
                    Icon::new(IconName::File)
                        .small()
                        .flex_none()
                        .mt_0p5()
                        .text_color(quiet),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .font_medium()
                                        .child(item.name.clone()),
                                )
                                .when(item.status == UploadStatus::Done, |this| {
                                    this.child(
                                        Icon::new(IconName::CircleCheck)
                                            .small()
                                            .flex_none()
                                            .text_color(success),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_caption()
                                .text_color(detail_color)
                                .child(detail),
                        )
                        .when_some(bar, |this, color| {
                            this.child(
                                div()
                                    .mt_1()
                                    .h(px(2.))
                                    .w_full()
                                    .rounded_full_style(cx)
                                    .overflow_hidden()
                                    .bg(chip)
                                    .child(div().h_full().w(relative(item.progress)).bg(color)),
                            )
                        }),
                )
                .when(item.status == UploadStatus::Failed, |this| {
                    this.child(
                        Button::new(("retry", id))
                            .ghost()
                            .xsmall()
                            .icon(IconName::RefreshCw)
                            .accessibility_label(format!("Retry {}", item.name))
                            .on_click(cx.listener(move |this, _, _, cx| this.retry(id, cx))),
                    )
                })
                .child(
                    Button::new(("remove", id))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .accessibility_label(format!("Remove {}", item.name))
                        .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
                )
        });
        let rows: Vec<_> = rows.collect();

        let all_done = self
            .items
            .iter()
            .all(|item| item.status == UploadStatus::Done);
        let any_trouble = self
            .items
            .iter()
            .any(|item| matches!(item.status, UploadStatus::Failed | UploadStatus::Rejected));
        let footer = (!self.items.is_empty()).then(|| {
            h_flex()
                .gap_2()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(line)
                .when(all_done, |this| {
                    this.child(
                        Icon::new(IconName::CircleCheck)
                            .small()
                            .flex_none()
                            .text_color(success),
                    )
                })
                .when(any_trouble, |this| {
                    this.child(
                        Icon::new(IconName::TriangleAlert)
                            .small()
                            .flex_none()
                            .text_color(warning),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(summary(&self.items)),
                )
                .child(
                    Button::new("clear")
                        .outline()
                        .xsmall()
                        .label("Clear")
                        .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                )
        });

        v_flex()
            .w_full()
            .max_w(px(520.))
            .overflow_hidden()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(
                div().p_2().child(
                    div()
                        .id("drop-area")
                        .rounded(radius)
                        .border_1()
                        .border_dashed()
                        .border_color(line)
                        // The accent while something here would be taken, the
                        // danger colour when every dragged file would be turned away.
                        .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                            let color = if any_taken(&rules, paths.paths(), held) {
                                accent
                            } else {
                                danger
                            };
                            style.border_color(color).bg(color.opacity(0.06))
                        })
                        .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                            this.add_paths(paths.paths().to_vec(), cx)
                        }))
                        .child(
                            Pressable::new("browse")
                                .track_focus(&self.focus)
                                .accessibility_label(format!("Choose files. {hint}"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.browse(window, cx)),
                                )
                                .flex_col()
                                .gap_1()
                                .w_full()
                                .px_4()
                                .py_6()
                                .rounded(radius)
                                .cursor_pointer()
                                .when(is_focused, |this| this.focus_ring_style(window, cx))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size_8()
                                        .mb_1()
                                        .rounded(radius)
                                        .border_1()
                                        .border_color(line)
                                        .text_color(quiet)
                                        .child(Icon::new(IconName::ArrowUp).small()),
                                )
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .font_medium()
                                        .child("Drop files here or")
                                        .child(div().text_color(accent).child("browse")),
                                )
                                .child(div().text_caption().text_color(quiet).child(hint)),
                        ),
                ),
            )
            .children(rows)
            .children(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn item(status: UploadStatus) -> UploadItem {
        UploadItem {
            id: 0,
            name: "a.pdf".into(),
            path: None,
            size: MB,
            status,
            progress: 0.,
            error: None,
            rejection: None,
            started: Instant::now(),
        }
    }

    fn rules() -> Rules {
        Rules {
            accept: vec!["pdf".into(), "image/*".into()],
            max_size: 10 * MB,
            min_size: 1,
            max_files: Some(2),
            reject_duplicates: true,
        }
    }

    #[test]
    fn families_take_their_extensions() {
        let rules = rules();
        assert!(rules.check("photo.HEIC", MB, 0, &[]).is_ok());
        assert!(rules.check("scan.pdf", MB, 0, &[]).is_ok());
        assert_eq!(
            rules.check("clip.mov", MB, 0, &[]),
            Err(RejectReason::WrongType {
                extension: Some("mov".into())
            })
        );
        assert!(family_extensions("video/*").contains(&"mov"));
        assert!(family_extensions("model/*").is_empty());
    }

    #[test]
    fn each_rule_turns_a_file_away_with_its_reason() {
        let rules = rules();
        assert_eq!(
            rules.check("blank.pdf", 0, 0, &[]),
            Err(RejectReason::TooSmall { minimum: 1 })
        );
        assert_eq!(
            rules.check("big.pdf", 11 * MB, 0, &[]),
            Err(RejectReason::TooLarge { limit: 10 * MB })
        );
        let existing = [("scan.pdf".into(), MB)];
        assert_eq!(
            rules.check("scan.pdf", MB, 1, &existing),
            Err(RejectReason::Duplicate)
        );
        // Same name, other size: a different file.
        assert!(rules.check("scan.pdf", 2 * MB, 1, &existing).is_ok());
        assert_eq!(
            rules.check("third.pdf", MB, 2, &[]),
            Err(RejectReason::TooMany { limit: 2 })
        );
    }

    #[test]
    fn reasons_read_in_words() {
        assert_eq!(
            RejectReason::TooSmall { minimum: 1 }.message(),
            "This file is empty"
        );
        assert_eq!(
            RejectReason::TooSmall { minimum: 2048 }.message(),
            "Too small, the minimum is 2 KB"
        );
        assert_eq!(
            RejectReason::TooMany { limit: 1 }.message(),
            "Only one file at a time"
        );
        assert_eq!(
            RejectReason::TooMany { limit: 5 }.message(),
            "Only 5 files at a time"
        );
        assert_eq!(RejectReason::Duplicate.message(), "Already in the list");
        assert_eq!(
            default_hint(&["image/*".into(), "pdf".into()], MB),
            "Images, PDF · up to 1 MB each"
        );
    }

    #[test]
    fn a_drag_of_only_unwanted_files_is_not_taken() {
        let rules = rules();
        let paths = [PathBuf::from("/nowhere/clip.mov")];
        assert!(!any_taken(&rules, &paths, 0));
        // A missing file reads as empty, and the rules here turn away empty files.
        let paths = [PathBuf::from("/nowhere/scan.pdf")];
        assert!(!any_taken(&rules, &paths, 0));
        let open = Rules {
            min_size: 0,
            ..rules
        };
        assert!(any_taken(&open, &paths, 0));
    }

    #[test]
    fn sizes_read_in_the_largest_unit() {
        assert_eq!(format_size(12), "12 B");
        assert_eq!(format_size(850 * 1024), "850 KB");
        assert_eq!(format_size(25 * MB), "25 MB");
        assert_eq!(format_size(4 * MB + MB / 5), "4.2 MB");
        assert_eq!(format_size(3 * 1024 * MB / 2), "1.5 GB");
    }

    #[test]
    fn extensions_ignore_case_and_dots() {
        assert_eq!(normalize(".PDF"), "pdf");
        assert_eq!(extension("Report.Final.PDF").as_deref(), Some("pdf"));
        assert_eq!(extension(".gitignore"), None);
        assert_eq!(extension("README"), None);
    }

    #[test]
    fn files_are_checked_for_type_and_size() {
        let accept = ["pdf".into(), "png".into()];
        assert!(check("scan.PNG", MB, &accept, 25 * MB).is_ok());
        assert_eq!(
            check("clip.mp4", MB, &accept, 25 * MB),
            Err("MP4 files are not accepted".into())
        );
        assert_eq!(
            check("README", MB, &accept, 25 * MB),
            Err("Files without an extension are not accepted".into())
        );
        assert_eq!(
            check("scan.png", 26 * MB, &accept, 25 * MB),
            Err("Too large, the limit is 25 MB".into())
        );
        // With nothing to accept by, any type passes.
        assert!(check("clip.mp4", MB, &[], 25 * MB).is_ok());
    }

    #[test]
    fn the_hint_lists_a_few_types_and_the_limit() {
        assert_eq!(default_hint(&[], 25 * MB), "Any file · up to 25 MB each");
        let two = ["pdf".into(), "png".into()];
        assert_eq!(default_hint(&two, MB), "PDF, PNG · up to 1 MB each");
        let four = ["pdf".into(), "png".into(), "jpg".into(), "csv".into()];
        assert_eq!(
            default_hint(&four, MB),
            "PDF, PNG, JPG and more · up to 1 MB each"
        );
    }

    #[test]
    fn time_left_follows_the_pace_so_far() {
        let ten = Duration::from_secs(10);
        assert_eq!(time_left(ten, 0.5), Some(ten));
        assert_eq!(time_left(ten, 0.01), None);
        assert_eq!(time_left(ten, 1.), None);
        assert_eq!(
            time_left_label(Duration::from_secs(3)),
            "a few seconds left"
        );
        assert_eq!(time_left_label(Duration::from_secs(23)), "about 25 s left");
        assert_eq!(
            time_left_label(Duration::from_secs(150)),
            "about 3 min left"
        );
    }

    #[test]
    fn the_summary_counts_what_is_running_first() {
        let running = [item(UploadStatus::Uploading), item(UploadStatus::Done)];
        assert_eq!(summary(&running), "Uploading 1 of 2");
        let settled = [
            item(UploadStatus::Done),
            item(UploadStatus::Failed),
            item(UploadStatus::Rejected),
        ];
        assert_eq!(summary(&settled), "1 of 3 uploaded");
    }
}
