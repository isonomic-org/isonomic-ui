// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Reach rings: what one thing can reach, hop by hop, as rings round it.
//!
//! The thing measured from sits in the middle. Whatever it reaches directly
//! sits on the first ring, whatever those reach on the next, each fanned out
//! round the node it is reached through. Affected nodes carry a ring in the
//! danger colour and their path is traced in it back to the middle, so the
//! size of the damage reads at a glance. Pointing at a node, or stepping with
//! the arrow keys, lights its path and names it.

use std::{cell::Cell, collections::HashSet, f32::consts::TAU, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{ActiveTheme as _, Icon, StyledExt as _, TextRole as _, h_flex};
use isonomic_ui::{
    AnyElement, App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{panel, shell, shell_header};

/// The rings are laid out on a 320 box and scaled to the width they get.
const BOX: f32 = 320.;
const MIDDLE: f32 = BOX / 2.;
/// Radius of the outermost ring, and of the disc in the middle.
const OUTER: f32 = 132.;
const HUB: f32 = 22.;
/// Share of a node's sector its children fan across, so neighbouring fans
/// never touch.
const SPREAD: f32 = 0.72;
/// How near the pointer must come to a node to pick it.
const NEAR: f32 = 14.;

/// One thing that can be reached.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachNode {
    id: SharedString,
    label: SharedString,
    kind: SharedString,
    parent: Option<SharedString>,
    affected: bool,
    indirect: bool,
    note: Option<SharedString>,
}

impl ReachNode {
    /// A node reached straight from the middle, of a kind such as "store".
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        kind: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: kind.into(),
            parent: None,
            affected: false,
            indirect: false,
            note: None,
        }
    }

    /// The id of the node this one is reached through.
    pub fn through(mut self, parent: impl Into<SharedString>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Mark it affected: it carries a ring and its path is traced.
    pub fn affected(mut self) -> Self {
        self.affected = true;
        self
    }

    /// Reached indirectly: its edge and ring are dashed.
    pub fn indirect(mut self) -> Self {
        self.indirect = true;
        self
    }

    /// A line shown under its name when it is picked.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// A node with its place on the 320 box.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    /// Its position in the list of nodes given.
    pub index: usize,
    /// Where it sits.
    pub at: (f32, f32),
    /// Where the node it is reached through sits: the middle for the first hop.
    pub from: (f32, f32),
    /// How many hops from the middle.
    pub depth: usize,
    /// Itself, then every node back to the middle, as positions in the list.
    pub path: Vec<usize>,
    /// It is affected, or leads to something affected.
    pub traced: bool,
}

/// Every node placed, in clockwise order ring by ring within each branch,
/// which is also the order the arrow keys step in.
#[derive(Debug, Clone, PartialEq)]
pub struct RingLayout {
    /// The nodes that could be placed.
    pub placed: Vec<PlacedNode>,
    /// How many rings there are.
    pub rings: usize,
    /// How far round the ring labels sit: between two branches, nearest nine
    /// o'clock, where no edge runs.
    pub label_turn: f32,
}

/// A point `turn` of the way round (0 at the top, clockwise), `radius` from
/// the middle of the box.
pub fn ring_point(turn: f32, radius: f32) -> (f32, f32) {
    let angle = turn * TAU - TAU / 4.;
    (MIDDLE + radius * angle.cos(), MIDDLE + radius * angle.sin())
}

/// Radius of ring `depth` when there are `rings` of them.
pub fn ring_radius(depth: usize, rings: usize) -> f32 {
    OUTER * depth as f32 / rings.max(1) as f32
}

