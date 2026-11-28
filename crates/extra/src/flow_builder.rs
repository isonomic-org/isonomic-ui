// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Flow builder: an automation's steps from top to bottom, with branches and a test run.
//!
//! The flow starts with its trigger. Each step is a card joined to the next
//! by a line with a plus on it, which adds a step there. A branch step splits
//! the flow into a Yes and a No lane. The selected step can move up or down
//! within its lane or be removed. Test runs the flow where it stands: each
//! card shows whether it waits, runs, passed or failed, with a line of
//! output.
//!
//! The builder only shows the steps it is given and reports what the person
//! asks for. [`insert_step`], [`remove_step`] and [`move_step`] apply those
//! requests to a list of steps for the owner.
//!
//! Keyboard: the flow is a Tab stop. Up and Down select steps in reading
//! order, Alt with Up or Down moves the selected step, Delete removes it and
//! Enter runs the test.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, div,
    percentage, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// What a step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowStepKind {
    /// What starts the flow. Only the first step.
    Trigger,
    /// Something the flow does.
    Action,
    /// A question that splits the flow into Yes and No.
    Branch,
    /// A wait before the next step.
    Delay,
}

impl FlowStepKind {
    fn icon(self) -> IconName {
        match self {
            Self::Trigger => IconName::Play,
            Self::Action => IconName::SquareTerminal,
            Self::Branch => IconName::Network,
            Self::Delay => IconName::Calendar,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Trigger => "When",
            Self::Action => "Do",
            Self::Branch => "If",
            Self::Delay => "Wait",
        }
    }
}

/// Where a step stands in a test run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StepRun {
    /// Not part of a run.
    #[default]
    Idle,
    /// Waiting its turn.
    Waiting,
    /// Running now.
    Running,
    /// Done, with a line of output.
    Passed(SharedString),
    /// Failed, with what went wrong.
    Failed(SharedString),
    /// Not reached, such as the lane a branch didn't take.
    Skipped,
}

/// One step of a flow.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowStep {
    /// Identifies the step.
    pub id: SharedString,
    /// What it does.
    pub kind: FlowStepKind,
    /// Its name.
    pub title: SharedString,
    /// A line about it, such as the condition of a branch.
    pub detail: Option<SharedString>,
    /// The Yes lane of a branch.
    pub yes: Vec<FlowStep>,
    /// The No lane of a branch.
    pub no: Vec<FlowStep>,
    /// Where it stands in a test run.
    pub run: StepRun,
}

impl FlowStep {
    /// Create a step.
    pub fn new(
        id: impl Into<SharedString>,
        kind: FlowStepKind,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            detail: None,
            yes: Vec::new(),
            no: Vec::new(),
            run: StepRun::Idle,
        }
    }

    /// Add a line about the step.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set a branch's two lanes.
    pub fn lanes(mut self, yes: Vec<FlowStep>, no: Vec<FlowStep>) -> Self {
        self.yes = yes;
        self.no = no;
        self
    }

    /// Set where it stands in a test run.
    pub fn run(mut self, run: StepRun) -> Self {
        self.run = run;
        self
    }
}

/// Where a new step goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertAt {
    /// Right after this step, in the same lane.
    After(SharedString),
    /// At the top of a branch's lane.
    LaneStart {
        /// The branch step.
        branch: SharedString,
        /// The Yes lane, or else the No lane.
        yes: bool,
    },
}

/// Put `step` where `at` says. Returns whether a place was found.
pub fn insert_step(steps: &mut Vec<FlowStep>, at: &InsertAt, step: FlowStep) -> bool {
    match at {
        InsertAt::After(id) => {
            if let Some(ix) = steps.iter().position(|s| &s.id == id) {
                steps.insert(ix + 1, step);
                return true;
            }
        }
        InsertAt::LaneStart { branch, yes } => {
            if let Some(s) = steps.iter_mut().find(|s| &s.id == branch) {
                let lane = if *yes { &mut s.yes } else { &mut s.no };
                lane.insert(0, step);
                return true;
            }
        }
    }
    for s in steps.iter_mut() {
        if insert_step(&mut s.yes, at, step.clone()) || insert_step(&mut s.no, at, step.clone()) {
            return true;
        }
    }
    false
}

