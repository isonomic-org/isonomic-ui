// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Agent issue stack: what your agents are on right now, one issue at a time.
//!
//! Each issue is a card: a tinted tile, the pull request or branch it lives
//! on, how long ago it moved, its title and a two-line note, and a foot with
//! its priority, who it touches (or the lines it changes) and the agent that
//! is on it, with a small pixel mark that twinkles while the agent works.
//!
//! Two looks:
//! - **Stack**: the cards behind peek out above the front one. Moving on
//!   slides the front card down and away, and the next one comes forward.
//! - **Cycle**: the open card sits between two slim strips, the issue before
//!   and the one after. Moving on morphs the strip below into the open card.
//!
//! The issues move on by themselves every few seconds. Hovering, keyboard
//! focus or reduced motion holds them still.
//!
//! Keys: Left/Up and Right/Down move to the previous and next issue, Home and
//! End jump to the first and last.

use std::time::{Duration, Instant};

use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::base::{Easing, Presence, transition};
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, Icon, h_flex, look, v_flex};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, BoxShadow, ClickEvent, Context, ElementId,
    EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Task, TestSupportExt as _, Window, div, hsla, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{ease, hover_state, over, track_hover};

/// Height of an open card.
const CARD_H: f32 = 164.;
/// Height of a strip in the cycle look.
const STRIP_H: f32 = 44.;
/// Height of the whole cycle look.
const CYCLE_H: f32 = 268.;
/// How far each card behind the front one peeks out.
const PEEK: f32 = 10.;
/// How many cards behind the front one can show.
const BEHIND: usize = 3;
/// The widest the stack grows.
const MAX_W: f32 = 360.;
/// Time between issues when they move on by themselves.
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(3200);
/// The shortest time between issues.
pub const MIN_INTERVAL: Duration = Duration::from_millis(1200);

/// The drawer curve: quick to leave, long to settle.
fn settle() -> Easing {
    Easing::cubic_bezier(0.32, 0.72, 0.0, 1.0).expect("static curve is valid")
}

/// How pressing an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuePriority {
    /// Drop everything.
    Urgent,
    /// Next up.
    High,
    /// Soon.
    Medium,
    /// When there is time.
    Low,
}

impl IssuePriority {
    /// The priority in words.
    pub fn word(self) -> &'static str {
        match self {
            Self::Urgent => "Urgent",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }

    /// How many of the three bars are lit.
    pub fn bars(self) -> usize {
        match self {
            Self::Urgent | Self::High => 3,
            Self::Medium => 2,
            Self::Low => 1,
        }
    }
}

/// Where the work on an issue lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// An open pull request.
    Pull,
    /// A merged pull request.
    Merged,
    /// A branch.
    Branch,
}

impl RefKind {
    fn icon(self) -> AssetIcon {
        match self {
            Self::Pull => AssetIcon::PullRequest,
            Self::Merged => AssetIcon::Merge,
            Self::Branch => AssetIcon::Branch,
        }
    }
}

/// The agent working on an issue.
#[derive(Debug, Clone, PartialEq)]
pub struct IssueOwner {
    name: SharedString,
    color: Option<Hsla>,
    working: bool,
}

impl IssueOwner {
    /// An agent by name.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            color: None,
            working: false,
        }
    }

    /// The colour of its pixel mark.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Whether it is working on the issue right now.
    pub fn working(mut self, working: bool) -> Self {
        self.working = working;
        self
    }
}

/// One issue in the stack.
#[derive(Debug, Clone, PartialEq)]
pub struct StackIssue {
    id: SharedString,
    title: SharedString,
    text: Option<SharedString>,
    icon: Option<AssetIcon>,
    tint: Option<Hsla>,
    reference: Option<(RefKind, SharedString)>,
    time: Option<SharedString>,
    priority: Option<IssuePriority>,
    users: Option<u32>,
    diff: Option<(u32, u32)>,
    owner: Option<IssueOwner>,
}

