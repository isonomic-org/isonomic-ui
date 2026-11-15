//! Checkpoint timeline: an agent's run as a line of saved points.
//!
//! People can rewind to any checkpoint. The agent then tries again from
//! there, on a new lane that bends out of the checkpoint they picked, while
//! the attempt it left stays in the list, faded. Nothing is thrown away.

use std::{collections::HashMap, rc::Rc, time::Duration};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, button::Button, h_flex, v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, PathBuilder, Pixels, RenderOnce, SharedString,
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
        }
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
        }
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

#[derive(Default)]
struct TimelineUi {
    hovered: Option<usize>,
    selected: Option<usize>,
    focus: Option<FocusHandle>,
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
            focus: Some(cx.focus_handle()),
            ..Default::default()
        });
        let (hovered, selected, focus) = {
            let ui = ui.read(cx);
            (ui.hovered, ui.selected, ui.focus.clone())
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
                Button::new(("checkpoint-rewind", ix))
                    .outline()
                    .xsmall()
                    .icon(IconName::Undo2)
                    .label("Rewind")
                    .on_click(move |_, window, cx| rewind_at(ix, window, cx, &handler, &points))
                    .into_any_element()
            } else {
                h_flex()
                    .gap_3()
                    .text_xs()
                    .text_color(quiet)
                    .children(checkpoint.files.map(|files| {
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
                .id(("checkpoint", ix))
                .h(px(ROW))
                .gap_2()
                .pl(px(gutter + 4.))
                .pr_2()
                .rounded(radius)
                .when(
                    hovered == Some(ix) || (keyboard && selected == Some(ix)),
                    |this| this.bg(chip),
                )
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
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if count == 0 {
                    return;
                }
                let at = ui.read(cx).selected.or(head_ix).unwrap_or(0);
                let next = match event.keystroke.key.as_str() {
                    "up" => Some(at.saturating_sub(1)),
                    "down" => Some((at + 1).min(count - 1)),
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

        let no_task = self.task.is_none();
        v_flex()
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(key_down)
            .w_full()
            .max_w(px(480.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill_color)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.3))
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .child(Icon::new(IconName::Redo2).small().text_color(quiet))
                    .child(div().flex_none().font_semibold().child(self.title))
                    .children(self.task.map(|task| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(quiet)
                            .child(task)
                    }))
                    .when(no_task, |this| this.child(div().flex_1()))
                    .child(div().flex_none().text_xs().text_color(quiet).child(summary)),
            )
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
        .text_xs()
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
    fn run_times_read_as_a_clock() {
        assert_eq!(run_clock(Duration::from_secs(47)), "0:47");
        assert_eq!(run_clock(Duration::from_secs(725)), "12:05");
    }
}
