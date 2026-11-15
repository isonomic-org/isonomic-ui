//! Command palette: every action in one searchable list, grouped, with recent ones first and each one's shortcut.
//!
//! With nothing typed, recent commands lead, then every group. Typing
//! narrows to the commands whose names hold the typed letters in order,
//! best matches first, across groups. Up and Down move, Enter runs, and
//! Escape asks the owner to close the palette. Commands that can't run right
//! now are shown greyed with the reason.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::lifted_shadow;

/// One command.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteCommand {
    /// Identifies it.
    pub id: SharedString,
    /// What it does.
    pub name: SharedString,
    /// Its group, such as "Invoices".
    pub group: SharedString,
    /// Its shortcut, if it has one.
    pub keys: Option<Keystroke>,
    /// Why it can't run right now, if it can't.
    pub unavailable: Option<SharedString>,
}

impl PaletteCommand {
    /// Create a command.
    pub fn new(id: impl Into<SharedString>, group: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            group: group.into(),
            keys: None,
            unavailable: None,
        }
    }

    /// Give it a shortcut, such as "cmd-shift-p".
    pub fn keys(mut self, keys: &'static str) -> Self {
        self.keys = Keystroke::parse(keys).ok();
        self
    }

    /// Say why it can't run right now.
    pub fn unavailable(mut self, why: impl Into<SharedString>) -> Self {
        self.unavailable = Some(why.into());
        self
    }
}

/// Whether the typed letters appear in order in `name`, and how well: lower is better.
pub fn match_rank(name: &str, typed: &str) -> Option<usize> {
    let name = name.to_lowercase();
    let typed: String = typed.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if typed.is_empty() {
        return Some(0);
    }
    if let Some(at) = name.find(&typed) {
        return Some(at);
    }
    let mut first = None;
    let mut last = 0;
    let mut letters = typed.chars().peekable();
    for (at, c) in name.char_indices() {
        if letters.peek() == Some(&c) {
            first.get_or_insert(at);
            last = at;
            letters.next();
        }
    }
    (letters.peek().is_none()).then(|| 100 + first.unwrap_or(0) + last)
}

/// What the palette reports.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteEvent {
    /// Run this command.
    Run(SharedString),
    /// Close the palette.
    Dismiss,
}

