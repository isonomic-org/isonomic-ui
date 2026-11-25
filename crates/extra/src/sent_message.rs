// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Sent message: the person's own turn in a conversation.
//!
//! Attachments sit above the bubble as chips with a name, a kind and a size.
//! Under the bubble a quiet line says where delivery stands: sending with a
//! spinner, sent or edited with the time, or not sent with a way to try
//! again. Edit and copy appear under the pointer or with keyboard focus.
//! Editing swaps the bubble for a field in place: Enter saves and sends the
//! new text again, Shift+Enter breaks the line, Escape cancels.

use std::time::Duration;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    attachment::{
        Attachment, AttachmentContent, AttachmentDescription, AttachmentMedia, AttachmentTitle,
    },
    bubble::{Bubble, BubbleContent, BubbleVariant},
    button::{Button, ButtonVariants as _},
    clipboard::Clipboard,
    h_flex,
    input::{InputEvent, Position, Textarea, TextareaState},
    look,
    message::MessageAlignment,
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, AppContext as _, Context, ElementId, Entity,
    EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, px, relative,
};

/// How long the bubble and the field take to trade places.
const ENTER: Duration = Duration::from_millis(200);

/// What an attachment is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    /// A picture.
    Image,
    /// Any other file.
    File,
}

/// A file sent with the message.
#[derive(Debug, Clone, PartialEq)]
pub struct SentAttachment {
    /// Identifies it.
    pub id: SharedString,
    /// Its file name.
    pub name: SharedString,
    /// What it is.
    pub kind: AttachmentKind,
    /// Its size in bytes, when known.
    pub size: Option<u64>,
}

impl SentAttachment {
    /// A picture.
    pub fn image(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: AttachmentKind::Image,
            size: None,
        }
    }

    /// Any other file.
    pub fn file(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: AttachmentKind::File,
            size: None,
        }
    }

    /// Its size in bytes.
    pub fn size(mut self, bytes: u64) -> Self {
        self.size = Some(bytes);
        self
    }

    /// The quiet line under the name: "Image · 240 KB".
    pub fn detail(&self) -> String {
        let kind = match self.kind {
            AttachmentKind::Image => "Image",
            AttachmentKind::File => "File",
        };
        match self.size {
            Some(bytes) => format!("{kind} · {}", format_bytes(bytes)),
            None => kind.to_string(),
        }
    }
}

/// Where delivery stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Delivery {
    /// On its way.
    Sending,
    /// Arrived.
    #[default]
    Sent,
    /// Did not go out.
    Failed,
}

/// The message as its owner knows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SentMessageData {
    /// The text.
    pub content: SharedString,
    /// Files sent with it.
    pub attachments: Vec<SentAttachment>,
    /// The time to show, already formatted: "14:02".
    pub sent_at: Option<SharedString>,
    /// Where delivery stands.
    pub status: Delivery,
    /// The text was changed after it was first sent.
    pub edited: bool,
}

impl SentMessageData {
    /// A sent message with this text.
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            ..Self::default()
        }
    }

    /// Files sent with it.
    pub fn attachments(mut self, attachments: Vec<SentAttachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// The time to show, already formatted.
    pub fn sent_at(mut self, time: impl Into<SharedString>) -> Self {
        self.sent_at = Some(time.into());
        self
    }

    /// Where delivery stands.
    pub fn status(mut self, status: Delivery) -> Self {
        self.status = status;
        self
    }

    /// Mark the text as changed after it was first sent.
    pub fn edited(mut self, edited: bool) -> Self {
        self.edited = edited;
        self
    }
}

/// A size as the chip shows it: "512 B", "240 KB", "3.4 MB".
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * KB;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{} KB", (bytes as f64 / KB as f64).round())
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

/// The quiet line under the bubble: "Sending", "Not sent", "Sent · 14:02", "Edited · 14:02".
pub fn delivery_text(status: Delivery, edited: bool, sent_at: Option<&str>) -> String {
    match status {
        Delivery::Sending => "Sending".to_string(),
        Delivery::Failed => "Not sent".to_string(),
        Delivery::Sent => {
            let word = if edited { "Edited" } else { "Sent" };
            match sent_at {
                Some(time) => format!("{word} · {time}"),
                None => word.to_string(),
            }
        }
    }
}

