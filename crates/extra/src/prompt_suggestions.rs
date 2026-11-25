// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Prompt suggestions: prompts a person can send with one click.
//!
//! Two layouts. Cards open an empty conversation: each has an icon, a title
//! and a quieter second line, in one or two columns. Chips follow an answer:
//! short follow-ups in a wrapping row, with an optional button that asks for a
//! new set. Picking one reports its id and the text to send, which is the
//! title unless the suggestion carries a longer prompt.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    h_flex, look, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

/// How long one suggestion takes to appear.
const APPEAR: Duration = Duration::from_millis(220);
/// The delay between one suggestion appearing and the next.
const STAGGER: Duration = Duration::from_millis(40);
/// How far a suggestion rises as it appears.
const RISE: f32 = 6.;

/// One prompt to offer.
#[derive(Clone)]
pub struct Suggestion {
    id: SharedString,
    title: SharedString,
    hint: Option<SharedString>,
    icon: Option<Icon>,
    prompt: Option<SharedString>,
}

impl Suggestion {
    /// A suggestion that sends its title.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            hint: None,
            icon: None,
            prompt: None,
        }
    }

    /// A second, quieter line. Shown on cards only.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// An icon in a tinted square. Shown on cards only.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// The text to send, when it should be longer than the title.
    pub fn prompt(mut self, prompt: impl Into<SharedString>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// The text this suggestion sends.
    pub fn text_to_send(&self) -> SharedString {
        self.prompt.clone().unwrap_or_else(|| self.title.clone())
    }
}

/// Which layout the suggestions take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SuggestionLayout {
    /// Cards for an empty conversation.
    #[default]
    Cards,
    /// Chips for follow-ups under an answer.
    Chips,
}

/// How far along its own appearance suggestion `ix` is, from 0 to 1, when the
/// whole staggered entrance of `count` suggestions is `time` (0 to 1) through.
pub fn appear_progress(time: f32, ix: usize, count: usize) -> f32 {
    let total = entrance(count).as_secs_f32();
    let start = STAGGER.as_secs_f32() * ix as f32;
    let linear = ((time * total - start) / APPEAR.as_secs_f32()).clamp(0., 1.);
    1. - (1. - linear).powi(3)
}

/// How long the staggered entrance of `count` suggestions takes.
fn entrance(count: usize) -> Duration {
    APPEAR + STAGGER * count.saturating_sub(1) as u32
}

type SelectHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type RefreshHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Prompts to send with one click.
#[derive(IntoElement)]
pub struct PromptSuggestions {
    id: ElementId,
    style: StyleRefinement,
    suggestions: Vec<Suggestion>,
    layout: SuggestionLayout,
    columns: usize,
    heading: Option<SharedString>,
    refreshing: bool,
    on_select: Option<SelectHandler>,
    on_refresh: Option<RefreshHandler>,
}

impl PromptSuggestions {
    /// Create the cards layout in one column.
    pub fn new(id: impl Into<ElementId>, suggestions: Vec<Suggestion>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            suggestions,
            layout: SuggestionLayout::Cards,
            columns: 1,
            heading: None,
            refreshing: false,
            on_select: None,
            on_refresh: None,
        }
    }

    /// Show chips instead of cards.
    pub fn chips(mut self) -> Self {
        self.layout = SuggestionLayout::Chips;
        self
    }

    /// Lay cards out in this many columns, one or two. Use two from about
    /// 450px of width.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.clamp(1, 2);
        self
    }

    /// A line above the suggestions, which also names the list.
    pub fn heading(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = Some(heading.into());
        self
    }

    /// A new set is on its way: chips are off and the refresh button spins.
    pub fn refreshing(mut self, refreshing: bool) -> Self {
        self.refreshing = refreshing;
        self
    }

    /// Run with the suggestion's id and the text to send.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Show a button after the chips that asks for a new set.
    pub fn on_refresh(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    fn list_name(&self) -> SharedString {
        match (&self.heading, self.layout) {
            (Some(heading), _) => heading.clone(),
            (None, SuggestionLayout::Cards) => "Suggested prompts".into(),
            (None, SuggestionLayout::Chips) if self.refreshing => {
                "Suggested follow-ups, loading new suggestions".into()
            }
            (None, SuggestionLayout::Chips) => "Suggested follow-ups".into(),
        }
    }
}

