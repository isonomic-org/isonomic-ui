// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Plays one gallery story offscreen and writes a strip of frames, so motion
//! can be checked without a screen: a slide, a roll, a fade.
//!
//! The clock is the test clock, so frames are exact: `frames:8x40` draws eight
//! frames 40ms apart. Steps run in order and are given after the story name:
//!
//! ```sh
//! cargo test -p isonomic-ui-gallery --features test-support --test filmstrip -- \
//!     "rolling number" click:next-day frames:8x80
//! ```
//!
//! Steps: `click:<element id>`, `hover:<element id>` (a part of a component
//! is `<component id>/<part>`), `press:<key>`,
//! `type:<text>`, `tab` (move focus on), `wait:<ms>`, `frames:<count>x<ms>`,
//! `theme:dark`, `theme:light`, `still` (reduced motion on). Frames go to
//! `target/filmstrip/<story>-NN.png` in this crate (or under `$FILMSTRIP_DIR`).
fn main() {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    filmstrip::run();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    println!("filmstrip: skipped; GPUI does not supply a headless renderer on this platform");
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod filmstrip {
    use isonomic_ui::{
        ElementId, HeadlessAppContext, SharedString,
        assets::AllAssets,
        component::{Theme, ThemeMode},
        px, size,
        test::TestWindowExt as _,
    };
    use isonomic_ui_gallery::Gallery;
    use std::{path::PathBuf, sync::Arc, time::Duration};

    const WIDTH: f32 = 1200.;
    const HEIGHT: f32 = 900.;
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

    /// `card` is an element's own id; `list/card` is the part `card` of the
    /// element `list`, the way components name their parts; `row#3` is the
    /// numbered element `("row", 3)`.
    fn element_id(path: &str) -> ElementId {
        let named = |part: &str| match part.split_once('#') {
            Some((name, ix)) => ElementId::NamedInteger(
                name.to_string().into(),
                ix.parse().expect("an element id like `row#3`"),
            ),
            None => ElementId::Name(part.to_string().into()),
        };
        let mut parts = path.split('/');
        let mut id = named(parts.next().unwrap_or_default());
        for part in parts {
            id = ElementId::NamedChild(id.into(), part.to_string().into());
        }
        id
    }

    pub fn run() {
        let out = PathBuf::from(
            std::env::var("FILMSTRIP_DIR").unwrap_or_else(|_| "target/filmstrip".into()),
        );
        let mut args = std::env::args().skip(1).filter(|arg| !arg.starts_with('-'));
        let Some(wanted) = args.next() else {
            println!("filmstrip: give a story name, then steps");
            return;
        };
        let steps: Vec<String> = args.collect();
        let _ = std::fs::remove_file(GALLERY_STATE_FILE);
        std::fs::create_dir_all(&out).expect("failed to create the filmstrip directory");

        let mut cx = HeadlessAppContext::with_platform(
            isonomic_ui::platform::current_platform(true).text_system(),
            Arc::new(AllAssets),
            isonomic_ui::platform::current_headless_renderer,
        );
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
                        focus: true,
                        show: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| Gallery::embedded_view("Welcome", window, cx),
                )
            })
            .expect("failed to open the headless window");
        cx.run_until_parked();

        let name = cx
            .update(|cx| gallery.read(cx).story_names(cx))
            .into_iter()
            .map(|name| name.to_string())
            .find(|name| name.to_lowercase() == wanted.to_lowercase() || slug(name) == wanted)
            .unwrap_or_else(|| panic!("no story matches `{wanted}`"));

        cx.update(|cx| Theme::change(ThemeMode::Light, None, cx));
        cx.update_window(handle, |_, window, cx| {
            assert!(gallery.update(cx, |gallery, cx| gallery.select_story(&name, window, cx)));
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.advance_clock(Duration::from_millis(500));
        cx.run_until_parked();

        let mut frame = 0;
        for step in &steps {
            let (verb, rest) = step.split_once(':').unwrap_or((step.as_str(), ""));
            match verb {
                "click" | "hover" | "press" | "type" | "tab" => {
                    let rest = SharedString::from(rest.to_string());
                    let target = element_id(&rest);
                    cx.update_window(handle, |_, window, cx| {
                        match verb {
                            "click" => window.click(target, cx),
                            "hover" => window.hover(target, cx),
                            "press" => window.press(&rest, cx),
                            "type" => window.input(&rest, cx),
                            _ => window.focus_next(cx),
                        }
                        window.render_frame(cx);
                    })
                    .unwrap();
                }
                "wait" => {
                    let ms: u64 = rest.parse().expect("wait:<ms>");
                    cx.advance_clock(Duration::from_millis(ms));
                    cx.run_until_parked();
                }
                "theme" => {
                    let mode = if rest == "dark" {
                        ThemeMode::Dark
                    } else {
                        ThemeMode::Light
                    };
                    cx.update(|cx| Theme::change(mode, None, cx));
                }
                "still" => cx.update(|cx| cx.set_reduce_motion(true)),
                "frames" => {
                    let (count, ms) = rest.split_once('x').expect("frames:<count>x<ms>");
                    let count: usize = count.parse().expect("frames:<count>x<ms>");
                    let ms: u64 = ms.parse().expect("frames:<count>x<ms>");
                    for _ in 0..count {
                        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                            .unwrap();
                        let image = cx
                            .capture_screenshot(handle)
                            .expect("offscreen rendering must be available");
                        let path = out.join(format!("{}-{frame:02}.png", slug(&name)));
                        image.save(&path).unwrap_or_else(|err| {
                            panic!("failed to write {}: {err}", path.display())
                        });
                        println!("wrote {}", path.display());
                        frame += 1;
                        cx.advance_clock(Duration::from_millis(ms));
                        cx.run_until_parked();
                    }
                }
                other => panic!("unknown step `{other}`"),
            }
        }

        let _ = std::fs::remove_file(GALLERY_STATE_FILE);
        println!("filmstrip: {frame} frames of `{name}` in {}", out.display());
    }
}
