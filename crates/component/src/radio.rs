// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{rc::Rc, time::Duration};

use crate::{
    ActiveTheme, AxisExt, Sizable, Size, StyledExt, h_flex, text::Text, tooltip::ComponentTooltip,
    v_flex,
};
use crate::{StyleSized as _, ThemeStyled as _};
use gpui::{
    AnyElement, App, Axis, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px, relative, rems,
};
use isonomic_ui_base::{LogicalStyledExt as _, Radio as BaseRadio, RadioGroup as BaseRadioGroup};

/// A Radio element.
///
/// This is not included the Radio group implementation, you can manage the group by yourself.
#[derive(IntoElement)]
pub struct Radio {
    base: BaseRadio,
    style: StyleRefinement,
    id: ElementId,
    label: Option<Text>,
    /// The announced name, when the visible label is not it.
    accessibility_label: Option<SharedString>,
    children: Vec<AnyElement>,
    checked: bool,
    disabled: bool,
    tab_stop: bool,
    tab_index: isize,
    size: Size,
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    tooltip: ComponentTooltip,
    position_in_set: Option<usize>,
    size_of_set: Option<usize>,
    focus_ring_enabled: bool,
    /// Supplied by a group, so it can move focus between its radios.
    focus_handle: Option<gpui::FocusHandle>,
}

impl Radio {
    /// Create a new Radio element with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: BaseRadio::new(id.clone()),
            id,
            style: StyleRefinement::default(),
            label: None,
            accessibility_label: None,
            children: Vec::new(),
            checked: false,
            disabled: false,
            tab_index: 0,
            tab_stop: true,
            size: Size::default(),
            on_click: None,
            tooltip: ComponentTooltip::default(),
            position_in_set: None,
            size_of_set: None,
            focus_ring_enabled: true,
            focus_handle: None,
        }
    }

    /// Set tooltip text for the radio.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip.text = Some((tooltip.into(), None));
        self
    }

    /// Overrides how long the pointer must rest before the tooltip shows.
    ///
    /// Defaults to [`isonomic_ui_base::TooltipDefaults::show_delay`].
    pub fn tooltip_show_delay(mut self, delay: Duration) -> Self {
        self.tooltip.show_delay = Some(delay);
        self
    }

    /// Set the label of the Radio element.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the name a screen reader announces, when the visible label is not
    /// it.
    ///
    /// A radio's name comes from its [`label`](Self::label) by default. Setting
    /// this replaces the announced name without changing what is displayed.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Set the checked state of the Radio element, default is `false`.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set the disabled state of the Radio element, default is `false`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the tab index for the Radio element, default is `0`.
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// Set the tab stop for the Radio element, default is `true`.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Alias for [`Self::on_change`]. The last callback registered with either name wins.
    pub fn on_click(self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change(handler)
    }

    /// Handle a requested checked value from pointer or keyboard activation.
    ///
    /// This is a controlled value: the owner must write the requested value and
    /// call `cx.notify()` to render it. Disabled controls do not call the handler.
    /// This and [`Self::on_click`] share one callback; chaining them replaces
    /// the previous handler instead of calling both.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Radio {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl crate::FocusableExt for Radio {
    fn focus_ring(mut self, enabled: bool) -> Self {
        self.focus_ring_enabled = enabled;
        self
    }

    fn is_focus_ring_enabled(&self) -> bool {
        self.focus_ring_enabled
    }
}

impl Styled for Radio {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for Radio {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Radio {}

impl ParentElement for Radio {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let has_content = self.label.is_some() || !self.children.is_empty();
        let indicator_size = rems(match self.size {
            Size::XSmall => 0.75,
            Size::Small => 0.875,
            Size::Large => 1.125,
            _ => 1.,
        });
        let focus_handle = self.focus_handle.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        });
        let is_focused = focus_handle.is_focused(window);
        let disabled = self.disabled;
        let accessibility_label = self
            .accessibility_label
            .clone()
            .or_else(|| self.label.as_ref().map(|label| label.get_text(cx)));

