// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Components are discovered from the files in `src/`, so a build must notice
//! when a file is added or removed there.

fn main() {
    println!("cargo:rerun-if-changed=src");
}
