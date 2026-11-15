//! Agent hierarchy cards: a lead assistant and the helpers it handed work to, as a tree of cards.
//!
//! Each card says who the assistant is, what it's doing now, how many steps
//! it has taken and what it has spent. Helpers hang below the card that
//! started them, joined by lines, and any branch can be folded away. The
//! owner keeps which branches are folded and which card is selected.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    v_flex,
};
use isonomic_ui::{
    Role,
    Animation, AnimationExt as _, AnyElement, App, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Transformation, Window, div, percentage,
    prelude::FluentBuilder as _, px,
};

use crate::common::{Tone, resting_shadow};

/// What an assistant is doing.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    /// Started, nothing to do yet.
    Idle,
    /// Working on something.
    Working(SharedString),
    /// Waiting on someone or something.
    Waiting(SharedString),
    /// Finished.
    Done,
    /// Stopped with a problem.
    Failed(SharedString),
}

impl AgentState {
    fn tone(&self) -> Tone {
        match self {
            Self::Idle => Tone::Neutral,
            Self::Working(_) => Tone::Info,
            Self::Waiting(_) => Tone::Warning,
            Self::Done => Tone::Success,
            Self::Failed(_) => Tone::Danger,
        }
    }

    fn words(&self) -> SharedString {
        match self {
            Self::Idle => "Idle".into(),
            Self::Working(what) => what.clone(),
            Self::Waiting(on) => format!("Waiting on {on}").into(),
            Self::Done => "Done".into(),
            Self::Failed(why) => why.clone(),
        }
    }
}

/// One assistant and the helpers it started.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentNode {
    /// Identifies it.
    pub id: SharedString,
    /// Its name.
    pub name: SharedString,
    /// What it's for, in a few words.
    pub role: SharedString,
    /// What it's doing.
    pub state: AgentState,
    /// Steps taken so far.
    pub steps: u32,
    /// What it has spent, as written, such as "$0.42".
    pub spent: Option<SharedString>,
    /// The helpers it started, in the order it started them.
    pub helpers: Vec<AgentNode>,
}

impl AgentNode {
    /// Create an idle assistant with no helpers.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        role: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role: role.into(),
            state: AgentState::Idle,
            steps: 0,
            spent: None,
            helpers: Vec::new(),
        }
    }

    /// Set what it's doing.
    pub fn state(mut self, state: AgentState) -> Self {
        self.state = state;
        self
    }

    /// Set its steps and spend.
    pub fn progress(mut self, steps: u32, spent: impl Into<SharedString>) -> Self {
        self.steps = steps;
        self.spent = Some(spent.into());
        self
    }

    /// Add a helper.
    pub fn helper(mut self, helper: AgentNode) -> Self {
        self.helpers.push(helper);
        self
    }

    /// How many assistants this branch holds, itself included.
    pub fn count(&self) -> usize {
        1 + self.helpers.iter().map(AgentNode::count).sum::<usize>()
    }

    /// How many in this branch are in each tone: working, waiting, failed, done.
    pub fn tally(&self) -> [usize; 4] {
        let mut tally = [0; 4];
        self.walk(&mut |node| match node.state {
            AgentState::Working(_) => tally[0] += 1,
            AgentState::Waiting(_) => tally[1] += 1,
            AgentState::Failed(_) => tally[2] += 1,
            AgentState::Done => tally[3] += 1,
            AgentState::Idle => {}
        });
        tally
    }

    /// Find an assistant in this branch.
    pub fn find(&self, id: &str) -> Option<&AgentNode> {
        if self.id.as_ref() == id {
            return Some(self);
        }
        self.helpers.iter().find_map(|helper| helper.find(id))
    }

    fn walk(&self, visit: &mut impl FnMut(&AgentNode)) {
        visit(self);
        for helper in &self.helpers {
            helper.walk(visit);
        }
    }
}

type NodeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A lead assistant and its helpers as a tree of cards.
#[derive(IntoElement)]
pub struct AgentHierarchyCards {
    id: ElementId,
    style: StyleRefinement,
    root: AgentNode,
    folded: Vec<SharedString>,
    selected: Option<SharedString>,
    on_select: Option<NodeHandler>,
    on_toggle: Option<NodeHandler>,
}

