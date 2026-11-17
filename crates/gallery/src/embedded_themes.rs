// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

#[cfg(target_family = "wasm")]
use std::collections::HashMap;

/// Themes compiled into the web gallery. The built-in theme needs no entry.
#[cfg(target_family = "wasm")]
pub fn embedded_themes() -> HashMap<&'static str, &'static str> {
    HashMap::new()
}
