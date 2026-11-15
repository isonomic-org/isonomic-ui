//! Agent network graph: assistants, people and tools as nodes, joined by how much they talk.
//!
//! Nodes sit where the owner places them, or in a ring around the first
//! node when no place is given. Lines between them grow thicker with the
//! number of messages, and lines in use right now are drawn in the accent
//! colour. Selecting a node brings its lines forward, fades the rest and
//! lists who it talks to underneath.
//!
//! Keyboard: the graph is a Tab stop. Left and Right step through the
//! nodes, and Escape clears the selection.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    PathBuilder, RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, canvas, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkNodeKind {
    /// An assistant.
    Agent,
    /// A person.
    Person,
    /// A tool or outside service.
    Tool,
}

impl NetworkNodeKind {
    fn icon(self) -> IconName {
        match self {
            Self::Agent => IconName::Bot,
            Self::Person => IconName::User,
            Self::Tool => IconName::SquareTerminal,
        }
    }
}

/// One node.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkNode {
    /// Identifies it.
    pub id: SharedString,
    /// Its name.
    pub label: SharedString,
    /// What it stands for.
    pub kind: NetworkNodeKind,
    /// The state it's in, as a tone.
    pub tone: Tone,
    /// Where it sits, from 0 to 1 across and down, if placed.
    pub at: Option<(f32, f32)>,
}

impl NetworkNode {
    /// Create a node with no particular state.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        kind: NetworkNodeKind,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            tone: Tone::Neutral,
            at: None,
        }
    }

    /// Set its state.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Place it, from 0 to 1 across and down.
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.at = Some((x.clamp(0., 1.), y.clamp(0., 1.)));
        self
    }
}

/// A line between two nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkLink {
    /// One end.
    pub from: SharedString,
    /// The other end.
    pub to: SharedString,
    /// Messages sent along it.
    pub messages: u32,
    /// Whether it's in use right now.
    pub live: bool,
}

impl NetworkLink {
    /// Create a quiet line.
    pub fn new(from: impl Into<SharedString>, to: impl Into<SharedString>, messages: u32) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            messages,
            live: false,
        }
    }

    /// Mark it as in use right now.
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }
}

/// Where each node sits, from 0 to 1. Unplaced nodes go in a ring around
/// the first node, which goes in the middle when it isn't placed either.
pub fn place_nodes(nodes: &[NetworkNode]) -> Vec<(f32, f32)> {
    let ring: Vec<usize> = (1..nodes.len())
        .filter(|ix| nodes[*ix].at.is_none())
        .collect();
    let mut spots = vec![(0.5, 0.5); nodes.len()];
    for (ix, node) in nodes.iter().enumerate() {
        if let Some(at) = node.at {
            spots[ix] = at;
        }
    }
    let count = ring.len().max(1) as f32;
    for (n, ix) in ring.into_iter().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + n as f32 / count * std::f32::consts::TAU;
        spots[ix] = (0.5 + 0.38 * angle.cos(), 0.5 + 0.36 * angle.sin());
    }
    spots
}

/// Who a node talks to and how much, busiest first.
pub fn neighbours<'a>(links: &'a [NetworkLink], id: &str) -> Vec<(&'a SharedString, u32, bool)> {
    let mut found: Vec<_> = links
        .iter()
        .filter_map(|link| {
            if link.from.as_ref() == id {
                Some((&link.to, link.messages, link.live))
            } else if link.to.as_ref() == id {
                Some((&link.from, link.messages, link.live))
            } else {
                None
            }
        })
        .collect();
    found.sort_by_key(|(_, messages, _)| std::cmp::Reverse(*messages));
    found
}

/// The lines at a node, in words, so the drawing is not the only place they show.
pub fn link_words(nodes: &[NetworkNode], links: &[NetworkLink], id: &str) -> String {
    let talks = neighbours(links, id);
    if talks.is_empty() {
        return "No links yet".into();
    }
    let each: Vec<String> = talks
        .iter()
        .map(|(other, messages, live)| {
            let name = nodes
                .iter()
                .find(|n| &n.id == *other)
                .map(|n| n.label.to_string())
                .unwrap_or_else(|| other.to_string());
            let unit = if *messages == 1 {
                "message"
            } else {
                "messages"
            };
            format!(
                "{name}, {messages} {unit}{}",
                if *live { ", in use now" } else { "" }
            )
        })
        .collect();
    format!("Linked to {}", each.join("; "))
}

type NodeHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;

/// Nodes joined by lines that grow with how much they talk.
#[derive(IntoElement)]
pub struct AgentNetworkGraph {
    id: ElementId,
    style: StyleRefinement,
    nodes: Vec<NetworkNode>,
    links: Vec<NetworkLink>,
    selected: Option<SharedString>,
    height: f32,
    on_select: Option<NodeHandler>,
}

impl AgentNetworkGraph {
    /// Create a graph.
    pub fn new(id: impl Into<ElementId>, nodes: Vec<NetworkNode>, links: Vec<NetworkLink>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            nodes,
            links,
            selected: None,
            height: 340.,
            on_select: None,
        }
    }

    /// The selected node.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Set the drawing's height in pixels. 340 by default.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(160.);
        self
    }

    /// Run with the node picked, or `None` when the selection is cleared.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for AgentNetworkGraph {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

