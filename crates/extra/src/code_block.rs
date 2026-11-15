//! Code block: code to read and copy.
//!
//! A header names the file or the language and holds the copy button. The body
//! keeps exact spacing, can number and mark lines, and either scrolls long
//! lines sideways or wraps them. Past a maximum height it scrolls inside.
//!
//! Colors come from the base syntax highlighter, so a language is colored when
//! the application has enabled a grammar for it and shown as plain text otherwise.

use std::{
    hash::{DefaultHasher, Hash as _, Hasher as _},
    ops::Range,
    rc::Rc,
    sync::Arc,
};

use isonomic_ui::component::{
    ActiveTheme as _, Rope, StyledExt as _, clipboard::Clipboard, h_flex,
    highlighter::SyntaxHighlighter, v_flex,
};
use isonomic_ui::{
    App, ElementId, HighlightStyle, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    StyledText, Window, div, prelude::FluentBuilder as _, px, relative,
};

const LINE_HEIGHT: Pixels = px(20.);
const INSET: Pixels = px(14.);

type CopyHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// A panel of code with a header, optional line numbers and marked lines.
#[derive(IntoElement)]
pub struct CodeBlock {
    id: ElementId,
    style: StyleRefinement,
    code: SharedString,
    language: Option<SharedString>,
    filename: Option<SharedString>,
    line_numbers: bool,
    marked: Vec<usize>,
    wrap: bool,
    max_height: Option<Pixels>,
    copyable: bool,
    on_copy: Option<CopyHandler>,
}

impl CodeBlock {
    /// Create a block. The code is copied exactly as given.
    pub fn new(id: impl Into<ElementId>, code: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            code: code.into(),
            language: None,
            filename: None,
            line_numbers: false,
            marked: Vec::new(),
            wrap: false,
            max_height: None,
            copyable: true,
            on_copy: None,
        }
    }

    /// Name the language, such as `"rust"` or `"json"`. It colors the code when
    /// a grammar is available and names the header when there is no file name.
    pub fn language(mut self, language: impl Into<SharedString>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Show a file name in the header instead of the language.
    pub fn filename(mut self, filename: impl Into<SharedString>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Number every line. Numbers stay in place while long lines scroll sideways.
    pub fn line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }

    /// Mark lines, counted from 1, with a light tint and a thin edge.
    pub fn highlight_lines(mut self, lines: impl IntoIterator<Item = usize>) -> Self {
        self.marked = lines.into_iter().collect();
        self
    }

    /// Wrap long lines instead of scrolling them sideways.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Scroll the code inside the block once it is taller than this.
    pub fn max_height(mut self, max_height: impl Into<Pixels>) -> Self {
        self.max_height = Some(max_height.into());
        self
    }

    /// Show the copy button. On by default. Without it and without a file name
    /// or language the block has no header.
    pub fn copyable(mut self, copyable: bool) -> Self {
        self.copyable = copyable;
        self
    }

    /// Run after the code was copied, for example to show a toast.
    pub fn on_copy(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }
}

