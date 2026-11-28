// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The composer's working modes, its context tray and its queue of messages
//! waiting to be sent: what each holds, the arithmetic of the token budget
//! and how each is drawn.

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    StyledExt as _,
    button::{Button, ButtonGroup, ButtonVariants as _},
    h_flex, look,
    progress::Progress,
    v_flex,
};
use isonomic_ui::{
    AnyElement, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{PromptComposer, PromptComposerEvent};

/// A way of working the composer offers, such as asking, planning or acting.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerMode {
    /// Identifies it in [`PromptComposerEvent::SelectMode`].
    pub id: SharedString,
    /// Its name on the switch, such as "Plan".
    pub label: SharedString,
    /// An icon before the name.
    pub icon: Option<CatalogIcon>,
    /// A line on what the assistant does in this mode, for the tooltip and
    /// screen readers.
    pub hint: Option<SharedString>,
}

impl ComposerMode {
    /// Create a mode.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            hint: None,
        }
    }

    /// Put an icon before the name.
    pub fn icon(mut self, icon: impl Into<CatalogIcon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Say what the assistant does in this mode.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Something the assistant will read with the next message, such as a file
/// or an earlier answer, with what it costs in tokens.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextItem {
    /// Identifies it in [`PromptComposerEvent::RemoveContext`].
    pub id: SharedString,
    /// What the row says, usually a file name.
    pub name: SharedString,
    /// How many tokens it takes.
    pub tokens: u64,
    /// Added by the assistant or the app rather than the person: it shows
    /// without a remove button.
    pub pinned: bool,
}

impl ContextItem {
    /// Create an item the person can remove.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, tokens: u64) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tokens,
            pinned: false,
        }
    }

    /// Keep it in the tray: no remove button.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
}

/// A message written while the assistant was busy, waiting to be sent.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedMessage {
    /// Identifies it in the queue's events.
    pub id: SharedString,
    /// What it says.
    pub text: SharedString,
}

impl QueuedMessage {
    /// Create a queued message.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

/// A token count in short words: "820", "8.2k", "200k", "1.2M".
pub fn token_words(tokens: u64) -> String {
    match tokens {
        0..1_000 => tokens.to_string(),
        1_000..10_000 => trim_point(format!("{:.1}", tokens as f64 / 1_000.)) + "k",
        10_000..1_000_000 => format!("{}k", (tokens as f64 / 1_000.).round() as u64),
        _ => trim_point(format!("{:.1}", tokens as f64 / 1_000_000.)) + "M",
    }
}

/// "4.0" reads as "4".
fn trim_point(number: String) -> String {
    number
        .strip_suffix(".0")
        .map(str::to_string)
        .unwrap_or(number)
}

/// How many tokens the tray holds in all.
pub(super) fn context_total(items: &[ContextItem]) -> u64 {
    items.iter().map(|item| item.tokens).sum()
}

/// How much of the budget is used, from 0 to 1 and past it when over.
pub(super) fn budget_fraction(used: u64, budget: u64) -> f32 {
    if budget == 0 {
        return 0.;
    }
    used as f32 / budget as f32
}

/// How full the budget reads: under 80 % is fine, then near, then over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BudgetLevel {
    Fine,
    Near,
    Over,
}

pub(super) fn budget_level(used: u64, budget: Option<u64>) -> BudgetLevel {
    match budget {
        Some(budget) if used > budget => BudgetLevel::Over,
        Some(budget) if budget_fraction(used, budget) >= 0.8 => BudgetLevel::Near,
        _ => BudgetLevel::Fine,
    }
}

/// The tray's summary: "3 items, 12.4k of 200k tokens".
pub(super) fn tray_words(items: &[ContextItem], budget: Option<u64>) -> String {
    let count = match items.len() {
        1 => "1 item".to_string(),
        n => format!("{n} items"),
    };
    let used = token_words(context_total(items));
    match budget {
        Some(budget) => format!("{count}, {used} of {} tokens", token_words(budget)),
        None => format!("{count}, {used} tokens"),
    }
}

/// The mode the mode key moves to: the next one, or the one before with
/// Shift, wrapping round.
pub(super) fn next_mode(current: Option<usize>, count: usize, back: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (current, back) {
        (None, false) => 0,
        (None, true) => count - 1,
        (Some(ix), false) => (ix + 1) % count,
        (Some(ix), true) => (ix + count - 1) % count,
    })
}

/// What a screen reader hears for a mode's button.
pub(super) fn mode_words(mode: &ComposerMode, on: bool) -> String {
    let mut words = format!("{} mode", mode.label);
    if on {
        words.push_str(", on");
    }
    if let Some(hint) = &mode.hint {
        words.push_str(&format!(". {hint}"));
    }
    words
}

