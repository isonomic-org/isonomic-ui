// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{
    Action, AsKeystroke, ElementId, FocusHandle, InteractiveElement as _, IntoElement, KeyBinding,
    KeyContext, Keystroke, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, relative,
};

use isonomic_ui_base::TestTextExt as _;

use crate::{ActiveTheme, StyledExt};

/// A tag for displaying keyboard keybindings.
#[derive(IntoElement, Clone, Debug)]
pub struct Kbd {
    style: StyleRefinement,
    stroke: Keystroke,
    appearance: bool,
    outline: bool,
    platform: Option<KeyPlatform>,
    text: Option<SharedString>,
}

impl From<Keystroke> for Kbd {
    fn from(stroke: Keystroke) -> Self {
        Self {
            style: StyleRefinement::default(),
            stroke,
            appearance: true,
            outline: false,
            platform: None,
            text: None,
        }
    }
}

impl Kbd {
    /// Create a new Kbd element with the given [`Keystroke`].
    pub fn new(stroke: Keystroke) -> Self {
        Self {
            style: StyleRefinement::default(),
            stroke,
            appearance: true,
            outline: false,
            platform: None,
            text: None,
        }
    }

    /// A key cap holding `text` as it is, such as "⌘" or "Ctrl", for a key
    /// written outside a [`Keystroke`]. `spoken` is what a screen reader says.
    pub fn text(text: impl Into<SharedString>, spoken: impl Into<SharedString>) -> Self {
        let spoken = spoken.into();
        Self {
            style: StyleRefinement::default(),
            stroke: Keystroke {
                key: spoken.to_string(),
                ..Keystroke::default()
            },
            appearance: true,
            outline: false,
            platform: None,
            text: Some(text.into()),
        }
    }

    /// Write the keys the way `platform` does, rather than the platform this
    /// build runs on. A settings page can show a shortcut for each platform.
    pub fn platform(mut self, platform: KeyPlatform) -> Self {
        self.platform = Some(platform);
        self
    }

    /// What this cap shows.
    fn label(&self) -> String {
        match &self.text {
            Some(text) => text.to_string(),
            None => Self::format_for(&self.stroke, self.platform.unwrap_or(KeyPlatform::legacy())),
        }
    }

    /// What a screen reader says for this cap.
    fn spoken(&self) -> String {
        match (&self.text, self.platform) {
            (Some(_), _) => self.stroke.key.clone(),
            (None, Some(platform)) => spoken_keystroke_for(&self.stroke, platform),
            (None, None) => spoken_keystroke(&self.stroke, cfg!(target_os = "macos")),
        }
    }

    /// The keystroke this tag shows.
    pub(crate) fn keystroke(&self) -> &Keystroke {
        &self.stroke
    }

    /// Set the appearance of the keybinding, default is `true`.
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    /// Use outline style for the keybinding, default is `false`.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// Return the first keybinding for the given action and context.
    pub fn binding_for_action(
        action: &dyn Action,
        context: Option<&str>,
        window: &Window,
    ) -> Option<Self> {
        let key_context = context.and_then(|context| KeyContext::parse(context).ok());
        let binding = match key_context {
            Some(context) => {
                window.highest_precedence_binding_for_action_in_context(action, context)
            }
            None => window.highest_precedence_binding_for_action(action),
        }?;

        Self::from_binding(&binding)
    }

    /// Return the first keybinding for the given action and focus handle.
    ///
    /// GPUI resolves the handle in the previously rendered frame, so this
    /// finds nothing for a handle whose element is drawn for the first time
    /// in the current frame.
    pub fn binding_for_action_in(
        action: &dyn Action,
        focus_handle: &FocusHandle,
        window: &Window,
    ) -> Option<Self> {
        let binding = window.highest_precedence_binding_for_action_in(action, focus_handle)?;
        Self::from_binding(&binding)
    }

    /// Return the first keybinding for the given action that was registered
    /// without a key context, so it applies wherever focus is.
    pub fn global_binding_for_action(action: &dyn Action, window: &Window) -> Option<Self> {
        let binding = window
            .highest_precedence_binding_for_action_in_context(action, KeyContext::default())?;
        Self::from_binding(&binding)
    }

    fn from_binding(binding: &KeyBinding) -> Option<Self> {
        let key = binding.keystrokes().first()?;
        Some(Self::new(key.as_keystroke().clone()))
    }

