//! Tips grid: a panel of short tips, each with one action.
//!
//! Taking a tip's action turns it into a done state and moves the counter in
//! the heading. Only the tips scroll; the heading and the close row stay put.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

use crate::common::resting_shadow;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One tip: a title, a short explanation and one thing to do.
pub struct Tip {
    id: SharedString,
    title: SharedString,
    description: Option<AnyElement>,
    action: SharedString,
    done_label: SharedString,
    primary: bool,
    on_action: Option<Handler>,
}

impl Tip {
    /// Create a tip whose button reads "Try it".
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            action: "Try it".into(),
            done_label: "Done".into(),
            primary: false,
            on_action: None,
        }
    }

    /// A short explanation. It can hold more than text, such as a key cap.
    pub fn description(mut self, description: impl IntoElement) -> Self {
        self.description = Some(description.into_any_element());
        self
    }

    /// The button's text.
    pub fn action(mut self, action: impl Into<SharedString>) -> Self {
        self.action = action.into();
        self
    }

    /// The text shown once the action was taken, such as "Enabled".
    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        self.done_label = label.into();
        self
    }

    /// Show the action as the filled button. Use it for one tip at most.
    pub fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }

    /// Run when the action is taken.
    pub fn on_action(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

/// A card holding a grid of tips.
#[derive(IntoElement)]
pub struct TipsGrid {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    tips: Vec<Tip>,
    columns: u16,
    max_height: Pixels,
    close_label: SharedString,
    on_close: Option<Handler>,
}

impl TipsGrid {
    /// Create an empty panel titled "Tips" with three columns.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Tips".into(),
            tips: Vec::new(),
            columns: 3,
            max_height: px(420.),
            close_label: "Close".into(),
            on_close: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Add one tip.
    pub fn tip(mut self, tip: Tip) -> Self {
        self.tips.push(tip);
        self
    }

    /// Add several tips.
    pub fn tips(mut self, tips: impl IntoIterator<Item = Tip>) -> Self {
        self.tips.extend(tips);
        self
    }

    /// How many tips sit side by side. Use fewer in a narrow panel.
    pub fn columns(mut self, columns: u16) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// How tall the tips grow before they scroll.
    pub fn max_height(mut self, max_height: impl Into<Pixels>) -> Self {
        self.max_height = max_height.into();
        self
    }

    /// Show a close button in the last row and run this when it is pressed.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// The close button's text.
    pub fn close_label(mut self, label: impl Into<SharedString>) -> Self {
        self.close_label = label.into();
        self
    }
}

impl Styled for TipsGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct GridState {
    done: Vec<SharedString>,
}

/// A key for each tip that stays unique when two tips share an id, so marking
/// one never marks the other.
fn keys(ids: &[SharedString]) -> Vec<SharedString> {
    let mut out: Vec<SharedString> = Vec::with_capacity(ids.len());
    for (ix, id) in ids.iter().enumerate() {
        let earlier = ids[..ix].iter().filter(|other| *other == id).count();
        out.push(match earlier {
            0 => id.clone(),
            n => format!("{id}#{n}").into(),
        });
    }
    out
}

impl RenderOnce for TipsGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| GridState::default());
        let ids: Vec<SharedString> = self.tips.iter().map(|tip| tip.id.clone()).collect();
        let keys = keys(&ids);
        let done = state.read(cx).done.clone();
        // Only tips still in the list count.
        let done_count = keys.iter().filter(|key| done.contains(key)).count();
        let total = self.tips.len();
        let grid_id = self.id.clone();

        let cells = self.tips.into_iter().zip(keys).map(|(tip, key)| {
            let is_done = done.contains(&key);
            let action = if is_done {
                h_flex()
                    .h(px(28.))
                    .gap_1p5()
                    .font_medium()
                    .text_color(success)
                    .child(Icon::new(IconName::CircleCheck).small())
                    .child(tip.done_label)
                    .into_any_element()
            } else {
                let state = state.clone();
                let on_action = tip.on_action;
                let button_id = ElementId::NamedChild(grid_id.clone().into(), key.clone());
                Button::new(button_id)
                    .small()
                    .map(|this| {
                        if tip.primary {
                            this.primary()
                        } else {
                            this.outline()
                        }
                    })
                    .label(tip.action)
                    .on_click(move |_, window, cx| {
                        state.update(cx, |state, cx| {
                            state.done.push(key.clone());
                            cx.notify();
                        });
                        if let Some(on_action) = &on_action {
                            on_action(window, cx);
                        }
                    })
                    .into_any_element()
            };

            v_flex()
                .gap_1()
                .p_3()
                .min_w_0()
                .rounded(radius)
                .border_1()
                .border_color(line)
                .child(div().font_medium().child(tip.title))
                .child(div().flex_1().text_color(quiet).children(tip.description))
                .child(h_flex().mt_2().child(action))
        });
        let cells: Vec<_> = cells.collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .overflow_hidden()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .gap_3()
                    .h(px(48.))
                    .px_4()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_base()
                            .font_medium()
                            .child(self.title),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(quiet)
                            .child(format!("{done_count}/{total} done")),
                    ),
            )
            .child(
                div()
                    .id("tips")
                    .p_3()
                    .max_h(self.max_height)
                    .overflow_y_scroll()
                    .child(div().grid().grid_cols(self.columns).gap_2().children(cells)),
            )
            .when_some(self.on_close, |this, on_close| {
                this.child(
                    h_flex()
                        .flex_none()
                        .justify_end()
                        .p_2()
                        .border_t_1()
                        .border_color(line)
                        .child(
                            Button::new((self.id.clone(), "close"))
                                .outline()
                                .small()
                                .label(self.close_label)
                                .on_click(move |_, window, cx| on_close(window, cx)),
                        ),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_ids_are_their_own_keys() {
        let ids: Vec<SharedString> = vec!["search".into(), "digest".into()];
        assert_eq!(keys(&ids), ["search", "digest"]);
    }

    #[test]
    fn repeated_ids_get_separate_keys() {
        let ids: Vec<SharedString> = vec!["a".into(), "b".into(), "a".into(), "a".into()];
        assert_eq!(keys(&ids), ["a", "b", "a#1", "a#2"]);
    }

    #[test]
    fn a_panel_always_has_a_column() {
        assert_eq!(TipsGrid::new("tips").columns(0).columns, 1);
    }
}
