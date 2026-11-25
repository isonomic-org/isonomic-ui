// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Catch-up summary: what happened while you were away, as a card that folds to a divider.
//!
//! Open, the card counts what finished, what needs you and what failed, and
//! lists the moments worth knowing, each with a way to jump to it. Folded,
//! it becomes a thin divider in the conversation that still says how long
//! you were away and how much needs you. Folding and every jump go to the owner.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, button::Button, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// What kind of moment an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatchUpKind {
    /// Something finished.
    Done,
    /// Something waits on you.
    NeedsYou,
    /// Something went wrong.
    Failed,
    /// Worth knowing, nothing to do.
    Note,
}

impl CatchUpKind {
    fn tone(self) -> Tone {
        match self {
            Self::Done => Tone::Success,
            Self::NeedsYou => Tone::Warning,
            Self::Failed => Tone::Danger,
            Self::Note => Tone::Neutral,
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Done => IconName::CircleCheck,
            Self::NeedsYou => IconName::Bell,
            Self::Failed => IconName::CircleX,
            Self::Note => IconName::Info,
        }
    }
}

/// One moment worth knowing.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchUpItem {
    /// Identifies it.
    pub id: SharedString,
    /// What kind of moment.
    pub kind: CatchUpKind,
    /// What happened, in a sentence.
    pub text: SharedString,
    /// When, as written.
    pub when: SharedString,
    /// The jump button's label, if it has one.
    pub action: Option<SharedString>,
}

impl CatchUpItem {
    /// Create an item.
    pub fn new(
        id: impl Into<SharedString>,
        kind: CatchUpKind,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            text: text.into(),
            when: SharedString::default(),
            action: None,
        }
    }

    /// Say when it happened.
    pub fn when(mut self, when: impl Into<SharedString>) -> Self {
        self.when = when.into();
        self
    }

    /// Give it a jump button.
    pub fn action(mut self, label: impl Into<SharedString>) -> Self {
        self.action = Some(label.into());
        self
    }
}

/// How many items are of a kind.
pub fn count_kind(items: &[CatchUpItem], kind: CatchUpKind) -> usize {
    items.iter().filter(|i| i.kind == kind).count()
}

