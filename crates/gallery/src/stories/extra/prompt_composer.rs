//! Gallery story for the prompt composer.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window, div, px,
};
use isonomic_ui_extra::prompt_composer::{
    AttachKind, ComposerAttachment, ComposerChoice, PromptComposer, PromptComposerEvent,
    SuggestTrigger,
};

/// Commands offered after "/".
const COMMANDS: [(&str, &str, &str); 4] = [
    ("close", "close-month", "Close the month"),
    ("summary", "summary", "Summarise the books"),
    ("chase", "chase", "Chase unpaid invoices"),
    ("reconcile", "reconcile", "Match the bank feed"),
];

/// People offered after "@".
const PEOPLE: [(&str, &str); 3] = [
    ("dana", "Dana Reyes"),
    ("omar", "Omar Haddad"),
    ("li", "Li Chen"),
];

fn matches(trigger: SuggestTrigger, query: &str) -> Vec<ComposerChoice> {
    let query = query.to_lowercase();
    match trigger {
        SuggestTrigger::Slash => COMMANDS
            .iter()
            .filter(|(word, _, _)| word.starts_with(&query))
            .map(|(word, id, note)| ComposerChoice::new(*id, *word).note(*note))
            .collect(),
        SuggestTrigger::Mention => PEOPLE
            .iter()
            .filter(|(_, name)| {
                name.to_lowercase()
                    .split(' ')
                    .any(|part| part.starts_with(&query))
            })
            .map(|(id, name)| ComposerChoice::new(*id, name.replace(' ', "")).note(*name))
            .collect(),
    }
}

/// Files the story pretends to attach, in turn.
const FILES: [(&str, &str); 3] = [
    ("march-statement.csv", "18 KB"),
    ("receipts-q1.zip", "2.4 MB"),
    ("chart-of-accounts.xlsx", "64 KB"),
];

/// Shows a bookkeeping assistant's composer. The story plays the session:
/// it keeps the attachments, models, folder and apps, answers for two
/// seconds after each message and pretends to hear a sentence when the
/// microphone is on. Type "/" or "@" for suggestions, or drop files on it.
pub struct PromptComposerStory {
    composer: Entity<PromptComposer>,
    attachments: Vec<ComposerAttachment>,
    apps: Vec<(ComposerChoice, bool)>,
    next_file: usize,
    sent: Vec<SharedString>,
    last: SharedString,
    _events: Subscription,
    _work: Option<Task<()>>,
}

