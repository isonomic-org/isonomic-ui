//! Prompt composer: the field where a person writes to an assistant.
//!
//! A growing text field sits over a row of controls: a plus menu for
//! attaching things, the working folder, the apps the assistant may use, the
//! model, a microphone and the send button. Attachments show as chips above
//! the field. While the assistant answers, send turns into stop.
//!
//! The composer only shows what it is given and reports what the person
//! does. Its owner keeps the attachments, the models, the folder, the apps,
//! whether the assistant is busy and whether dictation is listening.

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
    relative,
};

use crate::common::resting_shadow;

/// Something attached to the next message.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerAttachment {
    /// Identifies it in [`PromptComposerEvent::RemoveAttachment`].
    pub id: SharedString,
    /// What the chip says, usually a file name.
    pub name: SharedString,
    /// A short line after the name, such as a size.
    pub detail: Option<SharedString>,
    /// Still on its way in: the chip shows a spinner and send waits.
    pub loading: bool,
}

impl ComposerAttachment {
    /// Create a finished attachment.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            detail: None,
            loading: false,
        }
    }

    /// Add a short line after the name.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Mark it as still loading.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
}

/// One entry in a menu of the composer: a model, a folder or an app.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposerChoice {
    /// Identifies it in the events.
    pub id: SharedString,
    /// Its name.
    pub label: SharedString,
    /// A quiet line under the name in the menu.
    pub note: Option<SharedString>,
}

impl ComposerChoice {
    /// Create an entry.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            note: None,
        }
    }

    /// Add a quiet line under the name.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    fn menu_label(&self) -> SharedString {
        match &self.note {
            Some(note) => format!("{}  \u{b7}  {note}", self.label).into(),
            None => self.label.clone(),
        }
    }
}

/// What the person asked to attach from the plus menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachKind {
    /// Files from the computer.
    Files,
    /// Photos or screenshots.
    Images,
}

/// What happened in the composer.
#[derive(Debug, Clone, PartialEq)]
pub enum PromptComposerEvent {
    /// Send was pressed, or Enter in the field. Carries the text and the ids
    /// of the attachments. The field clears itself.
    Submit {
        /// The message, trimmed.
        text: SharedString,
        /// The attachments sent with it.
        attachments: Vec<SharedString>,
    },
    /// Stop was pressed while the assistant was busy.
    Stop,
    /// The plus menu asked for something to attach.
    Attach(AttachKind),
    /// An attachment's remove button was pressed.
    RemoveAttachment(SharedString),
    /// Another model was picked.
    SelectModel(SharedString),
    /// Another working folder was picked.
    SelectFolder(SharedString),
    /// An app was switched on or off.
    ToggleApp {
        /// The app.
        id: SharedString,
        /// Whether it is now on.
        on: bool,
    },
    /// The microphone was pressed. Start or stop your recogniser, then pass
    /// [`PromptComposer::set_listening`] and what it hears to
    /// [`PromptComposer::insert_text`].
    Dictate(bool),
    /// The text changed.
    Changed(SharedString),
}

/// The field where a person writes to an assistant.
pub struct PromptComposer {
    field: Entity<TextareaState>,
    attachments: Vec<ComposerAttachment>,
    models: Vec<ComposerChoice>,
    model: Option<SharedString>,
    folders: Vec<ComposerChoice>,
    folder: Option<SharedString>,
    apps: Vec<(ComposerChoice, bool)>,
    busy: bool,
    listening: bool,
    disabled: bool,
    dictation: bool,
    max_chars: Option<usize>,
    _field_events: Subscription,
}

impl EventEmitter<PromptComposerEvent> for PromptComposer {}

impl Focusable for PromptComposer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

/// Whether there is something to send: text, or a finished attachment with
/// nothing still loading.
fn can_send(text: &str, attachments: &[ComposerAttachment], over_limit: bool) -> bool {
    let loading = attachments.iter().any(|attachment| attachment.loading);
    let has_something = !text.trim().is_empty() || !attachments.is_empty();
    has_something && !loading && !over_limit
}

/// How many characters are left before the limit, once it is near.
fn chars_left(text: &str, max: Option<usize>) -> Option<i64> {
    let max = max? as i64;
    let used = text.chars().count() as i64;
    (used * 5 >= max * 4).then_some(max - used)
}

