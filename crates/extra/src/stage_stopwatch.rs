// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Stage stopwatch: a pipeline timed stage by stage on a stopwatch dial.
//!
//! The rim is cut into one sector per stage, each as wide as that stage took
//! last run. While the watch runs, the hand sweeps the current stage's sector
//! and fills it behind itself; a stage that runs past last run's time fills in
//! the warning colour. Finished stages carry a thin outer arc, green when they
//! beat last run and red when they lost time. Below the dial a table lists
//! each stage's split against last run, and the controls start, stop and reset
//! the watch. Under reduced motion the times and fills still change, but the
//! hand rests on the current stage instead of sweeping.

use std::{cell::Cell, f32::consts::TAU, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The dial is laid out on a 240 box and scaled to its width.
const BOX: f32 = 240.;
const CENTRE: f32 = 120.;
/// The band of stage sectors.
const SECTOR_OUTER: f32 = 104.;
const SECTOR_INNER: f32 = 90.;
/// The thin arc outside a finished sector that says whether it gained time.
const DELTA_OUTER: f32 = 111.;
const DELTA_INNER: f32 = 108.;
/// The minute-style ticks inside the band.
const TICK_OUTER: f32 = 84.;
const HAND: f32 = 98.;
const TAIL: f32 = 16.;
/// The space left between two sectors, in turns.
const SECTOR_GAP: f32 = 0.006;
/// The widest the dial is drawn.
const MAX_WIDTH: f32 = 260.;

/// One stage of the pipeline.
#[derive(Clone)]
pub struct StopwatchStage {
    name: SharedString,
    split: Option<Duration>,
    previous: Option<Duration>,
}

impl StopwatchStage {
    /// A stage that has not finished yet.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            split: None,
            previous: None,
        }
    }

    /// How long the stage took this run, once it has finished.
    pub fn split(mut self, split: Duration) -> Self {
        self.split = Some(split);
        self
    }

    /// How long the stage took in the run being compared against.
    pub fn previous(mut self, previous: Duration) -> Self {
        self.previous = Some(previous);
        self
    }
}

/// Where the watch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopwatchStatus {
    /// At zero, not started.
    Ready,
    /// Counting.
    Running,
    /// Stopped part way.
    Paused,
    /// Every stage has a split.
    Finished,
}

impl StopwatchStatus {
    fn word(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Paused => "stopped",
            Self::Finished => "finished",
        }
    }
}

/// A duration as a stopwatch reads it: "0:41.2", or "1:02:05" past an hour.
pub fn format_clock(time: Duration) -> String {
    let tenths = time.as_millis() / 100;
    let seconds = tenths / 10;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}.{}", seconds / 60, seconds % 60, tenths % 10)
    }
}

/// A change against last run in milliseconds, signed: "+2.1s", "−0.8s",
/// "+1:04.0" past a minute, "±0.0s" when even.
pub fn format_delta(ms: i64) -> String {
    let tenths = (ms.unsigned_abs() + 50) / 100;
    let sign = match ms.signum() {
        _ if tenths == 0 => "±",
        1 => "+",
        _ => "\u{2212}",
    };
    if tenths >= 600 {
        let seconds = tenths / 10;
        format!("{sign}{}:{:02}.{}", seconds / 60, seconds % 60, tenths % 10)
    } else {
        format!("{sign}{}.{}s", tenths / 10, tenths % 10)
    }
}

/// A change against last run in words: "2.1 seconds slower than last run".
pub fn spoken_delta(ms: i64) -> String {
    let tenths = (ms.unsigned_abs() + 50) / 100;
    if tenths == 0 {
        return "level with last run".into();
    }
    let amount = format!("{}.{} seconds", tenths / 10, tenths % 10);
    if ms > 0 {
        format!("{amount} slower than last run")
    } else {
        format!("{amount} faster than last run")
    }
}