impl PromptComposer {
    /// The switch between modes, for the start of the toolbar.
    pub(super) fn render_modes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.modes.is_empty() {
            return None;
        }
        let current = self.current_mode();
        let ids: Vec<SharedString> = self.modes.iter().map(|mode| mode.id.clone()).collect();
        Some(
            ButtonGroup::new("composer-modes")
                .small()
                .compact()
                .children(self.modes.iter().enumerate().map(|(ix, mode)| {
                    let on = current == Some(ix);
                    Button::new(ElementId::Name(format!("mode-{}", mode.id).into()))
                        .ghost()
                        .small()
                        .selected(on)
                        .disabled(self.disabled)
                        .when_some(mode.icon, |this, icon| this.icon(Icon::new(icon)))
                        .label(mode.label.clone())
                        .accessibility_label(mode_words(mode, on))
                        .tooltip(match &mode.hint {
                            Some(hint) => format!("{hint}  Shift+Tab"),
                            None => "Switch mode  Shift+Tab".to_string(),
                        })
                }))
                .on_click(cx.listener(move |_, picked: &Vec<usize>, _, cx| {
                    if let Some(id) = picked.first().and_then(|ix| ids.get(*ix)) {
                        cx.emit(PromptComposerEvent::SelectMode(id.clone()));
                    }
                }))
                .into_any_element(),
        )
    }

    /// The tray of context above the field: a summary line with the budget
    /// bar that opens into a row per item.
    pub(super) fn render_tray(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.context.is_empty() {
            return None;
        }
        let used = context_total(&self.context);
        let level = budget_level(used, self.token_budget);
        let open = self.tray_open;
        let fade = look::hover_fade("context-tray-toggle", self.tray_hovered, window, cx);
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let ink = theme.foreground;
        let line = theme.border;
        let wash = look::ink::wash(cx);
        let bar_color = match level {
            BudgetLevel::Fine => theme.info,
            BudgetLevel::Near => theme.warning,
            BudgetLevel::Over => theme.danger,
        };
        let words = tray_words(&self.context, self.token_budget);

        let rows = open.then(|| {
            v_flex()
                .id("context-tray-items")
                .role(Role::List)
                .aria_label("Context")
                .pt_0p5()
                .children(self.context.iter().map(|item| {
                    let id = item.id.clone();
                    let share = self
                        .token_budget
                        .map(|budget| budget_fraction(item.tokens, budget))
                        .unwrap_or(0.);
                    h_flex()
                        .id(ElementId::Name(format!("context-{}", item.id).into()))
                        .role(Role::ListItem)
                        .aria_label(format!(
                            "{}, {} tokens{}",
                            item.name,
                            token_words(item.tokens),
                            if item.pinned { ", kept" } else { "" }
                        ))
                        .gap_2()
                        .h(px(26.))
                        .pl_1()
                        .text_caption()
                        .child(Icon::new(IconName::FileText).xsmall().text_color(quiet))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(ink)
                                .child(item.name.clone()),
                        )
                        .when(self.token_budget.is_some(), |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .w(px(48.))
                                    .h(px(3.))
                                    .rounded_full()
                                    .bg(wash)
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded_full()
                                            .bg(quiet.opacity(0.6))
                                            .w(px(48. * share.clamp(0.02, 1.))),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .flex_none()
                                .w(px(40.))
                                .text_right()
                                .text_color(quiet)
                                .child(token_words(item.tokens)),
                        )
                        .child(div().flex_none().w(px(24.)).when(!item.pinned, |this| {
                            this.child(
                                Button::new(ElementId::Name(
                                    format!("remove-context-{}", item.id).into(),
                                ))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .disabled(self.disabled)
                                .accessibility_label(format!("Remove {} from context", item.name))
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(PromptComposerEvent::RemoveContext(id.clone()))
                                    },
                                )),
                            )
                        }))
                }))
        });

        Some(
            v_flex()
                .mx_1()
                .px_1()
                .pb_1()
                .border_b_1()
                .border_color(line)
                .child(
                    h_flex()
                        .id("context-tray-toggle")
                        .role(Role::Button)
                        .aria_label(format!(
                            "Context: {words}, {}",
                            if open { "shown" } else { "hidden" }
                        ))
                        .aria_expanded(open)
                        .tab_index(0)
                        .gap_2()
                        .h(px(26.))
                        .px_1()
                        .rounded(theme.radius)
                        .bg(look::mix(wash.opacity(0.), wash, fade))
                        .cursor_pointer()
                        .text_caption()
                        .text_color(quiet)
                        .on_hover(cx.listener(|this, over: &bool, _, cx| {
                            if this.tray_hovered != *over {
                                this.tray_hovered = *over;
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tray_open = !this.tray_open;
                            cx.notify();
                        }))
                        .child(
                            Icon::new(if open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .xsmall(),
                        )
                        .child(
                            div()
                                .flex_none()
                                .font_medium()
                                .text_color(ink)
                                .child("Context"),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(words))
                        .when_some(self.token_budget, |this, budget| {
                            this.child(
                                Progress::new("context-budget")
                                    .small()
                                    .flex_none()
                                    .w(px(72.))
                                    .value(budget_fraction(used, budget).min(1.) * 100.)
                                    .color(bar_color)
                                    .accessibility_label(format!(
                                        "Context budget, {} of {} tokens",
                                        token_words(used),
                                        token_words(budget)
                                    )),
                            )
                        })
                        .when(level == BudgetLevel::Over, |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .text_color(theme.danger)
                                    .child("Over budget"),
                            )
                        }),
                )
                .children(rows)
                .into_any_element(),
        )
    }

    /// The messages waiting to be sent, in order, each with send now and
    /// remove.
    pub(super) fn render_queue(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.queue.is_empty() {
            return None;
        }
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let count = self.queue.len();
        Some(
            v_flex()
                .id("composer-queue")
                .role(Role::List)
                .aria_label(match count {
                    1 => "1 message waiting to send".to_string(),
                    n => format!("{n} messages waiting to send"),
                })
                .mx_1()
                .pb_1()
                .border_b_1()
                .border_color(line)
                .child(
                    h_flex()
                        .h(px(22.))
                        .px_1()
                        .gap_1p5()
                        .text_eyebrow()
                        .text_color(quiet)
                        .child(Icon::new(IconName::Time).xsmall())
                        .child(format!("QUEUED \u{b7} {count}")),
                )
                .children(self.queue.iter().enumerate().map(|(ix, queued)| {
                    let now = queued.id.clone();
                    let drop = queued.id.clone();
                    let edit = queued.id.clone();
                    h_flex()
                        .id(ElementId::Name(format!("queued-{}", queued.id).into()))
                        .role(Role::ListItem)
                        .aria_label(format!("Queued {}: {}", ix + 1, queued.text))
                        .gap_1()
                        .min_h(px(28.))
                        .pl_1()
                        .text_caption()
                        .child(
                            div()
                                .flex_none()
                                .w(px(14.))
                                .text_color(quiet)
                                .child((ix + 1).to_string()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(queued.text.clone()),
                        )
                        .child(
                            Button::new(ElementId::Name(
                                format!("edit-queued-{}", queued.id).into(),
                            ))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Replace)
                            .tooltip("Edit")
                            .accessibility_label(format!("Edit queued message {}", ix + 1))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(PromptComposerEvent::EditQueued(edit.clone()))
                                },
                            )),
                        )
                        .child(
                            Button::new(ElementId::Name(
                                format!("send-queued-{}", queued.id).into(),
                            ))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowUp)
                            .tooltip("Send now")
                            .accessibility_label(format!("Send queued message {} now", ix + 1))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(PromptComposerEvent::SendQueuedNow(now.clone()))
                                },
                            )),
                        )
                        .child(
                            Button::new(ElementId::Name(
                                format!("remove-queued-{}", queued.id).into(),
                            ))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Remove")
                            .accessibility_label(format!("Remove queued message {}", ix + 1))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(PromptComposerEvent::RemoveQueued(drop.clone()))
                                },
                            )),
                        )
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_counts_read_in_short_words() {
        assert_eq!(token_words(820), "820");
        assert_eq!(token_words(4_000), "4k");
        assert_eq!(token_words(12_400), "12k");
        assert_eq!(token_words(1_240), "1.2k");
        assert_eq!(token_words(200_000), "200k");
        assert_eq!(token_words(1_000_000), "1M");
        assert_eq!(token_words(1_240_000), "1.2M");
    }

    #[test]
    fn the_budget_reads_fine_near_or_over() {
        assert_eq!(budget_level(10, None), BudgetLevel::Fine);
        assert_eq!(budget_level(79, Some(100)), BudgetLevel::Fine);
        assert_eq!(budget_level(80, Some(100)), BudgetLevel::Near);
        assert_eq!(budget_level(100, Some(100)), BudgetLevel::Near);
        assert_eq!(budget_level(101, Some(100)), BudgetLevel::Over);
        assert_eq!(budget_fraction(5, 0), 0.);
        assert_eq!(budget_fraction(50, 200), 0.25);
    }

    #[test]
    fn the_tray_sums_its_items() {
        let items = [
            ContextItem::new("a", "ledger.csv", 8_200),
            ContextItem::new("b", "notes.md", 4_200).pinned(true),
        ];
        assert_eq!(context_total(&items), 12_400);
        assert_eq!(
            tray_words(&items, Some(200_000)),
            "2 items, 12k of 200k tokens"
        );
        assert_eq!(tray_words(&items[..1], None), "1 item, 8.2k tokens");
    }

    #[test]
    fn the_mode_key_cycles_both_ways() {
        assert_eq!(next_mode(None, 3, false), Some(0));
        assert_eq!(next_mode(None, 3, true), Some(2));
        assert_eq!(next_mode(Some(2), 3, false), Some(0));
        assert_eq!(next_mode(Some(0), 3, true), Some(2));
        assert_eq!(next_mode(Some(0), 0, false), None);
    }

    #[test]
    fn a_mode_says_whether_it_is_on_and_what_it_does() {
        let mode = ComposerMode::new("plan", "Plan").hint("Writes a plan first");
        assert_eq!(
            mode_words(&mode, true),
            "Plan mode, on. Writes a plan first"
        );
        assert_eq!(
            mode_words(&ComposerMode::new("ask", "Ask"), false),
            "Ask mode"
        );
    }
}
