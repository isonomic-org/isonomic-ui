// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checkpoint timeline: an agent's run as a line of saved points.
//!
//! People can rewind to any checkpoint. The agent then tries again from
//! there, on a new lane that bends out of the checkpoint they picked, while
//! the attempt it left stays in the list, faded. Nothing is thrown away.
//!
//! Opt in to a scrubber that runs through the checkpoints turn by turn, and
//! to a panel that says what changed at the one picked: the files, lines
//! added and removed, and a button that restores the run to it. Left and
//! Right scrub, Up and Down move, Enter restores.

use std::{cell::Cell, collections::HashMap, rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, look, v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, fill, point,
    prelude::FluentBuilder as _, px, relative, size,
};

use crate::common::resting_shadow;

/// The height of every row, so the lanes can be drawn beside them.
const ROW: f32 = 30.;
/// The room each lane takes in the gutter.
const LANE: f32 = 12.;

/// Where a checkpoint stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointStatus {
    /// The agent is working toward it.
    Running,
    /// Saved.
    Done,
    /// The work toward it failed.
    Failed,
    /// People rewound away while the agent worked toward it.
    Stopped,
}

/// The height of the scrubber's track area.
const SCRUB: f32 = 28.;

/// One file a checkpoint changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// The file's path.
    pub path: SharedString,
    /// Lines added.
    pub added: usize,
    /// Lines removed.
    pub removed: usize,
}

/// One saved point of the run.
#[derive(Clone)]
pub struct Checkpoint {
    id: SharedString,
    title: SharedString,
    icon: Option<IconName>,
    parent: Option<SharedString>,
    status: CheckpointStatus,
    at: Option<Duration>,
    files: Option<usize>,
    cost: Option<f64>,
    turn: Option<usize>,
    summary: Option<SharedString>,
    changes: Vec<FileChange>,
}

impl Checkpoint {
    /// Create a saved checkpoint.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            parent: None,
            status: CheckpointStatus::Done,
            at: None,
            files: None,
            cost: None,
            turn: None,
            summary: None,
            changes: Vec::new(),
        }
    }

    /// Say which turn of the conversation it was taken in, counted from 1.
    pub fn turn(mut self, turn: usize) -> Self {
        self.turn = Some(turn);
        self
    }

    /// Say in a sentence what the work toward it did.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Add a file it changed, with the lines added and removed. Without
    /// [`Self::files`] the count of changed files comes from these.
    pub fn change(mut self, path: impl Into<SharedString>, added: usize, removed: usize) -> Self {
        self.changes.push(FileChange {
            path: path.into(),
            added,
            removed,
        });
        self
    }

    /// The files it changed.
    pub fn changes(&self) -> &[FileChange] {
        &self.changes
    }

    /// How many files it changed: as told, or as listed.
    fn file_count(&self) -> Option<usize> {
        self.files
            .or_else(|| (!self.changes.is_empty()).then_some(self.changes.len()))
    }

    /// Say which checkpoint this one follows. A checkpoint whose parent
    /// already has a later one starts a new attempt on its own lane.
    pub fn after(mut self, parent: impl Into<SharedString>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Set where it stands.
    pub fn status(mut self, status: CheckpointStatus) -> Self {
        self.status = status;
        self
    }

    /// Show a small mark before the title, such as the tool it worked in.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Say how far into the run it was taken.
    pub fn at(mut self, at: Duration) -> Self {
        self.at = Some(at);
        self
    }

    /// Say how many files it changed.
    pub fn files(mut self, files: usize) -> Self {
        self.files = Some(files);
        self
    }

    /// Say what the work toward it cost, added up in the header.
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }

    /// The checkpoint's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The checkpoint's title.
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Where it stands.
    pub fn current_status(&self) -> CheckpointStatus {
        self.status
    }
}

type RewindHandler = Rc<dyn Fn(&Checkpoint, &mut Window, &mut App)>;
type CostFormat = Rc<dyn Fn(f64) -> String>;

/// A run's checkpoints, with lanes for every attempt and rewind on each.
#[derive(IntoElement)]
pub struct CheckpointTimeline {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    task: Option<SharedString>,
    checkpoints: Vec<Checkpoint>,
    head: Option<SharedString>,
    on_rewind: Option<RewindHandler>,
    format_cost: Option<CostFormat>,
    scrubber: bool,
    show_changes: bool,
}