/// How wide each stage's sector is: last run's time where known, else the
/// average of the known ones, else an equal share.
pub fn reference_weights(stages: &[StopwatchStage]) -> Vec<f32> {
    let known: Vec<f32> = stages
        .iter()
        .filter_map(|stage| stage.previous.map(|time| time.as_secs_f32()))
        .filter(|secs| *secs > 0.)
        .collect();
    let fallback = if known.is_empty() {
        1.
    } else {
        known.iter().sum::<f32>() / known.len() as f32
    };
    stages
        .iter()
        .map(|stage| {
            stage
                .previous
                .map(|time| time.as_secs_f32())
                .filter(|secs| *secs > 0.)
                .unwrap_or(fallback)
        })
        .collect()
}

/// Where each sector starts and ends, in turns clockwise from the top, with a
/// small gap after each.
pub fn sector_spans(weights: &[f32]) -> Vec<(f32, f32)> {
    let total: f32 = weights.iter().map(|weight| weight.max(0.)).sum();
    if total <= 0. {
        return vec![(0., 0.); weights.len()];
    }
    let gap = if weights.len() > 1 { SECTOR_GAP } else { 0. };
    let mut at = 0.;
    weights
        .iter()
        .map(|weight| {
            let width = weight.max(0.) / total;
            let span = (at, (at + width - gap).max(at));
            at += width;
            span
        })
        .collect()
}

/// The first stage without a split: the one being timed.
pub fn current_stage(stages: &[StopwatchStage]) -> Option<usize> {
    stages.iter().position(|stage| stage.split.is_none())
}

/// How long the current stage has run: the total less every split so far.
pub fn stage_elapsed(stages: &[StopwatchStage], elapsed: Duration) -> Duration {
    let done: Duration = stages.iter().filter_map(|stage| stage.split).sum();
    elapsed.saturating_sub(done)
}

/// How far along its sector the hand sits: the share of last run's time the
/// stage has used, held at the sector's end once it runs over.
pub fn hand_turn(span: (f32, f32), used: Duration, reference: Option<Duration>) -> f32 {
    let share = match reference {
        Some(reference) if !reference.is_zero() => {
            (used.as_secs_f32() / reference.as_secs_f32()).min(1.)
        }
        _ => 0.,
    };
    span.0 + (span.1 - span.0) * share
}

/// The running gain or loss against last run, in milliseconds: every finished
/// stage's change, plus any time the current stage has run past last run.
pub fn running_delta(stages: &[StopwatchStage], elapsed: Duration) -> Option<i64> {
    let mut known = false;
    let mut delta = 0i64;
    for stage in stages {
        if let (Some(split), Some(previous)) = (stage.split, stage.previous) {
            known = true;
            delta += split.as_millis() as i64 - previous.as_millis() as i64;
        }
    }
    if let Some(ix) = current_stage(stages) {
        let used = stage_elapsed(stages, elapsed);
        if let Some(previous) = stages[ix].previous
            && used > previous
        {
            known = true;
            delta += (used - previous).as_millis() as i64;
        }
    }
    known.then_some(delta)
}

/// A point `turn` of the way round (0 at the top, clockwise), `radius` from
/// the middle of the dial.
pub fn dial_point(turn: f32, radius: f32) -> (f32, f32) {
    let angle = turn * TAU - TAU / 4.;
    (CENTRE + radius * angle.cos(), CENTRE + radius * angle.sin())
}

/// The sector under a point of the dial, if it lies on the band.
pub fn sector_at(x: f32, y: f32, spans: &[(f32, f32)]) -> Option<usize> {
    let (dx, dy) = (x - CENTRE, y - CENTRE);
    let distance = (dx * dx + dy * dy).sqrt();
    if !(SECTOR_INNER - 8.0..=DELTA_OUTER + 4.).contains(&distance) {
        return None;
    }
    let turn = (dy.atan2(dx) + TAU / 4.).rem_euclid(TAU) / TAU;
    spans
        .iter()
        .position(|(start, end)| turn >= *start && turn < *end + SECTOR_GAP)
}

/// Step a picked stage with a key: arrows wrap, Home and End jump, Escape
/// lets go. `None` for a key that does not move it.
pub fn step_stage(now: Option<usize>, key: &str, count: usize) -> Option<Option<usize>> {
    if count == 0 {
        return None;
    }
    Some(match key {
        "right" | "down" => Some(now.map_or(0, |at| (at + 1) % count)),
        "left" | "up" => Some(now.map_or(count - 1, |at| (at + count - 1) % count)),
        "home" => Some(0),
        "end" => Some(count - 1),
        "escape" => None,
        _ => return None,
    })
}