/// Items in reading order: what needs you first, then failures, then the rest as given.
pub fn reading_order(items: &[CatchUpItem]) -> Vec<&CatchUpItem> {
    let rank = |kind| match kind {
        CatchUpKind::NeedsYou => 0,
        CatchUpKind::Failed => 1,
        CatchUpKind::Done => 2,
        CatchUpKind::Note => 3,
    };
    let mut items: Vec<_> = items.iter().collect();
    items.sort_by_key(|item| rank(item.kind));
    items
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type ItemHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A summary of what happened while away.
#[derive(IntoElement)]
pub struct CatchUpSummary {
    id: ElementId,
    style: StyleRefinement,
    away: SharedString,
    summary: SharedString,
    items: Vec<CatchUpItem>,
    folded: bool,
    on_fold: Option<Handler>,
    on_open: Option<ItemHandler>,
}

impl CatchUpSummary {
    /// Create a summary for an absence, such as "3 hours".
    pub fn new(
        id: impl Into<ElementId>,
        away: impl Into<SharedString>,
        items: Vec<CatchUpItem>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            away: away.into(),
            summary: SharedString::default(),
            items,
            folded: false,
            on_fold: None,
            on_open: None,
        }
    }

    /// A sentence that sums it up.
    pub fn summary(mut self, text: impl Into<SharedString>) -> Self {
        self.summary = text.into();
        self
    }

    /// Fold the card to a divider.
    pub fn folded(mut self, folded: bool) -> Self {
        self.folded = folded;
        self
    }

    /// Run when the card is folded or unfolded.
    pub fn on_fold(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_fold = Some(Rc::new(handler));
        self
    }

    /// Run when an item's jump button is clicked.
    pub fn on_open(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl Styled for CatchUpSummary {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for CatchUpSummary {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let radius_lg = theme.radius_lg;
        let warning = Tone::Warning.color(cx);
        let needs = count_kind(&self.items, CatchUpKind::NeedsYou);
        let failed = count_kind(&self.items, CatchUpKind::Failed);
        let done = count_kind(&self.items, CatchUpKind::Done);
        let on_fold = self.on_fold.clone();
        let toggle = move |_: &isonomic_ui::ClickEvent, window: &mut Window, cx: &mut App| {
            if let Some(on_fold) = &on_fold {
                on_fold(window, cx);
            }
        };

        if self.folded {
            let rule = || div().flex_1().h(px(1.)).bg(line);
            return h_flex()
                .id(self.id.clone())
                .w_full()
                .gap_3()
                .py_1()
                .cursor_pointer()
                .text_caption()
                .text_color(quiet)
                .role(Role::Button)
                .aria_label(format!(
                    "While you were away, {}. {needs} need you. Show summary",
                    self.away
                ))
                .aria_expanded(false)
                .on_click(toggle)
                .child(rule())
                .child(
                    h_flex()
                        .flex_none()
                        .gap_1p5()
                        .child(Icon::new(IconName::Time).xsmall())
                        .child(format!("While you were away \u{b7} {}", self.away))
                        .when(needs > 0, |this| {
                            this.child(
                                div()
                                    .text_color(warning)
                                    .font_medium()
                                    .child(format!("\u{b7} {needs} need you")),
                            )
                        })
                        .child(Icon::new(IconName::ChevronDown).xsmall()),
                )
                .child(rule())
                .refine_style(&self.style)
                .into_any_element();
        }

        let stat = |count: usize, label: &'static str, color: Hsla| {
            h_flex()
                .gap_1p5()
                .child(
                    div()
                        .text_lg()
                        .font_medium()
                        .text_color(color)
                        .child(count.to_string()),
                )
                .child(div().text_caption().text_color(quiet).child(label))
        };

        let rows: Vec<_> = reading_order(&self.items)
            .into_iter()
            .map(|item| {
                let tone = item.kind.tone();
                let open = self.on_open.clone();
                let id = item.id.clone();
                h_flex()
                    .gap_2()
                    .py_1p5()
                    .child(
                        div()
                            .size(px(22.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(tone.tint(cx))
                            .child(
                                Icon::new(item.kind.icon())
                                    .xsmall()
                                    .text_color(tone.color(cx)),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().line_height(relative(1.35)).child(item.text.clone()))
                            .when(!item.when.is_empty(), |this| {
                                this.child(
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(item.when.clone()),
                                )
                            }),
                    )
                    .children(item.action.clone().map(|label| {
                        Button::new(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("open-{}", item.id).into(),
                        ))
                        .xsmall()
                        .outline()
                        .label(label.clone())
                        .accessibility_label(format!("{label}: {}", item.text))
                        .on_click(move |_, window, cx| {
                            if let Some(open) = &open {
                                open(&id, window, cx);
                            }
                        })
                    }))
            })
            .collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(560.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .role(Role::Region)
            .aria_label(format!("While you were away, {}", self.away))
            .child(
                h_flex()
                    .id(ElementId::NamedChild(self.id.clone().into(), "head".into()))
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .cursor_pointer()
                    .border_b_1()
                    .border_color(line)
                    .role(Role::Button)
                    .aria_label("Fold the summary")
                    .aria_expanded(true)
                    .on_click(toggle)
                    .child(Icon::new(IconName::Time).small().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .font_medium()
                            .child(format!("While you were away \u{b7} {}", self.away)),
                    )
                    .child(Icon::new(IconName::ChevronUp).xsmall().text_color(quiet)),
            )
            .child(
                v_flex()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .when(!self.summary.is_empty(), |this| {
                        this.child(
                            div()
                                .line_height(relative(1.45))
                                .child(self.summary.clone()),
                        )
                    })
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_5()
                            .child(stat(needs, "need you", Tone::Warning.color(cx)))
                            .child(stat(failed, "failed", Tone::Danger.color(cx)))
                            .child(stat(done, "done", Tone::Success.color(cx))),
                    )
                    .child(
                        v_flex()
                            .border_t_1()
                            .border_color(line)
                            .pt_1()
                            .children(rows),
                    ),
            )
            .refine_style(&self.style)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_needs_you_comes_first() {
        let items = vec![
            CatchUpItem::new("a", CatchUpKind::Done, "x"),
            CatchUpItem::new("b", CatchUpKind::Failed, "y"),
            CatchUpItem::new("c", CatchUpKind::NeedsYou, "z"),
            CatchUpItem::new("d", CatchUpKind::Done, "w"),
        ];
        let ids: Vec<_> = reading_order(&items)
            .iter()
            .map(|i| i.id.to_string())
            .collect();
        assert_eq!(ids, ["c", "b", "a", "d"]);
        assert_eq!(count_kind(&items, CatchUpKind::Done), 2);
    }
}