impl StackIssue {
    /// An issue with an id and a title.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            text: None,
            icon: None,
            tint: None,
            reference: None,
            time: None,
            priority: None,
            users: None,
            diff: None,
            owner: None,
        }
    }

    /// A short note under the title, cut at two lines.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// The icon in the tile.
    pub fn icon(mut self, icon: AssetIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The tile's colour.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    /// The open pull request the work is on, such as "#412".
    pub fn pull(mut self, label: impl Into<SharedString>) -> Self {
        self.reference = Some((RefKind::Pull, label.into()));
        self
    }

    /// The merged pull request the work went out in.
    pub fn merged(mut self, label: impl Into<SharedString>) -> Self {
        self.reference = Some((RefKind::Merged, label.into()));
        self
    }

    /// The branch the work is on.
    pub fn branch(mut self, label: impl Into<SharedString>) -> Self {
        self.reference = Some((RefKind::Branch, label.into()));
        self
    }

    /// When it last moved, already in words ("4 min ago").
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// How pressing it is.
    pub fn priority(mut self, priority: IssuePriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// How many people it touches.
    pub fn users(mut self, users: u32) -> Self {
        self.users = Some(users);
        self
    }

    /// The lines its change adds and removes. Shown when it has no user count.
    pub fn diff(mut self, added: u32, removed: u32) -> Self {
        self.diff = Some((added, removed));
        self
    }

    /// The agent on it.
    pub fn owner(mut self, owner: IssueOwner) -> Self {
        self.owner = Some(owner);
        self
    }

    /// The issue's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// How the issues are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IssueStackVariant {
    /// Cards stacked like a deck, the ones behind peeking out above.
    #[default]
    Stack,
    /// One open card between the strips of its neighbours.
    Cycle,
}

/// What the stack tells its owner.
#[derive(Debug, Clone, PartialEq)]
pub enum IssueStackEvent {
    /// This issue came to the front.
    Show(SharedString),
}

/// What a slide reads as: "2 of 6: Fix sign-up".
pub fn slide_label(ix: usize, count: usize, issue: &StackIssue) -> String {
    format!("{} of {count}: {}", ix + 1, issue.title)
}

/// The pager's count: "2 / 6".
pub fn count_label(ix: usize, count: usize) -> String {
    if count == 0 {
        "0 / 0".into()
    } else {
        format!("{} / {count}", ix + 1)
    }
}

/// Who is on an issue: "Triage is on it" while it works, its name otherwise.
pub fn who_line(owner: &IssueOwner) -> String {
    if owner.working {
        format!("{} is on it", owner.name)
    } else {
        owner.name.to_string()
    }
}

/// Which of an agent's nine pixels are lit. Always at least four, and always
/// the same for the same name.
pub fn pixel_pattern(name: &str) -> [bool; 9] {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    let mut bits = (hash ^ (hash >> 9) ^ (hash >> 18)) & 0x1ff;
    if bits.count_ones() < 4 {
        bits |= 0b010_111_010;
    }
    std::array::from_fn(|i| bits & (1 << i) != 0)
}

/// Where a card sits `pos` places behind the front one: how far up it moves,
/// how much it shrinks (as a share of its width) and how faint it is.
pub fn stack_layer(pos: f32) -> (f32, f32, f32) {
    const FADE: [f32; 5] = [1., 0.85, 0.6, 0.35, 0.];
    let pos = pos.clamp(0., 4.);
    let lower = pos.floor() as usize;
    let upper = (lower + 1).min(4);
    let t = pos - lower as f32;
    let opacity = FADE[lower] + (FADE[upper] - FADE[lower]) * t;
    (pos * PEEK, pos * 0.05, opacity)
}

/// The step that lands on `target` from `current` the short way round.
fn short_step(current: usize, target: usize, count: usize) -> isize {
    if count == 0 {
        return 0;
    }
    let forward = (target + count - current) % count;
    if forward * 2 <= count {
        forward as isize
    } else {
        forward as isize - count as isize
    }
}

/// A stack of the issues your agents are on.
pub struct AgentIssueStack {
    issues: Vec<StackIssue>,
    /// How many times the stack has moved on; can go below zero.
    step: isize,
    variant: IssueStackVariant,
    interval: Option<Duration>,
    label: SharedString,
    hovered: bool,
    focused: bool,
    moved: Instant,
    focus: FocusHandle,
    _timer: Task<()>,
}

impl EventEmitter<IssueStackEvent> for AgentIssueStack {}

impl Focusable for AgentIssueStack {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl AgentIssueStack {
    /// A stack of issues that moves on by itself every 3.2 seconds.
    pub fn new(issues: Vec<StackIssue>, cx: &mut Context<Self>) -> Self {
        let timer = cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |this, _| match this.interval {
                    Some(interval) => interval
                        .saturating_sub(this.moved.elapsed())
                        .max(Duration::from_millis(100)),
                    None => Duration::from_millis(500),
                }) else {
                    break;
                };
                cx.background_executor().timer(wait).await;
                let alive = this.update(cx, |this, cx| {
                    let Some(interval) = this.interval else {
                        return;
                    };
                    if this.held(cx) {
                        this.moved = Instant::now();
                    } else if this.moved.elapsed() >= interval {
                        this.go(1, cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            issues,
            step: 0,
            variant: IssueStackVariant::Stack,
            interval: Some(DEFAULT_INTERVAL),
            label: "Issues".into(),
            hovered: false,
            focused: false,
            moved: Instant::now(),
            focus: cx.focus_handle().tab_stop(true),
            _timer: timer,
        }
    }

    /// Pick the look.
    pub fn variant(mut self, variant: IssueStackVariant) -> Self {
        self.variant = variant;
        self
    }

    /// How long each issue stays in front, or `None` to only move by hand.
    /// Never shorter than 1.2 seconds.
    pub fn autoplay(mut self, interval: Option<Duration>) -> Self {
        self.interval = interval.map(|interval| interval.max(MIN_INTERVAL));
        self
    }

    /// The name the region reads as. Defaults to "Issues".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Switch the look.
    pub fn set_variant(&mut self, variant: IssueStackVariant, cx: &mut Context<Self>) {
        self.variant = variant;
        cx.notify();
    }

    /// Replace the issues, keeping the one in front if it is still there.
    pub fn set_issues(&mut self, issues: Vec<StackIssue>, cx: &mut Context<Self>) {
        let front = self.current().map(|issue| issue.id.clone());
        self.issues = issues;
        let ix = front
            .and_then(|id| self.issues.iter().position(|issue| issue.id == id))
            .unwrap_or(0);
        self.step = ix as isize;
        cx.notify();
    }

    /// The index of the issue in front.
    pub fn index(&self) -> usize {
        if self.issues.is_empty() {
            0
        } else {
            self.step.rem_euclid(self.issues.len() as isize) as usize
        }
    }

    /// The issue in front.
    pub fn current(&self) -> Option<&StackIssue> {
        self.issues.get(self.index())
    }

    fn held(&self, cx: &App) -> bool {
        self.hovered || self.focused || cx.reduce_motion()
    }

    fn go(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.issues.len() < 2 || delta == 0 {
            return;
        }
        self.step += delta;
        self.moved = Instant::now();
        if let Some(issue) = self.current() {
            cx.emit(IssueStackEvent::Show(issue.id.clone()));
        }
        cx.notify();
    }

    fn go_to(&mut self, ix: usize, cx: &mut Context<Self>) {
        let delta = short_step(self.index(), ix, self.issues.len());
        self.go(delta, cx);
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let last = self.issues.len().saturating_sub(1);
        match event.keystroke.key.as_str() {
            "left" | "up" => self.go(-1, cx),
            "right" | "down" => self.go(1, cx),
            "home" => {
                let delta = -(self.index() as isize);
                self.go(delta, cx);
            }
            "end" => {
                let delta = last as isize - self.index() as isize;
                self.go(delta, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// How many times issue `ix` has gone round to the back. Each lap keys
    /// fresh motion so a card that wraps round snaps instead of sliding back
    /// through the stack.
    fn lap(&self, ix: usize) -> isize {
        let count = self.issues.len() as isize;
        (self.step - ix as isize - 1).div_euclid(count) + 1
    }
}

fn front_shadow(cx: &App) -> Vec<BoxShadow> {
    let dark = cx.theme().is_dark();
    let mut shadows = look::surface::resting(cx);
    shadows.push(BoxShadow {
        color: hsla(0., 0., 0., if dark { 0.7 } else { 0.16 }),
        offset: point(px(0.), px(10.)),
        blur_radius: px(14.),
        spread_radius: px(-12.),
        inset: false,
    });
    shadows
}

fn tint_of(issue: &StackIssue, cx: &App) -> Hsla {
    issue.tint.unwrap_or(cx.theme().muted_foreground)
}

/// The tinted tile with the issue's icon.
fn tile(issue: &StackIssue, size: f32, cx: &App) -> impl IntoElement {
    let tint = tint_of(issue, cx);
    let wash = if cx.theme().is_dark() { 0.2 } else { 0.12 };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(7.))
        .bg(tint.opacity(wash))
        .when_some(issue.icon, |this, icon| {
            this.child(
                Icon::new(icon)
                    .size(px((size * 0.58).round()))
                    .text_color(tint),
            )
        })
}

/// The pull request or branch, with its icon.
fn reference(issue: &StackIssue, cx: &App) -> Option<impl IntoElement> {
    let (kind, label) = issue.reference.clone()?;
    Some(
        h_flex()
            .flex_shrink(1.)
            .min_w_0()
            .gap(px(4.))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(isonomic_ui::FontWeight::MEDIUM)
            .text_color(look::ink::muted(cx))
            .child(
                Icon::new(kind.icon())
                    .size(px(12.))
                    .flex_none()
                    .text_color(look::ink::muted(cx)),
            )
            .child(div().min_w_0().truncate().child(label)),
    )
}

/// Three bars that fill with the priority; urgent lights them all in red.
fn priority_bars(priority: IssuePriority, cx: &App) -> impl IntoElement {
    let urgent = priority == IssuePriority::Urgent;
    let lit = if urgent {
        cx.theme().danger
    } else {
        look::ink::strong(cx).opacity(0.7)
    };
    let dim = look::ink::faint(cx);
    h_flex()
        .flex_none()
        .size(px(14.))
        .items_end()
        .justify_center()
        .gap(px(1.5))
        .pb(px(2.))
        .children((0..3).map(|bar| {
            div()
                .w(px(2.5))
                .h(px(4. + 3. * bar as f32))
                .rounded(px(0.75))
                .bg(if bar < priority.bars() { lit } else { dim })
        }))
}

/// Nine pixels in the agent's colour; the lit ones twinkle while it works.
fn pixel_mark(id: ElementId, owner: &IssueOwner, still: bool, cx: &App) -> AnyElement {
    let color = owner.color.unwrap_or(cx.theme().info);
    let pattern = pixel_pattern(&owner.name);
    let working = owner.working && !still;
    let cells = pattern.iter().enumerate().map(move |(i, on)| {
        let cell =
            div()
                .size(px(3.))
                .rounded(px(0.75))
                .bg(color)
                .opacity(match (on, owner.working) {
                    (true, _) => 1.,
                    (false, true) => 0.16,
                    (false, false) => 0.,
                });
        if *on && working {
            // Each pixel starts at its own point of the cycle.
            let phase = ((i * 7) % 9) as f32 / 9.;
            cell.with_animation(
                ElementId::NamedInteger(format!("{id:?}-px").into(), i as u64),
                Animation::new(Duration::from_millis(1200)).repeat(),
                move |this, t| {
                    let t = (t + phase).fract();
                    let wave = 0.5 + 0.5 * (t * std::f32::consts::TAU).cos();
                    this.opacity(0.2 + 0.8 * wave)
                },
            )
            .into_any_element()
        } else {
            cell.into_any_element()
        }
    });
    let mut rows = Vec::new();
    let mut cells = cells.collect::<Vec<_>>().into_iter();
    for _ in 0..3 {
        rows.push(
            h_flex()
                .gap(px(1.5))
                .children(cells.by_ref().take(3))
                .into_any_element(),
        );
    }
    v_flex()
        .flex_none()
        .size(px(16.))
        .items_center()
        .justify_center()
        .gap(px(1.5))
        .children(rows)
        .into_any_element()
}

/// The whole card: head, title, note and foot.
fn full_card(issue: &StackIssue, still: bool, cx: &App) -> impl IntoElement {
    let muted = look::ink::muted(cx);
    let text = cx.theme().muted_foreground;
    v_flex()
        .w_full()
        .h(px(CARD_H))
        .pt(px(14.))
        .px(px(16.))
        .child(
            h_flex()
                .h(px(24.))
                .mb(px(10.))
                .gap(px(8.))
                .child(tile(issue, 24., cx))
                .children(reference(issue, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_right()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(muted)
                        .children(issue.time.clone()),
                ),
        )
        .child(
            div()
                .truncate()
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(isonomic_ui::FontWeight::MEDIUM)
                .text_color(look::ink::strong(cx))
                .child(issue.title.clone()),
        )
        .when_some(issue.text.clone(), |this, note| {
            this.child(
                div()
                    .mt(px(4.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .line_clamp(2)
                    .text_ellipsis()
                    .text_color(text)
                    .child(note),
            )
        })
        .child(div().flex_1())
        .child(
            h_flex()
                .flex_none()
                .h(px(20.))
                .mb(px(14.))
                .gap(px(10.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(text)
                .when_some(issue.priority, |this, priority| {
                    this.child(
                        h_flex()
                            .flex_none()
                            .gap(px(6.))
                            .child(priority_bars(priority, cx))
                            .child(priority.word()),
                    )
                })
                .map(|this| match (issue.users, issue.diff) {
                    (Some(users), _) => this.child(
                        h_flex()
                            .flex_none()
                            .gap(px(4.))
                            .child(
                                Icon::new(AssetIcon::UserMultiple)
                                    .size(px(14.))
                                    .text_color(muted),
                            )
                            .child(users.to_string()),
                    ),
                    (None, Some((added, removed))) => this.child(
                        h_flex()
                            .flex_none()
                            .gap(px(6.))
                            .child(format!("+{added}"))
                            .child(format!("\u{2212}{removed}")),
                    ),
                    (None, None) => this,
                })
                .when_some(issue.owner.clone(), |this, owner| {
                    this.child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .justify_end()
                            .gap(px(6.))
                            .text_color(muted)
                            .child(div().min_w_0().truncate().child(who_line(&owner)))
                            .child(pixel_mark(
                                ElementId::Name(format!("issue-mark-{}", issue.id).into()),
                                &owner,
                                still,
                                cx,
                            )),
                    )
                }),
        )
}

/// A slim strip: tile, title and where the work lives.
fn strip(issue: &StackIssue, cx: &App) -> impl IntoElement {
    h_flex()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(STRIP_H))
        .pl(px(9.))
        .pr(px(12.))
        .gap(px(8.))
        .child(tile(issue, 26., cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(isonomic_ui::FontWeight::MEDIUM)
                .text_color(look::ink::strong(cx).opacity(0.9))
                .child(issue.title.clone()),
        )
        .children(
            reference(issue, cx)
                .map(|reference| div().flex_shrink(1.).max_w(relative(0.42)).child(reference)),
        )
}

impl AgentIssueStack {
    fn render_stack(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let count = self.issues.len();
        let front = self.index();
        let behind = count.saturating_sub(1).min(BEHIND);
        let pad = behind as f32 * PEEK;
        let still = cx.reduce_motion();
        let card = cx.theme().background;

        let mut layers: Vec<(f32, AnyElement)> = Vec::new();
        for (ix, issue) in self.issues.iter().enumerate() {
            let rel = (ix + count - front) % count;
            let lap = self.lap(ix);
            let pos = transition(
                ElementId::Name(format!("issue-pos-{}-{lap}", issue.id).into()),
                rel as f32,
                over(420, settle()),
                window,
                cx,
            );
            let shown = transition(
                ElementId::Name(format!("issue-full-{}-{lap}", issue.id).into()),
                if rel == 0 { 1.0_f32 } else { 0. },
                over(240, ease()),
                window,
                cx,
            );
            let (lift, shrink, opacity) = stack_layer(pos);
            if opacity <= 0.001 {
                continue;
            }
            let entity = cx.entity().downgrade();
            let layer = div()
                .id(ElementId::Name(format!("issue-card-{}", issue.id).into()))
                .test_support()
                .role(Role::Group)
                .aria_label(slide_label(ix, count, issue))
                .absolute()
                .top(px(pad - lift))
                .left(relative(shrink / 2.))
                .right(relative(shrink / 2.))
                .h(px(CARD_H * (1. - shrink)))
                .overflow_hidden()
                .rounded(px(14.))
                .bg(card)
                .shadow(if rel == 0 {
                    front_shadow(cx)
                } else {
                    look::surface::resting(cx)
                })
                .opacity(opacity)
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _, cx| {
                    _ = entity.update(cx, |this, cx| this.go(1, cx));
                })
                .when(shown > 0.001, |this| {
                    this.child(
                        div()
                            .w_full()
                            .opacity(shown)
                            .child(full_card(issue, still, cx)),
                    )
                })
                .into_any_element();
            layers.push((pos, layer));
        }

        // The card that just left the front slides down and fades as it goes.
        for (ix, issue) in self.issues.iter().enumerate() {
            let rel = (ix + count - front) % count;
            let leave = Presence::new(
                ElementId::Name(format!("issue-front-{}", issue.id).into()),
                rel == 0,
            )
            .transition(over(280, settle()))
            .sample(window, cx);
            if rel == 0 || count < 2 || rel != count - 1 || !leave.should_render() {
                continue;
            }
            let p = leave.progress;
            layers.push((
                -1.,
                div()
                    .absolute()
                    .top(px(pad + 28. * (1. - p)))
                    .left(relative(0.02 * (1. - p)))
                    .right(relative(0.02 * (1. - p)))
                    .h(px(CARD_H * (0.96 + 0.04 * p)))
                    .overflow_hidden()
                    .rounded(px(14.))
                    .bg(card)
                    .shadow(front_shadow(cx))
                    .opacity(p)
                    .child(full_card(issue, still, cx))
                    .into_any_element(),
            ));
        }

        layers.sort_by(|a, b| b.0.total_cmp(&a.0));
        div()
            .relative()
            .w_full()
            .h(px(pad + CARD_H))
            .children(layers.into_iter().map(|(_, layer)| layer))
            .into_any_element()
    }

    fn render_cycle(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let count = self.issues.len();
        let front = self.index();
        let still = cx.reduce_motion();
        let card = cx.theme().background;
        let open_top = (CYCLE_H - CARD_H) / 2.;

        let mut layers: Vec<(bool, AnyElement)> = Vec::new();
        for (ix, issue) in self.issues.iter().enumerate() {
            let rel = (ix + count - front) % count;
            // top, inset, height, radius, opacity
            let (top, inset, height, radius, opacity) = if rel == 0 {
                (open_top, 0., CARD_H, 14., 1.)
            } else if rel == 1 {
                (CYCLE_H - STRIP_H, 14., STRIP_H, 11., 1.)
            } else if rel == count - 1 {
                (0., 14., STRIP_H, 11., 1.)
            } else if rel * 2 <= count {
                (CYCLE_H, 24., STRIP_H, 11., 0.)
            } else {
                (-STRIP_H, 24., STRIP_H, 11., 0.)
            };
            let key =
                |part: &str| ElementId::Name(format!("issue-cycle-{part}-{}", issue.id).into());
            let morph = over(640, settle());
            let top = transition(key("top"), top, morph.clone(), window, cx);
            let inset = transition(key("inset"), inset, morph.clone(), window, cx);
            let height = transition(key("height"), height, morph.clone(), window, cx);
            let radius = transition(key("radius"), radius, morph, window, cx);
            let opacity = transition(key("fade"), opacity, over(360, ease()), window, cx);
            let open = rel == 0;
            let full = transition(
                key("full"),
                if open { 1.0_f32 } else { 0. },
                if open {
                    over(380, ease()).delay(Duration::from_millis(180))
                } else {
                    over(200, ease())
                },
                window,
                cx,
            );
            let strip_shown = transition(
                key("strip"),
                if open { 0.0_f32 } else { 1. },
                if open {
                    over(160, ease())
                } else {
                    over(260, ease()).delay(Duration::from_millis(240))
                },
                window,
                cx,
            );
            if opacity <= 0.001 {
                continue;
            }
            let entity = cx.entity().downgrade();
            let layer = div()
                .id(ElementId::Name(format!("issue-card-{}", issue.id).into()))
                .test_support()
                .role(if open { Role::Group } else { Role::Button })
                .aria_label(slide_label(ix, count, issue))
                .absolute()
                .top(px(top))
                .left(px(inset))
                .right(px(inset))
                .h(px(height))
                .overflow_hidden()
                .rounded(px(radius))
                .bg(card)
                .shadow(if open {
                    front_shadow(cx)
                } else {
                    look::surface::resting(cx)
                })
                .opacity(opacity)
                .when(!open, |this| {
                    this.cursor_pointer()
                        .on_click(move |_: &ClickEvent, _, cx| {
                            _ = entity.update(cx, |this, cx| this.go_to(ix, cx));
                        })
                })
                .when(strip_shown > 0.001, |this| {
                    this.child(div().opacity(strip_shown).child(strip(issue, cx)))
                })
                .when(full > 0.001, |this| {
                    this.child(
                        div()
                            .absolute()
                            .top(px(6. * (1. - full)))
                            .left(px(-inset))
                            .right(px(-inset))
                            .opacity(full)
                            .child(full_card(issue, still, cx)),
                    )
                })
                .into_any_element();
            layers.push((open, layer));
        }
        // The open card draws over the strips it grows out of.
        layers.sort_by_key(|(open, _)| *open);
        div()
            .relative()
            .w_full()
            .h(px(CYCLE_H))
            .children(layers.into_iter().map(|(_, layer)| layer))
            .into_any_element()
    }

    fn pager_button(
        &self,
        next: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = if next { "next" } else { "previous" };
        let hovered = hover_state(
            ElementId::Name(format!("issue-pager-hover-{name}").into()),
            window,
            cx,
        );
        let lit = transition(
            ElementId::Name(format!("issue-pager-lit-{name}").into()),
            if *hovered.read(cx) { 1.0_f32 } else { 0. },
            over(140, ease()),
            window,
            cx,
        );
        let rest = cx.theme().muted_foreground;
        let ink = look::ink::strong(cx);
        let color = Hsla {
            a: rest.a + (ink.a - rest.a) * lit,
            ..if lit > 0.5 { ink } else { rest }
        };
        let entity = cx.entity().downgrade();
        div()
            .id(ElementId::Name(format!("issue-pager-{name}").into()))
            .test_support()
            .role(Role::Button)
            .aria_label(if next { "Next issue" } else { "Previous issue" })
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded(px(6.))
            .bg(cx.theme().background)
            .shadow(look::surface::resting(cx))
            .cursor_pointer()
            .on_hover(track_hover(hovered))
            .on_click(move |_: &ClickEvent, _, cx| {
                _ = entity.update(cx, |this, cx| this.go(if next { 1 } else { -1 }, cx));
            })
            .child(
                Icon::new(if next {
                    AssetIcon::ChevronRight
                } else {
                    AssetIcon::ChevronLeft
                })
                .size(px(16.))
                .text_color(color),
            )
    }
}

impl Render for AgentIssueStack {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        if focused != self.focused {
            self.focused = focused;
            self.moved = Instant::now();
        }
        let count = self.issues.len();
        let body = if count == 0 {
            div()
                .py(px(40.))
                .w_full()
                .text_center()
                .text_color(look::ink::muted(cx))
                .child("No issues right now")
                .into_any_element()
        } else {
            match self.variant {
                IssueStackVariant::Stack => self.render_stack(window, cx),
                IssueStackVariant::Cycle => self.render_cycle(window, cx),
            }
        };
        let current = self
            .current()
            .map(|issue| slide_label(self.index(), count, issue));
        let entity = cx.entity().downgrade();

        v_flex()
            .id("agent-issue-stack")
            .w_full()
            .max_w(px(MAX_W))
            .items_center()
            .gap(px(14.))
            .text_body()
            .text_color(cx.theme().foreground)
            .on_hover(move |hovered, _, cx| {
                let hovered = *hovered;
                _ = entity.update(cx, |this, cx| {
                    if this.hovered != hovered {
                        this.hovered = hovered;
                        this.moved = Instant::now();
                        cx.notify();
                    }
                });
            })
            .child(
                div()
                    .id("issue-stack-region")
                    .test_support()
                    .track_focus(&self.focus)
                    .key_context("AgentIssueStack")
                    .on_key_down(cx.listener(Self::key_down))
                    .role(Role::Region)
                    .aria_label(match &current {
                        Some(current) => format!("{}, {current}", self.label),
                        None => self.label.to_string(),
                    })
                    .relative()
                    .w_full()
                    .child(body)
                    .when(focused, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(-4.))
                                .bottom(px(-4.))
                                .left(px(-4.))
                                .right(px(-4.))
                                .rounded(px(18.))
                                .border_2()
                                .border_color(cx.theme().ring),
                        )
                    }),
            )
            .when(count > 1, |this| {
                this.child(
                    h_flex()
                        .gap(px(8.))
                        .child(self.pager_button(false, window, cx))
                        .child(
                            div()
                                .min_w(px(44.))
                                .text_center()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(look::ink::muted(cx))
                                .child(count_label(self.index(), count)),
                        )
                        .child(self.pager_button(true, window, cx)),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(title: &str) -> StackIssue {
        StackIssue::new(title.to_lowercase(), title)
    }

    #[test]
    fn slides_and_the_pager_read_their_place() {
        assert_eq!(
            slide_label(1, 6, &issue("Fix sign-up")),
            "2 of 6: Fix sign-up"
        );
        assert_eq!(count_label(4, 6), "5 / 6");
        assert_eq!(count_label(0, 0), "0 / 0");
    }

    #[test]
    fn an_agent_at_work_is_on_it() {
        assert_eq!(
            who_line(&IssueOwner::new("Triage").working(true)),
            "Triage is on it"
        );
        assert_eq!(who_line(&IssueOwner::new("Docs")), "Docs");
    }

    #[test]
    fn every_agent_gets_a_steady_mark_with_enough_pixels() {
        for name in ["Triage", "Coder", "Ledger", "Docs", "", "A"] {
            let pattern = pixel_pattern(name);
            assert_eq!(pattern, pixel_pattern(name));
            assert!(pattern.iter().filter(|on| **on).count() >= 4, "{name}");
        }
        assert_ne!(pixel_pattern("Triage"), pixel_pattern("Coder"));
    }

    #[test]
    fn cards_behind_rise_shrink_and_fade() {
        assert_eq!(stack_layer(0.), (0., 0., 1.));
        let (lift, shrink, opacity) = stack_layer(2.);
        assert_eq!(lift, 20.);
        assert!((shrink - 0.1).abs() < 1e-6);
        assert!((opacity - 0.6).abs() < 1e-6);
        assert_eq!(stack_layer(9.).2, 0.);
        let (_, _, half) = stack_layer(0.5);
        assert!((half - 0.925).abs() < 1e-6);
    }

    #[test]
    fn jumping_takes_the_short_way_round() {
        assert_eq!(short_step(0, 1, 6), 1);
        assert_eq!(short_step(0, 5, 6), -1);
        assert_eq!(short_step(4, 1, 6), 3);
        assert_eq!(short_step(2, 2, 6), 0);
        assert_eq!(short_step(0, 0, 0), 0);
    }

    #[test]
    fn priorities_light_their_bars() {
        assert_eq!(IssuePriority::Low.bars(), 1);
        assert_eq!(IssuePriority::Medium.bars(), 2);
        assert_eq!(IssuePriority::Urgent.bars(), 3);
        assert_eq!(IssuePriority::High.word(), "High");
    }
}