fn paint_band(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    scale: f32,
    (start, end): (f32, f32),
    (inner, outer): (f32, f32),
    color: Hsla,
) {
    if end <= start {
        return;
    }
    let steps = ((end - start) * 180.).ceil().max(2.) as usize;
    let at =
        |(x, y): (f32, f32)| point(bounds.left() + px(x * scale), bounds.top() + px(y * scale));
    let mut outline = Vec::with_capacity(steps * 2 + 2);
    for step in 0..=steps {
        let turn = start + (end - start) * step as f32 / steps as f32;
        outline.push(at(dial_point(turn, outer)));
    }
    for step in (0..=steps).rev() {
        let turn = start + (end - start) * step as f32 / steps as f32;
        outline.push(at(dial_point(turn, inner)));
    }
    let mut path = PathBuilder::fill();
    path.add_polygon(&outline, true);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn paint_disc(window: &mut Window, bounds: Bounds<Pixels>, scale: f32, r: f32, color: Hsla) {
    let ring: Vec<_> = (0..20)
        .map(|step| {
            let angle = step as f32 / 20. * TAU;
            point(
                bounds.left() + px((CENTRE + r * angle.cos()) * scale),
                bounds.top() + px((CENTRE + r * angle.sin()) * scale),
            )
        })
        .collect();
    let mut disc = PathBuilder::fill();
    disc.add_polygon(&ring, true);
    if let Ok(disc) = disc.build() {
        window.paint_path(disc, color);
    }
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// A pipeline's stages timed on a stopwatch dial.
#[derive(IntoElement)]
pub struct StageStopwatch {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    stages: Vec<StopwatchStage>,
    elapsed: Duration,
    running: bool,
    on_start: Option<Handler>,
    on_stop: Option<Handler>,
    on_reset: Option<Handler>,
    on_select: Option<SelectHandler>,
}

impl StageStopwatch {
    /// Create a stopwatch for a list of stages.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        stages: Vec<StopwatchStage>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            stages,
            elapsed: Duration::ZERO,
            running: false,
            on_start: None,
            on_stop: None,
            on_reset: None,
            on_select: None,
        }
    }

    /// A quieter line under the title, such as the commit being built.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The total time on the watch.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = elapsed;
        self
    }

    /// Whether the watch is counting.
    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    /// Run when Start (or Resume) is pressed.
    pub fn on_start(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_start = Some(Rc::new(handler));
        self
    }

    /// Run when Stop is pressed.
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }

    /// Run when Reset is pressed.
    pub fn on_reset(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reset = Some(Rc::new(handler));
        self
    }

    /// Run with the stage the reader points at or steps to, or `None` when
    /// they let go.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Where the watch is.
    pub fn status(&self) -> StopwatchStatus {
        if !self.stages.is_empty() && current_stage(&self.stages).is_none() {
            StopwatchStatus::Finished
        } else if self.running {
            StopwatchStatus::Running
        } else if self.elapsed.is_zero() {
            StopwatchStatus::Ready
        } else {
            StopwatchStatus::Paused
        }
    }

    /// What a screen reader hears for the whole dial.
    fn spoken(&self) -> String {
        let status = self.status();
        let mut spoken = format!(
            "{}: {}, {}",
            self.title,
            status.word(),
            format_clock(self.elapsed)
        );
        if let Some(ix) = current_stage(&self.stages)
            && status != StopwatchStatus::Ready
        {
            spoken.push_str(&format!(
                ", stage {} of {}, {}",
                ix + 1,
                self.stages.len(),
                self.stages[ix].name
            ));
        }
        if let Some(delta) = running_delta(&self.stages, self.elapsed) {
            spoken.push_str(&format!(", {}", spoken_delta(delta)));
        }
        spoken.push_str(". Arrow keys read each stage.");
        spoken
    }

    /// What a screen reader hears for one stage.
    fn spoken_stage(&self, ix: usize) -> String {
        let stage = &self.stages[ix];
        let current = current_stage(&self.stages);
        match stage.split {
            Some(split) => {
                let mut line = format!("{}: {}", stage.name, format_clock(split));
                if let Some(previous) = stage.previous {
                    let delta = split.as_millis() as i64 - previous.as_millis() as i64;
                    line.push_str(&format!(", {}", spoken_delta(delta)));
                }
                line
            }
            None if current == Some(ix) && !self.elapsed.is_zero() => format!(
                "{}: {}, {} so far",
                stage.name,
                if self.running { "running" } else { "stopped" },
                format_clock(stage_elapsed(&self.stages, self.elapsed))
            ),
            None => match stage.previous {
                Some(previous) => format!(
                    "{}: waiting, last run {}",
                    stage.name,
                    format_clock(previous)
                ),
                None => format!("{}: waiting", stage.name),
            },
        }
    }
}

