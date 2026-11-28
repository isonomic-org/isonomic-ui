// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finds the stories in `src/stories/extra/`, one file per component of
//! `isonomic-ui-extra`, and in `src/stories/screens/`, one file per screen
//! composed from existing components, and writes the code that registers
//! them: the module for each file, the gallery panels, and the match that
//! rebuilds a panel from its story's name. Adding a story file there is all a
//! new story needs, so pull requests that each add one never touch the same
//! lines.
//!
//! The file `src/stories/<dir>/<name>.rs` holds `<Name>Story`, `<Name>` being
//! `<name>` in UpperCamelCase.

use std::{env, fmt::Write as _, fs, path::PathBuf};

fn main() {
    register("extra", "extra_stories.rs");
    register("screens", "screen_stories.rs");
}

/// Writes `OUT_DIR/<target>` for the stories in `src/stories/<group>/`.
fn register(group: &str, target: &str) {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by Cargo"))
        .join("src")
        .join("stories")
        .join(group);
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|_| panic!("src/stories/{group} must exist"))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()? != "rs" {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_owned();
            (name != "mod").then_some(name)
        })
        .collect();
    names.sort();

    let story = |name: &str| {
        let mut ty: String = name
            .split('_')
            .map(|word| {
                let mut letters = word.chars();
                letters
                    .next()
                    .map(|first| first.to_uppercase().chain(letters).collect::<String>())
                    .unwrap_or_default()
            })
            .collect();
        ty.push_str("Story");
        ty
    };

    let mut out = String::new();
    for name in &names {
        let path = dir.join(format!("{name}.rs"));
        let path = path.to_str().expect("story paths are UTF-8");
        writeln!(
            out,
            "#[path = {path:?}]\nmod {name};\npub use {name}::{};",
            story(name)
        )
        .unwrap();
    }

    out.push_str(
        "\n/// A gallery panel for every story of this group, in name order.\n\
         pub(crate) fn panels(\n    window: &mut isonomic_ui::Window,\n    cx: &mut isonomic_ui::App,\n\
         ) -> Vec<isonomic_ui::Entity<crate::StoryContainer>> {\n",
    );
    if names.is_empty() {
        out.push_str("    let _ = (window, cx);\n    Vec::new()\n}\n");
    } else {
        out.push_str("    vec![\n");
        for name in &names {
            writeln!(
                out,
                "        crate::StoryContainer::panel::<{}>(window, cx),",
                story(name)
            )
            .unwrap();
        }
        out.push_str("    ]\n}\n");
    }

    out.push_str(
        "\n/// What `StoryState::to_story` needs for the story of this group named `klass`.\n\
         pub(crate) fn story_for(\n    klass: &str,\n    window: &mut isonomic_ui::Window,\n    cx: &mut isonomic_ui::App,\n\
         ) -> Option<(\n    &'static str,\n    &'static str,\n    bool,\n\
         \x20   Option<isonomic_ui::component::dock::PanelControl>,\n    isonomic_ui::AnyView,\n\
         \x20   fn(isonomic_ui::AnyView, bool, &mut isonomic_ui::Window, &mut isonomic_ui::App),\n)> {\n",
    );
    if names.is_empty() {
        out.push_str("    let _ = (klass, window, cx);\n    None\n}\n");
    } else {
        out.push_str("    use crate::Story as _;\n    match klass {\n");
        for name in &names {
            let ty = story(name);
            writeln!(
                out,
                "        {ty:?} => Some((\n            {ty}::title(),\n            {ty}::description(),\n\
                 \x20           {ty}::closable(),\n            {ty}::zoomable(),\n\
                 \x20           {ty}::view(window, cx).into(),\n            {ty}::on_active_any,\n        )),"
            )
            .unwrap();
        }
        out.push_str("        _ => None,\n    }\n}\n");
    }

    let target = PathBuf::from(env::var("OUT_DIR").expect("set by Cargo")).join(target);
    fs::write(target, out).expect("OUT_DIR is writable");
}
