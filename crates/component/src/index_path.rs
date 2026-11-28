// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

pub use isonomic_ui_base::IndexPath;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_export_is_the_base_type() {
        fn accepts_base(_: isonomic_ui_base::IndexPath) {}

        let legacy: crate::IndexPath = IndexPath::new(2).section(1).column(3);
        accepts_base(legacy);
    }
}