    /// Return the Platform specific keybinding string by KeyStroke
    ///
    /// macOS writes modifiers as symbols run together (⌃⌥⇧⌘); other platforms
    /// write words joined by "+" (Ctrl+Alt+Shift+Win).
    pub fn format(key: &Keystroke) -> String {
        Self::format_for(key, KeyPlatform::legacy())
    }

    /// The keystroke written the way `platform` writes it, whatever this build
    /// runs on: "⌘⇧P" for [`KeyPlatform::Mac`], "Ctrl+Shift+P" elsewhere, with
    /// the platform key as "Win" on Windows and "Super" on Linux.
    pub fn format_for(key: &Keystroke, platform: KeyPlatform) -> String {
        let mac = platform == KeyPlatform::Mac;
        let separator = if mac { "" } else { "+" };
        let platform_key = match platform {
            KeyPlatform::Mac => "⌘",
            KeyPlatform::Windows => "Win",
            KeyPlatform::Linux => "Super",
        };
        let pick = |mac_text: &'static str, other: &'static str| if mac { mac_text } else { other };

        // The key map order in macOS is: ⌃⌥⇧⌘
        // And in Windows is: Ctrl+Alt+Shift+Win
        let mut parts: Vec<String> = vec![];
        if key.modifiers.control {
            parts.push(pick("⌃", "Ctrl").into());
        }
        if key.modifiers.alt {
            parts.push(pick("⌥", "Alt").into());
        }
        if key.modifiers.shift {
            parts.push(pick("⇧", "Shift").into());
        }
        if key.modifiers.platform {
            parts.push(platform_key.into());
        }

        let key_str = key.key.as_str();
        let keys: String = match key_str {
            "ctrl" => pick("⌃", "Ctrl").into(),
            "alt" => pick("⌥", "Alt").into(),
            "shift" => pick("⇧", "Shift").into(),
            "cmd" => platform_key.into(),
            "space" => "Space".into(),
            "backspace" => pick("⌫", "Backspace").into(),
            "delete" => pick("⌫", "Delete").into(),
            "escape" => pick("⎋", "Esc").into(),
            "enter" => pick("⏎", "Enter").into(),
            "pagedown" => "Page Down".into(),
            "pageup" => "Page Up".into(),
            "left" => pick("←", "Left").into(),
            "right" => pick("→", "Right").into(),
            "up" => pick("↑", "Up").into(),
            "down" => pick("↓", "Down").into(),
            _ => {
                if key_str.len() == 1 {
                    key_str.to_uppercase()
                } else {
                    let mut chars = key_str.chars();
                    match chars.next() {
                        Some(first_char) => {
                            format!("{}{}", first_char.to_uppercase(), chars.collect::<String>())
                        }
                        None => key_str.to_string(),
                    }
                }
            }
        };

        parts.push(keys);
        parts.join(separator)
    }
}

/// Which platform's way of writing keys a [`Kbd`] follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPlatform {
    /// Symbols run together: ⌃⌥⇧⌘.
    Mac,
    /// Words joined by "+", the platform key written "Win".
    Windows,
    /// Words joined by "+", the platform key written "Super".
    Linux,
}

impl KeyPlatform {
    /// The platform this build runs on.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    /// What [`Kbd::format`] has always written: the Mac's symbols on a Mac,
    /// the Windows words everywhere else.
    fn legacy() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Windows
        }
    }

    /// The word a screen reader says for the platform key.
    fn platform_key_word(self) -> &'static str {
        match self {
            Self::Mac => "Command",
            Self::Windows => "Windows",
            Self::Linux => "Super",
        }
    }
}

/// A keystroke in words for screen readers, such as "Command Shift P".
/// The drawn symbols, such as ⌘ and ⇧, read poorly or not at all.
pub fn spoken_keystroke(stroke: &Keystroke, mac: bool) -> String {
    let m = &stroke.modifiers;
    let mut words: Vec<String> = Vec::new();
    if m.function {
        words.push("Function".into());
    }
    if m.control {
        words.push("Control".into());
    }
    if m.alt {
        words.push(if mac { "Option" } else { "Alt" }.into());
    }
    if m.shift {
        words.push("Shift".into());
    }
    if m.platform {
        words.push(if mac { "Command" } else { "Super" }.into());
    }
    words.push(spoken_key(&stroke.key));
    words.join(" ")
}