impl CheckpointTimeline {
    /// Create an empty timeline.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Checkpoints".into(),
            task: None,
            checkpoints: Vec::new(),
            head: None,
            on_rewind: None,
            format_cost: None,
            scrubber: false,
            show_changes: false,
        }
    }

    /// Show a scrubber above the list: one tick per checkpoint, in the order
    /// taken. Dragging along it, or Left and Right, picks a checkpoint.
    pub fn scrubber(mut self, scrubber: bool) -> Self {
        self.scrubber = scrubber;
        self
    }

    /// Show what changed at the picked checkpoint below the list, with a
    /// button that restores the run to it.
    pub fn show_changes(mut self, show_changes: bool) -> Self {
        self.show_changes = show_changes;
        self
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Say what the run is for, next to the heading.
    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Add a checkpoint, in the order they were taken.
    pub fn checkpoint(mut self, checkpoint: Checkpoint) -> Self {
        self.checkpoints.push(checkpoint);
        self
    }

    /// Say which checkpoint the agent stands on. The last one otherwise.
    pub fn head(mut self, id: impl Into<SharedString>) -> Self {
        self.head = Some(id.into());
        self
    }

    /// Run when people rewind. Add a checkpoint with this one as its parent,
    /// and make it the head.
    pub fn on_rewind(
        mut self,
        handler: impl Fn(&Checkpoint, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_rewind = Some(Rc::new(handler));
        self
    }

    /// Write the total cost your way. Dollars with cents otherwise.
    pub fn format_cost(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format_cost = Some(Rc::new(format));
        self
    }
}

impl Styled for CheckpointTimeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The lane of each checkpoint. The first child keeps its parent's lane; a
/// later child of the same parent opens the next free lane.
fn lanes(checkpoints: &[Checkpoint]) -> Vec<usize> {
    let mut lane_of: HashMap<&str, usize> = HashMap::new();
    let mut has_child: HashMap<&str, bool> = HashMap::new();
    let mut next = 0;
    checkpoints
        .iter()
        .map(|checkpoint| {
            let lane = match checkpoint
                .parent
                .as_deref()
                .and_then(|parent| lane_of.get(parent).map(|lane| (parent, *lane)))
            {
                Some((parent, lane)) if !has_child.get(parent).copied().unwrap_or(false) => {
                    has_child.insert(parent, true);
                    lane
                }
                Some((parent, _)) => {
                    next += 1;
                    has_child.insert(parent, true);
                    next
                }
                None if lane_of.is_empty() => 0,
                None => {
                    next += 1;
                    next
                }
            };
            lane_of.insert(checkpoint.id.as_ref(), lane);
            lane
        })
        .collect()
}

/// Which checkpoints lie on the way to the head.
fn on_way(checkpoints: &[Checkpoint], head: &str) -> Vec<bool> {
    let by_id: HashMap<&str, &Checkpoint> = checkpoints
        .iter()
        .map(|checkpoint| (checkpoint.id.as_ref(), checkpoint))
        .collect();
    let mut way = std::collections::HashSet::new();
    let mut at = by_id.get(head).copied();
    while let Some(checkpoint) = at {
        if !way.insert(checkpoint.id.as_ref()) {
            break;
        }
        at = checkpoint
            .parent
            .as_deref()
            .and_then(|parent| by_id.get(parent).copied());
    }
    checkpoints
        .iter()
        .map(|checkpoint| way.contains(checkpoint.id.as_ref()))
        .collect()
}

/// A time into the run: "0:47", "12:05".
fn run_clock(at: Duration) -> String {
    let secs = at.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Where the scrubber's tick for checkpoint `ix` sits, from 0 to `width`.
/// A lone checkpoint sits in the middle.
fn tick_x(ix: usize, count: usize, width: f32) -> f32 {
    if count <= 1 {
        width / 2.
    } else {
        width * ix as f32 / (count - 1) as f32
    }
}

/// The checkpoint nearest to `x` along a scrubber `width` wide.
fn scrub_index(x: f32, width: f32, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    if count == 1 || width <= 0. {
        return Some(0);
    }
    let step = width / (count - 1) as f32;
    Some(((x / step).round().max(0.) as usize).min(count - 1))
}

/// What changed, in words: "3 files, 42 added, 7 removed".
fn change_words(changes: &[FileChange], files: Option<usize>) -> String {
    let files = files.unwrap_or(changes.len());
    let mut words = if files == 1 {
        "1 file".to_string()
    } else {
        format!("{files} files")
    };
    if !changes.is_empty() {
        let added: usize = changes.iter().map(|change| change.added).sum();
        let removed: usize = changes.iter().map(|change| change.removed).sum();
        words.push_str(&format!(", {added} added, {removed} removed"));
    }
    words
}

/// The scrubber's caption: "Turn 4 · 3 of 9".
fn scrub_caption(turn: Option<usize>, ix: usize, count: usize) -> String {
    match turn {
        Some(turn) => format!("Turn {turn} \u{b7} {} of {count}", ix + 1),
        None => format!("{} of {count}", ix + 1),
    }
}

#[derive(Default)]
struct TimelineUi {
    hovered: Option<usize>,
    selected: Option<usize>,
    focus: Option<FocusHandle>,
    /// Where the scrubber's track was laid out, for turning a pointer into a tick.
    scrub_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Whether the pointer is dragging along the scrubber.
    dragging: bool,
}

impl RenderOnce for CheckpointTimeline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill_color = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let ui = window.use_keyed_state(self.id.clone(), cx, |_, cx| TimelineUi {
            focus: Some(cx.focus_handle().tab_stop(true)),
            ..Default::default()
        });
        let (hovered, selected, focus, scrub_bounds) = {
            let ui = ui.read(cx);
            (
                ui.hovered,
                ui.selected,
                ui.focus.clone(),
                ui.scrub_bounds.clone(),
            )
        };
        let focus = focus.unwrap_or_else(|| cx.focus_handle());
        let keyboard = focus.is_focused(window);

        let points = Rc::new(self.checkpoints);
        let count = points.len();
        let head = self
            .head
            .clone()
            .or_else(|| points.last().map(|checkpoint| checkpoint.id.clone()));
        let lane = lanes(&points);
        let way = head
            .as_deref()
            .map(|head| on_way(&points, head))
            .unwrap_or_else(|| vec![true; count]);
        let attempts = lane.iter().max().map(|max| max + 1).unwrap_or(0);
        let cost: f64 = points.iter().filter_map(|checkpoint| checkpoint.cost).sum();
        let format_cost = self.format_cost.clone();
        let cost_words = match &format_cost {
            Some(format) => format(cost),
            None => format!("${cost:.2}"),
        };
        let gutter = LANE * attempts.max(1) as f32 + 6.;
        let index: HashMap<SharedString, usize> = points
            .iter()
            .enumerate()
            .map(|(ix, checkpoint)| (checkpoint.id.clone(), ix))
            .collect();

        // The lanes, drawn beside the rows.
        let rails = {
            let points = points.clone();
            let lane = lane.clone();
            let way = way.clone();
            let index = index.clone();
            let head = head.clone();
            canvas(
                |_, _, _| {},
                move |bounds: Bounds<Pixels>, _, window, _| {
                    let x = |lane: usize| bounds.left() + px(LANE * lane as f32 + LANE / 2. + 1.);
                    let y = |row: usize| bounds.top() + px(ROW * row as f32 + ROW / 2.);
                    for (ix, checkpoint) in points.iter().enumerate() {
                        let Some(from) = checkpoint
                            .parent
                            .as_ref()
                            .and_then(|parent| index.get(parent).copied())
                        else {
                            continue;
                        };
                        let color = if way[ix] { accent } else { quiet.opacity(0.35) };
                        let mut path = PathBuilder::stroke(px(1.5));
                        path.move_to(point(x(lane[from]), y(from)));
                        if lane[from] != lane[ix] {
                            // Bend out of the parent just below it, then run down.
                            let bend = y(from) + px(ROW * 0.45);
                            path.line_to(point(x(lane[from]), bend));
                            path.line_to(point(x(lane[ix]), bend + px(ROW * 0.4)));
                        }
                        path.line_to(point(x(lane[ix]), y(ix)));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                    for (ix, checkpoint) in points.iter().enumerate() {
                        let center = point(x(lane[ix]), y(ix));
                        let is_head = head.as_ref() == Some(&checkpoint.id);
                        let dot = if is_head { 9. } else { 7. };
                        let color = match (way[ix], checkpoint.status) {
                            (_, CheckpointStatus::Failed) => danger,
                            (true, _) => accent,
                            (false, _) => quiet.opacity(0.5),
                        };
                        if way[ix] || checkpoint.status == CheckpointStatus::Failed {
                            window.paint_quad(
                                fill(Bounds::centered_at(center, size(px(dot), px(dot))), color)
                                    .corner_radii(px(dot / 2.)),
                            );
                        } else {
                            // Left behind: a hollow ring.
                            window.paint_quad(
                                fill(Bounds::centered_at(center, size(px(dot), px(dot))), color)
                                    .corner_radii(px(dot / 2.)),
                            );
                            window.paint_quad(
                                fill(
                                    Bounds::centered_at(center, size(px(dot - 3.), px(dot - 3.))),
                                    fill_color,
                                )
                                .corner_radii(px((dot - 3.) / 2.)),
                            );
                        }
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .w(px(gutter))
            .h(px(ROW * count as f32))
        };

        let rewind_at = |ix: usize,
                         window: &mut Window,
                         cx: &mut App,
                         handler: &Option<RewindHandler>,
                         points: &[Checkpoint]| {
            if let (Some(handler), Some(checkpoint)) = (handler, points.get(ix)) {
                handler(checkpoint, window, cx);
            }
        };

        // A checkpoint in words: "Wrote the tests, failed, attempt 2, 3 files, at 1:20".
        let point_words = |ix: usize| -> String {
            let Some(checkpoint) = points.get(ix) else {
                return String::new();
            };
            let mut words = checkpoint.title.to_string();
            words.push_str(match checkpoint.status {
                CheckpointStatus::Running => ", in progress",
                CheckpointStatus::Done => "",
                CheckpointStatus::Failed => ", failed",
                CheckpointStatus::Stopped => ", stopped",
            });
            if lane[ix] > 0 {
                words.push_str(&format!(", attempt {}", lane[ix] + 1));
            }
            if let Some(turn) = checkpoint.turn {
                words.push_str(&format!(", turn {turn}"));
            }
            if let Some(files) = checkpoint.file_count() {
                words.push_str(&if files == 1 {
                    ", 1 file".to_string()
                } else {
                    format!(", {files} files")
                });
            }
            if let Some(at) = checkpoint.at {
                words.push_str(&format!(", at {}", run_clock(at)));
            }
            if head.as_ref() == Some(&checkpoint.id) {
                words.push_str(", current");
            } else if !way[ix] {
                words.push_str(", left behind");
            }
            words
        };
        let row_hover: Vec<f32> = (0..count)
            .map(|ix| {
                look::hover_fade(
                    ElementId::NamedChild(self.id.clone().into(), format!("row-hover-{ix}").into()),
                    hovered == Some(ix) || (keyboard && selected == Some(ix)),
                    window,
                    cx,
                )
            })
            .collect();
        let rows = points.iter().enumerate().map(|(ix, checkpoint)| {
            let is_head = head.as_ref() == Some(&checkpoint.id);
            let left_behind = !way[ix];
            let shows_rewind = self.on_rewind.is_some()
                && !is_head
                && (hovered == Some(ix) || (keyboard && selected == Some(ix)));
            let starts_attempt = lane[ix] > 0
                && (ix == 0
                    || checkpoint
                        .parent
                        .as_ref()
                        .and_then(|parent| index.get(parent))
                        .is_some_and(|parent| lane[*parent] != lane[ix]));
            let hover = {
                let ui = ui.clone();
                move |hovered: &bool, _: &mut Window, cx: &mut App| {
                    ui.update(cx, |ui, cx| {
                        if *hovered {
                            ui.hovered = Some(ix);
                        } else if ui.hovered == Some(ix) {
                            ui.hovered = None;
                        }
                        cx.notify();
                    })
                }
            };
            let right = if shows_rewind {
                let handler = self.on_rewind.clone();
                let points = points.clone();
                Button::new(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("rewind-{ix}").into(),
                ))
                .outline()
                .xsmall()
                .icon(IconName::Undo2)
                .label("Rewind")
                .on_click(move |_, window, cx| rewind_at(ix, window, cx, &handler, &points))
                .into_any_element()
            } else {
                h_flex()
                    .gap_3()
                    .text_caption()
                    .text_color(quiet)
                    .children(checkpoint.file_count().map(|files| {
                        h_flex()
                            .gap_1()
                            .child(Icon::new(IconName::Folder).xsmall())
                            .child(if files == 1 {
                                "1 file".to_string()
                            } else {
                                format!("{files} files")
                            })
                    }))
                    .children(
                        checkpoint
                            .at
                            .map(|at| div().w(px(34.)).text_right().child(run_clock(at))),
                    )
                    .into_any_element()
            };
            h_flex()
                .id(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("checkpoint-{ix}").into(),
                ))
                .role(Role::ListItem)
                .aria_label(point_words(ix))
                .aria_selected(selected == Some(ix))
                .h(px(ROW))
                .gap_2()
                .pl(px(gutter + 4.))
                .pr_2()
                .rounded(radius)
                .bg(look::mix(chip.opacity(0.), chip, row_hover[ix]))
                .on_hover(hover)
                .on_click({
                    let ui = ui.clone();
                    let focus = focus.clone();
                    move |_, window, cx| {
                        focus.focus(window, cx);
                        ui.update(cx, |ui, cx| {
                            ui.selected = Some(ix);
                            cx.notify();
                        })
                    }
                })
                .when(left_behind, |this| this.opacity(0.55))
                .children(checkpoint.icon.clone().map(|icon| {
                    div()
                        .flex_none()
                        .size(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(line)
                        .child(Icon::new(icon).xsmall())
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(checkpoint.status == CheckpointStatus::Running, |this| {
                            this.text_color(accent)
                        })
                        .when(left_behind, |this| this.text_color(quiet))
                        .when(is_head, |this| this.font_medium())
                        .child(checkpoint.title.clone()),
                )
                .when(starts_attempt, |this| {
                    this.child(small_tag(format!("Attempt {}", lane[ix] + 1), chip, quiet))
                })
                .when(checkpoint.status == CheckpointStatus::Failed, |this| {
                    this.child(small_tag(
                        "Failed".to_string(),
                        danger.opacity(0.14),
                        danger,
                    ))
                })
                .when(checkpoint.status == CheckpointStatus::Stopped, |this| {
                    this.child(small_tag("Stopped".to_string(), chip, quiet))
                })
                .child(div().flex_none().child(right))
        });
        let rows: Vec<_> = rows.collect();

        let key_down = {
            let ui = ui.clone();
            let handler = self.on_rewind.clone();
            let points = points.clone();
            let head_ix = head.as_ref().and_then(|head| index.get(head).copied());
            let scrubs = self.scrubber;
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if count == 0 {
                    return;
                }
                let at = ui.read(cx).selected.or(head_ix).unwrap_or(0);
                let next = match event.keystroke.key.as_str() {
                    "up" => Some(at.saturating_sub(1)),
                    "down" => Some((at + 1).min(count - 1)),
                    "left" if scrubs => Some(at.saturating_sub(1)),
                    "right" if scrubs => Some((at + 1).min(count - 1)),
                    "home" => Some(0),
                    "end" => Some(count - 1),
                    "enter" | "space" => {
                        if Some(at) != head_ix {
                            rewind_at(at, window, cx, &handler, &points);
                        }
                        cx.stop_propagation();
                        None
                    }
                    _ => return,
                };
                if let Some(next) = next {
                    ui.update(cx, |ui, cx| {
                        ui.selected = Some(next);
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            }
        };

        let summary = [
            Some(if count == 1 {
                "1 checkpoint".to_string()
            } else {
                format!("{count} checkpoints")
            }),
            (attempts > 1).then(|| format!("{attempts} attempts")),
            (cost > 0.).then_some(cost_words),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");

        let at_words = selected
            .or_else(|| head.as_ref().and_then(|head| index.get(head).copied()))
            .map(|ix| format!(". {}", point_words(ix)))
            .unwrap_or_default();
        let timeline_words = format!(
            "Checkpoints, {summary}{at_words}. {}{}",
            if self.scrubber {
                "Left and Right scrub, Up and Down move"
            } else {
                "Up and Down move"
            },
            if self.on_rewind.is_some() {
                ", Enter rewinds"
            } else {
                ""
            }
        );

        // The checkpoint the scrubber and the changes panel are about.
        let head_ix = head.as_ref().and_then(|head| index.get(head).copied());
        let picked = selected.or(head_ix).filter(|ix| *ix < count);

        let scrubber = (self.scrubber && count > 0).then(|| {
            let ticks = {
                let points = points.clone();
                let way = way.clone();
                let bounds_cell = scrub_bounds.clone();
                canvas(
                    move |bounds, _, _| bounds_cell.set(bounds),
                    move |bounds: Bounds<Pixels>, _, window, _| {
                        // Inset so the end ticks' knobs are not clipped.
                        let inset = 8.;
                        let width = (bounds.size.width.as_f32() - inset * 2.).max(0.);
                        let mid = bounds.top() + px(SCRUB / 2.);
                        let x = |ix: usize| bounds.left() + px(inset + tick_x(ix, count, width));
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(bounds.left() + px(inset), mid - px(1.)),
                                    size(px(width), px(2.)),
                                ),
                                chip,
                            )
                            .corner_radii(px(1.)),
                        );
                        if let Some(at) = picked {
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        point(bounds.left() + px(inset), mid - px(1.)),
                                        size(x(at) - bounds.left() - px(inset), px(2.)),
                                    ),
                                    accent.opacity(0.6),
                                )
                                .corner_radii(px(1.)),
                            );
                        }
                        for (ix, checkpoint) in points.iter().enumerate() {
                            let color = match (checkpoint.status, way[ix]) {
                                (CheckpointStatus::Failed, _) => danger,
                                (_, true) => accent,
                                (_, false) => quiet.opacity(0.5),
                            };
                            let dot = if picked == Some(ix) { 12. } else { 6. };
                            let center = point(x(ix), mid);
                            if picked == Some(ix) {
                                window.paint_quad(
                                    fill(
                                        Bounds::centered_at(center, size(px(dot), px(dot))),
                                        color,
                                    )
                                    .corner_radii(px(dot / 2.)),
                                );
                                window.paint_quad(
                                    fill(
                                        Bounds::centered_at(center, size(px(6.), px(6.))),
                                        fill_color,
                                    )
                                    .corner_radii(px(3.)),
                                );
                            } else {
                                window.paint_quad(
                                    fill(
                                        Bounds::centered_at(center, size(px(dot), px(dot))),
                                        color,
                                    )
                                    .corner_radii(px(dot / 2.)),
                                );
                            }
                        }
                    },
                )
                .absolute()
                .inset_0()
            };
            let pick_at = {
                let ui = ui.clone();
                let bounds_cell = scrub_bounds.clone();
                move |x: Pixels, cx: &mut App| {
                    let bounds = bounds_cell.get();
                    let inset = 8.;
                    let width = bounds.size.width.as_f32() - inset * 2.;
                    let at = (x - bounds.left()).as_f32() - inset;
                    if let Some(ix) = scrub_index(at, width, count) {
                        ui.update(cx, |ui, cx| {
                            if ui.selected != Some(ix) {
                                ui.selected = Some(ix);
                                cx.notify();
                            }
                        });
                    }
                }
            };
            let caption = picked
                .and_then(|ix| points.get(ix).map(|checkpoint| (ix, checkpoint)))
                .map(|(ix, checkpoint)| {
                    (
                        scrub_caption(checkpoint.turn, ix, count),
                        checkpoint.title.clone(),
                    )
                });
            v_flex()
                .px_3()
                .pb_1()
                .child(
                    div()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            "scrubber".into(),
                        ))
                        .role(Role::Slider)
                        .aria_label(match &caption {
                            Some((where_, title)) => {
                                format!("Scrub through checkpoints, {where_}, {title}")
                            }
                            None => "Scrub through checkpoints".to_string(),
                        })
                        .relative()
                        .h(px(SCRUB))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, {
                            let ui = ui.clone();
                            let focus = focus.clone();
                            let pick_at = pick_at.clone();
                            move |event, window, cx| {
                                focus.focus(window, cx);
                                ui.update(cx, |ui, _| ui.dragging = true);
                                pick_at(event.position.x, cx);
                            }
                        })
                        .on_mouse_move({
                            let ui = ui.clone();
                            let pick_at = pick_at.clone();
                            move |event, _, cx| {
                                if ui.read(cx).dragging && event.dragging() {
                                    pick_at(event.position.x, cx);
                                }
                            }
                        })
                        .on_mouse_up(MouseButton::Left, {
                            let ui = ui.clone();
                            move |_, _, cx| ui.update(cx, |ui, _| ui.dragging = false)
                        })
                        .on_mouse_up_out(MouseButton::Left, {
                            let ui = ui.clone();
                            move |_, _, cx| ui.update(cx, |ui, _| ui.dragging = false)
                        })
                        .child(ticks),
                )
                .children(caption.map(|(where_, title)| {
                    h_flex()
                        .gap_2()
                        .text_caption()
                        .child(div().flex_none().text_color(quiet).child(where_))
                        .child(div().flex_1().min_w_0().truncate().child(title))
                }))
        });

        let changes = picked
            .filter(|_| self.show_changes)
            .and_then(|ix| points.get(ix).map(|checkpoint| (ix, checkpoint.clone())))
            .map(|(ix, checkpoint)| {
                let is_head = head_ix == Some(ix);
                let success = cx.theme().success;
                let where_ = [
                    checkpoint.turn.map(|turn| format!("Turn {turn}")),
                    checkpoint.at.map(|at| format!("at {}", run_clock(at))),
                    (lane[ix] > 0).then(|| format!("attempt {}", lane[ix] + 1)),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" \u{b7} ");
                let restore = (!is_head).then(|| {
                    let handler = self.on_rewind.clone();
                    let points = points.clone();
                    Button::new(ElementId::NamedChild(
                        self.id.clone().into(),
                        "restore".into(),
                    ))
                    .small()
                    .primary()
                    .icon(IconName::Undo2)
                    .label("Restore to here")
                    .accessibility_label(format!("Restore the run to {}", checkpoint.title))
                    .disabled(handler.is_none())
                    .on_click(move |_, window, cx| rewind_at(ix, window, cx, &handler, &points))
                });
                v_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "changes".into(),
                    ))
                    .role(Role::Region)
                    .aria_label(format!(
                        "What changed at {}: {}",
                        checkpoint.title,
                        change_words(&checkpoint.changes, checkpoint.file_count())
                    ))
                    .gap_2()
                    .mx_2()
                    .mb_2()
                    .p_3()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_medium()
                                            .child(checkpoint.title.clone()),
                                    )
                                    .when(!where_.is_empty(), |this| {
                                        this.child(
                                            div().text_caption().text_color(quiet).child(where_),
                                        )
                                    }),
                            )
                            .map(|this| match restore {
                                Some(restore) => this.child(restore),
                                None => this.child(small_tag("Current".to_string(), chip, quiet)),
                            }),
                    )
                    .children(
                        checkpoint
                            .summary
                            .clone()
                            .map(|summary| div().text_color(quiet).child(summary)),
                    )
                    .child(div().text_eyebrow().text_color(quiet).child(format!(
                        "WHAT CHANGED \u{b7} {}",
                        change_words(&checkpoint.changes, checkpoint.file_count()).to_uppercase()
                    )))
                    .when(checkpoint.changes.is_empty(), |this| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child("No file list for this checkpoint."),
                        )
                    })
                    .children(checkpoint.changes.iter().map(|change| {
                        h_flex()
                            .gap_3()
                            .text_caption()
                            .child(Icon::new(IconName::FileText).xsmall().text_color(quiet))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child(change.path.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(success)
                                    .child(format!("+{}", change.added)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(36.))
                                    .text_right()
                                    .text_color(danger)
                                    .child(format!("\u{2212}{}", change.removed)),
                            )
                    }))
            });
        let no_task = self.task.is_none();
        v_flex()
            .id(self.id.clone())
            .role(Role::List)
            .aria_label(timeline_words)
            .track_focus(&focus)
            .on_key_down(key_down)
            .w_full()
            .max_w(px(480.))
            .rounded(radius_lg)
            .bg(fill_color)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.3))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .child(Icon::new(IconName::Redo2).small().text_color(quiet))
                    .child(div().flex_none().font_medium().child(self.title))
                    .children(self.task.map(|task| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(quiet)
                            .child(task)
                    }))
                    .when(no_task, |this| this.child(div().flex_1()))
                    .child(
                        div()
                            .flex_none()
                            .text_caption()
                            .text_color(quiet)
                            .child(summary),
                    ),
            )
            .children(scrubber)
            .child(
                div()
                    .relative()
                    .mx_1()
                    .mb_2()
                    .child(v_flex().children(rows))
                    // On top, so a highlighted row does not cover the lanes.
                    .child(rails)
                    .when(count == 0, |this| {
                        this.child(
                            div()
                                .px_2()
                                .py_2()
                                .text_color(quiet)
                                .child("No checkpoints yet"),
                        )
                    }),
            )
            .children(changes)
            .refine_style(&self.style)
    }
}

