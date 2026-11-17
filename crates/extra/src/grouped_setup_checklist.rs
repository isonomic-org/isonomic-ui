// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Grouped setup checklist: onboarding split into areas, each a few steps that
//! build on each other.
//!
//! A bar and a count show overall progress. Each group folds open to its
//! tasks; inside a group a task unlocks once the ones before it are done. A
//! finished group is struck through, folds away and opens the next one.

use std::rc::Rc;

use isonomic_ui::base::{Button as Pressable, Toggle};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    progress::Progress,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One step inside a group.
#[derive(Debug, Clone)]
pub struct SetupTask {
    id: SharedString,
    label: SharedString,
    done: bool,
}

impl SetupTask {
    /// Create an open task.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            done: false,
        }
    }

    /// Whether the task starts finished.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }
}

/// An area of setup with its tasks, in the order they must be done.
#[derive(Debug, Clone)]
pub struct SetupGroup {
    id: SharedString,
    title: SharedString,
    tasks: Vec<SetupTask>,
}

impl SetupGroup {
    /// Create an empty group.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tasks: Vec::new(),
        }
    }

    /// Add a task after the ones already added.
    pub fn task(mut self, task: SetupTask) -> Self {
        self.tasks.push(task);
        self
    }
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type ChangeHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;

/// A panel that tracks setup progress through collapsible groups of tasks.
#[derive(IntoElement)]
pub struct GroupedSetupChecklist {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    groups: Vec<SetupGroup>,
    on_change: Option<ChangeHandler>,
    on_edit: Option<Handler>,
    on_close: Option<Handler>,
}

impl GroupedSetupChecklist {
    /// Create an empty panel titled "Setup".
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Setup".into(),
            groups: Vec::new(),
            on_change: None,
            on_edit: None,
            on_close: None,
        }
    }

    /// Set the heading above the bar.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Add a group.
    pub fn group(mut self, group: SetupGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Run after a task was ticked or unticked, with the ids of all finished tasks.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Show an Edit button in the header.
    pub fn on_edit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// Show a close button in the header.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl Styled for GroupedSetupChecklist {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct PanelState {
    seeded: bool,
    /// Ids of the finished tasks.
    done: Vec<SharedString>,
    /// Ids of the groups folded open.
    open: Vec<SharedString>,
    /// The whole panel folded down to its header and bar.
    minimized: bool,
}

/// The plain shape of the groups: each group's id with its task ids in order.
type Shape = Vec<(SharedString, Vec<SharedString>)>;

fn is_finished(tasks: &[SharedString], done: &[SharedString]) -> bool {
    !tasks.is_empty() && tasks.iter().all(|task| done.contains(task))
}

/// The first group that still has something to do.
fn first_unfinished(shape: &Shape, done: &[SharedString]) -> Option<SharedString> {
    shape
        .iter()
        .find(|(_, tasks)| !is_finished(tasks, done))
        .map(|(id, _)| id.clone())
}

/// A task can be pressed when it is finished, or when it is the first one in
/// its group that is not.
fn is_unlocked(tasks: &[SharedString], done: &[SharedString], task: &SharedString) -> bool {
    done.contains(task) || tasks.iter().find(|other| !done.contains(other)) == Some(task)
}

impl PanelState {
    fn seed(&mut self, groups: &[SetupGroup], shape: &Shape) {
        if self.seeded {
            return;
        }
        self.seeded = true;
        self.done = groups
            .iter()
            .flat_map(|group| &group.tasks)
            .filter(|task| task.done)
            .map(|task| task.id.clone())
            .collect();
        self.open = first_unfinished(shape, &self.done).into_iter().collect();
    }

    /// Tick or untick a task. Finishing a group folds it away and opens the
    /// next group with something left to do.
    fn toggle(&mut self, shape: &Shape, group: &SharedString, task: &SharedString) {
        match self.done.iter().position(|done| done == task) {
            Some(ix) => {
                self.done.remove(ix);
            }
            None => self.done.push(task.clone()),
        }
        let finished = shape
            .iter()
            .any(|(id, tasks)| id == group && is_finished(tasks, &self.done));
        if finished {
            self.open.retain(|open| open != group);
            if let Some(next) = first_unfinished(shape, &self.done)
                && !self.open.contains(&next)
            {
                self.open.push(next);
            }
        }
    }
}