/// A keystroke in words the way `platform` names its keys: "Option" and
/// "Command" on a Mac, "Alt" and "Windows" or "Super" elsewhere.
pub fn spoken_keystroke_for(stroke: &Keystroke, platform: KeyPlatform) -> String {
    let m = &stroke.modifiers;
    let mut words: Vec<String> = Vec::new();
    if m.function {
        words.push("Function".into());
    }
    if m.control {
        words.push("Control".into());
    }
    if m.alt {
        words.push(
            if platform == KeyPlatform::Mac {
                "Option"
            } else {
                "Alt"
            }
            .into(),
        );
    }
    if m.shift {
        words.push("Shift".into());
    }
    if m.platform {
        words.push(platform.platform_key_word().into());
    }
    words.push(spoken_key(&stroke.key));
    words.join(" ")
}

/// A key's own name in words.
fn spoken_key(key: &str) -> String {
    match key {
        "enter" => "Enter".to_string(),
        "escape" => "Escape".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "tab" => "Tab".to_string(),
        "space" => "Space".to_string(),
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        "pageup" => "Page Up".to_string(),
        "pagedown" => "Page Down".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "-" => "Minus".to_string(),
        "=" => "Equals".to_string(),
        "," => "Comma".to_string(),
        "." => "Period".to_string(),
        "/" => "Slash".to_string(),
        other => other.to_uppercase(),
    }
}

impl Styled for Kbd {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Kbd {
    fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let label = self.label();
        if !self.appearance {
            return label.into_any_element();
        }

        let spoken = self.spoken();
        div()
            // Lets a test ask whether a given shortcut hint was painted this
            // frame; a no-op outside test-support builds.
            .debug_selector(|| format!("kbd:{}", self.stroke.unparse(cx.platform_info().os)))
            // Named by the keystroke, so the same shortcut shown twice side by
            // side is the only clash.
            .id(ElementId::Name(SharedString::from(format!("kbd-{spoken}"))))
            // A `Label` node is named by its value.
            .role(Role::Label)
            .aria_value(spoken)
            // A key cap is a faint pressed-in chip: a wash with a hairline
            // inner edge, medium 12px ink at the muted level.
            .text_color(crate::look::ink::muted(cx))
            .map(|this| {
                let dark = cx.theme().mode.is_dark();
                let wash = if dark {
                    gpui::hsla(0., 0., 1., 0.08)
                } else {
                    gpui::hsla(0., 0., 0., 0.043)
                };
                let edge = if dark {
                    gpui::hsla(0., 0., 1., 0.06)
                } else {
                    gpui::hsla(0., 0., 0., 0.037)
                };
                if self.outline {
                    this.bg(cx.theme().tokens.background)
                        .shadow(vec![gpui::BoxShadow {
                            color: crate::look::ink::hairline(cx).alpha(0.12),
                            offset: gpui::point(gpui::px(0.), gpui::px(0.)),
                            blur_radius: gpui::px(0.),
                            spread_radius: gpui::px(1.),
                            inset: true,
                        }])
                } else {
                    this.bg(wash).shadow(vec![gpui::BoxShadow {
                        color: edge,
                        offset: gpui::point(gpui::px(0.), gpui::px(0.)),
                        blur_radius: gpui::px(0.),
                        spread_radius: gpui::px(0.5),
                        inset: true,
                    }])
                }
            })
            .flex()
            .items_center()
            .justify_center()
            .h(gpui::px(20.))
            .px(gpui::px(5.))
            .min_w(gpui::px(20.))
            .text_center()
            .rounded(if cx.theme().radius > gpui::px(0.) {
                gpui::px(5.)
            } else {
                gpui::px(0.)
            })
            .line_height(relative(1.))
            .text_size(gpui::px(12.))
            .font_medium()
            .whitespace_normal()
            .flex_shrink_0()
            .refine_style(&self.style)
            .child(label.clone())
            .test_text(label)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Kbd, KeyPlatform, spoken_keystroke_for};
    use gpui::Keystroke;

    fn key(text: &'static str) -> Keystroke {
        Keystroke::parse(text).unwrap()
    }

