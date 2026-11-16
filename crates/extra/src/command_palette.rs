//! Command palette: every action in one searchable list, grouped, with recent ones first and each one's shortcut.
//!
//! With nothing typed, recent commands lead, then every group. Typing
//! narrows to the commands whose names hold the typed letters in order,
//! best matches first, across groups. Up and Down move, Enter runs, and
//! Escape asks the owner to close the palette. Commands that can't run right
//! now are shown greyed with the reason.

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, Size, StyledExt as _, h_flex,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke, ParentElement as _, Pixels,
    Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
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
    /// An icon in front of its name, if it has one.
    pub icon: Option<IconName>,
    /// A quiet note at the end of its row, such as a date or a count.
    pub meta: Option<SharedString>,
}

impl PaletteCommand {
    /// Create a command.
    pub fn new(
        id: impl Into<SharedString>,
        group: impl Into<SharedString>,
        name: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            group: group.into(),
            keys: None,
            unavailable: None,
            icon: None,
            meta: None,
        }
    }

    /// Put an icon in front of its name.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Add a quiet note at the end of its row, such as a date or a count.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
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
    let typed: String = typed
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
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

/// How the palette is drawn and behaves, apart from what it holds. Every
/// field starts at the palette's long-standing look.
struct PaletteOptions {
    width: Pixels,
    input_size: Size,
    escape_hint: bool,
    row_height: Pixels,
    keep_groups: bool,
    footer_hints: Option<Vec<SharedString>>,
    select_first: bool,
    empty_text: SharedString,
    hold_focus: bool,
}

impl Default for PaletteOptions {
    fn default() -> Self {
        Self {
            width: px(520.),
            input_size: Size::Medium,
            escape_hint: false,
            row_height: px(32.),
            keep_groups: false,
            footer_hints: None,
            select_first: true,
            empty_text: "No commands match.".into(),
            hold_focus: false,
        }
    }
}

