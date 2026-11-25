# Isonomic UI design language

The rules every Isonomic UI component follows. Components read these values
from the theme (`cx.theme()`); nothing hard-codes a colour, radius or duration.

## Status

| Area | State |
|---|---|
| Colour, light and dark | In the default theme |
| Radius | In the default theme |
| Typeface and icons | Bundled and on by default |
| Type scale and control density | Applied: text roles in `look::TextRole`, a 13/20 default line in the window root, button and field sizes |
| Elevation, motion, focus | Raised and field surfaces and the hover fade live in `look`; components are being moved onto them (issue #396) |

## Principles

1. Quiet surfaces. Near-white or near-black backgrounds, hairline borders,
   one strong colour: the ink itself.
2. Translucent neutrals. Borders, fills and hovers are black or white at low
   alpha, so they sit correctly on any surface.
3. Colour means something. Hue is reserved for status, selection and data.
4. Dense but calm. Small controls on a 2px grid, with generous line height.
5. Motion confirms, it does not perform. Short, eased, and off under reduced
   motion.

## Colour

### Neutrals

| Token | Light | Dark |
|---|---|---|
| Background | `#ffffff` | `#121213` |
| Foreground (ink) | `#1f1f1f` | `#e8e8e8` |
| Muted foreground | `#707070` | `#8a8a8b` |
| Border | black 8% | white 14% |
| Input border | black 13% | white 20% |
| Muted fill | black 3% | white 4% |
| Hover fill | black 5% | white 7% |
| Secondary fill | black 4% (6% hover, 8% pressed) | white 6% (9% hover, 12% pressed) |
| Popover | `#ffffff` | `#1f1f22` |
| Raised panel | black 3% | `#1c1c1e` |
| Sidebar, title bar, status bar | `#fafafa` | `#0c0c0d` |

### Primary

The primary action is ink on the surface, not a brand hue.

| Token | Light | Dark |
|---|---|---|
| Primary | `#1f1f1f` | `#e8e8e8` |
| Primary hover | `#333333` | `#ffffff` |
| Primary pressed | `#141414` | `#d4d4d4` |
| On primary | `#ffffff` | `#161618` |

### Status

| Token | Light | Dark |
|---|---|---|
| Danger | `#c4320a` | `#ff8a7a` |
| Warning | `#b25e00` | `#ffb35c` |
| Success | `#1b8049` | `#7dcf9f` |
| Info | `#0070d6` | `#4fb0fe` |
| Text on status | `#ffffff` | `#161618` |

### Selection and focus

| Token | Light | Dark |
|---|---|---|
| Focus ring | `#4d90eb` | `#4174d0` |
| Selected row fill | `#0190fe` at 10% | `#4fb0fe` at 10% |
| Selected row border | `#0190fe` | `#4fb0fe` |

### Data palette

Used in order for chart series.

| # | Light | Dark |
|---|---|---|
| 1 | `#0190fe` | `#4fb0fe` |
| 2 | `#5fc88a` | `#7dcf9f` |
| 3 | `#e8b000` | `#e6c24d` |
| 4 | `#f28a2e` | `#f2a15c` |
| 5 | `#a651f0` | `#b98af0` |

## Shape

| Token | Value | Used for |
|---|---|---|
| Radius | 6px | Controls: buttons, inputs, tags, menu items |
| Radius, large | 10px | Containers: cards, popovers, dialogs |

## Typography

- The typefaces are IBM Plex Sans, IBM Plex Mono and IBM Plex Serif, and the icons are IBM Carbon; Isonomic uses no others. See [brand.md](brand.md) for what each is for and for the wordmark.
- Interface text uses the bundled sans family (`assets::SANS`); code and
  tabular figures use the bundled mono family (`assets::MONO`).
- Long-form reading, such as an article in a read mode, uses the bundled
  serif family (`assets::SERIF`, the theme's `serif_font_family`), in
  regular or medium. Wordmarks, headings, greetings and interface text
  never do.
- Text roles (`look::TextRole`), each a size, a line height and a weight:

| Role | Size / line | Weight | Used for |
|---|---|---|---|
| `text_body` | 13 / 20 | 400 | Running text; the window's default |
| `text_label` | 13 / 20 | 500 | Control labels, row and card titles |
| `text_caption` | 12 / 16 | 400 | Hints, meta, secondary lines |
| `text_label_small` | 12 / 16 | 500 | Small controls |
| `text_badge` | 11 / 16 | 500 | Counts and status pills |
| `text_eyebrow` | 10 / 16 | 500 | Small section headings, in capitals, spaced |
| `text_title` | 15 / 22 | 500 | Panel and dialog titles |

- Headings step up in size, not weight: 15, 18, 24px at weight 600.
- Ink levels (`look::ink`) are the foreground at fixed strengths: strong
  (titles, values), muted (quiet controls at rest, about 61% black), icon,
  subtle (hints, the muted-foreground token) and faint (shortcuts, counters).
- Every text colour meets 4.5:1 against the surface it sits on, in both
  themes. Muted text is the lightest grey that does.

## Spacing and density

- Everything sits on a 2px grid. The steps are 2, 4, 6, 8, 12, 16 and 24px.
- Button heights: 24px small, 28px medium (the default), 32px large. Text
  fields are 32px at medium and 36px at large.
- Horizontal padding in a button: 10px (8px beside a leading icon), 8px small
  (6px beside an icon), 12px large (10px beside an icon). The gap between an
  icon and its label is 6px, 4px in a small button. Icons are 16px, 14px in a
  small button. A text field pads 10px each side.

## Elevation

Edges come from a hairline ring, depth from a soft shadow. Borders are not
drawn on raised surfaces.

| Level | Used for | Shadow |
|---|---|---|
| Card (`look::surface::resting`) | Raised controls, cards, the default button | 0.5px ring (black 6%, white 10% in dark), 0 1px 1px -1px at 10% and 0 1px 2px at 5%; the ring firms to 10% (16%) under the pointer |
| Field (`look::surface::field`) | Text fields | 0.5px ring at 8% (14%), firming to 13% (20%) on hover and a full 1px at 28% (30%) with the caret in it; no blue halo |
| Popover (`look::surface::floating`) | Menus, popovers, tooltips | 0.5px ring at 10%, plus 0 12px 32px -8px at 16% |

GPUI's shadow blur is the gaussian's deviation, half of what CSS calls the
blur radius, so `look` halves every CSS blur it stands for.
| Modal | Dialogs, sheets | Popover shadow over a dimmed page |

Stacking order, top first: modal, overlays, toasts, page.

## Motion

| Token | Value |
|---|---|
| Ease | `cubic-bezier(0.22, 1, 0.36, 1)` |
| Spring (pop only) | `cubic-bezier(0.34, 1.56, 0.64, 1)` |
| Durations | 140 (fast), 200 (normal), 320ms (slow) in the theme's motion tokens |
| Long duration | 600ms |

- Hover: fills, ink and rings fade over 140ms on the plain ease curve
  (`look::hover_fade`); they never snap.
- Overlays rise 4px and fade in over 160-220ms.
- A pop scales from 0.86 to 1 with the spring curve.
- Rolling digits use the long duration with the ease curve, each digit
  starting 24ms after the one to its right. Nothing else is this slow.
- Under reduced motion the state still changes; the movement does not play.

## Focus

- A 2px ring in the focus colour, shown for keyboard focus only.
- The ring sits 2px outside a control, and 2px inside a row or cell.
- Text fields show no ring: their border steps up to the input-border colour.

## Cursor

- Anything a person can press shows the pointing hand while it can be pressed:
  buttons of every variant, links, checkboxes, switches, radios, toggles, tabs,
  menu items, select and date triggers, accordion headers, pagination, sidebar
  rows, list rows and the clickable parts of extras.
- A disabled or loading control shows the arrow.
- Text fields and selectable text keep the text cursor; elements that cannot be
  pressed set no cursor.
- Components apply this with `PressableExt::pressable(enabled)` from the base
  crate, before the caller's style, so one instance can still override it.
  New code uses it rather than `cursor_pointer()`, so disabled is handled too.

## States

- Disabled: 40% opacity, no hover, no pointer cursor (the arrow).
- Danger actions: a soft danger fill with danger-coloured text, solid only for
  the final confirmation.
- Ghost actions: no fill at rest, the hover fill on hover.

## Icons

- Drawn on a 32px grid and shown at 12, 14, 16 or 20px.
- Muted at rest, ink on hover or when selected.
- Refer to icons by `IconName`; never embed an SVG path in a component.