impl Styled for CodeBlock {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// One line of code and the colored ranges inside it.
#[derive(Clone)]
struct Line {
    text: SharedString,
    runs: Vec<(Range<usize>, HighlightStyle)>,
}

/// Lines kept between frames so highlighting runs only when its inputs change.
#[derive(Default)]
struct Highlighted {
    key: u64,
    lines: Rc<[Line]>,
}

/// The text shown: line endings normalized, and one trailing newline dropped
/// because that is how files end, not an empty last line.
fn displayed(code: &str) -> String {
    let text = code.replace("\r\n", "\n").replace('\r', "\n");
    match text.strip_suffix('\n') {
        Some(stripped) => stripped.to_string(),
        None => text,
    }
}

/// Split text into lines, cutting each colored range at the line boundaries so
/// a comment or string that spans lines keeps its color on every one.
fn split_lines(text: &str, styles: &[(Range<usize>, HighlightStyle)]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;
    for part in text.split('\n') {
        let end = start + part.len();
        let runs = styles
            .iter()
            .filter(|(range, _)| range.start < end && range.end > start)
            .map(|(range, style)| {
                (
                    range.start.max(start) - start..range.end.min(end) - start,
                    *style,
                )
            })
            .collect();
        lines.push(Line {
            text: part.to_string().into(),
            runs,
        });
        start = end + 1;
    }
    lines
}

/// A readable name for the header when there is no file name.
fn language_label(language: &str) -> SharedString {
    let name = match language.to_ascii_lowercase().as_str() {
        "rs" | "rust" => "Rust",
        "ts" | "typescript" => "TypeScript",
        "tsx" => "TSX",
        "js" | "javascript" | "mjs" | "cjs" => "JavaScript",
        "jsx" => "JSX",
        "json" | "jsonc" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "sh" | "shell" => "Shell",
        "bash" => "Bash",
        "zsh" => "Zsh",
        "py" | "python" => "Python",
        "go" => "Go",
        "sql" => "SQL",
        "html" => "HTML",
        "css" => "CSS",
        "md" | "markdown" => "Markdown",
        _ => return language.to_string().into(),
    };
    name.into()
}

impl RenderOnce for CodeBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let faint = theme.muted_foreground.opacity(0.6);
        let line = theme.border;
        let panel = theme.background.blend(theme.foreground.opacity(0.02));
        let mark = theme.info.opacity(0.08);
        let mark_edge = theme.info.opacity(0.6);
        let clear = theme.transparent;
        let radius = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let highlight_theme = theme.highlight_theme.clone();

        let key = {
            let mut hasher = DefaultHasher::new();
            self.code.hash(&mut hasher);
            self.language.hash(&mut hasher);
            (Arc::as_ptr(&highlight_theme) as usize).hash(&mut hasher);
            hasher.finish()
        };
        let state = window.use_keyed_state((self.id.clone(), "lines"), cx, |_, _| {
            Highlighted::default()
        });
        if state.read(cx).key != key || state.read(cx).lines.is_empty() {
            let text = displayed(&self.code);
            let styles = match &self.language {
                Some(language) if !text.is_empty() => {
                    let mut highlighter = SyntaxHighlighter::new(language);
                    highlighter.update(None, &Rope::from_str(&text), None);
                    highlighter.styles(&(0..text.len()), highlight_theme.as_ref())
                }
                _ => Vec::new(),
            };
            let lines: Rc<[Line]> = split_lines(&text, &styles).into();
            state.update(cx, |state, _| {
                state.key = key;
                state.lines = lines;
            });
        }
        let lines = state.read(cx).lines.clone();

        let label = self
            .filename
            .clone()
            .or_else(|| self.language.as_deref().map(language_label));
        let has_header = label.is_some() || self.copyable;
        let digits = lines.len().to_string().len() as f32;
        let gutter_width = px(digits * 7.5 + 30.);
        let line_numbers = self.line_numbers;
        let wrap = self.wrap;
        let is_marked = |ix: usize| self.marked.contains(&(ix + 1));

        // The tint and the thin edge of a marked line, on whichever element
        // starts the line. Unmarked lines keep the edge's width so text aligns.
        let paint = |marked: bool, this: isonomic_ui::Div| -> isonomic_ui::Div {
            this.border_l_2()
                .border_color(if marked { mark_edge } else { clear })
                .when(marked, |this| this.bg(mark))
        };
        let number = |ix: usize, marked: bool| {
            div()
                .flex_none()
                .w(gutter_width)
                .h(LINE_HEIGHT)
                .pl(px(14.))
                .pr(INSET)
                .text_color(if marked { quiet } else { faint })
                .text_right()
                .child((ix + 1).to_string())
        };
        let code = |line: &Line, this: isonomic_ui::Div| -> isonomic_ui::Div {
            this.pr(INSET)
                .when(!line_numbers, |this| this.pl(INSET))
                .child(StyledText::new(line.text.clone()).with_highlights(line.runs.clone()))
        };

