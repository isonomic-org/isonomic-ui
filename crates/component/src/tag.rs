// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{ColorName, Sizable, Size, StyledExt, theme::ActiveTheme as _};
use gpui::{
    AbsoluteLength, AnyElement, App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, rems, transparent_white,
};

/// The variant of the Tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagVariant {
    Primary,
    #[default]
    Secondary,
    Danger,
    Success,
    Warning,
    Info,
    Color(ColorName),
    Custom {
        color: Hsla,
        foreground: Hsla,
        border: Hsla,
    },
}

impl TagVariant {
    fn bg(&self, cx: &App) -> Hsla {
        match self {
            // Status tones are soft tints with the tone's own colour as ink.
            Self::Primary => cx.theme().primary,
            Self::Secondary => {
                cx.theme()
                    .foreground
                    .alpha(if cx.theme().is_dark() { 0.08 } else { 0.06 })
            }
            Self::Danger => cx
                .theme()
                .danger
                .alpha(if cx.theme().is_dark() { 0.16 } else { 0.1 }),
            Self::Success => {
                cx.theme()
                    .success
                    .alpha(if cx.theme().is_dark() { 0.16 } else { 0.1 })
            }
            Self::Warning => cx.theme().warning.alpha(0.16),
            Self::Info => cx
                .theme()
                .info
                .alpha(if cx.theme().is_dark() { 0.16 } else { 0.1 }),
            Self::Color(color) => {
                if cx.theme().is_dark() {
                    color.scale(950).opacity(0.5)
                } else {
                    color.scale(50)
                }
            }
            Self::Custom { color, .. } => *color,
        }
    }

    fn border(&self, cx: &App) -> Hsla {
        match self {
            Self::Primary => cx.theme().primary,
            Self::Secondary => cx.theme().border,
            Self::Danger => cx.theme().danger,
            Self::Success => cx.theme().success,
            Self::Warning => cx.theme().warning,
            Self::Info => cx.theme().info,
            Self::Color(color) => {
                if cx.theme().is_dark() {
                    color.scale(800).opacity(0.5)
                } else {
                    color.scale(200)
                }
            }
            Self::Custom { border, .. } => *border,
        }
    }

    fn fg(&self, outline: bool, cx: &App) -> Hsla {
        match self {
            Self::Primary => {
                if outline {
                    cx.theme().primary
                } else {
                    cx.theme().primary_foreground
                }
            }
            Self::Secondary => {
                let _ = outline;
                crate::look::ink::muted(cx)
            }
            Self::Danger => cx.theme().danger,
            Self::Success => cx.theme().success,
            Self::Warning => cx.theme().warning,
            Self::Info => cx.theme().info,
            Self::Color(color) => {
                if cx.theme().is_dark() {
                    color.scale(300)
                } else {
                    color.scale(600)
                }
            }
            Self::Custom { foreground, .. } => *foreground,
        }
    }
}

/// Tag is a small status indicator.
///
/// Only support: Medium, Small
#[derive(IntoElement)]
pub struct Tag {
    style: StyleRefinement,
    variant: TagVariant,
    outline: bool,
    size: Size,
    rounded: Option<AbsoluteLength>,
    mono: bool,
    children: Vec<AnyElement>,
}
impl Tag {
    /// Create a new Tag.
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            variant: TagVariant::default(),
            outline: false,
            size: Size::default(),
            rounded: None,
            mono: false,
            children: Vec::new(),
        }
    }

    /// Create a new tag with default variant ([`TagVariant::Primary`]).
    pub fn primary() -> Self {
        Self::new().with_variant(TagVariant::Primary)
    }

    /// Create a new tag with default variant ([`TagVariant::Secondary`]).
    pub fn secondary() -> Self {
        Self::new().with_variant(TagVariant::Secondary)
    }

    /// Create a new tag with default variant ([`TagVariant::Danger`]).
    pub fn danger() -> Self {
        Self::new().with_variant(TagVariant::Danger)
    }

    /// Create a new tag with default variant ([`TagVariant::Success`]).
    pub fn success() -> Self {
        Self::new().with_variant(TagVariant::Success)
    }

    /// Create a new tag with default variant ([`TagVariant::Warning`]).
    pub fn warning() -> Self {
        Self::new().with_variant(TagVariant::Warning)
    }

    /// Create a new tag with default variant ([`TagVariant::Info`]).
    pub fn info() -> Self {
        Self::new().with_variant(TagVariant::Info)
    }

    /// Create a new tag with default variant ([`TagVariant::Custom`]).
    pub fn custom(color: Hsla, foreground: Hsla, border: Hsla) -> Self {
        Self::new().with_variant(TagVariant::Custom {
            color,
            foreground,
            border,
        })
    }

    /// Create a new tag with default variant ([`TagVariant::Color`]).
    pub fn color(color: impl Into<ColorName>) -> Self {
        Self::new().with_variant(TagVariant::Color(color.into()))
    }

    /// Set the variant of the Tag.
    pub fn with_variant(mut self, variant: TagVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Use outline style
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// Set the text in the mono family at regular weight, for identifiers,
    /// versions, hashes and keys: "v2.4.1", "INV-2240", "a3f9c1e".
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Set rounded corners.
    pub fn rounded(mut self, radius: impl Into<AbsoluteLength>) -> Self {
        self.rounded = Some(radius.into());
        self
    }

    /// Set rounded full
    pub fn rounded_full(mut self) -> Self {
        self.rounded = Some(rems(1.).into());
        self
    }
}

impl Sizable for Tag {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl ParentElement for Tag {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Tag {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bg = if self.outline {
            transparent_white()
        } else {
            self.variant.bg(cx)
        };
        let fg = self.variant.fg(self.outline, cx);
        let border = self.variant.border(cx);
        // A pill: 18px tall with 11px medium text at medium, a tinted fill and
        // no edge unless it is outlined.
        let theme_radius = cx.theme().radius;
        let rounded = self.rounded.unwrap_or(
            if theme_radius <= px(0.) {
                px(0.)
            } else {
                match self.size {
                    Size::XSmall | Size::Small => px(4.),
                    Size::Large => px(5.),
                    _ => px(4.),
                }
            }
            .into(),
        );

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .whitespace_nowrap()
            .when(self.outline, |this| this.border_1().border_color(border))
            .map(|this| match self.size {
                Size::XSmall | Size::Small => this
                    .h(px(16.))
                    .px(px(5.))
                    .text_size(px(10.))
                    .line_height(px(14.)),
                Size::Large => this
                    .h(px(20.))
                    .px(px(7.))
                    .text_size(px(12.))
                    .line_height(px(16.)),
                _ => this
                    .h(px(18.))
                    .px(px(6.))
                    .text_size(px(11.))
                    .line_height(px(16.)),
            })
            .font_weight(FontWeight::MEDIUM)
            .when(self.mono, |this| {
                this.font_family(cx.theme().mono_font_family.clone())
                    .font_weight(FontWeight::NORMAL)
            })
            .bg(bg)
            .text_color(fg)
            .rounded(rounded)
            .refine_style(&self.style)
            .children(self.children)
    }
}
