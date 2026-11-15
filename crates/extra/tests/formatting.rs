//! The components here and their stories are found at build time, not through
//! `mod` lines, so `cargo fmt` never visits them. This test does.

use std::{path::Path, process::Command};

fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", dir.display()))
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    files
}

#[test]
fn components_and_stories_are_formatted() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = rust_files(&root.join("src"));
    files.extend(rust_files(&root.join("../gallery/src/stories/extra")));

    let output = match Command::new("rustfmt")
        .args(["--edition", "2024", "--check"])
        .args(&files)
        .output()
    {
        Ok(output) => output,
        // A toolchain without rustfmt cannot check; the pull request checks have it.
        Err(_) => return,
    };
    assert!(
        output.status.success(),
        "run `rustfmt --edition 2024` on these files:\n{}",
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.starts_with("Diff in"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
