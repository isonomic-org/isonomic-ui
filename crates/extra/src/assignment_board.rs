//! Assignment board: work items in one column per assignee, agents and people alike.
//!
//! Each column shows who it belongs to, whether they're free, and how much
//! work they hold against what they can take. Cards are dragged between
//! columns to hand work over; a column that would go over its limit says
//! so while the card hovers. The board never moves a card itself: it
//! reports the hand-over and the owner decides.
//!
//! Keyboard: the board is a Tab stop. The arrow keys move between cards,
//! Alt with Left or Right hands the selected card to the next column, and
//! Enter opens it.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    v_flex,
};
use isonomic_ui::{
    Role,
    App, AppContext as _, BoxShadow, Context, ElementId, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// Who a column belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssigneeKind {
    /// Work nobody has picked up.
    Unassigned,
    /// An assistant.
    Agent,
    /// A person.
    Person,
}

/// Whether an assignee can take work right now.
#[derive(Debug, Clone, PartialEq)]
pub enum Availability {
    /// Ready for more.
    Free,
    /// Working, with a note on what.
    Busy(SharedString),
    /// Not taking work, with a note on why.
    Away(SharedString),
}

/// One column.
#[derive(Debug, Clone, PartialEq)]
pub struct AssigneeLane {
    /// Identifies it.
    pub id: SharedString,
    /// Who it belongs to.
    pub name: SharedString,
    /// What kind of assignee.
    pub kind: AssigneeKind,
    /// How many points of work they can hold, if limited.
    pub capacity: Option<u32>,
    /// Whether they can take work.
    pub availability: Availability,
}

impl AssigneeLane {
    /// Create a column with no limit.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>, kind: AssigneeKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            capacity: None,
            availability: Availability::Free,
        }
    }

    /// Limit the work they can hold.
    pub fn capacity(mut self, points: u32) -> Self {
        self.capacity = Some(points);
        self
    }

    /// Say whether they can take work.
    pub fn availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }
}

/// How soon an item matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkPriority {
    /// Whenever.
    Low,
    /// Soon.
    Normal,
    /// Now.
    Urgent,
}

/// One piece of work.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkItem {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub title: SharedString,
    /// The column it's in.
    pub lane: SharedString,
    /// How much work, in points.
    pub points: u32,
    /// How soon it matters.
    pub priority: WorkPriority,
    /// A short tag, such as the area it belongs to.
    pub tag: Option<SharedString>,
    /// What it's waiting on, if anything.
    pub blocked_by: Option<SharedString>,
}

impl WorkItem {
    /// Create a one-point item of normal priority.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        lane: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            lane: lane.into(),
            points: 1,
            priority: WorkPriority::Normal,
            tag: None,
            blocked_by: None,
        }
    }

    /// Set how much work it is.
    pub fn points(mut self, points: u32) -> Self {
        self.points = points;
        self
    }

    /// Set how soon it matters.
    pub fn priority(mut self, priority: WorkPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Tag it.
    pub fn tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Say what it's waiting on.
    pub fn blocked_by(mut self, reason: impl Into<SharedString>) -> Self {
        self.blocked_by = Some(reason.into());
        self
    }
}

/// The points of work a column holds.
pub fn lane_load(items: &[WorkItem], lane: &str) -> u32 {
    items
        .iter()
        .filter(|item| item.lane.as_ref() == lane)
        .map(|item| item.points)
        .sum()
}

/// Whether handing `item` to `lane` would take it over its limit.
pub fn would_overload(lanes: &[AssigneeLane], items: &[WorkItem], item: &str, lane: &str) -> bool {
    let Some(target) = lanes.iter().find(|l| l.id.as_ref() == lane) else {
        return false;
    };
    let Some(item) = items.iter().find(|i| i.id.as_ref() == item) else {
        return false;
    };
    if item.lane == target.id {
        return false;
    }
    target
        .capacity
        .is_some_and(|capacity| lane_load(items, lane) + item.points > capacity)
}

/// A column's cards in order: most urgent first, then as given.
pub fn lane_cards<'a>(items: &'a [WorkItem], lane: &str) -> Vec<&'a WorkItem> {
    let mut cards: Vec<_> = items.iter().filter(|i| i.lane.as_ref() == lane).collect();
    cards.sort_by_key(|card| std::cmp::Reverse(card.priority));
    cards
}

/// Initials for a name: up to two letters.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// What's carried while a card is dragged.
#[derive(Clone)]
struct DraggedWork {
    id: SharedString,
    title: SharedString,
    points: u32,
    fill: Hsla,
    ink: Hsla,
    line: Hsla,
}

