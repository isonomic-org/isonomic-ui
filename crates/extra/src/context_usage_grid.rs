// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Context usage grid: what fills a model's context budget.
//!
//! The budget is a field of small cells, coloured by the part that takes
//! them up: instructions, tools, attached files, the conversation. Pointing
//! at a part lights its cells and puts its size in the headline. Files can be
//! removed one by one, and a long part can be compacted: the card asks, and
//! its owner passes the parts as they are afterwards.

use std::{collections::HashSet, rc::Rc};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One thing inside a part, such as an attached file.
#[derive(Debug, Clone)]
pub struct BudgetItem {
    id: SharedString,
    label: SharedString,
    tokens: u64,
}

impl BudgetItem {
    /// Create an item with its size in tokens.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, tokens: u64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tokens,
        }
    }
}

/// One part of the budget.
#[derive(Debug, Clone)]
pub struct BudgetPart {
    id: SharedString,
    label: SharedString,
    tokens: u64,
    items: Vec<BudgetItem>,
    locked: bool,
    compactable: bool,
    compacted: bool,
    color: Option<Hsla>,
}

impl BudgetPart {
    /// Create a part with its size in tokens.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, tokens: u64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tokens,
            items: Vec::new(),
            locked: false,
            compactable: false,
            compacted: false,
            color: None,
        }
    }

    /// Add an item. A part with items takes its size from them.
    pub fn item(mut self, item: BudgetItem) -> Self {
        self.items.push(item);
        self
    }

    /// Keep its items from being removed.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Offer a Compact button for the part.
    pub fn compactable(mut self, compactable: bool) -> Self {
        self.compactable = compactable;
        self
    }

    /// Say the part has been compacted: the button gives way to a check.
    pub fn compacted(mut self, compacted: bool) -> Self {
        self.compacted = compacted;
        self
    }

    /// Set its colour. The theme's base hues otherwise, in order.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

type RemoveHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type CompactHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A field of cells showing what fills a context budget, with the parts
/// listed under it.
#[derive(IntoElement)]
pub struct ContextUsageGrid {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    model: Option<SharedString>,
    limit: u64,
    parts: Vec<BudgetPart>,
    warn_at: f32,
    cells: usize,
    columns: usize,
    on_remove_item: Option<RemoveHandler>,
    on_compact: Option<CompactHandler>,
    action: Option<(SharedString, ActionHandler)>,
}

impl ContextUsageGrid {
    /// Create a grid for a budget of `limit` tokens.
    pub fn new(id: impl Into<ElementId>, limit: u64) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Context".into(),
            model: None,
            limit,
            parts: Vec::new(),
            warn_at: 0.8,
            cells: 216,
            columns: 36,
            on_remove_item: None,
            on_compact: None,
            action: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Name the model, in a small tag in the head.
    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Add a part. Parts fill the field in this order; up to six read well.
    pub fn part(mut self, part: BudgetPart) -> Self {
        self.parts.push(part);
        self
    }

    /// Set the share of the budget from which the card warns. 0.8 by
    /// default; from 0.95 the budget counts as full.
    pub fn warn_at(mut self, share: f32) -> Self {
        self.warn_at = share;
        self
    }

    /// Set how many cells the field has, and how many make a row.
    pub fn cells(mut self, cells: usize, columns: usize) -> Self {
        self.cells = cells.max(1);
        self.columns = columns.max(1);
        self
    }

    /// Run with the part's and the item's ids when an item's Remove is
    /// pressed. Pass the parts again without it.
    pub fn on_remove_item(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove_item = Some(Rc::new(handler));
        self
    }

    /// Run with the part's id when its Compact is pressed. Pass the part again
    /// at its new size, marked `compacted`.
    pub fn on_compact(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_compact = Some(Rc::new(handler));
        self
    }

    /// Add a button to the last row, such as one that starts afresh.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }
}

impl Styled for ContextUsageGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A token count in a few characters: "950", "4.2K", "1.2M".
fn tokens_words(tokens: u64) -> String {
    let (scaled, suffix) = if tokens >= 1_000_000 {
        (tokens as f64 / 1e6, "M")
    } else if tokens >= 1000 {
        (tokens as f64 / 1e3, "K")
    } else {
        (tokens as f64, "")
    };
    let rounded = (scaled * 10.).round() / 10.;
    if rounded.fract() == 0. {
        format!("{rounded:.0}{suffix}")
    } else {
        format!("{rounded:.1}{suffix}")
    }
}

