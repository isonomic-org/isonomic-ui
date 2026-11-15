//! Option tree: the branches open at a decision point, what each leads to, and the risks flagged on the way.
//!
//! The decision sits at the left; each option branches to the right, and
//! an option's own consequences branch on from it. Every card says what
//! happens and how likely or costly it is, and a risk flag marks the ones
//! that need a second look. The assistant's recommended path is traced in
//! the accent colour. Picking an option goes to the owner.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// How worrying a branch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BranchRisk {
    /// Nothing flagged.
    None,
    /// Worth knowing.
    Some,
    /// Needs a second look.
    High,
}

/// One branch: an option or a consequence of one.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// What it leads to, in a sentence.
    pub detail: SharedString,
    /// How worrying it is.
    pub risk: BranchRisk,
    /// The risk itself, in a few words.
    pub risk_note: Option<SharedString>,
    /// Whether it's on the recommended path.
    pub recommended: bool,
    /// What follows from it.
    pub children: Vec<Branch>,
}

impl Branch {
    /// Create a branch with no risk.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        detail: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: detail.into(),
            risk: BranchRisk::None,
            risk_note: None,
            recommended: false,
            children: Vec::new(),
        }
    }

    /// Flag a risk.
    pub fn risk(mut self, risk: BranchRisk, note: impl Into<SharedString>) -> Self {
        self.risk = risk;
        self.risk_note = Some(note.into());
        self
    }

    /// Put it on the recommended path.
    pub fn recommended(mut self) -> Self {
        self.recommended = true;
        self
    }

    /// Add what follows from it.
    pub fn child(mut self, branch: Branch) -> Self {
        self.children.push(branch);
        self
    }

    /// The highest risk anywhere in this branch.
    pub fn worst_risk(&self) -> BranchRisk {
        self.children
            .iter()
            .map(Branch::worst_risk)
            .chain(std::iter::once(self.risk))
            .max()
            .unwrap_or(BranchRisk::None)
    }
}

type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A decision and the branches open from it.
#[derive(IntoElement)]
pub struct OptionTree {
    id: ElementId,
    style: StyleRefinement,
    question: SharedString,
    options: Vec<Branch>,
    chosen: Option<SharedString>,
    on_pick: Option<PickHandler>,
}

