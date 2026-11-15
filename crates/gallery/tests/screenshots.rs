//! Renders every gallery story offscreen, in light and dark, and writes one PNG
//! per story and theme to `target/screenshots/<theme>/<story>.png` inside this
//! crate's directory (or under `$SCREENSHOT_DIR`).
//!
//! This needs GPUI's real offscreen renderer: Metal on macOS, WGPU on Linux
//! (a software Vulkan adapter such as Mesa lavapipe works). It is opt-in, like
//! the `rendering` target in `crates/ui`:
//!
//! ```sh
//! cargo test -p isonomic-ui-gallery --features test-support --test screenshots
//! ```
//!
//! Story names after `--` render a subset: `... --test screenshots -- button badge`.
fn main() {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    screenshots::run();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    println!("screenshots: skipped; GPUI does not supply a headless renderer on this platform");
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod screenshots {
    use isonomic_ui::{
        HeadlessAppContext,
        assets::AllAssets,
        component::{ActiveTheme as _, Theme, ThemeMode},
        px, size,
        test::TestWindowExt as _,
    };
    use isonomic_ui_gallery::Gallery;
    use std::{path::PathBuf, sync::Arc, time::Duration};

    /// Logical window size. The headless window renders at 2x, so images are
    /// twice these dimensions.
    const WIDTH: f32 = 1200.;
    const HEIGHT: f32 = 1600.;

    /// Where the gallery persists the theme chosen in its last session. A
    /// capture must not depend on an earlier run, and must not leave its own
    /// theme behind for the next interactive session.
    const GALLERY_STATE_FILE: &str = "target/state.json";

    fn slug(name: &str) -> String {
        let mut slug = String::new();
        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
        }
        slug.trim_end_matches('-').to_string()
    }

    pub fn run() {
        let out = PathBuf::from(
            std::env::var("SCREENSHOT_DIR").unwrap_or_else(|_| "target/screenshots".into()),
        );
        let filter: Vec<String> = std::env::args()
            .skip(1)
            .filter(|arg| !arg.starts_with('-'))
            .map(|arg| arg.to_lowercase())
            .collect();
        let _ = std::fs::remove_file(GALLERY_STATE_FILE);

        let mut cx = HeadlessAppContext::with_platform(
            isonomic_ui::platform::current_platform(true).text_system(),
            Arc::new(AllAssets),
            isonomic_ui::platform::current_headless_renderer,
        );
        // Stories read files and decode assets on real threads.
        cx.allow_parking();
        cx.update(isonomic_ui_gallery::init);

        let (handle, gallery) = cx
            .update(|cx| {
                isonomic_ui::open_window(
                    isonomic_ui::WindowOptions {
                        window_bounds: Some(isonomic_ui::WindowBounds::Windowed(
                            isonomic_ui::Bounds {
                                origin: Default::default(),
                                size: size(px(WIDTH), px(HEIGHT)),
                            },
                        )),
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| Gallery::embedded_view("Welcome", window, cx),
                )
            })
            .expect("failed to open the headless window");
        // The gallery loads its theme directory in the background and applies
        // the result when it arrives; let that land before choosing a theme.
        cx.run_until_parked();

        let names: Vec<String> = cx
            .update(|cx| gallery.read(cx).story_names(cx))
            .into_iter()
            .map(|name| name.to_string())
            .filter(|name| {
                filter.is_empty()
                    || filter
                        .iter()
                        .any(|wanted| name.to_lowercase() == *wanted || slug(name) == *wanted)
            })
            .collect();
        assert!(!names.is_empty(), "no story matches {filter:?}");

        let mut written = 0;
        for (mode, dir) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            let dir = out.join(dir);
            std::fs::create_dir_all(&dir).expect("failed to create the screenshot directory");

            for name in &names {
                // A story may switch the theme itself, so set it for each one.
                cx.update(|cx| Theme::change(mode, None, cx));
                cx.update_window(handle, |_, window, cx| {
                    let selected =
                        gallery.update(cx, |gallery, cx| gallery.select_story(name, window, cx));
                    assert!(selected, "story `{name}` is not in the gallery");
                    window.render_frame(cx);
                })
                .unwrap();
                // Let deferred work (asset decoding, entrance motion) settle,
                // then draw the frame that is captured.
                cx.run_until_parked();
                cx.advance_clock(Duration::from_millis(500));
                cx.run_until_parked();
                cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                    .unwrap();
                // A capture filed under the wrong theme is worse than no capture.
                let is_dark = cx.update(|cx| cx.theme().is_dark());
                assert_eq!(
                    is_dark,
                    mode.is_dark(),
                    "story `{name}` was drawn in the wrong theme"
                );

                let image = cx
                    .capture_screenshot(handle)
                    .expect("offscreen rendering must be available");
                let path = dir.join(format!("{}.png", slug(name)));
                image
                    .save(&path)
                    .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
                written += 1;
                println!("wrote {}", path.display());
            }
        }

        let _ = std::fs::remove_file(GALLERY_STATE_FILE);
        println!(
            "screenshots: {written} images for {} stories in {}",
            names.len(),
            out.display()
        );
    }
}