/// Take a step out, wherever it is. Returns it.
pub fn remove_step(steps: &mut Vec<FlowStep>, id: &str) -> Option<FlowStep> {
    if let Some(ix) = steps.iter().position(|s| s.id.as_ref() == id) {
        return Some(steps.remove(ix));
    }
    steps
        .iter_mut()
        .find_map(|s| remove_step(&mut s.yes, id).or_else(|| remove_step(&mut s.no, id)))
}

/// Move a step one place up or down within its lane. The trigger stays
/// first. Returns whether it moved.
pub fn move_step(steps: &mut [FlowStep], id: &str, up: bool) -> bool {
    if let Some(ix) = steps.iter().position(|s| s.id.as_ref() == id) {
        let to = if up { ix.checked_sub(1) } else { Some(ix + 1) };
        return match to {
            Some(to)
                if to < steps.len()
                    && steps[to].kind != FlowStepKind::Trigger
                    && steps[ix].kind != FlowStepKind::Trigger =>
            {
                steps.swap(ix, to);
                true
            }
            _ => false,
        };
    }
    steps
        .iter_mut()
        .any(|s| move_step(&mut s.yes, id, up) || move_step(&mut s.no, id, up))
}

/// Every step's id in reading order: a branch, then its Yes lane, then its
/// No lane.
pub fn reading_order(steps: &[FlowStep]) -> Vec<SharedString> {
    let mut out = Vec::new();
    for step in steps {
        out.push(step.id.clone());
        out.extend(reading_order(&step.yes));
        out.extend(reading_order(&step.no));
    }
    out
}

fn find<'a>(steps: &'a [FlowStep], id: &str) -> Option<&'a FlowStep> {
    steps.iter().find_map(|s| {
        if s.id.as_ref() == id {
            Some(s)
        } else {
            find(&s.yes, id).or_else(|| find(&s.no, id))
        }
    })
}

type AddHandler = Rc<dyn Fn(&InsertAt, FlowStepKind, &mut Window, &mut App)>;
type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type MoveHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type TestHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone, Default)]
struct Handlers {
    add: Option<AddHandler>,
    remove: Option<IdHandler>,
    select: Option<IdHandler>,
    moved: Option<MoveHandler>,
    test: Option<TestHandler>,
}

/// An automation's steps from top to bottom.
#[derive(IntoElement)]
pub struct FlowBuilder {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    steps: Vec<FlowStep>,
    selected: Option<SharedString>,
    testing: bool,
    handlers: Handlers,
}

impl FlowBuilder {
    /// Create a builder for these steps.
    pub fn new(id: impl Into<ElementId>, steps: Vec<FlowStep>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Flow".into(),
            steps,
            selected: None,
            testing: false,
            handlers: Handlers::default(),
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Mark a step as selected.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Say a test run is going: Test turns into a running label and editing
    /// is held.
    pub fn testing(mut self, testing: bool) -> Self {
        self.testing = testing;
        self
    }

    /// Run when a plus asks for a new step of a kind at a place.
    pub fn on_add(
        mut self,
        handler: impl Fn(&InsertAt, FlowStepKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.add = Some(Rc::new(handler));
        self
    }

    /// Run when a step is removed.
    pub fn on_remove(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.remove = Some(Rc::new(handler));
        self
    }

    /// Run when a step is selected.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.select = Some(Rc::new(handler));
        self
    }

    /// Run when a step moves, `true` for up.
    pub fn on_move(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.moved = Some(Rc::new(handler));
        self
    }

    /// Run when Test is pressed.
    pub fn on_test(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.handlers.test = Some(Rc::new(handler));
        self
    }
}

impl Styled for FlowBuilder {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The colours a card needs.
#[derive(Clone, Copy)]
struct Paint {
    ink: Hsla,
    quiet: Hsla,
    fill: Hsla,
    line: Hsla,
    chip: Hsla,
    accent: Hsla,
    success: Hsla,
    danger: Hsla,
}

struct Ctx<'a> {
    id: &'a ElementId,
    selected: Option<&'a SharedString>,
    testing: bool,
    handlers: &'a Handlers,
    paint: Paint,
}

impl Ctx<'_> {
    fn child_id(&self, name: String) -> ElementId {
        ElementId::NamedChild(self.id.clone().into(), name.into())
    }

