// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Agent finder: a multi-agent run browsed like files in a window.
//!
//! Every agent is a folder holding its subagents, and every tool call is a
//! document inside the agent that made it. Folders show as icons or as a
//! list. Opening a folder steps into it, with back, forward and a path bar
//! to step out again, and a document opens in a quick look that shows what
//! the tool was sent and what came back.
//!
//! Keys: the arrows move between items, Enter opens, Space shows or hides
//! the quick look, Backspace goes up, and Escape closes the quick look. In
//! the list, Right and Left open and close a folder in place.

use std::{cell::Cell, collections::HashSet, rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::base::{Presence, PressableExt as _, transition};
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, Icon, StyledExt as _, h_flex, look, v_flex};
use isonomic_ui::{
    AnyElement, App, ClickEvent, Context, ElementId, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Pixels,
    Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
    Toggled, Window, canvas, div, percentage, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{ease, ease_out, over};

/// Width of one icon in the icon view, gap included.
const CELL: f32 = 104.;

/// Where an agent stands. Each status but idle is a coloured tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinderStatus {
    /// At work.
    Running,
    /// Waiting on someone or something.
    Waiting,
    /// Stopped with an error.
    Failed,
    /// Finished.
    Done,
    /// Not started, or nothing to say.
    #[default]
    Idle,
}

impl FinderStatus {
    fn word(self) -> &'static str {
        match self {
            FinderStatus::Running => "Running",
            FinderStatus::Waiting => "Waiting",
            FinderStatus::Failed => "Failed",
            FinderStatus::Done => "Done",
            FinderStatus::Idle => "Idle",
        }
    }

    fn color(self, cx: &App) -> Option<Hsla> {
        let theme = cx.theme();
        match self {
            FinderStatus::Running => Some(theme.info),
            FinderStatus::Waiting => Some(theme.warning),
            FinderStatus::Failed => Some(theme.danger),
            FinderStatus::Done => Some(theme.success),
            FinderStatus::Idle => None,
        }
    }
}

/// One tool call, shown as a document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FinderFile {
    /// Unique in the whole run.
    pub id: SharedString,
    /// The file name, such as "staging-login.log".
    pub name: SharedString,
    /// What kind of document it is. Defaults to one read off the extension.
    pub kind: Option<SharedString>,
    /// The tool that was called.
    pub tool: Option<SharedString>,
    /// How big the result is, such as "212 KB".
    pub size: Option<SharedString>,
    /// How long the call took, such as "2.1 s".
    pub duration: Option<SharedString>,
    /// The call failed.
    pub failed: bool,
    /// What the tool was sent.
    pub request: Option<SharedString>,
    /// What came back.
    pub response: Option<SharedString>,
}

impl FinderFile {
    /// A document with only a name.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Self::default()
        }
    }

    /// Set the tool that was called.
    pub fn tool(mut self, tool: impl Into<SharedString>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    /// Set how big the result is.
    pub fn size(mut self, size: impl Into<SharedString>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Set how long the call took.
    pub fn duration(mut self, duration: impl Into<SharedString>) -> Self {
        self.duration = Some(duration.into());
        self
    }

    /// Set what the tool was sent and what came back.
    pub fn exchange(
        mut self,
        request: impl Into<SharedString>,
        response: impl Into<SharedString>,
    ) -> Self {
        self.request = Some(request.into());
        self.response = Some(response.into());
        self
    }

    /// Mark the call as failed.
    pub fn failed(mut self, failed: bool) -> Self {
        self.failed = failed;
        self
    }

    /// Name the kind of document instead of reading it off the extension.
    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    fn extension(&self) -> SharedString {
        self.name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_uppercase().into())
            .unwrap_or_default()
    }

    fn kind_word(&self) -> SharedString {
        self.kind.clone().unwrap_or_else(|| kind_for(&self.name))
    }
}

/// The kind of document a file name stands for, read off its extension.
pub fn kind_for(name: &str) -> SharedString {
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match ext.as_deref() {
        Some("pdf") => "PDF document".into(),
        Some("json") => "JSON file".into(),
        Some("log") => "Log file".into(),
        Some("md") => "Markdown document".into(),
        Some("txt") => "Plain text".into(),
        Some("csv") => "CSV file".into(),
        Some("html") => "Web page".into(),
        Some("png" | "jpg" | "jpeg") => "Image".into(),
        Some(ext) => format!("{} file", ext.to_uppercase()).into(),
        None => "Document".into(),
    }
}

/// One agent, shown as a folder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FinderAgent {
    /// Unique in the whole run.
    pub id: SharedString,
    /// The agent's name.
    pub name: SharedString,
    /// Where it stands.
    pub status: FinderStatus,
    /// The model it runs on.
    pub model: Option<SharedString>,
    /// What it was asked to do.
    pub task: Option<SharedString>,
    /// Tokens it used.
    pub tokens: Option<u64>,
    /// How far along a running agent is, 0 to 1. Without it the bar sweeps.
    pub progress: Option<f32>,
    /// Its subagents.
    pub children: Vec<FinderAgent>,
    /// Its tool calls.
    pub files: Vec<FinderFile>,
}

impl FinderAgent {
    /// An idle agent with nothing inside.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Self::default()
        }
    }

    /// Set where it stands.
    pub fn status(mut self, status: FinderStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the model it runs on.
    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set what it was asked to do.
    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Set the tokens it used.
    pub fn tokens(mut self, tokens: u64) -> Self {
        self.tokens = Some(tokens);
        self
    }

    /// Set how far along it is, 0 to 1.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress.clamp(0., 1.));
        self
    }

    /// Add a subagent.
    pub fn child(mut self, child: FinderAgent) -> Self {
        self.children.push(child);
        self
    }

    /// Add a tool call.
    pub fn file(mut self, file: FinderFile) -> Self {
        self.files.push(file);
        self
    }
}

/// How the items of a folder are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinderView {
    /// A grid of big icons.
    #[default]
    Icons,
    /// Rows with columns, folders opening in place.
    List,
}

