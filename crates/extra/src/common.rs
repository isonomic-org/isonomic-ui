// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Small pieces shared by the components in this crate.

use isonomic_ui::component::{ActiveTheme as _, Icon, TextRole as _, h_flex, look};
use isonomic_ui::{
    AnyElement, App, BoxShadow, Div, ElementId, Hsla, IntoElement, ParentElement as _, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};

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

/// Outer frame of a component built from layers. Layers are told apart by
/// their fill alone: headers and footers sit in the shell's padding, content
/// sits in a [`panel`].
pub fn shell(cx: &App) -> Div {
    div()
        .w_full()
        .p(px(6.))
        .rounded(px(22.))
        .bg(cx.theme().muted)
}

/// Main content area inside a [`shell`].
pub fn panel(cx: &App) -> Div {
    div().rounded(px(16.)).bg(cx.theme().background)
}

/// One level deeper than a [`panel`]: code, payloads, details.
pub fn inset(cx: &App) -> Div {
    div().rounded(px(12.)).bg(look::ink::wash_strong(cx))
}

/// An icon in a tinted square, 36px with a 16px icon.
pub fn icon_tile(icon: impl Into<Icon>, cx: &App) -> Div {
    let icon: Icon = icon.into();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(36.))
        .rounded(px(16.))
        .bg(look::ink::wash_strong(cx))
        .text_color(look::ink::icon(cx))
        .child(icon.size(px(16.)))
}

/// Header row of a [`shell`]: an optional icon tile, a title over a quieter
/// line, and whatever belongs at the far end.
pub fn shell_header(
    icon: Option<Icon>,
    title: impl IntoElement,
    subtitle: Option<AnyElement>,
    aside: Option<AnyElement>,
    cx: &App,
) -> Div {
    let quiet = cx.theme().muted_foreground;
    let no_icon = icon.is_none();
    h_flex()
        .gap_3()
        .pl(px(6.))
        .pr(px(14.))
        .pt(px(6.))
        .pb_3()
        .when_some(icon, |this, icon| this.child(icon_tile(icon, cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .when(no_icon, |this| this.pl_2())
                .child(div().truncate().text_title().child(title))
                .when_some(subtitle, |this, subtitle| {
                    this.child(
                        div()
                            .truncate()
                            .text_body()
                            .text_color(quiet)
                            .child(subtitle),
                    )
                }),
        )
        .when_some(aside, |this, aside| {
            this.child(h_flex().flex_none().gap_2().child(aside))
        })
}

/// The hover wash at `fade` of its strength, for a quiet row, chip or icon
/// button whose hover is eased with `look::tracked_hover` instead of
/// snapping. Set it before any selected fill, so the selected fill wins.
pub fn hover_wash(fade: f32, cx: &App) -> Hsla {
    faded(look::ink::wash(cx), fade)
}

/// `color` let in by an eased hover: clear at 0, itself at 1.
pub fn faded(color: Hsla, fade: f32) -> Hsla {
    color.alpha(color.a * fade.clamp(0., 1.))
}

/// Eased hovers for a run of parts of `id` (rows, chips, segments), one per
/// item of `enabled`, named `<part>-<index>`. Take each with `Option::take`
/// while building the part. Call before borrowing the theme.
pub fn tracked_hovers(
    id: &ElementId,
    part: &str,
    enabled: impl IntoIterator<Item = bool>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<Option<(f32, look::HoverListener)>> {
    enabled
        .into_iter()
        .enumerate()
        .map(|(ix, enabled)| {
            Some(look::tracked_hover(
                ElementId::NamedChild(id.clone().into(), format!("{part}-{ix}").into()),
                enabled,
                window,
                cx,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Tone;

    #[test]
    fn tone_defaults_to_neutral() {
        assert_eq!(Tone::default(), Tone::Neutral);
    }
}