    /// The line between two steps, with the plus that adds one there.
    fn joint(&self, at: InsertAt, key: String, branch_allowed: bool) -> impl IntoElement {
        let paint = self.paint;
        let add = self.handlers.add.clone();
        let line = || div().w(px(2.)).h(px(10.)).bg(paint.line);
        v_flex().items_center().child(line()).when_some(
            add.filter(|_| !self.testing),
            |this, add| {
                this.child(
                    Button::new(self.child_id(format!("add-{key}")))
                        .outline()
                        .xsmall()
                        .icon(IconName::Plus)
                        .accessibility_label("Add a step here")
                        .dropdown_menu(move |menu, _, _| {
                            let kinds: &[(FlowStepKind, &str)] = if branch_allowed {
                                &[
                                    (FlowStepKind::Action, "Action"),
                                    (FlowStepKind::Branch, "Branch"),
                                    (FlowStepKind::Delay, "Delay"),
                                ]
                            } else {
                                &[
                                    (FlowStepKind::Action, "Action"),
                                    (FlowStepKind::Delay, "Delay"),
                                ]
                            };
                            let mut menu = menu.label("Add a step");
                            for (kind, label) in kinds {
                                let add = add.clone();
                                let at = at.clone();
                                let kind = *kind;
                                menu = menu.item(
                                    PopupMenuItem::new(*label)
                                        .icon(kind.icon())
                                        .on_click(move |_, window, cx| add(&at, kind, window, cx)),
                                );
                            }
                            menu
                        }),
                )
                .child(line())
            },
        )
    }

    fn badge(&self, run: &StepRun) -> Option<impl IntoElement> {
        let paint = self.paint;
        let (icon, color, word) = match run {
            StepRun::Idle => return None,
            StepRun::Waiting => (IconName::Ellipsis, paint.quiet, "Waiting"),
            StepRun::Running => (IconName::LoaderCircle, paint.accent, "Running"),
            StepRun::Passed(_) => (IconName::CircleCheck, paint.success, "Passed"),
            StepRun::Failed(_) => (IconName::CircleX, paint.danger, "Failed"),
            StepRun::Skipped => (IconName::Minus, paint.quiet, "Skipped"),
        };
        let icon = Icon::new(icon).xsmall();
        let icon = if matches!(run, StepRun::Running) {
            icon.with_animation(
                "spin",
                Animation::new(std::time::Duration::from_millis(900)).repeat(),
                |icon, time| icon.transform(Transformation::rotate(percentage(time))),
            )
            .into_any_element()
        } else {
            icon.into_any_element()
        };
        Some(
            h_flex()
                .flex_none()
                .gap_1()
                .text_caption()
                .text_color(color)
                .child(icon)
                .child(word),
        )
    }

