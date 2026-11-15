# isonomic-ui

Shared GPUI component library and design system for Isonomic apps.

## Layout

| Crate | Path | What it is |
|---|---|---|
| `isonomic-ui` | `crates/ui` | The one crate applications depend on. Re-exports GPUI and the layers below. |
| `isonomic-ui-base` | `crates/base` | Headless behavior: text editing, input methods, focus, overlays, virtual lists, accessibility. |
| `isonomic-ui-component` | `crates/component` | The styled components and the theme. |
| `isonomic-ui-assets` | `crates/assets` | The icon set and the typeface. |
| `isonomic-ui-macros` | `crates/macros` | Procedural macros. |
| `isonomic-ui-gallery` | `crates/gallery` | The gallery application: one story per component. |
| `isonomic-ui-fps` | `crates/fps` | Performance HUD used by the gallery. |

This is the only repository that names a GPUI version. Every other Isonomic
repository gets GPUI through `isonomic-ui`.

## Develop

```sh
script/bootstrap          # Linux system packages; a no-op on macOS
cargo run                 # the gallery
cargo test --workspace --features isonomic-ui-gallery/test-support
```

Render every story to `crates/gallery/target/screenshots/` in light and dark:

```sh
cargo test -p isonomic-ui-gallery --features test-support --test screenshots
```

## Design

See [docs/design-language.md](docs/design-language.md).

## Third-party work

See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
