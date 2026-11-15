# Isonomic UI design language

The rules every Isonomic UI component follows. Components read these values
from the theme (`cx.theme()`); nothing hard-codes a colour, radius or duration.

## Status

| Area | State |
|---|---|
| Colour, light and dark | In the default theme |
| Radius | In the default theme |
| Typeface and icons | Bundled and on by default |
| Type scale and control density | Specified below; not yet applied |
| Elevation, motion, focus | Specified below; components are being moved onto them |

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
| Muted foreground | `#8c8c8c` | `#808081` |
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
| Success | `#1e8a4f` | `#7dcf9f` |
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

- Interface text uses the bundled sans family (`assets::SANS`); code and
  tabular figures use the bundled mono family (`assets::MONO`).
- Target scale: 13px body at weight 400, 13px controls at weight 500, 20px
  line height; 12px for small controls and captions.
- Headings step up in size, not weight: 15, 18, 24px at weight 600.

## Spacing and density

- Everything sits on a 2px grid. The steps are 2, 4, 6, 8, 12, 16 and 24px.
- Control heights: 24px small, 28px medium (the default), 32px large.
- Horizontal padding in a control: 10px (8px beside a leading icon), 6px small,
  12px large. The gap between an icon and its label is 6px.

## Elevation

Edges come from a hairline ring, depth from a soft shadow. Borders are not
drawn on raised surfaces.

| Level | Used for | Shadow |
|---|---|---|
| Card | Raised controls, cards | 0.5px ring, plus 0 1px 2px at low alpha |
| Popover | Menus, popovers, tooltips | 0.5px ring, plus 0 12px 32px -8px |
| Modal | Dialogs, sheets | Popover shadow over a dimmed page |

Stacking order, top first: modal, overlays, toasts, page.

## Motion

| Token | Value |
|---|---|
| Ease | `cubic-bezier(0.22, 1, 0.36, 1)` |
| Spring (pop only) | `cubic-bezier(0.34, 1.56, 0.64, 1)` |
| Durations | 120, 160, 220, 320, 420ms |

- Hover and press: 120-160ms.
- Overlays rise 4px and fade in over 160-220ms.
- A pop scales from 0.86 to 1 with the spring curve.
- Under reduced motion the state still changes; the movement does not play.

## Focus

- A 2px ring in the focus colour, shown for keyboard focus only.
- The ring sits 2px outside a control, and 2px inside a row or cell.
- Text fields show no ring: their border steps up to the input-border colour.

## States

- Disabled: 40% opacity, no hover, no pointer cursor.
- Danger actions: a soft danger fill with danger-coloured text, solid only for
  the final confirmation.
- Ghost actions: no fill at rest, the hover fill on hover.

## Icons

- Drawn on a 32px grid and shown at 12, 14, 16 or 20px.
- Muted at rest, ink on hover or when selected.
- Refer to icons by `IconName`; never embed an SVG path in a component.
