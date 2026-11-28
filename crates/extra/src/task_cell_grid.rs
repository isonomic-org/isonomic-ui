// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task cell grid: one large job split across many workers.
//!
//! Every task is a small cell on a grid that fills in as the workers get
//! through it. The bar on top says how far the job is, what it has cost and
//! roughly how long is left. The counts under the grid filter it to one
//! state, and the cell under the pointer or the arrow keys shows its details.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// The side of one cell.
const CELL: f32 = 14.;

/// Where one task stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellStatus {
    /// Waiting for a worker.
    Queued,
    /// A worker is on it.
    Running,
    /// It failed once and is being tried again.
    Retrying,
    /// Finished.
    Done,
    /// Gave up.
    Failed,
}

impl CellStatus {
    const ALL: [Self; 5] = [
        Self::Done,
        Self::Running,
        Self::Retrying,
        Self::Failed,
        Self::Queued,
    ];

    fn words(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Retrying => "retrying",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// One task of the job.
#[derive(Debug, Clone)]
pub struct GridTask {
    id: SharedString,
    label: SharedString,
    status: CellStatus,
    worker: Option<SharedString>,
    started: Option<Duration>,
    ended: Option<Duration>,
    cost: Option<f64>,
    attempts: u32,
    outcome: Option<SharedString>,
}

impl GridTask {
    /// Create a queued task.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: CellStatus::Queued,
            worker: None,
            started: None,
            ended: None,
            cost: None,
            attempts: 1,
            outcome: None,
        }
    }

    /// Set where the task stands.
    pub fn status(mut self, status: CellStatus) -> Self {
        self.status = status;
        self
    }

    /// Name the worker on it.
    pub fn worker(mut self, worker: impl Into<SharedString>) -> Self {
        self.worker = Some(worker.into());
        self
    }

    /// Set when the task started and, once it is over, when it ended. Both
    /// count from the start of the job.
    pub fn span(mut self, started: Duration, ended: Option<Duration>) -> Self {
        self.started = Some(started);
        self.ended = ended;
        self
    }

    /// Set what the task cost.
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Set how many times the task has been tried.
    pub fn attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts.max(1);
        self
    }

    /// Set a line about the result, or the error of a failed task.
    pub fn outcome(mut self, outcome: impl Into<SharedString>) -> Self {
        self.outcome = Some(outcome.into());
        self
    }
}

type Format = Rc<dyn Fn(f64) -> String>;
type RetryHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;

/// A grid with one cell per task, a progress bar and filters by state.
#[derive(IntoElement)]
pub struct TaskCellGrid {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    job: Option<SharedString>,
    tasks: Vec<GridTask>,
    workers: Option<usize>,
    columns: usize,
    now: Duration,
    format_cost: Format,
    on_retry: Option<RetryHandler>,
    on_select: Option<SelectHandler>,
}