impl PromptComposerStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer =
                cx.new(|cx| PromptComposer::new("Ask about your books\u{2026}", window, cx));
            let apps = vec![
                (
                    ComposerChoice::new("bank", "Bank feed").note("Read only"),
                    true,
                ),
                (
                    ComposerChoice::new("mail", "Mail").note("Drafts only"),
                    true,
                ),
                (ComposerChoice::new("payroll", "Payroll"), false),
            ];
            composer.update(cx, |composer, cx| {
                composer.set_models(
                    vec![
                        ComposerChoice::new("ledger-fast", "Ledger Fast").note("Quick answers"),
                        ComposerChoice::new("ledger-deep", "Ledger Deep").note("Careful reviews"),
                    ],
                    Some("ledger-fast".into()),
                    cx,
                );
                composer.set_folders(
                    vec![
                        ComposerChoice::new("2026", "Books 2026"),
                        ComposerChoice::new("2025", "Books 2025").note("Closed"),
                    ],
                    Some("2026".into()),
                    cx,
                );
                composer.set_apps(apps.clone(), cx);
                composer.set_max_chars(Some(400), cx);
            });
            let events = cx.subscribe_in(&composer, window, Self::on_event);
            Self {
                composer,
                attachments: Vec::new(),
                apps,
                next_file: 0,
                sent: Vec::new(),
                last: "Nothing sent yet".into(),
                _events: events,
                _work: None,
            }
        })
    }

    fn on_event(
        &mut self,
        composer: &Entity<PromptComposer>,
        event: &PromptComposerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            PromptComposerEvent::Submit { text, attachments } => {
                let line = if attachments.is_empty() {
                    text.to_string()
                } else {
                    format!("{text} (+{} files)", attachments.len())
                };
                self.sent.push(line.into());
                self.attachments.clear();
                self.last = "Answering\u{2026}".into();
                composer.update(cx, |composer, cx| {
                    composer.set_attachments(Vec::new(), cx);
                    composer.set_busy(true, cx);
                });
                let composer = composer.clone();
                self._work = Some(cx.spawn(async move |this, cx| {
                    // Long enough to try Stop before the answer ends.
                    cx.background_executor().timer(Duration::from_secs(5)).await;
                    composer.update(cx, |composer, cx| composer.set_busy(false, cx));
                    this.update(cx, |this, cx| {
                        this.last = "Answered".into();
                        cx.notify();
                    })
                    .ok();
                }));
            }
            PromptComposerEvent::Stop => {
                self._work = None;
                self.last = "Stopped".into();
                composer.update(cx, |composer, cx| composer.set_busy(false, cx));
            }
            PromptComposerEvent::Attach(kind) => {
                let (name, size) = FILES[self.next_file % FILES.len()];
                self.next_file += 1;
                let name = match kind {
                    AttachKind::Files => name.to_string(),
                    AttachKind::Images => format!("screenshot-{}.png", self.next_file),
                };
                let id: SharedString = format!("file-{}", self.next_file).into();
                self.attachments.push(
                    ComposerAttachment::new(id.clone(), name)
                        .detail(size)
                        .loading(true),
                );
                self.push_attachments(composer, cx);
                let composer = composer.clone();
                self._work = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(900))
                        .await;
                    this.update(cx, |this, cx| {
                        for attachment in &mut this.attachments {
                            if attachment.id == id {
                                attachment.loading = false;
                            }
                        }
                        this.push_attachments(&composer, cx);
                    })
                    .ok();
                }));
            }
            PromptComposerEvent::RemoveAttachment(id) => {
                self.attachments.retain(|attachment| &attachment.id != id);
                self.push_attachments(composer, cx);
            }
            PromptComposerEvent::SelectModel(id) => {
                let models = vec![
                    ComposerChoice::new("ledger-fast", "Ledger Fast").note("Quick answers"),
                    ComposerChoice::new("ledger-deep", "Ledger Deep").note("Careful reviews"),
                ];
                composer.update(cx, |composer, cx| {
                    composer.set_models(models, Some(id.clone()), cx)
                });
                self.last = format!("Model: {id}").into();
            }
            PromptComposerEvent::SelectFolder(id) => {
                let folders = vec![
                    ComposerChoice::new("2026", "Books 2026"),
                    ComposerChoice::new("2025", "Books 2025").note("Closed"),
                ];
                composer.update(cx, |composer, cx| {
                    composer.set_folders(folders, Some(id.clone()), cx)
                });
                self.last = format!("Folder: {id}").into();
            }
            PromptComposerEvent::ToggleApp { id, on } => {
                for (app, is_on) in &mut self.apps {
                    if &app.id == id {
                        *is_on = *on;
                    }
                }
                let apps = self.apps.clone();
                composer.update(cx, |composer, cx| composer.set_apps(apps, cx));
            }
            PromptComposerEvent::Dictate(on) => {
                composer.update(cx, |composer, cx| composer.set_listening(*on, cx));
                if *on {
                    let composer = composer.clone();
                    self._work = Some(cx.spawn_in(window, async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(2500))
                            .await;
                        composer
                            .update_in(cx, |composer, window, cx| {
                                composer.insert_text(
                                    "Which supplier invoices are still unpaid?",
                                    window,
                                    cx,
                                );
                                composer.set_listening(false, cx);
                            })
                            .ok();
                    }));
                } else {
                    self._work = None;
                }
            }
            PromptComposerEvent::Changed(_) | PromptComposerEvent::QueryClosed => {}
            PromptComposerEvent::Query { trigger, query } => {
                let found = matches(*trigger, query);
                composer.update(cx, |composer, cx| composer.set_suggestions(found, cx));
            }
            PromptComposerEvent::AcceptSuggestion { trigger, id } => {
                self.last = match trigger {
                    SuggestTrigger::Slash => format!("Command: {id}"),
                    SuggestTrigger::Mention => format!("Mentioned: {id}"),
                }
                .into();
            }
            PromptComposerEvent::DropFiles(paths) => {
                for path in paths {
                    self.next_file += 1;
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".into());
                    self.attachments.push(ComposerAttachment::new(
                        format!("file-{}", self.next_file),
                        name,
                    ));
                }
                self.push_attachments(composer, cx);
            }
            PromptComposerEvent::PasteImages(images) => {
                for _ in images {
                    self.next_file += 1;
                    self.attachments.push(ComposerAttachment::new(
                        format!("file-{}", self.next_file),
                        format!("pasted-{}.png", self.next_file),
                    ));
                }
                self.push_attachments(composer, cx);
            }
        }
        cx.notify();
    }

    fn push_attachments(&self, composer: &Entity<PromptComposer>, cx: &mut Context<Self>) {
        let attachments = self.attachments.clone();
        composer.update(cx, |composer, cx| composer.set_attachments(attachments, cx));
    }
}

impl Story for PromptComposerStory {
    fn title() -> &'static str {
        "Prompt composer"
    }

    fn description() -> &'static str {
        "The field where a person writes to an assistant."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PromptComposerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .w_full()
            .max_w(px(640.))
            .gap_3()
            .children(self.sent.iter().map(|message| {
                div()
                    .self_end()
                    .max_w(px(420.))
                    .px_3()
                    .py_2()
                    .rounded(cx.theme().radius_lg)
                    .bg(cx.theme().muted)
                    .text_sm()
                    .child(message.clone())
            }))
            .child(self.composer.clone())
            .child(div().text_xs().text_color(quiet).child(self.last.clone()))
    }
}
