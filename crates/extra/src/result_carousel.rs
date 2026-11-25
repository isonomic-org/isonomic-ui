// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Result carousel: search results as cards in a row that scrolls sideways.
//!
//! Each card has a tinted picture area with an optional badge, a title, a
//! quieter line, a rating and a price, and an action button; the first card's
//! action is the primary one. Arrows in the header move one card at a time
//! and switch off at either end. A compact form drops the picture.

use std::rc::Rc;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition};
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    rolling_number::RollingNumber,
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, linear_color_stop,
    linear_gradient, point, prelude::FluentBuilder as _, px,
};

use crate::common::{shell, shell_header};

const CARD: f32 = 240.;
const CARD_COMPACT: f32 = 208.;
const GAP: f32 = 6.;

/// One result.
#[derive(Clone)]
pub struct ResultItem {
    id: SharedString,
    title: SharedString,
    meta: Option<SharedString>,
    price: Option<SharedString>,
    rating: Option<f32>,
    tint: Option<Hsla>,
    badge: Option<SharedString>,
    action: Option<SharedString>,
}

impl ResultItem {
    /// A result with a title.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            meta: None,
            price: None,
            rating: None,
            tint: None,
            badge: None,
            action: None,
        }
    }

    /// A quieter line under the title.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// The price, already written out.
    pub fn price(mut self, price: impl Into<SharedString>) -> Self {
        self.price = Some(price.into());
        self
    }

    /// A rating out of five.
    pub fn rating(mut self, rating: f32) -> Self {
        self.rating = Some(rating.clamp(0., 5.));
        self
    }

    /// The colour of the picture area.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }

    /// A short label over the picture area.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// The action button's words. "Select" by default.
    pub fn action(mut self, action: impl Into<SharedString>) -> Self {
        self.action = Some(action.into());
        self
    }
}

/// Where one step from `from` lands: one card and gap along, kept between the
/// start and `most`.
pub fn step_target(from: f32, direction: f32, card: f32, most: f32) -> f32 {
    (from + direction * (card + GAP)).clamp(0., most.max(0.))
}

/// Whether the row is at its start and at its end.
pub fn at_edges(scrolled: f32, most: f32) -> (bool, bool) {
    (scrolled <= 2., scrolled >= most - 2.)
}

/// What a screen reader hears for a card.
pub fn slide_label(ix: usize, count: usize, item: &ResultItem) -> String {
    let mut spoken = format!("{} of {count}: {}", ix + 1, item.title);
    if let Some(rating) = item.rating {
        spoken.push_str(&format!(", {rating:.1} out of 5"));
    }
    if let Some(price) = &item.price {
        spoken.push_str(&format!(", {price}"));
    }
    spoken
}

/// A scroll in progress from an arrow press.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Glide {
    from: f32,
    to: f32,
    turn: u64,
    live: bool,
}

type ItemHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Results as cards in a row.
#[derive(IntoElement)]
pub struct ResultCarousel {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    items: Vec<ResultItem>,
    compact: bool,
    on_action: Option<ItemHandler>,
    on_open: Option<ItemHandler>,
}

