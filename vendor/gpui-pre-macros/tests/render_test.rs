// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

#[test]
fn test_derive_render() {
    use gpui_macros::Render;

    #[derive(Render)]
    struct _Element;
}
