//! Select-all checklist: the kit's checkboxes under a parent box that's ticked, empty or part-way, with a count.
//!
//! The parent shows a dash when some but not all are ticked, and ticking it
//! ticks everything (or clears everything when all are ticked already).
//! Each item can carry a note on the right, such as an amount, and items
//! can be locked so they stay as they are.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, checkbox::Checkbox, h_flex,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

/// One item.
#[derive(Debug, Clone, PartialEq)]
pub struct ChecklistItem {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// A note on the right.
    pub note: Option<SharedString>,
    /// Whether it's ticked.
    pub checked: bool,
    /// Whether it stays as it is.
    pub locked: bool,
}

impl ChecklistItem {
    /// Create an unticked item.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            note: None,
            checked: false,
            locked: false,
        }
    }

    /// Add a note.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Tick it.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Lock it.
    pub fn locked(mut self) -> Self {
        self.locked = true;
        self
    }
}

/// Where the parent box stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentState {
    /// Nothing ticked.
    None,
    /// Some ticked.
    Some,
    /// Everything ticked.
    All,
}

/// The parent box's state for these items.
pub fn parent_state(items: &[ChecklistItem]) -> ParentState {
    let ticked = items.iter().filter(|i| i.checked).count();
    match ticked {
        0 => ParentState::None,
        n if n == items.len() => ParentState::All,
        _ => ParentState::Some,
    }
}

/// What ticking the parent does: every unlocked item's new state.
pub fn toggle_all(items: &[ChecklistItem]) -> Vec<(SharedString, bool)> {
    let to = parent_state(items) != ParentState::All;
    items
        .iter()
        .filter(|i| !i.locked && i.checked != to)
        .map(|i| (i.id.clone(), to))
        .collect()
}

type ChangeHandler = Rc<dyn Fn(Vec<(SharedString, bool)>, &mut Window, &mut App)>;

/// Checkboxes under a select-all parent.
#[derive(IntoElement)]
pub struct SelectAllChecklist {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    items: Vec<ChecklistItem>,
    on_change: Option<ChangeHandler>,
}

impl SelectAllChecklist {
    /// Create a checklist with a title for its parent box.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, items: Vec<ChecklistItem>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            items,
            on_change: None,
        }
    }

    /// Run with the items whose state should change.
    pub fn on_change(mut self, handler: impl Fn(Vec<(SharedString, bool)>, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for SelectAllChecklist {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SelectAllChecklist {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let accent = theme.primary;
        let fill = theme.background;
        let state = parent_state(&self.items);
        let ticked = self.items.iter().filter(|i| i.checked).count();
        let changes = toggle_all(&self.items);
        let on_all = self.on_change.clone();

        let parent = h_flex()
            .id(ElementId::NamedChild(self.id.clone().into(), "all".into()))
            .gap_2()
            .py_1p5()
            .cursor_pointer()
            .aria_label(format!("{}, {ticked} of {} ticked", self.title, self.items.len()))
            .aria_selected(state == ParentState::All)
            .on_click(move |_, window, cx| {
                if let Some(on_all) = &on_all {
                    on_all(changes.clone(), window, cx);
                }
            })
            .child(
                div()
                    .size(px(16.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if state == ParentState::None { line } else { accent })
                    .when(state != ParentState::None, |this| this.bg(accent))
                    .map(|this| match state {
                        ParentState::All => this.child(Icon::new(IconName::Check).xsmall().text_color(fill)),
                        ParentState::Some => this.child(div().w(px(8.)).h(px(2.)).rounded_full().bg(fill)),
                        ParentState::None => this,
                    }),
            )
            .child(div().flex_1().font_medium().child(self.title.clone()))
            .child(div().text_xs().text_color(quiet).child(format!("{ticked} of {}", self.items.len())));

        let rows: Vec<_> = self
            .items
            .iter()
            .map(|item| {
                let on_change = self.on_change.clone();
                let id = item.id.clone();
                h_flex()
                    .gap_2()
                    .py_1()
                    .pl(px(24.))
                    .child(
                        Checkbox::new(ElementId::NamedChild(self.id.clone().into(), format!("item-{}", item.id).into()))
                            .checked(item.checked)
                            .label(item.label.clone())
                            .when(item.locked, |this| this.tooltip("Locked"))
                            .on_click(move |checked, window, cx| {
                                if let Some(on_change) = &on_change {
                                    on_change(vec![(id.clone(), *checked)], window, cx);
                                }
                            })
                            .disabled(item.locked),
                    )
                    .child(div().flex_1())
                    .when(item.locked, |this| this.child(Icon::new(IconName::Ban).xsmall().text_color(quiet)))
                    .children(item.note.clone().map(|n| div().text_xs().text_color(quiet).child(n)))
            })
            .collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(480.))
            .text_sm()
            .child(div().border_b_1().border_color(line).child(parent))
            .children(rows)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parent_ticks_or_clears_whats_unlocked() {
        let items = vec![
            ChecklistItem::new("a", "A").checked(true),
            ChecklistItem::new("b", "B"),
            ChecklistItem::new("c", "C").locked(),
        ];
        assert_eq!(parent_state(&items), ParentState::Some);
        assert_eq!(toggle_all(&items), [("b".into(), true)]);
        let all = vec![ChecklistItem::new("a", "A").checked(true), ChecklistItem::new("b", "B").checked(true)];
        assert_eq!(parent_state(&all), ParentState::All);
        assert_eq!(toggle_all(&all).len(), 2);
    }
}