impl Render for DraggedWork {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .w(px(200.))
            .px_3()
            .py_2()
            .rounded(px(8.))
            .border_1()
            .border_color(self.line)
            .bg(self.fill)
            .text_color(self.ink)
            .text_sm()
            .shadow_lg()
            .child(div().flex_1().min_w_0().truncate().child(self.title.clone()))
            .child(div().text_xs().opacity(0.7).child(format!("{} pt", self.points)))
    }
}

type AssignHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type ItemHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Columns of assignees with the work each one holds.
#[derive(IntoElement)]
pub struct AssignmentBoard {
    id: ElementId,
    style: StyleRefinement,
    lanes: Vec<AssigneeLane>,
    items: Vec<WorkItem>,
    selected: Option<SharedString>,
    on_assign: Option<AssignHandler>,
    on_select: Option<ItemHandler>,
    on_open: Option<ItemHandler>,
}

impl AssignmentBoard {
    /// Create a board.
    pub fn new(id: impl Into<ElementId>, lanes: Vec<AssigneeLane>, items: Vec<WorkItem>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            lanes,
            items,
            selected: None,
            on_assign: None,
            on_select: None,
            on_open: None,
        }
    }

    /// The selected card.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run with an item and the column it was handed to.
    pub fn on_assign(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_assign = Some(Rc::new(handler));
        self
    }

    /// Run when a card is selected.
    pub fn on_select(mut self, handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when a card is opened with a double click or Enter.
    pub fn on_open(mut self, handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl Styled for AssignmentBoard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Where the arrow keys go from the selected card.
fn step(
    lanes: &[AssigneeLane],
    items: &[WorkItem],
    selected: Option<&SharedString>,
    key: &str,
) -> Option<SharedString> {
    let columns: Vec<Vec<&WorkItem>> = lanes.iter().map(|l| lane_cards(items, &l.id)).collect();
    let first = || columns.iter().flatten().next().map(|i| i.id.clone());
    let Some(selected) = selected else {
        return first();
    };
    let Some((col, row)) = columns.iter().enumerate().find_map(|(c, cards)| {
        cards.iter().position(|i| &i.id == selected).map(|r| (c, r))
    }) else {
        return first();
    };
    match key {
        "up" => columns[col].get(row.saturating_sub(1)).map(|i| i.id.clone()),
        "down" => columns[col]
            .get((row + 1).min(columns[col].len() - 1))
            .map(|i| i.id.clone()),
        "left" | "right" => {
            let mut order: Box<dyn Iterator<Item = usize>> = if key == "left" {
                Box::new((0..col).rev())
            } else {
                Box::new(col + 1..columns.len())
            };
            order
                .find(|c| !columns[*c].is_empty())
                .map(|c| columns[c][row.min(columns[c].len() - 1)].id.clone())
                .or(Some(selected.clone()))
        }
        _ => None,
    }
}

impl RenderOnce for AssignmentBoard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let success = theme.success;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let lanes = Rc::new(self.lanes);
        let items = Rc::new(self.items);
        let total: u32 = items.iter().map(|i| i.points).sum();
        let unassigned = lanes
            .iter()
            .filter(|l| l.kind == AssigneeKind::Unassigned)
            .map(|l| lane_cards(&items, &l.id).len())
            .sum::<usize>();

        let key_down = {
            let lanes = lanes.clone();
            let items = items.clone();
            let selected = self.selected.clone();
            let on_assign = self.on_assign.clone();
            let on_select = self.on_select.clone();
            let on_open = self.on_open.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let key = event.keystroke.key.as_str();
                let alt = event.keystroke.modifiers.alt;
                if alt && matches!(key, "left" | "right") {
                    let Some(id) = &selected else { return };
                    let Some(item) = items.iter().find(|i| &i.id == id) else {
                        return;
                    };
                    let at = lanes.iter().position(|l| l.id == item.lane);
                    let to = match (at, key) {
                        (Some(at), "left") => at.checked_sub(1),
                        (Some(at), _) => Some(at + 1).filter(|to| *to < lanes.len()),
                        (None, _) => None,
                    };
                    if let (Some(to), Some(assign)) = (to, &on_assign) {
                        assign(id, &lanes[to].id, window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.modifiers.modified() {
                    return;
                }
                if key == "enter" {
                    if let (Some(id), Some(open)) = (&selected, &on_open) {
                        open(id, window, cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if let Some(next) = step(&lanes, &items, selected.as_ref(), key) {
                    if let Some(select) = &on_select {
                        select(&next, window, cx);
                    }
                    cx.stop_propagation();
                }
            }
        };

        let columns: Vec<_> = lanes
            .iter()
            .map(|lane| {
                let cards = lane_cards(&items, &lane.id);
                let load = lane_load(&items, &lane.id);
                let over = lane.capacity.is_some_and(|c| load > c);
                let away = matches!(lane.availability, Availability::Away(_));
                let (dot, note) = match &lane.availability {
                    Availability::Free => (success, "Free".into()),
                    Availability::Busy(what) => (warning, what.clone()),
                    Availability::Away(why) => (quiet, why.clone()),
                };
                let badge = match lane.kind {
                    AssigneeKind::Unassigned => div()
                        .size(px(26.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_dashed()
                        .border_color(line)
                        .child(Icon::new(IconName::Inbox).xsmall().text_color(quiet)),
                    AssigneeKind::Agent => div()
                        .size(px(26.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(7.))
                        .bg(accent.opacity(0.14))
                        .child(Icon::new(IconName::Bot).xsmall().text_color(accent)),
                    AssigneeKind::Person => div()
                        .size(px(26.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(chip)
                        .text_xs()
                        .font_semibold()
                        .child(initials(&lane.name)),
                };
                let meter = lane.capacity.map(|capacity| {
                    let share = if capacity == 0 {
                        1.
                    } else {
                        (load as f32 / capacity as f32).min(1.)
                    };
                    v_flex()
                        .gap_1()
                        .child(
                            div().h(px(4.)).rounded_full().bg(chip).child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(if over { danger } else { accent })
                                    .w(relative(share)),
                            ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if over { danger } else { quiet })
                                .child(if over {
                                    format!("{load} of {capacity} pt, over by {}", load - capacity)
                                } else {
                                    format!("{load} of {capacity} pt")
                                }),
                        )
                });

                let drop_lane = lane.id.clone();
                let assign = self.on_assign.clone();
                let hover_lanes = lanes.clone();
                let hover_items = items.clone();
                let hover_lane = lane.id.clone();

                let card_views: Vec<_> = cards
                    .iter()
                    .map(|item| {
                        let is_selected = self.selected.as_ref() == Some(&item.id);
                        let select = self.on_select.clone();
                        let open = self.on_open.clone();
                        let id = item.id.clone();
                        let open_id = item.id.clone();
                        let dragged = DraggedWork {
                            id: item.id.clone(),
                            title: item.title.clone(),
                            points: item.points,
                            fill,
                            ink,
                            line,
                        };
                        let (flag, flag_color) = match item.priority {
                            WorkPriority::Urgent => (Some("Urgent"), danger),
                            WorkPriority::Low => (Some("Low"), quiet),
                            WorkPriority::Normal => (None, quiet),
                        };
                        v_flex()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("card-{}", item.id).into(),
                            ))
                            .gap_1p5()
                            .p_2p5()
                            .rounded(radius)
                            .border_1()
                            .border_color(if is_selected { accent } else { line })
                            .bg(fill)
                            .cursor_grab()
                            .when(is_selected && is_focused, |this| {
                                this.shadow(vec![
                                    BoxShadow::new(px(0.), px(0.), accent.opacity(0.3))
                                        .spread_radius(px(2.)),
                                ])
                            })
                            .role(Role::ListItem)
                            .aria_label(format!(
                                "{}, {} pt{}{}",
                                item.title,
                                item.points,
                                flag.map(|f| format!(", {f}")).unwrap_or_default(),
                                item.blocked_by
                                    .as_ref()
                                    .map(|b| format!(", waiting on {b}"))
                                    .unwrap_or_default()
                            ))
                            .aria_selected(is_selected)
                            .on_click(move |event, window, cx| {
                                if event.click_count() > 1 {
                                    if let Some(open) = &open {
                                        open(&open_id, window, cx);
                                    }
                                } else if let Some(select) = &select {
                                    select(&id, window, cx);
                                }
                            })
                            .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
                            .child(
                                div()
                                    .line_height(relative(1.35))
                                    .when(item.blocked_by.is_some(), |this| this.text_color(quiet))
                                    .child(item.title.clone()),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .text_xs()
                                    .text_color(quiet)
                                    .children(flag.map(|flag| {
                                        div().text_color(flag_color).font_medium().child(flag)
                                    }))
                                    .children(item.tag.clone().map(|tag| {
                                        div().px_1().rounded(px(4.)).bg(chip).child(tag)
                                    }))
                                    .child(div().flex_1())
                                    .child(format!("{} pt", item.points)),
                            )
                            .children(item.blocked_by.clone().map(|reason| {
                                h_flex()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(warning)
                                    .child(Icon::new(IconName::Ban).xsmall())
                                    .child(div().min_w_0().truncate().child(format!("Waiting on {reason}")))
                            }))
                    })
                    .collect();

                v_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("lane-{}", lane.id).into(),
                    ))
                    .w(px(220.))
                    .flex_none()
                    .gap_2()
                    .p_2()
                    .rounded(radius_lg)
                    .bg(chip.opacity(0.45))
                    .border_1()
                    .border_color(line.opacity(0.))
                    .role(Role::Group)
                    .aria_label(format!(
                        "{}, {}, {} cards",
                        lane.name,
                        match &lane.availability {
                            Availability::Free => "free".to_string(),
                            Availability::Busy(what) => format!("busy, {what}"),
                            Availability::Away(why) => format!("away, {why}"),
                        },
                        cards.len()
                    ))
                    .drag_over::<DraggedWork>(move |style, dragged, _, _| {
                        if would_overload(&hover_lanes, &hover_items, &dragged.id, &hover_lane) {
                            style.border_color(danger).bg(danger.opacity(0.06))
                        } else {
                            style.border_color(accent).bg(accent.opacity(0.06))
                        }
                    })
                    .on_drop(move |dragged: &DraggedWork, window, cx| {
                        if let Some(assign) = &assign {
                            assign(&dragged.id, &drop_lane, window, cx);
                        }
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(badge)
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_medium()
                                            .when(away, |this| this.text_color(quiet))
                                            .child(lane.name.clone()),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .text_xs()
                                            .text_color(quiet)
                                            .when(lane.kind != AssigneeKind::Unassigned, |this| {
                                                this.child(div().size(px(6.)).flex_none().rounded_full().bg(dot))
                                                    .child(div().min_w_0().truncate().child(note))
                                            })
                                            .when(lane.kind == AssigneeKind::Unassigned, |this| {
                                                this.child(format!("{} waiting", cards.len()))
                                            }),
                                    ),
                            ),
                    )
                    .children(meter)
                    .children(card_views)
                    .when(cards.is_empty(), |this| {
                        this.child(
                            div()
                                .py_4()
                                .rounded(radius)
                                .border_1()
                                .border_dashed()
                                .border_color(line)
                                .text_xs()
                                .text_color(quiet)
                                .text_center()
                                .child("Drop work here"),
                        )
                    })
            })
            .collect();

        v_flex()
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(key_down)
            .role(Role::Group)
            .aria_label(format!("Assignment board, {} columns", lanes.len()))
            .w_full()
            .gap_3()
            .p_3()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(quiet)
                    .child(format!("{} items, {total} pt", items.len()))
                    .when(unassigned > 0, |this| {
                        this.child("\u{b7}").child(
                            div()
                                .text_color(warning)
                                .child(format!("{unassigned} not picked up")),
                        )
                    }),
            )
            .child(
                h_flex()
                    .id(ElementId::NamedChild(self.id.clone().into(), "scroll".into()))
                    .items_start()
                    .gap_2()
                    .overflow_x_scroll()
                    .children(columns),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> (Vec<AssigneeLane>, Vec<WorkItem>) {
        (
            vec![
                AssigneeLane::new("open", "Not picked up", AssigneeKind::Unassigned),
                AssigneeLane::new("bot", "Reconciler", AssigneeKind::Agent).capacity(5),
                AssigneeLane::new("ana", "Ana Ruiz", AssigneeKind::Person).capacity(3),
            ],
            vec![
                WorkItem::new("a", "Match card payouts", "bot").points(3),
                WorkItem::new("b", "Chase a refund", "open").points(2),
                WorkItem::new("c", "Approve a write-off", "ana")
                    .points(2)
                    .priority(WorkPriority::Urgent),
                WorkItem::new("d", "File receipts", "ana"),
            ],
        )
    }

    #[test]
    fn load_counts_points_in_a_column() {
        let (_, items) = board();
        assert_eq!(lane_load(&items, "ana"), 3);
        assert_eq!(lane_load(&items, "open"), 2);
    }

    #[test]
    fn handing_over_warns_only_past_the_limit() {
        let (lanes, items) = board();
        assert!(!would_overload(&lanes, &items, "b", "bot"));
        assert!(would_overload(&lanes, &items, "b", "ana"));
        assert!(!would_overload(&lanes, &items, "c", "ana"));
        assert!(!would_overload(&lanes, &items, "a", "open"));
    }

    #[test]
    fn urgent_cards_lead_and_arrows_walk_the_board() {
        let (lanes, items) = board();
        let ids: Vec<_> = lane_cards(&items, "ana").iter().map(|i| i.id.to_string()).collect();
        assert_eq!(ids, ["c", "d"]);
        let at = |id: &str| SharedString::from(id.to_string());
        assert_eq!(step(&lanes, &items, None, "down").as_deref(), Some("b"));
        assert_eq!(step(&lanes, &items, Some(&at("b")), "right").as_deref(), Some("a"));
        assert_eq!(step(&lanes, &items, Some(&at("c")), "down").as_deref(), Some("d"));
        assert_eq!(step(&lanes, &items, Some(&at("d")), "left").as_deref(), Some("a"));
        assert_eq!(step(&lanes, &items, Some(&at("c")), "right").as_deref(), Some("c"));
    }

    #[test]
    fn initials_take_two_letters() {
        assert_eq!(initials("Ana Ruiz"), "AR");
        assert_eq!(initials("Priya"), "P");
    }
}