/// A searchable list of commands.
pub struct CommandPalette {
    commands: Vec<PaletteCommand>,
    recent: Vec<SharedString>,
    field: Entity<InputState>,
    cursor: usize,
    /// Whether the person has typed or pressed an arrow since the palette
    /// was made or last reset. Until then no row is selected when
    /// `select_first` is off.
    touched: bool,
    options: PaletteOptions,
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
                this.touched = true;
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
            touched: false,
            options: PaletteOptions::default(),
            _events: events,
        }
    }

    /// Set the palette's greatest width. The default is 520 px.
    pub fn set_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.options.width = width;
        cx.notify();
    }

    /// Set the words shown in the empty field. The default is "Type a command".
    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.field.update(cx, |field, cx| {
            field.set_placeholder(placeholder, window, cx)
        });
    }

    /// Set the size of the field. The default is medium.
    pub fn set_input_size(&mut self, size: Size, cx: &mut Context<Self>) {
        self.options.input_size = size;
        cx.notify();
    }

    /// Show an Esc key at the end of the field. Off by default.
    pub fn set_escape_hint(&mut self, show: bool, cx: &mut Context<Self>) {
        self.options.escape_hint = show;
        cx.notify();
    }

    /// Set the height of each row. The default is 32 px.
    pub fn set_row_height(&mut self, height: Pixels, cx: &mut Context<Self>) {
        self.options.row_height = height;
        cx.notify();
    }

    /// Keep commands under their group headings while the person types,
    /// instead of one list of best matches. Off by default.
    pub fn set_keep_groups(&mut self, keep: bool, cx: &mut Context<Self>) {
        self.options.keep_groups = keep;
        cx.notify();
    }

    /// Set the hints in the footer, such as "Enter to open". An empty list
    /// hides the footer. The default is the three hints for moving, running
    /// and closing.
    pub fn set_footer_hints(
        &mut self,
        hints: impl IntoIterator<Item = impl Into<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        self.options.footer_hints = Some(hints.into_iter().map(Into::into).collect());
        cx.notify();
    }

    /// Whether the first row is selected as soon as the palette shows. On by
    /// default. Off, no row is selected, and Enter does nothing, until the
    /// person types or presses an arrow.
    pub fn set_select_first(&mut self, select: bool, cx: &mut Context<Self>) {
        self.options.select_first = select;
        cx.notify();
    }

    /// Set the words shown when nothing matches. The default is "No commands match."
    pub fn set_empty_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.options.empty_text = text.into();
        cx.notify();
    }

    /// Keep the keyboard inside the palette: Tab and Shift+Tab stay on the
    /// field instead of moving to whatever is behind it. Off by default; turn
    /// it on where the palette is shown over other content.
    pub fn set_hold_focus(&mut self, hold: bool, cx: &mut Context<Self>) {
        self.options.hold_focus = hold;
        cx.notify();
    }

    /// Forget that the person typed or pressed an arrow, so the next showing
    /// starts with no row selected when selecting the first row is off.
    pub fn reset_selection(&mut self, cx: &mut Context<Self>) {
        self.cursor = 0;
        self.touched = false;
        cx.notify();
    }

    /// The selected row, if one is.
    fn selected(&self, count: usize) -> Option<usize> {
        if count == 0 || (!self.options.select_first && !self.touched) {
            return None;
        }
        Some(self.cursor.min(count - 1))
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
            if self.options.keep_groups {
                // Groups keep the order they first appear in; inside a
                // group the best match leads.
                let mut groups: Vec<SharedString> = Vec::new();
                for c in &self.commands {
                    if !groups.contains(&c.group) {
                        groups.push(c.group.clone());
                    }
                }
                let mut rows = Vec::new();
                for group in groups {
                    for (ix, (_, c)) in found.iter().filter(|(_, c)| c.group == group).enumerate() {
                        rows.push((if ix == 0 { Some(group.clone()) } else { None }, c.clone()));
                    }
                }
                return rows;
            }
            return found
                .into_iter()
                .enumerate()
                .map(|(ix, (_, c))| {
                    (
                        if ix == 0 {
                            Some("Best matches".into())
                        } else {
                            None
                        },
                        c,
                    )
                })
                .collect();
        }
        let mut rows = Vec::new();
        for (ix, id) in self.recent.iter().enumerate() {
            if let Some(c) = self.commands.iter().find(|c| &c.id == id) {
                rows.push((
                    if ix == 0 { Some("Recent".into()) } else { None },
                    c.clone(),
                ));
            }
        }
        let mut groups: Vec<SharedString> = Vec::new();
        for c in &self.commands {
            if !groups.contains(&c.group) {
                groups.push(c.group.clone());
            }
        }
        for group in groups {
            for (ix, c) in self
                .commands
                .iter()
                .filter(|c| c.group == group)
                .enumerate()
            {
                rows.push((if ix == 0 { Some(group.clone()) } else { None }, c.clone()));
            }
        }
        rows
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let Some(selected) = self.selected(rows.len()) else {
            return;
        };
        if let Some((_, command)) = rows.get(selected) {
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
        let selected = self.selected(count);
        let row_height = self.options.row_height;
        let field_focus = self.field.focus_handle(cx);
        let hold_focus = self.options.hold_focus;
        let footer_hints: Vec<SharedString> =
            self.options.footer_hints.clone().unwrap_or_else(|| {
                vec![
                    "\u{2191}\u{2193} to move".into(),
                    "Enter to run".into(),
                    "Esc to close".into(),
                ]
            });

        let list: Vec<_> =
            rows.iter()
                .enumerate()
                .map(|(ix, (heading, command))| {
                    let id = command.id.clone();
                    let off = command.unavailable.is_some();
                    v_flex()
                        .children(heading.clone().map(|h| {
                            div()
                                .px_2()
                                .pt_2()
                                .pb_1()
                                .text_xs()
                                .font_medium()
                                .text_color(quiet)
                                .child(h)
                        }))
                        .child(
                            h_flex()
                                .id(ElementId::Name(
                                    format!("command-{}-{ix}", command.id).into(),
                                ))
                                .gap_2()
                                .px_2()
                                .h(row_height)
                                .rounded(px(6.))
                                .when(selected == Some(ix), |this| this.bg(chip))
                                .when(!off, |this| this.cursor_pointer())
                                .aria_selected(selected == Some(ix))
                                .role(Role::ListBoxOption)
                                .aria_label(match &command.unavailable {
                                    Some(why) => format!("{}, unavailable: {why}", command.name),
                                    None => command.name.to_string(),
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.cursor = ix;
                                    this.touched = true;
                                    if !off {
                                        cx.emit(PaletteEvent::Run(id.clone()));
                                    }
                                }))
                                .children(command.icon.clone().map(|icon| {
                                    Icon::new(icon).small().flex_none().text_color(quiet)
                                }))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .when(off, |this| this.text_color(quiet))
                                                .child(command.name.clone()),
                                        )
                                        .children(command.unavailable.clone().map(|why| {
                                            div().text_xs().text_color(quiet).truncate().child(why)
                                        })),
                                )
                                .children(command.meta.clone().map(|meta| {
                                    div().flex_none().text_xs().text_color(quiet).child(meta)
                                }))
                                .children(command.keys.clone().map(|keys| Kbd::new(keys))),
                        )
                })
                .collect();

        v_flex()
            .w_full()
            .max_w(self.options.width)
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(lifted_shadow(cx))
            .when(hold_focus, |this| {
                // Tab would move to whatever is behind the palette; keep it
                // on the field, where typing and the arrows work.
                this.capture_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "tab" {
                        field_focus.focus(window, cx);
                        cx.stop_propagation();
                    }
                })
            })
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "down" if count > 0 => {
                        // The first arrow lands on the first row when none was selected.
                        this.cursor = match selected {
                            Some(at) => (at + 1) % count,
                            None => 0,
                        };
                        this.touched = true;
                        cx.notify();
                        cx.stop_propagation();
                    }
                    "up" if count > 0 => {
                        this.cursor = match selected {
                            Some(at) => (at + count - 1) % count,
                            None => count - 1,
                        };
                        this.touched = true;
                        cx.notify();
                        cx.stop_propagation();
                    }
                    "escape" => cx.emit(PaletteEvent::Dismiss),
                    _ => {}
                }
            }))
            .child(
                div().p_2().border_b_1().border_color(line).child(
                    Input::new(&self.field)
                        .appearance(false)
                        .with_size(self.options.input_size)
                        .prefix(Icon::new(IconName::Search).small().text_color(quiet))
                        .when(self.options.escape_hint, |this| {
                            this.suffix(
                                h_flex().children(Keystroke::parse("escape").ok().map(Kbd::new)),
                            )
                        }),
                ),
            )
            .child(
                v_flex()
                    .id("palette-list")
                    .max_h(px(340.))
                    .overflow_y_scroll()
                    .p_1()
                    .children(list)
                    .when(count == 0, |this| {
                        this.child(
                            div()
                                .px_3()
                                .py_4()
                                .text_color(quiet)
                                .child(self.options.empty_text.clone()),
                        )
                    }),
            )
            .when(!footer_hints.is_empty(), |this| {
                this.child(
                    h_flex()
                        .gap_3()
                        .px_3()
                        .py_1p5()
                        .border_t_1()
                        .border_color(line)
                        .text_xs()
                        .text_color(quiet)
                        .children(footer_hints),
                )
            })
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
