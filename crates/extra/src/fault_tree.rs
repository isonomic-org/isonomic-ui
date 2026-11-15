//! Fault tree: the possible causes of a failure, joined by and/or gates, each with a verdict and its evidence.
//!
//! The failure sits at the top. Under it, causes branch down through gates:
//! an "any of" gate fails if one cause holds, an "all of" gate only if they
//! all do. Each cause carries a verdict (confirmed, ruled out, or still
//! open) and the evidence behind it. The tree works out which branches
//! still explain the failure and fades the ones ruled out, so the search
//! narrows as evidence comes in. Folding and verdicts go to the owner.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    Role,
    AnyElement, App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// What's known about a cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Not checked yet.
    Open,
    /// It holds.
    Confirmed,
    /// It doesn't.
    RuledOut,
}

/// How a node's causes combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// One cause is enough.
    Any,
    /// Every cause is needed.
    All,
}

/// A cause, and the causes under it.
#[derive(Debug, Clone, PartialEq)]
pub struct FaultNode {
    /// Identifies it.
    pub id: SharedString,
    /// What it supposes.
    pub label: SharedString,
    /// What's known about it, for a leaf.
    pub verdict: Verdict,
    /// What the verdict rests on.
    pub evidence: Option<SharedString>,
    /// How its causes combine.
    pub gate: Gate,
    /// The causes under it.
    pub causes: Vec<FaultNode>,
}

impl FaultNode {
    /// Create an open cause.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            verdict: Verdict::Open,
            evidence: None,
            gate: Gate::Any,
            causes: Vec::new(),
        }
    }

    /// Set what's known, with its evidence.
    pub fn verdict(mut self, verdict: Verdict, evidence: impl Into<SharedString>) -> Self {
        self.verdict = verdict;
        self.evidence = Some(evidence.into());
        self
    }

    /// Set its gate and causes.
    pub fn gate(mut self, gate: Gate, causes: Vec<FaultNode>) -> Self {
        self.gate = gate;
        self.causes = causes;
        self
    }

    /// What follows for this node from what's known below it: a leaf's own
    /// verdict, or its gate applied to its causes.
    pub fn standing(&self) -> Verdict {
        if self.causes.is_empty() {
            return self.verdict;
        }
        if self.verdict != Verdict::Open {
            return self.verdict;
        }
        let below: Vec<Verdict> = self.causes.iter().map(FaultNode::standing).collect();
        match self.gate {
            Gate::Any if below.contains(&Verdict::Confirmed) => Verdict::Confirmed,
            Gate::Any if below.iter().all(|v| *v == Verdict::RuledOut) => Verdict::RuledOut,
            Gate::All if below.iter().all(|v| *v == Verdict::Confirmed) => Verdict::Confirmed,
            Gate::All if below.contains(&Verdict::RuledOut) => Verdict::RuledOut,
            _ => Verdict::Open,
        }
    }

    /// How many leaves are still open.
    pub fn open_leaves(&self) -> usize {
        if self.causes.is_empty() {
            usize::from(self.verdict == Verdict::Open)
        } else {
            self.causes.iter().map(FaultNode::open_leaves).sum()
        }
    }
}

type VerdictHandler = Rc<dyn Fn(&SharedString, Verdict, &mut Window, &mut App)>;

/// A failure and its possible causes.
#[derive(IntoElement)]
pub struct FaultTree {
    id: ElementId,
    style: StyleRefinement,
    root: FaultNode,
    on_verdict: Option<VerdictHandler>,
}

impl FaultTree {
    /// Create a tree for a failure.
    pub fn new(id: impl Into<ElementId>, root: FaultNode) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            root,
            on_verdict: None,
        }
    }

    /// Run when a leaf's verdict is set from its buttons.
    pub fn on_verdict(mut self, handler: impl Fn(&SharedString, Verdict, &mut Window, &mut App) + 'static) -> Self {
        self.on_verdict = Some(Rc::new(handler));
        self
    }
}