impl PromptComposer {
    /// Create a composer with a placeholder in the empty field.
    pub fn new(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = placeholder.into();
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .placeholder(placeholder);
            field.set_submit_on_enter(true, cx);
            field
        });
        let field_events =
            cx.subscribe_in(
                &field,
                window,
                |this, field, event, window, cx| match event {
                    InputEvent::PressEnter { shift: false, .. } => this.submit(window, cx),
                    InputEvent::Change => {
                        let text = field.read(cx).value();
                        cx.emit(PromptComposerEvent::Changed(text));
                        cx.notify();
                    }
                    _ => {}
                },
            );
        Self {
            field,
            attachments: Vec::new(),
            models: Vec::new(),
            model: None,
            folders: Vec::new(),
            folder: None,
            apps: Vec::new(),
            busy: false,
            listening: false,
            disabled: false,
            dictation: true,
            max_chars: None,
            _field_events: field_events,
        }
    }

    /// Show these attachments as chips over the field.
    pub fn set_attachments(
        &mut self,
        attachments: Vec<ComposerAttachment>,
        cx: &mut Context<Self>,
    ) {
        self.attachments = attachments;
        cx.notify();
    }

    /// Offer these models, with `selected` shown on the model button.
    pub fn set_models(
        &mut self,
        models: Vec<ComposerChoice>,
        selected: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.models = models;
        self.model = selected;
        cx.notify();
    }

    /// Offer these working folders, with `selected` shown on the folder button.
    pub fn set_folders(
        &mut self,
        folders: Vec<ComposerChoice>,
        selected: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.folders = folders;
        self.folder = selected;
        cx.notify();
    }

    /// List these apps, each on or off.
    pub fn set_apps(&mut self, apps: Vec<(ComposerChoice, bool)>, cx: &mut Context<Self>) {
        self.apps = apps;
        cx.notify();
    }

    /// Say whether the assistant is answering: send turns into stop.
    pub fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        cx.notify();
    }

    /// Say whether dictation is listening: the microphone pulses.
    pub fn set_listening(&mut self, listening: bool, cx: &mut Context<Self>) {
        self.listening = listening;
        cx.notify();
    }

    /// Turn the whole composer off, such as while offline.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    /// Show or hide the microphone. Shown by default.
    pub fn set_dictation(&mut self, dictation: bool, cx: &mut Context<Self>) {
        self.dictation = dictation;
        cx.notify();
    }

    /// Count characters down near this limit, and hold send past it.
    pub fn set_max_chars(&mut self, max: Option<usize>, cx: &mut Context<Self>) {
        self.max_chars = max;
        cx.notify();
    }

    /// Replace the text in the field.
    pub fn set_text(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = text.into();
        self.field
            .update(cx, |field, cx| field.set_value(text, window, cx));
    }

    /// Add text at the end of the field, such as words heard by dictation.
    pub fn insert_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.field.read(cx).value();
        let joined = if current.is_empty() || current.ends_with(' ') {
            format!("{current}{text}")
        } else {
            format!("{current} {text}")
        };
        self.field
            .update(cx, |field, cx| field.set_value(joined, window, cx));
    }

    /// The text in the field.
    pub fn text(&self, cx: &App) -> SharedString {
        self.field.read(cx).value()
    }

    /// Put the caret in the field.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.field.focus_handle(cx).focus(window, cx);
    }

    fn over_limit(&self, text: &str) -> bool {
        self.max_chars.is_some_and(|max| text.chars().count() > max)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.disabled {
            return;
        }
        let text = self.field.read(cx).value();
        if !can_send(&text, &self.attachments, self.over_limit(&text)) {
            return;
        }
        let attachments = self.attachments.iter().map(|a| a.id.clone()).collect();
        self.field
            .update(cx, |field, cx| field.set_value("", window, cx));
        cx.emit(PromptComposerEvent::Submit {
            text: text.trim().to_string().into(),
            attachments,
        });
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Escape stops an answer that is coming in.
        if event.keystroke.key == "escape" && self.busy {
            cx.emit(PromptComposerEvent::Stop);
            cx.stop_propagation();
        }
    }

    fn chip(&self, attachment: &ComposerAttachment, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let id = attachment.id.clone();
        h_flex()
            .flex_none()
            .max_w(px(220.))
            .gap_1p5()
            .h(px(30.))
            .pl_2()
            .pr_0p5()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted.opacity(0.5))
            .text_xs()
            .child(if attachment.loading {
                Icon::new(IconName::LoaderCircle)
                    .xsmall()
                    .text_color(quiet)
                    .with_animation(
                        SharedString::from(format!("loading-{}", attachment.id)),
                        Animation::new(std::time::Duration::from_millis(900)).repeat(),
                        |icon, time| {
                            icon.transform(isonomic_ui::Transformation::rotate(
                                isonomic_ui::percentage(time),
                            ))
                        },
                    )
                    .into_any_element()
            } else {
                Icon::new(IconName::File)
                    .xsmall()
                    .text_color(quiet)
                    .into_any_element()
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_medium()
                    .child(attachment.name.clone()),
            )
            .children(
                attachment
                    .detail
                    .clone()
                    .map(|detail| div().flex_none().text_color(quiet).child(detail)),
            )
            .child(
                Button::new(SharedString::from(format!("remove-{}", attachment.id)))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .disabled(self.disabled)
                    .accessibility_label(format!("Remove {}", attachment.name))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(PromptComposerEvent::RemoveAttachment(id.clone()))
                    })),
            )
    }
}