/// What saving a draft does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Nothing to send: the field stays open.
    Stay,
    /// The text is unchanged: the field closes and nothing is sent.
    Close,
    /// The field closes and this text is sent again.
    Resend(String),
}

/// Decide what saving `draft` over `content` does. Space around the draft is dropped.
pub fn save_outcome(draft: &str, content: &str) -> SaveOutcome {
    let text = draft.trim();
    if text.is_empty() {
        SaveOutcome::Stay
    } else if text == content {
        SaveOutcome::Close
    } else {
        SaveOutcome::Resend(text.to_string())
    }
}

/// The line and column just past the last character of `text`, both counted
/// from zero, with the column in UTF-16 units as text positions are.
pub fn end_of(text: &str) -> (u32, u32) {
    let line = text.matches('\n').count() as u32;
    let last = text.rsplit('\n').next().unwrap_or_default();
    (line, last.encode_utf16().count() as u32)
}

/// What the message reports.
#[derive(Debug, Clone, PartialEq)]
pub enum SentMessageEvent {
    /// Saved from the field with new text: send it again.
    Edited(SharedString),
    /// Asked to send a failed message again.
    Retry,
    /// The text was copied.
    Copied(SharedString),
}

/// The person's own message.
pub struct SentMessage {
    id: ElementId,
    data: SentMessageData,
    editable: bool,
    retryable: bool,
    editing: bool,
    /// Counts trades between bubble and field, so each one moves once.
    trades: usize,
    field: Entity<TextareaState>,
    actions_focus: FocusHandle,
    _events: Subscription,
}

impl EventEmitter<SentMessageEvent> for SentMessage {}

impl SentMessage {
    /// Create the message. It can be edited and, when it fails, retried.
    pub fn new(
        id: impl Into<ElementId>,
        data: SentMessageData,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx).auto_grow(2, 6);
            field.set_submit_on_enter(true, cx);
            field
        });
        let events = cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            InputEvent::PressEnter { shift: false, .. } => this.save(window, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        Self {
            id: id.into(),
            data,
            editable: true,
            retryable: true,
            editing: false,
            trades: 0,
            field,
            actions_focus: cx.focus_handle(),
            _events: events,
        }
    }

    /// Whether the edit action is offered. On by default.
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    /// Whether a failed message offers Retry. On by default.
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// The message as it stands.
    pub fn data(&self) -> &SentMessageData {
        &self.data
    }

    /// Replace the message.
    pub fn set_data(&mut self, data: SentMessageData, cx: &mut Context<Self>) {
        self.data = data;
        cx.notify();
    }

    /// Change where delivery stands.
    pub fn set_status(&mut self, status: Delivery, cx: &mut Context<Self>) {
        self.data.status = status;
        cx.notify();
    }

    /// Whether the field is open.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// Open the field on the current text.
    pub fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.data.content.clone();
        let end = end_of(&text);
        self.field.update(cx, |field, cx| {
            field.set_value(text, window, cx);
            field.set_cursor_position(Position::new(end.0, end.1), window, cx);
        });
        self.editing = true;
        self.trades += 1;
        cx.notify();
    }

    /// Close the field and keep the text as it was.
    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.trades += 1;
        self.actions_focus.focus(window, cx);
        cx.notify();
    }

    /// Save the field: new text is reported, unchanged text only closes it.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.field.read(cx).value();
        match save_outcome(&draft, &self.data.content) {
            SaveOutcome::Stay => {}
            SaveOutcome::Close => self.cancel(window, cx),
            SaveOutcome::Resend(text) => {
                self.cancel(window, cx);
                cx.emit(SentMessageEvent::Edited(text.into()));
            }
        }
    }

    fn part(&self, name: &'static str) -> ElementId {
        ElementId::NamedChild(self.id.clone().into(), name.into())
    }

    fn attachments(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        if self.data.attachments.is_empty() {
            return None;
        }
        let wash = look::ink::wash_strong(cx);
        let clear = cx.theme().transparent;
        let chips = self.data.attachments.iter().map(|file| {
            let icon = match file.kind {
                AttachmentKind::Image => CatalogIcon::Image,
                AttachmentKind::File => CatalogIcon::Document,
            };
            Attachment::new()
                .small()
                .w_auto()
                .max_w(px(240.))
                .rounded(px(16.))
                .border_color(clear)
                .bg(wash)
                .media(AttachmentMedia::new().child(Icon::new(icon)))
                .content(
                    AttachmentContent::new()
                        .title(AttachmentTitle::new(file.name.clone()))
                        .description(AttachmentDescription::new(file.detail())),
                )
        });
        Some(
            h_flex()
                .id(self.part("attachments"))
                .max_w(relative(0.85))
                .flex_wrap()
                .justify_end()
                .gap(px(6.))
                .role(Role::List)
                .aria_label("Attachments")
                .children(chips),
        )
    }

    fn delivery(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let danger = theme.danger;
        let words = delivery_text(
            self.data.status,
            self.data.edited,
            self.data.sent_at.as_deref(),
        );
        match self.data.status {
            Delivery::Sending => h_flex()
                .gap_1()
                .px_1()
                .child(Spinner::new().xsmall())
                .child(words)
                .into_any_element(),
            Delivery::Failed => h_flex()
                .gap(px(6.))
                .pl_1()
                .text_color(danger)
                .child(Icon::new(CatalogIcon::WarningFilled).size(px(14.)))
                .child(words)
                .when(self.retryable, |this| {
                    this.child(
                        Button::new(self.part("retry"))
                            .danger()
                            .small()
                            .icon(Icon::new(CatalogIcon::Renew))
                            .label("Retry")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SentMessageEvent::Retry))),
                    )
                })
                .into_any_element(),
            Delivery::Sent => div().px_1().child(words).into_any_element(),
        }
    }
}

