# App facade fixture

Checks that a crate which depends only on `isonomic-app` can use the UI
macros without naming GPUI or `isonomic-ui`.

`isonomic-app` here is a stand-in for the SDK's crate of that name: it
re-exports `isonomic-ui` as its `ui` module, which is all the macros rely on.
`consumer` depends on it alone and derives an element, implements `Render`,
declares an action and runs a UI test, all through `isonomic_app::ui`.

This is its own workspace so it is not built with the library. Run it by
hand after changing how the macros find the facade:

```sh
CARGO_TARGET_DIR=target cargo test --manifest-path fixtures/app-facade/Cargo.toml
```
