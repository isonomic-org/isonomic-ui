// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Semantic design tokens shared by application-owned components.
//!
//! These tokens describe visual roles and scales. They intentionally do not
//! contain component names such as `button`, `table`, or `sidebar`.

use gpui::{BoxShadow, FontWeight, Hsla, Pixels, SharedString, point, px, rgb, rgba};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct SemanticThemeTokens {
    pub colors: ColorTokens,
    pub radius: RadiusTokens,
    pub spacing: SpacingTokens,
    pub typography: TypographyTokens,
    pub shadow: ShadowTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ColorTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub surface: Hsla,
    pub surface_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
    pub border: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    /// Background painted behind selected text.
    ///
    /// Selection quads are painted under the glyphs, so this is a translucent
    /// wash that leaves the text legible. It carries a serde default so
    /// palettes written before the token existed still load.
    #[serde(default = "ColorTokens::default_selection")]
    pub selection: Hsla,
}

impl Default for ColorTokens {
    fn default() -> Self {
        Self::light()
    }
}

impl ColorTokens {
    /// Default light palette, equal to the styled Default Light theme.
    pub fn light() -> Self {
        Self {
            background: Hsla::from(rgb(0xffffff)),
            foreground: Hsla::from(rgb(0x1f1f1f)),
            surface: Hsla::from(rgb(0xffffff)),
            surface_foreground: Hsla::from(rgb(0x1f1f1f)),
            primary: Hsla::from(rgb(0x1f1f1f)),
            primary_foreground: Hsla::from(rgb(0xffffff)),
            secondary: Hsla::from(rgba(0x0000000a)),
            secondary_foreground: Hsla::from(rgb(0x1f1f1f)),
            muted: Hsla::from(rgba(0x00000008)),
            muted_foreground: Hsla::from(rgb(0x707070)),
            accent: Hsla::from(rgba(0x0000000d)),
            accent_foreground: Hsla::from(rgb(0x1f1f1f)),
            destructive: Hsla::from(rgb(0xc4320a)),
            destructive_foreground: Hsla::from(rgb(0xffffff)),
            border: Hsla::from(rgba(0x00000014)),
            input: Hsla::from(rgba(0x00000021)),
            ring: Hsla::from(rgb(0x4d90eb)),
            selection: Hsla::from(rgb(0x55a0fc)).alpha(0.3),
        }
    }

    /// Default dark palette, equal to the styled Default Dark theme.
    pub fn dark() -> Self {
        Self {
            background: Hsla::from(rgb(0x121213)),
            foreground: Hsla::from(rgb(0xe8e8e8)),
            surface: Hsla::from(rgb(0x1f1f22)),
            surface_foreground: Hsla::from(rgb(0xe8e8e8)),
            primary: Hsla::from(rgb(0xe8e8e8)),
            primary_foreground: Hsla::from(rgb(0x161618)),
            secondary: Hsla::from(rgba(0xffffff0f)),
            secondary_foreground: Hsla::from(rgb(0xe8e8e8)),
            muted: Hsla::from(rgba(0xffffff0a)),
            muted_foreground: Hsla::from(rgb(0x8a8a8b)),
            accent: Hsla::from(rgba(0xffffff12)),
            accent_foreground: Hsla::from(rgb(0xe8e8e8)),
            destructive: Hsla::from(rgb(0xff8a7a)),
            destructive_foreground: Hsla::from(rgb(0x161618)),
            border: Hsla::from(rgba(0xffffff24)),
            input: Hsla::from(rgba(0xffffff33)),
            ring: Hsla::from(rgb(0x4174d0)),
            selection: Hsla::from(rgb(0x1d4ed8)).alpha(0.3),
        }
    }

    /// The selection color a palette falls back to when it predates the token.
    fn default_selection() -> Hsla {
        Self::light().selection
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct RadiusTokens {
    pub none: Pixels,
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub xl: Pixels,
    pub full: Pixels,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            none: px(0.),
            sm: px(3.),
            md: px(6.),
            lg: px(8.),
            xl: px(12.),
            full: px(9999.),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct SpacingTokens {
    pub xxs: Pixels,
    pub xs: Pixels,
    pub sm: Pixels,
    pub md: Pixels,
    pub lg: Pixels,
    pub xl: Pixels,
    pub xxl: Pixels,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xxs: px(2.),
            xs: px(4.),
            sm: px(8.),
            md: px(12.),
            lg: px(16.),
            xl: px(24.),
            xxl: px(32.),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct TextStyleToken {
    pub size: Pixels,
    pub line_height: Pixels,
    pub weight: FontWeight,
}

impl Default for TextStyleToken {
    /// The body text style, matching [`TypographyTokens::md`].
    fn default() -> Self {
        text_style(16., 24.)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct TypographyTokens {
    pub sans: SharedString,
    pub mono: SharedString,
    pub xs: TextStyleToken,
    pub sm: TextStyleToken,
    pub md: TextStyleToken,
    pub lg: TextStyleToken,
    pub xl: TextStyleToken,
    pub mono_md: TextStyleToken,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            sans: ".SystemUIFont".into(),
            mono: default_mono_font_family(),
            xs: text_style(12., 16.),
            sm: text_style(14., 20.),
            md: text_style(16., 24.),
            lg: text_style(18., 28.),
            xl: text_style(20., 28.),
            mono_md: text_style(13., 20.),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ShadowTokens {
    pub sm: Vec<BoxShadow>,
    pub md: Vec<BoxShadow>,
    pub lg: Vec<BoxShadow>,
}

impl ShadowTokens {
    pub fn elevations(color: Hsla) -> Self {
        Self {
            sm: vec![box_shadow(0., 1., 2., 0., color)],
            md: vec![box_shadow(0., 4., 8., -2., color)],
            lg: vec![box_shadow(0., 12., 24., -4., color)],
        }
    }
}

fn text_style(size: f32, line_height: f32) -> TextStyleToken {
    TextStyleToken {
        size: px(size),
        line_height: px(line_height),
        weight: FontWeight::NORMAL,
    }
}

fn default_mono_font_family() -> SharedString {
    if cfg!(target_os = "macos") {
        "Menlo".into()
    } else if cfg!(target_os = "windows") {
        "Consolas".into()
    } else {
        "DejaVu Sans Mono".into()
    }
}

fn box_shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

#[cfg(test)]
mod tests {
    use super::ColorTokens;

    #[test]
    fn default_colors_are_the_light_palette_and_both_palettes_are_readable() {
        let light = ColorTokens::light();
        let dark = ColorTokens::dark();

        assert_eq!(ColorTokens::default(), light);
        assert_eq!(light.background.l, 1.);
        assert!(light.foreground.l < light.background.l);
        assert!(dark.background.l < dark.foreground.l);
        assert_eq!(light.primary.a, 1.);
        assert_eq!(dark.primary.a, 1.);
    }
}
