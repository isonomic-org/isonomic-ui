// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Profile menu: the card behind an avatar, with the signed-in user, account
//! actions and an optional nudge to finish setting up.

use std::rc::Rc;

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    avatar::Avatar, h_flex, progress::Progress, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One action in the menu.
#[derive(Clone)]
pub struct ProfileMenuItem {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    badge: Option<SharedString>,
    on_select: Option<Handler>,
}

impl ProfileMenuItem {
    /// Create an action.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            on_select: None,
        }
    }

    /// The icon in front of the label.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A small pill at the end of the row, such as "New".
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Run when the action is chosen.
    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

/// How far the user is with setting up, shown as the last section of the menu.
#[derive(Clone)]
pub struct ProfileSetup {
    title: SharedString,
    done: u32,
    total: u32,
    next: Option<SharedString>,
    on_continue: Option<Handler>,
}

impl ProfileSetup {
    /// Create the section from the steps finished and the steps in all.
    pub fn new(done: u32, total: u32) -> Self {
        Self {
            title: "Setup".into(),
            done: done.min(total),
            total,
            next: None,
            on_continue: None,
        }
    }

    /// Name what is being set up. Defaults to "Setup".
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The next open step, offered as a shortcut under the bar.
    pub fn next(mut self, next: impl Into<SharedString>) -> Self {
        self.next = Some(next.into());
        self
    }

    /// Run when the shortcut to the next step is chosen.
    pub fn on_continue(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_continue = Some(Rc::new(handler));
        self
    }

    /// Progress from 0 to 100. Nothing to do counts as nothing done.
    fn percent(&self) -> f32 {
        if self.total == 0 {
            0.
        } else {
            self.done as f32 * 100. / self.total as f32
        }
    }

    fn is_complete(&self) -> bool {
        self.done >= self.total
    }
}

/// An account card: who is signed in, what they can do, and setup progress.
#[derive(IntoElement)]
pub struct ProfileMenu {
    id: ElementId,
    style: StyleRefinement,
    name: SharedString,
    email: Option<SharedString>,
    avatar: Option<AnyElement>,
    items: Vec<ProfileMenuItem>,
    setup: Option<ProfileSetup>,
}

