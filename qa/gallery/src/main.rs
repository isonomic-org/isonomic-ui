//! QA gallery for isonomic-ui.
//!
//! Shows one story of the component gallery (`crates/gallery`) in a bare
//! window: no sidebar, no title bar, only the story. The story registry is the
//! gallery's own, so every story it lists can be tested here without editing
//! anything in this crate. Light/dark and the window size are set from the
//! command line or by the QA runner over stdin.
//!
//! Command line:
//!   qa-gallery [--story <name>] [--theme light|dark] [--width <px>] [--height <px>] [--list]
//!
//! `--story` takes a story's name as the gallery lists it (case does not
//! matter). `--story __empty__` shows an empty window, which the runner times
//! redraws against. `--list` prints every story name in gallery order, one per
//! line as `story<TAB>name`, and exits.
//!
//! Stdin commands (one per line), used by the QA runner:
//!   theme light|dark      switch theme
//!   size <w> <h>          resize the window
//!   a11y <path>           write the last accessibility tree as JSON to <path>
//!   focus <path>          write the focused element's debug name to <path>
//!   ping <path>           write "ok" to <path> after the next frame
//!   tabstart              move focus to the first tab stop, as an app's initial focus would
//!   frames <path> <n>     redraw n frames back to back and write each frame's time (ms) as JSON
//!   quit                  exit

use std::{
    io::BufRead as _,
    sync::mpsc::{Receiver, channel},
    time::{Duration, Instant},
};

use isonomic_ui::{
    App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window, WindowBounds, WindowOptions,
    assets::AllAssets,
    component::{ActiveTheme as _, Theme, ThemeMode},
    div, px, size,
};
use isonomic_ui_gallery::Gallery;

/// The story name that shows an empty window.
const EMPTY: &str = "__empty__";

#[derive(Clone, Debug)]
struct Args {
    story: Option<String>,
    dark: bool,
    width: f32,
    height: f32,
    list: bool,
}

fn parse_args() -> Option<Args> {
    let mut args = Args {
        story: None,
        dark: false,
        width: 960.,
        height: 900.,
        list: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--story" => args.story = it.next(),
            "--theme" => args.dark = it.next().as_deref() == Some("dark"),
            "--width" => args.width = it.next().and_then(|v| v.parse().ok()).unwrap_or(960.),
            "--height" => args.height = it.next().and_then(|v| v.parse().ok()).unwrap_or(900.),
            "--list" => args.list = true,
            "--help" | "-h" => {
                println!(
                    "qa-gallery [--story <name>] [--theme light|dark] [--width <px>] [--height <px>] [--list]"
                );
                return None;
            }
            other => eprintln!("qa-gallery: ignoring unknown argument {other}"),
        }
    }
    Some(args)
}

/// The window's content: the gallery showing one story, or nothing.
struct Stage {
    gallery: Option<Entity<Gallery>>,
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .children(self.gallery.clone())
    }
}

fn set_dark(dark: bool, window: &mut Window, cx: &mut App) {
    let mode = if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Theme::change(mode, Some(window), cx);
    window.refresh();
}

enum Command {
    Theme(bool),
    Size(f32, f32),
    A11y(String),
    Focus(String),
    Ping(String),
    Frames(String, usize),
    TabStart,
    Quit,
}

fn parse_command(line: &str) -> Option<Command> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "theme" => Some(Command::Theme(parts.next() == Some("dark"))),
        "size" => {
            let w = parts.next()?.parse().ok()?;
            let h = parts.next()?.parse().ok()?;
            Some(Command::Size(w, h))
        }
        "a11y" => Some(Command::A11y(parts.next()?.to_string())),
        "focus" => Some(Command::Focus(parts.next()?.to_string())),
        "ping" => Some(Command::Ping(parts.next()?.to_string())),
        "frames" => {
            let path = parts.next()?.to_string();
            let n = parts.next().and_then(|n| n.parse().ok()).unwrap_or(120);
            Some(Command::Frames(path, n))
        }
        "tabstart" => Some(Command::TabStart),
        "quit" => Some(Command::Quit),
        _ => None,
    }
}

