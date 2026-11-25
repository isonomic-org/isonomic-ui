// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Project summary card: a project at a glance, opening for quick edits to its status, people and subtasks.
//!
//! Closed, the card shows the project's name, its status, who's on it, when
//! it's due and how many subtasks are done. Open, the status becomes a menu,
//! each person can be taken off, and subtasks can be ticked. Every edit goes
//! to the owner, which passes the project back.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// How a project is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    /// Going to plan.
    OnTrack,
    /// Could slip.
    AtRisk,
    /// Has slipped.
    OffTrack,
    /// Finished.
    Done,
}

impl ProjectStatus {
    /// Every status, in menu order.
    pub const ALL: [ProjectStatus; 4] = [Self::OnTrack, Self::AtRisk, Self::OffTrack, Self::Done];

    /// Its name.
    pub fn word(self) -> &'static str {
        match self {
            Self::OnTrack => "On track",
            Self::AtRisk => "At risk",
            Self::OffTrack => "Off track",
            Self::Done => "Done",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::OnTrack => Tone::Info,
            Self::AtRisk => Tone::Warning,
            Self::OffTrack => Tone::Danger,
            Self::Done => Tone::Success,
        }
    }
}

/// One subtask.
#[derive(Debug, Clone, PartialEq)]
pub struct Subtask {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// Whether it's done.
    pub done: bool,
}

impl Subtask {
    /// Create an open subtask.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            done: false,
        }
    }

    /// Mark it done.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }
}

/// What the card reports.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectEdit {
    /// The card was opened or closed.
    Toggle,
    /// A new status was picked.
    Status(ProjectStatus),
    /// A person was taken off.
    RemovePerson(SharedString),
    /// A subtask was ticked or unticked.
    Subtask(SharedString, bool),
}

/// Initials for a name: up to two letters.
pub fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

type EditHandler = Rc<dyn Fn(ProjectEdit, &mut Window, &mut App)>;

/// A project summary that opens for quick edits.
#[derive(IntoElement)]
pub struct ProjectSummaryCard {
    id: ElementId,
    style: StyleRefinement,
    name: SharedString,
    status: ProjectStatus,
    people: Vec<SharedString>,
    due: SharedString,
    subtasks: Vec<Subtask>,
    open: bool,
    on_edit: Option<EditHandler>,
}

impl ProjectSummaryCard {
    /// Create a card.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        status: ProjectStatus,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            name: name.into(),
            status,
            people: Vec::new(),
            due: SharedString::default(),
            subtasks: Vec::new(),
            open: false,
            on_edit: None,
        }
    }

    /// Set who's on it.
    pub fn people(mut self, people: Vec<SharedString>) -> Self {
        self.people = people;
        self
    }

    /// Set when it's due, as written.
    pub fn due(mut self, due: impl Into<SharedString>) -> Self {
        self.due = due.into();
        self
    }

    /// Set its subtasks.
    pub fn subtasks(mut self, subtasks: Vec<Subtask>) -> Self {
        self.subtasks = subtasks;
        self
    }

    /// Open it for edits.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Run with each edit.
    pub fn on_edit(
        mut self,
        handler: impl Fn(ProjectEdit, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }
}