        // The same recessed well as a checkbox, round: a 1px inner edge that
        // firms under the pointer and a hairline drop. Chosen, it fills with
        // ink and a dot grows out of the middle.
        let dark = cx.theme().mode.is_dark();
        let edge = |alpha: f32| gpui::BoxShadow {
            color: if dark {
                gpui::hsla(0., 0., 1., alpha + 0.04)
            } else {
                gpui::hsla(0., 0., 0., alpha)
            },
            offset: gpui::point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(1.),
            inset: true,
        };
        let drop = gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., if dark { 0.3 } else { 0.04 }),
            offset: gpui::point(px(0.), px(1.)),
            blur_radius: px(0.5),
            spread_radius: px(0.),
            inset: false,
        };
        let hover_state = window.use_keyed_state(
            ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "hovered".into()),
            cx,
            |_, _| false,
        );
        let edge_alpha = 0.18
            + 0.12
                * crate::look::hover_fade(
                    self.id.clone(),
                    !disabled && *hover_state.read(cx),
                    window,
                    cx,
                );
        let grown = isonomic_ui_base::motion::transition(
            ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "dot".into()),
            if checked { 1.0_f32 } else { 0.0 },
            isonomic_ui_base::motion::Transition::new(cx.theme().motion.duration_normal)
                .easing(cx.theme().motion.easing_enter.clone()),
            window,
            cx,
        );
        let indicator_px = indicator_size.to_pixels(window.rem_size());
        let dot = indicator_px * 0.375 * grown;
        let fill = if disabled {
            cx.theme().primary.opacity(0.5)
        } else {
            *cx.theme().tokens.primary
        };

        self.base
            .id(self.id.clone())
            .checked(self.checked)
            .disabled(self.disabled)
            .track_focus(&focus_handle)
            .tab_stop(self.tab_stop)
            .tab_index(self.tab_index)
            .when_some(accessibility_label, |this, label| {
                this.accessibility_label(label)
            })
            .when_some(
                self.position_in_set.zip(self.size_of_set),
                |this, (position, size)| this.set_position(position, size),
            )
            .h_flex()
            // The indicator leads, on the start edge.
            .flex_row_dir(cx)
            .gap_x_2p5()
            .text_color(cx.theme().foreground)
            .items_start()
            .line_height(relative(1.))
            .rounded(cx.theme().radius * 0.5)
            .when(is_focused && self.focus_ring_enabled, |this| {
                this.focus_ring_style(window, cx)
            })
            .map(|this| match self.size {
                Size::Size(_) => this,
                size => this.input_text_size(size),
            })
            .refine_style(&self.style)
            .child(
                div()
                    .relative()
                    .size(indicator_size)
                    // Center on the label's first 20px line, including when it wraps.
                    .when(has_content, |this| this.mt((px(20.) - indicator_px) / 2.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .map(|this| {
                        if checked {
                            this.bg(fill)
                        } else {
                            this.bg(cx.theme().popover).shadow(vec![
                                edge(if disabled { 0.1 } else { edge_alpha }),
                                drop.clone(),
                            ])
                        }
                    })
                    .when(dot > px(0.), |this| {
                        this.child(
                            div()
                                .size(dot)
                                .rounded_full()
                                .bg(cx.theme().primary_foreground),
                        )
                    }),
            )
            .when(!self.children.is_empty() || self.label.is_some(), |this| {
                this.child(
                    v_flex()
                        .w_full()
                        .line_height(px(20.))
                        .gap_0p5()
                        .when_some(self.label, |this, label| {
                            this.child(
                                div()
                                    .size_full()
                                    .when(self.disabled, |this| {
                                        this.text_color(cx.theme().muted_foreground)
                                    })
                                    .child(label),
                            )
                        })
                        .children(self.children),
                )
            })
            .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                window.prevent_default()
            })
            .when_some(self.on_click.clone(), |this, on_click| {
                this.on_change(move |next, _, window, cx| {
                    window.prevent_default();
                    on_click(&next, window, cx);
                })
            })
            .map(|this| {
                let hover: crate::tooltip::HoverListener = Rc::new(move |hovered, window, cx| {
                    let hovered = *hovered;
                    hover_state.update(cx, |state, cx| {
                        if *state != hovered {
                            *state = hovered;
                            cx.notify();
                        }
                    });
                    window.refresh();
                });
                self.tooltip.apply_with_hover(this, Some(hover))
            })
    }
}

