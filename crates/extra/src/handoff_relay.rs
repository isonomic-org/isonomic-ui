// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Handoff relay: a task passed from worker to worker.
//!
//! The workers stand in a row, each with what it is for and what it is on.
//! Between two workers, a column holds what one handed to the next: a card
//! per task with what went along, and what got lost on the way. Cards can be
//! pushed on to a later worker, by dragging or from the opened card.

use std::{any::Any, rc::Rc};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Bounds, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, Pixels, Render, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// The height of the band where the lines run from workers to columns.
const LINES: f32 = 34.;

/// One worker of the relay.
#[derive(Debug, Clone)]
pub struct RelayWorker {
    id: SharedString,
    name: SharedString,
    model: Option<SharedString>,
    about: Option<SharedString>,
    cost: Option<f64>,
}

impl RelayWorker {
    /// Create a worker.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            model: None,
            about: None,
            cost: None,
        }
    }

    /// Name the model behind it.
    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Say what it is for, in a sentence.
    pub fn about(mut self, about: impl Into<SharedString>) -> Self {
        self.about = Some(about.into());
        self
    }

    /// Say what it has cost so far, in dollars.
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }
}

/// Where a task stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayTaskStatus {
    /// Not picked up yet.
    Todo,
    /// A worker is on it.
    Working,
    /// Through the relay and fine.
    Done,
    /// Through, but someone should look.
    Warning,
}

/// A task that travels down the relay.
#[derive(Clone)]
pub struct RelayTask {
    id: SharedString,
    title: SharedString,
    icon: Option<IconName>,
    status: RelayTaskStatus,
    worker: Option<SharedString>,
    note: Option<SharedString>,
}

impl RelayTask {
    /// Create a task that waits to be picked up.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            status: RelayTaskStatus::Todo,
            worker: None,
            note: None,
        }
    }

    /// Show a mark before the title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set where it stands.
    pub fn status(mut self, status: RelayTaskStatus) -> Self {
        self.status = status;
        self
    }

    /// Say which worker is on it, and what it is doing in a word.
    pub fn with(mut self, worker: impl Into<SharedString>, note: impl Into<SharedString>) -> Self {
        self.status = RelayTaskStatus::Working;
        self.worker = Some(worker.into());
        self.note = Some(note.into());
        self
    }

    /// The task's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// What kind of thing went along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// A file.
    File,
    /// A source looked at.
    Source,
    /// A fact found.
    Fact,
    /// A decision taken.
    Decision,
    /// Anything else.
    Note,
}

/// One thing handed over, or lost on the way.
#[derive(Debug, Clone)]
pub struct HandoffItem {
    id: SharedString,
    label: SharedString,
    kind: ItemKind,
    lost: Option<SharedString>,
}

impl HandoffItem {
    /// Create an item that went along.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: ItemKind::Note,
            lost: None,
        }
    }

    /// Say what kind of thing it is.
    pub fn kind(mut self, kind: ItemKind) -> Self {
        self.kind = kind;
        self
    }

    /// Mark it lost on the way, and why.
    pub fn lost(mut self, why: impl Into<SharedString>) -> Self {
        self.lost = Some(why.into());
        self
    }

    /// The item's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// One handover from a worker to another.
#[derive(Debug, Clone)]
pub struct Handoff {
    id: SharedString,
    from: SharedString,
    to: SharedString,
    task: SharedString,
    items: Vec<HandoffItem>,
}

impl Handoff {
    /// Create a handover of `task` from one worker to another.
    pub fn new(
        id: impl Into<SharedString>,
        from: impl Into<SharedString>,
        to: impl Into<SharedString>,
        task: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            task: task.into(),
            items: Vec::new(),
        }
    }

    /// Add something that went along, or was lost.
    pub fn item(mut self, item: HandoffItem) -> Self {
        self.items.push(item);
        self
    }

    /// Mark a lost item as passed on after all.
    pub fn restore(mut self, item: &str) -> Self {
        for found in self
            .items
            .iter_mut()
            .filter(|found| found.id.as_ref() == item)
        {
            found.lost = None;
        }
        self
    }

    /// The handover's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

type AssignHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type RestoreHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;

/// A row of workers with what each handed to the next between them.
#[derive(IntoElement)]
pub struct HandoffRelay {
    id: ElementId,
    style: StyleRefinement,
    workers: Vec<RelayWorker>,
    tasks: Vec<RelayTask>,
    handoffs: Vec<Handoff>,
    on_assign: Option<AssignHandler>,
    on_restore: Option<RestoreHandler>,
}