/// How many cells each part takes. A part with any tokens gets at least one
/// cell, and the parts never take more cells than the field has.
fn cell_counts(sizes: &[u64], limit: u64, cells: usize) -> Vec<usize> {
    let per_cell = limit.max(1) as f64 / cells as f64;
    let mut left = cells;
    sizes
        .iter()
        .map(|size| {
            let wanted = if *size == 0 {
                0
            } else {
                ((*size as f64 / per_cell).round() as usize).max(1)
            };
            let given = wanted.min(left);
            left -= given;
            given
        })
        .collect()
}

#[derive(Default)]
struct GridState {
    hovered: Option<usize>,
    open: HashSet<SharedString>,
}

impl RenderOnce for ContextUsageGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let success = theme.success;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        // Distinct hues, so neighbouring parts can be told apart.
        let palette = [
            theme.blue,
            theme.cyan,
            theme.yellow,
            theme.magenta,
            theme.green,
            theme.red,
        ];

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| GridState::default());
        let (hovered, open) = {
            let state = state.read(cx);
            (state.hovered, state.open.clone())
        };

        // A part with items takes its size from them.
        let sizes: Vec<u64> = self
            .parts
            .iter()
            .map(|part| {
                if part.items.is_empty() {
                    part.tokens
                } else {
                    part.items.iter().map(|item| item.tokens).sum()
                }
            })
            .collect();
        let used: u64 = sizes.iter().sum();
        let share = used as f32 / self.limit.max(1) as f32;
        let full = share >= 0.95;
        let warn = share >= self.warn_at;
        let hovered = hovered.filter(|ix| *ix < self.parts.len());
        let colors: Vec<Hsla> = self
            .parts
            .iter()
            .enumerate()
            .map(|(ix, part)| part.color.unwrap_or(palette[ix % palette.len()]))
            .collect();
        let counts = cell_counts(&sizes, self.limit, self.cells);
        let hover = |ix: usize| {
            let state = state.clone();
            move |hovered: &bool, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    if *hovered {
                        state.hovered = Some(ix);
                        cx.notify();
                    } else if state.hovered == Some(ix) {
                        state.hovered = None;
                        cx.notify();
                    }
                })
            }
        };

        // Every cell, in part order, then the empty ones.
        let mut cells: Vec<Option<usize>> = Vec::with_capacity(self.cells);
        for (ix, count) in counts.iter().enumerate() {
            cells.extend(std::iter::repeat_n(Some(ix), *count));
        }
        cells.resize(self.cells, None);
        let field = v_flex()
            .gap(px(2.))
            .children(cells.chunks(self.columns).map(|row| {
                h_flex().gap(px(2.)).children(row.iter().map(|cell| {
                    let color = match cell {
                        Some(ix) => {
                            let lit = hovered.is_none_or(|hovered| hovered == *ix);
                            colors[*ix].opacity(if lit { 0.85 } else { 0.25 })
                        }
                        None => chip,
                    };
                    div().flex_1().h(px(7.)).rounded(px(1.5)).bg(color)
                }))
            }));

        let (figure, caption) = match hovered {
            Some(ix) => (
                tokens_words(sizes[ix]),
                format!("{} tokens", self.parts[ix].label),
            ),
            None => (
                tokens_words(used),
                format!("of {} tokens", tokens_words(self.limit)),
            ),
        };
        let percent = (share * 100.).round() as u32;
        let tag = div()
            .flex_none()
            .px_1p5()
            .rounded(px(5.))
            .text_label_small()
            .map(|this| {
                if full {
                    this.bg(danger.opacity(0.16))
                        .child(format!("{percent}%, full"))
                } else if warn {
                    this.bg(warning.opacity(0.18)).child(format!("{percent}%"))
                } else {
                    this.bg(chip).child(format!("{percent}%"))
                }
            });

        let rows = self.parts.iter().enumerate().map(|(ix, part)| {
            let items = &part.items;
            let tokens = &sizes[ix];
            let is_open = open.contains(&part.id);
            let has_items = !part.items.is_empty();
            let compact = part.compactable.then(|| {
                if part.compacted {
                    h_flex()
                        .gap_1()
                        .text_caption()
                        .text_color(success)
                        .child(Icon::new(IconName::Check).xsmall())
                        .child("Compacted")
                        .into_any_element()
                } else {
                    let id = part.id.clone();
                    let on_compact = self.on_compact.clone();
                    Button::new(("compact", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Minimize)
                        .label("Compact")
                        .on_click(move |_, window, cx| {
                            if let Some(handler) = &on_compact {
                                handler(&id, window, cx);
                            }
                        })
                        .into_any_element()
                }
            });
            let head = h_flex()
                .id(("part", ix))
                .gap_2()
                .h(px(30.))
                .px_1()
                .rounded(radius)
                .when(hovered == Some(ix), |this| this.bg(chip))
                .on_hover(hover(ix))
                .when(has_items, |this| {
                    let state = state.clone();
                    let id = part.id.clone();
                    this.cursor_pointer()
                        .aria_expanded(is_open)
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                if !state.open.remove(&id) {
                                    state.open.insert(id.clone());
                                }
                                cx.notify();
                            })
                        })
                })
                .child(div().flex_none().size_2().rounded(px(2.)).bg(colors[ix]))
                .child(div().min_w_0().truncate().child(part.label.clone()))
                .when(has_items, |this| {
                    this.child(
                        Icon::new(if is_open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .xsmall()
                        .text_color(quiet),
                    )
                })
                .child(div().flex_1())
                .children(compact)
                .child(
                    div()
                        .flex_none()
                        .w(px(48.))
                        .text_right()
                        .font_medium()
                        .child(tokens_words(*tokens)),
                );
            let list = (is_open && has_items).then(|| {
                v_flex()
                    .pl(px(22.))
                    .children(items.iter().map(|item| {
                        let on_remove = self.on_remove_item.clone();
                        let (part_id, item_id) = (part.id.clone(), item.id.clone());
                        h_flex()
                            .gap_2()
                            .h(px(26.))
                            .pr_1()
                            .text_caption()
                            .child(Icon::new(IconName::File).xsmall().text_color(quiet))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(item.label.clone()),
                            )
                            .child(div().text_color(quiet).child(tokens_words(item.tokens)))
                            .when(!part.locked, |this| {
                                this.child(
                                    Button::new(SharedString::from(format!(
                                        "remove-{part_id}-{item_id}"
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .accessibility_label(format!("Remove {}", item.label))
                                    .on_click(
                                        move |_, window, cx| {
                                            if let Some(handler) = &on_remove {
                                                handler(&part_id, &item_id, window, cx);
                                            }
                                        },
                                    ),
                                )
                            })
                    }))
                    .when(items.is_empty(), |this| {
                        this.child(
                            div()
                                .h(px(26.))
                                .text_caption()
                                .text_color(quiet)
                                .child("Nothing left"),
                        )
                    })
            });
            v_flex().child(head).children(list)
        });
        let rows: Vec<_> = rows.collect();

        let left = self.limit.saturating_sub(used);
        let footer_words = if full {
            "Nearly full: remove something or compact".to_string()
        } else {
            format!("{} left", tokens_words(left))
        };
        let action = self.action.map(|(label, handler)| {
            Button::new((self.id.clone(), "action"))
                .outline()
                .small()
                .label(label)
                .on_click(move |_, window, cx| handler(window, cx))
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(380.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.4))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_medium()
                                    .child(self.title),
                            )
                            .children(self.model.map(|model| {
                                div()
                                    .flex_none()
                                    .px_1p5()
                                    .rounded(px(5.))
                                    .bg(chip)
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(model)
                            })),
                    )
                    .child(
                        h_flex()
                            .items_end()
                            .gap_2()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_medium()
                                    .line_height(relative(1.1))
                                    .child(figure),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(quiet)
                                    .child(caption),
                            )
                            .child(tag),
                    )
                    .child(field)
                    .child(v_flex().children(rows)),
            )
            .child(
                h_flex()
                    .gap_3()
                    .min_h(px(44.))
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_caption()
                            .text_color(if warn { warning } else { quiet })
                            .child(footer_words),
                    )
                    .children(action),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_counts_shorten() {
        assert_eq!(tokens_words(950), "950");
        assert_eq!(tokens_words(4200), "4.2K");
        assert_eq!(tokens_words(200_000), "200K");
        assert_eq!(tokens_words(1_250_000), "1.3M");
    }

    #[test]
    fn every_part_with_tokens_gets_a_cell() {
        // 100 tokens a cell: 40 rounds to nothing but still gets one.
        assert_eq!(cell_counts(&[40, 1000, 0], 10_000, 100), [1, 10, 0]);
    }

    #[test]
    fn the_parts_never_take_more_than_the_field() {
        assert_eq!(cell_counts(&[9000, 9000], 10_000, 10), [9, 1]);
    }
}
