// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

#[test]
fn test_derive_context() {
    use gpui::{App, Window};
    use gpui_macros::{AppContext, VisualContext};

    #[derive(AppContext, VisualContext)]
    struct _MyCustomContext<'a, 'b> {
        #[app]
        app: &'a mut App,
        #[window]
        window: &'b mut Window,
    }
}