/// A Radio group element.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    style: StyleRefinement,
    radios: Vec<Radio>,
    layout: Axis,
    selected_index: Option<usize>,
    disabled: bool,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl RadioGroup {
    /// Creates a radio group with vertical layout and no selected item.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default().flex_1(),
            on_click: None,
            layout: Axis::Vertical,
            selected_index: None,
            disabled: false,
            radios: vec![],
        }
    }

    /// Create a new Radio group with default Vertical layout.
    pub fn vertical(id: impl Into<ElementId>) -> Self {
        Self::new(id)
    }

    /// Create a new Radio group with Horizontal layout.
    pub fn horizontal(id: impl Into<ElementId>) -> Self {
        Self::new(id).layout(Axis::Horizontal)
    }

    /// Set the layout of the Radio group. Default is `Axis::Vertical`.
    pub fn layout(mut self, layout: Axis) -> Self {
        self.layout = layout;
        self
    }

    /// Alias for [`Self::on_change`]. The last callback registered with either name wins.
    pub fn on_click(self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change(handler)
    }

    /// Handle a requested selected index from pointer or keyboard activation.
    ///
    /// This is a controlled value: the owner must write the requested value and
    /// call `cx.notify()` to render it. Disabled controls do not call the handler.
    /// This and [`Self::on_click`] share one callback; chaining them replaces
    /// the previous handler instead of calling both.
    pub fn on_change(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Set the selected index.
    pub fn selected_index(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }

    /// Set the disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Add a child Radio element.
    pub fn child(mut self, child: impl Into<Radio>) -> Self {
        self.radios.push(child.into());
        self
    }

    /// Add multiple child Radio elements.
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Radio>>) -> Self {
        self.radios.extend(children.into_iter().map(Into::into));
        self
    }
}

impl Styled for RadioGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl From<&'static str> for Radio {
    fn from(label: &'static str) -> Self {
        Self::new(label).label(label)
    }
}

impl From<SharedString> for Radio {
    fn from(label: SharedString) -> Self {
        Self::new(label.clone()).label(label)
    }
}

impl From<String> for Radio {
    fn from(label: String) -> Self {
        Self::new(SharedString::from(label.clone())).label(SharedString::from(label))
    }
}

/// A key that moves the choice in a radio group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RadioStep {
    Previous,
    Next,
    First,
    Last,
}

impl RadioStep {
    /// The step a key asks for. Left and right swap in a right-to-left
    /// layout, so "next" always follows the reading order.
    pub(crate) fn from_key(key: &str, rtl: bool) -> Option<Self> {
        match (key, rtl) {
            ("up", _) | ("left", false) | ("right", true) => Some(Self::Previous),
            ("down", _) | ("right", false) | ("left", true) => Some(Self::Next),
            ("home", _) => Some(Self::First),
            ("end", _) => Some(Self::Last),
            _ => None,
        }
    }
}

/// The radio a step lands on, skipping disabled ones and wrapping at the
/// ends. `None` when nothing in the group can be chosen.
pub(crate) fn radio_step(from: Option<usize>, step: RadioStep, enabled: &[bool]) -> Option<usize> {
    let total = enabled.len();
    if !enabled.iter().any(|e| *e) {
        return None;
    }
    let first = enabled.iter().position(|e| *e);
    let last = enabled.iter().rposition(|e| *e);
    match (step, from) {
        (RadioStep::First, _) => first,
        (RadioStep::Last, _) => last,
        (RadioStep::Next, None) => first,
        (RadioStep::Previous, None) => last,
        (RadioStep::Next, Some(from)) => (1..=total)
            .map(|offset| (from + offset) % total)
            .find(|ix| enabled[*ix]),
        (RadioStep::Previous, Some(from)) => (1..=total)
            .map(|offset| (from + total * 2 - offset) % total)
            .find(|ix| enabled[*ix]),
    }
}