impl ResultCarousel {
    /// Create a carousel.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        items: Vec<ResultItem>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            items,
            compact: false,
            on_action: None,
            on_open: None,
        }
    }

    /// A line under the title in place of the result count.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Narrower cards without the picture area.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Show an action button on each card; it reports the result's id.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }

    /// Make titles and pictures open the result; reports its id.
    pub fn on_open(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl Styled for ResultCarousel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ResultCarousel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let still = cx.reduce_motion();
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let surface = theme.background;
        let star = theme.warning;
        let default_tint = theme.chart_1;
        let wash = look::ink::wash_strong(cx);
        let slide_time = theme.motion_tokens().duration_slow;
        let card = if self.compact { CARD_COMPACT } else { CARD };
        let count = self.items.len();

        let handle = window
            .use_keyed_state(part("scroll".into()), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let glide = window.use_keyed_state(part("glide".into()), cx, |_, _| Glide {
            from: 0.,
            to: 0.,
            turn: 0,
            live: false,
        });
        let now = *glide.read(cx);
        if now.live {
            let along = Presence::new(part(format!("glide-{}", now.turn)), true)
                .transition(Transition::new(slide_time))
                .sample(window, cx)
                .progress;
            let along = if still { 1. } else { along };
            handle.set_offset(point(px(-(now.from + (now.to - now.from) * along)), px(0.)));
            if along >= 1. {
                glide.update(cx, |state, _| state.live = false);
            }
        }
        let scrolled = if now.live {
            now.to
        } else {
            -f32::from(handle.offset().x)
        };
        let most = f32::from(handle.max_offset().x);
        let (at_start, at_end) = at_edges(scrolled, most);

        let arrow = |name: &'static str, direction: f32, off: bool| {
            let handle = handle.clone();
            let glide = glide.clone();
            Button::new(part(name.into()))
                .secondary()
                .small()
                .icon(Icon::new(if direction < 0. {
                    CatalogIcon::ChevronLeft
                } else {
                    CatalogIcon::ChevronRight
                }))
                .accessibility_label(if direction < 0. {
                    "Previous results"
                } else {
                    "Next results"
                })
                .disabled(off)
                .on_click(move |_, window, cx| {
                    let from = -f32::from(handle.offset().x);
                    let most = f32::from(handle.max_offset().x);
                    glide.update(cx, |state, _| {
                        *state = Glide {
                            from,
                            to: step_target(from, direction, card, most),
                            turn: state.turn + 1,
                            live: true,
                        };
                    });
                    window.refresh();
                })
        };

        let cards = self.items.iter().enumerate().map(|(ix, item)| {
            let tint = item.tint.unwrap_or(default_tint);
            let open = self.on_open.clone();
            let picture = (!self.compact).then(|| {
                let open = open.clone();
                let id = item.id.clone();
                div()
                    .id(part(format!("picture-{}", item.id)))
                    .relative()
                    .m(px(6.))
                    .mb_0()
                    .h(px((card - 12.) * 0.75))
                    .rounded(px(12.))
                    .bg(linear_gradient(
                        160.,
                        linear_color_stop(tint.opacity(0.45), 0.),
                        linear_color_stop(wash, 1.),
                    ))
                    .when(open.is_some(), |this| this.cursor_pointer())
                    .on_click(move |_, window, cx| {
                        if let Some(open) = &open {
                            open(&id, window, cx);
                        }
                    })
                    .when_some(item.badge.clone(), |this, badge| {
                        this.child(
                            div()
                                .absolute()
                                .left_2()
                                .top_2()
                                .px_2()
                                .py(px(2.))
                                .rounded_full()
                                .bg(surface.opacity(0.9))
                                .text_size(px(11.))
                                .line_height(px(16.))
                                .font_medium()
                                .child(badge),
                        )
                    })
            });
            let title = match open.clone() {
                Some(open) => {
                    let id = item.id.clone();
                    div()
                        .id(part(format!("title-{}", item.id)))
                        .test_support()
                        .truncate()
                        .cursor_pointer()
                        .hover(|style| style.underline())
                        .role(Role::Link)
                        .aria_label(item.title.clone())
                        .tab_index(0)
                        .on_click(move |_, window, cx| open(&id, window, cx))
                        .child(item.title.clone())
                        .into_any_element()
                }
                None => div()
                    .truncate()
                    .child(item.title.clone())
                    .into_any_element(),
            };
            let action = self.on_action.clone().map(|action| {
                let id = item.id.clone();
                let button = Button::new(part(format!("action-{}", item.id))).small();
                if ix == 0 {
                    button.primary()
                } else {
                    button.secondary()
                }
                .label(item.action.clone().unwrap_or_else(|| "Select".into()))
                .w_full()
                .mt(px(10.))
                .on_click(move |_, window, cx| action(&id, window, cx))
            });
            let body = v_flex()
                .flex_1()
                .gap_1()
                .p(px(14.))
                .child(
                    div()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .font_medium()
                        .child(title),
                )
                .when_some(item.meta.clone(), |this, meta| {
                    this.child(
                        div()
                            .truncate()
                            .text_caption()
                            .text_color(quiet)
                            .child(meta),
                    )
                })
                .child(
                    h_flex()
                        .mt_auto()
                        .gap_2()
                        .pt(px(10.))
                        .when_some(item.rating, |this, rating| {
                            this.child(
                                h_flex()
                                    .gap_1()
                                    .text_caption()
                                    .child(
                                        Icon::new(CatalogIcon::StarFilled)
                                            .size(px(14.))
                                            .text_color(star),
                                    )
                                    .child(format!("{rating:.1}")),
                            )
                        })
                        .when_some(item.price.clone(), |this, price| {
                            this.child(div().ml_auto().text_label().child(price))
                        }),
                )
                .children(action);
            let slide = v_flex()
                .id(part(format!("card-{}", item.id)))
                .relative()
                .flex_none()
                .w(px(card))
                .rounded(px(16.))
                .bg(surface)
                .role(Role::Group)
                .aria_label(slide_label(ix, count, item))
                .children(picture)
                .child(body);
            if still {
                slide.into_any_element()
            } else {
                // The first six cards rise in one after another.
                let delay = 0.05 * ix.min(5) as f32;
                slide
                    .with_animation(
                        part(format!("appear-{}", item.id)),
                        Animation::new(std::time::Duration::from_millis(490)),
                        move |this, time| {
                            let linear = ((time * 0.49 - delay) / 0.24).clamp(0., 1.);
                            let shown = 1. - (1. - linear).powi(3);
                            this.opacity(shown).top(px(8. * (1. - shown)))
                        },
                    )
                    .into_any_element()
            }
        });
        let cards: Vec<AnyElement> = cards.collect();

        let subtitle = match self.subtitle.clone() {
            Some(line) => line.into_any_element(),
            None => h_flex()
                .gap_1()
                .child(
                    RollingNumber::new(part("count".into()), count.to_string())
                        .line_height(px(20.)),
                )
                .child(if count == 1 { "result" } else { "results" })
                .into_any_element(),
        };

        shell(cx)
            .id(self.id.clone())
            .max_w(px(672.))
            .role(Role::Region)
            .aria_label(format!("{}, {count} results", self.title))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Search)),
                self.title.clone(),
                Some(subtitle),
                Some(
                    h_flex()
                        .gap_2()
                        .child(arrow("previous", -1., at_start))
                        .child(arrow("next", 1., at_end))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                h_flex()
                    .id(part("track".into()))
                    .items_stretch()
                    .gap(px(GAP))
                    .rounded(px(16.))
                    .overflow_x_scroll()
                    .track_scroll(&handle)
                    .on_scroll(|_, window, _| window.refresh())
                    .children(cards),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arrow_moves_one_card_and_stops_at_the_ends() {
        assert_eq!(step_target(0., 1., 240., 800.), 246.);
        assert_eq!(step_target(246., -1., 240., 800.), 0.);
        assert_eq!(step_target(700., 1., 240., 800.), 800.);
        assert_eq!(step_target(100., -1., 240., 800.), 0.);
        // Nothing to scroll.
        assert_eq!(step_target(0., 1., 240., 0.), 0.);
    }

    #[test]
    fn arrows_switch_off_at_either_end() {
        assert_eq!(at_edges(0., 800.), (true, false));
        assert_eq!(at_edges(400., 800.), (false, false));
        assert_eq!(at_edges(799., 800.), (false, true));
        assert_eq!(at_edges(0., 0.), (true, true));
    }

    #[test]
    fn a_card_is_read_with_its_place_rating_and_price() {
        let item = ResultItem::new("a", "Harbour loft")
            .rating(4.72)
            .price("€140");
        assert_eq!(
            slide_label(1, 5, &item),
            "2 of 5: Harbour loft, 4.7 out of 5, €140"
        );
        assert_eq!(
            slide_label(0, 1, &ResultItem::new("b", "Garden room")),
            "1 of 1: Garden room"
        );
        assert_eq!(ResultItem::new("c", "x").rating(9.).rating, Some(5.));
    }
}