    fn card(&self, step: &FlowStep, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let paint = self.paint;
        let is_selected = self.selected == Some(&step.id);
        let select = self.handlers.select.clone();
        let id = step.id.clone();
        let output = match &step.run {
            StepRun::Passed(text) => Some((text.clone(), paint.quiet)),
            StepRun::Failed(text) => Some((text.clone(), paint.danger)),
            _ => None,
        };
        let faded = matches!(step.run, StepRun::Skipped | StepRun::Waiting);
        let name = format!("{} {}", step.kind.word(), step.title);
        div()
            .id(self.child_id(format!("step-{}", step.id)))
            .w(px(260.))
            .max_w_full()
            .flex()
            .gap_2p5()
            .p_2p5()
            .rounded(px(10.))
            .border_1()
            .border_color(if is_selected {
                paint.accent
            } else {
                paint.line
            })
            .bg(paint.fill)
            .shadow(resting_shadow(cx))
            .cursor_pointer()
            .when(faded, |this| this.opacity(0.55))
            .when(is_selected, |this| this.focus_ring_style(window, cx))
            .role(Role::Button)
            .aria_label(name)
            .aria_selected(is_selected)
            .on_click(move |_, window, cx| {
                if let Some(select) = &select {
                    select(&id, window, cx);
                }
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(px(7.))
                    .bg(if step.kind == FlowStepKind::Trigger {
                        paint.accent.opacity(0.15)
                    } else {
                        paint.chip
                    })
                    .text_color(if step.kind == FlowStepKind::Trigger {
                        paint.accent
                    } else {
                        paint.ink
                    })
                    .child(Icon::new(step.kind.icon()).small()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_none()
                                    .text_caption()
                                    .text_color(paint.quiet)
                                    .child(step.kind.word()),
                            )
                            .child(div().flex_1())
                            .children(self.badge(&step.run)),
                    )
                    .child(div().truncate().font_medium().child(step.title.clone()))
                    .children(step.detail.clone().map(|detail| {
                        div()
                            .text_caption()
                            .text_color(paint.quiet)
                            .line_clamp(2)
                            .child(detail)
                    }))
                    .children(output.map(|(text, color)| {
                        div()
                            .mt_1()
                            .px_1p5()
                            .py_0p5()
                            .rounded(px(5.))
                            .bg(paint.chip)
                            .text_caption()
                            .text_color(color)
                            .line_clamp(2)
                            .child(text)
                    })),
            )
    }

    /// A lane of steps, joined top to bottom. `head` is where the first plus
    /// adds: before the first step of a branch's lane.
    fn lane(
        &self,
        steps: &[FlowStep],
        head: Option<InsertAt>,
        top_level: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let mut column = v_flex().items_center();
        if let Some(head) = head {
            let key = match &head {
                InsertAt::LaneStart { branch, yes } => format!("{branch}-{yes}"),
                InsertAt::After(id) => id.to_string(),
            };
            column = column.child(self.joint(head, key, top_level));
        }
        for (ix, step) in steps.iter().enumerate() {
            if ix > 0 {
                column = column.child(self.joint(
                    InsertAt::After(steps[ix - 1].id.clone()),
                    steps[ix - 1].id.to_string(),
                    top_level,
                ));
            }
            column = column.child(self.card(step, window, cx));
            if step.kind == FlowStepKind::Branch {
                let paint = self.paint;
                let lane_box =
                    |label: &'static str, yes: bool, cx: &mut App, window: &mut Window| {
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .child(div().w(px(2.)).h(px(10.)).bg(paint.line))
                            .child(
                                div()
                                    .px_2()
                                    .rounded(px(5.))
                                    .bg(if yes {
                                        paint.success.opacity(0.14)
                                    } else {
                                        paint.chip
                                    })
                                    .text_label_small()
                                    .child(label),
                            )
                            .child(self.lane(
                                if yes { &step.yes } else { &step.no },
                                Some(InsertAt::LaneStart {
                                    branch: step.id.clone(),
                                    yes,
                                }),
                                false,
                                window,
                                cx,
                            ))
                    };
                column = column.child(
                    h_flex()
                        .items_start()
                        .gap_4()
                        .child(lane_box("Yes", true, cx, window))
                        .child(lane_box("No", false, cx, window)),
                );
            }
        }
        if let Some(last) = steps.last().filter(|_| top_level) {
            column = column.child(self.joint(
                InsertAt::After(last.id.clone()),
                format!("{}-end", last.id),
                true,
            ));
        }
        column
    }
}