/// A searchable list of commands.
pub struct CommandPalette {
    commands: Vec<PaletteCommand>,
    recent: Vec<SharedString>,
    field: Entity<InputState>,
    cursor: usize,
    _events: Subscription,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl CommandPalette {
    /// Create a palette.
    pub fn new(commands: Vec<PaletteCommand>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder("Type a command"));
        let events = cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
            InputEvent::Change => {
                this.cursor = 0;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.run(cx),
            _ => {}
        });
        Self {
            commands,
            recent: Vec::new(),
            field,
            cursor: 0,
            _events: events,
        }
    }

    /// Set the recent commands, newest first, by id.
    pub fn set_recent(&mut self, recent: Vec<SharedString>, cx: &mut Context<Self>) {
        self.recent = recent;
        cx.notify();
    }

    /// The rows showing, each with its heading if it starts a group.
    fn rows(&self, cx: &App) -> Vec<(Option<SharedString>, PaletteCommand)> {
        let typed = self.field.read(cx).value().to_string();
        if !typed.trim().is_empty() {
            let mut found: Vec<_> = self
                .commands
                .iter()
                .filter_map(|c| match_rank(&c.name, &typed).map(|r| (r, c.clone())))
                .collect();
            found.sort_by_key(|(r, _)| *r);
            return found.into_iter().enumerate().map(|(ix, (_, c))| (if ix == 0 { Some("Best matches".into()) } else { None }, c)).collect();
        }
        let mut rows = Vec::new();
        for (ix, id) in self.recent.iter().enumerate() {
            if let Some(c) = self.commands.iter().find(|c| &c.id == id) {
                rows.push((if ix == 0 { Some("Recent".into()) } else { None }, c.clone()));
            }
        }
        let mut groups: Vec<SharedString> = Vec::new();
        for c in &self.commands {
            if !groups.contains(&c.group) {
                groups.push(c.group.clone());
            }
        }
        for group in groups {
            for (ix, c) in self.commands.iter().filter(|c| c.group == group).enumerate() {
                rows.push((if ix == 0 { Some(group.clone()) } else { None }, c.clone()));
            }
        }
        rows
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if let Some((_, command)) = rows.get(self.cursor.min(rows.len().saturating_sub(1))) {
            if command.unavailable.is_none() {
                cx.emit(PaletteEvent::Run(command.id.clone()));
            }
        }
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let rows = self.rows(cx);
        let count = rows.len();
        let cursor = self.cursor.min(count.saturating_sub(1));

        let list: Vec<_> = rows
            .iter()
            .enumerate()
            .map(|(ix, (heading, command))| {
                let id = command.id.clone();
                let off = command.unavailable.is_some();
                v_flex()
                    .children(heading.clone().map(|h| div().px_2().pt_2().pb_1().text_xs().font_medium().text_color(quiet).child(h)))
                    .child(
                        h_flex()
                            .id(ElementId::Name(format!("command-{}-{ix}", command.id).into()))
                            .gap_2()
                            .px_2()
                            .h(px(32.))
                            .rounded(px(6.))
                            .when(ix == cursor, |this| this.bg(chip))
                            .when(!off, |this| this.cursor_pointer())
                            .aria_selected(ix == cursor)
                            .aria_label(match &command.unavailable {
                                Some(why) => format!("{}, unavailable: {why}", command.name),
                                None => command.name.to_string(),
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cursor = ix;
                                if !off {
                                    cx.emit(PaletteEvent::Run(id.clone()));
                                }
                            }))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().truncate().when(off, |this| this.text_color(quiet)).child(command.name.clone()))
                                    .children(command.unavailable.clone().map(|why| div().text_xs().text_color(quiet).truncate().child(why))),
                            )
                            .children(command.keys.clone().map(|keys| Kbd::new(keys))),
                    )
            })
            .collect();

        v_flex()
            .w_full()
            .max_w(px(520.))
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(lifted_shadow(cx))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| match event.keystroke.key.as_str() {
                "down" if count > 0 => {
                    this.cursor = (cursor + 1) % count;
                    cx.notify();
                    cx.stop_propagation();
                }
                "up" if count > 0 => {
                    this.cursor = (cursor + count - 1) % count;
                    cx.notify();
                    cx.stop_propagation();
                }
                "escape" => cx.emit(PaletteEvent::Dismiss),
                _ => {}
            }))
            .child(
                div()
                    .p_2()
                    .border_b_1()
                    .border_color(line)
                    .child(Input::new(&self.field).appearance(false).prefix(Icon::new(IconName::Search).small().text_color(quiet))),
            )
            .child(
                v_flex()
                    .id("palette-list")
                    .max_h(px(340.))
                    .overflow_y_scroll()
                    .p_1()
                    .children(list)
                    .when(count == 0, |this| this.child(div().px_3().py_4().text_color(quiet).child("No commands match."))),
            )
            .child(
                h_flex()
                    .gap_3()
                    .px_3()
                    .py_1p5()
                    .border_t_1()
                    .border_color(line)
                    .text_xs()
                    .text_color(quiet)
                    .child("\u{2191}\u{2193} to move")
                    .child("Enter to run")
                    .child("Esc to close"),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_words_beat_scattered_letters() {
        assert_eq!(match_rank("Send reminders", "remind"), Some(5));
        assert!(match_rank("Send reminders", "sdr").unwrap() >= 100);
        assert_eq!(match_rank("Void invoice", "zz"), None);
        assert_eq!(match_rank("Anything", ""), Some(0));
    }
}
