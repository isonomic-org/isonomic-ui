# Isonomic brand rules

The rules for the name, the typefaces and the icons. They hold for the brand
and for every application. The design language built on them is in
[design-language.md](design-language.md).

## The wordmark

- **"Isonomic" is set in IBM Plex Sans Regular.**
- **A product's name follows it after a space, set in IBM Plex Sans Light**, at the same size and in the same colour.

| Written | "Isonomic" | Product word |
|---|---|---|
| Isonomic | Regular | none |
| Isonomic Chat | Regular | "Chat" in Light |
| Isonomic Docs | Regular | "Docs" in Light |

- The wordmark is never set in the serif or the mono, and never in another weight.
- It does not change when a theme changes the interface font.
- In code, use the `Wordmark` component, which applies these rules:

```rust
use isonomic_ui_extra::wordmark::Wordmark;

Wordmark::new()                         // Isonomic
Wordmark::product("Chat")               // Isonomic Chat
Wordmark::product("Docs").size(px(24.)) // Isonomic Docs, 24 px
```

## Typefaces

Isonomic uses three families and no others.

| Family | Used for | Never for |
|---|---|---|
| IBM Plex Sans | The interface: controls, labels, headings, greetings, body text. Wordmarks. | |
| IBM Plex Mono | Code and tabular figures. | Wordmarks, interface text. |
| IBM Plex Serif | Long-form reading: an article in a reader's read mode. Rarely, chat bubbles. | Wordmarks, headings, greetings, interface text. |

In the library these are `assets::SANS`, `assets::MONO` and `assets::SERIF`,
and the theme's `font_family`, `mono_font_family` and `serif_font_family`.

Bundled weights:

- Sans: Light, Regular, Medium, SemiBold and Bold. Italics for all but Light.
- Mono: Regular, Medium, SemiBold and Bold, with Italic and Bold Italic.
- Serif: Regular and Medium, no italic.

## Icons

Isonomic uses IBM Carbon icons and no others. The one exception is the
Isonomic mark itself. Refer to icons by `IconName`; see the Icons section of
the design language.