impl OptionTree {
    /// Create a tree for a decision.
    pub fn new(
        id: impl Into<ElementId>,
        question: impl Into<SharedString>,
        options: Vec<Branch>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            question: question.into(),
            options,
            chosen: None,
            on_pick: None,
        }
    }

    /// The option picked, if any.
    pub fn chosen(mut self, id: Option<SharedString>) -> Self {
        self.chosen = id;
        self
    }

    /// Run when an option is picked.
    pub fn on_pick(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl Styled for OptionTree {
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
    warning: Hsla,
    danger: Hsla,
}

fn node(tree: &OptionTree, paint: &Paint, branch: &Branch, top: bool, cx: &mut App) -> AnyElement {
    let chosen = tree.chosen.as_ref() == Some(&branch.id);
    let flag = match branch.risk {
        BranchRisk::None => None,
        BranchRisk::Some => Some(paint.warning),
        BranchRisk::High => Some(paint.danger),
    };
    let card = v_flex()
        .id(ElementId::NamedChild(
            tree.id.clone().into(),
            format!("branch-{}", branch.id).into(),
        ))
        .w(px(210.))
        .flex_none()
        .gap_1()
        .p_2p5()
        .rounded(px(8.))
        .border_1()
        .border_color(if chosen {
            paint.accent
        } else if branch.recommended {
            paint.accent.opacity(0.5)
        } else {
            paint.line
        })
        .when(chosen, |this| this.border_2())
        .bg(paint.fill)
        .shadow(resting_shadow(cx))
        .role(Role::Group)
        .aria_label(format!(
            "{}. {}{}{}",
            branch.label,
            branch.detail,
            branch
                .risk_note
                .as_ref()
                .map(|n| format!(". Risk: {n}"))
                .unwrap_or_default(),
            if branch.recommended {
                ". Recommended"
            } else {
                ""
            }
        ))
        .aria_selected(chosen)
        .child(
            h_flex()
                .gap_1p5()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_medium()
                        .line_height(relative(1.3))
                        .child(branch.label.clone()),
                )
                .when(branch.recommended, |this| {
                    this.child(Icon::new(IconName::Star).xsmall().text_color(paint.accent))
                }),
        )
        .child(
            div()
                .text_xs()
                .text_color(paint.quiet)
                .line_height(relative(1.4))
                .child(branch.detail.clone()),
        )
        .children(branch.risk_note.clone().zip(flag).map(|(note, color)| {
            h_flex()
                .gap_1()
                .text_xs()
                .text_color(color)
                .child(Icon::new(IconName::TriangleAlert).xsmall())
                .child(div().min_w_0().child(note))
        }))
        .when(top, |this| {
            let pick = tree.on_pick.clone();
            let id = branch.id.clone();
            this.child(
                div().pt_1().child(
                    Button::new(ElementId::NamedChild(
                        tree.id.clone().into(),
                        format!("pick-{}", branch.id).into(),
                    ))
                    .xsmall()
                    .map(|b| if chosen { b.primary() } else { b.outline() })
                    .label(if chosen { "Picked" } else { "Pick this" })
                    .accessibility_label(format!("Pick {}", branch.label))
                    .on_click(move |_, window, cx| {
                        if let Some(pick) = &pick {
                            pick(&id, window, cx);
                        }
                    }),
                ),
            )
        });

    if branch.children.is_empty() {
        return card.into_any_element();
    }
    let count = branch.children.len();
    let children: Vec<_> = branch
        .children
        .iter()
        .enumerate()
        .map(|(ix, child)| {
            let stroke = if child.recommended {
                paint.accent
            } else {
                paint.line
            };
            h_flex()
                .items_start()
                .child(
                    v_flex()
                        .w(px(16.))
                        .flex_none()
                        .self_stretch()
                        .child(
                            div()
                                .h(px(22.))
                                .w_full()
                                .border_b_1()
                                .border_color(stroke)
                                .when(ix > 0, |this| this.border_l_1().border_color(paint.line)),
                        )
                        .child(div().flex_1().when(ix + 1 < count, |this| {
                            this.border_l_1().border_color(paint.line)
                        })),
                )
                .child(div().pb_2().child(node(tree, paint, child, false, cx)))
        })
        .collect();
    h_flex()
        .items_start()
        .child(card)
        .child(
            div()
                .w(px(12.))
                .h(px(22.))
                .flex_none()
                .border_b_1()
                .border_color(if branch.children.iter().any(|c| c.recommended) {
                    paint.accent
                } else {
                    paint.line
                }),
        )
        .child(v_flex().children(children))
        .into_any_element()
}

impl RenderOnce for OptionTree {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let paint = Paint {
            ink: theme.foreground,
            quiet: theme.muted_foreground,
            fill: theme.background,
            line: theme.border,
            accent: theme.primary,
            warning: Tone::Warning.color(cx),
            danger: Tone::Danger.color(cx),
        };
        let radius_lg = cx.theme().radius_lg;
        let flagged = self
            .options
            .iter()
            .filter(|o| o.worst_risk() == BranchRisk::High)
            .count();
        let options: Vec<_> = self
            .options
            .iter()
            .map(|option| div().pb_3().child(node(&self, &paint, option, true, cx)))
            .collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .rounded(radius_lg)
            .border_1()
            .border_color(paint.line)
            .bg(paint.fill)
            .text_color(paint.ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_0p5()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(paint.line)
                    .child(div().font_semibold().child(self.question.clone()))
                    .child(div().text_xs().text_color(paint.quiet).child(format!(
                        "{} options{}",
                        self.options.len(),
                        if flagged > 0 {
                            format!(", {flagged} with a high risk on the way")
                        } else {
                            String::new()
                        }
                    ))),
            )
            .child(
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "scroll".into(),
                    ))
                    .overflow_x_scroll()
                    .child(v_flex().p_4().children(options)),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worst_risk_rises_from_below() {
        let branch = Branch::new("a", "A", "x")
            .child(
                Branch::new("a1", "A1", "y")
                    .child(Branch::new("a2", "A2", "z").risk(BranchRisk::High, "locked")),
            )
            .risk(BranchRisk::Some, "slow");
        assert_eq!(branch.worst_risk(), BranchRisk::High);
        assert_eq!(Branch::new("b", "B", "x").worst_risk(), BranchRisk::None);
    }
}
