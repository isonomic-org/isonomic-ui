//! Context treemap: what fills a context budget as nested boxes, before and after compaction.
//!
//! Each kind of content (instructions, files, conversation, tool results)
//! gets a column as wide as its share of the budget, split into boxes for
//! its items; free space fills the rest. A switch shows the same budget
//! after compaction, where items that would shrink are drawn at their
//! smaller size and the space won back shows as free. Selecting a box names
//! it and its size underneath.

use std::rc::Rc;

use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    Role,
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One item in the context.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextBlock {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// The kind it belongs to, such as "Files".
    pub kind: SharedString,
    /// Tokens it takes now.
    pub tokens: u32,
    /// Tokens it would take after compaction, if it would shrink.
    pub compacted: Option<u32>,
}

impl ContextBlock {
    /// Create an item.
    pub fn new(id: impl Into<SharedString>, kind: impl Into<SharedString>, label: impl Into<SharedString>, tokens: u32) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: kind.into(),
            tokens,
            compacted: None,
        }
    }

    /// Say what it would shrink to.
    pub fn compacts_to(mut self, tokens: u32) -> Self {
        self.compacted = Some(tokens.min(self.tokens));
        self
    }

    /// Its size in the chosen view.
    pub fn size(&self, after: bool) -> u32 {
        if after { self.compacted.unwrap_or(self.tokens) } else { self.tokens }
    }
}

/// The kinds in the order they first appear, with their total size.
pub fn kinds(blocks: &[ContextBlock], after: bool) -> Vec<(SharedString, u32)> {
    let mut kinds: Vec<(SharedString, u32)> = Vec::new();
    for block in blocks {
        match kinds.iter_mut().find(|(k, _)| k == &block.kind) {
            Some((_, total)) => *total += block.size(after),
            None => kinds.push((block.kind.clone(), block.size(after))),
        }
    }
    kinds
}

/// Write a token count short: "820", "12.4k", "1.2M".
pub fn tokens_words(tokens: u32) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}k", tokens as f32 / 1000.),
        _ => format!("{:.1}M", tokens as f32 / 1_000_000.),
    }
}

type SelectHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;
type ViewHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A context budget as nested boxes.
#[derive(IntoElement)]
pub struct ContextTreemap {
    id: ElementId,
    style: StyleRefinement,
    blocks: Vec<ContextBlock>,
    budget: u32,
    after: bool,
    selected: Option<SharedString>,
    on_select: Option<SelectHandler>,
    on_view: Option<ViewHandler>,
}

impl ContextTreemap {
    /// Create a treemap for a budget in tokens.
    pub fn new(id: impl Into<ElementId>, blocks: Vec<ContextBlock>, budget: u32) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            blocks,
            budget: budget.max(1),
            after: false,
            selected: None,
            on_select: None,
            on_view: None,
        }
    }

    /// Show the budget after compaction.
    pub fn after(mut self, after: bool) -> Self {
        self.after = after;
        self
    }

    /// The selected item.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when a box is clicked, or `None` for free space.
    pub fn on_select(mut self, handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when the before and after switch is clicked, with the view wanted.
    pub fn on_view(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_view = Some(Rc::new(handler));
        self
    }
}