impl RenderOnce for GroupedSetupChecklist {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let panel = theme.foreground.opacity(0.03);
        let accent = theme.info;
        let on_accent = theme.info_foreground;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let shape: Shape = self
            .groups
            .iter()
            .map(|group| {
                let tasks = group.tasks.iter().map(|task| task.id.clone()).collect();
                (group.id.clone(), tasks)
            })
            .collect();
        let shape = Rc::new(shape);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| PanelState::default());
        let (done, open, minimized) = state.update(cx, |state, _| {
            state.seed(&self.groups, &shape);
            (state.done.clone(), state.open.clone(), state.minimized)
        });

        let total: usize = shape.iter().map(|(_, tasks)| tasks.len()).sum();
        let count = shape
            .iter()
            .flat_map(|(_, tasks)| tasks)
            .filter(|task| done.contains(task))
            .count();
        let percent = if total == 0 {
            0.
        } else {
            count as f32 * 100. / total as f32
        };
        let child_id =
            |name: &SharedString| ElementId::NamedChild(self.id.clone().into(), name.clone());
        let on_change = self.on_change;

        let groups = self.groups.into_iter().map(|group| {
            let task_ids: Vec<SharedString> =
                group.tasks.iter().map(|task| task.id.clone()).collect();
            let finished = is_finished(&task_ids, &done);
            let is_open = open.contains(&group.id);
            let head_id = child_id(&group.id);
            let head_focus = window
                .use_keyed_state((head_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone();
            let head_focused = head_focus.is_focused(window);
            let group_id = group.id.clone();
            let group_done = task_ids.iter().filter(|id| done.contains(id)).count();
            let head_name = format!("{}, {group_done} of {} done", group.title, task_ids.len());

            let head = Pressable::new(head_id)
                .accessibility_label(head_name)
                .aria_expanded(is_open)
                .track_focus(&head_focus)
                .on_click({
                    let state = state.clone();
                    let group_id = group_id.clone();
                    move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            match state.open.iter().position(|open| open == &group_id) {
                                Some(ix) => {
                                    state.open.remove(ix);
                                }
                                None => state.open.push(group_id.clone()),
                            }
                            cx.notify();
                        })
                    }
                })
                .justify_between()
                .gap_2()
                .w_full()
                .h(px(36.))
                .px_3()
                .rounded(radius)
                .line_height(relative(1.4))
                .cursor_pointer()
                .when(head_focused, |this| this.focus_ring_style(window, cx))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .when(finished, |this| this.text_color(quiet).line_through())
                        .child(group.title),
                )
                .child(
                    Icon::new(if is_open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .small()
                    .flex_none()
                    .text_color(quiet),
                );

            let tasks = group.tasks.into_iter().map(|task| {
                let is_done = done.contains(&task.id);
                let unlocked = is_unlocked(&task_ids, &done, &task.id);
                let row_id = child_id(&task.id);
                let focus = window
                    .use_keyed_state((row_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone();
                let is_focused = focus.is_focused(window);
                let state = state.clone();
                let shape = shape.clone();
                let on_change = on_change.clone();
                let group_id = group_id.clone();
                let task_id = task.id.clone();

                Toggle::new(row_id)
                    .pressed(is_done)
                    .disabled(!unlocked)
                    .accessibility_label(task.label.clone())
                    .track_focus(&focus)
                    .on_change(move |_, _, window, cx| {
                        let done = state.update(cx, |state, cx| {
                            state.toggle(&shape, &group_id, &task_id);
                            cx.notify();
                            state.done.clone()
                        });
                        if let Some(on_change) = &on_change {
                            on_change(&done, window, cx);
                        }
                    })
                    .h_flex()
                    .gap_2p5()
                    .w_full()
                    .h(px(32.))
                    .px_3()
                    .rounded(radius)
                    .when(unlocked, |this| this.cursor_pointer())
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(18.))
                            .rounded_full_style(cx)
                            .border_1()
                            .map(|this| {
                                if is_done {
                                    this.border_color(accent)
                                        .bg(accent)
                                        .text_color(on_accent)
                                        .child(Icon::new(IconName::Check).size_3())
                                } else if unlocked {
                                    this.border_color(quiet)
                                } else {
                                    this.border_color(line).border_dashed()
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(is_done, |this| this.line_through())
                            .when(is_done || !unlocked, |this| this.text_color(quiet))
                            .child(task.label),
                    )
            });
            let tasks: Vec<_> = tasks.collect();

            v_flex()
                .rounded(radius)
                .when(is_open, |this| this.bg(panel).pb_1())
                .child(head)
                .when(is_open, |this| this.children(tasks))
        });
        let groups: Vec<_> = groups.collect();

        let press = |handler: Handler| {
            move |_: &isonomic_ui::ClickEvent, window: &mut Window, cx: &mut App| {
                handler(window, cx)
            }
        };
        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(340.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_base()
                                    .font_medium()
                                    .child(self.title.clone()),
                            )
                            .when_some(self.on_edit, |this, on_edit| {
                                this.child(
                                    Button::new((self.id.clone(), "edit"))
                                        .ghost()
                                        .xsmall()
                                        .label("Edit")
                                        .on_click(press(on_edit)),
                                )
                            })
                            .child(
                                Button::new((self.id.clone(), "minimize"))
                                    .ghost()
                                    .xsmall()
                                    .icon(if minimized {
                                        IconName::Maximize
                                    } else {
                                        IconName::Minimize
                                    })
                                    .accessibility_label(if minimized {
                                        "Show the tasks"
                                    } else {
                                        "Hide the tasks"
                                    })
                                    .on_click({
                                        let state = state.clone();
                                        move |_, _, cx| {
                                            state.update(cx, |state, cx| {
                                                state.minimized = !state.minimized;
                                                cx.notify();
                                            })
                                        }
                                    }),
                            )
                            .when_some(self.on_close, |this, on_close| {
                                this.child(
                                    Button::new((self.id.clone(), "close"))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Close)
                                        .accessibility_label("Close")
                                        .on_click(press(on_close)),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                div().flex_1().child(
                                    Progress::new("progress")
                                        .value(percent)
                                        .accessibility_label(self.title),
                                ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(quiet)
                                    .child(format!("{count}/{total}")),
                            ),
                    ),
            )
            .when(!minimized, |this| {
                this.child(
                    v_flex()
                        .gap_0p5()
                        .p_1()
                        .border_t_1()
                        .border_color(line)
                        .children(groups),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape() -> Shape {
        vec![
            ("workspace".into(), vec!["name".into(), "invite".into()]),
            ("billing".into(), vec!["card".into()]),
        ]
    }

    #[test]
    fn tasks_unlock_in_order() {
        let tasks: Vec<SharedString> = vec!["name".into(), "invite".into()];
        assert!(is_unlocked(&tasks, &[], &"name".into()));
        assert!(!is_unlocked(&tasks, &[], &"invite".into()));
        assert!(is_unlocked(&tasks, &["name".into()], &"invite".into()));
        // A finished task can always be pressed again to untick it.
        assert!(is_unlocked(&tasks, &["name".into()], &"name".into()));
    }

    #[test]
    fn finishing_a_group_folds_it_and_opens_the_next() {
        let mut state = PanelState {
            seeded: true,
            open: vec!["workspace".into()],
            ..PanelState::default()
        };
        state.toggle(&shape(), &"workspace".into(), &"name".into());
        assert_eq!(state.open, ["workspace"]);
        state.toggle(&shape(), &"workspace".into(), &"invite".into());
        assert_eq!(state.open, ["billing"]);
    }

    #[test]
    fn seeding_opens_the_first_group_with_work_left() {
        let groups = [
            SetupGroup::new("workspace", "Workspace")
                .task(SetupTask::new("name", "Name it").done(true))
                .task(SetupTask::new("invite", "Invite").done(true)),
            SetupGroup::new("billing", "Billing").task(SetupTask::new("card", "Add a card")),
        ];
        let mut state = PanelState::default();
        state.seed(&groups, &shape());
        assert_eq!(state.done, ["name", "invite"]);
        assert_eq!(state.open, ["billing"]);
    }

    #[test]
    fn an_empty_group_is_never_finished() {
        assert!(!is_finished(&[], &[]));
    }
}
