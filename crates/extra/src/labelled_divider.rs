//! Labelled divider: a rule that splits a list or feed into named runs, such as days in an activity feed, with an optional action at the end.
//!
//! The label sits at the start or in the middle of the rule. Day headings
//! come from `day_heading`, which says "Today", "Yesterday" or "Tomorrow"
//! when it can and falls back to the date you pass. A vertical divider with
//! no label splits items in a row.

use std::rc::Rc;

use isonomic_ui::component::{ActiveTheme as _, Sizable as _, StyledExt as _, button::{Button, ButtonVariants as _}, h_flex};
use isonomic_ui::{
    App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

/// Where the label sits on the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelPlacement {
    /// At the start, with the rule running after it.
    #[default]
    Start,
    /// In the middle, with rules on both sides.
    Center,
}

/// A heading for a day `offset` days from today, or `fallback` when no word fits.
pub fn day_heading(offset: i64, fallback: &str) -> String {
    match offset {
        0 => "Today".into(),
        -1 => "Yesterday".into(),
        1 => "Tomorrow".into(),
        _ => fallback.to_string(),
    }
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A rule with an optional label and action.
#[derive(IntoElement)]
pub struct LabelledDivider {
    id: ElementId,
    style: StyleRefinement,
    label: Option<SharedString>,
    placement: LabelPlacement,
    vertical: bool,
    action: Option<(SharedString, ClickHandler)>,
}

impl LabelledDivider {
    /// Create a horizontal rule with no label.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            label: None,
            placement: LabelPlacement::Start,
            vertical: false,
            action: None,
        }
    }

    /// Name the run that follows.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Put the label at the start or in the middle.
    pub fn placement(mut self, placement: LabelPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Draw an upright rule for splitting items in a row. Labels and actions don't show.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// End the rule with a small text action, such as "Mark all read".
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }
}

impl Styled for LabelledDivider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LabelledDivider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;

        if self.vertical {
            return div()
                .id(self.id.clone())
                .w(px(1.))
                .h_full()
                .min_h(px(16.))
                .flex_none()
                .bg(line)
                .refine_style(&self.style)
                .into_any_element();
        }

        let rule = || div().flex_1().h(px(1.)).bg(line);
        let centered = self.placement == LabelPlacement::Center;
        let action = self.action.map(|(label, handler)| {
            Button::new(ElementId::NamedChild(self.id.clone().into(), "action".into()))
                .xsmall()
                .ghost()
                .label(label)
                .on_click(move |event, window, cx| handler(event, window, cx))
        });

        h_flex()
            .id(self.id.clone())
            .w_full()
            .gap_2()
            .when_some(self.label.clone(), |this, label| this.aria_label(label))
            .when(centered && self.label.is_some(), |this| this.child(rule()))
            .when_some(self.label, |this, label| {
                this.child(div().flex_none().text_xs().font_medium().text_color(quiet).child(label))
            })
            .child(rule())
            .children(action)
            .refine_style(&self.style)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_days_get_words() {
        assert_eq!(day_heading(0, "Sat 15 Nov"), "Today");
        assert_eq!(day_heading(-1, "Fri 14 Nov"), "Yesterday");
        assert_eq!(day_heading(1, "Sun 16 Nov"), "Tomorrow");
        assert_eq!(day_heading(-3, "Wed 12 Nov"), "Wed 12 Nov");
    }
}
