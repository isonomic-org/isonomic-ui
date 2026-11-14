//! Change ledger: everything an agent changed, each with an undo.
//!
//! Every row says what changed and where, with the before and after. Undo is
//! honest about consequences: pointing at it lights up the changes built on
//! this one, which go too, and the ones that cannot be taken back.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

use crate::common::resting_shadow;

/// How often the "2m ago" times redraw.
const TICK: Duration = Duration::from_secs(5);

/// One change made in some system.
#[derive(Clone)]
pub struct LedgerEntry {
    id: SharedString,
    action: SharedString,
    target: Option<SharedString>,
    system: SharedString,
    icon: Option<IconName>,
    at: Instant,
    change: Option<(SharedString, SharedString)>,
    detail: Option<SharedString>,
    reversible: bool,
    depends_on: Vec<SharedString>,
}

impl LedgerEntry {
    /// Create an entry: what was done ("Changed price"), in which system,
    /// and when.
    pub fn new(
        id: impl Into<SharedString>,
        action: impl Into<SharedString>,
        system: impl Into<SharedString>,
        at: Instant,
    ) -> Self {
        Self {
            id: id.into(),
            action: action.into(),
            target: None,
            system: system.into(),
            icon: None,
            at,
            change: None,
            detail: None,
            reversible: true,
            depends_on: Vec::new(),
        }
    }

