//! Notice stack: announcements stacked behind each other, read one at a time.
//!
//! Only the front notice can be read and used. The next ones peek out below
//! it, and dismissing the front notice brings the next one forward.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How far each notice behind peeks out below the one in front of it.
const PEEK: Pixels = px(12.);
/// How many notices behind the front one are hinted at.
const MAX_BEHIND: usize = 2;
const ENTER: Duration = Duration::from_millis(260);

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One announcement.
#[derive(Clone)]
pub struct Notice {
    id: SharedString,
    badge: Option<SharedString>,
    title: SharedString,
    text: Option<SharedString>,
    action: Option<(SharedString, bool, ClickHandler)>,
}

impl Notice {
    /// Create a notice. The id tells notices apart and is passed to
    /// [`NoticeStack::on_dismiss`].
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            badge: None,
            title: title.into(),
            text: None,
            action: None,
        }
    }

    /// A small label above the title, such as "New".
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// A sentence or two under the title.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// The one thing to do about this notice.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), false, Rc::new(handler)));
        self
    }

    /// Like [`Self::action`], marked as leaving the application.
    pub fn external_action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), true, Rc::new(handler)));
        self
    }
}

type DismissHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A stack of notices that shows the first one and hints at the rest.
#[derive(IntoElement)]
pub struct NoticeStack {
    id: ElementId,
    style: StyleRefinement,
    notices: Vec<Notice>,
    on_dismiss: Option<DismissHandler>,
}

impl NoticeStack {
    /// Create an empty stack.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            notices: Vec::new(),
            on_dismiss: None,
        }
    }

    /// Add a notice behind the ones already added.
    pub fn notice(mut self, notice: Notice) -> Self {
        self.notices.push(notice);
        self
    }

    /// Add several notices, first in front.
    pub fn notices(mut self, notices: impl IntoIterator<Item = Notice>) -> Self {
        self.notices.extend(notices);
        self
    }

    /// Run after a notice was dismissed, with its id.
    ///
    /// The stack remembers what was dismissed, so a notice stays gone even if
    /// it is passed in again. Notices with new ids join the stack.
    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl Styled for NoticeStack {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct StackState {
    dismissed: Vec<SharedString>,
}

/// The notices still to show, in order.
fn remaining(notices: Vec<Notice>, dismissed: &[SharedString]) -> Vec<Notice> {
    notices
        .into_iter()
        .filter(|notice| !dismissed.contains(&notice.id))
        .collect()
}

impl RenderOnce for NoticeStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let pill = theme.muted;
        let radius = theme.radius_lg;
        let shadow = resting_shadow(cx);
        let animate = !cx.reduce_motion();

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| StackState::default());
        let mut visible = remaining(self.notices, &state.read(cx).dismissed).into_iter();
        let root = div().id(self.id.clone()).w_full().max_w(px(420.));
        let Some(front) = visible.next() else {
            return root.refine_style(&self.style);
        };
        let behind = visible.len().min(MAX_BEHIND);

        // The notices behind are drawn as empty edges below the front one,
        // each a little lower and narrower, deepest first so nearer ones cover it.
        let edges = (1..=behind).rev().map(|depth| {
            let inset = relative(0.025 * depth as f32);
            div()
                .absolute()
                .left(inset)
                .right(inset)
                .bottom(PEEK * (behind - depth) as f32)
                .h(px(40.))
                .rounded(radius)
                .border_1()
                .border_color(line)
                .bg(fill)
                .shadow(shadow.clone())
        });

        let front_id = front.id.clone();
        let on_dismiss = self.on_dismiss;
        let dismiss = {
            let id = front.id.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.dismissed.push(id.clone());
                    cx.notify();
                });
                if let Some(on_dismiss) = &on_dismiss {
                    on_dismiss(&id, window, cx);
                }
            }
        };

        let card = v_flex()
            .relative()
            .w_full()
            .gap_1()
            .p_4()
            .rounded(radius)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.5))
            .shadow(shadow.clone())
            .child(
                h_flex()
                    .justify_between()
                    .mb_1()
                    .child(div().when_some(front.badge, |this, badge| {
                        this.px_1p5()
                            .rounded(px(5.))
                            .bg(pill)
                            .text_color(quiet)
                            .text_xs()
                            .font_medium()
                            .child(badge.to_uppercase())
                    }))
                    .child(
                        Button::new((self.id.clone(), "dismiss"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .accessibility_label(format!("Dismiss {}", front.title))
                            .on_click(dismiss),
                    ),
            )
            .child(div().text_base().font_medium().child(front.title))
            .when_some(front.text, |this, text| {
                this.child(div().text_color(quiet).child(text))
            })
            .when_some(front.action, |this, (label, external, handler)| {
                this.child(
                    h_flex().mt_2().child(
                        Button::new((self.id.clone(), "action"))
                            .outline()
                            .small()
                            .accessibility_label(if external {
                                format!("{label}, opens outside the app")
                            } else {
                                label.to_string()
                            })
                            .child(h_flex().gap_1().child(label).when(external, |this| {
                                this.child(Icon::new(IconName::ExternalLink).xsmall())
                            }))
                            .on_click(move |event, window, cx| handler(event, window, cx)),
                    ),
                )
            });
        // A notice that has just come to the front rises into place.
        let card = if animate {
            card.with_animation(
                ElementId::Name(front_id),
                Animation::new(ENTER),
                |this, time| {
                    let eased = 1. - (1. - time).powi(3);
                    this.opacity(0.4 + 0.6 * eased).top(px(8. * (1. - eased)))
                },
            )
            .into_any_element()
        } else {
            card.into_any_element()
        };

        root.relative()
            .role(Role::Region)
            .aria_label("Notifications")
            .pb(PEEK * behind as f32)
            .children(edges)
            .child(card)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(notices: &[Notice]) -> Vec<&str> {
        notices.iter().map(|notice| notice.id.as_ref()).collect()
    }

    #[test]
    fn dismissed_notices_stay_gone() {
        let notices = vec![
            Notice::new("a", "First"),
            Notice::new("b", "Second"),
            Notice::new("c", "Third"),
        ];
        let left = remaining(notices, &["a".into(), "c".into()]);
        assert_eq!(ids(&left), ["b"]);
    }

    #[test]
    fn order_is_kept() {
        let notices = vec![Notice::new("a", "First"), Notice::new("b", "Second")];
        assert_eq!(ids(&remaining(notices, &[])), ["a", "b"]);
    }
}