/// An item in a folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FinderItem {
    /// An agent's folder, by id.
    Agent(SharedString),
    /// A document, by id.
    File(SharedString),
}

/// What the finder reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinderEvent {
    /// The selection changed.
    Select(Option<FinderItem>),
    /// An agent's folder was opened.
    Open(SharedString),
    /// A document was opened: its id and the agent it belongs to.
    OpenFile(SharedString, SharedString),
}

fn find<'a>(agents: &'a [FinderAgent], id: &str) -> Option<&'a FinderAgent> {
    agents.iter().find_map(|agent| {
        if agent.id == id {
            Some(agent)
        } else {
            find(&agent.children, id)
        }
    })
}

fn find_file<'a>(agents: &'a [FinderAgent], id: &str) -> Option<(&'a FinderFile, &'a FinderAgent)> {
    agents.iter().find_map(|agent| {
        agent
            .files
            .iter()
            .find(|file| file.id == id)
            .map(|file| (file, agent))
            .or_else(|| find_file(&agent.children, id))
    })
}

/// "2 subagents, 1 document": what an agent's folder holds.
pub fn holds(agent: &FinderAgent) -> String {
    let plural = |n: usize, one: &str, many: &str| match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    };
    let mut parts = Vec::new();
    if !agent.children.is_empty() {
        parts.push(plural(agent.children.len(), "subagent", "subagents"));
    }
    if !agent.files.is_empty() {
        parts.push(plural(agent.files.len(), "document", "documents"));
    }
    if parts.is_empty() {
        "Empty".into()
    } else {
        parts.join(", ")
    }
}

/// "6 items, 2 running": the path bar's count for a folder's items.
pub fn count_line(agents: &[FinderAgent], files: usize) -> String {
    let items = agents.len() + files;
    let running = agents
        .iter()
        .filter(|agent| agent.status == FinderStatus::Running)
        .count();
    let items = match items {
        1 => "1 item".to_string(),
        n => format!("{n} items"),
    };
    match running {
        0 => items,
        n => format!("{items}, {n} running"),
    }
}

/// What a screen reader hears for an agent: "Writer, Waiting, 2 subagents".
pub fn agent_name(agent: &FinderAgent) -> String {
    let holds = holds(agent);
    let mut parts = vec![agent.name.to_string()];
    if agent.status != FinderStatus::Idle {
        parts.push(agent.status.word().into());
    }
    if holds != "Empty" {
        parts.push(holds);
    }
    parts.join(", ")
}

/// What a screen reader hears for a document: "staging-login.log, Log file, failed".
pub fn file_name(file: &FinderFile) -> String {
    let mut parts = vec![file.name.to_string(), file.kind_word().to_string()];
    if file.failed {
        parts.push("failed".into());
    }
    parts.join(", ")
}

/// Token counts with thousands separators, shortened from ten million up.
pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000_000 {
        format!("{:.1}B", tokens as f64 / 1e9)
    } else if tokens >= 10_000_000 {
        format!("{:.1}M", tokens as f64 / 1e6)
    } else {
        let digits = tokens.to_string();
        let mut out = String::new();
        for (ix, ch) in digits.chars().enumerate() {
            if ix > 0 && (digits.len() - ix) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        out
    }
}

/// Where an arrow key moves in a grid of `count` items laid out `columns` to a
/// row, from `at`.
pub fn grid_step(at: Option<usize>, count: usize, columns: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let columns = columns.max(1);
    let Some(at) = at else {
        return Some(0);
    };
    let next = match key {
        "left" => at.checked_sub(1)?,
        "right" => at + 1,
        "up" => at.checked_sub(columns)?,
        "down" => at + columns,
        "home" => 0,
        "end" => count - 1,
        _ => return None,
    };
    (next < count).then_some(next)
}

/// One row of the list: an item and how deep it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ListRow {
    item: FinderItem,
    depth: usize,
    /// The agent the item sits in, if any.
    parent: Option<SharedString>,
}

fn list_rows(
    agents: &[FinderAgent],
    files: &[FinderFile],
    open: &HashSet<SharedString>,
    depth: usize,
    parent: Option<&SharedString>,
    out: &mut Vec<ListRow>,
) {
    for agent in agents {
        out.push(ListRow {
            item: FinderItem::Agent(agent.id.clone()),
            depth,
            parent: parent.cloned(),
        });
        if open.contains(&agent.id) {
            list_rows(
                &agent.children,
                &agent.files,
                open,
                depth + 1,
                Some(&agent.id),
                out,
            );
        }
    }
    for file in files {
        out.push(ListRow {
            item: FinderItem::File(file.id.clone()),
            depth,
            parent: parent.cloned(),
        });
    }
}

/// A multi-agent run, browsed like files.
pub struct AgentFinder {
    title: SharedString,
    subtitle: Option<SharedString>,
    agents: Vec<FinderAgent>,
    view: FinderView,
    /// The folders from the top down to the one showing.
    path: Vec<SharedString>,
    back: Vec<Vec<SharedString>>,
    forward: Vec<Vec<SharedString>>,
    selected: Option<FinderItem>,
    /// Folders open in place in the list.
    open_rows: HashSet<SharedString>,
    quick_look: Option<SharedString>,
    focus: FocusHandle,
    /// The icon view's width at the last paint, for the arrow keys.
    width: Rc<Cell<Pixels>>,
}

impl EventEmitter<FinderEvent> for AgentFinder {}

impl Focusable for AgentFinder {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl AgentFinder {
    /// Create a finder at the top of a run.
    pub fn new(
        title: impl Into<SharedString>,
        agents: Vec<FinderAgent>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            agents,
            view: FinderView::Icons,
            path: Vec::new(),
            back: Vec::new(),
            forward: Vec::new(),
            selected: None,
            open_rows: HashSet::new(),
            quick_look: None,
            focus: cx.focus_handle(),
            width: Rc::new(Cell::new(px(CELL * 5.))),
        }
    }

    /// Set the line under the title at the top, such as when the run started.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Start in this view.
    pub fn view(mut self, view: FinderView) -> Self {
        self.view = view;
        self
    }

