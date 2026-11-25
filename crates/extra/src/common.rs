// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Small pieces shared by the components in this crate.

use isonomic_ui::component::ActiveTheme as _;
use isonomic_ui::{App, BoxShadow, Hsla};

/// The meaning a status carries, resolved to theme colors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    /// No particular meaning.
    #[default]
    Neutral,
    /// Informational, in progress.
    Info,
    /// Good, finished, safe.
    Success,
    /// Needs attention.
    Warning,
    /// Bad, failed, destructive.
    Danger,
}

impl Tone {
    /// Color for text and icons in this tone.
    pub fn color(self, cx: &App) -> Hsla {
        let theme = cx.theme();
        match self {
            Tone::Neutral => theme.muted_foreground,
            Tone::Info => theme.info,
            Tone::Success => theme.success,
            Tone::Warning => theme.warning,
            Tone::Danger => theme.danger,
        }
    }

    /// Soft fill to sit behind content in this tone.
    pub fn tint(self, cx: &App) -> Hsla {
        match self {
            Tone::Neutral => cx.theme().muted,
            _ => self.color(cx).opacity(0.12),
        }
    }
}

/// Shadow of a surface resting on the page: the library's raised surface,
/// a hairline ring and a soft drop.
pub fn resting_shadow(cx: &App) -> Vec<BoxShadow> {
    isonomic_ui::component::look::surface::resting(cx)
}

/// Shadow of a surface lifted under the pointer: the raised surface's hover
/// state.
pub fn lifted_shadow(cx: &App) -> Vec<BoxShadow> {
    isonomic_ui::component::look::surface::resting_hover(cx)
}

#[cfg(test)]
mod tests {
    use super::Tone;

    #[test]
    fn tone_defaults_to_neutral() {
        assert_eq!(Tone::default(), Tone::Neutral);
    }
}
