//! Shortcut keys: a keyboard shortcut written the way each platform writes it, including sequences ("⌘K, then P") and alternatives ("or").
//!
//! A shortcut is written once in a neutral form, such as "mod-k p", where
//! "mod" means Command on a Mac and Ctrl elsewhere and a space separates
//! the steps of a sequence. Each step shows as key caps; a screen reader
//! hears it in words, such as "Command K, then P".

use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, px,
};

/// Which platform's way of writing keys to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPlatform {
    /// Symbols such as ⌘ and ⌥.
    Mac,
    /// Words such as Ctrl and Alt.
    Other,
}

impl KeyPlatform {
    /// The platform this build runs on.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Other
        }
    }
}

/// One key cap's label and its spoken word.
pub fn key_cap(key: &str, platform: KeyPlatform) -> (String, String) {
    let mac = platform == KeyPlatform::Mac;
    let (cap, word) = match key.to_lowercase().as_str() {
        "mod" | "cmd" | "cmd-or-ctrl" => {
            if mac {
                ("\u{2318}", "Command")
            } else {
                ("Ctrl", "Control")
            }
        }
        "ctrl" | "control" => {
            if mac {
                ("\u{2303}", "Control")
            } else {
                ("Ctrl", "Control")
            }
        }
        "alt" | "option" => {
            if mac {
                ("\u{2325}", "Option")
            } else {
                ("Alt", "Alt")
            }
        }
        "shift" => {
            if mac {
                ("\u{21e7}", "Shift")
            } else {
                ("Shift", "Shift")
            }
        }
        "enter" | "return" => ("\u{21b5}", "Enter"),
        "escape" | "esc" => ("Esc", "Escape"),
        "backspace" => ("\u{232b}", "Backspace"),
        "tab" => ("\u{21e5}", "Tab"),
        "up" => ("\u{2191}", "Up"),
        "down" => ("\u{2193}", "Down"),
        "left" => ("\u{2190}", "Left"),
        "right" => ("\u{2192}", "Right"),
        "space" => ("Space", "Space"),
        other => return (other.to_uppercase(), other.to_uppercase()),
    };
    (cap.to_string(), word.to_string())
}

/// The steps of a shortcut, each a list of keys.
pub fn parse_shortcut(shortcut: &str) -> Vec<Vec<String>> {
    shortcut
        .split_whitespace()
        .map(|step| {
            step.split('-')
                .filter(|k| !k.is_empty())
                .map(str::to_string)
                .collect()
        })
        .filter(|keys: &Vec<String>| !keys.is_empty())
        .collect()
}

/// A shortcut in words, such as "Command K, then P".
pub fn spoken_shortcut(shortcut: &str, platform: KeyPlatform) -> String {
    parse_shortcut(shortcut)
        .iter()
        .map(|step| {
            step.iter()
                .map(|k| key_cap(k, platform).1)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(", then ")
}

/// A shortcut, or several alternatives, as key caps.
#[derive(IntoElement)]
pub struct ShortcutKeys {
    id: ElementId,
    style: StyleRefinement,
    shortcuts: Vec<SharedString>,
    platform: KeyPlatform,
}

impl ShortcutKeys {
    /// Create one shortcut, such as "mod-shift-p" or "g i".
    pub fn new(id: impl Into<ElementId>, shortcut: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            shortcuts: vec![shortcut.into()],
            platform: KeyPlatform::current(),
        }
    }

    /// Add another way to do the same thing.
    pub fn or(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcuts.push(shortcut.into());
        self
    }

    /// Write keys for another platform.
    pub fn platform(mut self, platform: KeyPlatform) -> Self {
        self.platform = platform;
        self
    }
}

impl Styled for ShortcutKeys {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ShortcutKeys {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let chip = theme.muted;
        let platform = self.platform;
        let spoken = self
            .shortcuts
            .iter()
            .map(|s| spoken_shortcut(s, platform))
            .collect::<Vec<_>>()
            .join(", or ");
        let mut parts: Vec<isonomic_ui::AnyElement> = Vec::new();
        for (alt, shortcut) in self.shortcuts.iter().enumerate() {
            if alt > 0 {
                parts.push(
                    div()
                        .px_1()
                        .text_xs()
                        .text_color(quiet)
                        .child("or")
                        .into_any_element(),
                );
            }
            for (ix, step) in parse_shortcut(shortcut).iter().enumerate() {
                if ix > 0 {
                    parts.push(
                        div()
                            .px_0p5()
                            .text_xs()
                            .text_color(quiet)
                            .child("then")
                            .into_any_element(),
                    );
                }
                parts.push(
                    h_flex()
                        .gap_0p5()
                        .children(step.iter().map(|k| {
                            div()
                                .min_w(px(20.))
                                .h(px(20.))
                                .px_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .border_1()
                                .border_b_2()
                                .border_color(line)
                                .bg(chip.opacity(0.5))
                                .text_xs()
                                .font_medium()
                                .child(key_cap(k, platform).0)
                        }))
                        .into_any_element(),
                );
            }
        }
        h_flex()
            .id(self.id.clone())
            .flex_none()
            .gap_1()
            .role(Role::Label)
            .aria_label(spoken)
            .children(parts)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_read_per_platform() {
        assert_eq!(
            spoken_shortcut("mod-k p", KeyPlatform::Mac),
            "Command K, then P"
        );
        assert_eq!(
            spoken_shortcut("mod-shift-p", KeyPlatform::Other),
            "Control Shift P"
        );
        assert_eq!(key_cap("alt", KeyPlatform::Mac).0, "\u{2325}");
        assert_eq!(
            parse_shortcut("g  i"),
            [vec!["g".to_string()], vec!["i".to_string()]]
        );
    }
}
