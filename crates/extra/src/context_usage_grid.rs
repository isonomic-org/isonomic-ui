// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Context usage grid: what fills a model's context budget.
//!
//! The budget is a field of small cells, coloured by the part that takes
//! them up: instructions, tools, attached files, the conversation. Pointing
//! at a part lights its cells and puts its size in the headline. Files can be
//! removed one by one, and a long part can be compacted: the card asks, and
//! its owner passes the parts as they are afterwards.
//!
//! Opt-in extras: a slim meter (one bar split by part, with a tick where
//! the warning starts) in place of the field, and a near-limit warning
//! that says how much room is left and offers to compact the whole
//! context, with a working state while that runs.

use std::{collections::HashSet, rc::Rc};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    h_flex, look, v_flex,
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

/// How the budget is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextDisplay {
    /// A field of small cells.
    #[default]
    Cells,
    /// One slim bar split by part, with a tick where the warning starts.
    Meter,
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
    display: ContextDisplay,
    on_compact_all: Option<ActionHandler>,
    compacting: bool,
    frees: Option<u64>,
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
            display: ContextDisplay::default(),
            on_compact_all: None,
            compacting: false,
            frees: None,
        }
    }

    /// Draw the budget as a field of cells (the default) or a slim meter.
    pub fn display(mut self, display: ContextDisplay) -> Self {
        self.display = display;
        self
    }

    /// Near the limit, warn and offer to compact the whole context; run
    /// when Compact is pressed. Pass `compacting` while it runs, then the
    /// parts at their new sizes.
    pub fn on_compact_all(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_compact_all = Some(Rc::new(handler));
        self
    }

    /// Say the whole context is being compacted: Compact shows it working.
    pub fn compacting(mut self, compacting: bool) -> Self {
        self.compacting = compacting;
        self
    }

    /// Say about how many tokens compacting would free, for the warning.
    pub fn frees(mut self, tokens: u64) -> Self {
        self.frees = Some(tokens);
        self
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

/// The near-limit warning's title and words, or `None` below the warning
/// share. From 0.95 of the limit the budget counts as full.
pub fn limit_warning(
    used: u64,
    limit: u64,
    warn_at: f32,
    frees: Option<u64>,
) -> Option<(&'static str, String)> {
    let share = used as f32 / limit.max(1) as f32;
    if share < warn_at {
        return None;
    }
    let left = limit.saturating_sub(used);
    let title = if share >= 0.95 {
        "Context is full"
    } else {
        "Context is nearly full"
    };
    let mut words = format!(
        "{}% used, {} tokens left. Older turns will be dropped soon.",
        (share * 100.).round() as u32,
        tokens_words(left)
    );
    if let Some(frees) = frees.filter(|frees| *frees > 0) {
        words.push_str(&format!(" Compacting frees about {}.", tokens_words(frees)));
    }
    Some((title, words))
}

/// What the meter's bar says out loud: the total, then each part.
pub fn meter_words(labels: &[SharedString], sizes: &[u64], limit: u64) -> String {
    let used: u64 = sizes.iter().sum();
    let mut words = format!(
        "{} of {} tokens used, {}%",
        tokens_words(used),
        tokens_words(limit),
        (used as f64 / limit.max(1) as f64 * 100.).round() as u32
    );
    for (label, size) in labels.iter().zip(sizes) {
        if *size > 0 {
            words.push_str(&format!(", {label} {}", tokens_words(*size)));
        }
    }
    words
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
        let ring = theme.ring;
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
        let cell_field = v_flex()
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
        let field = match self.display {
            ContextDisplay::Cells => cell_field.into_any_element(),
            ContextDisplay::Meter => {
                let labels: Vec<SharedString> =
                    self.parts.iter().map(|part| part.label.clone()).collect();
                let spoken = meter_words(&labels, &sizes, self.limit);
                let limit = self.limit.max(1) as f32;
                let segments = sizes.iter().enumerate().map(|(ix, size)| {
                    let lit = hovered.is_none_or(|hovered| hovered == ix);
                    div()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("segment-{ix}").into(),
                        ))
                        .h_full()
                        .flex_basis(relative((*size as f32 / limit).min(1.)))
                        .flex_shrink(1.)
                        .bg(colors[ix].opacity(if lit { 0.85 } else { 0.25 }))
                        .when(ix > 0, |this| this.border_l_1().border_color(fill))
                        .on_hover(hover(ix))
                });
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                "meter".into(),
                            ))
                            .relative()
                            .h(px(10.))
                            .role(isonomic_ui::Role::Meter)
                            .aria_label(spoken)
                            .child(
                                h_flex()
                                    .size_full()
                                    .rounded(px(3.))
                                    .overflow_hidden()
                                    .bg(chip)
                                    .children(segments),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(-3.))
                                    .bottom(px(-3.))
                                    .left(relative(self.warn_at.clamp(0., 1.)))
                                    .w(px(1.5))
                                    .rounded_full()
                                    .bg(if warn { warning } else { quiet.opacity(0.6) }),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .h(px(16.))
                            .text_size(px(10.))
                            .line_height(px(16.))
                            .text_color(quiet)
                            .child(div().absolute().left_0().child("0"))
                            .child(
                                div()
                                    .absolute()
                                    .left(relative(self.warn_at.clamp(0., 1.)))
                                    .w(px(0.))
                                    .flex()
                                    .justify_center()
                                    .child(div().flex_none().whitespace_nowrap().child(format!(
                                        "warn {}%",
                                        (self.warn_at * 100.).round() as u32
                                    ))),
                            )
                            .child(div().absolute().right_0().child(tokens_words(self.limit))),
                    )
                    .into_any_element()
            }
        };

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
            let lit = look::hover_fade(
                ElementId::NamedChild(self.id.clone().into(), format!("head-{ix}").into()),
                hovered == Some(ix),
                window,
                cx,
            );
            let head = h_flex()
                .id(("part", ix))
                .gap_2()
                .h(px(30.))
                .px_1()
                .rounded(radius)
                .bg(look::mix(chip.opacity(0.), chip, lit))
                .on_hover(hover(ix))
                .when(has_items, |this| {
                    let state = state.clone();
                    let id = part.id.clone();
                    this.cursor_pointer()
                        .tab_index(0)
                        .focus_visible(move |s| s.border_1().border_color(ring))
                        .role(isonomic_ui::Role::Button)
                        .aria_label(format!(
                            "{}, {} tokens, {}",
                            part.label,
                            tokens_words(*tokens),
                            if is_open { "open" } else { "closed" }
                        ))
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
        let warning_note = self
            .on_compact_all
            .is_some()
            .then(|| limit_warning(used, self.limit, self.warn_at, self.frees))
            .flatten()
            .map(|(title, words)| {
                Alert::warning(
                    ElementId::NamedChild(self.id.clone().into(), "warning".into()),
                    words,
                )
                .small()
                .title(title)
            });
        let compact_all = self
            .on_compact_all
            .clone()
            .filter(|_| warn || self.compacting)
            .map(|handler| {
                Button::new((self.id.clone(), "compact-all"))
                    .when(full, |this| this.primary())
                    .when(!full, |this| this.outline())
                    .small()
                    .icon(IconName::Minimize)
                    .label(if self.compacting {
                        "Compacting…"
                    } else {
                        "Compact"
                    })
                    .loading(self.compacting)
                    .disabled(self.compacting)
                    .on_click(move |_, window, cx| handler(window, cx))
            });
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
                    .children(warning_note)
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
                    .children(compact_all)
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
    fn warning_starts_at_the_share_and_names_the_room_left() {
        assert_eq!(limit_warning(70_000, 100_000, 0.8, None), None);
        let (title, words) = limit_warning(84_000, 100_000, 0.8, Some(30_000)).unwrap();
        assert_eq!(title, "Context is nearly full");
        assert_eq!(
            words,
            "84% used, 16K tokens left. Older turns will be dropped soon. Compacting frees about 30K."
        );
        let (title, words) = limit_warning(97_000, 100_000, 0.8, Some(0)).unwrap();
        assert_eq!(title, "Context is full");
        assert!(!words.contains("Compacting"));
    }

    #[test]
    fn meter_speaks_the_total_then_each_part() {
        let labels: Vec<SharedString> = vec!["Tools".into(), "Files".into(), "Notes".into()];
        assert_eq!(
            meter_words(&labels, &[9_800, 0, 2_700], 128_000),
            "12.5K of 128K tokens used, 10%, Tools 9.8K, Notes 2.7K"
        );
    }

    #[test]
    fn the_parts_never_take_more_than_the_field() {
        assert_eq!(cell_counts(&[9000, 9000], 10_000, 10), [9, 1]);
    }
}