impl Styled for ProjectSummaryCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ProjectSummaryCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius_lg = theme.radius_lg;
        let tone = self.status.tone();
        let tone_color = tone.color(cx);
        let tint = tone.tint(cx);
        let done = self.subtasks.iter().filter(|s| s.done).count();
        let total = self.subtasks.len();
        let share = if total == 0 {
            0.
        } else {
            done as f32 / total as f32
        };
        let edit = self.on_edit.clone();
        let emit = move |e: ProjectEdit, window: &mut Window, cx: &mut App| {
            if let Some(edit) = &edit {
                edit(e, window, cx);
            }
        };
        let emit = Rc::new(emit);

        let faces = h_flex().children(self.people.iter().take(4).enumerate().map(|(ix, name)| {
            div()
                .size(px(22.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_2()
                .border_color(fill)
                .bg(chip)
                .when(ix > 0, |this| this.ml(px(-6.)))
                .text_label_small()
                .child(initials(name))
        }));

        let status_chip = h_flex()
            .gap_1()
            .px_1p5()
            .rounded(px(4.))
            .bg(tint)
            .text_caption()
            .text_color(tone_color)
            .child(div().size(px(6.)).rounded_full().bg(tone_color))
            .child(self.status.word());

        let head = {
            let emit = emit.clone();
            h_flex()
                .id(ElementId::NamedChild(self.id.clone().into(), "head".into()))
                .gap_3()
                .px_4()
                .py_3()
                .cursor_pointer()
                .role(Role::Button)
                .aria_label(format!(
                    "{}, {}, {done} of {total} subtasks done",
                    self.name,
                    self.status.word()
                ))
                .aria_expanded(self.open)
                .on_click(move |_, window, cx| emit(ProjectEdit::Toggle, window, cx))
                .child(
                    div()
                        .w(px(3.))
                        .h(px(34.))
                        .flex_none()
                        .rounded_full()
                        .bg(tone_color),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(div().font_medium().truncate().child(self.name.clone()))
                        .child(
                            h_flex()
                                .gap_2()
                                .text_caption()
                                .text_color(quiet)
                                .when(!self.open, |this| this.child(status_chip))
                                .when(!self.due.is_empty(), |this| {
                                    this.child(format!("Due {}", self.due))
                                })
                                .when(total > 0, |this| this.child(format!("{done}/{total} done"))),
                        ),
                )
                .child(faces)
                .child(
                    Icon::new(if self.open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall()
                    .text_color(quiet),
                )
        };

        let body = self.open.then(|| {
            let status_menu = {
                let emit = emit.clone();
                let current = self.status;
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    "status".into(),
                ))
                .xsmall()
                .outline()
                .label(self.status.word())
                .dropdown_caret(true)
                .accessibility_label(format!("Status: {}", self.status.word()))
                .dropdown_menu(move |menu, _, _| {
                    ProjectStatus::ALL.iter().fold(menu, |menu, status| {
                        let emit = emit.clone();
                        let status = *status;
                        menu.item(
                            PopupMenuItem::new(status.word())
                                .checked(status == current)
                                .on_click(move |_, window, cx| {
                                    emit(ProjectEdit::Status(status), window, cx)
                                }),
                        )
                    })
                })
            };
            let people: Vec<_> = self
                .people
                .iter()
                .map(|name| {
                    let emit = emit.clone();
                    let who = name.clone();
                    h_flex()
                        .gap_1()
                        .pl_0p5()
                        .rounded_full()
                        .border_1()
                        .border_color(line)
                        .text_caption()
                        .child(
                            div()
                                .size(px(18.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(chip)
                                .text_caption()
                                .child(initials(name)),
                        )
                        .child(name.clone())
                        .child(
                            Button::new(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("off-{name}").into(),
                            ))
                            .xsmall()
                            .ghost()
                            .icon(IconName::Close)
                            .accessibility_label(format!("Take {name} off the project"))
                            .on_click(move |_, window, cx| {
                                emit(ProjectEdit::RemovePerson(who.clone()), window, cx)
                            }),
                        )
                })
                .collect();
            let tasks: Vec<_> = self
                .subtasks
                .iter()
                .map(|task| {
                    let emit = emit.clone();
                    let id = task.id.clone();
                    let done = task.done;
                    h_flex()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("task-{}", task.id).into(),
                        ))
                        .gap_2()
                        .py_1()
                        .cursor_pointer()
                        .role(Role::CheckBox)
                        .aria_label(format!(
                            "{}, {}",
                            task.label,
                            if done { "done" } else { "open" }
                        ))
                        .aria_selected(done)
                        .on_click(move |_, window, cx| {
                            emit(ProjectEdit::Subtask(id.clone(), !done), window, cx)
                        })
                        .child(
                            div()
                                .size(px(16.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .border_1()
                                .border_color(if done { tone_color } else { line })
                                .when(done, |this| this.bg(tone_color))
                                .when(done, |this| {
                                    this.child(Icon::new(IconName::Check).xsmall().text_color(fill))
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .when(done, |this| this.line_through().text_color(quiet))
                                .child(task.label.clone()),
                        )
                })
                .collect();
            v_flex()
                .gap_3()
                .px_4()
                .pb_4()
                .pt_3()
                .border_t_1()
                .border_color(line)
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().w(px(64.)).text_caption().text_color(quiet).child("Status"))
                        .child(status_menu),
                )
                .child(
                    h_flex()
                        .items_start()
                        .gap_2()
                        .child(
                            div()
                                .w(px(64.))
                                .pt_0p5()
                                .text_caption()
                                .text_color(quiet)
                                .child("People"),
                        )
                        .child(h_flex().flex_1().flex_wrap().gap_1p5().children(people)),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(64.))
                                        .text_caption()
                                        .text_color(quiet)
                                        .child("Subtasks"),
                                )
                                .child(
                                    div().flex_1().h(px(4.)).rounded_full().bg(chip).child(
                                        div()
                                            .h_full()
                                            .rounded_full()
                                            .bg(tone_color)
                                            .w(relative(share)),
                                    ),
                                )
                                .child(
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(format!("{done}/{total}")),
                                ),
                        )
                        .child(v_flex().pl(px(72.)).children(tasks)),
                )
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(520.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(head)
            .children(body)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_and_status_words() {
        assert_eq!(initials("Maya Okafor"), "MO");
        assert_eq!(ProjectStatus::AtRisk.word(), "At risk");
        assert_eq!(ProjectStatus::ALL.len(), 4);
    }
}
