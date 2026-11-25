# isonomic-ui

Shared GPUI component library and design system for Isonomic apps.

## Layout

| Crate | Path | What it is |
|---|---|---|
| `isonomic-ui` | `crates/ui` | The one crate applications depend on. Re-exports GPUI and the layers below. |
| `isonomic-ui-base` | `crates/base` | Headless behavior: text editing, input methods, focus, overlays, virtual lists, accessibility. |
| `isonomic-ui-component` | `crates/component` | The styled components and the theme. |
| `isonomic-ui-extra` | `crates/extra` | Components for agent and assistant apps, built on the ones above. |
| `isonomic-ui-assets` | `crates/assets` | The IBM Carbon icon set and the IBM Plex typefaces. |
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

See [docs/design-language.md](docs/design-language.md) and
[docs/brand.md](docs/brand.md).

## Scope

This library draws interfaces for native Rust applications. It does not run
scripts or host apps written in other languages: there is no script engine or
app runtime here, and none is planned. Apps from outside run as Views, which
the desktop hosts in its own sandbox. Keeping a runtime out keeps the library
small, keeps every app on one code path, and leaves sandboxing to the one
place that enforces it.

## Third-party work

See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
