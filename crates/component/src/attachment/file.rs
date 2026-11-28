// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a file name and byte count say about a file: its family, its type,
//! its size in words, and what a screen reader hears for the card.

use gpui::SharedString;

use super::AttachmentStatus;

/// A broad family of file types, chosen from a file name's extension. It
/// picks the glyph an icon-style preview shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFamily {
    /// Pictures: png, jpg, svg and the like.
    Image,
    /// Moving pictures.
    Video,
    /// Sound.
    Audio,
    /// Portable documents.
    Pdf,
    /// Compressed bundles of files.
    Archive,
    /// Tables of values.
    Spreadsheet,
    /// Source code and configuration.
    Code,
    /// Prose and plain text.
    Text,
    /// Anything else, or no extension.
    Other,
}

impl FileFamily {
    /// The family of a file, from the extension of its name.
    pub fn of(name: &str) -> Self {
        let Some(extension) = extension(name) else {
            return Self::Other;
        };
        match extension.to_ascii_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "heic" | "tif" | "tiff"
            | "avif" | "ico" => Self::Image,
            "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" => Self::Video,
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" => Self::Audio,
            "pdf" => Self::Pdf,
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" => Self::Archive,
            "csv" | "tsv" | "xls" | "xlsx" | "ods" | "parquet" => Self::Spreadsheet,
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "c" | "h" | "cpp" | "java"
            | "kt" | "swift" | "rb" | "sh" | "json" | "toml" | "yaml" | "yml" | "html" | "css"
            | "sql" | "xml" => Self::Code,
            "txt" | "md" | "rtf" | "doc" | "docx" | "odt" | "pages" | "log" => Self::Text,
            _ => Self::Other,
        }
    }

    /// The glyph an icon-style preview shows for this family.
    pub fn icon(self) -> isonomic_ui_assets::IconName {
        use isonomic_ui_assets::IconName as Glyph;
        match self {
            Self::Image => Glyph::Image,
            Self::Video => Glyph::Video,
            Self::Audio => Glyph::Music,
            Self::Pdf => Glyph::DocumentPdf,
            Self::Archive => Glyph::Zip,
            Self::Spreadsheet => Glyph::DataTable,
            Self::Code => Glyph::Code,
            Self::Text => Glyph::Document,
            Self::Other => Glyph::DocumentBlank,
        }
    }
}

/// The extension of a file name, without the dot. A leading dot alone, as in
/// `.env`, names a file and is not an extension.
pub(super) fn extension(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty() && !extension.contains(['/', '\\', ' ']))
        .then_some(extension)
}

/// A file's type in capitals, from its extension: `"PDF"` for
/// `"report.pdf"`, `None` without an extension.
pub fn file_kind(name: &str) -> Option<SharedString> {
    extension(name).map(|extension| extension.to_ascii_uppercase().into())
}

/// A file size the way people read it, in binary units: whole bytes below a
/// kilobyte, one decimal below ten of a unit and none from ten up.
pub fn format_file_size(bytes: u64) -> SharedString {
    const UNITS: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B").into();
    }
    let mut value = bytes as f64 / 1024.;
    let mut unit = 0;
    while value >= 1024. && unit + 1 < UNITS.len() {
        value /= 1024.;
        unit += 1;
    }
    // Rounding can carry into the next unit: 1023.96 KB reads as 1 MB.
    if value.round() >= 1024. && unit + 1 < UNITS.len() {
        value /= 1024.;
        unit += 1;
    }
    if value < 9.95 {
        format!("{value:.1} {}", UNITS[unit]).into()
    } else {
        format!("{} {}", value.round() as u64, UNITS[unit]).into()
    }
}

/// The type and size line of a file: `"PDF · 2.4 MB"`, or the size alone
/// when the name has no extension.
pub fn file_meta(name: &str, bytes: u64) -> SharedString {
    match file_kind(name) {
        Some(kind) => format!("{kind} · {}", format_file_size(bytes)).into(),
        None => format_file_size(bytes),
    }
}

/// What a screen reader says for an attachment: its name, then its state in
/// words, with the percentage while it uploads.
pub fn spoken_label(name: &str, status: AttachmentStatus, progress: Option<f32>) -> SharedString {
    match progress.filter(|_| status.is_uploading()) {
        Some(percent) => format!(
            "{name}, {}, {} percent",
            status.spoken(),
            percent.clamp(0., 100.).round() as u32
        )
        .into(),
        None => format!("{name}, {}", status.spoken()).into(),
    }
}