    /// Replace the run's agents, keeping where the finder is if it still exists.
    pub fn set_agents(&mut self, agents: Vec<FinderAgent>, cx: &mut Context<Self>) {
        self.agents = agents;
        let depth = self
            .path
            .iter()
            .take_while(|id| find(&self.agents, id).is_some())
            .count();
        self.path.truncate(depth);
        cx.notify();
    }

    /// The current selection.
    pub fn selected(&self) -> Option<&FinderItem> {
        self.selected.as_ref()
    }

    /// The folder showing: its agents and documents.
    fn here(&self) -> (&[FinderAgent], &[FinderFile]) {
        match self.path.last().and_then(|id| find(&self.agents, id)) {
            Some(agent) => (&agent.children, &agent.files),
            None => (&self.agents, &[]),
        }
    }

    fn items(&self) -> Vec<FinderItem> {
        match self.view {
            FinderView::Icons => {
                let (agents, files) = self.here();
                agents
                    .iter()
                    .map(|agent| FinderItem::Agent(agent.id.clone()))
                    .chain(files.iter().map(|file| FinderItem::File(file.id.clone())))
                    .collect()
            }
            FinderView::List => self.rows().into_iter().map(|row| row.item).collect(),
        }
    }

    fn rows(&self) -> Vec<ListRow> {
        let (agents, files) = self.here();
        let mut rows = Vec::new();
        list_rows(agents, files, &self.open_rows, 0, None, &mut rows);
        rows
    }

    fn select(&mut self, item: Option<FinderItem>, cx: &mut Context<Self>) {
        if self.selected != item {
            self.selected = item.clone();
            if self.quick_look.is_some() {
                self.quick_look = match &item {
                    Some(FinderItem::File(id)) => Some(id.clone()),
                    _ => None,
                };
            }
            cx.emit(FinderEvent::Select(item));
        }
        cx.notify();
    }

    fn go(&mut self, path: Vec<SharedString>, cx: &mut Context<Self>) {
        if path == self.path {
            return;
        }
        let from = std::mem::replace(&mut self.path, path);
        self.back.push(from);
        self.forward.clear();
        self.arrive(cx);
    }

    fn arrive(&mut self, cx: &mut Context<Self>) {
        self.quick_look = None;
        self.open_rows.clear();
        self.selected = None;
        cx.emit(FinderEvent::Select(None));
        cx.notify();
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.back.pop() {
            let from = std::mem::replace(&mut self.path, path);
            self.forward.push(from);
            self.arrive(cx);
        }
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.forward.pop() {
            let from = std::mem::replace(&mut self.path, path);
            self.back.push(from);
            self.arrive(cx);
        }
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        if self.path.is_empty() {
            return;
        }
        let left = self.path.last().cloned();
        let mut path = self.path.clone();
        path.pop();
        self.go(path, cx);
        if let Some(left) = left {
            self.select(Some(FinderItem::Agent(left)), cx);
        }
    }

    /// The path down to an agent, found from the top of the run.
    fn path_to(&self, id: &SharedString) -> Vec<SharedString> {
        fn walk(agents: &[FinderAgent], id: &str, out: &mut Vec<SharedString>) -> bool {
            for agent in agents {
                out.push(agent.id.clone());
                if agent.id == id || walk(&agent.children, id, out) {
                    return true;
                }
                out.pop();
            }
            false
        }
        let mut out = Vec::new();
        walk(&self.agents, id, &mut out);
        out
    }

    fn open(&mut self, item: &FinderItem, cx: &mut Context<Self>) {
        match item {
            FinderItem::Agent(id) => {
                let path = self.path_to(id);
                self.go(path, cx);
                cx.emit(FinderEvent::Open(id.clone()));
                // Like a file window, the first item inside starts selected.
                let first = self.items().into_iter().next();
                self.select(first, cx);
            }
            FinderItem::File(id) => {
                if let Some((_, agent)) = find_file(&self.agents, id) {
                    cx.emit(FinderEvent::OpenFile(id.clone(), agent.id.clone()));
                }
                self.quick_look = Some(id.clone());
                cx.notify();
            }
        }
    }

    fn toggle_quick_look(&mut self, cx: &mut Context<Self>) {
        self.quick_look = match (&self.quick_look, &self.selected) {
            (Some(_), _) => None,
            (None, Some(FinderItem::File(id))) => Some(id.clone()),
            _ => None,
        };
        cx.notify();
    }