/// Place nodes from the data alone: the first hop evenly round the middle,
/// every later hop fanned round the node it is reached through. A node whose
/// parent is not in the list hangs off the middle.
pub fn ring_layout(nodes: &[ReachNode]) -> RingLayout {
    let known = |id: &SharedString| nodes.iter().position(|node| &node.id == id);
    let parents: Vec<Option<usize>> = nodes
        .iter()
        .map(|node| node.parent.as_ref().and_then(known))
        .collect();
    let children = |parent: Option<usize>| -> Vec<usize> {
        (0..nodes.len())
            .filter(|ix| parents[*ix] == parent)
            .collect()
    };

    // Walk outward from the middle. Each entry is a node with its turn, the
    // share of the circle its own children may use, its depth and its path.
    let mut walked: Vec<(usize, f32, usize, Vec<usize>)> = Vec::new();
    let mut seen = HashSet::new();
    let first = children(None);
    let mut pending: Vec<(usize, f32, f32, usize, Vec<usize>)> = first
        .iter()
        .enumerate()
        .rev()
        .map(|(j, ix)| {
            let count = first.len() as f32;
            (*ix, j as f32 / count, 1. / count, 1, vec![*ix])
        })
        .collect();
    while let Some((ix, turn, span, depth, path)) = pending.pop() {
        if !seen.insert(ix) {
            continue;
        }
        let kids = children(Some(ix));
        let share = span * SPREAD;
        for (j, kid) in kids.iter().enumerate().rev() {
            let count = kids.len() as f32;
            let mut kid_path = vec![*kid];
            kid_path.extend(&path);
            pending.push((
                *kid,
                turn + share * ((j as f32 + 0.5) / count - 0.5),
                share / count,
                depth + 1,
                kid_path,
            ));
        }
        walked.push((ix, turn, depth, path));
    }

    let rings = walked.iter().map(|one| one.2).max().unwrap_or(1);
    let spot = |ix: usize| {
        walked
            .iter()
            .find(|one| one.0 == ix)
            .map(|one| ring_point(one.1, ring_radius(one.2, rings)))
            .unwrap_or((MIDDLE, MIDDLE))
    };
    let traced: HashSet<usize> = walked
        .iter()
        .filter(|one| nodes[one.0].affected)
        .flat_map(|one| one.3.iter().copied())
        .collect();
    let placed = walked
        .iter()
        .map(|(ix, _, depth, path)| PlacedNode {
            index: *ix,
            at: spot(*ix),
            from: path.get(1).map_or((MIDDLE, MIDDLE), |parent| spot(*parent)),
            depth: *depth,
            path: path.clone(),
            traced: traced.contains(ix),
        })
        .collect();
    let label_turn = (0..first.len())
        .map(|ix| (ix as f32 + 0.5) / first.len() as f32)
        .min_by(|a, b| (a - 0.75).abs().total_cmp(&(b - 0.75).abs()))
        .unwrap_or(0.75);
    RingLayout {
        placed,
        rings,
        label_turn,
    }
}

/// The placed node nearest a point of the box, if one is near enough.
pub fn node_near(placed: &[PlacedNode], x: f32, y: f32) -> Option<usize> {
    placed
        .iter()
        .enumerate()
        .map(|(ix, one)| (ix, (one.at.0 - x).hypot(one.at.1 - y)))
        .filter(|(_, distance)| *distance <= NEAR)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(ix, _)| ix)
}

/// A distance in words: "1 hop", "3 hops".
pub fn hops(depth: usize) -> String {
    if depth == 1 {
        "1 hop".into()
    } else {
        format!("{depth} hops")
    }
}

/// One node read aloud.
pub fn node_reading(node: &ReachNode, kind: &str, depth: usize) -> String {
    format!(
        "{}, {kind}, {}{}{}{}",
        node.label,
        hops(depth),
        if node.affected { ", affected" } else { "" },
        if node.indirect { ", indirect" } else { "" },
        node.note
            .as_ref()
            .map(|note| format!(". {note}"))
            .unwrap_or_default()
    )
}

