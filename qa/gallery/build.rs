//! Finds every story under crates/gallery/src/stories/extra/ and writes a
//! registry, so a new component story shows up without touching this crate.
//!
//! Set QA_STORIES_DIR to point at another checkout. Set QA_ONLY to a
//! comma-separated list of slugs to build a subset (used when one story fails
//! to compile in the combined build).

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dir = env::var("QA_STORIES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../../crates/gallery/src/stories/extra"));
    let dir = dir.canonicalize().unwrap_or(dir);
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-env-changed=QA_STORIES_DIR");
    println!("cargo:rerun-if-env-changed=QA_ONLY");
    let only: Option<Vec<String>> = env::var("QA_ONLY")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.split(',').map(|x| x.trim().replace('-', "_")).collect());

    let mut entries = Vec::new();
    if let Ok(read) = fs::read_dir(&dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let module = path.file_stem().unwrap().to_string_lossy().to_string();
            if module == "mod" {
                continue;
            }
            if let Some(only) = &only {
                if !only.contains(&module) {
                    continue;
                }
            }
            println!("cargo:rerun-if-changed={}", path.display());
            let text = fs::read_to_string(&path).unwrap_or_default();
            // The story type is the public struct whose name ends in "Story"
            // and that has a `view` constructor.
            let story = text
                .lines()
                .filter_map(|l| l.trim().strip_prefix("pub struct "))
                .map(|rest| {
                    rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or("")
                        .to_string()
                })
                .find(|name| name.ends_with("Story"));
            if let Some(story) = story {
                entries.push((module, story, path));
            } else {
                println!("cargo:warning=no *Story struct in {}", path.display());
            }
        }
    }
    entries.sort();

    let mut out = String::new();
    for (module, _, path) in &entries {
        out.push_str(&format!(
            "#[path = {:?}]\n#[allow(dead_code, unused_imports, clippy::all)]\npub mod {module};\n",
            path.display().to_string()
        ));
    }
    out.push_str("\n/// Every story found: (slug, constructor).\npub const STORIES: &[(&str, fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView)] = &[\n");
    for (module, story, _) in &entries {
        let slug = module.replace('_', "-");
        out.push_str(&format!(
            "    ({slug:?}, |window, cx| {module}::{story}::view(window, cx).into()),\n"
        ));
    }
    out.push_str("];\n");
    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("stories.rs");
    fs::write(dest, out).unwrap();
}
