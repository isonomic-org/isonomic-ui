// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Evaluation case grid: every case of a test run as one cell, read against
//! a baseline.
//!
//! The header carries the pass rate and how far it moved from the baseline.
//! In the grid each cell is a case: passed, failed, regressed, improved, or
//! still running. The switch above it dims everything outside one outcome.
//! Picking a cell, with the pointer or with the arrow keys and Enter, opens
//! what went in, what was expected and what came out. The compare view shows
//! metrics as baseline and candidate bars over the list of regressed cases.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex, look,
    progress::Progress,
    rolling_number::RollingNumber,
    spinner::Spinner,
    tab::{Tab, TabBar},
    tag::Tag,
    tooltip::Tooltip,
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ElementId, FocusHandle, Hsla, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::{inset, panel, shell, shell_header};

/// Cells per row.
const COLUMNS: usize = 12;
/// How long a cell takes to change shape or fade, and the detail to appear.
const SETTLE: Duration = Duration::from_millis(200);

/// Where one case stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStatus {
    /// Not started.
    Pending,
    /// Being graded now.
    Running,
    /// Passed, as it did on the baseline.
    Pass,
    /// Failed, as it did on the baseline.
    Fail,
    /// Passed on the baseline and fails now.
    Regressed,
    /// Failed on the baseline and passes now.
    Improved,
}

impl CaseStatus {
    /// The status in a word.
    pub fn words(self) -> &'static str {
        match self {
            CaseStatus::Pending => "Pending",
            CaseStatus::Running => "Running",
            CaseStatus::Pass => "Pass",
            CaseStatus::Fail => "Fail",
            CaseStatus::Regressed => "Regressed",
            CaseStatus::Improved => "Improved",
        }
    }

    /// The case has a result.
    pub fn finished(self) -> bool {
        !matches!(self, CaseStatus::Pending | CaseStatus::Running)
    }

    /// The case counts toward the pass rate.
    pub fn passed(self) -> bool {
        matches!(self, CaseStatus::Pass | CaseStatus::Improved)
    }
}

/// One test case and how it went.
#[derive(Debug, Clone)]
pub struct EvaluationCase {
    name: SharedString,
    status: CaseStatus,
    score: Option<f64>,
    input: Option<SharedString>,
    expected: Option<SharedString>,
    actual: Option<SharedString>,
    reason: Option<SharedString>,
}

impl EvaluationCase {
    /// A case by name and status.
    pub fn new(name: impl Into<SharedString>, status: CaseStatus) -> Self {
        Self {
            name: name.into(),
            status,
            score: None,
            input: None,
            expected: None,
            actual: None,
            reason: None,
        }
    }

    /// The grader's score, 0 to 1.
    pub fn score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    /// What went in.
    pub fn input(mut self, input: impl Into<SharedString>) -> Self {
        self.input = Some(input.into());
        self
    }

    /// What should have come out.
    pub fn expected(mut self, expected: impl Into<SharedString>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// What came out.
    pub fn actual(mut self, actual: impl Into<SharedString>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    /// Why the grader scored it this way.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Its name.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// Where it stands.
    pub fn status(&self) -> CaseStatus {
        self.status
    }
}

/// Which outcome the grid is narrowed to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CaseFilter {
    /// Every case.
    #[default]
    All,
    /// Cases that fail now, regressed ones included.
    Failed,
    /// Cases that passed on the baseline and fail now.
    Regressed,
    /// Cases that failed on the baseline and pass now.
    Improved,
}

impl CaseFilter {
    /// Every filter, in the order the switch shows them.
    pub const EVERY: [CaseFilter; 4] = [
        CaseFilter::All,
        CaseFilter::Failed,
        CaseFilter::Regressed,
        CaseFilter::Improved,
    ];

    /// The filter in a word.
    pub fn words(self) -> &'static str {
        match self {
            CaseFilter::All => "All",
            CaseFilter::Failed => "Failed",
            CaseFilter::Regressed => "Regressed",
            CaseFilter::Improved => "Improved",
        }
    }

