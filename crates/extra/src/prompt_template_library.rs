//! Prompt template library: saved prompts with blanks to fill in.
//!
//! A list of templates sits on the left, with a search over them. The chosen
//! one shows on the right as a sentence whose blanks are real text fields, so
//! people type straight into the prompt. The finished prompt assembles in a
//! preview underneath, with a rough token count, and Use only works once
//! every blank is filled. What was typed is kept per template.

use std::{collections::HashMap, time::Duration};

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState, MoveEnd, MoveHome},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, HighlightStyle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText, Subscription, TextRun, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// One blank in a template.
#[derive(Clone)]
pub struct PromptVariable {
    name: SharedString,
    description: Option<SharedString>,
    default: Option<SharedString>,
}

impl PromptVariable {
    /// Describe the blank written as `{name}` in a template's body.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            description: None,
            default: None,
        }
    }

    /// Say what goes in the blank. It shows as the field's placeholder.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Fill the blank to start with.
    pub fn default(mut self, value: impl Into<SharedString>) -> Self {
        self.default = Some(value.into());
        self
    }
}

/// A saved prompt. Its body marks blanks in curly braces: `{audience}`.
#[derive(Clone)]
pub struct PromptTemplate {
    id: SharedString,
    title: SharedString,
    body: SharedString,
    category: Option<SharedString>,
    tone: Option<Hsla>,
    variables: Vec<PromptVariable>,
}

impl PromptTemplate {
    /// Create a template.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            category: None,
            tone: None,
            variables: Vec::new(),
        }
    }

    /// File it under a category, shown as a tag.
    pub fn category(mut self, category: impl Into<SharedString>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Colour its category tag. Categories take colours in order otherwise.
    pub fn tone(mut self, tone: impl Into<Hsla>) -> Self {
        self.tone = Some(tone.into());
        self
    }

    /// Describe one of the blanks.
    pub fn variable(mut self, variable: PromptVariable) -> Self {
        self.variables.push(variable);
        self
    }

    /// The template's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    fn info(&self, name: &str) -> Option<&PromptVariable> {
        self.variables.iter().find(|variable| variable.name == name)
    }
}

/// What happened in the library.
#[derive(Debug, Clone)]
pub enum PromptLibraryEvent {
    /// Another template was chosen.
    Selected(SharedString),
    /// Use was pressed with every blank filled.
    Used {
        /// The template's id.
        id: SharedString,
        /// The finished prompt.
        prompt: String,
        /// Each blank's name and what was typed into it.
        values: Vec<(SharedString, String)>,
    },
    /// The finished prompt was copied.
    Copied(String),
}

/// A run of a template's body.
#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Text(String),
    Blank(String),
}

/// Split a body into text and blanks. A blank is `{name}` with letters,
/// digits, `-` or `_`; any other braces stay as text.
fn parse(body: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut rest = body;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let name_len = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(after.len());
        if name_len > 0 && after[name_len..].starts_with('}') {
            text.push_str(&rest[..open]);
            if !text.is_empty() {
                pieces.push(Piece::Text(std::mem::take(&mut text)));
            }
            pieces.push(Piece::Blank(after[..name_len].to_string()));
            rest = &after[name_len + 1..];
        } else {
            text.push_str(&rest[..=open]);
            rest = after;
        }
    }
    text.push_str(rest);
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    pieces
}

/// The blanks in order, each once.
fn blanks(pieces: &[Piece]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for piece in pieces {
        if let Piece::Blank(name) = piece
            && !names.contains(name)
        {
            names.push(name.clone());
        }
    }
    names
}

/// The prompt with every filled blank put in. Empty blanks stay as `{name}`.
/// Returns the text and, for each blank, where it landed and which it was.
fn assemble(
    pieces: &[Piece],
    value: impl Fn(&str) -> String,
) -> (String, Vec<(std::ops::Range<usize>, String)>) {
    let mut out = String::new();
    let mut spans = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Text(text) => out.push_str(text),
            Piece::Blank(name) => {
                let typed = value(name);
                let start = out.len();
                if typed.trim().is_empty() {
                    out.push('{');
                    out.push_str(name);
                    out.push('}');
                } else {
                    out.push_str(typed.trim());
                }
                spans.push((start..out.len(), name.clone()));
            }
        }
    }
    (out, spans)
}

/// A rough token count: about four characters to a token.
fn tokens(prompt: &str) -> usize {
    prompt.chars().count().div_ceil(4)
}

