//! Small pieces shared by the components in this crate.

use isonomic_ui::component::ActiveTheme as _;
use isonomic_ui::{App, BoxShadow, Hsla, hsla, px};

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

/// Shadow of a surface resting on the page. Empty when the theme turns shadows off.
pub fn resting_shadow(cx: &App) -> Vec<BoxShadow> {
    if !cx.theme().shadow {
        return Vec::new();
    }
    vec![
        BoxShadow::new(px(0.), px(1.), hsla(0., 0., 0., 0.06)).blur_radius(px(1.)),
        BoxShadow::new(px(0.), px(1.), hsla(0., 0., 0., 0.04))
            .blur_radius(px(1.))
            .spread_radius(px(-1.)),
    ]
}

/// Shadow of a surface lifted under the pointer. Empty when the theme turns shadows off.
pub fn lifted_shadow(cx: &App) -> Vec<BoxShadow> {
    if !cx.theme().shadow {
        return Vec::new();
    }
    vec![
        BoxShadow::new(px(0.), px(2.), hsla(0., 0., 0., 0.09))
            .blur_radius(px(3.))
            .spread_radius(px(-1.)),
        BoxShadow::new(px(0.), px(1.), hsla(0., 0., 0., 0.05)).blur_radius(px(1.)),
    ]
}

#[cfg(test)]
mod tests {
    use super::Tone;

    #[test]
    fn tone_defaults_to_neutral() {
        assert_eq!(Tone::default(), Tone::Neutral);
    }
}