/// A kind's name when none was given: its key with a capital.
fn kind_name(key: &str) -> String {
    let mut letters = key.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

fn step_node(now: Option<usize>, key: &str, count: usize) -> Option<Option<usize>> {
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

fn circle(centre: Point<Pixels>, radius: f32) -> Vec<Point<Pixels>> {
    let steps = ((radius * 1.6) as usize).clamp(12, 96);
    (0..steps)
        .map(|step| {
            let angle = step as f32 / steps as f32 * TAU;
            point(
                centre.x + px(radius * angle.cos()),
                centre.y + px(radius * angle.sin()),
            )
        })
        .collect()
}

fn paint_disc(window: &mut Window, centre: Point<Pixels>, radius: f32, color: Hsla) {
    let mut disc = PathBuilder::fill();
    disc.add_polygon(&circle(centre, radius), true);
    if let Ok(disc) = disc.build() {
        window.paint_path(disc, color);
    }
}

fn paint_ring(
    window: &mut Window,
    centre: Point<Pixels>,
    radius: f32,
    wide: f32,
    dash: Option<[f32; 2]>,
    color: Hsla,
) {
    let mut ring = PathBuilder::stroke(px(wide));
    if let Some([on, off]) = dash {
        ring = ring.dash_array(&[px(on), px(off)]);
    }
    ring.add_polygon(&circle(centre, radius), true);
    if let Ok(ring) = ring.build() {
        window.paint_path(ring, color);
    }
}

type SelectHandler = Rc<dyn Fn(&ReachNode, &mut Window, &mut App)>;

/// What one thing can reach, as rings of hops round it.
#[derive(IntoElement)]
pub struct ReachRings {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    centre: SharedString,
    initials: Option<SharedString>,
    icon: Option<Icon>,
    nodes: Vec<ReachNode>,
    kinds: Vec<(SharedString, SharedString, Option<Hsla>)>,
    aside: Option<AnyElement>,
    on_select: Option<SelectHandler>,
}

impl ReachRings {
    /// Create the rings round the thing everything is measured from.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        centre: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            centre: centre.into(),
            initials: None,
            icon: None,
            nodes: Vec::new(),
            kinds: Vec::new(),
            aside: None,
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Letters for the middle. By default the first two of its name.
    pub fn initials(mut self, initials: impl Into<SharedString>) -> Self {
        self.initials = Some(initials.into());
        self
    }

    /// An icon for the middle, instead of letters.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Everything that can be reached.
    pub fn nodes(mut self, nodes: impl IntoIterator<Item = ReachNode>) -> Self {
        self.nodes = nodes.into_iter().collect();
        self
    }

    /// Name a kind for the legend. Kinds take the data colours in the order
    /// they are named, then in the order they first appear.
    pub fn kind(mut self, key: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        self.kinds.push((key.into(), name.into(), None));
        self
    }

    /// Name a kind and give it a colour of its own.
    pub fn kind_colored(
        mut self,
        key: impl Into<SharedString>,
        name: impl Into<SharedString>,
        color: impl Into<Hsla>,
    ) -> Self {
        self.kinds
            .push((key.into(), name.into(), Some(color.into())));
        self
    }

    /// Whatever belongs at the far end of the header: a status, a count.
    pub fn aside(mut self, aside: impl IntoElement) -> Self {
        self.aside = Some(aside.into_any_element());
        self
    }

    /// Run with the node the reader points at or steps to.
    pub fn on_select(
        mut self,
        handler: impl Fn(&ReachNode, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for ReachRings {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ReachRings {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &'static str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let brand = theme.chart_1;
        let surface = theme.background;
        let raised = theme.popover;
        let mono = theme.mono_font_family.clone();
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_5,
            theme.chart_3,
            theme.chart_4,
        ];

        let layout = ring_layout(&self.nodes);
        let count = layout.placed.len();
        let rings = layout.rings;
        let affected = layout
            .placed
            .iter()
            .filter(|one| self.nodes[one.index].affected)
            .count();

        // Kinds in legend order: the named ones in use, then the rest as met.
        let mut kinds: Vec<(SharedString, SharedString, Option<Hsla>)> = self
            .kinds
            .iter()
            .filter(|(key, ..)| {
                layout
                    .placed
                    .iter()
                    .any(|one| &self.nodes[one.index].kind == key)
            })
            .cloned()
            .collect();
        for one in &layout.placed {
            let key = &self.nodes[one.index].kind;
            if !kinds.iter().any(|(known, ..)| known == key) {
                kinds.push((key.clone(), kind_name(key).into(), None));
            }
        }
        let kinds: Vec<(SharedString, SharedString, Hsla)> = kinds
            .into_iter()
            .enumerate()
            .map(|(ix, (key, name, color))| {
                (key, name, color.unwrap_or(palette[ix % palette.len()]))
            })
            .collect();
        let kind_of = |node: &ReachNode| {
            kinds
                .iter()
                .find(|(key, ..)| key == &node.kind)
                .map(|(_, name, color)| (name.clone(), *color))
                .unwrap_or((node.kind.clone(), quiet))
        };

        // The node picked now, and the last one, so its path fades out too.
        let active = window.use_keyed_state(part("active"), cx, |_, _| (None::<usize>, 0_usize));
        let (picked, last) = *active.read(cx);
        let picked = picked.filter(|ix| *ix < count);
        let lit: HashSet<usize> = layout
            .placed
            .get(picked.unwrap_or(last))
            .map(|one| one.path.iter().copied().collect())
            .unwrap_or_default();
        let dimming = transition(
            part("dim"),
            if picked.is_some() { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(150)),
            window,
            cx,
        );
        let seen = window
            .use_keyed_state(part("bounds"), cx, |_, _| {
                Rc::new(Cell::new(Bounds::<Pixels>::default()))
            })
            .read(cx)
            .clone();
        let width = match f32::from(seen.get().size.width) {
            width if width > 0. => width,
            _ => BOX,
        };
        let appear_time = Duration::from_secs_f32(0.3 + 0.12 * rings as f32 + 0.12);
        let appeared = Presence::new(part("appear"), true)
            .transition(Transition::new(appear_time).ease(|time| time))
            .sample(window, cx)
            .progress;
        let elapsed = appeared * appear_time.as_secs_f32();
        // Ring by ring, then clockwise within a ring.
        let shown = move |delay: f32| {
            let linear = ((elapsed - delay) / 0.3).clamp(0., 1.);
            1. - (1. - linear).powi(3)
        };
        let node_delay = |one: &PlacedNode, ix: usize| {
            (one.depth - 1) as f32 * 0.12 + (ix as f32 * 0.012).min(0.12)
        };

        // What the canvas needs of each node.
        struct Mark {
            at: (f32, f32),
            from: (f32, f32),
            first: bool,
            traced: bool,
            affected: bool,
            indirect: bool,
            lit: bool,
            picked: bool,
            shown: f32,
            color: Hsla,
        }
        let marks: Vec<Mark> = layout
            .placed
            .iter()
            .enumerate()
            .map(|(ix, one)| {
                let node = &self.nodes[one.index];
                Mark {
                    at: one.at,
                    from: one.from,
                    first: one.depth == 1,
                    traced: one.traced,
                    affected: node.affected,
                    indirect: node.indirect,
                    lit: lit.contains(&one.index),
                    picked: picked == Some(ix),
                    shown: shown(node_delay(one, ix)),
                    color: kind_of(node).1,
                }
            })
            .collect();
        let ring_shown: Vec<f32> = (0..rings).map(|ring| shown(ring as f32 * 0.12)).collect();
        let hub_shown = shown(0.);
        let paint_seen = seen.clone();
        let rings_canvas = canvas(
            move |bounds, window, _| {
                if paint_seen.get() != bounds {
                    paint_seen.set(bounds);
                    window.request_animation_frame();
                }
            },
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / BOX;
                let at = |(x, y): (f32, f32)| {
                    point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                };
                let middle = at((MIDDLE, MIDDLE));
                for (ring, shown) in ring_shown.iter().enumerate() {
                    paint_ring(
                        window,
                        middle,
                        ring_radius(ring + 1, rings) * scale,
                        1.,
                        Some([2., 4.]),
                        ink.opacity(0.15 * shown),
                    );
                }
                for mark in &marks {
                    let wide = match (mark.traced, mark.first) {
                        (true, true) => 1.75,
                        (true, false) => 1.25,
                        (false, _) => 1.,
                    };
                    let fade = if mark.lit { 1. } else { 1. - 0.75 * dimming };
                    let color = if mark.traced {
                        danger.opacity(fade)
                    } else if mark.lit {
                        ink.opacity(0.15 + 0.45 * dimming)
                    } else {
                        ink.opacity(0.15 * fade)
                    };
                    let mut edge = PathBuilder::stroke(px(wide * scale.max(0.75)));
                    if mark.indirect {
                        edge = edge.dash_array(&[px(3.), px(3.)]);
                    }
                    edge.move_to(at(mark.from));
                    edge.line_to(at(mark.at));
                    if let Ok(edge) = edge.build() {
                        window.paint_path(edge, color.opacity(color.a * mark.shown));
                    }
                }
                let grown = (0.9 + 0.1 * hub_shown) * scale;
                if affected > 0 {
                    paint_ring(
                        window,
                        middle,
                        (HUB + 4.) * grown,
                        1.75,
                        None,
                        danger.opacity(hub_shown),
                    );
                }
                paint_disc(window, middle, HUB * grown, brand.opacity(hub_shown));
                for mark in &marks {
                    let radius = if mark.first { 9. } else { 6.5 };
                    let grown = (0.9 + 0.1 * mark.shown) * scale;
                    // Off the picked path nodes step back; with nothing
                    // picked, the ones clear of the damage do.
                    let rest = if affected > 0 && !mark.traced {
                        0.6
                    } else {
                        1.
                    };
                    let focus = if mark.lit { 1. } else { 0.25 };
                    let strength = (rest + (focus - rest) * dimming) * mark.shown;
                    let centre = at(mark.at);
                    if mark.affected {
                        paint_ring(
                            window,
                            centre,
                            (radius + 3.5) * grown,
                            1.5,
                            mark.indirect.then_some([2., 2.]),
                            danger.opacity(strength),
                        );
                    } else if mark.picked {
                        paint_ring(
                            window,
                            centre,
                            (radius + 3.5) * grown,
                            1.5,
                            None,
                            ink.opacity(0.4 * strength),
                        );
                    }
                    paint_disc(
                        window,
                        centre,
                        radius * grown,
                        mark.color.opacity(mark.color.a * strength),
                    );
                }
            },
        )
        .w_full()
        .aspect_ratio(1.);

        // Text laid over a point of the box.
        let over = |(x, y): (f32, f32)| {
            div()
                .absolute()
                .left(relative(x / BOX))
                .top(relative(y / BOX))
                .w(px(0.))
                .h(px(0.))
                .flex()
                .items_center()
                .justify_center()
        };
        let ring_labels = (1..=rings).map(|ring| {
            over(ring_point(layout.label_turn, ring_radius(ring, rings)))
                .opacity(shown((ring - 1) as f32 * 0.12))
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .px(px(3.))
                        .rounded(px(4.))
                        .bg(surface)
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(quiet)
                        .child(hops(ring)),
                )
        });
        let hub = over((MIDDLE, MIDDLE))
            .opacity(hub_shown)
            .text_color(surface)
            .child(match self.icon {
                Some(icon) => icon.size(px(18.)).into_any_element(),
                None => div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(13.))
                    .line_height(px(16.))
                    .font_medium()
                    .child(self.initials.clone().unwrap_or_else(|| {
                        self.centre
                            .chars()
                            .take(2)
                            .collect::<String>()
                            .to_uppercase()
                            .into()
                    }))
                    .into_any_element(),
            });

        let tip = picked.map(|ix| {
            let one = &layout.placed[ix];
            let node = &self.nodes[one.index];
            let (kind, _) = kind_of(node);
            let scale = width / BOX;
            // Opens away from the nearest edge, so it stays inside the card.
            let (leftward, upward) = (one.at.0 >= MIDDLE, one.at.1 >= MIDDLE);
            let room = if leftward { one.at.0 } else { BOX - one.at.0 };
            div()
                .absolute()
                .map(|this| {
                    if leftward {
                        this.right(px((BOX - one.at.0) * scale + 12.))
                    } else {
                        this.left(px(one.at.0 * scale + 12.))
                    }
                })
                .map(|this| {
                    if upward {
                        this.bottom(px((BOX - one.at.1) * scale + 4.))
                    } else {
                        this.top(px(one.at.1 * scale + 4.))
                    }
                })
                .max_w(px((room * scale - 12.).clamp(96., 208.)))
                .px_3()
                .py_2()
                .rounded(px(12.))
                .bg(raised)
                .shadow_md()
                .text_caption()
                .child(
                    div()
                        .font_family(mono.clone())
                        .font_medium()
                        .child(node.label.clone()),
                )
                .when_some(node.note.clone(), |this, note| {
                    this.child(div().mt(px(2.)).text_color(quiet).child(note))
                })
                .child(
                    h_flex()
                        .mt(px(2.))
                        .flex_wrap()
                        .text_color(quiet)
                        .child(format!("{kind} · {}", hops(one.depth)))
                        .when(node.affected, |this| {
                            this.child(div().text_color(danger).child(" · Affected"))
                        }),
                )
        });

        let spoken = match picked {
            Some(ix) => {
                let one = &layout.placed[ix];
                let node = &self.nodes[one.index];
                node_reading(node, &kind_of(node).0, one.depth)
            }
            None => format!(
                "{}: {affected} of {count} reachable from {} affected. Arrow keys read each node.",
                self.title, self.centre
            ),
        };

        let legend = h_flex()
            .mt_3()
            .flex_wrap()
            .justify_center()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .children(kinds.iter().map(|(_, name, color)| {
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded_full().bg(*color))
                    .child(name.clone())
            }))
            .when(affected > 0, |this| {
                this.child(
                    h_flex()
                        .gap(px(6.))
                        .child(
                            div()
                                .size(px(10.))
                                .rounded_full()
                                .border(px(1.5))
                                .border_color(danger),
                        )
                        .child("Affected"),
                )
            });

        let set = |state: &isonomic_ui::Entity<(Option<usize>, usize)>,
                   next: Option<usize>,
                   cx: &mut App| {
            state.update(cx, |state, cx| {
                *state = (next, next.unwrap_or(state.1));
                cx.notify();
            });
        };
        let hover_state = active.clone();
        let hover_select = self.on_select.clone();
        let hover_nodes = self.nodes.clone();
        let hover_placed = layout.placed.clone();
        let leave_state = active.clone();
        let key_state = active;
        let key_select = self.on_select.clone();
        let key_nodes = self.nodes.clone();
        let key_placed = layout.placed.clone();
        shell(cx)
            .id(self.id.clone())
            .max_w(px(448.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartNetwork)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                self.aside,
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        div()
                            .id(part("rings"))
                            .test_support()
                            .relative()
                            .mx_auto()
                            .w_full()
                            .max_w(px(BOX))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                                let bounds = seen.get();
                                let scale = f32::from(bounds.size.width) / BOX;
                                if scale <= 0. {
                                    return;
                                }
                                let next = node_near(
                                    &hover_placed,
                                    f32::from(event.position.x - bounds.left()) / scale,
                                    f32::from(event.position.y - bounds.top()) / scale,
                                );
                                if hover_state.read(cx).0 != next {
                                    set(&hover_state, next, cx);
                                    if let (Some(select), Some(ix)) = (&hover_select, next) {
                                        select(&hover_nodes[hover_placed[ix].index], window, cx);
                                    }
                                }
                            })
                            .on_hover(move |over, _, cx| {
                                if !*over && leave_state.read(cx).0.is_some() {
                                    set(&leave_state, None, cx);
                                }
                            })
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let now = key_state.read(cx).0;
                                let Some(next) =
                                    step_node(now, event.keystroke.key.as_str(), count)
                                else {
                                    return;
                                };
                                set(&key_state, next, cx);
                                if let (Some(select), Some(ix)) = (&key_select, next) {
                                    select(&key_nodes[key_placed[ix].index], window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .child(rings_canvas)
                            .children(ring_labels)
                            .child(hub)
                            .children(tip),
                    )
                    .child(legend),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depot() -> Vec<ReachNode> {
        vec![
            ReachNode::new("a", "Alpha", "store"),
            ReachNode::new("b", "Beta", "store"),
            ReachNode::new("c", "Gamma", "person"),
            ReachNode::new("d", "Delta", "person"),
            ReachNode::new("a1", "Alpha one", "shelf").through("a"),
            ReachNode::new("a2", "Alpha two", "shelf")
                .through("a")
                .affected()
                .indirect()
                .note("Left unlocked"),
            ReachNode::new("lost", "Lost", "shelf").through("nowhere"),
        ]
    }

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
    }

    #[test]
    fn the_first_hop_is_spread_evenly_and_children_fan_round_their_parent() {
        let layout = ring_layout(&depot());
        assert_eq!(layout.rings, 2);
        // Five hang off the middle: the four named and the one with no known parent.
        let first: Vec<usize> = layout
            .placed
            .iter()
            .filter(|one| one.depth == 1)
            .map(|one| one.index)
            .collect();
        assert_eq!(first, [0, 1, 2, 3, 6]);
        // The first sits at the top of the inner ring.
        assert!(close(layout.placed[0].at, (160., 160. - 66.)));
        assert!(close(layout.placed[0].from, (160., 160.)));
        // Its children come straight after it, either side of its direction.
        assert_eq!(layout.placed[1].index, 4);
        assert_eq!(layout.placed[2].index, 5);
        let (left, right) = (layout.placed[1].at, layout.placed[2].at);
        assert!(left.0 < 160. && right.0 > 160.);
        assert!((left.1 - right.1).abs() < 0.01);
        assert!(close(layout.placed[1].from, layout.placed[0].at));
        assert_eq!(layout.placed[2].path, [5, 0]);
    }

    #[test]
    fn the_path_to_an_affected_node_is_traced() {
        let layout = ring_layout(&depot());
        let traced: Vec<usize> = layout
            .placed
            .iter()
            .filter(|one| one.traced)
            .map(|one| one.index)
            .collect();
        assert_eq!(traced, [0, 5]);
        // A loop of parents is never reached from the middle, and never placed.
        let looped = [
            ReachNode::new("x", "X", "store").through("y"),
            ReachNode::new("y", "Y", "store").through("x"),
        ];
        let layout = ring_layout(&looped);
        assert!(layout.placed.is_empty());
        assert_eq!(layout.rings, 1);
    }

    #[test]
    fn ring_labels_sit_between_branches_near_nine_o_clock() {
        // Four branches at 12, 3, 6 and 9: the gaps are the diagonals.
        let four: Vec<ReachNode> = (0..4)
            .map(|ix| ReachNode::new(format!("n{ix}"), "N", "store"))
            .collect();
        assert_eq!(ring_layout(&four).label_turn, 0.625);
        assert_eq!(ring_layout(&[]).label_turn, 0.75);
        assert_eq!(ring_radius(1, 3), 44.);
        assert_eq!(ring_radius(3, 3), 132.);
    }

    #[test]
    fn the_nearest_node_is_picked_when_close_enough() {
        let layout = ring_layout(&depot());
        let top = layout.placed[0].at;
        assert_eq!(node_near(&layout.placed, top.0 + 5., top.1 - 5.), Some(0));
        assert_eq!(node_near(&layout.placed, top.0 + 15., top.1), None);
        assert_eq!(node_near(&layout.placed, 160., 160.), None);
        // Between two nodes the nearer one wins.
        let (a, b) = (layout.placed[1].at, layout.placed[2].at);
        let between = (a.0 + (b.0 - a.0) * 0.6, a.1);
        if (b.0 - a.0) * 0.4 <= 14. {
            assert_eq!(node_near(&layout.placed, between.0, between.1), Some(2));
        }
    }

    #[test]
    fn nodes_are_read_with_their_distance_and_flags() {
        let nodes = depot();
        assert_eq!(hops(1), "1 hop");
        assert_eq!(hops(2), "2 hops");
        assert_eq!(node_reading(&nodes[0], "Store", 1), "Alpha, Store, 1 hop");
        assert_eq!(
            node_reading(&nodes[5], "Shelf", 2),
            "Alpha two, Shelf, 2 hops, affected, indirect. Left unlocked"
        );
        assert_eq!(kind_name("shelf"), "Shelf");
        assert_eq!(step_node(None, "down", 3), Some(Some(0)));
        assert_eq!(step_node(Some(0), "up", 3), Some(Some(2)));
        assert_eq!(step_node(Some(1), "escape", 3), Some(None));
    }
}