impl HandoffRelay {
    /// Create an empty relay.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            workers: Vec::new(),
            tasks: Vec::new(),
            handoffs: Vec::new(),
            on_assign: None,
            on_restore: None,
        }
    }

    /// Add a worker, in relay order.
    pub fn worker(mut self, worker: RelayWorker) -> Self {
        self.workers.push(worker);
        self
    }

    /// Add a task.
    pub fn task(mut self, task: RelayTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Add a handover. The latest shows on top of its column.
    pub fn handoff(mut self, handoff: Handoff) -> Self {
        self.handoffs.push(handoff);
        self
    }

    /// Run when a task is pushed on to a later worker: `(task, worker)`.
    /// Cards only move down the relay, never back.
    pub fn on_assign(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_assign = Some(Rc::new(handler));
        self
    }

    /// Offer "Add back" on lost items: `(handoff, item)`.
    pub fn on_restore(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_restore = Some(Rc::new(handler));
        self
    }
}

impl Styled for HandoffRelay {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// What a dragged card carries.
#[derive(Clone)]
struct DraggedTask {
    task: SharedString,
    title: SharedString,
    /// The column it came from; it may go to workers after this one.
    column: usize,
}

impl Render for DraggedTask {
    fn render(&mut self, _: &mut Window, cx: &mut isonomic_ui::Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px_3()
            .py_1p5()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_body()
            .text_color(theme.foreground)
            .shadow_md()
            .child(self.title.clone())
    }
}

#[derive(Default)]
struct RelayUi {
    open: Option<SharedString>,
    why: Option<(SharedString, SharedString)>,
}

/// The column a handover sits in: the gap after the worker it came from.
fn column_of(workers: &[RelayWorker], handoff: &Handoff) -> Option<usize> {
    let at = workers
        .iter()
        .position(|worker| worker.id == handoff.from)?;
    (at + 1 < workers.len()).then_some(at)
}

/// The carried items in a line: "brief.md, 12 sources".
fn carried_line(items: &[HandoffItem]) -> String {
    items
        .iter()
        .filter(|item| item.lost.is_none())
        .map(|item| item.label.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn kind_icon(kind: ItemKind) -> IconName {
    match kind {
        ItemKind::File => IconName::File,
        ItemKind::Source => IconName::BookOpen,
        ItemKind::Fact => IconName::Info,
        ItemKind::Decision => IconName::Check,
        ItemKind::Note => IconName::FileText,
    }
}

impl RenderOnce for HandoffRelay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        // A tint in the ring colour marks the part the keyboard is on.
        let focus_tint = theme.ring.opacity(0.25);
        let accent = theme.blue;
        let success = theme.success;
        let warning = theme.warning;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let ui = window.use_keyed_state(self.id.clone(), cx, |_, _| RelayUi::default());
        let (open, why) = {
            let ui = ui.read(cx);
            (ui.open.clone(), ui.why.clone())
        };
        let workers = Rc::new(self.workers);
        let count = workers.len();
        let tasks = Rc::new(self.tasks);
        let task_of = |id: &SharedString| tasks.iter().find(|task| &task.id == id);
        let name_of = |id: &SharedString| {
            workers
                .iter()
                .find(|worker| &worker.id == id)
                .map(|worker| worker.name.clone())
                .unwrap_or_else(|| id.clone())
        };

        let cards = workers.iter().enumerate().map(|(ix, worker)| {
            let working = tasks.iter().find(|task| {
                task.status == RelayTaskStatus::Working && task.worker.as_ref() == Some(&worker.id)
            });
            let waiting: Vec<&RelayTask> = tasks
                .iter()
                .filter(|task| task.status == RelayTaskStatus::Todo)
                .collect();
            let worker_words = format!(
                "{}{}, {}{}",
                worker.name,
                worker
                    .model
                    .as_ref()
                    .map(|m| format!(", {m}"))
                    .unwrap_or_default(),
                match working {
                    Some(task) => format!("working on {}", task.title),
                    None => "idle".to_string(),
                },
                if ix == 0 && !waiting.is_empty() {
                    format!(", {} waiting", waiting.len())
                } else {
                    String::new()
                },
            );
            let current = match (working, ix == 0 && !waiting.is_empty()) {
                (Some(task), _) => h_flex()
                    .gap_1p5()
                    .children(task.icon.clone().map(|icon| Icon::new(icon).xsmall()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(task.title.clone()),
                    )
                    .children(
                        task.note
                            .clone()
                            .map(|note| div().flex_none().text_color(quiet).child(note)),
                    )
                    .into_any_element(),
                (None, true) => h_flex()
                    .gap_1p5()
                    .child(div().flex_none().text_color(quiet).child("Up next"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(waiting[0].title.clone()),
                    )
                    .when(waiting.len() > 1, |this| {
                        this.child(
                            div()
                                .flex_none()
                                .text_color(quiet)
                                .child(format!("+{}", waiting.len() - 1)),
                        )
                    })
                    .into_any_element(),
                (None, false) => div()
                    .text_color(quiet)
                    .child("Ready for a task")
                    .into_any_element(),
            };
            let accepts = move |value: &dyn Any, _: &mut Window, _: &mut App| {
                value
                    .downcast_ref::<DraggedTask>()
                    .is_some_and(|dragged| ix > dragged.column)
            };
            let on_assign = self.on_assign.clone();
            let worker_id = worker.id.clone();
            v_flex()
                .id(("relay-worker", ix))
                .role(Role::Group)
                .aria_label(worker_words)
                .flex_1()
                .min_w_0()
                .gap_2()
                .p_3()
                .rounded(radius_lg)
                .bg(fill)
                .shadow(resting_shadow(cx))
                .can_drop(accepts)
                .drag_over::<DraggedTask>(move |style, dragged, _, _| {
                    if ix > dragged.column {
                        style.border_color(accent).bg(accent.opacity(0.06))
                    } else {
                        style.opacity(0.5)
                    }
                })
                .on_drop(move |dragged: &DraggedTask, window, cx| {
                    if let Some(handler) = &on_assign {
                        handler(&dragged.task, &worker_id, window, cx);
                    }
                })
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .border_1()
                                .border_color(line)
                                .child(Icon::new(IconName::Bot).small().text_color(quiet)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().font_medium().truncate().child(worker.name.clone()))
                                .children(worker.model.clone().map(|model| {
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .truncate()
                                        .child(model)
                                })),
                        )
                        .children(worker.cost.map(|cost| {
                            div()
                                .flex_none()
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("${cost:.2}"))
                        }))
                        .child(
                            div()
                                .flex_none()
                                .px_1p5()
                                .rounded(px(4.))
                                .text_caption()
                                .map(|this| {
                                    if working.is_some() {
                                        this.bg(accent.opacity(0.12))
                                            .text_color(accent)
                                            .child("Working")
                                    } else {
                                        this.bg(chip).text_color(quiet).child("Idle")
                                    }
                                }),
                        ),
                )
                .children(worker.about.clone().map(|about| {
                    div()
                        .text_caption()
                        .text_color(quiet)
                        .line_height(relative(1.4))
                        .child(about)
                }))
                .child(
                    div()
                        .px_2()
                        .py_1p5()
                        .rounded(radius)
                        .bg(chip.opacity(0.6))
                        .text_caption()
                        .child(current),
                )
        });
        let cards: Vec<_> = cards.collect();

        // Columns of handovers, one in each gap.
        let columns = (0..count.saturating_sub(1)).map(|column| {
            let mut here: Vec<&Handoff> = self
                .handoffs
                .iter()
                .filter(|handoff| column_of(&workers, handoff) == Some(column))
                .collect();
            here.reverse();
            let (from, to) = (&workers[column], &workers[column + 1]);
            let handoff_cards = here.into_iter().enumerate().map(|(row, handoff)| {
                let task = task_of(&handoff.task);
                let title = task
                    .map(|task| task.title.clone())
                    .unwrap_or_else(|| handoff.task.clone());
                let is_open = open.as_ref() == Some(&handoff.id);
                // A card that skipped ahead says who it went to.
                let skipped = (handoff.to != to.id).then(|| name_of(&handoff.to));
                let lost = handoff
                    .items
                    .iter()
                    .filter(|item| item.lost.is_some())
                    .count();
                let (state_color, state_words) = match task
                    .map(|task| (task.status, task.worker.clone()))
                {
                    Some((RelayTaskStatus::Working, Some(worker))) => {
                        (accent, format!("With {}", name_of(&worker)))
                    }
                    Some((RelayTaskStatus::Done, _)) => (success, "Looks good".to_string()),
                    Some((RelayTaskStatus::Warning, _)) => (warning, "Needs a look".to_string()),
                    _ => (quiet, "Waiting".to_string()),
                };
                let toggle = {
                    let ui = ui.clone();
                    let id = handoff.id.clone();
                    move |_: &isonomic_ui::ClickEvent, _: &mut Window, cx: &mut App| {
                        ui.update(cx, |ui, cx| {
                            ui.open = if ui.open.as_ref() == Some(&id) {
                                None
                            } else {
                                Some(id.clone())
                            };
                            ui.why = None;
                            cx.notify();
                        })
                    }
                };
                let dragged = DraggedTask {
                    task: handoff.task.clone(),
                    title: title.clone(),
                    column,
                };
                let items = is_open.then(|| {
                    h_flex().flex_wrap().gap_1().pt_1().children(
                        handoff.items.iter().enumerate().map(|(ix, item)| {
                            let is_lost = item.lost.is_some();
                            let ui = ui.clone();
                            let key = (handoff.id.clone(), item.id.clone());
                            let item_words = match &item.lost {
                                Some(_) => format!("{}, lost on the way. Show why", item.label),
                                None => format!("{}, carried", item.label),
                            };
                            h_flex()
                                .id(("relay-item", ix))
                                .role(if is_lost {
                                    Role::Button
                                } else {
                                    Role::ListItem
                                })
                                .aria_label(item_words)
                                .when(is_lost, |this| {
                                    this.tab_index(0).focus_visible(move |s| s.bg(focus_tint))
                                })
                                .gap_1()
                                .px_1p5()
                                .py_0p5()
                                .rounded(px(4.))
                                .text_caption()
                                .border_1()
                                .map(|this| {
                                    if is_lost {
                                        this.border_color(warning.opacity(0.5))
                                            .text_color(warning)
                                            .line_through()
                                            .cursor_pointer()
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                ui.update(cx, |ui, cx| {
                                                    ui.why = if ui.why.as_ref() == Some(&key) {
                                                        None
                                                    } else {
                                                        Some(key.clone())
                                                    };
                                                    cx.notify();
                                                })
                                            })
                                    } else {
                                        this.border_color(line)
                                    }
                                })
                                .child(Icon::new(kind_icon(item.kind)).xsmall())
                                .child(item.label.clone())
                        }),
                    )
                });
                let why_row = why
                    .as_ref()
                    .filter(|(handoff_id, _)| handoff_id == &handoff.id)
                    .and_then(|(_, item_id)| handoff.items.iter().find(|item| &item.id == item_id))
                    .and_then(|item| item.lost.clone().map(|reason| (item.id.clone(), reason)))
                    .map(|(item_id, reason)| {
                        let restore = self.on_restore.clone();
                        let handoff_id = handoff.id.clone();
                        h_flex()
                            .gap_2()
                            .pt_1()
                            .text_caption()
                            .child(div().flex_1().min_w_0().text_color(quiet).child(reason))
                            .when(restore.is_some(), |this| {
                                this.child(
                                    Button::new(("relay-restore", row))
                                        .outline()
                                        .xsmall()
                                        .label("Add back")
                                        .on_click(move |_, window, cx| {
                                            cx.stop_propagation();
                                            if let Some(restore) = &restore {
                                                restore(&handoff_id, &item_id, window, cx);
                                            }
                                        }),
                                )
                            })
                    });
                let give = (is_open && self.on_assign.is_some()).then(|| {
                    h_flex().flex_wrap().gap_1().pt_1().children(
                        workers
                            .iter()
                            .enumerate()
                            .skip(column + 1)
                            .map(|(at, worker)| {
                                let on_assign = self.on_assign.clone();
                                let (task_id, worker_id) =
                                    (handoff.task.clone(), worker.id.clone());
                                Button::new(("relay-give", row * 100 + at))
                                    .ghost()
                                    .xsmall()
                                    .label(format!("Give to {}", worker.name))
                                    .on_click(move |_, window, cx| {
                                        cx.stop_propagation();
                                        if let Some(handler) = &on_assign {
                                            handler(&task_id, &worker_id, window, cx);
                                        }
                                    })
                            }),
                    )
                });
                let handoff_words = format!(
                    "{title}, from {} to {}, {}{}{}",
                    from.name,
                    name_of(&handoff.to),
                    state_words,
                    if lost > 0 {
                        format!(", {lost} lost")
                    } else {
                        String::new()
                    },
                    if is_open {
                        ""
                    } else {
                        ". Press Enter for details"
                    },
                );
                v_flex()
                    .id(("relay-handoff", column * 1000 + row))
                    .role(Role::Button)
                    .aria_label(handoff_words)
                    .aria_expanded(is_open)
                    .tab_index(0)
                    .focus_visible(move |s| s.bg(focus_tint))
                    .gap_0p5()
                    .p_2()
                    .rounded(radius)
                    .border_1()
                    .border_color(if is_open { accent.opacity(0.5) } else { line })
                    .bg(fill)
                    .cursor_pointer()
                    .on_click(toggle)
                    .when(self.on_assign.is_some(), |this| {
                        this.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
                    })
                    .child(
                        h_flex()
                            .gap_1p5()
                            .font_medium()
                            .children(
                                task.and_then(|task| task.icon.clone())
                                    .map(|icon| Icon::new(icon).xsmall()),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(title))
                            .children(skipped.map(|name| {
                                div()
                                    .flex_none()
                                    .text_caption()
                                    .font_normal()
                                    .text_color(quiet)
                                    .child(format!("To {name}"))
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .text_caption()
                            .text_color(quiet)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(carried_line(&handoff.items)),
                            )
                            .when(lost > 0, |this| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .text_color(warning)
                                        .child(format!("{lost} lost")),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .text_caption()
                            .child(div().size(px(6.)).rounded_full().bg(state_color))
                            .child(state_words),
                    )
                    .children(items)
                    .children(why_row)
                    .children(give)
            });
            let cards: Vec<_> = handoff_cards.collect();
            let total = cards.len();
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1p5()
                .p_1p5()
                .rounded(radius_lg)
                .bg(chip.opacity(0.5))
                .child(
                    h_flex()
                        .gap_1()
                        .px_1()
                        .text_caption()
                        .text_color(quiet)
                        .child(div().truncate().child(from.name.clone()))
                        .child(Icon::new(IconName::ArrowRight).xsmall())
                        .child(div().flex_1().min_w_0().truncate().child(to.name.clone()))
                        .child(
                            div()
                                .flex_none()
                                .px_1()
                                .rounded(px(4.))
                                .bg(accent.opacity(0.12))
                                .text_color(accent)
                                .child(total.to_string()),
                        ),
                )
                .children(cards)
                .when(total == 0, |this| {
                    this.child(
                        div()
                            .px_1()
                            .py_2()
                            .text_caption()
                            .text_color(quiet)
                            .child("Nothing handed over yet"),
                    )
                })
        });
        let columns: Vec<_> = columns.collect();

        // Lines from each pair of workers down into the column between them.
        let lines = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                if count < 2 {
                    return;
                }
                let slot = bounds.size.width / count as f32;
                let middle = bounds.top() + px(LINES / 2.);
                for column in 0..count - 1 {
                    let gap = bounds.left() + slot * (column + 1) as f32;
                    for (start, end) in [
                        (bounds.left() + slot * (column as f32 + 0.5), gap - px(8.)),
                        (bounds.left() + slot * (column as f32 + 1.5), gap + px(8.)),
                    ] {
                        let mut path = PathBuilder::stroke(px(1.5));
                        path.move_to(point(start, bounds.top()));
                        path.line_to(point(start, middle));
                        path.line_to(point(end, middle));
                        path.line_to(point(end, bounds.bottom()));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, accent.opacity(0.45));
                        }
                    }
                }
            },
        )
        .w_full()
        .h(px(LINES));

        let slot = relative(1. / count.max(1) as f32);
        v_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label("Handoff relay")
            .w_full()
            .text_color(ink)
            .text_body()
            .line_height(relative(1.3))
            .child(h_flex().items_start().gap_3().children(cards))
            .child(lines)
            .child(
                h_flex()
                    .items_start()
                    .gap_3()
                    .pl(relative(0.5 / count.max(1) as f32))
                    .pr(relative(0.5 / count.max(1) as f32))
                    .children(
                        columns
                            .into_iter()
                            .map(|column| div().w(slot).flex_1().child(column)),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workers() -> Vec<RelayWorker> {
        vec![
            RelayWorker::new("a", "A"),
            RelayWorker::new("b", "B"),
            RelayWorker::new("c", "C"),
        ]
    }

    #[test]
    fn handovers_sit_after_their_sender() {
        let workers = workers();
        assert_eq!(
            column_of(&workers, &Handoff::new("1", "a", "b", "t")),
            Some(0)
        );
        assert_eq!(
            column_of(&workers, &Handoff::new("2", "b", "c", "t")),
            Some(1)
        );
        // From the last worker there is no column to sit in.
        assert_eq!(column_of(&workers, &Handoff::new("3", "c", "a", "t")), None);
    }

    #[test]
    fn the_line_leaves_out_what_was_lost() {
        let items = [
            HandoffItem::new("1", "brief.md"),
            HandoffItem::new("2", "Old rates").lost("Not in the export"),
            HandoffItem::new("3", "12 sources"),
        ];
        assert_eq!(carried_line(&items), "brief.md, 12 sources");
        let handoff = items
            .into_iter()
            .fold(Handoff::new("h", "a", "b", "t"), Handoff::item)
            .restore("2");
        assert_eq!(
            carried_line(&handoff.items),
            "brief.md, Old rates, 12 sources"
        );
    }
}
