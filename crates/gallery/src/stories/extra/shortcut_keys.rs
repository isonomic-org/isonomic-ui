//! Gallery story for the shortcut keys.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::shortcut_keys::{KeyPlatform, ShortcutKeys};

/// A bookkeeping app's shortcuts, written for a Mac and for other systems.
pub struct ShortcutKeysStory;

impl ShortcutKeysStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for ShortcutKeysStory {
    fn title() -> &'static str {
        "Shortcut keys"
    }

    fn description() -> &'static str {
        "A keyboard shortcut written the way each platform writes it, including sequences and alternatives."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ShortcutKeysStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let rows = [
            ("Open the command palette", "mod-k", None),
            ("New invoice", "mod-shift-i", None),
            ("Go to invoices", "g i", None),
            ("Approve the front request", "a", Some("enter")),
            ("Move a step down", "alt-down", None),
        ];
        let table = |platform: KeyPlatform, key: &str| {
            v_flex()
                .gap_2()
                .child(div().text_xs().font_medium().text_color(quiet).child(
                    if platform == KeyPlatform::Mac {
                        "On a Mac"
                    } else {
                        "On Windows and Linux"
                    },
                ))
                .children(rows.iter().enumerate().map(|(ix, (what, keys, alt))| {
                    let mut shortcut = ShortcutKeys::new(
                        isonomic_ui::ElementId::Name(format!("{key}-{ix}").into()),
                        *keys,
                    )
                    .platform(platform);
                    if let Some(alt) = alt {
                        shortcut = shortcut.or(*alt);
                    }
                    h_flex()
                        .gap_3()
                        .child(div().w(px(200.)).text_sm().child(*what))
                        .child(shortcut)
                }))
        };
        h_flex()
            .items_start()
            .flex_wrap()
            .gap_8()
            .child(table(KeyPlatform::Mac, "mac"))
            .child(table(KeyPlatform::Other, "other"))
    }
}