    fn set_view(&mut self, view: FinderView, cx: &mut Context<Self>) {
        self.view = view;
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let command = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        if self.quick_look.is_some() && key == "escape" {
            self.quick_look = None;
            cx.notify();
            cx.stop_propagation();
            return;
        }
        match key {
            "[" if command => self.go_back(cx),
            "]" if command => self.go_forward(cx),
            "up" if command => self.go_up(cx),
            "backspace" => self.go_up(cx),
            "space" => self.toggle_quick_look(cx),
            "enter" => {
                if let Some(item) = self.selected.clone() {
                    self.open(&item, cx);
                }
            }
            "left" | "right" if self.view == FinderView::List => {
                let rows = self.rows();
                let at = rows
                    .iter()
                    .position(|row| Some(&row.item) == self.selected.as_ref());
                let Some(row) = at.map(|ix| rows[ix].clone()) else {
                    self.select(rows.first().map(|row| row.item.clone()), cx);
                    cx.stop_propagation();
                    return;
                };
                match (&row.item, key) {
                    (FinderItem::Agent(id), "right") => {
                        self.open_rows.insert(id.clone());
                    }
                    (FinderItem::Agent(id), "left") if self.open_rows.contains(id) => {
                        self.open_rows.remove(id);
                    }
                    (_, "left") => {
                        if let Some(parent) = row.parent {
                            self.select(Some(FinderItem::Agent(parent)), cx);
                        }
                    }
                    _ => {}
                }
                cx.notify();
            }
            "left" | "right" | "up" | "down" | "home" | "end" => {
                let items = self.items();
                let columns = match self.view {
                    FinderView::Icons => (f32::from(self.width.get()) / CELL).floor() as usize,
                    FinderView::List => 1,
                };
                let key = match (self.view, key) {
                    (FinderView::List, "left" | "right") => return,
                    (_, key) => key,
                };
                let at = items
                    .iter()
                    .position(|item| Some(item) == self.selected.as_ref());
                if let Some(next) = grid_step(at, items.len(), columns, key) {
                    self.select(Some(items[next].clone()), cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

/// A folder drawn in the status colour: a tab behind, a body and a flap.
fn folder_art(size: f32, cx: &App) -> impl IntoElement {
    let blue = cx.theme().info;
    let unit = size / 64.;
    div()
        .relative()
        .flex_none()
        .w(px(64. * unit))
        .h(px(50. * unit))
        .child(
            div()
                .absolute()
                .top_0()
                .left(px(2. * unit))
                .w(px(26. * unit))
                .h(px(12. * unit))
                .rounded_t(px(3. * unit))
                .bg(shade(blue, 0.2)),
        )
        .child(
            div()
                .absolute()
                .top(px(5. * unit))
                .left_0()
                .right_0()
                .bottom_0()
                .rounded(px(5. * unit))
                .bg(shade(blue, 0.1)),
        )
        .child(
            div()
                .absolute()
                .top(px(11. * unit))
                .left_0()
                .right_0()
                .bottom_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5. * unit))
                .bg(blue)
                .text_color(shade(blue, 0.38))
                .when(size >= 32., |this| {
                    this.child(Icon::new(AssetIcon::Bot).size(px(16. * unit)))
                }),
        )
}

/// A page with its type written at the foot.
fn document_art(file: &FinderFile, size: f32, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let unit = size / 64.;
    let page = if theme.is_dark() {
        look::mix(theme.background, theme.foreground, 0.82)
    } else {
        theme.background
    };
    let ink = if file.failed {
        theme.danger
    } else {
        shade(page, 0.55)
    };
    div()
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .w(px(46. * unit))
        .h(px(60. * unit))
        .pt(px(14. * unit))
        .pb(px(6. * unit))
        .rounded(px(3. * unit))
        .bg(page)
        .shadow(crate::common::resting_shadow(cx))
        .text_color(ink)
        .when(size >= 32., |this| {
            this.child(Icon::new(AssetIcon::DocumentBlank).size(px(18. * unit)))
                .child(
                    div()
                        .text_size(px(7. * unit))
                        .line_height(px(8. * unit))
                        .child(file.extension()),
                )
        })
}

/// `color` darkened by `amount` of its lightness, keeping its hue.
fn shade(color: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: color.l * (1. - amount),
        ..color
    }
}

/// The thin bar under a running agent: its share done, or a sweep.
fn progress_bar(id: ElementId, progress: Option<f32>, width: f32, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let track = theme.foreground.opacity(0.14);
    let fill = theme.info;
    match progress {
        Some(done) => div()
            .w(px(width))
            .h(px(4.))
            .rounded(px(2.))
            .bg(track)
            .overflow_hidden()
            .child(div().h_full().w(relative(done)).rounded(px(2.)).bg(fill))
            .into_any_element(),
        None => {
            use isonomic_ui::{Animation, AnimationExt as _};
            div()
                .relative()
                .w(px(width))
                .h(px(4.))
                .rounded(px(2.))
                .bg(track)
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .h_full()
                        .w(px(width * 0.4))
                        .rounded(px(2.))
                        .bg(fill)
                        .with_animation(
                            id,
                            Animation::new(Duration::from_millis(1400))
                                .repeat()
                                .with_easing(|t| {
                                    // ease-in-out
                                    if t < 0.5 {
                                        2. * t * t
                                    } else {
                                        1. - (-2. * t + 2.).powi(2) / 2.
                                    }
                                }),
                            move |this, t| this.left(px(width * 0.4 * (-1. + 3.5 * t))),
                        ),
                )
                .into_any_element()
        }
    }
}

fn tag(status: FinderStatus, ring: Option<Hsla>, cx: &App) -> Option<impl IntoElement> {
    let color = status.color(cx)?;
    Some(
        div()
            .flex_none()
            .size(px(6.))
            .rounded_full()
            .bg(color)
            .when_some(ring, |this, ring| this.border_1().border_color(ring)),
    )
}

impl AgentFinder {
    fn item_click(
        &self,
        item: FinderItem,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        let focus = self.focus.clone();
        move |event, window, cx| {
            focus.focus(window, cx);
            let item = item.clone();
            let twice = event.click_count() >= 2;
            _ = this.update(cx, |this, cx| {
                this.select(Some(item.clone()), cx);
                if twice {
                    this.open(&item, cx);
                }
            });
        }
    }

    fn render_icons(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let ink = theme.foreground;
        let on = theme.info;
        let on_ink = theme.info_foreground;
        let wash = look::ink::wash(cx);
        let is_focused = self.focus.is_focused(window);
        let (agents, files) = self.here();
        let folder_key: SharedString = self.path.last().cloned().unwrap_or_else(|| "top".into());

        let mut cells: Vec<AnyElement> = Vec::new();
        let entries: Vec<(FinderItem, SharedString, String)> = agents
            .iter()
            .map(|agent| {
                (
                    FinderItem::Agent(agent.id.clone()),
                    agent.name.clone(),
                    agent_name(agent),
                )
            })
            .chain(files.iter().map(|file| {
                (
                    FinderItem::File(file.id.clone()),
                    file.name.clone(),
                    file_name(file),
                )
            }))
            .collect();
        for (ix, (item, name, label)) in entries.into_iter().enumerate() {
            let key = match &item {
                FinderItem::Agent(id) | FinderItem::File(id) => id.clone(),
            };
            let selected = self.selected.as_ref() == Some(&item);
            let entered = Presence::new(
                ElementId::Name(format!("finder-pop-{folder_key}-{key}").into()),
                true,
            )
            .transition(over(300, ease_out()).delay(Duration::from_millis(24 * ix.min(20) as u64)))
            .sample(window, cx)
            .progress;
            let (art, status, failed): (AnyElement, FinderStatus, bool) = match &item {
                FinderItem::Agent(id) => {
                    let agent = find(&self.agents, id).expect("listed agent exists");
                    let running = agent.status == FinderStatus::Running;
                    (
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_full()
                            .child(folder_art(64., cx))
                            .when(running, |this| {
                                this.child(div().absolute().bottom(px(3.)).left(px(12.)).child(
                                    progress_bar(
                                        ElementId::Name(format!("finder-bar-{key}").into()),
                                        agent.progress,
                                        52.,
                                        cx,
                                    ),
                                ))
                            })
                            .into_any_element(),
                        agent.status,
                        false,
                    )
                }
                FinderItem::File(id) => {
                    let (file, _) = find_file(&self.agents, id).expect("listed file exists");
                    (
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_full()
                            .child(document_art(file, 64., cx))
                            .into_any_element(),
                        FinderStatus::Idle,
                        file.failed,
                    )
                }
            };
            cells.push(
                v_flex()
                    .id(ElementId::Name(format!("finder-icon-{key}").into()))
                    .test_support()
                    .role(Role::ListBoxOption)
                    .aria_label(label)
                    .aria_selected(selected)
                    .w(px(CELL))
                    .items_center()
                    .gap(px(4.))
                    .pt(px(4.))
                    .px(px(2.))
                    .pb(px(6.))
                    .rounded(px(8.))
                    .opacity(entered)
                    .on_click(self.item_click(item.clone(), cx))
                    .child(
                        div()
                            .size(px(76.))
                            .rounded(px(8.))
                            .when(selected, |this| this.bg(wash))
                            .when(selected && is_focused, |this| {
                                this.border_2().border_color(theme.ring)
                            })
                            .child(art),
                    )
                    .child(
                        h_flex()
                            .max_w_full()
                            .py(px(1.))
                            .px(px(5.))
                            .rounded(px(4.))
                            .text_size(px(12.))
                            .line_height(px(15.))
                            .text_color(if failed { theme.danger } else { ink })
                            .when(selected, |this| this.bg(on).text_color(on_ink))
                            .children(
                                tag(status, selected.then_some(on_ink), cx)
                                    .map(|dot| div().mr(px(4.)).child(dot)),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .px(px(1.5))
                                    .text_center()
                                    .line_clamp(2)
                                    .child(name),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let width = self.width.clone();
        div()
            .relative()
            .size_full()
            .child(
                canvas(
                    move |bounds, _, _| width.set(bounds.size.width),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                h_flex()
                    .id("finder-icons")
                    .role(Role::ListBox)
                    .aria_label(self.heading())
                    .flex_wrap()
                    .items_start()
                    .content_start()
                    .pt(px(14.))
                    .px(px(12.))
                    .gap_y_2p5()
                    .children(cells),
            )
            .into_any_element()
    }

    fn render_list(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let ink = theme.foreground;
        let text = look::ink::muted(cx);
        let quiet = theme.muted_foreground;
        let on = theme.info;
        let on_ink = theme.info_foreground;
        let zebra = theme
            .foreground
            .opacity(if theme.is_dark() { 0.035 } else { 0.028 });
        let hair = look::ink::hairline(cx);
        let is_focused = self.focus.is_focused(window);
        let rows = self.rows();

        let head_cell = |label: &'static str, first: bool, right: bool| {
            div()
                .px(px(14.))
                .when(first, |this| this.pl(px(28.)))
                .when(!first, |this| this.border_l(px(0.5)).border_color(hair))
                .when(right, |this| this.text_right())
                .truncate()
                .child(label)
        };
        let head = h_flex()
            .h(px(26.))
            .px(px(10.))
            .border_b(px(0.5))
            .border_color(look::ink::hairline(cx))
            .text_size(px(11.))
            .font_medium()
            .text_color(text)
            .child(head_cell("Name", true, false).flex_basis(relative(0.43)))
            .child(head_cell("Status", false, false).flex_basis(relative(0.16)))
            .child(head_cell("Model", false, false).flex_basis(relative(0.25)))
            .child(head_cell("Tokens", false, true).flex_basis(relative(0.16)));

        let mut lines: Vec<AnyElement> = Vec::new();
        for (ix, row) in rows.iter().enumerate() {
            let selected = self.selected.as_ref() == Some(&row.item);
            let (name, status_word, model, amount, status, folder, open, label, failed) =
                match &row.item {
                    FinderItem::Agent(id) => {
                        let agent = find(&self.agents, id).expect("listed agent exists");
                        (
                            agent.name.clone(),
                            SharedString::from(agent.status.word()),
                            agent.model.clone(),
                            agent.tokens.map(|t| SharedString::from(format_tokens(t))),
                            agent.status,
                            true,
                            self.open_rows.contains(id),
                            agent_name(agent),
                            false,
                        )
                    }
                    FinderItem::File(id) => {
                        let (file, _) = find_file(&self.agents, id).expect("listed file exists");
                        (
                            file.name.clone(),
                            file.kind_word(),
                            file.tool.clone(),
                            file.size.clone(),
                            FinderStatus::Idle,
                            false,
                            false,
                            file_name(file),
                            file.failed,
                        )
                    }
                };
            let key = match &row.item {
                FinderItem::Agent(id) | FinderItem::File(id) => id.clone(),
            };
            let turn = transition(
                ElementId::Name(format!("finder-tri-{key}").into()),
                if open { 0.25_f32 } else { 0. },
                over(160, ease_out()),
                window,
                cx,
            );
            let cell_ink = if selected { on_ink } else { text };
            let toggle = folder.then(|| {
                let this = cx.entity().downgrade();
                let key = key.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    _ = this.update(cx, |this, cx| {
                        if !this.open_rows.remove(&key) {
                            this.open_rows.insert(key.clone());
                        }
                        cx.notify();
                    });
                }
            });
            lines.push(
                h_flex()
                    .id(ElementId::Name(format!("finder-row-{key}").into()))
                    .test_support()
                    .role(Role::TreeItem)
                    .aria_label(label)
                    .aria_selected(selected)
                    .when(folder, |this| this.aria_expanded(open))
                    .h(px(24.))
                    .mx(px(10.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .when(ix % 2 == 1, |this| this.bg(zebra))
                    .when(selected, |this| this.bg(on))
                    .when(selected && is_focused, |this| {
                        this.border_2().border_color(theme.ring)
                    })
                    .on_click(self.item_click(row.item.clone(), cx))
                    .child(
                        h_flex()
                            .flex_basis(relative(0.43))
                            .min_w_0()
                            .gap(px(6.))
                            .pl(px(14. + 16. * row.depth as f32))
                            .pr(px(14.))
                            .text_color(if selected {
                                on_ink
                            } else if failed {
                                theme.danger
                            } else {
                                ink
                            })
                            .child(
                                div()
                                    .id(ElementId::Name(format!("finder-disclose-{key}").into()))
                                    .flex_none()
                                    .size(px(14.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(if selected { on_ink } else { quiet })
                                    .when_some(toggle, |this, toggle| {
                                        this.pressable(true)
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(toggle)
                                            .child(
                                                Icon::new(AssetIcon::CaretRight)
                                                    .size(px(12.))
                                                    .rotate(percentage(turn)),
                                            )
                                    }),
                            )
                            .child(div().flex_none().w(px(16.)).flex().justify_center().map(
                                |this| match &row.item {
                                    FinderItem::Agent(_) => this.child(folder_art(16., cx)),
                                    FinderItem::File(id) => {
                                        let (file, _) = find_file(&self.agents, id)
                                            .expect("listed file exists");
                                        this.child(document_art(file, 16., cx))
                                    }
                                },
                            ))
                            .child(div().min_w_0().truncate().child(name))
                            .children(tag(status, selected.then_some(on_ink), cx)),
                    )
                    .child(
                        div()
                            .flex_basis(relative(0.16))
                            .min_w_0()
                            .px(px(14.))
                            .truncate()
                            .text_color(cell_ink)
                            .child(status_word),
                    )
                    .child(
                        div()
                            .flex_basis(relative(0.25))
                            .min_w_0()
                            .px(px(14.))
                            .truncate()
                            .text_color(cell_ink)
                            .children(model),
                    )
                    .child(
                        div()
                            .flex_basis(relative(0.16))
                            .min_w_0()
                            .px(px(14.))
                            .text_right()
                            .truncate()
                            .text_color(cell_ink)
                            .children(amount),
                    )
                    .into_any_element(),
            );
        }
        v_flex()
            .size_full()
            .child(head)
            .child(
                v_flex()
                    .id("finder-list")
                    .role(Role::Tree)
                    .aria_label(self.heading())
                    .pb(px(8.))
                    .children(lines),
            )
            .into_any_element()
    }

    fn heading(&self) -> SharedString {
        match self.path.last().and_then(|id| find(&self.agents, id)) {
            Some(agent) => agent.name.clone(),
            None => self.title.clone(),
        }
    }

    fn render_quick_look(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let shown = Presence::new("finder-quick-look", self.quick_look.is_some())
            .transition(over(260, ease_out()))
            .sample(window, cx);
        if !shown.should_render() {
            return None;
        }
        let id = self.quick_look.clone()?;
        let (file, _) = find_file(&self.agents, &id)?;
        let theme = cx.theme().clone();
        let quiet = theme.muted_foreground;
        let line = look::ink::hairline(cx);
        let code = look::ink::wash(cx);
        let mono = theme.mono_font_family.clone();
        let section = |head: &'static str, body: SharedString, bad: bool| {
            v_flex()
                .child(
                    div()
                        .mb(px(6.))
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .font_semibold()
                        .text_color(quiet)
                        .child(head),
                )
                .child(
                    div()
                        .py(px(10.))
                        .px(px(12.))
                        .rounded(px(8.))
                        .bg(if bad { theme.danger.opacity(0.1) } else { code })
                        .text_color(if bad { theme.danger } else { theme.foreground })
                        .font_family(mono.clone())
                        .text_size(px(11.5))
                        .line_height(px(17.))
                        .child(body),
                )
        };
        let sub = [Some(file.kind_word()), file.size.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        let close = {
            let this = cx.entity().downgrade();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                _ = this.update(cx, |this, cx| {
                    this.quick_look = None;
                    cx.notify();
                });
            }
        };
        let empty = file.request.is_none() && file.response.is_none();
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(20.))
                .bg(theme.overlay)
                .opacity(shown.progress)
                .child(
                    v_flex()
                        .id("finder-quick-look-card")
                        .test_support()
                        .role(Role::Dialog)
                        .aria_label(format!("Quick look: {}", file.name))
                        .w(px(480.))
                        .max_w_full()
                        .max_h_full()
                        .overflow_hidden()
                        .rounded(px(12.))
                        .bg(theme.popover)
                        .shadow(look::surface::floating(cx))
                        .mt(px(12. * (1. - shown.progress)))
                        .child(
                            h_flex()
                                .flex_none()
                                .gap(px(8.))
                                .h(px(46.))
                                .pl(px(14.))
                                .pr(px(8.))
                                .border_b(px(0.5))
                                .border_color(line)
                                .child(document_art(file, 16., cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(13.))
                                                .line_height(px(16.))
                                                .font_semibold()
                                                .child(file.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(11.))
                                                .line_height(px(14.))
                                                .text_color(quiet)
                                                .child(sub),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("finder-quick-look-close")
                                        .role(Role::Button)
                                        .aria_label("Close")
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(28.))
                                        .rounded(px(6.))
                                        .text_color(look::ink::muted(cx))
                                        .hover(|this| this.bg(look::ink::wash(cx)))
                                        .pressable(true)
                                        .on_click(close)
                                        .child(Icon::new(AssetIcon::Close).size(px(16.))),
                                ),
                        )
                        .child(
                            v_flex()
                                .id("finder-quick-look-body")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .gap(px(14.))
                                .p(px(14.))
                                .when_some(file.request.clone(), |this, request| {
                                    this.child(section("Request", request, false))
                                })
                                .when_some(file.response.clone(), |this, response| {
                                    this.child(section("Response", response, file.failed))
                                })
                                .when(empty, |this| {
                                    this.child(
                                        div()
                                            .py(px(18.))
                                            .text_center()
                                            .text_color(look::ink::muted(cx))
                                            .child("Nothing was recorded for this call."),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .gap(px(8.))
                                .min_h(px(36.))
                                .px(px(14.))
                                .border_t(px(0.5))
                                .border_color(line)
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(look::ink::muted(cx))
                                .children(
                                    file.tool
                                        .clone()
                                        .map(|tool| div().font_family(mono.clone()).child(tool)),
                                )
                                .children(
                                    file.duration
                                        .clone()
                                        .map(|d| div().text_color(quiet).child(d)),
                                )
                                .child(
                                    h_flex()
                                        .ml_auto()
                                        .gap(px(5.))
                                        .when(file.failed, |this| this.text_color(theme.danger))
                                        .child(div().size(px(6.)).rounded_full().bg(
                                            if file.failed {
                                                theme.danger
                                            } else {
                                                theme.success
                                            },
                                        ))
                                        .child(if file.failed { "Failed" } else { "Succeeded" }),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for AgentFinder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ink = theme.foreground;
        let text = look::ink::muted(cx);
        let quiet = theme.muted_foreground;
        let line = look::ink::hairline(cx);
        let wash = look::ink::wash(cx);

        let (agents, files) = self.here();
        let count = count_line(agents, files.len());
        let here = self.path.last().and_then(|id| find(&self.agents, id));
        let subtitle: Option<SharedString> = match here {
            Some(agent) => Some(holds(agent).into()),
            None => self.subtitle.clone(),
        };
        let heading = self.heading();

        let tool = |id: &'static str,
                    icon: AssetIcon,
                    label: &'static str,
                    enabled: bool,
                    on_click: Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded(px(6.))
                .text_color(if enabled {
                    text
                } else {
                    theme.foreground.opacity(0.22)
                })
                .when(enabled, |this| {
                    this.hover(|this| this.bg(wash).text_color(ink))
                        .on_click(on_click)
                })
                .pressable(enabled)
                .child(Icon::new(icon).size(px(16.)))
        };
        let entity = cx.entity().downgrade();
        let act = move |f: fn(&mut AgentFinder, &mut Context<AgentFinder>)| {
            let entity = entity.clone();
            Box::new(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                _ = entity.update(cx, |this, cx| f(this, cx));
            }) as Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>
        };

        let view_button =
            |id: &'static str, view: FinderView, icon: AssetIcon, label: &'static str| {
                let on = self.view == view;
                let entity = cx.entity().downgrade();
                div()
                    .id(id)
                    .role(Role::RadioButton)
                    .aria_label(label)
                    .aria_toggled(if on { Toggled::True } else { Toggled::False })
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(30.))
                    .h(px(24.))
                    .rounded(px(5.))
                    .text_color(if on { ink } else { text })
                    .when(on, |this| {
                        this.bg(theme.background).shadow(look::surface::knob(cx))
                    })
                    .pressable(true)
                    .on_click(move |_, _, cx| {
                        _ = entity.update(cx, |this, cx| this.set_view(view, cx));
                    })
                    .child(Icon::new(icon).size(px(16.)))
            };

        let bar = h_flex()
            .flex_none()
            .gap(px(10.))
            .h(px(52.))
            .pl(px(10.))
            .pr(px(12.))
            .border_b(px(0.5))
            .border_color(line)
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(2.))
                    .child(tool(
                        "finder-back",
                        AssetIcon::ChevronLeft,
                        "Back",
                        !self.back.is_empty(),
                        act(AgentFinder::go_back),
                    ))
                    .child(tool(
                        "finder-forward",
                        AssetIcon::ChevronRight,
                        "Forward",
                        !self.forward.is_empty(),
                        act(AgentFinder::go_forward),
                    )),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(15.))
                            .line_height(px(19.))
                            .font_semibold()
                            .text_color(ink)
                            .child(heading),
                    )
                    .children(subtitle.map(|subtitle| {
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .line_height(px(14.))
                            .text_color(quiet)
                            .child(subtitle)
                    })),
            )
            .child(
                h_flex()
                    .id("finder-views")
                    .role(Role::RadioGroup)
                    .aria_label("View")
                    .flex_none()
                    .gap(px(2.))
                    .p(px(2.))
                    .rounded(px(7.))
                    .bg(wash)
                    .child(view_button(
                        "finder-view-icons",
                        FinderView::Icons,
                        AssetIcon::Grid,
                        "Icons",
                    ))
                    .child(view_button(
                        "finder-view-list",
                        FinderView::List,
                        AssetIcon::ListBulleted,
                        "List",
                    )),
            );

        // The path bar: every folder from the top to here.
        let mut crumbs: Vec<AnyElement> = Vec::new();
        let names: Vec<(Vec<SharedString>, SharedString)> =
            std::iter::once((Vec::new(), self.title.clone()))
                .chain(self.path.iter().enumerate().map(|(ix, id)| {
                    let name = find(&self.agents, id)
                        .map(|agent| agent.name.clone())
                        .unwrap_or_default();
                    (self.path[..=ix].to_vec(), name)
                }))
                .collect();
        let last = names.len() - 1;
        for (ix, (path, name)) in names.into_iter().enumerate() {
            if ix > 0 {
                crumbs.push(
                    div()
                        .mx(px(2.))
                        .text_color(quiet)
                        .child(Icon::new(AssetIcon::ChevronRight).size(px(11.)))
                        .into_any_element(),
                );
            }
            let entity = cx.entity().downgrade();
            crumbs.push(
                h_flex()
                    .id(ElementId::Name(format!("finder-crumb-{ix}").into()))
                    .role(Role::Link)
                    .aria_label(name.clone())
                    .min_w_0()
                    .gap(px(4.))
                    .h(px(20.))
                    .px(px(4.))
                    .rounded(px(4.))
                    .text_color(if ix == last { ink } else { text })
                    .hover(|this| this.bg(wash))
                    .pressable(true)
                    .on_click(move |_, _, cx| {
                        let path = path.clone();
                        _ = entity.update(cx, |this, cx| this.go(path, cx));
                    })
                    .child(folder_art(14., cx))
                    .child(div().min_w_0().truncate().child(name))
                    .into_any_element(),
            );
        }
        let pathbar = h_flex()
            .flex_none()
            .gap(px(10.))
            .h(px(26.))
            .px(px(12.))
            .border_t(px(0.5))
            .border_color(line)
            .text_size(px(11.))
            .line_height(px(16.))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .children(crumbs),
            )
            .child(div().flex_none().text_color(quiet).child(count));

        let content = match self.view {
            FinderView::Icons => self.render_icons(window, cx),
            FinderView::List => self.render_list(window, cx),
        };
        let content_key = format!("{}-{:?}", self.path.join("/"), self.view);
        let content_in = Presence::new(
            ElementId::Name(format!("finder-in-{content_key}").into()),
            true,
        )
        .transition(over(260, ease()))
        .sample(window, cx)
        .progress;
        let quick_look = self.render_quick_look(window, cx);

        v_flex()
            .id("agent-finder")
            .test_support()
            .role(Role::Region)
            .aria_label(self.title.clone())
            .key_context("AgentFinder")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .relative()
            .w_full()
            .h(px(440.))
            .overflow_hidden()
            .rounded(px(14.))
            .bg(theme.background)
            .shadow(look::surface::resting(cx))
            .text_color(ink)
            .text_body()
            .child(bar)
            .child(
                div()
                    .id("finder-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .opacity(content_in)
                    .on_click({
                        let entity = cx.entity().downgrade();
                        let focus = self.focus.clone();
                        move |_, window, cx| {
                            focus.focus(window, cx);
                            _ = entity.update(cx, |this, cx| this.select(None, cx));
                        }
                    })
                    .child(content),
            )
            .child(pathbar)
            .children(quick_look)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Vec<FinderAgent> {
        vec![
            FinderAgent::new("planner", "Planner")
                .status(FinderStatus::Running)
                .child(FinderAgent::new("research", "Researcher").status(FinderStatus::Done))
                .child(FinderAgent::new("spec", "Spec Writer").status(FinderStatus::Done))
                .file(FinderFile::new("plan", "Release plan.pdf"))
                .file(FinderFile::new("issue", "BIL-482.json")),
            FinderAgent::new("writer", "Writer")
                .status(FinderStatus::Waiting)
                .child(FinderAgent::new("a", "A"))
                .child(FinderAgent::new("b", "B"))
                .file(FinderFile::new("log", "staging-login.log").failed(true)),
            FinderAgent::new("release", "Release"),
        ]
    }

    #[test]
    fn folders_say_what_they_hold() {
        let agents = run();
        assert_eq!(holds(&agents[0]), "2 subagents, 2 documents");
        assert_eq!(holds(&agents[1]), "2 subagents, 1 document");
        assert_eq!(holds(&agents[2]), "Empty");
    }

    #[test]
    fn items_are_read_with_their_state() {
        let agents = run();
        assert_eq!(
            agent_name(&agents[1]),
            "Writer, Waiting, 2 subagents, 1 document"
        );
        assert_eq!(agent_name(&agents[2]), "Release");
        assert_eq!(
            file_name(&agents[1].files[0]),
            "staging-login.log, Log file, failed"
        );
    }

    #[test]
    fn the_count_names_what_runs() {
        let agents = run();
        assert_eq!(count_line(&agents, 0), "3 items, 1 running");
        assert_eq!(count_line(&agents[0].children, 2), "4 items");
        assert_eq!(count_line(&[], 1), "1 item");
    }

    #[test]
    fn kinds_come_from_the_extension() {
        assert_eq!(kind_for("Release plan.pdf").as_ref(), "PDF document");
        assert_eq!(kind_for("BIL-482.JSON").as_ref(), "JSON file");
        assert_eq!(kind_for("notes").as_ref(), "Document");
        assert_eq!(kind_for("dump.bin").as_ref(), "BIN file");
    }

    #[test]
    fn tokens_read_with_separators_and_shorten_when_huge() {
        assert_eq!(format_tokens(912), "912");
        assert_eq!(format_tokens(18_240), "18,240");
        assert_eq!(format_tokens(1_204_311), "1,204,311");
        assert_eq!(format_tokens(48_000_000), "48.0M");
        assert_eq!(format_tokens(2_400_000_000), "2.4B");
    }

    #[test]
    fn arrows_move_through_the_grid() {
        assert_eq!(grid_step(None, 6, 5, "down"), Some(0));
        assert_eq!(grid_step(Some(0), 6, 5, "right"), Some(1));
        assert_eq!(grid_step(Some(0), 6, 5, "left"), None);
        assert_eq!(grid_step(Some(1), 6, 5, "down"), None);
        assert_eq!(grid_step(Some(0), 6, 5, "down"), Some(5));
        assert_eq!(grid_step(Some(5), 6, 5, "up"), Some(0));
        assert_eq!(grid_step(Some(2), 6, 5, "end"), Some(5));
        assert_eq!(grid_step(Some(2), 0, 5, "end"), None);
    }

    #[test]
    fn the_list_opens_folders_in_place() {
        let agents = run();
        let mut open = HashSet::new();
        let mut rows = Vec::new();
        list_rows(&agents, &[], &open, 0, None, &mut rows);
        assert_eq!(rows.len(), 3);
        open.insert(SharedString::from("planner"));
        rows.clear();
        list_rows(&agents, &[], &open, 0, None, &mut rows);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[1].parent.as_deref(), Some("planner"));
        assert_eq!(rows[3].item, FinderItem::File("plan".into()));
    }
}