impl AgentHierarchyCards {
    /// Create the tree from its lead assistant.
    pub fn new(id: impl Into<ElementId>, root: AgentNode) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            root,
            folded: Vec::new(),
            selected: None,
            on_select: None,
            on_toggle: None,
        }
    }

    /// The assistants whose helpers are folded away.
    pub fn folded(mut self, ids: Vec<SharedString>) -> Self {
        self.folded = ids;
        self
    }

    /// The selected card.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when a card is clicked.
    pub fn on_select(mut self, handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when a card's helpers are folded or unfolded.
    pub fn on_toggle(mut self, handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Styled for AgentHierarchyCards {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct Paint {
    ink: Hsla,
    quiet: Hsla,
    fill: Hsla,
    line: Hsla,
    accent: Hsla,
    radius: isonomic_ui::Pixels,
}

struct Ctx<'a> {
    tree: &'a AgentHierarchyCards,
    paint: Paint,
}

fn card(ctx: &Ctx, node: &AgentNode, depth: usize, cx: &mut App) -> AnyElement {
    let paint = &ctx.paint;
    let tree = ctx.tree;
    let tone = node.state.tone();
    let tone_color = tone.color(cx);
    let tint = tone.tint(cx);
    let is_selected = tree.selected.as_ref() == Some(&node.id);
    let is_folded = tree.folded.contains(&node.id);
    let helpers = node.count() - 1;
    let select = tree.on_select.clone();
    let toggle = tree.on_toggle.clone();
    let id = node.id.clone();
    let toggle_id = node.id.clone();
    let working = matches!(node.state, AgentState::Working(_));
    let mark = match &node.state {
        AgentState::Done => Icon::new(IconName::CircleCheck).xsmall().text_color(tone_color).into_any_element(),
        AgentState::Failed(_) => Icon::new(IconName::CircleX).xsmall().text_color(tone_color).into_any_element(),
        AgentState::Waiting(_) => Icon::new(IconName::Time).xsmall().text_color(tone_color).into_any_element(),
        AgentState::Working(_) => Icon::new(IconName::LoaderCircle)
            .xsmall()
            .text_color(tone_color)
            .with_animation(
                ElementId::NamedChild(tree.id.clone().into(), format!("spin-{}", node.id).into()),
                Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
            )
            .into_any_element(),
        AgentState::Idle => div()
            .size(px(8.))
            .rounded_full()
            .border_1()
            .border_color(paint.line)
            .into_any_element(),
    };
    let danger = Tone::Danger.color(cx);
    let mut waiting = 0;
    let mut failed = 0;
    for helper in &node.helpers {
        let [_, w, f, _] = helper.tally();
        waiting += w;
        failed += f;
    }

    v_flex()
        .id(ElementId::NamedChild(tree.id.clone().into(), format!("card-{}", node.id).into()))
        .w(px(208.))
        .flex_none()
        .gap_1p5()
        .p_2p5()
        .rounded(paint.radius)
        .border_1()
        .border_color(if is_selected { paint.accent } else { paint.line })
        .bg(paint.fill)
        .cursor_pointer()
        .shadow(resting_shadow(cx))
        .when(depth == 0, |this| this.border_t_2().border_color(if is_selected { paint.accent } else { tone_color }))
        .role(Role::Group)
        .aria_label(format!(
            "{}, {}, {}{}",
            node.name,
            node.role,
            node.state.words(),
            if helpers > 0 {
                format!(", {helpers} helpers{}", if is_folded { ", folded" } else { "" })
            } else {
                String::new()
            }
        ))
        .aria_selected(is_selected)
        .when(helpers > 0, |this| this.aria_expanded(!is_folded))
        .on_click(move |_, window, cx| {
            if let Some(select) = &select {
                select(&id, window, cx);
            }
        })
        .child(
            h_flex()
                .gap_2()
                .child(
                    div()
                        .size(px(24.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .bg(tint)
                        .child(Icon::new(IconName::Bot).xsmall().text_color(tone_color)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().font_medium().child(node.name.clone()))
                        .child(div().truncate().text_xs().text_color(paint.quiet).child(node.role.clone())),
                ),
        )
        .child(
            h_flex()
                .gap_1p5()
                .text_xs()
                .child(div().flex_none().child(mark))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(if working { paint.ink } else { tone_color })
                        .child(node.state.words()),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .text_xs()
                .text_color(paint.quiet)
                .child(format!("{} steps", node.steps))
                .children(node.spent.clone())
                .child(div().flex_1())
                .when(helpers > 0, |this| {
                    this.child(
                        h_flex()
                            .id(ElementId::NamedChild(
                                tree.id.clone().into(),
                                format!("fold-{}", node.id).into(),
                            ))
                            .gap_0p5()
                            .px_1()
                            .rounded(px(4.))
                            .hover(|style| style.bg(paint.line.opacity(0.5)))
                            .role(Role::Button)
                            .aria_label(format!(
                                "{} {helpers} helpers of {}",
                                if is_folded { "Show" } else { "Hide" },
                                node.name
                            ))
                            .aria_expanded(!is_folded)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                if let Some(toggle) = &toggle {
                                    toggle(&toggle_id, window, cx);
                                }
                            })
                            .child(
                                Icon::new(if is_folded {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .xsmall(),
                            )
                            .child(match (is_folded, failed, waiting) {
                                (true, f, _) if f > 0 => format!("{helpers}, {f} failed"),
                                (true, _, w) if w > 0 => format!("{helpers}, {w} waiting"),
                                _ => helpers.to_string(),
                            })
                            .when(is_folded && failed > 0, |this| this.text_color(danger)),
                    )
                }),
        )
        .into_any_element()
}

fn branch(ctx: &Ctx, node: &AgentNode, depth: usize, cx: &mut App) -> AnyElement {
    let line = ctx.paint.line;
    let show_helpers = !node.helpers.is_empty() && !ctx.tree.folded.contains(&node.id);
    let count = node.helpers.len();
    let top = card(ctx, node, depth, cx);
    let helpers: Vec<_> = if show_helpers {
        node.helpers
            .iter()
            .enumerate()
            .map(|(ix, helper)| {
                let first = ix == 0;
                let last = ix + 1 == count;
                v_flex()
                    .items_center()
                    .child(
                        h_flex()
                            .w_full()
                            .items_start()
                            .child(div().flex_1().h(px(1.)).when(!first, |this| this.bg(line)))
                            .child(div().w(px(1.)).h(px(14.)).bg(line))
                            .child(div().flex_1().h(px(1.)).when(!last, |this| this.bg(line))),
                    )
                    .child(div().px_1p5().child(branch(ctx, helper, depth + 1, cx)))
            })
            .collect()
    } else {
        Vec::new()
    };
    v_flex()
        .items_center()
        .child(top)
        .when(show_helpers, |this| {
            this.child(div().w(px(1.)).h(px(14.)).bg(line))
                .child(h_flex().items_start().children(helpers))
        })
        .into_any_element()
}

impl RenderOnce for AgentHierarchyCards {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let paint = Paint {
            ink: theme.foreground,
            quiet: theme.muted_foreground,
            fill: theme.background,
            line: theme.border,
            accent: theme.primary,
            radius: theme.radius_lg,
        };
        let quiet = paint.quiet;
        let tally = self.root.tally();
        let total = self.root.count();
        let summary = [
            (tally[0], "working"),
            (tally[1], "waiting"),
            (tally[2], "failed"),
            (tally[3], "done"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, word)| format!("{n} {word}"))
        .collect::<Vec<_>>()
        .join(", ");
        let id = self.id.clone();
        let style = self.style.clone();
        let ctx = Ctx { tree: &self, paint };
        let tree = branch(&ctx, &self.root, 0, cx);

        v_flex()
            .id(id.clone())
            .w_full()
            .gap_3()
            .text_sm()
            .text_color(ctx.paint.ink)
            .role(Role::Group)
            .aria_label(format!("{total} assistants, {summary}"))
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(quiet)
                    .child(format!("{total} assistants"))
                    .when(!summary.is_empty(), |this| this.child("\u{b7}").child(summary.clone())),
            )
            .child(
                div()
                    .id(ElementId::NamedChild(id.into(), "scroll".into()))
                    .w_full()
                    .overflow_x_scroll()
                    .child(h_flex().justify_center().min_w_full().py_1().child(tree)),
            )
            .refine_style(&style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> AgentNode {
        AgentNode::new("lead", "Lead", "Runs the close")
            .state(AgentState::Working("Planning".into()))
            .helper(
                AgentNode::new("a", "A", "Matches")
                    .state(AgentState::Done)
                    .helper(AgentNode::new("a1", "A1", "Cards").state(AgentState::Failed("Locked".into()))),
            )
            .helper(AgentNode::new("b", "B", "Pays").state(AgentState::Waiting("approval".into())))
    }

    #[test]
    fn counts_and_tallies_cover_the_whole_branch() {
        let root = tree();
        assert_eq!(root.count(), 4);
        assert_eq!(root.tally(), [1, 1, 1, 1]);
        assert_eq!(root.helpers[0].count(), 2);
    }

    #[test]
    fn find_reaches_nested_helpers() {
        let root = tree();
        assert_eq!(root.find("a1").map(|n| n.name.as_ref()), Some("A1"));
        assert!(root.find("zz").is_none());
    }
}
