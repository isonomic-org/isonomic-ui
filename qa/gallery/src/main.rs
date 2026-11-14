//! QA gallery for isonomic-ui.
//!
//! Shows one story by name (`--story card`) or a browsable list of every story
//! found under `crates/gallery/src/stories/extra/`. Light/dark and window width
//! can be switched from the toolbar, from the command line, or by the QA runner
//! over stdin.
//!
//! Command line:
//!   qa-gallery [--story <slug>] [--theme light|dark] [--width <px>] [--height <px>] [--list]
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

use gpui::{
    AnyView, App, AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, size,
};
use gpui_component::{
    ActiveTheme as _, Sizable as _, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

mod stories {
    include!(concat!(env!("OUT_DIR"), "/stories.rs"));
}

#[derive(Clone, Debug)]
struct Args {
    story: Option<String>,
    dark: bool,
    width: f32,
    height: f32,
}

fn parse_args() -> Option<Args> {
    let mut args = Args {
        story: None,
        dark: false,
        width: 960.,
        height: 900.,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--story" => args.story = it.next(),
            "--theme" => args.dark = it.next().as_deref() == Some("dark"),
            "--width" => args.width = it.next().and_then(|v| v.parse().ok()).unwrap_or(960.),
            "--height" => args.height = it.next().and_then(|v| v.parse().ok()).unwrap_or(900.),
            "--list" => {
                for (slug, _) in stories::STORIES {
                    println!("{slug}");
                }
                return None;
            }
            "--help" | "-h" => {
                println!(
                    "qa-gallery [--story <slug>] [--theme light|dark] [--width <px>] [--height <px>] [--list]"
                );
                return None;
            }
            other => eprintln!("qa-gallery: ignoring unknown argument {other}"),
        }
    }
    Some(args)
}

const WIDTHS: [f32; 3] = [360., 960., 1440.];

struct Gallery {
    /// Index into `stories::STORIES` of the story on show.
    current: Option<usize>,
    view: Option<AnyView>,
    /// Single-story mode hides the list and toolbar so screenshots show only the story.
    single: bool,
    height: f32,
}

impl Gallery {
    fn new(args: &Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current = args
            .story
            .as_ref()
            .and_then(|s| stories::STORIES.iter().position(|(slug, _)| slug == s));
        if let (Some(name), None) = (&args.story, current) {
            eprintln!("qa-gallery: no story named {name}");
        }
        let mut this = Self {
            current: None,
            view: None,
            single: args.story.is_some(),
            height: args.height,
        };
        let first = current.or(if this.single { None } else { Some(0) });
        if let Some(ix) = first {
            this.show(ix, window, cx);
        }
        this
    }

    fn show(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, make)) = stories::STORIES.get(ix) {
            self.current = Some(ix);
            self.view = Some(make(window, cx));
            cx.notify();
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

    fn toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let width = f32::from(window.bounds().size.width).round();
        let title: SharedString = self
            .current
            .and_then(|ix| stories::STORIES.get(ix))
            .map(|(slug, _)| SharedString::from(slug.to_string()))
            .unwrap_or_else(|| "No story".into());
        h_flex()
            .id("qa-toolbar")
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().flex_1().text_sm().font_weight(gpui::FontWeight::MEDIUM).child(title))
            .children(WIDTHS.iter().map(|w| {
                let w = *w;
                Button::new(SharedString::from(format!("qa-width-{w}")))
                    .xsmall()
                    .when(width == w, |b| b.primary())
                    .when(width != w, |b| b.ghost())
                    .label(format!("{w}"))
                    .on_click(cx.listener(move |this, _, window, _| {
                        window.resize(size(px(w), px(this.height)));
                    }))
            }))
            .child(
                Button::new("qa-theme")
                    .xsmall()
                    .outline()
                    .label(if dark { "Light" } else { "Dark" })
                    .on_click(cx.listener(move |_, _, window, cx| {
                        Self::set_dark(!dark, window, cx);
                    })),
            )
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let stage = div()
            .id("qa-stage")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_6()
            .bg(theme.background)
            .text_color(theme.foreground)
            .when_some(self.view.clone(), |this, view| this.child(view));

        if self.single {
            return div()
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground)
                .font_family(theme.font_family.clone())
                .child(stage)
                .into_any_element();
        }

        let list = v_flex()
            .id("qa-story-list")
            .w(px(220.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(theme.border)
            .p_2()
            .gap_0p5()
            .children(stories::STORIES.iter().enumerate().map(|(ix, (slug, _))| {
                let selected = self.current == Some(ix);
                Button::new(("qa-story", ix))
                    .xsmall()
                    .w_full()
                    .justify_start()
                    .when(selected, |b| b.primary())
                    .when(!selected, |b| b.ghost())
                    .label(*slug)
                    .on_click(cx.listener(move |this, _, window, cx| this.show(ix, window, cx)))
            }));

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .child(self.toolbar(window, cx))
            .child(h_flex().flex_1().min_h_0().child(list).child(stage))
            .into_any_element()
    }
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
fn record_frames(window: &mut Window, left: usize, last: Instant, mut times: Vec<f64>, path: String) {
    window.refresh();
    window.on_next_frame(move |window, _| {
        let now = Instant::now();
        times.push(now.duration_since(last).as_secs_f64() * 1000.);
        if left <= 1 {
            let json = format!(
                "[{}]",
                times.iter().map(|t| format!("{t:.3}")).collect::<Vec<_>>().join(",")
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
    env_logger::init();

    let app = gpui_platform::application().with_assets(gpui_kit_assets::Assets);
    app.run(move |cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, size(px(args.width), px(args.height)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let window = cx
            .open_window(options, |window, cx| {
                Gallery::set_dark(args.dark, window, cx);
                let gallery: Entity<Gallery> = cx.new(|cx| Gallery::new(&args, window, cx));
                cx.new(|cx| gpui_base::Root::new(gallery, window, cx))
            })
            .expect("open window");
        cx.activate(true);

        let rx = spawn_stdin();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                while let Ok(cmd) = rx.try_recv() {
                    let quit = matches!(cmd, Command::Quit);
                    let _ = window.update(cx, |_, window, cx| match cmd {
                        Command::Theme(dark) => Gallery::set_dark(dark, window, cx),
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