impl Styled for PromptSuggestions {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Let `element` rise and fade in at its turn in the staggered entrance.
fn appear(
    element: impl IntoElement,
    key: ElementId,
    ix: usize,
    count: usize,
    still: bool,
) -> AnyElement {
    let slot = div().relative().child(element);
    if still {
        return slot.into_any_element();
    }
    slot.with_animation(key, Animation::new(entrance(count)), move |this, time| {
        let shown = appear_progress(time, ix, count);
        this.opacity(shown).top(px(RISE * (1. - shown)))
    })
    .into_any_element()
}

impl RenderOnce for PromptSuggestions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = cx.reduce_motion();
        let count = self.suggestions.len();
        let list_name = self.list_name();
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let ink = theme.foreground;
        let surface = theme.muted;
        let surface_hover = theme.secondary_hover;
        let tile = look::ink::wash_strong(cx);

        let child_id =
            |part: String| ElementId::NamedChild(self.id.clone().into(), SharedString::from(part));

        match self.layout {
            SuggestionLayout::Chips => {
                let chips = self.suggestions.iter().enumerate().map(|(ix, suggestion)| {
                    let pick = self.on_select.clone();
                    let id = suggestion.id.clone();
                    let text = suggestion.text_to_send();
                    let chip = Button::new(child_id(format!("chip-{}", suggestion.id)))
                        .secondary()
                        .large()
                        .rounded(px(16.))
                        .disabled(self.refreshing)
                        .accessibility_label(suggestion.title.clone())
                        .max_w(px(320.))
                        .pl(px(14.))
                        .pr(px(10.))
                        .gap(px(6.))
                        .font_normal()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_body()
                                .font_normal()
                                .child(suggestion.title.clone()),
                        )
                        .child(
                            Icon::new(CatalogIcon::ArrowUpRight)
                                .size(px(12.))
                                .text_color(quiet),
                        )
                        .on_click(move |_, window, cx| {
                            if let Some(pick) = &pick {
                                pick(&id, &text, window, cx);
                            }
                        });
                    appear(
                        chip,
                        child_id(format!("appear-{}", suggestion.id)),
                        ix,
                        count,
                        still,
                    )
                });
                let refresh = self.on_refresh.clone().map(|refresh| {
                    Button::new(child_id("refresh".into()))
                        .ghost()
                        .icon(Icon::new(CatalogIcon::Renew))
                        .loading(self.refreshing)
                        .disabled(self.refreshing)
                        .rounded(px(14.))
                        .tooltip("More suggestions")
                        .accessibility_label("More suggestions")
                        .on_click(move |_, window, cx| refresh(window, cx))
                });

                v_flex()
                    .id(self.id.clone())
                    .w_full()
                    .gap_2()
                    .when_some(self.heading.clone(), |this, heading| {
                        this.child(
                            div()
                                .px_1()
                                .text_label_small()
                                .text_color(quiet)
                                .child(heading),
                        )
                    })
                    .child(
                        h_flex()
                            .id(child_id("list".into()))
                            .flex_wrap()
                            .gap(px(6.))
                            .role(Role::List)
                            .aria_label(list_name)
                            .children(chips)
                            .children(refresh),
                    )
                    .refine_style(&self.style)
                    .into_any_element()
            }
            SuggestionLayout::Cards => {
                let columns = self.columns;
                let cards = self.suggestions.iter().enumerate().map(|(ix, suggestion)| {
                    let pick = self.on_select.clone();
                    let id = suggestion.id.clone();
                    let text = suggestion.text_to_send();
                    // The arrow shows while the pointer is over the card.
                    let hovered = window.use_keyed_state(
                        child_id(format!("hover-{}", suggestion.id)),
                        cx,
                        |_, _| false,
                    );
                    let over = *hovered.read(cx);
                    let arrow = look::hover_fade(
                        child_id(format!("arrow-{}", suggestion.id)),
                        over,
                        window,
                        cx,
                    );
                    let spoken = match &suggestion.hint {
                        Some(hint) => format!("{}, {}", suggestion.title, hint),
                        None => suggestion.title.to_string(),
                    };
                    let card = Button::new(child_id(format!("card-{}", suggestion.id)))
                        .ghost()
                        .rounded(px(18.))
                        .accessibility_label(spoken)
                        .w_full()
                        .h_auto()
                        .justify_start()
                        .p(px(6.))
                        .pr(px(14.))
                        .gap(px(12.))
                        .bg(look::mix(surface, surface_hover, arrow))
                        .font_normal()
                        .when_some(suggestion.icon.clone(), |this, icon| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .size(px(36.))
                                    .rounded(px(16.))
                                    .bg(tile)
                                    .text_color(quiet)
                                    .child(icon.size(px(16.))),
                            )
                        })
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .py_1()
                                .items_start()
                                .when(suggestion.icon.is_none(), |this| this.pl_2())
                                .child(
                                    div()
                                        .w_full()
                                        .truncate()
                                        .text_label()
                                        .text_color(ink)
                                        .child(suggestion.title.clone()),
                                )
                                .when_some(suggestion.hint.clone(), |this, hint| {
                                    this.child(
                                        div()
                                            .w_full()
                                            .truncate()
                                            .text_caption()
                                            .text_color(quiet)
                                            .child(hint),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .relative()
                                .flex_none()
                                .left(px(2. * arrow))
                                .opacity(arrow)
                                .child(
                                    Icon::new(CatalogIcon::ArrowUpRight)
                                        .size(px(14.))
                                        .text_color(quiet),
                                ),
                        )
                        .on_hover(move |over, _, cx| {
                            hovered.update(cx, |state, cx| {
                                *state = *over;
                                cx.notify();
                            });
                        })
                        .on_click(move |_, window, cx| {
                            if let Some(pick) = &pick {
                                pick(&id, &text, window, cx);
                            }
                        });
                    let slot = appear(
                        card,
                        child_id(format!("appear-{}", suggestion.id)),
                        ix,
                        count,
                        still,
                    );
                    div()
                        .when(columns == 1, |this| this.w_full())
                        // Two columns share the row less the 8px gap between them.
                        .when(columns == 2, |this| this.min_w_0())
                        .child(slot)
                });

                v_flex()
                    .id(self.id.clone())
                    .w_full()
                    .max_w(px(576.))
                    .gap_3()
                    .when_some(self.heading.clone(), |this, heading| {
                        this.child(div().px_1().text_title().child(heading))
                    })
                    .child(
                        div()
                            .id(child_id("list".into()))
                            .when(columns == 1, |this| this.flex().flex_col().gap_2())
                            .when(columns == 2, |this| this.grid().grid_cols(2).gap_2())
                            .role(Role::List)
                            .aria_label(list_name)
                            .children(cards),
                    )
                    .refine_style(&self.style)
                    .into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_suggestion_sends_its_prompt_or_its_title() {
        let plain = Suggestion::new("a", "Plan the week");
        assert_eq!(plain.text_to_send(), "Plan the week");
        let long =
            Suggestion::new("b", "Plan the week").prompt("Plan my week around two deadlines");
        assert_eq!(long.text_to_send(), "Plan my week around two deadlines");
    }

    #[test]
    fn suggestions_appear_one_after_another() {
        // Four suggestions: 220ms each, 40ms apart, 340ms in all.
        assert_eq!(entrance(4), Duration::from_millis(340));
        assert_eq!(entrance(0), APPEAR);
        assert_eq!(appear_progress(0., 0, 4), 0.);
        assert_eq!(appear_progress(1., 3, 4), 1.);
        // Half way through, the first is done and the last has barely begun.
        assert_eq!(appear_progress(0.65, 0, 4), 1.);
        assert!(appear_progress(0.5, 3, 4) < 0.6);
        assert!(appear_progress(0.3, 0, 4) > appear_progress(0.3, 1, 4));
        assert_eq!(appear_progress(0.1, 3, 4), 0.);
    }

    #[test]
    fn the_list_is_named_by_its_heading_or_its_layout() {
        let cards = PromptSuggestions::new("s", Vec::new());
        assert_eq!(cards.list_name(), "Suggested prompts");
        let chips = PromptSuggestions::new("s", Vec::new()).chips();
        assert_eq!(chips.list_name(), "Suggested follow-ups");
        let loading = PromptSuggestions::new("s", Vec::new())
            .chips()
            .refreshing(true);
        assert!(loading.list_name().contains("loading"));
        let named = PromptSuggestions::new("s", Vec::new()).heading("Try asking");
        assert_eq!(named.list_name(), "Try asking");
        assert_eq!(
            PromptSuggestions::new("s", Vec::new()).columns(5).columns,
            2
        );
    }
}
