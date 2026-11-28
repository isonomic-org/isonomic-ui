// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use isonomic_ui::{AssetSource, IntoElement, ParentElement, assets::IconName};

isonomic_ui::assets::icon_assets!(AppAssets, [Search, Check]);

#[test]
fn assets_are_usable_through_kit_without_component() {
    let _ = isonomic_ui::div()
        .child(IconName::Search)
        .into_any_element();
    assert!(AppAssets.load(&IconName::Search.path()).unwrap().is_some());
    assert!(
        AppAssets
            .load(&IconName::Accessibility.path())
            .unwrap()
            .is_none()
    );
}

#[cfg(feature = "component")]
#[test]
fn component_accepts_shared_names_and_preserves_legacy_views() {
    use isonomic_ui::component::{Icon, IconNameExt};
    use isonomic_ui::component::{IconName as LegacyIconName, IconNamed};
    let _: fn(LegacyIconName, &mut isonomic_ui::App) -> isonomic_ui::Entity<Icon> =
        LegacyIconName::view;
    assert_eq!(LegacyIconName::Search.path(), "icons/search.svg");
    let _: isonomic_ui::AnyElement = LegacyIconName::Search.into();
    let shared: IconName = LegacyIconName::Search.into();
    assert_eq!(shared, IconName::Search);
    let _ = Icon::new(shared);
    let _ = Icon::new(IconName::Accessibility);
    let _ = Icon::new(LegacyIconName::Search);
    let _: fn(IconName, &mut isonomic_ui::App) -> isonomic_ui::Entity<Icon> = IconNameExt::view;
}
