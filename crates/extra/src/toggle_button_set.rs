//! Toggle button set: a row of buttons that each stay pressed or not, where either any number or at most one can be pressed at a time.
//!
//! In `Many` mode each button flips on its own, like bold and italic. In
//! `One` mode pressing a button releases the others, and pressing the
//! pressed one releases it unless the set must keep one pressed. Buttons
//! can be icons with a label for screen readers, or text.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, IconName, Selectable as _, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, px,
};

/// How many buttons can be pressed at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PressMode {
    /// Any number.
    #[default]
    Many,
    /// At most one.
    One,
    /// Exactly one.
    ExactlyOne,
}

/// The pressed states after pressing button `ix`.
pub fn press(pressed: &[bool], ix: usize, mode: PressMode) -> Vec<bool> {
    let mut next = pressed.to_vec();
    if ix >= next.len() {
        return next;
    }
    match mode {
        PressMode::Many => next[ix] = !next[ix],
        PressMode::One | PressMode::ExactlyOne => {
            let was = next[ix];
            next.iter_mut().for_each(|p| *p = false);
            next[ix] = !was || mode == PressMode::ExactlyOne;
        }
    }
    next
}

/// One button in the set.
#[derive(Clone)]
pub struct ToggleItem {
    label: SharedString,
    icon: Option<IconName>,
}

impl ToggleItem {
    /// A text button.
    pub fn text(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), icon: None }
    }

    /// An icon button, with a label for screen readers and the tooltip.
    pub fn icon(icon: IconName, label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), icon: Some(icon) }
    }
}

type PressHandler = Rc<dyn Fn(Vec<bool>, &mut Window, &mut App)>;

/// A row of buttons that stay pressed.
#[derive(IntoElement)]
pub struct ToggleButtonSet {
    id: ElementId,
    style: StyleRefinement,
    items: Vec<ToggleItem>,
    pressed: Vec<bool>,
    mode: PressMode,
    label: SharedString,
    on_change: Option<PressHandler>,
}

impl ToggleButtonSet {
    /// Create a set with each button's pressed state.
    pub fn new(id: impl Into<ElementId>, items: Vec<ToggleItem>, pressed: Vec<bool>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            items,
            pressed,
            mode: PressMode::Many,
            label: "Options".into(),
            on_change: None,
        }
    }

    /// Set how many can be pressed at once.
    pub fn mode(mut self, mode: PressMode) -> Self {
        self.mode = mode;
        self
    }

    /// Name the set for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Run with every button's new state.
    pub fn on_change(mut self, handler: impl Fn(Vec<bool>, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for ToggleButtonSet {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ToggleButtonSet {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let line = cx.theme().border;
        let count = self.items.len();
        let mut pressed = self.pressed.clone();
        pressed.resize(count, false);
        let buttons = self.items.into_iter().enumerate().map(|(ix, item)| {
            let on = pressed[ix];
            let change = self.on_change.clone();
            let now = pressed.clone();
            let mode = self.mode;
            let button = Button::new(ElementId::NamedChild(self.id.clone().into(), format!("toggle-{ix}").into()))
                .small()
                .ghost()
                .selected(on)
                .accessibility_label(format!("{}, {}", item.label, if on { "on" } else { "off" }))
                .on_click(move |_, window, cx| {
                    if let Some(change) = &change {
                        change(press(&now, ix, mode), window, cx);
                    }
                });
            match item.icon {
                Some(icon) => button.icon(icon).tooltip(item.label),
                None => button.label(item.label),
            }
        });
        h_flex()
            .id(self.id.clone())
            .flex_none()
            .gap_0p5()
            .p_0p5()
            .rounded(px(8.))
            .border_1()
            .border_color(line)
            .aria_label(self.label.clone())
            .children(buttons)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_press_differently() {
        assert_eq!(press(&[true, false], 1, PressMode::Many), [true, true]);
        assert_eq!(press(&[true, false, false], 2, PressMode::One), [false, false, true]);
        assert_eq!(press(&[false, true], 1, PressMode::One), [false, false]);
        assert_eq!(press(&[false, true], 1, PressMode::ExactlyOne), [false, true]);
        assert_eq!(press(&[false], 4, PressMode::Many), [false]);
        let item = ToggleItem::icon(IconName::Search, "Find");
        assert_eq!(item.label, "Find");
    }
}
