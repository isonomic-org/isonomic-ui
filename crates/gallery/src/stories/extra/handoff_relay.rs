// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the handoff relay.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use isonomic_ui_extra::handoff_relay::{
    Handoff, HandoffItem, HandoffRelay, ItemKind, RelayTask, RelayTaskStatus, RelayWorker,
};

/// How long a pretend worker takes before it hands a task on.
const WORK_FOR: Duration = Duration::from_secs(3);

/// A month-end relay: one worker collects, one prepares entries, one checks.
/// Pushing a card on to a later worker starts it there; a moment later it is
/// handed on with what went along.
pub struct HandoffRelayStory {
    tasks: Vec<RelayTask>,
    handoffs: Vec<Handoff>,
    restored: Vec<(SharedString, SharedString)>,
    last: SharedString,
    _work: Option<Task<()>>,
}

impl HandoffRelayStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            tasks: vec![
                RelayTask::new("vat", "VAT return for Q1")
                    .icon(IconName::FileText)
                    .with("prepare", "Drafting"),
                RelayTask::new("recon", "Supplier statement match")
                    .icon(IconName::Search)
                    .status(RelayTaskStatus::Done),
                RelayTask::new("payroll", "Payroll journal")
                    .icon(IconName::User)
                    .status(RelayTaskStatus::Warning),
                RelayTask::new("assets", "Fixed asset register").icon(IconName::Building2),
                RelayTask::new("accruals", "Month-end accruals").icon(IconName::Calendar),
            ],
            handoffs: vec![
                Handoff::new("h1", "collect", "prepare", "recon")
                    .item(HandoffItem::new("i1", "statements.zip").kind(ItemKind::File))
                    .item(HandoffItem::new("i2", "14 suppliers").kind(ItemKind::Source)),
                Handoff::new("h2", "collect", "prepare", "payroll")
                    .item(HandoffItem::new("i1", "payroll-march.csv").kind(ItemKind::File))
                    .item(
                        HandoffItem::new("i2", "Two leavers")
                            .kind(ItemKind::Fact)
                            .lost("The HR export stopped at the 20th"),
                    ),
                Handoff::new("h3", "collect", "prepare", "vat")
                    .item(HandoffItem::new("i1", "sales-ledger.csv").kind(ItemKind::File))
                    .item(HandoffItem::new("i2", "Use the flat rate").kind(ItemKind::Decision))
                    .item(HandoffItem::new("i3", "3 credit notes").kind(ItemKind::Fact)),
                Handoff::new("h4", "prepare", "check", "recon")
                    .item(HandoffItem::new("i1", "Matched list").kind(ItemKind::File))
                    .item(HandoffItem::new("i2", "2 open items").kind(ItemKind::Fact)),
                Handoff::new("h5", "prepare", "check", "payroll")
                    .item(HandoffItem::new("i1", "Journal draft").kind(ItemKind::File))
                    .item(
                        HandoffItem::new("i2", "Leaver pay")
                            .kind(ItemKind::Fact)
                            .lost("Never reached this step"),
                    ),
            ],
            restored: Vec::new(),
            last: "Nothing yet".into(),
            _work: None,
        })
    }

    fn assign(
        &mut self,
        task: &SharedString,
        worker: &SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.tasks.iter_mut().find(|slot| slot.id() == task) {
            *slot = slot.clone().with(worker.clone(), "Working");
        }
        self.last = format!("Gave {task} to {worker}").into();
        let (task, worker) = (task.clone(), worker.clone());
        self._work = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(WORK_FOR).await;
            this.update(cx, |this, cx| {
                let next = match worker.as_ref() {
                    "collect" => Some("prepare"),
                    "prepare" => Some("check"),
                    _ => None,
                };
                if let Some(slot) = this.tasks.iter_mut().find(|slot| slot.id() == &task) {
                    *slot = match next {
                        Some(next) => slot.clone().with(next, "Starting"),
                        None => slot.clone().status(RelayTaskStatus::Done),
                    };
                }
                if let Some(next) = next {
                    let id = format!("h{}", this.handoffs.len() + 1);
                    this.handoffs.push(
                        Handoff::new(id, worker.clone(), next, task.clone())
                            .item(HandoffItem::new("i1", "Working notes").kind(ItemKind::Note))
                            .item(HandoffItem::new("i2", "Totals agreed").kind(ItemKind::Fact)),
                    );
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

impl Story for HandoffRelayStory {
    fn title() -> &'static str {
        "Handoff relay"
    }

    fn description() -> &'static str {
        "A task passed from worker to worker."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for HandoffRelayStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (assign_story, restore_story) = (cx.entity(), cx.entity());
        let mut relay = HandoffRelay::new("close-relay")
            .worker(
                RelayWorker::new("collect", "Collector")
                    .model("ledger-small")
                    .about("Gathers statements, exports and anything the month needs.")
                    .cost(0.41),
            )
            .worker(
                RelayWorker::new("prepare", "Preparer")
                    .model("ledger-large")
                    .about("Turns what was gathered into entries and drafts.")
                    .cost(1.12),
            )
            .worker(
                RelayWorker::new("check", "Checker")
                    .model("ledger-large")
                    .about("Ties every entry back to its source before it posts.")
                    .cost(0.36),
            );
        for task in &self.tasks {
            relay = relay.task(task.clone());
        }
        for handoff in &self.handoffs {
            let mut handoff = handoff.clone();
            for (handoff_id, item_id) in &self.restored {
                if handoff.id() == handoff_id {
                    handoff = handoff.restore(item_id);
                }
            }
            relay = relay.handoff(handoff);
        }
        let relay = relay
            .on_assign(move |task, worker, window, cx| {
                assign_story.update(cx, |this, cx| this.assign(task, worker, window, cx))
            })
            .on_restore(move |handoff, item, _, cx| {
                restore_story.update(cx, |this, cx| {
                    this.restored.push((handoff.clone(), item.clone()));
                    this.last = format!("Added back {item} in {handoff}").into();
                    cx.notify();
                })
            });

        v_flex().gap_3().child(relay).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