impl TaskCellGrid {
    /// Create a grid with a heading.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            job: None,
            tasks: Vec::new(),
            workers: None,
            columns: 30,
            now: Duration::ZERO,
            format_cost: Rc::new(|cost| format!("{cost:.2}")),
            on_retry: None,
            on_select: None,
        }
    }

    /// Say what the whole job does, next to the heading.
    pub fn job(mut self, job: impl Into<SharedString>) -> Self {
        self.job = Some(job.into());
        self
    }

    /// Set the tasks, in the order their cells should show. Pass all of
    /// them each time something changes; ids keep cells in place.
    pub fn tasks(mut self, tasks: impl IntoIterator<Item = GridTask>) -> Self {
        self.tasks = tasks.into_iter().collect();
        self
    }

    /// Say how many workers run at once, for the time left. The tasks
    /// running now are counted otherwise.
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = Some(workers);
        self
    }

    /// Set how many cells make a row. 30 by default.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// Set the clock, counted from the start of the job.
    pub fn now(mut self, now: Duration) -> Self {
        self.now = now;
        self
    }

    /// Set how costs are written. Two decimals by default.
    pub fn format_cost(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format_cost = Rc::new(format);
        self
    }

    /// Offer to retry the failed tasks, and run with their ids.
    pub fn on_retry(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    /// Run when a task is pinned with a click or Enter, or with `None` when
    /// it is let go.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for TaskCellGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// How many tasks are in each state, in `CellStatus::ALL` order.
fn counts(tasks: &[GridTask]) -> [usize; 5] {
    let mut counts = [0; 5];
    for task in tasks {
        if let Some(ix) = CellStatus::ALL.iter().position(|s| *s == task.status) {
            counts[ix] += 1;
        }
    }
    counts
}

/// Roughly how long is left: the mean time of a finished task, times the
/// tasks not finished, over the workers. Needs three finished tasks.
fn time_left(tasks: &[GridTask], workers: Option<usize>) -> Option<Duration> {
    let finished: Vec<Duration> = tasks
        .iter()
        .filter(|task| task.status == CellStatus::Done)
        .filter_map(|task| Some(task.ended?.saturating_sub(task.started?)))
        .collect();
    if finished.len() < 3 {
        return None;
    }
    let open = tasks
        .iter()
        .filter(|task| !matches!(task.status, CellStatus::Done | CellStatus::Failed))
        .count();
    if open == 0 {
        return Some(Duration::ZERO);
    }
    let running = tasks
        .iter()
        .filter(|task| matches!(task.status, CellStatus::Running | CellStatus::Retrying))
        .count();
    let workers = workers.unwrap_or(running).max(1);
    let mean = finished.iter().sum::<Duration>() / finished.len() as u32;
    Some(mean * open.div_ceil(workers) as u32)
}

/// A duration in a few characters: "850 ms", "7.1 s", "3 m 20 s".
fn duration_words(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1000 {
        format!("{millis} ms")
    } else if millis < 60_000 {
        format!("{:.1} s", duration.as_secs_f32())
    } else {
        let seconds = duration.as_secs();
        format!("{} m {:02} s", seconds / 60, seconds % 60)
    }
}

/// Splits the cells into rows of `columns`.
fn chunked<T>(items: Vec<T>, columns: usize) -> Vec<Vec<T>> {
    let mut rows = Vec::new();
    let mut items = items.into_iter().peekable();
    while items.peek().is_some() {
        rows.push(items.by_ref().take(columns.max(1)).collect());
    }
    rows
}

/// The cell an arrow key moves to on a grid `columns` wide.
fn step_cell(current: Option<usize>, cells: usize, columns: usize, key: &str) -> Option<usize> {
    let last = cells.checked_sub(1)?;
    let columns = columns.max(1);
    let Some(ix) = current else {
        return matches!(key, "right" | "down" | "home" | "left" | "up" | "end").then_some(
            if matches!(key, "left" | "up" | "end") {
                last
            } else {
                0
            },
        );
    };
    match key {
        "right" => Some((ix + 1).min(last)),
        "left" => Some(ix.saturating_sub(1)),
        "down" => Some(if ix + columns <= last {
            ix + columns
        } else {
            ix
        }),
        "up" => Some(ix.checked_sub(columns).unwrap_or(ix)),
        "home" => Some(0),
        "end" => Some(last),
        _ => None,
    }
}

struct GridState {
    /// The cell the pointer or the arrows are on.
    active: Option<usize>,
    /// The cell kept open with a click or Enter.
    pinned: Option<usize>,
    /// The state the grid is filtered to.
    only: Option<CellStatus>,
    focus: FocusHandle,
}

impl RenderOnce for TaskCellGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let chip = theme.muted;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let (info, warning, success, danger) =
            (theme.info, theme.warning, theme.success, theme.danger);
        let tone = move |status: CellStatus| -> Hsla {
            match status {
                CellStatus::Queued => quiet.opacity(0.25),
                CellStatus::Running => info,
                CellStatus::Retrying => warning,
                CellStatus::Done => success,
                CellStatus::Failed => danger,
            }
        };

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| GridState {
            active: None,
            pinned: None,
            only: None,
            focus: cx.focus_handle().tab_stop(true),
        });
        let (active, pinned, only, focus) = {
            let state = state.read(cx);
            (state.active, state.pinned, state.only, state.focus.clone())
        };
        let total = self.tasks.len();
        let active = active.filter(|ix| *ix < total);
        let pinned = pinned.filter(|ix| *ix < total);
        let shown = pinned.or(active);
        let counts = counts(&self.tasks);
        let done = counts[0];
        let failed = counts[3];
        let spent: f64 = self.tasks.iter().filter_map(|task| task.cost).sum();
        let left = time_left(&self.tasks, self.workers);
        let failed_ids: Vec<SharedString> = self
            .tasks
            .iter()
            .filter(|task| task.status == CellStatus::Failed)
            .map(|task| task.id.clone())
            .collect();
        let columns = self.columns;

        let select = {
            let on_select = self.on_select.clone();
            let ids: Vec<SharedString> = self.tasks.iter().map(|task| task.id.clone()).collect();
            move |pin: Option<usize>, window: &mut Window, cx: &mut App| {
                if let Some(handler) = &on_select {
                    handler(pin.and_then(|ix| ids.get(ix)), window, cx);
                }
            }
        };
        let select = Rc::new(select);

        let cells = self.tasks.iter().enumerate().map(|(ix, task)| {
            let color = tone(task.status);
            let faded = only.is_some_and(|only| only != task.status);
            let is_shown = shown == Some(ix);
            div()
                .id(("cell", ix))
                .size(px(CELL))
                .rounded(px(3.))
                .border_1()
                .border_color(if is_shown { ink } else { color.opacity(0.6) })
                .bg(color.opacity(if task.status == CellStatus::Queued {
                    0.6
                } else {
                    0.3
                }))
                .when(faded, |this| this.opacity(0.2))
                .cursor_pointer()
                .on_hover({
                    let state = state.clone();
                    move |hovered, _, cx| {
                        state.update(cx, |state, cx| {
                            if *hovered {
                                state.active = Some(ix);
                                cx.notify();
                            } else if state.active == Some(ix) {
                                state.active = None;
                                cx.notify();
                            }
                        })
                    }
                })
                .on_click({
                    let state = state.clone();
                    let select = select.clone();
                    move |_, window, cx| {
                        let next = state.update(cx, |state, cx| {
                            state.pinned = (state.pinned != Some(ix)).then_some(ix);
                            cx.notify();
                            state.pinned
                        });
                        select(next, window, cx);
                    }
                })
        });
        let cells: Vec<_> = cells.collect();

        let details = shown.map(|ix| {
            let task = &self.tasks[ix];
            let mut facts: Vec<String> = vec![task.status.words().to_string()];
            if let Some(worker) = &task.worker {
                facts.push(worker.to_string());
            }
            if let Some(started) = task.started {
                let end = task.ended.unwrap_or(self.now);
                facts.push(duration_words(end.saturating_sub(started)));
            }
            if let Some(cost) = task.cost {
                facts.push((self.format_cost)(cost));
            }
            if task.attempts > 1 {
                facts.push(format!("try {}", task.attempts));
            }
            v_flex()
                .gap_0p5()
                .px_3()
                .py_2()
                .rounded(radius)
                .bg(chip)
                .text_caption()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .size_2()
                                .rounded(px(2.))
                                .bg(tone(task.status)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_medium()
                                .text_color(ink)
                                .child(task.label.clone()),
                        )
                        .when(pinned == Some(ix), |this| {
                            this.child(div().flex_none().text_color(quiet).child("kept open"))
                        }),
                )
                .child(div().truncate().text_color(quiet).child(facts.join(", ")))
                .children(task.outcome.clone().map(|outcome| {
                    div()
                        .truncate()
                        .text_color(if task.status == CellStatus::Failed {
                            danger
                        } else {
                            quiet
                        })
                        .child(outcome)
                }))
        });

        let is_focused = focus.is_focused(window);
        let grid = div()
            .id((self.id.clone(), "grid"))
            .track_focus(&focus)
            // The grid is one Tab stop driven by the arrow keys, so it is
            // announced as a named group that says so.
            .role(Role::Group)
            .aria_label(format!("{}, tasks", self.title))
            .aria_description("Arrow keys move between tasks.")
            .on_key_down({
                let state = state.clone();
                let select = select.clone();
                move |event, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let key = event.keystroke.key.as_str();
                    match key {
                        "enter" | "space" => {
                            let next = state.update(cx, |state, cx| {
                                state.pinned = match (state.pinned, state.active) {
                                    (Some(_), _) => None,
                                    (None, active) => active,
                                };
                                cx.notify();
                                state.pinned
                            });
                            select(next, window, cx);
                        }
                        "escape" => {
                            let had = state.update(cx, |state, cx| {
                                let had = state.pinned.is_some();
                                state.pinned = None;
                                state.active = None;
                                cx.notify();
                                had
                            });
                            if had {
                                select(None, window, cx);
                            }
                        }
                        key => {
                            let Some(next) = step_cell(shown, total, columns, key) else {
                                return;
                            };
                            // A kept-open task moves along with the arrows.
                            let moved_pin = state.update(cx, |state, cx| {
                                state.active = Some(next);
                                cx.notify();
                                state
                                    .pinned
                                    .is_some()
                                    .then(|| state.pinned = Some(next))
                                    .is_some()
                            });
                            if moved_pin {
                                select(Some(next), window, cx);
                            }
                        }
                    }
                    cx.stop_propagation();
                }
            })
            .flex()
            .flex_col()
            .gap(px(3.))
            .p_1()
            .rounded(radius)
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .children(
                chunked(cells, columns)
                    .into_iter()
                    .map(|row| h_flex().gap(px(3.)).children(row)),
            );

        let filters =
            h_flex()
                .flex_wrap()
                .gap_1()
                .children(
                    CellStatus::ALL
                        .iter()
                        .enumerate()
                        .filter_map(|(ix, status)| {
                            let count = counts[ix];
                            if count == 0 && only != Some(*status) {
                                return None;
                            }
                            let status = *status;
                            let is_on = only == Some(status);
                            let state = state.clone();
                            Some(
                                h_flex()
                                    .id(("filter", ix))
                                    .gap_1p5()
                                    .h(px(24.))
                                    .px_1p5()
                                    .rounded(radius)
                                    .cursor_pointer()
                                    .text_caption()
                                    .hover(|this| this.bg(chip))
                                    .when(is_on, |this| this.bg(chip).font_medium())
                                    .on_click(move |_, _, cx| {
                                        state.update(cx, |state, cx| {
                                            state.only =
                                                (state.only != Some(status)).then_some(status);
                                            cx.notify();
                                        })
                                    })
                                    .child(div().size_2().rounded(px(2.)).bg(tone(status)))
                                    .child(
                                        div()
                                            .font_medium()
                                            .text_color(ink)
                                            .child(count.to_string()),
                                    )
                                    .child(div().text_color(quiet).child(status.words())),
                            )
                        }),
                );
        let retry = self
            .on_retry
            .clone()
            .filter(|_| !failed_ids.is_empty())
            .map(|handler| {
                Button::new((self.id.clone(), "retry"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::RotateCw)
                    .label(format!("Retry {failed} failed"))
                    .on_click(move |_, window, cx| handler(&failed_ids, window, cx))
            });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_2()
            .text_body()
            .line_height(relative(1.45))
            .text_color(ink)
            .child(
                h_flex()
                    .gap_3()
                    .child(Icon::new(IconName::Network).small().text_color(quiet))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(div().flex_none().font_medium().child(self.title))
                            .children(self.job.map(|job| {
                                div().min_w_0().truncate().text_color(quiet).child(job)
                            })),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .child(div().font_medium().child(done.to_string()))
                            .child(div().text_color(quiet).child(format!("of {total}"))),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_medium()
                            .child((self.format_cost)(spent)),
                    )
                    .children(left.filter(|_| done + failed < total).map(|left| {
                        div()
                            .flex_none()
                            .text_color(quiet)
                            .child(format!("about {} left", duration_words(left)))
                    })),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .rounded(radius_lg)
                    .bg(fill)
                    .shadow(resting_shadow(cx))
                    .child(grid)
                    .children(details)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().min_w_0().child(filters))
                            .children(retry),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: CellStatus, span: Option<(u64, u64)>) -> GridTask {
        let task = GridTask::new("t", "Task").status(status);
        match span {
            Some((start, end)) => task.span(
                Duration::from_millis(start),
                Some(Duration::from_millis(end)),
            ),
            None => task,
        }
    }

    #[test]
    fn counts_follow_the_filter_order() {
        let tasks = [
            task(CellStatus::Done, None),
            task(CellStatus::Done, None),
            task(CellStatus::Failed, None),
            task(CellStatus::Queued, None),
        ];
        assert_eq!(counts(&tasks), [2, 0, 0, 1, 1]);
    }

    #[test]
    fn the_time_left_needs_three_finished_tasks() {
        let mut tasks = vec![
            task(CellStatus::Done, Some((0, 1000))),
            task(CellStatus::Done, Some((0, 3000))),
            task(CellStatus::Running, None),
            task(CellStatus::Queued, None),
        ];
        assert_eq!(time_left(&tasks, Some(2)), None);
        tasks.push(task(CellStatus::Done, Some((1000, 3000))));
        // Mean 2 s, two open tasks on two workers: one round.
        assert_eq!(time_left(&tasks, Some(2)), Some(Duration::from_secs(2)));
        // One worker: two rounds.
        assert_eq!(time_left(&tasks, Some(1)), Some(Duration::from_secs(4)));
    }

    #[test]
    fn nothing_is_left_once_every_task_is_over() {
        let tasks = [
            task(CellStatus::Done, Some((0, 1000))),
            task(CellStatus::Done, Some((0, 1000))),
            task(CellStatus::Done, Some((0, 1000))),
            task(CellStatus::Failed, None),
        ];
        assert_eq!(time_left(&tasks, None), Some(Duration::ZERO));
    }

    #[test]
    fn arrows_walk_the_cells() {
        assert_eq!(step_cell(None, 50, 10, "right"), Some(0));
        assert_eq!(step_cell(None, 50, 10, "end"), Some(49));
        assert_eq!(step_cell(Some(3), 50, 10, "down"), Some(13));
        assert_eq!(step_cell(Some(45), 50, 10, "down"), Some(45));
        assert_eq!(step_cell(Some(5), 50, 10, "up"), Some(5));
        assert_eq!(step_cell(Some(49), 50, 10, "right"), Some(49));
        assert_eq!(step_cell(Some(3), 50, 10, "x"), None);
        assert_eq!(step_cell(None, 0, 10, "right"), None);
    }

    #[test]
    fn cells_fill_rows_of_the_given_width() {
        let rows = chunked((0..7).collect(), 3);
        assert_eq!(rows, [vec![0, 1, 2], vec![3, 4, 5], vec![6]]);
        assert!(chunked(Vec::<u8>::new(), 3).is_empty());
    }

    #[test]
    fn durations_read_in_the_right_unit() {
        assert_eq!(duration_words(Duration::from_millis(850)), "850 ms");
        assert_eq!(duration_words(Duration::from_millis(7100)), "7.1 s");
        assert_eq!(duration_words(Duration::from_secs(200)), "3 m 20 s");
    }
}