const NODE: f32 = 40.;

impl RenderOnce for AgentNetworkGraph {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius_lg = theme.radius_lg;

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let spots = place_nodes(&self.nodes);
        let index = |id: &SharedString| self.nodes.iter().position(|n| &n.id == id);
        let busiest = self
            .links
            .iter()
            .map(|l| l.messages)
            .max()
            .unwrap_or(1)
            .max(1);
        let selected = self.selected.clone().filter(|id| index(id).is_some());

        // Lines to draw: (from spot, to spot, width, colour).
        let strokes: Vec<((f32, f32), (f32, f32), f32, Hsla)> = self
            .links
            .iter()
            .filter_map(|link| {
                let (a, b) = (index(&link.from)?, index(&link.to)?);
                let touches = selected
                    .as_ref()
                    .is_none_or(|id| &link.from == id || &link.to == id);
                let base = if link.live { accent } else { quiet };
                let color = if touches {
                    base.opacity(if link.live { 0.85 } else { 0.45 })
                } else {
                    base.opacity(0.1)
                };
                let width = 1. + 4. * link.messages as f32 / busiest as f32;
                Some((spots[a], spots[b], width, color))
            })
            .collect();

        let labels: Vec<_> = self
            .links
            .iter()
            .filter_map(|link| {
                let (a, b) = (index(&link.from)?, index(&link.to)?);
                let touches = selected
                    .as_ref()
                    .is_none_or(|id| &link.from == id || &link.to == id);
                let mid = (
                    (spots[a].0 + spots[b].0) / 2.,
                    (spots[a].1 + spots[b].1) / 2.,
                );
                Some(
                    div()
                        .absolute()
                        .left(relative(mid.0))
                        .top(relative(mid.1))
                        .ml(px(-14.))
                        .mt(px(-8.))
                        .w(px(28.))
                        .h(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(fill)
                        .border_1()
                        .border_color(if link.live { accent } else { line })
                        .text_xs()
                        .text_color(if link.live { accent } else { quiet })
                        .when(!touches, |this| this.opacity(0.25))
                        .child(link.messages.to_string()),
                )
            })
            .collect();

        let node_views: Vec<_> = self
            .nodes
            .iter()
            .zip(&spots)
            .map(|(node, spot)| {
                let is_selected = selected.as_ref() == Some(&node.id);
                let near = selected.as_ref().is_none_or(|id| {
                    id == &node.id
                        || self.links.iter().any(|l| {
                            (&l.from == id && l.to == node.id) || (&l.to == id && l.from == node.id)
                        })
                });
                let tone_color = match node.tone {
                    Tone::Neutral => ink,
                    tone => tone.color(cx),
                };
                let ring = match node.tone {
                    Tone::Neutral => line,
                    tone => tone.color(cx),
                };
                let select = self.on_select.clone();
                let id = node.id.clone();
                v_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("node-{}", node.id).into(),
                    ))
                    .absolute()
                    .left(relative(spot.0))
                    .top(relative(spot.1))
                    .ml(px(-48.))
                    .mt(px(-NODE / 2.))
                    .w(px(96.))
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .when(!near, |this| this.opacity(0.35))
                    .role(Role::Button)
                    .aria_label(format!(
                        "{}, {} links",
                        node.label,
                        neighbours(&self.links, &node.id).len()
                    ))
                    .aria_description(link_words(&self.nodes, &self.links, &node.id))
                    .aria_selected(is_selected)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        if let Some(select) = &select {
                            select(Some(&id), window, cx);
                        }
                    })
                    .child(
                        div()
                            .size(px(NODE))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(if node.kind == NetworkNodeKind::Tool {
                                px(10.)
                            } else {
                                px(NODE / 2.)
                            })
                            .bg(fill)
                            .border_2()
                            .border_color(if is_selected { accent } else { ring })
                            .when(is_selected, |this| this.bg(accent.opacity(0.1)))
                            .child(Icon::new(node.kind.icon()).small().text_color(tone_color)),
                    )
                    .child(
                        div()
                            .max_w_full()
                            .px_1()
                            .rounded(px(4.))
                            .bg(fill.opacity(0.85))
                            .truncate()
                            .text_xs()
                            .text_center()
                            .when(is_selected, |this| this.font_medium())
                            .child(node.label.clone()),
                    )
            })
            .collect();

        let key_down = {
            let ids: Vec<SharedString> = self.nodes.iter().map(|n| n.id.clone()).collect();
            let selected = selected.clone();
            let select = self.on_select.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.modifiers.modified() || ids.is_empty() {
                    return;
                }
                let at = selected
                    .as_ref()
                    .and_then(|id| ids.iter().position(|i| i == id));
                let next = match (event.keystroke.key.as_str(), at) {
                    ("right" | "down", None) => Some(0),
                    ("left" | "up", None) => Some(ids.len() - 1),
                    ("right" | "down", Some(at)) => Some((at + 1) % ids.len()),
                    ("left" | "up", Some(at)) => Some((at + ids.len() - 1) % ids.len()),
                    ("escape", Some(_)) => None,
                    _ => return,
                };
                if let Some(select) = &select {
                    select(next.map(|ix| &ids[ix]), window, cx);
                }
                cx.stop_propagation();
            }
        };

        let detail = selected.as_ref().and_then(|id| {
            let node = self.nodes.iter().find(|n| &n.id == id)?;
            let talks = neighbours(&self.links, id);
            let names: Vec<String> = talks
                .iter()
                .map(|(other, messages, live)| {
                    let name = self
                        .nodes
                        .iter()
                        .find(|n| &n.id == *other)
                        .map(|n| n.label.to_string())
                        .unwrap_or_else(|| other.to_string());
                    format!("{name} ({messages}{})", if *live { ", now" } else { "" })
                })
                .collect();
            Some(format!(
                "{} talks to {}: {}",
                node.label,
                talks.len(),
                if names.is_empty() {
                    "nobody yet".into()
                } else {
                    names.join(", ")
                }
            ))
        });
        let total: u32 = self.links.iter().map(|l| l.messages).sum();
        let live = self.links.iter().filter(|l| l.live).count();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(760.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
                    .child(Icon::new(IconName::Network).small())
                    .child(div().flex_1().child(format!(
                        "{} nodes, {} links, {total} messages",
                        self.nodes.len(),
                        self.links.len()
                    )))
                    .when(live > 0, |this| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .text_color(accent)
                                .child(div().size(px(6.)).rounded_full().bg(accent))
                                .child(format!("{live} in use now")),
                        )
                    }),
            )
            .child(
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "canvas".into(),
                    ))
                    .track_focus(&focus)
                    .on_key_down(key_down)
                    .role(Role::Group)
                    .aria_label("Network")
                    .relative()
                    .m_1()
                    .h(px(self.height))
                    .rounded(radius_lg)
                    .bg(chip.opacity(0.25))
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    // A click on empty space clears the selection. It sits on
                    // its own layer, which has no role, so the group is not
                    // announced as something to press.
                    .child(
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                "backdrop".into(),
                            ))
                            .absolute()
                            .inset_0()
                            .size_full()
                            .on_click({
                                let select = self.on_select.clone();
                                move |_, window, cx| {
                                    if let Some(select) = &select {
                                        select(None, window, cx);
                                    }
                                }
                            }),
                    )
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let at = |(x, y): (f32, f32)| {
                                    point(
                                        bounds.origin.x + bounds.size.width * x,
                                        bounds.origin.y + bounds.size.height * y,
                                    )
                                };
                                for (from, to, width, color) in &strokes {
                                    let mut path = PathBuilder::stroke(px(*width));
                                    path.move_to(at(*from));
                                    path.line_to(at(*to));
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, *color);
                                    }
                                }
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .children(labels)
                    .children(node_views),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
                    .child(
                        detail.unwrap_or_else(|| "Select a node to see who it talks to.".into()),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unplaced_nodes_ring_the_first_one() {
        let nodes = vec![
            NetworkNode::new("hub", "Hub", NetworkNodeKind::Agent),
            NetworkNode::new("a", "A", NetworkNodeKind::Tool),
            NetworkNode::new("b", "B", NetworkNodeKind::Tool).at(0.1, 0.9),
            NetworkNode::new("c", "C", NetworkNodeKind::Person),
        ];
        let spots = place_nodes(&nodes);
        assert_eq!(spots[0], (0.5, 0.5));
        assert_eq!(spots[2], (0.1, 0.9));
        assert!(
            (spots[1].1 - 0.14).abs() < 0.01,
            "the first ring node sits at the top"
        );
        assert!(spots[3].1 > 0.5, "the second sits opposite");
    }

    #[test]
    fn neighbours_are_busiest_first_in_both_directions() {
        let links = vec![
            NetworkLink::new("a", "hub", 3),
            NetworkLink::new("hub", "b", 9).live(),
            NetworkLink::new("a", "b", 1),
        ];
        let found: Vec<_> = neighbours(&links, "hub")
            .into_iter()
            .map(|(id, n, live)| (id.to_string(), n, live))
            .collect();
        assert_eq!(
            found,
            [("b".to_string(), 9, true), ("a".to_string(), 3, false)]
        );
    }

    #[test]
    fn link_words_name_each_line_busiest_first() {
        let nodes = vec![
            NetworkNode::new("hub", "Planner", NetworkNodeKind::Agent),
            NetworkNode::new("a", "Writer", NetworkNodeKind::Agent),
        ];
        let links = vec![
            NetworkLink::new("a", "hub", 1),
            NetworkLink::new("hub", "b", 9).live(),
        ];
        assert_eq!(
            link_words(&nodes, &links, "hub"),
            "Linked to b, 9 messages, in use now; Writer, 1 message"
        );
        assert_eq!(link_words(&nodes, &[], "hub"), "No links yet");
    }
}