impl ProfileMenu {
    /// Create a menu for the named user.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            name: name.into(),
            email: None,
            avatar: None,
            items: Vec::new(),
            setup: None,
        }
    }

    /// The email under the name. Long addresses end in an ellipsis.
    pub fn email(mut self, email: impl Into<SharedString>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Replace the default avatar, which shows the user's initials.
    pub fn avatar(mut self, avatar: impl IntoElement) -> Self {
        self.avatar = Some(avatar.into_any_element());
        self
    }

    /// Add an action.
    pub fn item(mut self, item: ProfileMenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Show setup progress under the actions.
    pub fn setup(mut self, setup: ProfileSetup) -> Self {
        self.setup = Some(setup);
        self
    }
}

impl Styled for ProfileMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Where an arrow, Home or End key moves focus among `len` actions.
fn next_index(key: &str, current: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match key {
        "down" => Some(current.map_or(0, |ix| (ix + 1) % len)),
        "up" => Some(current.map_or(len - 1, |ix| (ix + len - 1) % len)),
        "home" => Some(0),
        "end" => Some(len - 1),
        _ => None,
    }
}

impl RenderOnce for ProfileMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let wash = theme.foreground.opacity(0.05);
        let alert = theme.danger;
        let alert_tint = theme.danger.opacity(0.1);
        let radius = theme.radius;

        let child_id = |name: SharedString| ElementId::NamedChild(self.id.clone().into(), name);
        let focus = |id: &ElementId, window: &mut Window, cx: &mut App| -> FocusHandle {
            window
                .use_keyed_state((id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        };

        let item_ids: Vec<ElementId> = self
            .items
            .iter()
            .map(|item| child_id(item.id.clone()))
            .collect();
        let handles: Vec<FocusHandle> = item_ids.iter().map(|id| focus(id, window, cx)).collect();

        let rows = self
            .items
            .into_iter()
            .zip(item_ids)
            .zip(handles.iter())
            .map(|((item, id), handle)| {
                let is_focused = handle.is_focused(window);
                let on_select = item.on_select;
                let name = match &item.badge {
                    Some(badge) => format!("{}, {badge}", item.label),
                    None => item.label.to_string(),
                };
                Pressable::new(id)
                    .role(Role::MenuItem)
                    .accessibility_label(name)
                    .track_focus(handle)
                    .on_click(move |_, window, cx| {
                        if let Some(on_select) = &on_select {
                            on_select(window, cx);
                        }
                    })
                    .justify_start()
                    .gap_2p5()
                    .w_full()
                    .h(px(34.))
                    .px_2()
                    .rounded(radius)
                    .line_height(relative(1.4))
                    .cursor_pointer()
                    .hover(|style| style.bg(wash))
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    .child(
                        div()
                            .flex_none()
                            .w_4()
                            .text_color(quiet)
                            .children(item.icon.map(|icon| icon.small())),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(item.label))
                    .when_some(item.badge, |this, badge| {
                        this.child(
                            div()
                                .flex_none()
                                .px_1p5()
                                .rounded(px(5.))
                                .bg(alert_tint)
                                .text_color(alert)
                                .text_xs()
                                .font_medium()
                                .child(badge),
                        )
                    })
            });
        let rows: Vec<_> = rows.collect();

        let setup = self.setup.map(|setup| {
            let percent = setup.percent();
            let next = setup.next.clone().filter(|_| !setup.is_complete());
            let next_id = child_id("next-step".into());
            let next_focus = focus(&next_id, window, cx);
            let next_focused = next_focus.is_focused(window);
            let on_continue = setup.on_continue;
            v_flex()
                .gap_2()
                .p_3()
                .border_t_1()
                .border_color(line)
                .child(
                    h_flex()
                        .justify_between()
                        .text_color(quiet)
                        .child(div().font_medium().child(setup.title.clone()))
                        .child(format!("{}/{}", setup.done, setup.total)),
                )
                .child(
                    Progress::new(child_id("progress".into()))
                        .value(percent)
                        .accessibility_label(setup.title),
                )
                .when_some(next, |this, next| {
                    this.child(
                        Pressable::new(next_id)
                            .accessibility_label(format!("Next: {next}"))
                            .track_focus(&next_focus)
                            .on_click(move |_, window, cx| {
                                if let Some(on_continue) = &on_continue {
                                    on_continue(window, cx);
                                }
                            })
                            .justify_start()
                            .gap_1()
                            .w_full()
                            .rounded(radius)
                            .line_height(relative(1.4))
                            .cursor_pointer()
                            .when(next_focused, |this| this.focus_ring_style(window, cx))
                            .child(div().flex_none().text_color(quiet).child("Next:"))
                            .child(div().flex_1().min_w_0().truncate().child(next))
                            .child(
                                Icon::new(IconName::ArrowRight)
                                    .small()
                                    .flex_none()
                                    .text_color(quiet),
                            ),
                    )
                })
        });

        v_flex()
            .id(self.id)
            .w(px(288.))
            .popover_style(cx)
            .border_1()
            .border_color(line)
            .text_color(ink)
            .text_sm()
            .child(
                h_flex()
                    .gap_3()
                    .p_3()
                    .child(div().flex_none().child(self.avatar.unwrap_or_else(|| {
                        Avatar::new().name(self.name.clone()).into_any_element()
                    })))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .line_height(relative(1.4))
                            .child(div().truncate().font_medium().child(self.name))
                            .when_some(self.email, |this, email| {
                                this.child(div().truncate().text_color(quiet).child(email))
                            }),
                    ),
            )
            .when(!rows.is_empty(), |this| {
                this.child(
                    v_flex()
                        .id("actions")
                        .role(Role::Menu)
                        .aria_label("Account")
                        .p_1()
                        .border_t_1()
                        .border_color(line)
                        .on_key_down(move |event, window, cx| {
                            let current =
                                handles.iter().position(|handle| handle.is_focused(window));
                            let key = event.keystroke.key.as_str();
                            if let Some(next) = next_index(key, current, handles.len()) {
                                handles[next].focus(window, cx);
                                cx.stop_propagation();
                            }
                        })
                        .children(rows),
                )
            })
            .children(setup)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_wrap_around_the_actions() {
        assert_eq!(next_index("down", Some(0), 3), Some(1));
        assert_eq!(next_index("down", Some(2), 3), Some(0));
        assert_eq!(next_index("up", Some(0), 3), Some(2));
    }

    #[test]
    fn home_and_end_jump_to_the_ends() {
        assert_eq!(next_index("home", Some(1), 3), Some(0));
        assert_eq!(next_index("end", Some(1), 3), Some(2));
    }

    #[test]
    fn an_arrow_with_nothing_focused_enters_the_list() {
        assert_eq!(next_index("down", None, 3), Some(0));
        assert_eq!(next_index("up", None, 3), Some(2));
    }

    #[test]
    fn other_keys_and_empty_menus_do_nothing() {
        assert_eq!(next_index("enter", Some(0), 3), None);
        assert_eq!(next_index("down", None, 0), None);
    }

    #[test]
    fn setup_progress_is_clamped() {
        assert_eq!(ProfileSetup::new(3, 5).percent(), 60.);
        assert_eq!(ProfileSetup::new(9, 5).percent(), 100.);
        assert_eq!(ProfileSetup::new(0, 0).percent(), 0.);
        assert!(ProfileSetup::new(5, 5).is_complete());
    }
}
