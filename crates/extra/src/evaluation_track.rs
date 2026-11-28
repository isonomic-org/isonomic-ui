// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Evaluation track: several evaluation runs racing round an oval.
//!
//! Each run has its own lane and a marker that laps the oval as the run makes
//! progress through its laps (passes over the test set, say). The marker
//! carries the run's place, so the race reads at a glance; the infield names
//! the leader or the picked run. Beside the track a standings table lists the
//! runs in order with their lap, score and gap to the leader. Pick a run by
//! clicking its marker or its name, or focus the track and step with the
//! arrow keys; Escape lets go. Markers glide to their new place, and simply
//! move there under reduced motion.

use std::{f32::consts::PI, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, ThemeStyled as _, h_flex,
    look,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The track is laid out on a 376 by 236 box and scaled to its width.
const BOX_W: f32 = 376.;
const BOX_H: f32 = 236.;
const CX: f32 = 188.;
const CY: f32 = 118.;
/// Half the length of a straight.
const STRAIGHT: f32 = 70.;
/// The radius of the infield's ends, and how wide a lane is.
const INFIELD: f32 = 44.;
const LANE: f32 = 11.;
/// The most lanes drawn; more runs share the outer lanes.
const MAX_LANES: usize = 6;
const MARKER: f32 = 18.;
/// How long a marker takes to glide to its new place.
const GLIDE: Duration = Duration::from_millis(320);
/// How long the other markers take to dim when one is picked.
const FADE: Duration = Duration::from_millis(140);

/// One evaluation run on the track.
#[derive(Clone)]
pub struct TrackRun {
    name: SharedString,
    progress: f32,
    score: Option<SharedString>,
    color: Option<Hsla>,
}

impl TrackRun {
    /// A run that has covered `progress` laps: 2.4 is two laps and most of a
    /// third.
    pub fn new(name: impl Into<SharedString>, progress: f32) -> Self {
        Self {
            name: name.into(),
            progress: progress.max(0.),
            score: None,
            color: None,
        }
    }

    /// The run's score so far, as it should be written.
    pub fn score(mut self, score: impl Into<SharedString>) -> Self {
        self.score = Some(score.into());
        self
    }

    /// Its marker colour. By default the data colours in turn.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// One line of the standings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    /// The run's index in the list given.
    pub run: usize,
    /// Its place, from 1; runs level on progress share a place.
    pub place: usize,
    /// How many laps it trails the leader by.
    pub gap: f32,
}

/// The runs in race order, leader first, with places and gaps. Runs level on
/// progress share a place and keep the order they were given in.
pub fn standings(progress: &[f32]) -> Vec<Standing> {
    let mut order: Vec<usize> = (0..progress.len()).collect();
    order.sort_by(|a, b| progress[*b].total_cmp(&progress[*a]));
    let leader = order.first().map_or(0., |ix| progress[*ix]);
    let mut lines: Vec<Standing> = Vec::with_capacity(order.len());
    for (at, &run) in order.iter().enumerate() {
        let place = match lines.last() {
            Some(previous) if progress[previous.run] == progress[run] => previous.place,
            _ => at + 1,
        };
        lines.push(Standing {
            run,
            place,
            gap: leader - progress[run],
        });
    }
    lines
}

/// The radius of a lane's middle line round the ends.
pub fn lane_radius(lane: usize) -> f32 {
    INFIELD + LANE * (lane.min(MAX_LANES - 1) as f32 + 0.5)
}

/// A point `along` the way round a lane of `radius`, from the finish line at
/// the middle of the top straight, running clockwise.
pub fn track_point(along: f32, radius: f32) -> (f32, f32) {
    let bend = PI * radius;
    let mut d = along.rem_euclid(1.) * (4. * STRAIGHT + 2. * bend);
    if d < STRAIGHT {
        return (CX + d, CY - radius);
    }
    d -= STRAIGHT;
    if d < bend {
        let angle = d / radius;
        return (
            CX + STRAIGHT + radius * angle.sin(),
            CY - radius * angle.cos(),
        );
    }
    d -= bend;
    if d < 2. * STRAIGHT {
        return (CX + STRAIGHT - d, CY + radius);
    }
    d -= 2. * STRAIGHT;
    if d < bend {
        let angle = d / radius;
        return (
            CX - STRAIGHT - radius * angle.sin(),
            CY + radius * angle.cos(),
        );
    }
    d -= bend;
    (CX - STRAIGHT + d, CY - radius)
}

/// Where a run is in the race, in words for the table: "Lap 3 of 5", or
/// "Finished".
pub fn lap_label(progress: f32, laps: usize) -> String {
    if laps > 0 && progress >= laps as f32 {
        "Finished".into()
    } else {
        format!(
            "Lap {} of {laps}",
            (progress.floor() as usize + 1).min(laps.max(1))
        )
    }
}

/// The gap to the leader for the table: "Leader", "+0.42 laps", "+1.00 lap".
pub fn gap_label(gap: f32) -> String {
    if gap < 0.005 {
        "Leader".into()
    } else if (gap - 1.).abs() < 0.005 {
        "+1.00 lap".into()
    } else {
        format!("+{gap:.2} laps")
    }
}

/// The gap to the leader in words.
pub fn spoken_gap(gap: f32) -> String {
    if gap < 0.005 {
        "leading".into()
    } else {
        format!("{gap:.2} laps behind the leader")
    }
}

/// Step the picked run through the standings with a key: down and right go
/// down the order, up and left go up it, Home and End jump, Escape lets go.
/// Works on places in `order` (run indexes, leader first) and returns a run
/// index. `None` for a key that does not move it.
pub fn step_run(now: Option<usize>, key: &str, order: &[usize]) -> Option<Option<usize>> {
    let count = order.len();
    if count == 0 {
        return None;
    }
    let at = now.and_then(|run| order.iter().position(|ix| *ix == run));
    let next = match key {
        "down" | "right" => at.map_or(0, |at| (at + 1) % count),
        "up" | "left" => at.map_or(count - 1, |at| (at + count - 1) % count),
        "home" => 0,
        "end" => count - 1,
        "escape" => return Some(None),
        _ => return None,
    };
    Some(Some(order[next]))
}

fn paint_stadium_band(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    scale: f32,
    margin: f32,
    (inner, outer): (f32, f32),
    color: Hsla,
) {
    let steps = 96;
    let at = |(x, y): (f32, f32)| {
        point(
            bounds.left() + px((x - margin) * scale),
            bounds.top() + px((y - margin) * scale),
        )
    };
    let mut outline: Vec<_> = (0..=steps)
        .map(|step| at(track_point(step as f32 / steps as f32, outer)))
        .collect();
    outline.extend(
        (0..=steps)
            .rev()
            .map(|step| at(track_point(step as f32 / steps as f32, inner))),
    );
    let mut path = PathBuilder::fill();
    path.add_polygon(&outline, true);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

type SelectHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// Evaluation runs racing round an oval, with standings.
#[derive(IntoElement)]
pub struct EvaluationTrack {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    laps: usize,
    runs: Vec<TrackRun>,
    score_label: SharedString,
    on_select: Option<SelectHandler>,
}

impl EvaluationTrack {
    /// Create a track for a race of `laps` laps.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, laps: usize) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            laps: laps.max(1),
            runs: Vec::new(),
            score_label: "Score".into(),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a run.
    pub fn run(mut self, run: TrackRun) -> Self {
        self.runs.push(run);
        self
    }

    /// Add several runs.
    pub fn runs(mut self, runs: impl IntoIterator<Item = TrackRun>) -> Self {
        self.runs.extend(runs);
        self
    }

    /// The heading of the score column. "Score" by default.
    pub fn score_label(mut self, label: impl Into<SharedString>) -> Self {
        self.score_label = label.into();
        self
    }

    /// Run with the run the reader picks, or `None` when they let go.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn progress(&self) -> Vec<f32> {
        self.runs.iter().map(|run| run.progress).collect()
    }

    /// What a screen reader hears for the track.
    fn spoken(&self, picked: Option<usize>) -> String {
        let table = standings(&self.progress());
        match picked.and_then(|run| table.iter().find(|line| line.run == run)) {
            Some(line) => {
                let run = &self.runs[line.run];
                let mut spoken = format!(
                    "{}: place {} of {}, {}, {}",
                    run.name,
                    line.place,
                    self.runs.len(),
                    lap_label(run.progress, self.laps).to_lowercase(),
                    spoken_gap(line.gap)
                );
                if let Some(score) = &run.score {
                    spoken.push_str(&format!(", {} {score}", self.score_label.to_lowercase()));
                }
                spoken
            }
            None => match table.first() {
                Some(leader) => format!(
                    "{}: {} runs over {} laps. {} leads, {}. Arrow keys pick a run.",
                    self.title,
                    self.runs.len(),
                    self.laps,
                    self.runs[leader.run].name,
                    lap_label(self.runs[leader.run].progress, self.laps).to_lowercase()
                ),
                None => format!("{}: no runs yet", self.title),
            },
        }
    }
}

impl Styled for EvaluationTrack {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for EvaluationTrack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let numbered = |name: &'static str, ix: usize| {
            ElementId::NamedChild(self.id.clone().into(), format!("{name}-{ix}").into())
        };
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let surface = theme.background;
        let mono = theme.mono_font_family.clone();
        let on_status = theme.background;
        let picked_fill = theme.table_active;
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];
        let colors: Vec<Hsla> = self
            .runs
            .iter()
            .enumerate()
            .map(|(ix, run)| run.color.unwrap_or(palette[ix % palette.len()]))
            .collect();
        let count = self.runs.len();
        let lanes = count.clamp(1, MAX_LANES);
        // The view is cropped to the lanes in use.
        let margin = LANE * (MAX_LANES - lanes) as f32;
        let table = standings(&self.progress());
        let order: Vec<usize> = table.iter().map(|line| line.run).collect();

        let active = window.use_keyed_state(part("picked"), cx, |_, _| None::<usize>);
        let picked = (*active.read(cx)).filter(|ix| *ix < count);
        let focus = window
            .use_keyed_state(part("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let ring = focus.is_focused(window) && window.last_input_was_keyboard();

        let pick: Rc<dyn Fn(Option<usize>, &mut Window, &mut App)> = {
            let active = active.clone();
            let select = self.on_select.clone();
            Rc::new(move |next, window, cx| {
                active.update(cx, |state, cx| {
                    *state = next;
                    cx.notify();
                });
                if let Some(select) = &select {
                    select(next, window, cx);
                }
            })
        };

        let ground = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / (BOX_W - 2. * margin);
                let outer = INFIELD + LANE * lanes as f32;
                paint_stadium_band(
                    window,
                    bounds,
                    scale,
                    margin,
                    (INFIELD, outer),
                    ink.opacity(0.05),
                );
                for line in 0..=lanes {
                    let radius = INFIELD + LANE * line as f32;
                    let edge = line == 0 || line == lanes;
                    paint_stadium_band(
                        window,
                        bounds,
                        scale,
                        margin,
                        (radius - 0.5, radius + 0.5),
                        ink.opacity(if edge { 0.22 } else { 0.1 }),
                    );
                }
                // The finish line crosses every lane at the top.
                let mut finish = PathBuilder::stroke(px(2.5 * scale));
                finish.move_to(point(
                    bounds.left() + px((CX - margin) * scale),
                    bounds.top() + px((CY - margin - INFIELD + 1.) * scale),
                ));
                finish.line_to(point(
                    bounds.left() + px((CX - margin) * scale),
                    bounds.top() + px((CY - margin - outer - 1.) * scale),
                ));
                if let Ok(finish) = finish.build() {
                    window.paint_path(finish, ink.opacity(0.7));
                }
            },
        )
        .w_full()
        .aspect_ratio((BOX_W - 2. * margin) / (BOX_H - 2. * margin));

        // Markers are drawn leader last, so the front of the race sits on top.
        let markers: Vec<_> = order
            .iter()
            .rev()
            .filter_map(|&ix| {
                let run = &self.runs[ix];
                let line = table.iter().find(|line| line.run == ix)?;
                let shown = transition(
                    numbered("glide", ix),
                    run.progress,
                    Transition::new(GLIDE),
                    window,
                    cx,
                );
                let (x, y) = track_point(shown, lane_radius(ix));
                let is_picked = picked == Some(ix);
                let dim = picked.is_some() && !is_picked;
                let hovered = window.use_keyed_state(numbered("hovered", ix), cx, |_, _| false);
                let fade = look::hover_fade(numbered("marker", ix), *hovered.read(cx), window, cx);
                let shade = transition(
                    numbered("dim", ix),
                    if dim { 0.45 } else { 1. },
                    Transition::new(FADE),
                    window,
                    cx,
                );
                let lift = if is_picked { 1. } else { fade * 0.6 };
                let size = MARKER + 4. * lift;
                let pick = pick.clone();
                Some(
                    div()
                        .absolute()
                        .left(relative((x - margin) / (BOX_W - 2. * margin)))
                        .top(relative((y - margin) / (BOX_H - 2. * margin)))
                        .w(px(0.))
                        .h(px(0.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id(numbered("marker", ix))
                                .test_support()
                                .flex_none()
                                .size(px(size))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(colors[ix])
                                .border_2()
                                .border_color(look::mix(surface, ink, lift))
                                .opacity(shade.max(fade))
                                .cursor_pointer()
                                .on_hover(move |over, _, cx| {
                                    hovered.update(cx, |state, cx| {
                                        *state = *over;
                                        cx.notify();
                                    })
                                })
                                .text_size(px(10.))
                                .line_height(px(12.))
                                .font_semibold()
                                .text_color(on_status)
                                .child(line.place.to_string())
                                .on_click(move |_, window, cx| {
                                    pick((!is_picked).then_some(ix), window, cx)
                                }),
                        ),
                )
            })
            .collect();

        let infield_run = picked.or(order.first().copied());
        let infield = infield_run.map(|ix| {
            let run = &self.runs[ix];
            let place = table
                .iter()
                .find(|line| line.run == ix)
                .map_or(1, |line| line.place);
            div()
                .absolute()
                .inset_0()
                .v_flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_eyebrow()
                        .text_color(quiet)
                        .child(if picked.is_some() {
                            format!("PLACE {place}")
                        } else {
                            "LEADER".into()
                        }),
                )
                .child(
                    div()
                        .max_w(relative(0.36))
                        .truncate()
                        .text_label()
                        .child(run.name.clone()),
                )
                .child(
                    div()
                        .text_caption()
                        .text_color(quiet)
                        .child(lap_label(run.progress, self.laps)),
                )
        });

        let key_pick = pick.clone();
        let key_state = active;
        let key_order = order;
        let track = div()
            .id(part("track"))
            .rounded(px(12.))
            .when(ring, |this| this.focus_ring_style(window, cx))
            .test_support()
            .track_focus(&focus)
            .relative()
            .w_full()
            .role(Role::Group)
            .aria_label(self.spoken(picked))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let now = *key_state.read(cx);
                if let Some(next) = step_run(now, event.keystroke.key.as_str(), &key_order) {
                    key_pick(next, window, cx);
                    cx.stop_propagation();
                }
            })
            .child(ground)
            .children(infield)
            .children(markers);

        let has_score = self.runs.iter().any(|run| run.score.is_some());
        let figure = move |text: String, color: Hsla| {
            div()
                .font_family(mono.clone())
                .text_caption()
                .text_color(color)
                .child(text)
        };
        let wash = look::ink::wash(cx);
        let name_hover: Vec<_> = (0..count)
            .map(|ix| {
                let hovered =
                    window.use_keyed_state(numbered("name-hovered", ix), cx, |_, _| false);
                let fade = look::hover_fade(numbered("name", ix), *hovered.read(cx), window, cx);
                (hovered, fade)
            })
            .collect();
        let rows =
            table.iter().map(|line| {
                let run = &self.runs[line.run];
                let ix = line.run;
                let is_picked = picked == Some(ix);
                let pick = pick.clone();
                TableRow::new()
                    .id(("run", ix))
                    .when(is_picked, |this| this.bg(picked_fill))
                    .child(
                        TableCell::new()
                            .w(px(36.))
                            .min_w_0()
                            .child(figure(line.place.to_string(), ink)),
                    )
                    .child(
                        TableCell::new().flex_1().min_w_0().child(
                            h_flex()
                                .id(numbered("name", ix))
                                .min_w_0()
                                .gap_2()
                                .px_1()
                                .mx(px(-4.))
                                .rounded(px(4.))
                                .bg(wash.opacity(wash.a * name_hover[ix].1))
                                .cursor_pointer()
                                .on_hover({
                                    let hovered = name_hover[ix].0.clone();
                                    move |over, _, cx| {
                                        hovered.update(cx, |state, cx| {
                                            *state = *over;
                                            cx.notify();
                                        })
                                    }
                                })
                                .on_click(move |_, window, cx| {
                                    pick((!is_picked).then_some(ix), window, cx)
                                })
                                .child(div().flex_none().size(px(8.)).rounded_full().bg(colors[ix]))
                                .child(
                                    div()
                                        .truncate()
                                        .text_caption()
                                        .when(is_picked, |this| this.font_medium())
                                        .child(run.name.clone()),
                                ),
                        ),
                    )
                    .child(
                        TableCell::new().w(px(84.)).min_w_0().child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child(lap_label(run.progress, self.laps)),
                        ),
                    )
                    .when(has_score, |this| {
                        this.child(TableCell::new().w(px(64.)).min_w_0().text_right().child(
                            figure(
                                run.score.as_ref().map_or_else(
                                    || "\u{2014}".to_string(),
                                    |score| score.to_string(),
                                ),
                                ink,
                            ),
                        ))
                    })
                    .child(
                        TableCell::new()
                            .w(px(84.))
                            .min_w_0()
                            .text_right()
                            .child(figure(gap_label(line.gap), quiet)),
                    )
            });
        let standings_table = Table::new()
            .small()
            .accessibility_label(format!("{} standings", self.title))
            .child(
                TableHeader::new().child(
                    TableRow::new()
                        .child(TableHead::new().w(px(36.)).min_w_0().child("Pos"))
                        .child(TableHead::new().flex_1().min_w_0().child("Run"))
                        .child(TableHead::new().w(px(84.)).min_w_0().child("Lap"))
                        .when(has_score, |this| {
                            this.child(
                                TableHead::new()
                                    .w(px(64.))
                                    .min_w_0()
                                    .text_right()
                                    .child(self.score_label.clone()),
                            )
                        })
                        .child(
                            TableHead::new()
                                .w(px(84.))
                                .min_w_0()
                                .text_right()
                                .child("Gap"),
                        ),
                ),
            )
            .child(TableBody::new().children(rows));

        shell(cx)
            .id(self.id.clone())
            .max_w(px(560.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Trophy)),
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
                    .child(track)
                    .child(standings_table),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
    }

    #[test]
    fn the_oval_starts_at_the_finish_line_and_runs_clockwise() {
        let r = 50.;
        assert!(close(track_point(0., r), (CX, CY - r)));
        // Half way round is the middle of the bottom straight.
        assert!(close(track_point(0.5, r), (CX, CY + r)));
        // The far end of the right bend sits level with the middle.
        let quarter = (STRAIGHT + PI * r / 2.) / (4. * STRAIGHT + 2. * PI * r);
        assert!(close(track_point(quarter, r), (CX + STRAIGHT + r, CY)));
        // Whole laps land back on the line.
        assert!(close(track_point(3., r), track_point(0., r)));
        assert!(close(track_point(2.5, r), track_point(0.5, r)));
    }

    #[test]
    fn lanes_step_outward_and_stay_in_the_box() {
        assert_eq!(lane_radius(0), INFIELD + LANE / 2.);
        assert!(lane_radius(1) > lane_radius(0));
        assert_eq!(lane_radius(20), lane_radius(MAX_LANES - 1));
        let widest = INFIELD + LANE * MAX_LANES as f32;
        assert!(CY - widest >= 0. && CY + widest <= BOX_H);
        assert!(CX - STRAIGHT - widest >= 0. && CX + STRAIGHT + widest <= BOX_W);
    }

    #[test]
    fn standings_rank_by_progress_and_share_tied_places() {
        let table = standings(&[1.5, 2.25, 1.5, 0.5]);
        let runs: Vec<usize> = table.iter().map(|line| line.run).collect();
        assert_eq!(runs, [1, 0, 2, 3]);
        let places: Vec<usize> = table.iter().map(|line| line.place).collect();
        assert_eq!(places, [1, 2, 2, 4]);
        assert_eq!(table[0].gap, 0.);
        assert!((table[1].gap - 0.75).abs() < 1e-6);
        assert!(standings(&[]).is_empty());
    }

    #[test]
    fn laps_and_gaps_in_words() {
        assert_eq!(lap_label(0., 5), "Lap 1 of 5");
        assert_eq!(lap_label(2.4, 5), "Lap 3 of 5");
        assert_eq!(lap_label(4.99, 5), "Lap 5 of 5");
        assert_eq!(lap_label(5., 5), "Finished");
        assert_eq!(gap_label(0.), "Leader");
        assert_eq!(gap_label(0.421), "+0.42 laps");
        assert_eq!(gap_label(1.), "+1.00 lap");
        assert_eq!(spoken_gap(0.), "leading");
        assert_eq!(spoken_gap(1.25), "1.25 laps behind the leader");
    }

    #[test]
    fn keys_walk_the_standings_in_order() {
        let order = [2, 0, 1];
        assert_eq!(step_run(None, "down", &order), Some(Some(2)));
        assert_eq!(step_run(Some(2), "down", &order), Some(Some(0)));
        assert_eq!(step_run(Some(1), "right", &order), Some(Some(2)));
        assert_eq!(step_run(None, "up", &order), Some(Some(1)));
        assert_eq!(step_run(Some(0), "home", &order), Some(Some(2)));
        assert_eq!(step_run(Some(0), "end", &order), Some(Some(1)));
        assert_eq!(step_run(Some(0), "escape", &order), Some(None));
        assert_eq!(step_run(Some(0), "x", &order), None);
        assert_eq!(step_run(None, "down", &[]), None);
    }

    #[test]
    fn the_spoken_line_names_the_leader_or_the_picked_run() {
        let track = EvaluationTrack::new("t", "Nightly evals", 5)
            .run(TrackRun::new("Baseline", 2.4).score("0.71"))
            .run(TrackRun::new("Candidate", 3.1).score("0.78"));
        assert_eq!(
            track.spoken(None),
            "Nightly evals: 2 runs over 5 laps. Candidate leads, lap 4 of 5. \
             Arrow keys pick a run."
        );
        assert_eq!(
            track.spoken(Some(0)),
            "Baseline: place 2 of 2, lap 3 of 5, 0.70 laps behind the leader, score 0.71"
        );
        assert_eq!(
            EvaluationTrack::new("t", "Empty", 3).spoken(None),
            "Empty: no runs yet"
        );
    }
}