        let body = if wrap {
            // Wrapped lines differ in height, so each number travels with its
            // line and the whole row carries the mark.
            v_flex()
                .w_full()
                .children(lines.iter().enumerate().map(|(ix, line)| {
                    let marked = is_marked(ix);
                    paint(marked, h_flex().items_start())
                        .when(line_numbers, |this| this.child(number(ix, marked)))
                        .child(code(line, div().flex_1().min_w_0().min_h(LINE_HEIGHT)))
                }))
                .into_any_element()
        } else {
            // The number column sits outside the sideways scroll, so numbers
            // stay put. The mark is painted in both columns of a marked line.
            let numbers = line_numbers.then(|| {
                v_flex()
                    .flex_none()
                    .children(lines.iter().enumerate().map(|(ix, _)| {
                        let marked = is_marked(ix);
                        paint(marked, div()).child(number(ix, marked))
                    }))
            });
            let rows = lines.iter().enumerate().map(|(ix, line)| {
                let marked = is_marked(ix);
                let row = code(line, div().h(LINE_HEIGHT).whitespace_nowrap());
                if line_numbers {
                    row.when(marked, |this| this.bg(mark))
                } else {
                    paint(marked, row)
                }
            });
            h_flex()
                .items_start()
                .w_full()
                .children(numbers)
                .child(
                    div()
                        .id("code")
                        .flex_1()
                        .min_w_0()
                        .overflow_x_scroll()
                        .child(v_flex().min_w_full().children(rows)),
                )
                .into_any_element()
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .rounded(radius)
            .border_1()
            .border_color(line)
            .bg(panel)
            .text_color(ink)
            .when(has_header, |this| {
                this.child(header(
                    label,
                    self.copyable.then(|| {
                        Clipboard::new((self.id.clone(), "copy"))
                            .value(self.code.clone())
                            .when_some(self.on_copy.clone(), |this, on_copy| {
                                this.on_copied(move |value, window, cx| on_copy(value, window, cx))
                            })
                    }),
                    quiet,
                    line,
                ))
            })
            .child(
                div()
                    .id("scroll")
                    .w_full()
                    .py(px(10.))
                    .overflow_y_scroll()
                    .when_some(self.max_height, |this, max_height| this.max_h(max_height))
                    .font_family(mono)
                    .text_xs()
                    .line_height(LINE_HEIGHT)
                    .child(body),
            )
            .refine_style(&self.style)
    }
}

/// The header row: the name at the start, the copy button at the end.
fn header(
    label: Option<SharedString>,
    copy: Option<Clipboard>,
    quiet: Hsla,
    line: Hsla,
) -> impl IntoElement {
    h_flex()
        .flex_none()
        .gap_2()
        .h(px(36.))
        .pl(INSET)
        .pr(px(6.))
        .border_b_1()
        .border_color(line)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .line_height(relative(1.35))
                .font_medium()
                .text_color(quiet)
                .children(label),
        )
        .children(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colored(range: Range<usize>) -> (Range<usize>, HighlightStyle) {
        (range, HighlightStyle::default())
    }

    #[test]
    fn one_trailing_newline_is_not_an_empty_line() {
        assert_eq!(displayed("a\nb\n"), "a\nb");
        // Only one is dropped: a deliberate blank last line stays.
        assert_eq!(displayed("a\n\n"), "a\n");
        assert_eq!(displayed("a\r\nb\r\n"), "a\nb");
    }

    #[test]
    fn lines_keep_their_text() {
        let lines = split_lines("let a = 1;\n\nlet b = 2;", &[]);
        let text: Vec<&str> = lines.iter().map(|line| line.text.as_ref()).collect();
        assert_eq!(text, ["let a = 1;", "", "let b = 2;"]);
    }

    #[test]
    fn a_range_spanning_lines_is_cut_at_each_line() {
        // "/* a" + newline + "b */": the comment covers all nine bytes.
        let lines = split_lines("/* a\nb */", &[colored(0..9)]);
        assert_eq!(lines[0].runs[0].0, 0..4);
        assert_eq!(lines[1].runs[0].0, 0..4);
    }

    #[test]
    fn ranges_are_relative_to_their_line() {
        let lines = split_lines("ab\ncdef", &[colored(4..6)]);
        assert!(lines[0].runs.is_empty());
        assert_eq!(lines[1].runs[0].0, 1..3);
    }

    #[test]
    fn unknown_languages_keep_their_own_name() {
        assert_eq!(language_label("rs").as_ref(), "Rust");
        assert_eq!(language_label("JSON").as_ref(), "JSON");
        assert_eq!(language_label("nix").as_ref(), "nix");
    }
}