    #[test]
    fn every_platform_writes_its_own_keys_on_any_build() {
        assert_eq!(
            Kbd::format_for(&key("cmd-shift-p"), KeyPlatform::Mac),
            "⇧⌘P"
        );
        assert_eq!(
            Kbd::format_for(&key("cmd-shift-p"), KeyPlatform::Windows),
            "Shift+Win+P"
        );
        assert_eq!(
            Kbd::format_for(&key("cmd-shift-p"), KeyPlatform::Linux),
            "Shift+Super+P"
        );
        assert_eq!(Kbd::format_for(&key("ctrl-alt-a"), KeyPlatform::Mac), "⌃⌥A");
        assert_eq!(
            Kbd::format_for(&key("ctrl-alt-a"), KeyPlatform::Linux),
            "Ctrl+Alt+A"
        );
        assert_eq!(Kbd::format_for(&key("escape"), KeyPlatform::Mac), "⎋");
        assert_eq!(Kbd::format_for(&key("escape"), KeyPlatform::Windows), "Esc");
        assert_eq!(Kbd::format_for(&key("f12"), KeyPlatform::Mac), "F12");
    }

    #[test]
    fn spoken_keys_follow_the_platform() {
        assert_eq!(
            spoken_keystroke_for(&key("cmd-alt-k"), KeyPlatform::Mac),
            "Option Command K"
        );
        assert_eq!(
            spoken_keystroke_for(&key("cmd-alt-k"), KeyPlatform::Windows),
            "Alt Windows K"
        );
        assert_eq!(
            spoken_keystroke_for(&key("ctrl-pagedown"), KeyPlatform::Linux),
            "Control Page Down"
        );
    }

    #[test]
    fn a_text_cap_shows_and_says_its_own_words() {
        let cap = Kbd::text("⌘", "Command");
        assert_eq!(cap.label(), "⌘");
        assert_eq!(cap.spoken(), "Command");
        let cap = Kbd::new(key("cmd-k")).platform(KeyPlatform::Windows);
        assert_eq!(cap.label(), "Win+K");
        assert_eq!(cap.spoken(), "Windows K");
    }

    #[test]
    fn test_format() {
        if cfg!(target_os = "macos") {
            assert_eq!(Kbd::format(&Keystroke::parse("cmd-a").unwrap()), "⌘A");
            assert_eq!(
                super::spoken_keystroke(&Keystroke::parse("cmd-shift-p").unwrap(), true),
                "Shift Command P"
            );
            assert_eq!(
                super::spoken_keystroke(&Keystroke::parse("ctrl-alt-enter").unwrap(), false),
                "Control Alt Enter"
            );
            assert_eq!(Kbd::format(&Keystroke::parse("cmd--").unwrap()), "⌘-");
            assert_eq!(Kbd::format(&Keystroke::parse("cmd-+").unwrap()), "⌘+");
            assert_eq!(Kbd::format(&Keystroke::parse("cmd-enter").unwrap()), "⌘⏎");
            assert_eq!(Kbd::format(&Keystroke::parse("cmd-f12").unwrap()), "⌘F12");
            assert_eq!(
                Kbd::format(&Keystroke::parse("shift-pagedown").unwrap()),
                "⇧Page Down"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("shift-pageup").unwrap()),
                "⇧Page Up"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("shift-space").unwrap()),
                "⇧Space"
            );
            assert_eq!(Kbd::format(&Keystroke::parse("cmd-ctrl-a").unwrap()), "⌃⌘A");
            assert_eq!(
                Kbd::format(&Keystroke::parse("cmd-alt-backspace").unwrap()),
                "⌥⌘⌫"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("shift-delete").unwrap()),
                "⇧⌫"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("cmd-ctrl-shift-a").unwrap()),
                "⌃⇧⌘A"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("cmd-ctrl-shift-alt-a").unwrap()),
                "⌃⌥⇧⌘A"
            );
        } else {
            assert_eq!(Kbd::format(&Keystroke::parse("a").unwrap()), "A");
            assert_eq!(Kbd::format(&Keystroke::parse("ctrl-a").unwrap()), "Ctrl+A");
            assert_eq!(
                Kbd::format(&Keystroke::parse("shift-space").unwrap()),
                "Shift+Space"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("ctrl-alt-a").unwrap()),
                "Ctrl+Alt+A"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("ctrl-alt-shift-a").unwrap()),
                "Ctrl+Alt+Shift+A"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("ctrl-alt-shift-win-a").unwrap()),
                "Ctrl+Alt+Shift+Win+A"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("ctrl-shift-backspace").unwrap()),
                "Ctrl+Shift+Backspace"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("alt-delete").unwrap()),
                "Alt+Delete"
            );
            assert_eq!(
                Kbd::format(&Keystroke::parse("alt-tab").unwrap()),
                "Alt+Tab"
            );
        }
    }
}