/// Redraw `left` more frames back to back, timing each one, then write the times.
fn record_frames(
    window: &mut Window,
    left: usize,
    last: Instant,
    mut times: Vec<f64>,
    path: String,
) {
    window.refresh();
    window.on_next_frame(move |window, _| {
        let now = Instant::now();
        times.push(now.duration_since(last).as_secs_f64() * 1000.);
        if left <= 1 {
            let json = format!(
                "[{}]",
                times
                    .iter()
                    .map(|t| format!("{t:.3}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let _ = std::fs::write(&path, json);
        } else {
            record_frames(window, left - 1, now, times, path);
        }
    });
}

fn spawn_stdin() -> Receiver<Command> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            match parse_command(&line) {
                Some(cmd) => {
                    if tx.send(cmd).is_err() {
                        break;
                    }
                }
                None => eprintln!("qa-gallery: unknown command {line:?}"),
            }
        }
    });
    rx
}

fn main() {
    let Some(args) = parse_args() else {
        return;
    };
    if std::env::var_os("RUST_LOG").is_none() {
        // Safety: set before any other thread starts.
        unsafe { std::env::set_var("RUST_LOG", "warn") };
    }

    let app = isonomic_ui::application().with_assets(AllAssets);
    app.run(move |cx: &mut App| {
        isonomic_ui_gallery::init(cx);
        let bounds = Bounds::centered(None, size(px(args.width), px(args.height)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let empty = args.story.as_deref() == Some(EMPTY);
        // The gallery opens on its first story when the name is not one of its own.
        let story = args.story.clone().unwrap_or_default();
        let (window, stage) = isonomic_ui::open_window(options, cx, |window, cx| {
            set_dark(args.dark, window, cx);
            let gallery = (!empty).then(|| Gallery::embedded_view(&story, window, cx));
            cx.new(|_| Stage { gallery })
        })
        .expect("open window");

        if let Some(gallery) = stage.read(cx).gallery.clone() {
            let names = gallery.read(cx).story_names(cx);
            if args.list {
                use std::io::Write as _;
                let mut out = std::io::stdout().lock();
                for name in &names {
                    // The marker tells names apart from log lines on the same stream.
                    let _ = writeln!(out, "story\t{name}");
                }
                let _ = out.flush();
                std::process::exit(0);
            }
            if args.story.is_some() && !names.iter().any(|name| name.eq_ignore_ascii_case(&story)) {
                eprintln!("qa-gallery: no story named {story}");
                std::process::exit(3);
            }
        }
        cx.activate(true);

        let rx = spawn_stdin();
        let dark = args.dark;
        cx.spawn(async move |cx| {
            // The gallery restores its last theme in the background when it
            // starts; apply the requested one again once that has landed.
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = window.update(cx, |_, window, cx| set_dark(dark, window, cx));
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                while let Ok(cmd) = rx.try_recv() {
                    let quit = matches!(cmd, Command::Quit);
                    let _ = window.update(cx, |_, window, cx| match cmd {
                        Command::Theme(dark) => set_dark(dark, window, cx),
                        Command::Size(w, h) => window.resize(size(px(w), px(h))),
                        Command::A11y(path) => {
                            let json = window
                                .debug_a11y_tree_json()
                                .unwrap_or_else(|| "null".into());
                            let _ = std::fs::write(path, json);
                        }
                        Command::Focus(path) => {
                            let focused = window
                                .focused(cx)
                                .map(|f| format!("{f:?}"))
                                .unwrap_or_else(|| "none".into());
                            let _ = std::fs::write(path, focused);
                        }
                        Command::Ping(path) => {
                            window.refresh();
                            window.on_next_frame(move |_, _| {
                                let _ = std::fs::write(&path, "ok");
                            });
                        }
                        Command::Frames(path, n) => {
                            record_frames(window, n.max(1), Instant::now(), Vec::new(), path)
                        }
                        Command::TabStart => window.focus_next(cx),
                        Command::Quit => cx.quit(),
                    });
                    if quit {
                        return;
                    }
                }
            }
        })
        .detach();
    });
}