impl Render for PromptComposer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let fill = theme.background;
        let line = theme.border;
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let radius = theme.radius_lg;
        let focused = self.field.focus_handle(cx).contains_focused(window, cx);
        let edge = if focused { theme.ring } else { line };

        let text = self.field.read(cx).value();
        let over = self.over_limit(&text);
        let sendable = can_send(&text, &self.attachments, over) && !self.disabled;
        let left = chars_left(&text, self.max_chars);
        let disabled = self.disabled;
        let entity = cx.entity();

        let chips: Vec<_> = self
            .attachments
            .clone()
            .iter()
            .map(|attachment| self.chip(attachment, cx).into_any_element())
            .collect();

        let plus = Button::new("attach")
            .ghost()
            .small()
            .icon(IconName::Plus)
            .disabled(disabled)
            .tooltip("Add files and more")
            .accessibility_label("Add files and more")
            .dropdown_menu({
                let entity = entity.clone();
                move |menu, _, _| {
                    let files = entity.clone();
                    let images = entity.clone();
                    menu.item(
                        PopupMenuItem::new("Add files")
                            .icon(IconName::File)
                            .on_click(move |_, _, cx| {
                                files.update(cx, |_, cx| {
                                    cx.emit(PromptComposerEvent::Attach(AttachKind::Files))
                                })
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Add photos and screenshots")
                            .icon(IconName::Frame)
                            .on_click(move |_, _, cx| {
                                images.update(cx, |_, cx| {
                                    cx.emit(PromptComposerEvent::Attach(AttachKind::Images))
                                })
                            }),
                    )
                }
            });

        let folder = (!self.folders.is_empty()).then(|| {
            let current = self
                .folder
                .as_ref()
                .and_then(|id| self.folders.iter().find(|folder| &folder.id == id))
                .map(|folder| folder.label.clone())
                .unwrap_or_else(|| "No folder".into());
            let folders = self.folders.clone();
            let selected = self.folder.clone();
            let entity = entity.clone();
            Button::new("folder")
                .ghost()
                .small()
                .disabled(disabled)
                .icon(IconName::Folder)
                .label(current.clone())
                .accessibility_label(format!("Working folder: {current}"))
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.label("Working folder");
                    for folder in &folders {
                        let entity = entity.clone();
                        let id = folder.id.clone();
                        menu = menu.item(
                            PopupMenuItem::new(folder.menu_label())
                                .checked(selected.as_ref() == Some(&folder.id))
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    entity.update(cx, |_, cx| {
                                        cx.emit(PromptComposerEvent::SelectFolder(id))
                                    })
                                }),
                        );
                    }
                    menu
                })
        });

        let apps = (!self.apps.is_empty()).then(|| {
            let on = self.apps.iter().filter(|(_, on)| *on).count();
            let apps = self.apps.clone();
            let entity = entity.clone();
            let label = format!("Apps {on}/{}", apps.len());
            Button::new("apps")
                .ghost()
                .small()
                .disabled(disabled)
                .icon(IconName::LayoutDashboard)
                .label(label)
                .accessibility_label(format!("Apps, {on} of {} on", apps.len()))
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.label("Apps the assistant may use");
                    for (app, is_on) in &apps {
                        let entity = entity.clone();
                        let id = app.id.clone();
                        let next = !is_on;
                        menu = menu.item(
                            PopupMenuItem::new(app.menu_label())
                                .checked(*is_on)
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    entity.update(cx, |_, cx| {
                                        cx.emit(PromptComposerEvent::ToggleApp { id, on: next })
                                    })
                                }),
                        );
                    }
                    menu
                })
        });

        let model = (!self.models.is_empty()).then(|| {
            let current = self
                .model
                .as_ref()
                .and_then(|id| self.models.iter().find(|model| &model.id == id))
                .map(|model| model.label.clone())
                .unwrap_or_else(|| "Model".into());
            let models = self.models.clone();
            let selected = self.model.clone();
            let entity = entity.clone();
            Button::new("model")
                .ghost()
                .small()
                .disabled(disabled)
                .label(current.clone())
                .dropdown_caret(true)
                .accessibility_label(format!("Model: {current}"))
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.label("Model");
                    for model in &models {
                        let entity = entity.clone();
                        let id = model.id.clone();
                        menu = menu.item(
                            PopupMenuItem::new(model.menu_label())
                                .checked(selected.as_ref() == Some(&model.id))
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    entity.update(cx, |_, cx| {
                                        cx.emit(PromptComposerEvent::SelectModel(id))
                                    })
                                }),
                        );
                    }
                    menu
                })
        });

        let listening = self.listening;
        let mic = self.dictation.then(|| {
            let button = Button::new("dictate")
                .small()
                .icon(IconName::Mic)
                .disabled(disabled)
                .map(|this| {
                    if listening {
                        this.danger()
                    } else {
                        this.ghost()
                    }
                })
                .tooltip(if listening {
                    "Stop dictating"
                } else {
                    "Dictate"
                })
                .accessibility_label(if listening {
                    "Stop dictating"
                } else {
                    "Dictate"
                })
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(PromptComposerEvent::Dictate(!listening))
                }));
            if listening && !cx.reduce_motion() {
                div()
                    .child(button)
                    .with_animation(
                        "listening",
                        Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                        |this, time| {
                            let wave = (time * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                            this.opacity(0.65 + 0.35 * wave)
                        },
                    )
                    .into_any_element()
            } else {
                button.into_any_element()
            }
        });

        let send = if self.busy {
            Button::new("stop")
                .small()
                .icon(IconName::Square)
                .tooltip("Stop  Esc")
                .accessibility_label("Stop the answer")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(PromptComposerEvent::Stop)))
        } else {
            Button::new("send")
                .small()
                .primary()
                .icon(IconName::ArrowUp)
                .disabled(!sendable)
                .tooltip("Send  \u{21b5}")
                .accessibility_label("Send")
                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
        };

        v_flex()
            .id("prompt-composer")
            .key_context("PromptComposer")
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .gap_2()
            .p_2()
            .rounded(radius)
            .border_1()
            .border_color(edge)
            .bg(fill)
            .text_sm()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .when(disabled, |this| this.opacity(0.6))
            .when(!chips.is_empty(), |this| {
                this.child(h_flex().flex_wrap().gap_1p5().px_1().pt_1().children(chips))
            })
            .child(
                div().px_1().child(
                    Textarea::new(&self.field)
                        .appearance(false)
                        .disabled(disabled)
                        .aria_label("Message"),
                ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(plus)
                    .children(folder)
                    .children(apps)
                    .child(div().flex_1())
                    .children(left.map(|left| {
                        div()
                            .flex_none()
                            .px_1()
                            .text_xs()
                            .text_color(if left < 0 { danger } else { quiet })
                            .child(left.to_string())
                    }))
                    .children(model)
                    .children(mic)
                    .child(send),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_must_be_there_to_send() {
        assert!(!can_send("", &[], false));
        assert!(!can_send("   ", &[], false));
        assert!(can_send("Close March", &[], false));
        assert!(can_send(
            "",
            &[ComposerAttachment::new("a", "march.csv")],
            false
        ));
    }

    #[test]
    fn a_loading_attachment_or_too_much_text_holds_send() {
        let loading = ComposerAttachment::new("a", "march.csv").loading(true);
        assert!(!can_send("Close March", &[loading], false));
        assert!(!can_send("Close March", &[], true));
    }

    #[test]
    fn the_count_shows_near_the_limit() {
        assert_eq!(chars_left("abc", None), None);
        assert_eq!(chars_left("abc", Some(100)), None);
        assert_eq!(chars_left(&"a".repeat(80), Some(100)), Some(20));
        assert_eq!(chars_left(&"a".repeat(103), Some(100)), Some(-3));
    }

    #[test]
    fn menu_entries_carry_their_note() {
        let model = ComposerChoice::new("fast", "Ledger Fast").note("Quick answers");
        assert_eq!(
            model.menu_label().as_ref(),
            "Ledger Fast  \u{b7}  Quick answers"
        );
        assert_eq!(
            ComposerChoice::new("x", "Plain").menu_label().as_ref(),
            "Plain"
        );
    }
}