impl Styled for StageStopwatch {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StageStopwatch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.chart_1;
        let over = theme.warning;
        let gained = theme.success;
        let lost = theme.danger;
        let surface = theme.background;
        let mono = theme.mono_font_family.clone();
        let picked_fill = theme.table_active;
        let reduce_motion = cx.reduce_motion();

        let status = self.status();
        let count = self.stages.len();
        let spans = sector_spans(&reference_weights(&self.stages));
        let current = current_stage(&self.stages);
        let used = stage_elapsed(&self.stages, self.elapsed);
        let delta = running_delta(&self.stages, self.elapsed);

        let active = window.use_keyed_state(part("picked"), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);
        let dial_bounds = window
            .use_keyed_state(part("bounds"), cx, |_, _| {
                Rc::new(Cell::new(Bounds::default()))
            })
            .read(cx)
            .clone();

        // Where the hand points. It sweeps the current sector as the stage
        // uses last run's time; under reduced motion it rests mid-sector.
        let hand_target = match (current, status) {
            (_, StopwatchStatus::Ready | StopwatchStatus::Finished) | (None, _) => 0.,
            (Some(ix), _) if reduce_motion => (spans[ix].0 + spans[ix].1) / 2.,
            (Some(ix), _) => hand_turn(spans[ix], used, self.stages[ix].previous),
        };
        let hand = transition(
            part("hand"),
            hand_target,
            Transition::new(Duration::from_millis(140)).ease(|time| time),
            window,
            cx,
        );

        // The other sectors fade back while one is picked.
        let strengths: Vec<f32> = (0..count)
            .map(|ix| {
                transition(
                    ElementId::NamedChild(self.id.clone().into(), format!("dim-{ix}").into()),
                    if picked.is_some_and(|at| at != ix) {
                        0.4
                    } else {
                        1.
                    },
                    Transition::new(Duration::from_millis(140)),
                    window,
                    cx,
                )
            })
            .collect();
        // What each sector shows, worked out here so the painter only paints.
        struct Sector {
            span: (f32, f32),
            filled: Option<(f32, Hsla)>,
            edge: Option<Hsla>,
            strength: f32,
        }
        let sectors: Vec<Sector> = self
            .stages
            .iter()
            .enumerate()
            .map(|(ix, stage)| {
                let span = spans[ix];
                let filled = match stage.split {
                    Some(_) => Some((span.1, fill)),
                    None if current == Some(ix) && !self.elapsed.is_zero() => {
                        let overran = stage.previous.is_some_and(|previous| used > previous);
                        let reach = if reduce_motion {
                            hand_turn(span, used, stage.previous)
                        } else {
                            hand.clamp(span.0, span.1)
                        };
                        Some((reach, if overran { over } else { fill }))
                    }
                    None => None,
                };
                let edge = match (stage.split, stage.previous) {
                    (Some(split), Some(previous)) if split > previous => Some(lost),
                    (Some(split), Some(previous)) if split < previous => Some(gained),
                    _ => None,
                };
                Sector {
                    span,
                    filled,
                    edge,
                    strength: strengths[ix],
                }
            })
            .collect();
        let show_hand = status != StopwatchStatus::Ready || !reduce_motion;

