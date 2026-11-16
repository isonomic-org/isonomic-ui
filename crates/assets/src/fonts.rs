//! The bundled typeface.

use std::borrow::Cow;

/// The family name of the bundled interface typeface.
pub const SANS: &str = "IBM Plex Sans";
/// The family name of the bundled monospace typeface.
pub const MONO: &str = "IBM Plex Mono";
/// The family name of the bundled serif typeface, in regular and medium.
/// For display text such as a wordmark or a greeting, not for controls.
pub const SERIF: &str = "IBM Plex Serif";

macro_rules! font_files {
    ($($file:literal),* $(,)?) => {
        vec![$(Cow::Borrowed(include_bytes!(concat!("../fonts/", $file)).as_slice())),*]
    };
}

/// The font files of the bundled families, for `TextSystem::add_fonts`.
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    font_files![
        "IBMPlexMono-Bold.ttf",
        "IBMPlexMono-BoldItalic.ttf",
        "IBMPlexMono-Italic.ttf",
        "IBMPlexMono-Medium.ttf",
        "IBMPlexMono-Regular.ttf",
        "IBMPlexMono-SemiBold.ttf",
        "IBMPlexSans-Bold.ttf",
        "IBMPlexSans-BoldItalic.ttf",
        "IBMPlexSans-Italic.ttf",
        "IBMPlexSans-Medium.ttf",
        "IBMPlexSans-MediumItalic.ttf",
        "IBMPlexSans-Regular.ttf",
        "IBMPlexSans-SemiBold.ttf",
        "IBMPlexSans-SemiBoldItalic.ttf",
        "IBMPlexSerif-Medium.ttf",
        "IBMPlexSerif-Regular.ttf",
    ]
}