    /// A case in this status stays lit under the filter.
    pub fn matches(self, status: CaseStatus) -> bool {
        match self {
            CaseFilter::All => true,
            CaseFilter::Failed => matches!(status, CaseStatus::Fail | CaseStatus::Regressed),
            CaseFilter::Regressed => status == CaseStatus::Regressed,
            CaseFilter::Improved => status == CaseStatus::Improved,
        }
    }
}

type Format = Rc<dyn Fn(f64) -> String>;

/// One measure compared between the baseline and the candidate.
#[derive(Clone)]
pub struct EvaluationMetric {
    label: SharedString,
    baseline: f64,
    candidate: f64,
    higher_is_better: bool,
    format: Format,
}

impl EvaluationMetric {
    /// A measure where a higher value is the better one.
    pub fn new(label: impl Into<SharedString>, baseline: f64, candidate: f64) -> Self {
        Self {
            label: label.into(),
            baseline,
            candidate,
            higher_is_better: true,
            format: Rc::new(plain_number),
        }
    }

    /// A lower value is the better one, as with time or cost.
    pub fn lower_is_better(mut self) -> Self {
        self.higher_is_better = false;
        self
    }

    /// How a value is written, such as with a unit after it.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
}

/// How many cases there are, have a result, and passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    /// Every case.
    pub total: usize,
    /// Cases with a result.
    pub finished: usize,
    /// Cases that pass now.
    pub passed: usize,
}

impl Tally {
    /// The share of finished cases that passed, 0 to 1.
    pub fn rate(self) -> f64 {
        if self.finished == 0 {
            0.
        } else {
            self.passed as f64 / self.finished as f64
        }
    }

    /// Cases still pending or running.
    pub fn in_flight(self) -> usize {
        self.total - self.finished
    }
}

/// Count the cases.
pub fn tally(cases: &[EvaluationCase]) -> Tally {
    Tally {
        total: cases.len(),
        finished: cases.iter().filter(|case| case.status.finished()).count(),
        passed: cases.iter().filter(|case| case.status.passed()).count(),
    }
}

/// A share as a percentage with one decimal: "87.5%".
pub fn percent(share: f64) -> String {
    format!("{:.1}%", share * 100.)
}

/// A value with at most two decimals and no trailing zeros.
pub fn plain_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Where an arrow key takes the cursor in a grid of `count` cells, twelve to
/// a row. `None` for any other key.
pub fn step_cell(at: usize, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let last = count - 1;
    let next = match key {
        "right" => at + 1,
        "left" => at.saturating_sub(1),
        "down" => at + COLUMNS,
        "up" => at.saturating_sub(COLUMNS),
        "home" => 0,
        "end" => last,
        _ => return None,
    };
    Some(next.min(last))
}

/// Which way a measure moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// It did not move.
    Same,
    /// It moved the good way, by this much.
    Better(f64),
    /// It moved the bad way, by this much.
    Worse(f64),
}

/// Compare a candidate value with its baseline.
pub fn metric_change(baseline: f64, candidate: f64, higher_is_better: bool) -> Change {
    let moved = candidate - baseline;
    if moved.abs() < f64::EPSILON {
        Change::Same
    } else if (moved > 0.) == higher_is_better {
        Change::Better(moved.abs())
    } else {
        Change::Worse(moved.abs())
    }
}

type SelectHandler = Rc<dyn Fn(usize, &EvaluationCase, &mut Window, &mut App)>;
type FilterHandler = Rc<dyn Fn(CaseFilter, &mut Window, &mut App)>;

/// The results of one evaluation run.
#[derive(IntoElement)]
pub struct EvaluationCaseGrid {
    id: ElementId,
    style: StyleRefinement,
    suite: SharedString,
    label: Option<SharedString>,
    cases: Vec<EvaluationCase>,
    baseline_pass_rate: Option<f64>,
    metrics: Vec<EvaluationMetric>,
    compare: bool,
    filter: CaseFilter,
    on_select: Option<SelectHandler>,
    on_filter: Option<FilterHandler>,
}