/// A small label after the title.
fn small_tag(label: String, bg: Hsla, color: Hsla) -> impl IntoElement {
    div()
        .flex_none()
        .px_1p5()
        .rounded(px(4.))
        .bg(bg)
        .text_caption()
        .text_color(color)
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Vec<Checkpoint> {
        vec![
            Checkpoint::new("a", "A"),
            Checkpoint::new("b", "B").after("a"),
            Checkpoint::new("c", "C").after("b"),
            Checkpoint::new("d", "D").after("c"),
            // Rewound to b: a second attempt.
            Checkpoint::new("e", "E").after("b"),
            Checkpoint::new("f", "F").after("e"),
            // Rewound to a: a third.
            Checkpoint::new("g", "G").after("a"),
        ]
    }

    #[test]
    fn later_children_open_new_lanes() {
        assert_eq!(lanes(&run()), [0, 0, 0, 0, 1, 1, 2]);
    }

    #[test]
    fn the_way_to_the_head_follows_parents() {
        assert_eq!(
            on_way(&run(), "f"),
            [true, true, false, false, true, true, false]
        );
    }

    #[test]
    fn scrubber_ticks_spread_evenly_and_snap() {
        assert_eq!(tick_x(0, 5, 200.), 0.);
        assert_eq!(tick_x(4, 5, 200.), 200.);
        assert_eq!(tick_x(2, 5, 200.), 100.);
        assert_eq!(tick_x(0, 1, 200.), 100.);
        // 200 wide, 5 ticks: one every 50.
        assert_eq!(scrub_index(0., 200., 5), Some(0));
        assert_eq!(scrub_index(74., 200., 5), Some(1));
        assert_eq!(scrub_index(76., 200., 5), Some(2));
        assert_eq!(scrub_index(-30., 200., 5), Some(0));
        assert_eq!(scrub_index(900., 200., 5), Some(4));
        assert_eq!(scrub_index(10., 200., 0), None);
        assert_eq!(scrub_index(10., 200., 1), Some(0));
    }

    #[test]
    fn changes_read_in_words() {
        let changes = Checkpoint::new("a", "A")
            .change("src/a.rs", 30, 2)
            .change("src/b.rs", 12, 5);
        assert_eq!(
            change_words(changes.changes(), changes.file_count()),
            "2 files, 42 added, 7 removed"
        );
        assert_eq!(change_words(&[], Some(1)), "1 file");
        assert_eq!(scrub_caption(Some(4), 2, 9), "Turn 4 \u{b7} 3 of 9");
        assert_eq!(scrub_caption(None, 0, 1), "1 of 1");
    }

    #[test]
    fn run_times_read_as_a_clock() {
        assert_eq!(run_clock(Duration::from_secs(47)), "0:47");
        assert_eq!(run_clock(Duration::from_secs(725)), "12:05");
    }
}
