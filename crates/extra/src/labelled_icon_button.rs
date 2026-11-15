//! Labelled icon button: a square icon button that can't be built without a name, shows that name on hover, can stay pressed, and can carry a count.
//!
//! Icon-only buttons are where accessible names go missing, so this one
//! takes its name up front and uses it three ways: for screen readers, as
//! the hover tooltip, and (with a shortcut) in that tooltip too. A toggle
//! stays pressed and says so. A count sits on its corner, capped at 99.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, StyledExt as _, kbd::Kbd, tooltip::Tooltip,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, Keystroke, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _, px,
};

/// The words a screen reader hears for the button.
pub fn spoken_name(name: &str, pressed: Option<bool>, count: Option<u32>) -> String {
    let mut out = name.to_string();
    if let Some(count) = count.filter(|c| *c > 0) {
        out.push_str(&format!(", {count}"));
    }
    match pressed {
        Some(true) => out.push_str(", on"),
        Some(false) => out.push_str(", off"),
        None => {}
    }
    out
}

/// A square icon button with a name it can't do without.
#[derive(IntoElement)]
pub struct LabelledIconButton {
    id: ElementId,
    style: StyleRefinement,
    icon: IconName,
    name: SharedString,
    shortcut: Option<Keystroke>,
    pressed: Option<bool>,
    count: Option<u32>,
    size: f32,
    on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl LabelledIconButton {
    /// Create a button with its icon and name.
    pub fn new(id: impl Into<ElementId>, icon: IconName, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            icon,
            name: name.into(),
            shortcut: None,
            pressed: None,
            count: None,
            size: 30.,
            on_click: None,
        }
    }

    /// Name a keyboard shortcut for the tooltip, such as "cmd-k".
    pub fn shortcut(mut self, keys: &'static str) -> Self {
        self.shortcut = Keystroke::parse(keys).ok();
        self
    }

    /// Make it a toggle, pressed or not.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// Put a count on its corner.
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    /// Set its size in pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(20.);
        self
    }

    /// Run on click.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for LabelledIconButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for LabelledIconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let chip = theme.muted;
        let accent = theme.primary;
        let fill = theme.background;
        let on = self.pressed == Some(true);
        let name = self.name.clone();
        let shortcut = self.shortcut.clone();
        let on_click = self.on_click.clone();
        div()
            .id(self.id.clone())
            .relative()
            .flex_none()
            .size(px(self.size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(7.))
            .cursor_pointer()
            .text_color(if on { accent } else { quiet })
            .when(on, |this| this.bg(accent.opacity(0.12)))
            .hover(|s| s.bg(chip).text_color(ink))
            .aria_label(spoken_name(&self.name, self.pressed, self.count))
            .when_some(self.pressed, |this, pressed| this.aria_selected(pressed))
            .tooltip(move |window, cx| {
                Tooltip::new(name.clone())
                    .key_binding(shortcut.clone().map(Kbd::new))
                    .build(window, cx)
            })
            .on_click(move |_, window, cx| {
                if let Some(on_click) = &on_click {
                    on_click(window, cx);
                }
            })
            .child(Icon::new(self.icon).size(px(self.size * 0.5)))
            .children(self.count.filter(|c| *c > 0).map(|count| {
                div()
                    .absolute()
                    .top(px(-3.))
                    .right(px(-3.))
                    .min_w(px(16.))
                    .h(px(16.))
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_2()
                    .border_color(fill)
                    .bg(theme_danger(cx))
                    .text_color(fill)
                    .text_xs()
                    .font_semibold()
                    .child(if count > 99 { "99+".to_string() } else { count.to_string() })
            }))
            .refine_style(&self.style)
    }
}

fn theme_danger(cx: &App) -> isonomic_ui::Hsla {
    cx.theme().danger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spoken_name_carries_count_and_state() {
        assert_eq!(spoken_name("Inbox", None, Some(3)), "Inbox, 3");
        assert_eq!(spoken_name("Bold", Some(true), None), "Bold, on");
        assert_eq!(spoken_name("Mute", Some(false), Some(0)), "Mute, off");
    }
}
