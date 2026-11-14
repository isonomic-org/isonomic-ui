//! Card: a self-contained surface with a header, content and a footer.
//!
//! A card with an [`Card::on_click`] handler is one click target: the whole
//! surface responds to the pointer, takes keyboard focus and draws its focus
//! ring around itself. Buttons placed inside stay separate targets.

use std::rc::Rc;

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable, Selectable, StyledExt as _, ThemeStyled as _, h_flex, v_flex,
};
use isonomic_ui::{
    AnyElement, App, BoxShadow, ClickEvent, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative,
};

use crate::common::{lifted_shadow, resting_shadow};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A surface that groups one thing: its name, a line about it, content and actions.
#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    style: StyleRefinement,
    header: Option<CardHeader>,
    children: Vec<AnyElement>,
    footer: Option<CardFooter>,
    selected: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
    accessibility_label: Option<SharedString>,
}

impl Card {
    /// Create a card.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            header: None,
            children: Vec::new(),
            footer: None,
            selected: false,
            disabled: false,
            on_click: None,
            accessibility_label: None,
        }
    }

    /// Set the header.
    pub fn header(mut self, header: CardHeader) -> Self {
        self.header = Some(header);
        self
    }

    /// Set the footer, the last row of the card.
    pub fn footer(mut self, footer: CardFooter) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Make the whole card one click target.
    ///
    /// The card lifts under the pointer, is reachable with Tab and activates on
    /// Enter or Space. Buttons inside the card keep their own clicks.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Set the name a screen reader announces for a clickable card. The
    /// header's text title by default.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Card {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Selectable for Card {
    /// Show the card as picked: a gray fill and a firmer edge, no colored ring.
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Disableable for Card {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Paint the card's surface on any element.
fn surface<E: Styled>(
    element: E,
    fill: Hsla,
    edge: Hsla,
    ink: Hsla,
    radius: Pixels,
    shadow: Vec<BoxShadow>,
) -> E {
    element
        .flex()
        .flex_col()
        .items_stretch()
        .justify_start()
        .gap_3()
        .p_4()
        .min_w_0()
        .rounded(radius)
        .border_1()
        .border_color(edge)
        .bg(fill)
        .text_color(ink)
        .text_sm()
        .line_height(relative(1.5))
        .shadow(shadow)
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let radius = theme.radius_lg;
        let (fill, edge) = if self.selected {
            (
                theme.background.blend(theme.foreground.opacity(0.04)),
                theme.border.blend(theme.foreground.opacity(0.12)),
            )
        } else {
            (theme.background, theme.border)
        };
        let resting = if self.selected {
            Vec::new()
        } else {
            resting_shadow(cx)
        };
        let lifted = lifted_shadow(cx);
        let name = self
            .accessibility_label
            .or_else(|| self.header.as_ref().and_then(|header| header.name.clone()));

        let content = (!self.children.is_empty())
            .then(|| div().min_w_0().children(self.children).into_any_element());
        let parts = self
            .header
            .map(IntoElement::into_any_element)
            .into_iter()
            .chain(content)
            .chain(self.footer.map(IntoElement::into_any_element));

        let Some(on_click) = self.on_click else {
            return surface(div().id(self.id), fill, edge, ink, radius, resting)
                .children(parts)
                .refine_style(&self.style)
                .into_any_element();
        };

        let focus_handle = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);
        let can_lift = !self.selected && !self.disabled;

        surface(
            Pressable::new(self.id).track_focus(&focus_handle),
            fill,
            edge,
            ink,
            radius,
            resting,
        )
        .disabled(self.disabled)
        .selected(self.selected)
        .aria_selected(self.selected)
        .when_some(name, |this, label| this.accessibility_label(label))
        .on_click(move |event, window, cx| on_click(event, window, cx))
        .when(!self.disabled, |this| this.cursor_pointer())
        .when(self.disabled, |this| this.opacity(0.5).cursor_not_allowed())
        .when(can_lift, |this| this.hover(|style| style.shadow(lifted)))
        .when(is_focused, |this| this.focus_ring_style(window, cx))
        .children(parts)
        .refine_style(&self.style)
        .into_any_element()
    }
}

/// The top of a card: a small mark, the title with a quiet line under it, and an action.
#[derive(IntoElement, Default)]
pub struct CardHeader {
    icon: Option<AnyElement>,
    title: Option<AnyElement>,
    name: Option<SharedString>,
    description: Option<AnyElement>,
    action: Option<AnyElement>,
    children: Vec<AnyElement>,
}

impl CardHeader {
    /// Create an empty header.
    pub fn new() -> Self {
        Self::default()
    }

    /// Something small at the start: an icon, a logo or a tile holding one.
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }

    /// The title. A clickable card is announced by it.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        let title = title.into();
        self.name = Some(title.clone());
        self.title = Some(title.into_any_element());
        self
    }

    /// A title made of more than text. Give a clickable card its
    /// [`Card::accessibility_label`] then.
    pub fn title_element(mut self, title: impl IntoElement) -> Self {
        self.title = Some(title.into_any_element());
        self
    }

    /// A quiet line or two under the title.
    pub fn description(mut self, description: impl IntoElement) -> Self {
        self.description = Some(description.into_any_element());
        self
    }

    /// Something at the end of the title line: an icon button, a menu, a badge.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl ParentElement for CardHeader {
    /// Anything more under the title and description, such as a row of tags.
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = cx.theme().foreground;
        let quiet = cx.theme().muted_foreground;

        h_flex()
            .items_start()
            .gap_3()
            .min_w_0()
            .when_some(self.icon, |this, icon| {
                this.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .min_w_5()
                        .min_h_5()
                        .text_color(quiet)
                        .child(icon),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .when_some(self.title, |this, title| {
                        this.child(div().font_medium().text_color(ink).child(title))
                    })
                    .when_some(self.description, |this, description| {
                        this.child(div().text_color(quiet).child(description))
                    })
                    .children(self.children),
            )
            .when_some(self.action, |this, action| {
                // The action sits on the title's line, centered on it.
                this.child(h_flex().flex_none().h_5().gap_1().child(action))
            })
    }
}

/// The last row of a card: status or meta at the start, buttons at the end.
#[derive(IntoElement, Default)]
pub struct CardFooter {
    children: Vec<AnyElement>,
    trailing: Vec<AnyElement>,
}

impl CardFooter {
    /// Create an empty footer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add something to the end of the row, usually a button.
    pub fn end(mut self, element: impl IntoElement) -> Self {
        self.trailing.push(element.into_any_element());
        self
    }
}

impl ParentElement for CardFooter {
    /// Status or meta, shown at the start of the row in quiet text.
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardFooter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .flex_wrap()
            .gap_2()
            .min_w_0()
            .min_h_7()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .children(self.children)
            .when(!self.trailing.is_empty(), |this| {
                this.child(h_flex().ml_auto().gap_2().children(self.trailing))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_is_not_selected_until_asked() {
        assert!(!Card::new("card").is_selected());
        assert!(Card::new("card").selected(true).is_selected());
    }

    #[test]
    fn a_text_title_names_the_card() {
        assert_eq!(
            CardHeader::new().title("Invoice sync").name.as_deref(),
            Some("Invoice sync")
        );
        assert!(
            CardHeader::new()
                .title_element("Invoice sync")
                .name
                .is_none()
        );
    }

    #[test]
    fn a_card_without_a_handler_is_not_a_click_target() {
        assert!(Card::new("card").on_click.is_none());
        assert!(Card::new("card").on_click(|_, _, _| {}).on_click.is_some());
    }
}
