// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shortcut keys: a keyboard shortcut written the way each platform writes it, including sequences ("⌘K, then P") and alternatives ("or").
//!
//! A shortcut is written once in a neutral form, such as "mod-k p", where
//! "mod" means Command on a Mac and Ctrl elsewhere and a space separates
//! the steps of a sequence. Each step shows as key caps; a screen reader
//! hears it in words, such as "Command K, then P".
//!
//! Caps are the base key cap. A joined hint puts each step in one cap, the
//! way a menu writes it: "⌘⇧P" on a Mac, "Ctrl+Shift+P" elsewhere. The
//! platform defaults to the one the build runs on and can be set, so a
//! settings page can show each platform's keys side by side.

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, kbd::Kbd};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
};

/// Which platform's way of writing keys to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPlatform {
    /// Symbols such as ⌘ and ⌥.
    Mac,
    /// Words such as Ctrl and Alt.
    Other,
    /// Words, with the system key written Win.
    Windows,
    /// Words, with the system key written Super.
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

    /// What joins the keys of one step in a joined hint: nothing on a Mac,
    /// "+" elsewhere.
    pub fn joiner(self) -> &'static str {
        if self == Self::Mac { "" } else { "+" }
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
        "super" | "win" | "meta" => match platform {
            KeyPlatform::Mac => ("\u{2318}", "Command"),
            KeyPlatform::Linux => ("Super", "Super"),
            KeyPlatform::Windows | KeyPlatform::Other => ("Win", "Windows"),
        },
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

/// One step's keys as a single joined label, such as "⌘⇧P" or
/// "Ctrl+Shift+P".
pub fn joined_step(step: &[String], platform: KeyPlatform) -> String {
    step.iter()
        .map(|k| key_cap(k, platform).0)
        .collect::<Vec<_>>()
        .join(platform.joiner())
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
    joined: bool,
}

impl ShortcutKeys {
    /// Create one shortcut, such as "mod-shift-p" or "g i".
    pub fn new(id: impl Into<ElementId>, shortcut: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            shortcuts: vec![shortcut.into()],
            platform: KeyPlatform::current(),
            joined: false,
        }
    }

    /// Put each step's keys in one cap, the way a menu writes a shortcut:
    /// "⌘⇧P" on a Mac, "Ctrl+Shift+P" elsewhere. Off by default: one cap
    /// per key.
    pub fn joined(mut self, joined: bool) -> Self {
        self.joined = joined;
        self
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
        let quiet = cx.theme().muted_foreground;
        let platform = self.platform;
        let joined = self.joined;
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
                        .text_caption()
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
                            .text_caption()
                            .text_color(quiet)
                            .child("then")
                            .into_any_element(),
                    );
                }
                let spoken_step = step
                    .iter()
                    .map(|k| key_cap(k, platform).1)
                    .collect::<Vec<_>>()
                    .join(" ");
                let caps: Vec<Kbd> = if joined {
                    vec![Kbd::text(joined_step(step, platform), spoken_step)]
                } else {
                    step.iter()
                        .map(|k| {
                            let (cap, word) = key_cap(k, platform);
                            Kbd::text(cap, word)
                        })
                        .collect()
                };
                // Each step is its own id scope, so a key used in two steps
                // ("mod-k mod-p") names two distinct caps.
                parts.push(
                    h_flex()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("step-{alt}-{ix}").into(),
                        ))
                        .gap_0p5()
                        .children(caps)
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
    fn joined_steps_follow_the_platform() {
        let step = parse_shortcut("mod-shift-p").remove(0);
        assert_eq!(joined_step(&step, KeyPlatform::Mac), "\u{2318}\u{21e7}P");
        assert_eq!(joined_step(&step, KeyPlatform::Windows), "Ctrl+Shift+P");
        assert_eq!(joined_step(&step, KeyPlatform::Other), "Ctrl+Shift+P");
    }

    #[test]
    fn the_system_key_is_named_per_platform() {
        assert_eq!(key_cap("super", KeyPlatform::Mac).0, "\u{2318}");
        assert_eq!(
            key_cap("super", KeyPlatform::Windows),
            ("Win".into(), "Windows".into())
        );
        assert_eq!(
            key_cap("win", KeyPlatform::Linux),
            ("Super".into(), "Super".into())
        );
        assert_eq!(
            spoken_shortcut("super-l", KeyPlatform::Windows),
            "Windows L"
        );
    }

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