impl RenderOnce for FlowBuilder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let paint = Paint {
            ink: theme.foreground,
            quiet: theme.muted_foreground,
            fill: theme.background,
            line: theme.border,
            chip: theme.muted,
            accent: theme.info,
            success: theme.success,
            danger: theme.danger,
        };
        let radius_lg = theme.radius_lg;
        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);
        let order = reading_order(&self.steps);
        let count = order.len();
        let failed = order
            .iter()
            .filter_map(|id| find(&self.steps, id))
            .filter(|s| matches!(s.run, StepRun::Failed(_)))
            .count();
        let passed = order
            .iter()
            .filter_map(|id| find(&self.steps, id))
            .filter(|s| matches!(s.run, StepRun::Passed(_)))
            .count();

        let key_down = {
            let handlers = self.handlers.clone();
            let selected = self.selected.clone();
            let steps = self.steps.clone();
            let testing = self.testing;
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let key = event.keystroke.key.as_str();
                let alt = event.keystroke.modifiers.alt;
                let at = selected
                    .as_ref()
                    .and_then(|id| order.iter().position(|o| o == id));
                match key {
                    "up" | "down" if alt && !testing => {
                        if let (Some(id), Some(moved)) = (&selected, &handlers.moved)
                            && find(&steps, id).is_some_and(|s| s.kind != FlowStepKind::Trigger)
                        {
                            moved(id, key == "up", window, cx);
                        }
                    }
                    "up" | "down" => {
                        let next = match (at, key) {
                            (None, _) => 0,
                            (Some(ix), "up") => ix.saturating_sub(1),
                            (Some(ix), _) => (ix + 1).min(order.len().saturating_sub(1)),
                        };
                        if let (Some(id), Some(select)) = (order.get(next), &handlers.select) {
                            select(id, window, cx);
                        }
                    }
                    "delete" | "backspace" if !testing => {
                        if let (Some(id), Some(remove)) = (&selected, &handlers.remove)
                            && find(&steps, id).is_some_and(|s| s.kind != FlowStepKind::Trigger)
                        {
                            remove(id, window, cx);
                        }
                    }
                    "enter" if !testing => {
                        if let Some(test) = &handlers.test {
                            test(window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }
        };

        let ctx = Ctx {
            id: &self.id,
            selected: self.selected.as_ref(),
            testing: self.testing,
            handlers: &self.handlers,
            paint,
        };
        let flow = ctx
            .lane(&self.steps, None, true, window, cx)
            .into_any_element();

        let selected_step = self
            .selected
            .as_ref()
            .and_then(|id| find(&self.steps, id))
            .filter(|s| s.kind != FlowStepKind::Trigger)
            .cloned();
        let tools = selected_step.filter(|_| !self.testing).map(|step| {
            let (up, down, remove) = (step.id.clone(), step.id.clone(), step.id.clone());
            let moved_up = self.handlers.moved.clone();
            let moved_down = self.handlers.moved.clone();
            let removed = self.handlers.remove.clone();
            h_flex()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_caption()
                        .text_color(paint.quiet)
                        .child(format!("{} {}", step.kind.word(), step.title)),
                )
                .child(
                    Button::new(ctx.child_id("up".into()))
                        .ghost()
                        .xsmall()
                        .icon(IconName::ArrowUp)
                        .accessibility_label(format!("Move {} up", step.title))
                        .on_click(move |_, window, cx| {
                            if let Some(moved) = &moved_up {
                                moved(&up, true, window, cx);
                            }
                        }),
                )
                .child(
                    Button::new(ctx.child_id("down".into()))
                        .ghost()
                        .xsmall()
                        .icon(IconName::ArrowDown)
                        .accessibility_label(format!("Move {} down", step.title))
                        .on_click(move |_, window, cx| {
                            if let Some(moved) = &moved_down {
                                moved(&down, false, window, cx);
                            }
                        }),
                )
                .child(
                    Button::new(ctx.child_id("remove".into()))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Delete)
                        .accessibility_label(format!("Remove {}", step.title))
                        .on_click(move |_, window, cx| {
                            if let Some(removed) = &removed {
                                removed(&remove, window, cx);
                            }
                        }),
                )
        });

        let summary = if self.testing {
            "Testing\u{2026}".to_string()
        } else if failed > 0 {
            format!("{failed} failed, {passed} passed")
        } else if passed > 0 {
            format!("{passed} of {count} passed")
        } else {
            format!("{count} steps")
        };
        let test = self.handlers.test.clone();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(640.))
            .rounded(radius_lg)
            .border_1()
            .border_color(paint.line)
            .bg(paint.chip.opacity(0.35))
            .text_color(paint.ink)
            .text_body()
            .line_height(relative(1.4))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(paint.line)
                    .bg(paint.fill)
                    .rounded_t(radius_lg)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .child(self.title),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_caption()
                            .text_color(if failed > 0 {
                                paint.danger
                            } else {
                                paint.quiet
                            })
                            .child(summary),
                    )
                    .child(
                        Button::new(ctx.child_id("test".into()))
                            .small()
                            .primary()
                            .icon(IconName::Play)
                            .label(if self.testing { "Testing" } else { "Test" })
                            .disabled(self.testing || test.is_none())
                            .on_click(move |_, window, cx| {
                                if let Some(test) = &test {
                                    test(window, cx);
                                }
                            }),
                    ),
            )
            .children(tools.map(|tools| {
                div()
                    .px_4()
                    .py_1()
                    .border_b_1()
                    .border_color(paint.line)
                    .bg(paint.fill)
                    .child(tools)
            }))
            .child(
                div()
                    .id(ctx.child_id("canvas".into()))
                    .track_focus(&focus)
                    .on_key_down(key_down)
                    .role(Role::Group)
                    .aria_label(format!("Flow, {count} steps"))
                    .m_1()
                    .p_4()
                    .rounded(radius_lg)
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    .flex()
                    .justify_center()
                    .child(flow),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow() -> Vec<FlowStep> {
        vec![
            FlowStep::new("start", FlowStepKind::Trigger, "Invoice is overdue"),
            FlowStep::new("check", FlowStepKind::Branch, "Over 1,000?").lanes(
                vec![FlowStep::new(
                    "call",
                    FlowStepKind::Action,
                    "Ask for a call",
                )],
                vec![FlowStep::new(
                    "mail",
                    FlowStepKind::Action,
                    "Send a reminder",
                )],
            ),
            FlowStep::new("log", FlowStepKind::Action, "Log the chase"),
        ]
    }

    #[test]
    fn reading_order_walks_lanes_after_their_branch() {
        let order: Vec<_> = reading_order(&flow())
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(order, ["start", "check", "call", "mail", "log"]);
    }

    #[test]
    fn steps_insert_after_a_step_or_at_a_lane_start() {
        let mut steps = flow();
        let wait = FlowStep::new("wait", FlowStepKind::Delay, "Wait 3 days");
        assert!(insert_step(
            &mut steps,
            &InsertAt::After("mail".into()),
            wait
        ));
        assert_eq!(steps[1].no[1].id.as_ref(), "wait");
        let first = FlowStep::new("note", FlowStepKind::Action, "Add a note");
        let at = InsertAt::LaneStart {
            branch: "check".into(),
            yes: true,
        };
        assert!(insert_step(&mut steps, &at, first));
        assert_eq!(steps[1].yes[0].id.as_ref(), "note");
    }

    #[test]
    fn steps_come_out_from_anywhere() {
        let mut steps = flow();
        assert_eq!(
            remove_step(&mut steps, "call").map(|s| s.id),
            Some("call".into())
        );
        assert!(steps[1].yes.is_empty());
        assert!(remove_step(&mut steps, "nothing").is_none());
    }

    #[test]
    fn steps_move_within_their_lane_but_never_past_the_trigger() {
        let mut steps = flow();
        assert!(move_step(&mut steps, "log", true));
        assert_eq!(steps[1].id.as_ref(), "log");
        assert!(!move_step(&mut steps, "log", true));
        assert!(!move_step(&mut steps, "start", false));
    }
}