impl EvaluationCaseGrid {
    /// Create the grid for a suite and its cases.
    pub fn new(
        id: impl Into<ElementId>,
        suite: impl Into<SharedString>,
        cases: Vec<EvaluationCase>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            suite: suite.into(),
            label: None,
            cases,
            baseline_pass_rate: None,
            metrics: Vec::new(),
            compare: false,
            filter: CaseFilter::All,
            on_select: None,
            on_filter: None,
        }
    }

    /// What is compared, such as "recipe v8 against v7".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The baseline's pass rate, 0 to 1, for the change beside the big number.
    pub fn baseline_pass_rate(mut self, rate: f64) -> Self {
        self.baseline_pass_rate = Some(rate);
        self
    }

    /// Show the compare view: these metrics over the regressed cases.
    pub fn compare(mut self, metrics: Vec<EvaluationMetric>) -> Self {
        self.metrics = metrics;
        self.compare = true;
        self
    }

    /// The filter the grid starts on. All by default.
    pub fn filter(mut self, filter: CaseFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Run when a case is picked, with its place in the list.
    pub fn on_select(
        mut self,
        handler: impl Fn(usize, &EvaluationCase, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when the filter changes.
    pub fn on_filter(
        mut self,
        handler: impl Fn(CaseFilter, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_filter = Some(Rc::new(handler));
        self
    }
}

impl Styled for EvaluationCaseGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct GridState {
    filter: CaseFilter,
    selected: Option<usize>,
    /// The cell the arrow keys are on.
    cursor: usize,
    /// The last pick came from the keyboard, so the detail does not move in.
    by_keys: bool,
    focus: FocusHandle,
}

impl RenderOnce for EvaluationCaseGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let start_filter = self.filter;
        let state = window.use_keyed_state(part("state".into()), cx, |_, cx| GridState {
            filter: start_filter,
            selected: None,
            cursor: 0,
            by_keys: false,
            focus: cx.focus_handle().tab_stop(true),
        });
        let count = self.cases.len();
        let (filter, selected, cursor, by_keys, focus) = {
            let state = state.read(cx);
            (
                state.filter,
                state.selected.filter(|ix| *ix < count),
                state.cursor.min(count.saturating_sub(1)),
                state.by_keys,
                state.focus.clone(),
            )
        };
        let focused = focus.is_focused(window);
        let still = cx.reduce_motion();
        let counts = tally(&self.cases);
        let rate = counts.rate();
        let moved = self.baseline_pass_rate.map(|baseline| rate - baseline);

        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let accent = theme.info;
        let (good, bad, warn) = (theme.success, theme.danger, theme.warning);
        let (on_bad, on_warn) = (theme.danger_foreground, theme.warning_foreground);
        let ring = theme.ring;
        let mono = theme.mono_font_family.clone();
        let hover = look::ink::wash(cx);
        let slot = look::ink::wash_strong(cx);

        let pick = {
            let state = state.clone();
            let on_select = self.on_select.clone();
            let cases = self.cases.clone();
            Rc::new(
                move |ix: usize, by_keys: bool, window: &mut Window, cx: &mut App| {
                    state.update(cx, |state, cx| {
                        state.selected = Some(ix);
                        state.cursor = ix;
                        state.by_keys = by_keys;
                        cx.notify();
                    });
                    if let (Some(handler), Some(case)) = (&on_select, cases.get(ix)) {
                        handler(ix, case, window, cx);
                    }
                },
            )
        };

        let pill = |status: CaseStatus| {
            match status {
                CaseStatus::Pending => Tag::secondary(),
                CaseStatus::Running | CaseStatus::Improved => Tag::info(),
                CaseStatus::Pass => Tag::success(),
                CaseStatus::Fail => Tag::danger(),
                CaseStatus::Regressed => Tag::warning(),
            }
            .rounded_full()
            .flex_none()
            .h(px(20.))
            .px_2()
            .text_size(px(11.))
        };

        let cells: Vec<_> = self
            .cases
            .iter()
            .enumerate()
            .map(|(ix, case)| {
                let (fill, mark) = match case.status {
                    CaseStatus::Pending => (ink.opacity(0.07), ink),
                    CaseStatus::Running | CaseStatus::Improved => (accent.opacity(0.2), accent),
                    CaseStatus::Pass => (good.opacity(0.15), good),
                    CaseStatus::Fail => (bad.opacity(0.8), on_bad),
                    CaseStatus::Regressed => (warn.opacity(0.85), on_warn),
                };
                let glyph = |icon: CatalogIcon| Icon::new(icon).size(px(12.)).into_any_element();
                let icon = match case.status {
                    CaseStatus::Pending => None,
                    CaseStatus::Running => {
                        Some(Spinner::new().xsmall().color(mark).into_any_element())
                    }
                    CaseStatus::Pass => Some(glyph(CatalogIcon::Checkmark)),
                    CaseStatus::Fail => Some(glyph(CatalogIcon::Close)),
                    CaseStatus::Regressed => Some(glyph(CatalogIcon::ArrowDown)),
                    CaseStatus::Improved => Some(glyph(CatalogIcon::ArrowUp)),
                };
                let lit = if filter.matches(case.status) { 1. } else { 0.2 };
                let round = if selected == Some(ix) { 1. } else { 0. };
                let (lit, round) = if still {
                    (lit, round)
                } else {
                    (
                        transition(
                            part(format!("cell-{ix}-lit")),
                            lit,
                            Transition::new(SETTLE),
                            window,
                            cx,
                        ),
                        transition(
                            part(format!("cell-{ix}-round")),
                            round,
                            Transition::new(SETTLE),
                            window,
                            cx,
                        ),
                    )
                };
                let words: SharedString = format!("{}: {}", case.name, case.status.words()).into();
                let tip = words.clone();
                let pick = pick.clone();
                div()
                    .id(part(format!("cell-{ix}")))
                    .test_support()
                    .relative()
                    .aspect_square()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if focused && cursor == ix {
                        ring
                    } else {
                        ring.opacity(0.)
                    })
                    .cursor_pointer()
                    .role(Role::Button)
                    .aria_label(if selected == Some(ix) {
                        format!("{words}, selected")
                    } else {
                        words.to_string()
                    })
                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    .on_click(move |_, window, cx| pick(ix, false, window, cx))
                    .child(
                        div()
                            .absolute()
                            // Selection changes the shape, not an outline: the
                            // cell draws in a little and rounds off.
                            .top(relative(0.05 * round))
                            .bottom(relative(0.05 * round))
                            .left(relative(0.05 * round))
                            .right(relative(0.05 * round))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5. + 20. * round))
                            .bg(fill)
                            .text_color(mark)
                            .opacity(lit)
                            .children(icon),
                    )
            })
            .collect();

        let grid = {
            let state = state.clone();
            let pick = pick.clone();
            div()
                .id(part("grid".into()))
                .test_support()
                .track_focus(&focus)
                .grid()
                .grid_cols(COLUMNS as u16)
                .gap_1()
                .role(Role::Group)
                .aria_label(format!(
                    "Cases: {} of {} passed. Arrow keys move between cases, Enter opens one.",
                    counts.passed, counts.total
                ))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if count == 0 || event.keystroke.modifiers.modified() {
                        return;
                    }
                    let at = state.read(cx).cursor.min(count - 1);
                    match event.keystroke.key.as_str() {
                        "enter" | "space" => pick(at, true, window, cx),
                        key => {
                            let Some(next) = step_cell(at, count, key) else {
                                return;
                            };
                            state.update(cx, |state, cx| {
                                state.cursor = next;
                                cx.notify();
                            });
                        }
                    }
                    cx.stop_propagation();
                })
                .children(cells)
        };

        let switch = {
            let on_filter = self.on_filter.clone();
            let at = CaseFilter::EVERY
                .iter()
                .position(|one| *one == filter)
                .unwrap_or(0);
            TabBar::new(part("filter".into()))
                .segmented()
                .small()
                .selected_index(at)
                .on_click(move |ix: &usize, window, cx| {
                    let next = CaseFilter::EVERY[(*ix).min(3)];
                    state.update(cx, |state, cx| {
                        state.filter = next;
                        cx.notify();
                    });
                    if let Some(handler) = &on_filter {
                        handler(next, window, cx);
                    }
                })
                .children(CaseFilter::EVERY.iter().enumerate().map(|(ix, one)| {
                    let matching = self
                        .cases
                        .iter()
                        .filter(|case| one.matches(case.status))
                        .count();
                    Tab::new().label(one.words()).child(
                        div()
                            .ml(px(6.))
                            .font_family(mono.clone())
                            .text_caption()
                            .opacity(0.6)
                            .child(
                                RollingNumber::new(
                                    part(format!("count-{ix}")),
                                    matching.to_string(),
                                )
                                .line_height(px(16.)),
                            ),
                    )
                }))
        };

        let compare = self.compare.then(|| {
            let regressed: Vec<(usize, &EvaluationCase)> = self
                .cases
                .iter()
                .enumerate()
                .filter(|(_, case)| case.status == CaseStatus::Regressed)
                .collect();
            let metrics = self.metrics.iter().enumerate().map(|(ix, metric)| {
                let most = metric.baseline.max(metric.candidate).max(f64::EPSILON);
                let change =
                    metric_change(metric.baseline, metric.candidate, metric.higher_is_better);
                let verdict = match change {
                    Change::Same => Tag::secondary().child("No change"),
                    Change::Better(by) => {
                        Tag::success().child(format!("Better · {}", (metric.format)(by)))
                    }
                    Change::Worse(by) => {
                        Tag::danger().child(format!("Worse · {}", (metric.format)(by)))
                    }
                };
                let bar = |name: &'static str, value: f64, color: Hsla| {
                    let written = (metric.format)(value);
                    h_flex()
                        .gap_2()
                        .py(px(2.))
                        .child(
                            div()
                                .w(px(64.))
                                .flex_none()
                                .text_caption()
                                .text_color(quiet)
                                .child(name),
                        )
                        .child(
                            div().flex_1().min_w_0().child(
                                Progress::new(part(format!("metric-{ix}-{name}")))
                                    .striped(true)
                                    .color(color)
                                    .value((value / most * 100.) as f32)
                                    .accessibility_label(format!(
                                        "{}, {}: {written}",
                                        metric.label,
                                        name.to_lowercase()
                                    )),
                            ),
                        )
                        .child(
                            div()
                                .w(px(64.))
                                .flex_none()
                                .flex()
                                .justify_end()
                                .font_family(mono.clone())
                                .text_caption()
                                .child(written),
                        )
                };
                v_flex()
                    .child(
                        h_flex()
                            .mb(px(6.))
                            .justify_between()
                            .gap_2()
                            .text_body()
                            .child(div().min_w_0().truncate().child(metric.label.clone()))
                            .child(
                                verdict
                                    .rounded_full()
                                    .flex_none()
                                    .h(px(20.))
                                    .px_2()
                                    .text_size(px(11.)),
                            ),
                    )
                    .child(bar("Baseline", metric.baseline, quiet))
                    .child(bar("Candidate", metric.candidate, accent))
            });
            let rows = regressed.iter().map(|(ix, case)| {
                let ix = *ix;
                let pick = pick.clone();
                h_flex()
                    .id(part(format!("regressed-{ix}")))
                    .test_support()
                    .w_full()
                    .gap_2()
                    .p_2()
                    .rounded(px(12.))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .when(selected == Some(ix), |this| this.bg(hover))
                    .role(Role::Button)
                    .aria_label(format!("{}: regressed", case.name))
                    .tab_index(0)
                    .on_click(move |event, window, cx| pick(ix, event.is_keyboard(), window, cx))
                    .child(
                        Icon::new(CatalogIcon::ArrowDown)
                            .size(px(14.))
                            .text_color(warn),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .font_family(mono.clone())
                            .text_body()
                            .child(case.name.clone()),
                    )
                    .children(case.score.map(|score| {
                        div()
                            .flex_none()
                            .font_family(mono.clone())
                            .text_caption()
                            .text_color(quiet)
                            .child(format!("{score:.2}"))
                    }))
            });
            v_flex()
                .child(v_flex().gap_3().children(metrics))
                .child(
                    div()
                        .mt_4()
                        .mb_1()
                        .text_body()
                        .text_color(quiet)
                        .child(format!("Regressed cases · {}", regressed.len())),
                )
                .when(regressed.is_empty(), |this| {
                    this.child(
                        div()
                            .py_2()
                            .text_body()
                            .text_color(quiet)
                            .child("Nothing regressed."),
                    )
                })
                .children(rows)
        });

        let detail = selected.map(|ix| {
            let case = &self.cases[ix];
            let field = |name: &'static str, value: &Option<SharedString>| {
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .p_2()
                    .rounded(px(8.))
                    .bg(slot)
                    .child(div().mb_1().text_caption().text_color(quiet).child(name))
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_caption()
                            .child(value.clone().unwrap_or_else(|| "—".into())),
                    )
            };
            let body = inset(cx)
                .id(part("detail".into()))
                .relative()
                .mt_3()
                .p_3()
                .role(Role::Group)
                .aria_label(format!("{}: {}", case.name, case.status.words()))
                .child(
                    h_flex()
                        .mb_2()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .font_family(mono.clone())
                                .text_body()
                                .child(case.name.clone()),
                        )
                        .child(pill(case.status).child(case.status.words()))
                        .children(case.score.map(|score| {
                            div()
                                .flex_none()
                                .font_family(mono.clone())
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("score {score:.2}"))
                        })),
                )
                .child(
                    h_flex()
                        .items_start()
                        .gap_2()
                        .child(field("Input", &case.input))
                        .child(field("Expected", &case.expected))
                        .child(field("Actual", &case.actual)),
                )
                .children(
                    case.reason
                        .clone()
                        .map(|reason| div().mt_2().text_body().text_color(quiet).child(reason)),
                );
            if still || by_keys {
                body.into_any_element()
            } else {
                body.with_animation(
                    part(format!("detail-in-{ix}")),
                    Animation::new(SETTLE),
                    |this, time| {
                        let shown = 1. - (1. - time).powi(3);
                        this.opacity(shown).top(px(4. * (1. - shown)))
                    },
                )
                .into_any_element()
            }
        });

        let subtitle = h_flex()
            .gap_1()
            .children(self.label.clone().map(|label| format!("{label} ·")))
            .map(|this| {
                if counts.in_flight() > 0 {
                    this.child(
                        RollingNumber::new(
                            part("in-flight".into()),
                            counts.in_flight().to_string(),
                        )
                        .line_height(px(20.)),
                    )
                    .child(format!("of {} running", counts.total))
                } else {
                    this.child(format!("{} cases", counts.total))
                }
            });
        let aside = v_flex()
            .items_end()
            .child(
                div()
                    .font_family(mono.clone())
                    .text_size(px(24.))
                    .font_medium()
                    .child(
                        RollingNumber::new(part("rate".into()), percent(rate)).line_height(px(30.)),
                    ),
            )
            .children(moved.map(|moved| {
                h_flex()
                    .gap(px(2.))
                    .font_family(mono.clone())
                    .text_caption()
                    .text_color(if moved >= 0. { good } else { bad })
                    .child(
                        Icon::new(if moved >= 0. {
                            CatalogIcon::ArrowUp
                        } else {
                            CatalogIcon::ArrowDown
                        })
                        .size(px(12.)),
                    )
                    .child(
                        RollingNumber::new(
                            part("moved".into()),
                            format!("{:.1}", (moved * 100.).abs()),
                        )
                        .line_height(px(16.)),
                    )
                    .child(div().ml(px(4.)).child("pts vs baseline"))
            }));
        let spoken = format!(
            "Evaluation results: {}, {} passed{}",
            self.suite,
            percent(rate),
            match moved {
                Some(moved) if moved >= 0. =>
                    format!(", up {:.1} points on the baseline", moved * 100.),
                Some(moved) => format!(", down {:.1} points on the baseline", -moved * 100.),
                None => String::new(),
            }
        );

        shell(cx)
            .id(self.id.clone())
            .max_w(px(512.))
            .role(Role::Region)
            .aria_label(spoken)
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Chemistry)),
                div().font_family(mono).child(self.suite.clone()),
                Some(subtitle.into_any_element()),
                Some(aside.into_any_element()),
                cx,
            ))
            .child(
                panel(cx)
                    .p_3()
                    .map(|this| match compare {
                        Some(compare) => this.child(compare),
                        None => this.child(div().mb_3().child(switch)).child(grid),
                    })
                    .children(detail),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Vec<EvaluationCase> {
        [
            CaseStatus::Pass,
            CaseStatus::Pass,
            CaseStatus::Improved,
            CaseStatus::Fail,
            CaseStatus::Regressed,
            CaseStatus::Running,
            CaseStatus::Pending,
        ]
        .into_iter()
        .enumerate()
        .map(|(ix, status)| EvaluationCase::new(format!("case-{ix}"), status))
        .collect()
    }

    #[test]
    fn the_pass_rate_counts_only_finished_cases() {
        let counts = tally(&run());
        assert_eq!(
            counts,
            Tally {
                total: 7,
                finished: 5,
                passed: 3
            }
        );
        assert_eq!(counts.in_flight(), 2);
        assert_eq!(percent(counts.rate()), "60.0%");
        assert_eq!(tally(&[]).rate(), 0.);
    }

    #[test]
    fn failed_takes_in_regressions() {
        let lit = |filter: CaseFilter| {
            run()
                .iter()
                .filter(|case| filter.matches(case.status()))
                .count()
        };
        assert_eq!(lit(CaseFilter::All), 7);
        assert_eq!(lit(CaseFilter::Failed), 2);
        assert_eq!(lit(CaseFilter::Regressed), 1);
        assert_eq!(lit(CaseFilter::Improved), 1);
    }

    #[test]
    fn arrows_walk_the_grid_and_stop_at_its_ends() {
        assert_eq!(step_cell(0, 30, "right"), Some(1));
        assert_eq!(step_cell(0, 30, "left"), Some(0));
        assert_eq!(step_cell(3, 30, "down"), Some(15));
        assert_eq!(step_cell(20, 30, "down"), Some(29));
        assert_eq!(step_cell(15, 30, "up"), Some(3));
        assert_eq!(step_cell(3, 30, "up"), Some(0));
        assert_eq!(step_cell(7, 30, "home"), Some(0));
        assert_eq!(step_cell(7, 30, "end"), Some(29));
        assert_eq!(step_cell(7, 30, "a"), None);
        assert_eq!(step_cell(0, 0, "right"), None);
    }

    #[test]
    fn a_measure_reads_better_the_way_it_should_move() {
        assert_eq!(metric_change(0.8, 0.9, true), Change::Better(0.9 - 0.8));
        assert_eq!(metric_change(0.9, 0.8, true), Change::Worse(0.9 - 0.8));
        assert_eq!(metric_change(420., 380., false), Change::Better(40.));
        assert_eq!(metric_change(380., 420., false), Change::Worse(40.));
        assert_eq!(metric_change(5., 5., true), Change::Same);
        assert_eq!(plain_number(40.), "40");
        assert_eq!(plain_number(0.125), "0.13");
    }
}
