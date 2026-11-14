//! Gallery story for the findings feed.

use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use gpui_component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui_extra::findings_feed::{Finding, FindingWorker, FindingsFeed, Severity};

/// How often the pretend workers move on.
const BEAT: Duration = Duration::from_millis(1500);

/// What the pretend workers turn up, in order: worker, severity, title, place.
const LATER: [(usize, Severity, &str, &str); 5] = [
    (
        1,
        Severity::Warning,
        "Duplicate supplier bank details",
        "suppliers.csv:204",
    ),
    (
        3,
        Severity::Note,
        "Rounding differs from the policy",
        "fx-rates.csv:12",
    ),
    (
        0,
        Severity::Critical,
        "Payment approved by its requester",
        "approvals.log:881",
    ),
    (
        2,
        Severity::Warning,
        "Invoice dated after its payment",
        "ap-2026-03.csv:57",
    ),
    (
        3,
        Severity::Note,
        "Two cost centres share a code",
        "centres.csv:9",
    ),
];

/// Four pretend audit workers checking a month of payables. Every beat they
/// move on, and now and then one finds something.
pub struct FindingsFeedStory {
    workers: Vec<(
        SharedString,
        SharedString,
        IconName,
        usize,
        usize,
        Vec<SharedString>,
    )>,
    findings: Vec<Finding>,
    beat: usize,
    last: SharedString,
    _beat: Task<()>,
}

impl FindingsFeedStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let now = Instant::now();
            let ago = |secs| now.checked_sub(Duration::from_secs(secs)).unwrap_or(now);
            let steps = |steps: &[&str]| {
                steps
                    .iter()
                    .map(|step| SharedString::from(step.to_string()))
                    .collect()
            };
            let beat = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(BEAT).await;
                    if this
                        .update(cx, |this: &mut Self, cx| this.step(cx))
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Self {
                workers: vec![
                    (
                        "controls".into(),
                        "Controls".into(),
                        IconName::Settings2,
                        7,
                        20,
                        steps(&[
                            "Checking who approved what",
                            "Reading the approval log",
                            "Comparing limits",
                        ]),
                    ),
                    (
                        "duplicates".into(),
                        "Duplicates".into(),
                        IconName::Copy,
                        5,
                        16,
                        steps(&[
                            "Matching supplier records",
                            "Comparing bank details",
                            "Scanning invoice numbers",
                        ]),
                    ),
                    (
                        "dates".into(),
                        "Dates".into(),
                        IconName::Calendar,
                        3,
                        12,
                        steps(&[
                            "Lining up invoice and payment dates",
                            "Checking period cut-off",
                        ]),
                    ),
                    (
                        "rates".into(),
                        "Rates".into(),
                        IconName::ChartPie,
                        4,
                        10,
                        steps(&["Recomputing currency conversions", "Checking rounding"]),
                    ),
                ],
                findings: vec![
                    Finding::new(
                        "f1",
                        "Payment split to stay under a limit",
                        Severity::Critical,
                        "controls",
                        ago(8),
                    )
                    .location("ap-2026-03.csv:311"),
                    Finding::new(
                        "f2",
                        "Same invoice paid twice",
                        Severity::Warning,
                        "duplicates",
                        ago(5),
                    )
                    .location("ap-2026-03.csv:88"),
                    Finding::new(
                        "f3",
                        "Old rate used for one conversion",
                        Severity::Note,
                        "rates",
                        ago(50),
                    )
                    .location("fx-rates.csv:3"),
                ],
                beat: 0,
                last: "Nothing opened yet".into(),
                _beat: beat,
            }
        })
    }

    fn step(&mut self, cx: &mut Context<Self>) {
        self.beat += 1;
        for (ix, worker) in self.workers.iter_mut().enumerate() {
            if (self.beat + ix).is_multiple_of(2) && worker.3 < worker.4 {
                worker.3 += 1;
            }
        }
        if self.beat.is_multiple_of(3)
            && let Some((worker, severity, title, place)) = LATER.get(self.beat / 3 - 1)
        {
            let id = self.workers[*worker].0.clone();
            self.findings.push(
                Finding::new(
                    format!("later-{}", self.beat),
                    *title,
                    *severity,
                    id,
                    Instant::now(),
                )
                .location(*place),
            );
        }
        cx.notify();
    }
}

impl Render for FindingsFeedStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let mut feed = FindingsFeed::new("payables-audit").title("Payables audit");
        for (id, name, icon, done, total, steps) in &self.workers {
            let step = steps[(done / 3) % steps.len()].clone();
            feed = feed.worker(
                FindingWorker::new(id.clone(), name.clone(), *done, *total)
                    .icon(icon.clone())
                    .step(step),
            );
        }
        for finding in &self.findings {
            feed = feed.finding(finding.clone());
        }
        let feed = feed.on_open(move |finding, _, cx| {
            story.update(cx, |this, cx| {
                this.last = format!("Opened: {}", finding.title()).into();
                cx.notify();
            })
        });

        v_flex().gap_3().child(feed).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
