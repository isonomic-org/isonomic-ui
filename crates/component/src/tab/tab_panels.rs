// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The content under a [`super::TabBar`]: one panel per tab, built only when
//! it is shown.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use isonomic_ui_base::animation::{EffectTransition, cubic_bezier};

use crate::{ActiveTheme as _, StyledExt as _};

type PanelBuilder = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>;

/// What a panel does on this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelMount {
    /// The selected panel: built and shown.
    Shown,
    /// A panel seen before and kept alive: built, laid out of sight, so the
    /// state inside it (a scroll position, a half-typed field) survives.
    Hidden,
    /// Not built at all.
    Skipped,
}

/// Which panels to build this frame. Only the selected panel is built,
/// unless `keep_mounted` holds on to every panel shown before.
pub fn panel_mounts(
    selected: Option<usize>,
    visited: &[bool],
    keep_mounted: bool,
    count: usize,
) -> Vec<PanelMount> {
    (0..count)
        .map(|ix| {
            if selected == Some(ix) {
                PanelMount::Shown
            } else if keep_mounted && visited.get(ix).copied().unwrap_or(false) {
                PanelMount::Hidden
            } else {
                PanelMount::Skipped
            }
        })
        .collect()
}

/// The panels' memory across frames.
#[derive(Default)]
struct PanelMemory {
    visited: Vec<bool>,
    last_selected: Option<usize>,
    /// Bumped on every switch; keys the entrance fade so it restarts.
    epoch: u64,
}

impl PanelMemory {
    /// Notes the selection this frame. Returns whether it changed since the
    /// last frame (the first frame is not a change).
    fn select(&mut self, selected: Option<usize>, count: usize) -> bool {
        self.visited.resize(count, false);
        if let Some(ix) = selected.filter(|ix| *ix < count) {
            self.visited[ix] = true;
        }
        let changed = self.last_selected.is_some() && self.last_selected != selected;
        if self.last_selected != selected {
            self.last_selected = selected;
        }
        if changed {
            self.epoch += 1;
        }
        changed
    }
}

/// The panels a [`super::TabBar`] switches between.
///
/// Each panel is given as a closure, and only the selected one is called, so
/// a tab nobody opens costs nothing. With [`TabPanels::keep_mounted`] a panel
/// shown once stays alive out of sight. A switch fades the new panel in and
/// lifts it 4px, over the theme's normal duration; under reduced motion it
/// appears at once.
#[derive(IntoElement)]
pub struct TabPanels {
    id: ElementId,
    selected_index: Option<usize>,
    keep_mounted: bool,
    panels: Vec<(SharedString, PanelBuilder)>,
    style: StyleRefinement,
}

impl TabPanels {
    /// Creates an empty set of panels.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            selected_index: None,
            keep_mounted: false,
            panels: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Which panel shows; pass the same index as the bar's.
    pub fn selected_index(mut self, ix: usize) -> Self {
        self.selected_index = Some(ix);
        self
    }

    /// Keep every panel shown before alive when another one is selected,
    /// rather than dropping it. Off by default.
    pub fn keep_mounted(mut self, keep: bool) -> Self {
        self.keep_mounted = keep;
        self
    }

    /// Adds a panel. `label` names it for a screen reader (usually the tab's
    /// label); `build` runs only while the panel is shown or kept alive.
    pub fn panel<E: IntoElement>(
        mut self,
        label: impl Into<SharedString>,
        build: impl FnOnce(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.panels.push((
            label.into(),
            Box::new(move |window, cx| build(window, cx).into_any_element()),
        ));
        self
    }
}

impl Styled for TabPanels {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TabPanels {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.panels.len();
        let memory = window
            .use_keyed_state(
                ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "memory".into()),
                cx,
                |_, _| Rc::new(RefCell::new(PanelMemory::default())),
            )
            .read(cx)
            .clone();
        let (mounts, epoch) = {
            let mut memory = memory.borrow_mut();
            memory.select(self.selected_index, count);
            (
                panel_mounts(
                    self.selected_index,
                    &memory.visited,
                    self.keep_mounted,
                    count,
                ),
                memory.epoch,
            )
        };
        let animate = epoch > 0 && !cx.reduce_motion();
        let duration = cx.theme().motion.duration_normal;
        let id = self.id.clone();

        let mut root = div().id(self.id).relative().refine_style(&self.style);
        for (ix, ((label, build), mount)) in self.panels.into_iter().zip(mounts).enumerate() {
            if mount == PanelMount::Skipped {
                continue;
            }
            let content = build(window, cx);
            let panel = div()
                .id(ElementId::NamedChild(
                    std::sync::Arc::new(id.clone()),
                    format!("panel-{ix}").into(),
                ))
                .role(Role::TabPanel)
                .aria_label(label)
                .relative()
                .when(mount == PanelMount::Hidden, |this| this.hidden())
                .child(content);
            root = if mount == PanelMount::Shown && animate {
                root.child(
                    EffectTransition::new(duration)
                        .ease(cubic_bezier(0.22, 1., 0.36, 1.))
                        .fade(0., 1.)
                        .slide_y(px(4.), px(0.))
                        .apply(
                            panel,
                            ElementId::NamedInteger("tab-panel-enter".into(), epoch),
                        )
                        .into_any_element(),
                )
            } else {
                root.child(panel)
            };
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PanelMount::*;

    #[test]
    fn only_the_selected_panel_is_built() {
        assert_eq!(
            panel_mounts(Some(1), &[true, true, true], false, 3),
            vec![Skipped, Shown, Skipped]
        );
    }

    #[test]
    fn keep_mounted_holds_visited_panels_out_of_sight() {
        assert_eq!(
            panel_mounts(Some(2), &[true, false, true], true, 3),
            vec![Hidden, Skipped, Shown]
        );
    }

    #[test]
    fn nothing_selected_builds_nothing_new() {
        assert_eq!(panel_mounts(None, &[], true, 2), vec![Skipped, Skipped]);
        assert_eq!(panel_mounts(Some(5), &[], false, 2), vec![Skipped, Skipped]);
    }

    #[test]
    fn memory_marks_visits_and_counts_switches() {
        let mut memory = PanelMemory::default();
        assert!(
            !memory.select(Some(0), 3),
            "the first frame is not a switch"
        );
        assert!(!memory.select(Some(0), 3));
        assert_eq!(memory.epoch, 0);
        assert!(memory.select(Some(2), 3));
        assert_eq!(memory.epoch, 1);
        assert_eq!(memory.visited, vec![true, false, true]);
        // An out-of-range index is remembered but marks nothing.
        assert!(memory.select(Some(7), 3));
        assert_eq!(memory.visited, vec![true, false, true]);
    }
}
