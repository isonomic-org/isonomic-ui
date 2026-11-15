//! Split view: two panes side by side with a handle between them that you drag, nudge with the arrow keys, or double-click to put back.
//!
//! Each pane keeps a smallest width, so neither can be squeezed away; a
//! pane can instead be folded shut and opened again. The split is a share
//! of the width, so it holds as the window resizes, and the view reports it
//! when a drag ends so the owner can remember it.

use isonomic_ui::component::{ActiveTheme as _, h_flex};
use isonomic_ui::{
    AnyView, App, Bounds, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _,
    Pixels, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    canvas, div, prelude::FluentBuilder as _, px,
};

/// Keep a split share so each pane keeps its smallest width; when both can't fit, split by their smallest widths.
pub fn clamp_split(share: f32, total: f32, min_start: f32, min_end: f32) -> f32 {
    if total <= 0.0 {
        return share.clamp(0.0, 1.0);
    }
    if min_start + min_end >= total {
        return (min_start / (min_start + min_end)).clamp(0.0, 1.0);
    }
    share.clamp(min_start / total, 1.0 - min_end / total)
}

/// What the view reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitEvent {
    /// The split settled at this share of the width.
    Resized(f32),
    /// The first pane was folded shut or opened.
    Folded(bool),
}

/// Two panes with a handle between.
pub struct SplitView {
    start: AnyView,
    end: AnyView,
    share: f32,
    initial: f32,
    min_start: f32,
    min_end: f32,
    folded: bool,
    dragging: bool,
    bounds: Bounds<Pixels>,
    label: SharedString,
    focus: FocusHandle,
}

impl EventEmitter<SplitEvent> for SplitView {}

impl Focusable for SplitView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SplitView {
    /// Create a split with the first pane taking `share` of the width.
    pub fn new(
        start: impl Into<AnyView>,
        end: impl Into<AnyView>,
        share: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            share: share.clamp(0.05, 0.95),
            initial: share.clamp(0.05, 0.95),
            min_start: 160.0,
            min_end: 200.0,
            folded: false,
            dragging: false,
            bounds: Bounds::default(),
            label: "Resize panes".into(),
            focus: cx.focus_handle(),
        }
    }

    /// Keep each pane at least this wide, in pixels.
    pub fn min_widths(mut self, start: f32, end: f32) -> Self {
        self.min_start = start.max(0.0);
        self.min_end = end.max(0.0);
        self
    }

    /// Name the handle for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// The first pane's share of the width.
    pub fn share(&self) -> f32 {
        self.share
    }

    /// Fold the first pane shut or open it.
    pub fn set_folded(&mut self, folded: bool, cx: &mut Context<Self>) {
        if self.folded != folded {
            self.folded = folded;
            cx.emit(SplitEvent::Folded(folded));
            cx.notify();
        }
    }

    fn total(&self) -> f32 {
        f32::from(self.bounds.size.width)
    }

    fn set_share(&mut self, share: f32, cx: &mut Context<Self>) {
        self.share = clamp_split(share, self.total(), self.min_start, self.min_end);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let step = if event.keystroke.modifiers.shift {
            0.1
        } else {
            0.02
        };
        match event.keystroke.key.as_str() {
            "left" => self.set_share(self.share - step, cx),
            "right" => self.set_share(self.share + step, cx),
            "home" => self.set_share(0.0, cx),
            "end" => self.set_share(1.0, cx),
            "enter" | "space" => self.set_folded(!self.folded, cx),
            _ => return,
        }
        if !matches!(event.keystroke.key.as_str(), "enter" | "space") {
            cx.emit(SplitEvent::Resized(self.share));
        }
        cx.stop_propagation();
    }
}

impl Render for SplitView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let line = theme.border;
        let ring = theme.ring;
        let active = self.dragging || self.focus.is_focused(window);
        let measure = cx.entity();
        let share = clamp_split(self.share, self.total(), self.min_start, self.min_end);
        let percent = (share * 100.0).round();

        h_flex()
            .id("split-view")
            .relative()
            .size_full()
            .when(self.dragging, |this| this.cursor_col_resize())
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    let x = f32::from(event.position.x - this.bounds.left());
                    this.set_share(x / this.total().max(1.0), cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.dragging {
                        this.dragging = false;
                        cx.emit(SplitEvent::Resized(this.share));
                        cx.notify();
                    }
                }),
            )
            .child(
                canvas(
                    move |bounds, _, cx| measure.update(cx, |this, _| this.bounds = bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when(!self.folded, |this| {
                this.child(
                    div()
                        .h_full()
                        .min_w_0()
                        .overflow_hidden()
                        .w(isonomic_ui::relative(share))
                        .child(self.start.clone()),
                )
            })
            .child(
                div()
                    .id("split-handle")
                    .track_focus(&self.focus)
                    .flex_none()
                    .h_full()
                    .w(px(9.))
                    .flex()
                    .justify_center()
                    .cursor_col_resize()
                    .role(Role::Splitter)
                    .aria_numeric_value(f64::from(percent))
                    .aria_min_numeric_value(0.0)
                    .aria_max_numeric_value(100.0)
                    .aria_label(format!(
                        "{}, first pane {}",
                        self.label,
                        if self.folded {
                            "folded".to_string()
                        } else {
                            format!("{percent}%")
                        }
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus, cx);
                            if event.click_count >= 2 {
                                this.folded = false;
                                let initial = this.initial;
                                this.set_share(initial, cx);
                                cx.emit(SplitEvent::Resized(this.share));
                            } else if !this.folded {
                                this.dragging = true;
                                cx.notify();
                            }
                        }),
                    )
                    .on_key_down(cx.listener(Self::key_down))
                    .child(
                        div()
                            .w(px(if active { 2. } else { 1. }))
                            .h_full()
                            .bg(if active { ring } else { line }),
                    ),
            )
            .child(
                div()
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.end.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_keep_both_panes() {
        assert_eq!(clamp_split(0.5, 1000.0, 160.0, 200.0), 0.5);
        assert_eq!(clamp_split(0.05, 1000.0, 160.0, 200.0), 0.16);
        assert_eq!(clamp_split(0.95, 1000.0, 160.0, 200.0), 0.8);
        assert_eq!(clamp_split(0.5, 300.0, 200.0, 200.0), 0.5);
        assert_eq!(clamp_split(0.7, 0.0, 160.0, 200.0), 0.7);
    }
}