impl Styled for FaultTree {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct Paint {
    ink: Hsla,
    quiet: Hsla,
    fill: Hsla,
    line: Hsla,
    chip: Hsla,
}

fn verdict_look(verdict: Verdict, cx: &App) -> (Hsla, &'static str, IconName) {
    match verdict {
        Verdict::Open => (Tone::Neutral.color(cx), "Open", IconName::CircleAlert),
        Verdict::Confirmed => (Tone::Danger.color(cx), "Confirmed", IconName::CircleCheck),
        Verdict::RuledOut => (Tone::Success.color(cx), "Ruled out", IconName::CircleX),
    }
}

fn draw(tree: &FaultTree, paint: &Paint, node: &FaultNode, depth: usize, cx: &mut App) -> AnyElement {
    let standing = node.standing();
    let (color, word, icon) = verdict_look(standing, cx);
    let faded = standing == Verdict::RuledOut && depth > 0;
    let leaf = node.causes.is_empty();
    let buttons = (leaf && depth > 0).then(|| {
        let make = |label: &'static str, verdict: Verdict| {
            let on_verdict = tree.on_verdict.clone();
            let id = node.id.clone();
            Button::new(ElementId::NamedChild(tree.id.clone().into(), format!("{label}-{}", node.id).into()))
                .xsmall()
                .map(|b| if node.verdict == verdict { b.primary() } else { b.ghost() })
                .label(label)
                .accessibility_label(format!("{label}: {}", node.label))
                .on_click(move |_, window, cx| {
                    if let Some(on_verdict) = &on_verdict {
                        on_verdict(&id, verdict, window, cx);
                    }
                })
        };
        h_flex()
            .gap_0p5()
            .child(make("Confirm", Verdict::Confirmed))
            .child(make("Rule out", Verdict::RuledOut))
            .child(make("Reopen", Verdict::Open))
    });
    let card = v_flex()
        .id(ElementId::NamedChild(tree.id.clone().into(), format!("node-{}", node.id).into()))
        .gap_1()
        .p_2p5()
        .rounded(px(8.))
        .border_1()
        .border_color(if standing == Verdict::Confirmed { color } else { paint.line })
        .bg(if depth == 0 { color.opacity(0.08) } else { paint.fill })
        .when(faded, |this| this.opacity(0.5))
        .role(Role::TreeItem)
        .aria_label(format!(
            "{}, {word}{}",
            node.label,
            node.evidence.as_ref().map(|e| format!(". {e}")).unwrap_or_default()
        ))
        .child(
            h_flex()
                .gap_2()
                .child(Icon::new(icon).xsmall().text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .when(depth == 0, |this| this.font_semibold())
                        .when(depth > 0, |this| this.font_medium())
                        .when(faded, |this| this.line_through())
                        .line_height(relative(1.3))
                        .child(node.label.clone()),
                )
                .child(div().flex_none().text_xs().text_color(color).child(word)),
        )
        .children(node.evidence.clone().map(|e| {
            div().pl(px(20.)).text_xs().text_color(paint.quiet).line_height(relative(1.4)).child(e)
        }))
        .children(buttons.map(|b| div().pl(px(14.)).child(b)));

    if leaf {
        return card.into_any_element();
    }
    let gate_word = match node.gate {
        Gate::Any => "Any of",
        Gate::All => "All of",
    };
    v_flex()
        .child(card)
        .child(
            v_flex()
                .ml(px(16.))
                .pl_4()
                .border_l_1()
                .border_color(paint.line)
                .child(
                    div()
                        .py_1()
                        .child(
                            div()
                                .px_1p5()
                                .rounded(px(4.))
                                .bg(paint.chip)
                                .text_xs()
                                .font_medium()
                                .text_color(paint.ink)
                                .child(gate_word),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .pb_1()
                        .children(node.causes.iter().map(|child| draw(tree, paint, child, depth + 1, cx))),
                ),
        )
        .into_any_element()
}

impl RenderOnce for FaultTree {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let paint = Paint {
            ink: theme.foreground,
            quiet: theme.muted_foreground,
            fill: theme.background,
            line: theme.border,
            chip: theme.muted,
        };
        let radius_lg = theme.radius_lg;
        let open = self.root.open_leaves();
        let standing = self.root.standing();
        let tree = draw(&self, &paint, &self.root, 0, cx);

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(680.))
            .rounded(radius_lg)
            .border_1()
            .border_color(paint.line)
            .bg(paint.fill)
            .text_color(paint.ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(paint.line)
                    .text_xs()
                    .text_color(paint.quiet)
                    .child(Icon::new(IconName::Search).xsmall())
                    .child(div().flex_1().child(match standing {
                        Verdict::Confirmed => "A cause is confirmed".to_string(),
                        Verdict::RuledOut => "Every cause is ruled out: look wider".to_string(),
                        Verdict::Open => format!("{open} causes still open"),
                    })),
            )
            .child(div().p_4().child(tree))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_combine_what_is_known() {
        let any = FaultNode::new("r", "R").gate(
            Gate::Any,
            vec![
                FaultNode::new("a", "A").verdict(Verdict::RuledOut, "no"),
                FaultNode::new("b", "B"),
            ],
        );
        assert_eq!(any.standing(), Verdict::Open);
        assert_eq!(any.open_leaves(), 1);
        let all = FaultNode::new("r", "R").gate(
            Gate::All,
            vec![
                FaultNode::new("a", "A").verdict(Verdict::Confirmed, "yes"),
                FaultNode::new("b", "B").verdict(Verdict::RuledOut, "no"),
            ],
        );
        assert_eq!(all.standing(), Verdict::RuledOut);
        let found = FaultNode::new("r", "R").gate(Gate::Any, vec![FaultNode::new("a", "A").verdict(Verdict::Confirmed, "yes")]);
        assert_eq!(found.standing(), Verdict::Confirmed);
    }
}