impl Render for SentMessage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let surface = theme.muted;
        let ink = theme.foreground;
        let still = cx.reduce_motion();
        let status = self.data.status;
        let hover = SharedString::from(format!("sent-{}", self.id));
        let focus_within = self.actions_focus.contains_focused(window, cx);
        let traded = self.trades > 0 && !still;
        let trade_id = ElementId::NamedChild(
            self.id.clone().into(),
            format!("trade-{}", self.trades).into(),
        );

        let body = if self.editing {
            let empty = self.field.read(cx).value().trim().is_empty();
            let form = v_flex()
                .id(self.part("editor"))
                .w_full()
                .max_w(relative(0.85))
                .gap_2()
                .p(px(6.))
                .rounded(px(16.))
                .bg(surface)
                .role(Role::Form)
                .aria_label("Edit message")
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        this.cancel(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(Textarea::new(&self.field).aria_label("Edit message"))
                .child(
                    h_flex()
                        .justify_end()
                        .gap(px(6.))
                        .child(
                            Button::new(self.part("cancel"))
                                .ghost()
                                .small()
                                .label("Cancel")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.cancel(window, cx)),
                                ),
                        )
                        .child(
                            Button::new(self.part("save"))
                                .primary()
                                .small()
                                .label("Save & resend")
                                .disabled(empty)
                                .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                        ),
                );
            if traded {
                form.with_animation(trade_id, Animation::new(ENTER), |this, time| {
                    this.opacity(1. - (1. - time).powi(3))
                })
                .into_any_element()
            } else {
                form.into_any_element()
            }
        } else {
            let lines = self.data.content.split('\n').map(|line| {
                div()
                    .min_h(px(22.))
                    .child(SharedString::from(line.to_string()))
            });
            let bubble = Bubble::new()
                .with_variant(BubbleVariant::Secondary)
                .alignment(MessageAlignment::End)
                .max_w(relative(0.85))
                .when(status == Delivery::Sending, |this| this.opacity(0.7))
                .content(
                    BubbleContent::new()
                        .rounded(px(16.))
                        .rounded_br(px(10.))
                        .px(px(14.))
                        .py_2()
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .text_color(ink)
                        .children(lines),
                );
            if traded {
                div()
                    .flex()
                    .w_full()
                    .justify_end()
                    .child(bubble)
                    .with_animation(trade_id, Animation::new(ENTER), |this, time| {
                        this.opacity(1. - (1. - time).powi(3))
                    })
                    .into_any_element()
            } else {
                bubble.into_any_element()
            }
        };

        let footer = (!self.editing).then(|| {
            let this = cx.entity().downgrade();
            let can_edit = self.editable && status != Delivery::Sending;
            h_flex()
                .gap_1()
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(quiet)
                .child(
                    h_flex()
                        .id(self.part("actions"))
                        .test_support()
                        .track_focus(&self.actions_focus)
                        .gap_1()
                        .when(!focus_within, |this| {
                            this.opacity(0.)
                                .group_hover(hover.clone(), |this| this.opacity(1.))
                        })
                        .when(can_edit, |this| {
                            this.child(
                                Button::new(self.part("edit"))
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(CatalogIcon::Edit))
                                    .tooltip("Edit message")
                                    .accessibility_label("Edit message")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_editing(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Clipboard::new(self.part("copy"))
                                .value(self.data.content.clone())
                                .tooltip("Copy message")
                                .accessibility_label("Copy message")
                                .on_copied(move |text, _, cx| {
                                    this.update(cx, |_, cx| {
                                        cx.emit(SentMessageEvent::Copied(text));
                                    })
                                    .ok();
                                }),
                        ),
                )
                .child(self.delivery(cx))
        });

        let spoken = format!(
            "Your message, {}",
            delivery_text(status, self.data.edited, self.data.sent_at.as_deref()).to_lowercase()
        );
        v_flex()
            .id(self.id.clone())
            .test_support()
            .group(hover)
            .w_full()
            .items_end()
            .gap(px(6.))
            .role(Role::Group)
            .aria_label(spoken)
            .children(self.attachments(cx))
            .child(body)
            .children(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_bytes_then_kilobytes_then_megabytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(245_760), "240 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(3_565_158), "3.4 MB");
    }

    #[test]
    fn a_chip_names_the_kind_and_the_size() {
        let photo = SentAttachment::image("a", "awning.jpg").size(245_760);
        assert_eq!(photo.detail(), "Image · 240 KB");
        assert_eq!(SentAttachment::file("b", "quote.pdf").detail(), "File");
    }

    #[test]
    fn the_line_under_the_bubble_says_where_delivery_stands() {
        assert_eq!(delivery_text(Delivery::Sending, false, Some("14:02")), "Sending");
        assert_eq!(delivery_text(Delivery::Failed, true, None), "Not sent");
        assert_eq!(delivery_text(Delivery::Sent, false, None), "Sent");
        assert_eq!(delivery_text(Delivery::Sent, false, Some("14:02")), "Sent · 14:02");
        assert_eq!(delivery_text(Delivery::Sent, true, Some("14:02")), "Edited · 14:02");
    }

    #[test]
    fn editing_starts_after_the_last_character() {
        assert_eq!(end_of(""), (0, 0));
        assert_eq!(end_of("Hello"), (0, 5));
        assert_eq!(end_of("Hello\nthere, you"), (1, 10));
        assert_eq!(end_of("One\n"), (1, 0));
        // An emoji outside the basic plane takes two UTF-16 units.
        assert_eq!(end_of("ok \u{1f331}"), (0, 5));
    }

    #[test]
    fn saving_sends_only_text_that_changed() {
        assert_eq!(save_outcome("  \n ", "Hello"), SaveOutcome::Stay);
        assert_eq!(save_outcome(" Hello ", "Hello"), SaveOutcome::Close);
        assert_eq!(
            save_outcome("Hello there\n", "Hello"),
            SaveOutcome::Resend("Hello there".to_string())
        );
    }
}
