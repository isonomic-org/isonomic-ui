// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Recommendation card: one suggested option with the reason for it, how sure
//! the suggestion is, alternatives to switch to, and accept with undo.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How sure a suggestion is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Confidence {
    /// Nothing to go on.
    #[default]
    Unknown,
    /// A weak hint.
    Low,
    /// A reasonable basis.
    Medium,
    /// Strong evidence.
    High,
}

impl Confidence {
    /// How many of the three bars are lit.
    fn level(self) -> usize {
        match self {
            Confidence::Unknown => 0,
            Confidence::Low => 1,
            Confidence::Medium => 2,
            Confidence::High => 3,
        }
    }

    /// The confidence in words, shown next to the bars.
    fn label(self) -> &'static str {
        match self {
            Confidence::Unknown => "Unknown confidence",
            Confidence::Low => "Low confidence",
            Confidence::Medium => "Medium confidence",
            Confidence::High => "High confidence",
        }
    }
}

/// One option the card can suggest.
#[derive(Debug, Clone)]
pub struct RecommendationOption {
    id: SharedString,
    label: SharedString,
    reason: SharedString,
    confidence: Confidence,
}

impl RecommendationOption {
    /// Create an option. The label is its short name, used in the accept button.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            reason: SharedString::default(),
            confidence: Confidence::Unknown,
        }
    }

    /// One sentence on why this option fits.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = reason.into();
        self
    }

    /// How sure the suggestion is.
    pub fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }
}

type OptionHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A card that recommends the first option and offers the rest as alternatives.
#[derive(IntoElement)]
pub struct RecommendationCard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    icon: Option<Icon>,
    options: Vec<RecommendationOption>,
    default_open: bool,
    on_accept: Option<OptionHandler>,
    on_undo: Option<OptionHandler>,
}

impl RecommendationCard {
    /// Create a card titled "Recommendation" with no options. A card without
    /// options renders nothing.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Recommendation".into(),
            icon: None,
            options: Vec::new(),
            default_open: true,
            on_accept: None,
            on_undo: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Replace the mark in front of the heading.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Add an option. The first one added is the recommendation.
    pub fn option(mut self, option: RecommendationOption) -> Self {
        self.options.push(option);
        self
    }

    /// Whether the reason is visible to begin with. On by default.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Run when an option is accepted, with its id.
    pub fn on_accept(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_accept = Some(Rc::new(handler));
        self
    }

    /// Run when an accept is undone, with the option's id.
    pub fn on_undo(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }
}

impl Styled for RecommendationCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct CardState {
    /// Position of the option on show. By position, so equal ids stay apart.
    chosen: usize,
    accepted: Option<usize>,
    open: bool,
}

/// Lowercase a label for use mid-sentence, keeping acronyms: "Full refresh"
/// becomes "full refresh" while "CSV export" stays as it is.
fn lower_first(label: &str) -> String {
    let mut chars = label.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => {
            first.to_lowercase().chain([second]).chain(chars).collect()
        }
        _ => label.to_string(),
    }
}

/// Three bars of rising height, lit up to `level`.
fn bars(level: usize, lit: Hsla, unlit: Hsla) -> impl IntoElement {
    h_flex()
        .flex_none()
        .items_end()
        .gap(px(2.))
        .h(px(10.))
        .children((1..=3).map(move |bar| {
            div()
                .w(px(3.))
                .h(px(1. + 3. * bar as f32))
                .rounded(px(1.))
                .bg(if level >= bar { lit } else { unlit })
        }))
}