/// The one radio Tab stops on: the chosen one, or the first that can be
/// chosen when none is (or the chosen one is disabled).
pub(crate) fn radio_tab_stop(selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
    selected
        .filter(|ix| enabled.get(*ix).copied().unwrap_or(false))
        .or_else(|| enabled.iter().position(|e| *e))
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_click = self.on_click;
        let disabled = self.disabled;
        let selected_ix = self.selected_index;

        let base = if self.layout.is_vertical() {
            v_flex()
        } else {
            h_flex().flex_row_dir(cx).w_full().flex_wrap()
        };

        let total = self.radios.len();
        let enabled: Rc<[bool]> = self
            .radios
            .iter()
            .map(|radio| !(disabled || radio.disabled))
            .collect();
        // One handle per radio, kept by the group, so the arrow keys can move
        // focus along with the choice.
        let handles = window.use_keyed_state(
            ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "focus".into()),
            cx,
            |_, _| Vec::<gpui::FocusHandle>::new(),
        );
        handles.update(cx, |handles, cx| {
            while handles.len() < total {
                handles.push(cx.focus_handle());
            }
            handles.truncate(total);
        });
        let focus_handles: Vec<gpui::FocusHandle> = handles.read(cx).clone();
        let tab_stop = radio_tab_stop(selected_ix, &enabled);
        let rtl = isonomic_ui_base::LayoutDirection::global(cx).is_rtl();
        BaseRadioGroup::new(self.id)
            .axis(self.layout)
            .refine_style(&self.style)
            .when_some(on_click.clone(), |this, on_click| {
                let focus_handles = focus_handles.clone();
                let enabled = enabled.clone();
                this.on_key_down(move |event, window, cx| {
                    let Some(step) = RadioStep::from_key(event.keystroke.key.as_str(), rtl) else {
                        return;
                    };
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let focused = focus_handles.iter().position(|h| h.is_focused(window));
                    let Some(next) = radio_step(focused.or(selected_ix), step, &enabled) else {
                        return;
                    };
                    cx.stop_propagation();
                    focus_handles[next].focus(window, cx);
                    on_click(&next, window, cx);
                })
            })
            .child(
                base.gap_3()
                    .children(self.radios.into_iter().enumerate().map(|(ix, mut radio)| {
                        let checked = selected_ix == Some(ix);

                        radio.id = ix.into();
                        radio.position_in_set = Some(ix + 1);
                        radio.size_of_set = Some(total);
                        radio.focus_handle = focus_handles.get(ix).cloned();
                        let item_disabled = radio.disabled;
                        radio
                            .disabled(disabled || item_disabled)
                            .checked(checked)
                            // Tab enters the group once, on its chosen radio;
                            // the arrows move within it.
                            .tab_stop(tab_stop == Some(ix))
                            .when_some(on_click.clone(), |this, on_click| {
                                this.on_click(move |_, window, cx| on_click(&ix, window, cx))
                            })
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_keys_follow_reading_order() {
        assert_eq!(RadioStep::from_key("down", false), Some(RadioStep::Next));
        assert_eq!(RadioStep::from_key("right", false), Some(RadioStep::Next));
        assert_eq!(
            RadioStep::from_key("right", true),
            Some(RadioStep::Previous)
        );
        assert_eq!(RadioStep::from_key("up", true), Some(RadioStep::Previous));
        assert_eq!(RadioStep::from_key("home", false), Some(RadioStep::First));
        assert_eq!(RadioStep::from_key("end", false), Some(RadioStep::Last));
        assert_eq!(RadioStep::from_key("a", false), None);
    }

    #[test]
    fn steps_wrap_and_skip_disabled_radios() {
        let enabled = [true, false, true, true];
        assert_eq!(radio_step(Some(0), RadioStep::Next, &enabled), Some(2));
        assert_eq!(radio_step(Some(3), RadioStep::Next, &enabled), Some(0));
        assert_eq!(radio_step(Some(0), RadioStep::Previous, &enabled), Some(3));
        assert_eq!(radio_step(Some(2), RadioStep::Previous, &enabled), Some(0));
        assert_eq!(radio_step(None, RadioStep::Next, &enabled), Some(0));
        assert_eq!(radio_step(None, RadioStep::Previous, &enabled), Some(3));
        assert_eq!(radio_step(Some(2), RadioStep::First, &enabled), Some(0));
        assert_eq!(radio_step(Some(0), RadioStep::Last, &enabled), Some(3));
        assert_eq!(radio_step(Some(0), RadioStep::Next, &[false, false]), None);
        // A lone radio steps onto itself.
        assert_eq!(radio_step(Some(0), RadioStep::Next, &[true]), Some(0));
    }

    #[test]
    fn tab_lands_on_the_chosen_radio_or_the_first_enabled_one() {
        assert_eq!(radio_tab_stop(Some(2), &[true, true, true]), Some(2));
        assert_eq!(radio_tab_stop(None, &[false, true, true]), Some(1));
        assert_eq!(radio_tab_stop(Some(0), &[false, true]), Some(1));
        assert_eq!(radio_tab_stop(None, &[false, false]), None);
    }

    #[test]
    fn an_explicit_accessibility_label_replaces_the_visible_one() {
        let plain = Radio::new("automatic").label("Automatic");
        assert_eq!(plain.accessibility_label, None);
        assert!(matches!(
            &plain.label,
            Some(Text::String(label)) if label.as_ref() == "Automatic"
        ));

        let named = Radio::new("automatic")
            .label("Automatic")
            .accessibility_label("Choose automatic mode");
        assert_eq!(
            named.accessibility_label.as_deref(),
            Some("Choose automatic mode"),
            "an explicit name must win over the visible label"
        );
        assert!(
            matches!(
                &named.label,
                Some(Text::String(label)) if label.as_ref() == "Automatic"
            ),
            "and must not change what is drawn"
        );
    }
}