    /// Name what it was done to ("Pro plan").
    pub fn target(mut self, target: impl Into<SharedString>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Show a mark for the system.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Show what it was before and what it is now.
    pub fn change(
        mut self,
        before: impl Into<SharedString>,
        after: impl Into<SharedString>,
    ) -> Self {
        self.change = Some((before.into(), after.into()));
        self
    }

    /// Show one line about it instead of a before and after.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Mark it as something that cannot be taken back, such as a sent
    /// message or a payout.
    pub fn irreversible(mut self) -> Self {
        self.reversible = false;
        self
    }

    /// Say which change this one builds on. Undoing that one undoes this one
    /// too, if it can be undone.
    pub fn builds_on(mut self, id: impl Into<SharedString>) -> Self {
        self.depends_on.push(id.into());
        self
    }

    /// The entry's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

type IdsHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;

/// Every change an agent made, newest first, with honest undo.
#[derive(IntoElement)]
pub struct ChangeLedger {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    agent: Option<SharedString>,
    entries: Vec<LedgerEntry>,
    undone: Option<Vec<SharedString>>,
    on_undo: Option<IdsHandler>,
    on_redo: Option<IdsHandler>,
}

impl ChangeLedger {
    /// Create an empty ledger.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Ledger".into(),
            agent: None,
            entries: Vec::new(),
            undone: None,
            on_undo: None,
            on_redo: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Name who made the changes.
    pub fn agent(mut self, agent: impl Into<SharedString>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Add a change. Any order: the newest shows first.
    pub fn entry(mut self, entry: LedgerEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Say which changes are undone. Without it the ledger keeps its own.
    pub fn undone(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.undone = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Run on Undo with the change and the changes built on it that can go
    /// too. Reverting them for real is up to you.
    pub fn on_undo(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }

    /// Run on Redo with the change and the undone changes it builds on.
    pub fn on_redo(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_redo = Some(Rc::new(handler));
        self
    }
}

impl Styled for ChangeLedger {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Every change that builds on `id`, directly or through others.
fn built_on(entries: &[LedgerEntry], id: &str) -> Vec<SharedString> {
    let mut found: Vec<SharedString> = Vec::new();
    let mut seen: HashSet<SharedString> = HashSet::new();
    let mut queue = vec![SharedString::from(id.to_string())];
    while let Some(at) = queue.pop() {
        for entry in entries {
            if entry.depends_on.contains(&at) && seen.insert(entry.id.clone()) {
                found.push(entry.id.clone());
                queue.push(entry.id.clone());
            }
        }
    }
    found
}

/// What Undo on `id` would do: the changes it undoes (itself first) and the
/// changes built on it that cannot be undone.
fn undo_plan(
    entries: &[LedgerEntry],
    id: &str,
    undone: &HashSet<SharedString>,
) -> (Vec<SharedString>, Vec<SharedString>) {
    let reversible: HashMap<&str, bool> = entries
        .iter()
        .map(|entry| (entry.id.as_ref(), entry.reversible))
        .collect();
    let mut goes = vec![SharedString::from(id.to_string())];
    let mut kept = Vec::new();
    for dependent in built_on(entries, id) {
        if undone.contains(&dependent) {
            continue;
        }
        if reversible.get(dependent.as_ref()).copied().unwrap_or(true) {
            goes.push(dependent);
        } else {
            kept.push(dependent);
        }
    }
    (goes, kept)
}

/// What Redo on `id` would do: the undone changes it builds on, then itself.
fn redo_plan(
    entries: &[LedgerEntry],
    id: &str,
    undone: &HashSet<SharedString>,
) -> Vec<SharedString> {
    let by_id: HashMap<&str, &LedgerEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_ref(), entry))
        .collect();
    let mut plan: Vec<SharedString> = Vec::new();
    let mut queue = vec![SharedString::from(id.to_string())];
    while let Some(at) = queue.pop() {
        if !undone.contains(&at) || plan.contains(&at) {
            continue;
        }
        plan.insert(0, at.clone());
        if let Some(entry) = by_id.get(at.as_ref()) {
            queue.extend(entry.depends_on.iter().cloned());
        }
    }
    plan
}

/// "just now", "37s ago", "2m ago", "1h ago".
fn ago_words(ago: Duration) -> String {
    let secs = ago.as_secs();
    if secs < 5 {
        "just now".into()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

#[derive(Default)]
struct LedgerUi {
    pointed: Option<SharedString>,
    hovered: Option<SharedString>,
    undone: HashSet<SharedString>,
    ticker: Option<Task<()>>,
}

impl RenderOnce for ChangeLedger {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let warning = theme.warning;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let ui = window.use_keyed_state(self.id.clone(), cx, |_, _| LedgerUi::default());
        ui.update(cx, |ui, cx| {
            if ui.ticker.is_none() {
                ui.ticker = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(TICK).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
            }
        });
        let (pointed, hovered, own_undone) = {
            let ui = ui.read(cx);
            (ui.pointed.clone(), ui.hovered.clone(), ui.undone.clone())
        };
        let undone: HashSet<SharedString> = match &self.undone {
            Some(ids) => ids.iter().cloned().collect(),
            None => own_undone,
        };
        let controlled = self.undone.is_some();

        let mut entries = self.entries;
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.at));
        let entries = Rc::new(entries);
        let (also, kept) = pointed
            .as_ref()
            .map(|id| undo_plan(&entries, id, &undone))
            .map(|(goes, kept)| (goes.into_iter().skip(1).collect::<Vec<_>>(), kept))
            .unwrap_or_default();
        let now = Instant::now();
        let undone_count = entries
            .iter()
            .filter(|entry| undone.contains(&entry.id))
            .count();

        let rows = entries.iter().enumerate().map(|(ix, entry)| {
            let is_undone = undone.contains(&entry.id);
            let is_pointed = pointed.as_ref() == Some(&entry.id);
            let goes_too = also.contains(&entry.id);
            let stays = kept.contains(&entry.id);
            let show_button = hovered.as_ref() == Some(&entry.id) || is_pointed;
            let hover_row = {
                let ui = ui.clone();
                let id = entry.id.clone();
                move |hovered: &bool, _: &mut Window, cx: &mut App| {
                    ui.update(cx, |ui, cx| {
                        if *hovered {
                            ui.hovered = Some(id.clone());
                        } else if ui.hovered.as_ref() == Some(&id) {
                            ui.hovered = None;
                        }
                        cx.notify();
                    })
                }
            };

            let button = if is_undone {
                let ui = ui.clone();
                let entries = entries.clone();
                let undone = undone.clone();
                let on_redo = self.on_redo.clone();
                let id = entry.id.clone();
                Some(
                    Button::new(("ledger-redo", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Redo2)
                        .label("Redo")
                        .on_click(move |_, window, cx| {
                            let plan = redo_plan(&entries, &id, &undone);
                            if !controlled {
                                ui.update(cx, |ui, cx| {
                                    for id in &plan {
                                        ui.undone.remove(id);
                                    }
                                    cx.notify();
                                });
                            }
                            if let Some(handler) = &on_redo {
                                handler(&plan, window, cx);
                            }
                        })
                        .into_any_element(),
                )
            } else if entry.reversible {
                let ui_click = ui.clone();
                let ui_hover = ui.clone();
                let entries = entries.clone();
                let undone = undone.clone();
                let on_undo = self.on_undo.clone();
                let id = entry.id.clone();
                let hover_id = entry.id.clone();
                Some(
                    div()
                        .id(("ledger-undo-wrap", ix))
                        .on_hover(move |hovered: &bool, _, cx| {
                            ui_hover.update(cx, |ui, cx| {
                                if *hovered {
                                    ui.pointed = Some(hover_id.clone());
                                } else if ui.pointed.as_ref() == Some(&hover_id) {
                                    ui.pointed = None;
                                }
                                cx.notify();
                            })
                        })
                        .child(
                            Button::new(("ledger-undo", ix))
                                .outline()
                                .xsmall()
                                .icon(IconName::Undo2)
                                .label("Undo")
                                .on_click(move |_, window, cx| {
                                    let (goes, _) = undo_plan(&entries, &id, &undone);
                                    ui_click.update(cx, |ui, cx| {
                                        if !controlled {
                                            ui.undone.extend(goes.iter().cloned());
                                        }
                                        ui.pointed = None;
                                        cx.notify();
                                    });
                                    if let Some(handler) = &on_undo {
                                        handler(&goes, window, cx);
                                    }
                                }),
                        )
                        .into_any_element(),
                )
            } else {
                None
            };

            let flag = if goes_too {
                Some(("Also undone", accent))
            } else if stays {
                Some(("Can't undo", warning))
            } else {
                None
            };
            let right = match (flag, show_button.then_some(()).and(button)) {
                (Some((words, color)), _) => div()
                    .px_1p5()
                    .rounded(px(4.))
                    .bg(color.opacity(0.14))
                    .text_color(color)
                    .text_xs()
                    .font_medium()
                    .child(words)
                    .into_any_element(),
                (None, Some(button)) => button,
                (None, None) => h_flex()
                    .gap_1()
                    .text_xs()
                    .text_color(quiet)
                    .when(!entry.reversible, |this| {
                        this.child(Icon::new(IconName::Ban).xsmall())
                    })
                    .child(format!(
                        "{}, {}",
                        ago_words(now.saturating_duration_since(entry.at)),
                        entry.system
                    ))
                    .into_any_element(),
            };

            let second = match (&entry.change, &entry.detail) {
                (Some((before, after)), _) => Some(
                    h_flex()
                        .gap_1p5()
                        .text_xs()
                        .text_color(quiet)
                        .child(div().line_through().child(before.clone()))
                        .child(Icon::new(IconName::ArrowRight).xsmall())
                        .child(
                            div()
                                .when(!is_undone, |this| this.text_color(ink))
                                .child(after.clone()),
                        )
                        .into_any_element(),
                ),
                (None, Some(detail)) => Some(
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child(detail.clone())
                        .into_any_element(),
                ),
                _ => None,
            };

            h_flex()
                .id(("ledger-row", ix))
                .gap_3()
                .h(px(56.))
                .px_3()
                .border_b_1()
                .border_color(line)
                .on_hover(hover_row)
                .when(is_pointed, |this| this.bg(accent.opacity(0.06)))
                .when(goes_too, |this| this.bg(accent.opacity(0.06)))
                .when(stays, |this| this.bg(warning.opacity(0.08)))
                .when(is_undone, |this| this.opacity(0.55))
                .child(
                    div()
                        .flex_none()
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .child(
                            Icon::new(entry.icon.clone().unwrap_or(IconName::Settings))
                                .small()
                                .text_color(if is_undone { quiet } else { ink }),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .flex_none()
                                        .font_medium()
                                        .when(is_undone, |this| this.line_through())
                                        .child(entry.action.clone()),
                                )
                                .children(entry.target.clone().map(|target| {
                                    div().min_w_0().truncate().text_color(quiet).child(target)
                                })),
                        )
                        .children(second),
                )
                .child(div().flex_none().child(right))
        });
        let rows: Vec<_> = rows.collect();
        let count = entries.len();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(640.))
            .gap_2()
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.3))
            .child(
                h_flex()
                    .gap_2()
                    .px_1()
                    .child(Icon::new(IconName::FileText).small().text_color(accent))
                    .child(div().font_medium().child(self.title))
                    .children(self.agent.map(|agent| div().text_color(quiet).child(agent)))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(quiet)
                            .child(match (count, undone_count) {
                                (1, 0) => "1 change".to_string(),
                                (count, 0) => format!("{count} changes"),
                                (count, undone) => format!("{count} changes, {undone} undone"),
                            }),
                    ),
            )
            .child(
                v_flex()
                    .rounded(radius_lg)
                    .border_1()
                    .border_color(line)
                    .bg(fill)
                    .overflow_hidden()
                    .shadow(resting_shadow(cx))
                    .children(rows)
                    .when(count == 0, |this| {
                        this.child(
                            div()
                                .p_4()
                                .text_color(quiet)
                                .bg(chip)
                                .child("No changes yet"),
                        )
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<LedgerEntry> {
        let at = Instant::now();
        vec![
            LedgerEntry::new("price", "Changed price", "Billing", at),
            LedgerEntry::new("page", "Updated page", "Site", at).builds_on("price"),
            LedgerEntry::new("post", "Posted", "Chat", at)
                .builds_on("page")
                .irreversible(),
            LedgerEntry::new("deals", "Updated deals", "Sales", at).builds_on("price"),
        ]
    }

    fn ids(ids: &[SharedString]) -> Vec<&str> {
        ids.iter().map(|id| id.as_ref()).collect()
    }

    #[test]
    fn dependents_are_found_through_others() {
        let mut found = built_on(&entries(), "price");
        found.sort();
        assert_eq!(ids(&found), ["deals", "page", "post"]);
    }

    #[test]
    fn undo_takes_what_it_can_and_names_the_rest() {
        let (goes, kept) = undo_plan(&entries(), "price", &HashSet::new());
        assert_eq!(goes[0].as_ref(), "price");
        assert!(goes.iter().any(|id| id == "page") && goes.iter().any(|id| id == "deals"));
        assert_eq!(ids(&kept), ["post"]);
    }

    #[test]
    fn redo_brings_back_what_it_builds_on_first() {
        let undone: HashSet<SharedString> = ["price", "page"]
            .into_iter()
            .map(SharedString::from)
            .collect();
        assert_eq!(
            ids(&redo_plan(&entries(), "page", &undone)),
            ["price", "page"]
        );
        assert_eq!(ids(&redo_plan(&entries(), "price", &undone)), ["price"]);
    }

    #[test]
    fn ages_read_short() {
        assert_eq!(ago_words(Duration::from_secs(2)), "just now");
        assert_eq!(ago_words(Duration::from_secs(37)), "37s ago");
        assert_eq!(ago_words(Duration::from_secs(130)), "2m ago");
    }
}