impl RenderOnce for RecommendationCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let pill = theme.muted;
        let accent = theme.info;
        let success = theme.success;
        let radius = theme.radius_lg;

        let default_open = self.default_open;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| CardState {
            chosen: 0,
            accepted: None,
            open: default_open,
        });
        let (chosen, accepted, open) = {
            let state = state.read(cx);
            (state.chosen, state.accepted, state.open)
        };
        let root = v_flex().id(self.id.clone()).w_full().max_w(px(440.));
        let at = if chosen < self.options.len() {
            chosen
        } else {
            0
        };
        let Some(current) = self.options.get(at).cloned() else {
            return root.refine_style(&self.style);
        };
        let done = accepted == Some(at);

        let head_id = ElementId::NamedChild(self.id.clone().into(), "head".into());
        let head_focus = window
            .use_keyed_state((head_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let head_focused = head_focus.is_focused(window);
        let head_name = if at == 0 {
            self.title.to_string()
        } else {
            format!("{}, Alternative", self.title)
        };
        let head = Pressable::new(head_id)
            .accessibility_label(head_name)
            .aria_expanded(open)
            .track_focus(&head_focus)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |state, cx| {
                        state.open = !state.open;
                        cx.notify();
                    })
                }
            })
            .justify_start()
            .gap_2()
            .w_full()
            .h(px(44.))
            .px_4()
            .line_height(relative(1.4))
            .cursor_pointer()
            .when(head_focused, |this| this.focus_ring_style(window, cx))
            .child(
                self.icon
                    .unwrap_or_else(|| Icon::new(IconName::Star))
                    .small()
                    .flex_none()
                    .text_color(accent),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_medium()
                    .child(self.title),
            )
            .when(at != 0, |this| {
                this.child(
                    div()
                        .flex_none()
                        .px_1p5()
                        .rounded(px(5.))
                        .bg(pill)
                        .text_color(quiet)
                        .text_caption()
                        .child("Alternative"),
                )
            })
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .small()
                .flex_none()
                .text_color(quiet),
            );

        let alternatives = (self.options.len() > 1 && !done).then(|| {
            let labels: Vec<SharedString> = self
                .options
                .iter()
                .enumerate()
                .map(|(ix, option)| match ix {
                    0 => format!("{} (recommended)", option.label).into(),
                    _ => option.label.clone(),
                })
                .collect();
            let state = state.clone();
            Button::new((self.id.clone(), "alternatives"))
                .outline()
                .small()
                .label("Alternatives")
                .dropdown_menu(move |mut menu, _, _| {
                    for (ix, label) in labels.iter().enumerate() {
                        let state = state.clone();
                        menu = menu.item(
                            PopupMenuItem::new(label.clone())
                                .checked(ix == at)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.chosen = ix;
                                        state.open = true;
                                        cx.notify();
                                    })
                                }),
                        );
                    }
                    menu
                })
        });

        let id = current.id.clone();
        let decide = |accept: bool, handler: Option<OptionHandler>| {
            let state = state.clone();
            let id = id.clone();
            move |_: &isonomic_ui::ClickEvent, window: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.accepted = accept.then_some(at);
                    cx.notify();
                });
                if let Some(handler) = &handler {
                    handler(&id, window, cx);
                }
            }
        };
        let actions = if done {
            h_flex()
                .gap_2()
                .min_w_0()
                .child(
                    h_flex()
                        .gap_1p5()
                        .min_w_0()
                        .text_color(success)
                        .child(Icon::new(IconName::Check).small().flex_none())
                        .child(
                            div()
                                .truncate()
                                .child(format!("{} accepted", current.label)),
                        ),
                )
                .child(
                    Button::new((self.id.clone(), "undo"))
                        .outline()
                        .small()
                        .icon(IconName::Undo)
                        .label("Undo")
                        .on_click(decide(false, self.on_undo)),
                )
        } else {
            h_flex().gap_2().min_w_0().children(alternatives).child(
                Button::new((self.id.clone(), "accept"))
                    .primary()
                    .small()
                    .label(format!("Accept {}", lower_first(&current.label)))
                    .on_click(decide(true, self.on_accept)),
            )
        };

        let level = current.confidence.level();
        root.overflow_hidden()
            .rounded(radius)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .child(head)
            .when(open && !current.reason.is_empty(), |this| {
                this.child(
                    div()
                        .px_4()
                        .pb_3()
                        .text_color(quiet)
                        .child(current.reason.clone()),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .text_color(quiet)
                            .child(bars(level, quiet, line))
                            .child(current.confidence.label()),
                    )
                    .child(actions),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_lowercased_mid_sentence() {
        assert_eq!(lower_first("Full refresh"), "full refresh");
        assert_eq!(lower_first("Incremental"), "incremental");
    }

    #[test]
    fn acronyms_keep_their_capitals() {
        assert_eq!(lower_first("CSV export"), "CSV export");
        assert_eq!(lower_first("S3 sync"), "S3 sync");
        assert_eq!(lower_first("A"), "A");
        assert_eq!(lower_first(""), "");
    }

    #[test]
    fn confidence_lights_one_bar_per_step() {
        let levels = [
            Confidence::Unknown,
            Confidence::Low,
            Confidence::Medium,
            Confidence::High,
        ]
        .map(Confidence::level);
        assert_eq!(levels, [0, 1, 2, 3]);
        assert_eq!(Confidence::High.label(), "High confidence");
    }
}