        let seen = dial_bounds.clone();
        let face = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX;
                let at = |(x, y): (f32, f32)| {
                    point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                };
                for tick in 0..60 {
                    let turn = tick as f32 / 60.;
                    let major = tick % 5 == 0;
                    let mut path = PathBuilder::stroke(px(if major { 1.5 } else { 1. } * scale));
                    path.move_to(at(dial_point(
                        turn,
                        TICK_OUTER - if major { 8. } else { 4. },
                    )));
                    path.line_to(at(dial_point(turn, TICK_OUTER)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, ink.opacity(if major { 0.45 } else { 0.2 }));
                    }
                }
                for sector in &sectors {
                    let strength = sector.strength;
                    let band = (SECTOR_INNER, SECTOR_OUTER);
                    paint_band(
                        window,
                        bounds,
                        scale,
                        sector.span,
                        band,
                        ink.opacity(0.08 * strength),
                    );
                    if let Some((reach, color)) = sector.filled {
                        paint_band(
                            window,
                            bounds,
                            scale,
                            (sector.span.0, reach),
                            band,
                            color.opacity(color.a * strength),
                        );
                    }
                    if let Some(color) = sector.edge {
                        paint_band(
                            window,
                            bounds,
                            scale,
                            sector.span,
                            (DELTA_INNER, DELTA_OUTER),
                            color.opacity(color.a * strength),
                        );
                    }
                }
            },
        )
        .w_full()
        .aspect_ratio(1.);

        let hand_layer = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX;
                if show_hand {
                    let at = |(x, y): (f32, f32)| {
                        point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                    };
                    let mut path = PathBuilder::stroke(px(2. * scale));
                    path.move_to(at(dial_point(hand + 0.5, TAIL)));
                    path.line_to(at(dial_point(hand, HAND)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, ink);
                    }
                }
                paint_disc(window, bounds, scale, 5., ink);
                paint_disc(window, bounds, scale, 2., surface);
            },
        )
        .absolute()
        .inset_0();

        // The readout: the stage above the pivot, the time and change below.
        let stage_line: SharedString = match (status, current) {
            (StopwatchStatus::Finished, _) => format!("All {count} stages").into(),
            (_, Some(ix)) => self.stages[ix].name.clone(),
            _ => "".into(),
        };
        let delta_color = match delta {
            Some(ms) if ms > 50 => lost,
            Some(ms) if ms < -50 => gained,
            _ => quiet,
        };
        let readout =
            div()
                .absolute()
                .inset_0()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(relative(64. / BOX))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .max_w(relative(0.5))
                                .px_1()
                                .rounded(px(4.))
                                .bg(surface)
                                .truncate()
                                .text_caption()
                                .text_color(quiet)
                                .child(stage_line),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(relative(136. / BOX))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .v_flex()
                                .items_center()
                                .px_2()
                                .rounded(px(6.))
                                .bg(surface)
                                .child(
                                    div()
                                        .font_family(mono.clone())
                                        .text_size(px(22.))
                                        .line_height(px(28.))
                                        .font_medium()
                                        .child(format_clock(self.elapsed)),
                                )
                                .child(
                                    div()
                                        .font_family(mono.clone())
                                        .text_caption()
                                        .text_color(delta_color)
                                        .child(delta.map_or_else(
                                            || "No last run".to_string(),
                                            format_delta,
                                        )),
                                ),
                        ),
                );

        let spoken: SharedString = match picked {
            Some(ix) => self.spoken_stage(ix).into(),
            None => self.spoken().into(),
        };
        let hover_state = active.clone();
        let hover_select = self.on_select.clone();
        let hover_spans = spans;
        let leave_state = active.clone();
        let leave_select = self.on_select.clone();
        let key_state = active;
        let key_select = self.on_select.clone();
        let focus = window
            .use_keyed_state(part("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let ring = focus.is_focused(window) && window.last_input_was_keyboard();
        let dial = div()
            .id(part("dial"))
            .rounded_full()
            .when(ring, |this| this.focus_ring_style(window, cx))
            .test_support()
            .track_focus(&focus)
            .relative()
            .mx_auto()
            .w_full()
            .max_w(px(MAX_WIDTH))
            .role(Role::Group)
            .aria_label(spoken)
            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                let bounds = dial_bounds.get();
                let scale = f32::from(bounds.size.width) / BOX;
                if scale <= 0. {
                    return;
                }
                let x = f32::from(event.position.x - bounds.left()) / scale;
                let y = f32::from(event.position.y - bounds.top()) / scale;
                let next = sector_at(x, y, &hover_spans);
                if *hover_state.read(cx) != next {
                    hover_state.update(cx, |state, cx| {
                        *state = next;
                        cx.notify();
                    });
                    if let Some(select) = &hover_select {
                        select(next, window, cx);
                    }
                }
            })
            .on_hover(move |over, window, cx| {
                if !*over && leave_state.read(cx).is_some() {
                    leave_state.update(cx, |state, cx| {
                        *state = None;
                        cx.notify();
                    });
                    if let Some(select) = &leave_select {
                        select(None, window, cx);
                    }
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let now = *key_state.read(cx);
                let Some(next) = step_stage(now, event.keystroke.key.as_str(), count) else {
                    return;
                };
                key_state.update(cx, |state, cx| {
                    *state = next;
                    cx.notify();
                });
                if let Some(select) = &key_select {
                    select(next, window, cx);
                }
                cx.stop_propagation();
            })
            .child(face)
            .child(hand_layer)
            .child(readout);

        let splits = Table::new()
            .small()
            .accessibility_label(format!("{} splits", self.title))
            .child(
                TableHeader::new().child(
                    TableRow::new()
                        .child(TableHead::new().flex_1().min_w_0().child("Stage"))
                        .child(
                            TableHead::new()
                                .w(px(72.))
                                .min_w_0()
                                .text_right()
                                .child("Split"),
                        )
                        .child(
                            TableHead::new()
                                .w(px(72.))
                                .min_w_0()
                                .text_right()
                                .child("Last"),
                        )
                        .child(
                            TableHead::new()
                                .w(px(72.))
                                .min_w_0()
                                .text_right()
                                .child("Change"),
                        ),
                ),
            )
            .child(
                TableBody::new().children(self.stages.iter().enumerate().map(|(ix, stage)| {
                    let is_current = current == Some(ix) && !self.elapsed.is_zero();
                    let (split, split_color) = match stage.split {
                        Some(split) => (format_clock(split), ink),
                        None if is_current => (format_clock(used), quiet),
                        None => ("\u{2014}".to_string(), quiet),
                    };
                    let change = match (stage.split, stage.previous) {
                        (Some(split), Some(previous)) => {
                            Some(split.as_millis() as i64 - previous.as_millis() as i64)
                        }
                        (None, Some(previous)) if is_current && used > previous => {
                            Some((used - previous).as_millis() as i64)
                        }
                        _ => None,
                    };
                    let change_color = match change {
                        Some(ms) if ms > 50 => lost,
                        Some(ms) if ms < -50 => gained,
                        _ => quiet,
                    };
                    let marker = match stage.split {
                        Some(_) => fill,
                        None if is_current => {
                            if change.is_some() {
                                over
                            } else {
                                fill.opacity(0.5)
                            }
                        }
                        None => ink.opacity(0.15),
                    };
                    let figure = |text: String, color: Hsla| {
                        div()
                            .font_family(mono.clone())
                            .text_caption()
                            .text_color(color)
                            .child(text)
                    };
                    TableRow::new()
                        .id(("stage", ix))
                        .when(picked == Some(ix), |this| this.bg(picked_fill))
                        .child(
                            TableCell::new().flex_1().min_w_0().child(
                                h_flex()
                                    .min_w_0()
                                    .gap_2()
                                    .child(div().flex_none().size(px(8.)).rounded_full().bg(marker))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_caption()
                                            .when(is_current, |this| this.font_medium())
                                            .child(stage.name.clone()),
                                    ),
                            ),
                        )
                        .child(
                            TableCell::new()
                                .w(px(72.))
                                .min_w_0()
                                .text_right()
                                .child(figure(split, split_color)),
                        )
                        .child(
                            TableCell::new()
                                .w(px(72.))
                                .min_w_0()
                                .text_right()
                                .child(figure(
                                    stage
                                        .previous
                                        .map_or_else(|| "\u{2014}".to_string(), format_clock),
                                    quiet,
                                )),
                        )
                        .child(
                            TableCell::new()
                                .w(px(72.))
                                .min_w_0()
                                .text_right()
                                .child(figure(
                                    change.map_or_else(|| "\u{2014}".to_string(), format_delta),
                                    change_color,
                                )),
                        )
                })),
            );

        let (start_label, start_icon) = match status {
            StopwatchStatus::Running => ("Stop", CatalogIcon::PauseFilled),
            StopwatchStatus::Paused => ("Resume", CatalogIcon::PlayFilledAlt),
            _ => ("Start", CatalogIcon::PlayFilledAlt),
        };
        let toggle = if status == StopwatchStatus::Running {
            self.on_stop.clone()
        } else {
            self.on_start.clone()
        };
        let reset = self.on_reset.clone();
        let controls = h_flex()
            .gap_2()
            .justify_center()
            .child(
                Button::new(part("toggle"))
                    .primary()
                    .icon(Icon::new(start_icon))
                    .label(start_label)
                    .disabled(status == StopwatchStatus::Finished || count == 0)
                    .on_click(move |_, window, cx| {
                        if let Some(toggle) = &toggle {
                            toggle(window, cx);
                        }
                    }),
            )
            .child(
                Button::new(part("reset"))
                    .ghost()
                    .icon(Icon::new(CatalogIcon::Reset))
                    .label("Reset")
                    .disabled(status == StopwatchStatus::Ready)
                    .on_click(move |_, window, cx| {
                        if let Some(reset) = &reset {
                            reset(window, cx);
                        }
                    }),
            );

        shell(cx)
            .id(self.id.clone())
            .max_w(px(420.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Timer)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                None,
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .v_flex()
                    .gap_4()
                    .child(dial)
                    .child(controls)
                    .child(splits),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(secs: f32) -> Duration {
        Duration::from_secs_f32(secs)
    }

    fn pipeline() -> Vec<StopwatchStage> {
        vec![
            StopwatchStage::new("Fetch")
                .previous(secs(10.))
                .split(secs(8.)),
            StopwatchStage::new("Build")
                .previous(secs(30.))
                .split(secs(33.)),
            StopwatchStage::new("Test").previous(secs(40.)),
            StopwatchStage::new("Ship").previous(secs(20.)),
        ]
    }

    #[test]
    fn the_clock_reads_like_a_stopwatch() {
        assert_eq!(format_clock(Duration::ZERO), "0:00.0");
        assert_eq!(format_clock(Duration::from_millis(41_260)), "0:41.2");
        assert_eq!(format_clock(Duration::from_millis(125_900)), "2:05.9");
        assert_eq!(format_clock(Duration::from_secs(3_725)), "1:02:05");
    }

    #[test]
    fn changes_are_signed_and_spoken() {
        assert_eq!(format_delta(2_140), "+2.1s");
        assert_eq!(format_delta(-800), "\u{2212}0.8s");
        assert_eq!(format_delta(20), "±0.0s");
        assert_eq!(format_delta(64_000), "+1:04.0");
        assert_eq!(spoken_delta(2_140), "2.1 seconds slower than last run");
        assert_eq!(spoken_delta(-800), "0.8 seconds faster than last run");
        assert_eq!(spoken_delta(0), "level with last run");
    }

    #[test]
    fn sectors_follow_last_runs_times() {
        let spans = sector_spans(&reference_weights(&pipeline()));
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].0, 0.);
        // Fetch took 10 of 100 seconds: a tenth of the dial, less the gap.
        assert!((spans[0].1 - (0.1 - SECTOR_GAP)).abs() < 1e-5);
        assert!((spans[1].0 - 0.1).abs() < 1e-5);
        assert!((spans[3].1 - (1. - SECTOR_GAP)).abs() < 1e-5);
        // Without last run's times every stage gets an equal share.
        let plain = vec![StopwatchStage::new("A"), StopwatchStage::new("B")];
        assert_eq!(reference_weights(&plain), [1., 1.]);
        // A stage without a time takes the average of the others.
        let mixed = vec![
            StopwatchStage::new("A").previous(secs(10.)),
            StopwatchStage::new("B"),
            StopwatchStage::new("C").previous(secs(30.)),
        ];
        assert_eq!(reference_weights(&mixed), [10., 20., 30.]);
        assert_eq!(sector_spans(&[]), []);
    }

    #[test]
    fn the_hand_sweeps_the_current_sector_and_stops_at_its_end() {
        let span = (0.2, 0.4);
        assert_eq!(hand_turn(span, Duration::ZERO, Some(secs(10.))), 0.2);
        assert!((hand_turn(span, secs(5.), Some(secs(10.))) - 0.3).abs() < 1e-5);
        assert_eq!(hand_turn(span, secs(15.), Some(secs(10.))), 0.4);
        assert_eq!(hand_turn(span, secs(15.), None), 0.2);
    }

    #[test]
    fn the_current_stage_and_its_time() {
        let stages = pipeline();
        assert_eq!(current_stage(&stages), Some(2));
        assert_eq!(stage_elapsed(&stages, secs(50.)), secs(9.));
        assert_eq!(stage_elapsed(&stages, secs(10.)), Duration::ZERO);
    }

    #[test]
    fn the_running_change_counts_finished_stages_and_any_overrun() {
        let stages = pipeline();
        // Fetch gained 2s, Build lost 3s: one second behind.
        assert_eq!(running_delta(&stages, secs(50.)), Some(1_000));
        // Test has run 45s against 40s last time: five more behind.
        assert_eq!(running_delta(&stages, secs(86.)), Some(6_000));
        assert_eq!(running_delta(&[StopwatchStage::new("A")], secs(5.)), None);
    }

    #[test]
    fn a_point_on_the_band_names_its_stage() {
        let spans = sector_spans(&[1., 1., 1., 1.]);
        let (x, y) = dial_point(0.1, 97.);
        assert_eq!(sector_at(x, y, &spans), Some(0));
        let (x, y) = dial_point(0.6, 97.);
        assert_eq!(sector_at(x, y, &spans), Some(2));
        // The middle is no stage.
        assert_eq!(sector_at(CENTRE, CENTRE, &spans), None);
    }

    #[test]
    fn keys_step_through_the_stages() {
        assert_eq!(step_stage(None, "right", 4), Some(Some(0)));
        assert_eq!(step_stage(Some(3), "right", 4), Some(Some(0)));
        assert_eq!(step_stage(None, "left", 4), Some(Some(3)));
        assert_eq!(step_stage(Some(1), "end", 4), Some(Some(3)));
        assert_eq!(step_stage(Some(1), "escape", 4), Some(None));
        assert_eq!(step_stage(Some(1), "a", 4), None);
        assert_eq!(step_stage(None, "right", 0), None);
    }

    #[test]
    fn status_and_spoken_lines() {
        let watch = StageStopwatch::new("w", "Release", pipeline())
            .elapsed(secs(50.))
            .running(true);
        assert_eq!(watch.status(), StopwatchStatus::Running);
        assert_eq!(
            watch.spoken(),
            "Release: running, 0:50.0, stage 3 of 4, Test, 1.0 seconds slower than last run. \
             Arrow keys read each stage."
        );
        assert_eq!(
            watch.spoken_stage(1),
            "Build: 0:33.0, 3.0 seconds slower than last run"
        );
        assert_eq!(watch.spoken_stage(2), "Test: running, 0:09.0 so far");
        assert_eq!(watch.spoken_stage(3), "Ship: waiting, last run 0:20.0");
        let ready = StageStopwatch::new("w", "Release", vec![StopwatchStage::new("A")]);
        assert_eq!(ready.status(), StopwatchStatus::Ready);
        let paused = StageStopwatch::new("w", "Release", pipeline()).elapsed(secs(1.));
        assert_eq!(paused.status(), StopwatchStatus::Paused);
        let done = StageStopwatch::new("w", "R", vec![StopwatchStage::new("A").split(secs(1.))]);
        assert_eq!(done.status(), StopwatchStatus::Finished);
    }
}