/// Split text so it can wrap between words, keeping every space.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive(' ')
}

/// A library of prompt templates whose blanks are filled in place.
pub struct PromptTemplateLibrary {
    title: SharedString,
    use_label: SharedString,
    templates: Vec<PromptTemplate>,
    chosen: Option<SharedString>,
    fields: HashMap<(SharedString, String), Entity<InputState>>,
    search: Entity<InputState>,
    focus: FocusHandle,
    copied: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PromptLibraryEvent> for PromptTemplateLibrary {}

impl Focusable for PromptTemplateLibrary {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PromptTemplateLibrary {
    /// Create an empty library.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search templates"));
        let search_events = cx.subscribe_in(&search, window, |this, _, event, _, cx| {
            if let InputEvent::Change = event {
                this.keep_choice_visible(cx);
                cx.notify();
            }
        });
        Self {
            title: "Prompts".into(),
            use_label: "Use prompt".into(),
            templates: Vec::new(),
            chosen: None,
            fields: HashMap::new(),
            search,
            focus: cx.focus_handle(),
            copied: false,
            _subscriptions: vec![search_events],
        }
    }

    /// Add a template.
    pub fn template(mut self, template: PromptTemplate) -> Self {
        if self.chosen.is_none() {
            self.chosen = Some(template.id.clone());
        }
        self.templates.push(template);
        self
    }

    /// Choose a template to start with. The first one otherwise.
    pub fn chosen(mut self, id: impl Into<SharedString>) -> Self {
        self.chosen = Some(id.into());
        self
    }

    /// Change the heading above the list.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Change the use button's label.
    pub fn use_label(mut self, label: impl Into<SharedString>) -> Self {
        self.use_label = label.into();
        self
    }

    /// Choose a template.
    pub fn choose(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if self.chosen.as_ref() != Some(id) && self.templates.iter().any(|t| &t.id == id) {
            self.chosen = Some(id.clone());
            self.copied = false;
            cx.emit(PromptLibraryEvent::Selected(id.clone()));
            cx.notify();
        }
    }

    fn template_now(&self) -> Option<&PromptTemplate> {
        self.chosen
            .as_ref()
            .and_then(|id| self.templates.iter().find(|t| &t.id == id))
    }

    fn visible(&self, cx: &App) -> Vec<usize> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.templates
            .iter()
            .enumerate()
            .filter(|(_, template)| {
                query.is_empty()
                    || [Some(&template.title), template.category.as_ref()]
                        .into_iter()
                        .flatten()
                        .chain([&template.body])
                        .any(|text| text.to_lowercase().contains(&query))
            })
            .map(|(ix, _)| ix)
            .collect()
    }

    fn keep_choice_visible(&mut self, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        let shown = visible
            .iter()
            .any(|ix| self.chosen.as_ref() == Some(&self.templates[*ix].id));
        if !shown && let Some(first) = visible.first() {
            let id = self.templates[*first].id.clone();
            self.choose(&id, cx);
        }
    }

    fn step(&mut self, to: Step, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if visible.is_empty() {
            return;
        }
        let at = visible
            .iter()
            .position(|ix| self.chosen.as_ref() == Some(&self.templates[*ix].id));
        let last = visible.len() - 1;
        let next = match (to, at) {
            (Step::First, _) | (_, None) => 0,
            (Step::Last, _) => last,
            (Step::Up, Some(at)) => at.saturating_sub(1),
            (Step::Down, Some(at)) => (at + 1).min(last),
        };
        let id = self.templates[visible[next]].id.clone();
        self.choose(&id, cx);
    }

