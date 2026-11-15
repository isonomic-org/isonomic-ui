use std::env;

fn main() {
    // Keep the existing expansion-time directory available for custom macro
    // calls. The shared enum and legacy adapter no longer rely on this path.
    let icons_dir = env::var("DEP_ISONOMIC_UI_DEFAULT_ICONS_ICONS_DIR")
        .expect("isonomic-ui-assets publishes its icon directory through Cargo metadata");
    println!("cargo:rustc-env=ISONOMIC_UI_DEFAULT_ICONS_DIR={icons_dir}");
    println!("cargo:rerun-if-changed={icons_dir}");
    println!("cargo:rerun-if-changed=build.rs");
}