impl Styled for ContextTreemap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ContextTreemap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let palette: [Hsla; 5] = [theme.primary, theme.success, theme.warning, theme.info, theme.danger];

        let after = self.after;
        let groups = kinds(&self.blocks, after);
        let used: u32 = groups.iter().map(|(_, t)| *t).sum();
        let before_used: u32 = self.blocks.iter().map(|b| b.tokens).sum();
        let free = self.budget.saturating_sub(used);
        let over = used > self.budget;
        let scale = used.max(self.budget) as f32;
        let saved = before_used.saturating_sub(self.blocks.iter().map(|b| b.size(true)).sum());

        let columns: Vec<_> = groups
            .iter()
            .enumerate()
            .map(|(gx, (kind, total))| {
                let color = palette[gx % palette.len()];
                let items: Vec<_> = self
                    .blocks
                    .iter()
                    .filter(|b| &b.kind == kind && b.size(after) > 0)
                    .map(|block| {
                        let size = block.size(after);
                        let is_selected = self.selected.as_ref() == Some(&block.id);
                        let shrank = after && block.compacted.is_some();
                        let select = self.on_select.clone();
                        let id = block.id.clone();
                        div()
                            .id(ElementId::NamedChild(self.id.clone().into(), format!("block-{}", block.id).into()))
                            .flex_basis(relative(size as f32 / (*total).max(1) as f32))
                            .flex_grow(size as f32)
                            .min_h(px(4.))
                            .overflow_hidden()
                            .p_1()
                            .border_1()
                            .border_color(fill)
                            .rounded(px(3.))
                            .bg(color.opacity(if is_selected { 0.55 } else { 0.28 }))
                            .when(shrank, |this| this.border_dashed().border_color(color))
                            .hover(|style| style.bg(color.opacity(0.45)))
                            .cursor_pointer()
                            .role(Role::Button)
                            .aria_label(format!("{}, {} tokens{}", block.label, tokens_words(size), if shrank { ", compacted" } else { "" }))
                            .aria_selected(is_selected)
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(Some(&id), window, cx);
                                }
                            })
                            .child(div().text_xs().truncate().text_color(ink).child(block.label.clone()))
                    })
                    .collect();
                v_flex()
                    .flex_basis(relative(*total as f32 / scale))
                    .flex_grow(*total as f32)
                    .min_w(px(18.))
                    .h_full()
                    .gap_0()
                    .children(items)
            })
            .collect();

        let select_free = self.on_select.clone();
        let toggle = |label: &'static str, want: bool| {
            let on_view = self.on_view.clone();
            let active = after == want;
            div()
                .id(ElementId::NamedChild(self.id.clone().into(), format!("view-{label}").into()))
                .px_2()
                .py_0p5()
                .rounded(px(5.))
                .cursor_pointer()
                .text_xs()
                .when(active, |this| this.bg(fill).shadow(resting_shadow(cx)).font_medium())
                .when(!active, |this| this.text_color(quiet))
                .aria_selected(active)
                .on_click(move |_, window, cx| {
                    if let Some(on_view) = &on_view {
                        on_view(want, window, cx);
                    }
                })
                .child(label)
        };

        let detail = self
            .selected
            .as_ref()
            .and_then(|id| self.blocks.iter().find(|b| &b.id == id))
            .map(|block| match block.compacted {
                Some(small) => format!(
                    "{} ({}): {} tokens, {} after compaction",
                    block.label,
                    block.kind,
                    tokens_words(block.tokens),
                    tokens_words(small)
                ),
                None => format!("{} ({}): {} tokens, kept as is", block.label, block.kind, tokens_words(block.tokens)),
            })
            .unwrap_or_else(|| "Select a box to see its size.".into());

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
                    .flex_wrap()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(160.))
                            .child(div().font_medium().child(format!(
                                "{} of {} tokens",
                                tokens_words(used),
                                tokens_words(self.budget)
                            )))
                            .child(div().text_xs().text_color(if over { theme_danger(cx) } else { quiet }).child(if over {
                                format!("Over by {}", tokens_words(used - self.budget))
                            } else if after {
                                format!("Compaction wins back {}", tokens_words(saved))
                            } else {
                                format!("{} free, compaction could win back {}", tokens_words(free), tokens_words(saved))
                            })),
                    )
                    .child(
                        h_flex()
                            .p_0p5()
                            .rounded(px(7.))
                            .bg(chip)
                            .child(toggle("Now", false))
                            .child(toggle("After compaction", true)),
                    ),
            )
            .child(
                h_flex()
                    .m_3()
                    .h(px(240.))
                    .gap_0p5()
                    .children(columns)
                    .when(free > 0, |this| {
                        this.child(
                            div()
                                .id(ElementId::NamedChild(self.id.clone().into(), "free".into()))
                                .flex_basis(relative(free as f32 / scale))
                                .flex_grow(free as f32)
                                .h_full()
                                .rounded(radius)
                                .border_1()
                                .border_dashed()
                                .border_color(line)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(quiet)
                                .role(Role::Button)
                                .aria_label(format!("Free, {} tokens", tokens_words(free)))
                                .on_click(move |_, window, cx| {
                                    if let Some(select) = &select_free {
                                        select(None, window, cx);
                                    }
                                })
                                .child(format!("Free {}", tokens_words(free))),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_3()
                    .px_4()
                    .pb_2()
                    .text_xs()
                    .text_color(quiet)
                    .children(groups.iter().enumerate().map(|(gx, (kind, total))| {
                        h_flex()
                            .gap_1()
                            .child(div().size(px(8.)).rounded(px(2.)).bg(palette[gx % palette.len()].opacity(0.6)))
                            .child(format!("{kind} {}", tokens_words(*total)))
                    })),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
                    .child(detail),
            )
            .refine_style(&self.style)
    }
}

fn theme_danger(cx: &App) -> Hsla {
    cx.theme().danger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_add_up_before_and_after() {
        let blocks = vec![
            ContextBlock::new("a", "Files", "a.rs", 1_000).compacts_to(200),
            ContextBlock::new("b", "Talk", "turns", 3_000),
            ContextBlock::new("c", "Files", "b.rs", 500),
        ];
        assert_eq!(kinds(&blocks, false), [("Files".into(), 1_500), ("Talk".into(), 3_000)]);
        assert_eq!(kinds(&blocks, true)[0].1, 700);
    }

    #[test]
    fn tokens_read_short() {
        assert_eq!(tokens_words(820), "820");
        assert_eq!(tokens_words(12_400), "12.4k");
        assert_eq!(tokens_words(1_200_000), "1.2M");
    }
}