    /// Make the text fields for the chosen template's blanks, once.
    fn make_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(template) = self.template_now().cloned() else {
            return;
        };
        for name in blanks(&parse(&template.body)) {
            let key = (template.id.clone(), name.clone());
            if self.fields.contains_key(&key) {
                continue;
            }
            let info = template.info(&name).cloned();
            let field = cx.new(|cx| {
                let placeholder = info
                    .as_ref()
                    .and_then(|info| info.description.clone())
                    .unwrap_or_else(|| name.clone().into());
                let mut state = InputState::new(window, cx).placeholder(placeholder);
                if let Some(start) = info.and_then(|info| info.default) {
                    state.set_value(start, window, cx);
                }
                state
            });
            let id = template.id.clone();
            let blank = name.clone();
            let events =
                cx.subscribe_in(
                    &field,
                    window,
                    move |this, _, event, window, cx| match event {
                        InputEvent::Change => {
                            this.copied = false;
                            cx.notify();
                        }
                        InputEvent::PressEnter { .. } => this.after_enter(&id, &blank, window, cx),
                        _ => {}
                    },
                );
            self.fields.insert(key, field);
            self._subscriptions.push(events);
        }
    }

    fn value(&self, id: &SharedString, name: &str, cx: &App) -> String {
        self.fields
            .get(&(id.clone(), name.to_string()))
            .map(|field| field.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Enter in a blank goes to the next empty one, or uses the prompt.
    fn after_enter(
        &mut self,
        id: &SharedString,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(template) = self.templates.iter().find(|t| &t.id == id) else {
            return;
        };
        let names = blanks(&parse(&template.body));
        let at = names.iter().position(|n| n == name).unwrap_or(0);
        let empty = names
            .iter()
            .cycle()
            .skip(at + 1)
            .take(names.len())
            .find(|n| self.value(id, n, cx).trim().is_empty())
            .cloned();
        match empty {
            Some(next) => {
                if let Some(field) = self.fields.get(&(id.clone(), next)) {
                    field.update(cx, |field, cx| field.focus(window, cx));
                }
            }
            None => self.use_prompt(cx),
        }
    }

    fn finished(&self, cx: &App) -> Option<Finished> {
        let template = self.template_now()?;
        let pieces = parse(&template.body);
        let names = blanks(&pieces);
        let values: Vec<(SharedString, String)> = names
            .iter()
            .map(|name| {
                (
                    SharedString::from(name.clone()),
                    self.value(&template.id, name, cx).trim().to_string(),
                )
            })
            .collect();
        if values.iter().any(|(_, value)| value.is_empty()) {
            return None;
        }
        let (prompt, _) = assemble(&pieces, |name| self.value(&template.id, name, cx));
        Some((template.id.clone(), prompt, values))
    }

    fn use_prompt(&mut self, cx: &mut Context<Self>) {
        if let Some((id, prompt, values)) = self.finished(cx) {
            cx.emit(PromptLibraryEvent::Used { id, prompt, values });
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let Some(template) = self.template_now() else {
            return;
        };
        let (prompt, _) = assemble(&parse(&template.body), |name| {
            self.value(&template.id, name, cx)
        });
        cx.write_to_clipboard(ClipboardItem::new_string(prompt.clone()));
        cx.emit(PromptLibraryEvent::Copied(prompt));
        self.copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Home and End are text actions in the search field; there they move
    /// through the list instead, since the field is a single short line.
    fn list_jump(&mut self, to: Step, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.read(cx).focus_handle(cx).is_focused(window) {
            self.step(to, cx);
            cx.stop_propagation();
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let in_list = self.focus.is_focused(window)
            || self.search.read(cx).focus_handle(cx).is_focused(window);
        let step = match event.keystroke.key.as_str() {
            "up" => Step::Up,
            "down" => Step::Down,
            "home" => Step::First,
            "end" => Step::Last,
            "escape" if !self.search.read(cx).value().is_empty() => {
                self.search
                    .update(cx, |search, cx| search.set_value("", window, cx));
                cx.notify();
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        if in_list {
            self.step(step, cx);
            cx.stop_propagation();
        }
    }
}

/// A template's id, its finished prompt and what each blank holds.
type Finished = (SharedString, String, Vec<(SharedString, String)>);

#[derive(Clone, Copy)]
enum Step {
    Up,
    Down,
    First,
    Last,
}

impl Render for PromptTemplateLibrary {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.make_fields(window, cx);

        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let font = isonomic_ui::font(theme.font_family.clone());
        let text_size = window.rem_size() * 0.875;
        let palette = [
            theme.blue,
            theme.magenta,
            theme.yellow,
            theme.green,
            theme.cyan,
            theme.red,
        ];

        // Categories take colours in the order they first appear.
        let mut categories: Vec<SharedString> = Vec::new();
        for template in &self.templates {
            if let Some(category) = &template.category
                && !categories.contains(category)
            {
                categories.push(category.clone());
            }
        }
        let tone_of = |template: &PromptTemplate| {
            template.tone.unwrap_or_else(|| {
                let at = template
                    .category
                    .as_ref()
                    .and_then(|c| categories.iter().position(|known| known == c))
                    .unwrap_or(0);
                palette[at % palette.len()]
            })
        };
        let tag = |template: &PromptTemplate| {
            let tone = tone_of(template);
            template.category.clone().map(|category| {
                div()
                    .flex_none()
                    .px_1p5()
                    .rounded(px(4.))
                    .bg(tone.opacity(0.13))
                    .text_color(tone)
                    .text_xs()
                    .child(category)
            })
        };

        let visible = self.visible(cx);
        let rows = visible.iter().map(|ix| {
            let template = &self.templates[*ix];
            let picked = self.chosen.as_ref() == Some(&template.id);
            let id = template.id.clone();
            h_flex()
                .id(("prompt-template", *ix))
                .gap_2()
                .px_2()
                .py_1p5()
                .rounded(radius)
                .cursor_pointer()
                .when(picked, |this| this.bg(chip))
                .hover(|style| style.bg(chip.opacity(0.6)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    this.choose(&id, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(!picked, |this| this.text_color(quiet))
                        .child(template.title.clone()),
                )
                .children(tag(template))
        });

        let list = v_flex()
            .id("prompt-list")
            .track_focus(&self.focus)
            .w(px(250.))
            .flex_none()
            .gap_0p5()
            .p_2()
            .border_r_1()
            .border_color(line)
            .child(
                h_flex()
                    .gap_1p5()
                    .px_1()
                    .pb_1()
                    .child(div().font_semibold().child(self.title.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child(self.templates.len().to_string()),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .px_2()
                    .mb_1()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .child(Icon::new(IconName::Search).xsmall().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.search).appearance(false).small()),
                    ),
            )
            .children(rows)
            .when(visible.is_empty(), |this| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(quiet)
                        .child("No template matches"),
                )
            });

        let detail = self.template_now().cloned().map(|template| {
            let pieces = parse(&template.body);
            let names = blanks(&pieces);
            let filled = names
                .iter()
                .filter(|name| !self.value(&template.id, name, cx).trim().is_empty())
                .count();
            let total = names.len();
            let focused_blank = names
                .iter()
                .find(|name| {
                    self.fields
                        .get(&(template.id.clone(), name.to_string()))
                        .is_some_and(|field| field.read(cx).focus_handle(cx).is_focused(window))
                })
                .cloned();

            let sentence = h_flex()
                .flex_wrap()
                .items_center()
                .gap_y_1()
                .line_height(relative(1.6))
                .children(pieces.iter().enumerate().flat_map(|(ix, piece)| {
                    let parts: Vec<isonomic_ui::AnyElement> = match piece {
                        Piece::Text(text) => words(text)
                            .map(|word| {
                                div()
                                    .whitespace_nowrap()
                                    .child(word.to_string())
                                    .into_any_element()
                            })
                            .collect(),
                        Piece::Blank(name) => {
                            let field = self.fields.get(&(template.id.clone(), name.clone()));
                            let typed = self.value(&template.id, name, cx);
                            let shown: SharedString = if typed.is_empty() {
                                template
                                    .info(name)
                                    .and_then(|info| info.description.clone())
                                    .unwrap_or_else(|| name.clone().into())
                            } else {
                                typed.clone().into()
                            };
                            // Size the field to what it shows, with room for the caret and the
                            // next letter, so the text never scrolls inside it.
                            let run = TextRun {
                                len: shown.len(),
                                font: font.clone(),
                                color: ink,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            };
                            let width = window
                                .text_system()
                                .shape_line(shown, text_size, &[run], None)
                                .width
                                .max(px(24.))
                                + px(36.);
                            let lit = focused_blank.as_deref() == Some(name.as_str());
                            let empty = typed.trim().is_empty();
                            field
                                .map(|field| {
                                    div()
                                        .id(("prompt-blank", ix))
                                        .w(width)
                                        .max_w_full()
                                        .h(px(24.))
                                        .rounded(px(5.))
                                        .bg(if lit {
                                            accent.opacity(0.16)
                                        } else if empty {
                                            chip.opacity(0.6)
                                        } else {
                                            chip
                                        })
                                        .when(empty, |this| {
                                            this.border_1().border_dashed().border_color(line)
                                        })
                                        .child(Input::new(field).appearance(false).small())
                                        .into_any_element()
                                })
                                .into_iter()
                                .collect()
                        }
                    };
                    parts
                }));

            let (prompt, spans) = assemble(&pieces, |name| self.value(&template.id, name, cx));
            let highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = spans
                .into_iter()
                .map(|(range, name)| {
                    let empty = self.value(&template.id, &name, cx).trim().is_empty();
                    let lit = focused_blank.as_deref() == Some(name.as_str());
                    (
                        range,
                        HighlightStyle {
                            color: Some(if empty { quiet } else { ink }),
                            font_weight: (!empty).then_some(FontWeight::SEMIBOLD),
                            background_color: lit.then_some(accent.opacity(0.18)),
                            ..Default::default()
                        },
                    )
                })
                .collect();

            v_flex()
                .flex_1()
                .min_w_0()
                .gap_3()
                .p_3()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_semibold()
                                .child(template.title.clone()),
                        )
                        .children(tag(&template))
                        .when(total > 0, |this| {
                            this.child(h_flex().gap_0p5().children((0..total).map(|dot| {
                                div()
                                    .w(px(7.))
                                    .h(px(5.))
                                    .rounded(px(2.))
                                    .bg(if dot < filled { accent } else { line })
                            })))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(quiet)
                                    .child(format!("{filled} of {total} filled")),
                            )
                        }),
                )
                .child(sentence)
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_medium()
                                .text_color(quiet)
                                .child("PREVIEW"),
                        )
                        .child(
                            div()
                                .p_2p5()
                                .rounded(radius)
                                .bg(chip.opacity(0.5))
                                .border_1()
                                .border_color(line)
                                .text_color(quiet)
                                .line_height(relative(1.5))
                                .child(StyledText::new(prompt).with_highlights(highlights)),
                        ),
                )
                .into_any_element()
        });

        let finished = self.finished(cx);
        let ready = finished.is_some();
        let estimate = self
            .template_now()
            .map(|template| {
                let pieces = parse(&template.body);
                let missing = blanks(&pieces)
                    .iter()
                    .filter(|name| self.value(&template.id, name, cx).trim().is_empty())
                    .count();
                let (prompt, _) = assemble(&pieces, |name| self.value(&template.id, name, cx));
                match missing {
                    0 => format!("~{} tokens", tokens(&prompt)),
                    1 => format!("1 blank left \u{00b7} ~{} tokens", tokens(&prompt)),
                    n => format!("{n} blanks left \u{00b7} ~{} tokens", tokens(&prompt)),
                }
            })
            .unwrap_or_default();

        v_flex()
            .capture_key_down(cx.listener(Self::key_down))
            .capture_action(
                cx.listener(|this, _: &MoveHome, window, cx| {
                    this.list_jump(Step::First, window, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &MoveEnd, window, cx| this.list_jump(Step::Last, window, cx)),
            )
            .w_full()
            .max_w(px(700.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .items_stretch()
                    .min_h(px(260.))
                    .child(list)
                    .children(detail),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .child(div().flex_1().text_xs().text_color(quiet).child(estimate))
                    .child(
                        Button::new("prompt-copy")
                            .outline()
                            .small()
                            .icon(if self.copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .label(if self.copied { "Copied" } else { "Copy" })
                            .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                    )
                    .child(
                        Button::new("prompt-use")
                            .primary()
                            .small()
                            .accessibility_label(self.use_label.clone())
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(self.use_label.clone())
                                    .child(Icon::new(IconName::ArrowRight).xsmall()),
                            )
                            .disabled(!ready)
                            .on_click(cx.listener(|this, _, _, cx| this.use_prompt(cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blanks_are_found_and_other_braces_kept() {
        let pieces = parse("Hi {name}, see {a b} and {name} {x_1}!");
        assert_eq!(
            pieces,
            vec![
                Piece::Text("Hi ".into()),
                Piece::Blank("name".into()),
                Piece::Text(", see {a b} and ".into()),
                Piece::Blank("name".into()),
                Piece::Text(" ".into()),
                Piece::Blank("x_1".into()),
                Piece::Text("!".into()),
            ]
        );
        assert_eq!(blanks(&pieces), vec!["name", "x_1"]);
        assert_eq!(parse("{}{"), vec![Piece::Text("{}{".into())]);
    }

    #[test]
    fn the_prompt_fills_what_it_can() {
        let pieces = parse("Remind {client} about {invoice}.");
        let (prompt, spans) = assemble(&pieces, |name| match name {
            "client" => " Fernbrook ".into(),
            _ => String::new(),
        });
        assert_eq!(prompt, "Remind Fernbrook about {invoice}.");
        assert_eq!(spans[0].0, 7..16);
        assert_eq!(&prompt[spans[1].0.clone()], "{invoice}");
    }

    #[test]
    fn tokens_are_about_four_characters() {
        assert_eq!(tokens(""), 0);
        assert_eq!(tokens("abcd"), 1);
        assert_eq!(tokens("abcde"), 2);
    }
}
