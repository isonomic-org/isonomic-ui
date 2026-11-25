// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Draft review card: a message an assistant wrote, waiting for a person.
//!
//! The record the message is about heads the card. Below it the reviewer can
//! switch the recipient and the tone, edit the text in place and drop the
//! attachment, then send or discard. Either way the message folds into a
//! one-line receipt with an undo.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How long "Sending" shows before the receipt.
const SENDING_FOR: Duration = Duration::from_millis(700);

/// Someone the message can go to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftContact {
    /// The full name.
    pub name: SharedString,
    /// The address the message goes to.
    pub address: SharedString,
}

impl DraftContact {
    /// Create a contact.
    pub fn new(name: impl Into<SharedString>, address: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            address: address.into(),
        }
    }

    /// The first word of the name, for a greeting.
    pub fn first_name(&self) -> &str {
        self.name.split_whitespace().next().unwrap_or_default()
    }
}

/// One way to word the message.
#[derive(Clone)]
pub struct DraftTone {
    label: SharedString,
    body: Rc<dyn Fn(&DraftContact) -> String>,
}

impl DraftTone {
    /// Create a tone whose text depends on who the message goes to.
    pub fn new(
        label: impl Into<SharedString>,
        body: impl Fn(&DraftContact) -> String + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            body: Rc::new(body),
        }
    }
}

/// The message as it was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedDraft {
    /// Who it goes to.
    pub to: DraftContact,
    /// The final text.
    pub body: SharedString,
    /// The attachment, unless it was removed.
    pub attachment: Option<SharedString>,
}

/// What the card tells its owner.
#[derive(Debug, Clone)]
pub enum DraftReviewEvent {
    /// Send was pressed. The receipt offers an undo, so hold the message back
    /// for a moment if it must be possible to take it back.
    Send(ReviewedDraft),
    /// The draft was discarded.
    Discard,
    /// Sending or discarding was undone, and the draft is open again.
    Undo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Draft,
    Sending,
    Sent,
    Discarded,
}

/// The line the card folds into after a decision.
fn receipt(phase: Phase, recipient: &str) -> Option<String> {
    match phase {
        Phase::Sent if recipient.is_empty() => Some("Sent".into()),
        Phase::Sent => Some(format!("Sent to {recipient}")),
        Phase::Discarded => Some("Draft discarded".into()),
        Phase::Draft | Phase::Sending => None,
    }
}

/// A drafted message with the controls to adjust it before it goes out.
pub struct DraftReviewCard {
    reference: Option<SharedString>,
    account: Option<SharedString>,
    amount: Option<SharedString>,
    status: Option<SharedString>,
    author: Option<SharedString>,
    contacts: Vec<DraftContact>,
    recipient: usize,
    attachment: Option<SharedString>,
    attached: bool,
    tones: Vec<DraftTone>,
    tone: usize,
    link: Option<(SharedString, SharedString)>,
    /// The text after it was edited by hand. Switching tone or recipient
    /// goes back to the tone's own text.
    edited: Option<SharedString>,
    editing: bool,
    phase: Phase,
    editor: Entity<TextareaState>,
    _editor_events: Subscription,
    _sending: Option<Task<()>>,
}

impl EventEmitter<DraftReviewEvent> for DraftReviewCard {}

impl DraftReviewCard {
    /// Create an empty card.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 10));
        let editor_events = cx.subscribe_in(&editor, window, |this, _, event, _, cx| {
            // The platform's main modifier with Enter sends from the editor.
            if let InputEvent::PressEnter {
                secondary: true, ..
            } = event
            {
                this.send(cx);
            }
        });

        Self {
            reference: None,
            account: None,
            amount: None,
            status: None,
            author: None,
            contacts: Vec::new(),
            recipient: 0,
            attachment: None,
            attached: true,
            tones: Vec::new(),
            tone: 0,
            link: None,
            edited: None,
            editing: false,
            phase: Phase::Draft,
            editor,
            _editor_events: editor_events,
            _sending: None,
        }
    }

    /// The reference of the record the message is about, such as an invoice number.
    pub fn reference(mut self, reference: impl Into<SharedString>) -> Self {
        self.reference = Some(reference.into());
        self
    }

    /// The account or company, under the reference.
    pub fn account(mut self, account: impl Into<SharedString>) -> Self {
        self.account = Some(account.into());
        self
    }

    /// An amount shown on the right of the header, already formatted.
    pub fn amount(mut self, amount: impl Into<SharedString>) -> Self {
        self.amount = Some(amount.into());
        self
    }

    /// A status chip next to the reference.
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Who wrote the draft, shown in the last row.
    pub fn author(mut self, author: impl Into<SharedString>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Add someone the message can go to. The first one is picked at first.
    pub fn contact(mut self, contact: DraftContact) -> Self {
        self.contacts.push(contact);
        self
    }

    /// Attach a file by name. It can be removed and added back.
    pub fn attachment(mut self, name: impl Into<SharedString>) -> Self {
        self.attachment = Some(name.into());
        self
    }

    /// Add a way to word the message. The first one shows at first.
    pub fn tone(mut self, tone: DraftTone) -> Self {
        self.tones.push(tone);
        self
    }

    /// Append a link to the text, with its label and address.
    pub fn link(mut self, label: impl Into<SharedString>, url: impl Into<SharedString>) -> Self {
        self.link = Some((label.into(), url.into()));
        self
    }

    /// The text as it stands: edited by hand, or the picked tone's.
    pub fn body(&self) -> SharedString {
        if let Some(edited) = &self.edited {
            return edited.clone();
        }
        match (self.tones.get(self.tone), self.contacts.get(self.recipient)) {
            (Some(tone), Some(contact)) => (tone.body)(contact).into(),
            (Some(tone), None) => (tone.body)(&DraftContact::new("", "")).into(),
            (None, _) => SharedString::default(),
        }
    }

    fn set_recipient(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.recipient = ix;
        self.edited = None;
        self.editing = false;
        cx.notify();
    }

    fn set_tone(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.tone = ix;
        self.edited = None;
        self.editing = false;
        cx.notify();
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != Phase::Draft {
            return;
        }
        let body = self.body();
        self.editor.update(cx, |editor, cx| {
            editor.set_value(body, window, cx);
            editor.focus_handle(cx).focus(window, cx);
        });
        self.editing = true;
        cx.notify();
    }

    /// Leave the editor and keep what was typed.
    fn stop_editing(&mut self, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        let typed = self.editor.read(cx).value();
        self.edited = None;
        if typed != self.body() {
            self.edited = Some(typed);
        }
        cx.notify();
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        if self.phase != Phase::Draft {
            return;
        }
        let Some(to) = self.contacts.get(self.recipient).cloned() else {
            return;
        };
        self.stop_editing(cx);
        let draft = ReviewedDraft {
            to,
            body: self.body(),
            attachment: self.attachment.clone().filter(|_| self.attached),
        };
        self.phase = Phase::Sending;
        self._sending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SENDING_FOR).await;
            this.update(cx, |this, cx| {
                this.phase = Phase::Sent;
                cx.emit(DraftReviewEvent::Send(draft));
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn discard(&mut self, cx: &mut Context<Self>) {
        if self.phase != Phase::Draft {
            return;
        }
        self.stop_editing(cx);
        self.phase = Phase::Discarded;
        cx.emit(DraftReviewEvent::Discard);
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Draft;
        cx.emit(DraftReviewEvent::Undo);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing && event.keystroke.key == "escape" {
            self.stop_editing(cx);
            cx.stop_propagation();
        }
    }
}

impl Render for DraftReviewCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let warning = theme.warning;
        let success = theme.success;
        let radius_lg = theme.radius_lg;

        let to = self.contacts.get(self.recipient).cloned();
        let recipient_name = to.as_ref().map(|to| to.name.clone()).unwrap_or_default();
        let receipt = receipt(self.phase, &recipient_name);
        let sending = self.phase == Phase::Sending;
        let this = cx.entity();

        let header =
            h_flex()
                .items_start()
                .gap_3()
                .px_4()
                .py_3()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_2()
                                .children(self.reference.clone().map(|reference| {
                                    div().text_base().font_medium().child(reference)
                                }))
                                .children(self.status.clone().map(|status| {
                                    div()
                                        .flex_none()
                                        .px_1p5()
                                        .rounded(px(5.))
                                        .bg(warning.opacity(0.18))
                                        .text_label_small()
                                        .child(status)
                                })),
                        )
                        .children(
                            self.account
                                .clone()
                                .map(|account| div().truncate().text_color(quiet).child(account)),
                        ),
                )
                .children(
                    self.amount
                        .clone()
                        .map(|amount| div().flex_none().text_base().font_medium().child(amount)),
                );

        let card = v_flex()
            .w_full()
            .max_w(px(480.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.5))
            .shadow(resting_shadow(cx))
            .on_key_down(cx.listener(Self::key_down))
            .child(header);

        if let Some(receipt) = receipt {
            return card.child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        Icon::new(if self.phase == Phase::Sent {
                            IconName::CircleCheck
                        } else {
                            IconName::Delete
                        })
                        .small()
                        .flex_none()
                        .text_color(if self.phase == Phase::Sent {
                            success
                        } else {
                            quiet
                        }),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(receipt))
                    .child(
                        Button::new("undo")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Undo)
                            .label("Undo")
                            .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
                    ),
            );
        }

        let recipient = to.clone().map(|to| {
            let contacts = self.contacts.clone();
            let current = self.recipient;
            let this = this.clone();
            Button::new("recipient")
                .outline()
                .small()
                .max_w_full()
                .min_w_0()
                .disabled(sending)
                .accessibility_label(format!("To {}, {}", to.name, to.address))
                .child(
                    // A long address is cut short instead of running under
                    // the attachment.
                    h_flex()
                        .min_w_0()
                        .gap_2()
                        .child(div().flex_none().font_medium().child(to.name))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(quiet)
                                .child(to.address),
                        )
                        .child(
                            Icon::new(IconName::ChevronDown)
                                .xsmall()
                                .flex_none()
                                .text_color(quiet),
                        ),
                )
                .dropdown_menu(move |mut menu, _, _| {
                    for (ix, contact) in contacts.iter().enumerate() {
                        let this = this.clone();
                        let label = format!("{}  {}", contact.name, contact.address);
                        menu =
                            menu.item(PopupMenuItem::new(label).checked(ix == current).on_click(
                                move |_, _, cx| {
                                    this.update(cx, |this, cx| this.set_recipient(ix, cx))
                                },
                            ));
                    }
                    menu
                })
        });
        let attachment = self.attachment.clone().map(|name| {
            if self.attached {
                h_flex()
                    .flex_none()
                    .gap_1()
                    .child(Icon::new(IconName::File).small().text_color(quiet))
                    .child(div().max_w(px(160.)).truncate().child(name.clone()))
                    .child(
                        Button::new("detach")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .disabled(sending)
                            .accessibility_label(format!("Remove {name}"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.attached = false;
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            } else {
                Button::new("attach")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .label(format!("Attach {name}"))
                    .disabled(sending)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.attached = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
        });

        let body = if self.editing {
            Textarea::new(&self.editor).into_any_element()
        } else {
            let link = self.link.clone().map(|(label, url)| {
                div()
                    .id("link")
                    .font_medium()
                    .underline()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .child(label)
            });
            v_flex()
                .gap_1()
                .child(
                    div()
                        .id("body")
                        .cursor_text()
                        .on_click(cx.listener(|this, _, window, cx| this.edit(window, cx)))
                        .child(self.body()),
                )
                .children(link.map(|link| h_flex().child(link)))
                .into_any_element()
        };

        let tones = self.tones.iter().enumerate().map(|(ix, tone)| {
            Button::new(("tone", ix))
                .outline()
                .small()
                .label(tone.label.clone())
                .selected(ix == self.tone)
                .disabled(sending)
                .on_click(cx.listener(move |this, _, _, cx| this.set_tone(ix, cx)))
        });
        let tones: Vec<_> = tones.collect();

        card.child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .px_4()
                .py_2()
                .border_t_1()
                .border_color(line)
                .child(div().text_color(quiet).child("To"))
                // The recipient keeps its full width while it fits; when it
                // doesn't, the attachment wraps onto its own line.
                .child(
                    div()
                        .flex_grow(1.)
                        .max_w_full()
                        .min_w_0()
                        .overflow_hidden()
                        .children(recipient.map(|recipient| h_flex().min_w_0().child(recipient))),
                )
                .children(attachment),
        )
        .child(
            v_flex()
                .gap_3()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
                .child(body)
                .child(
                    h_flex()
                        .gap_1()
                        .children(tones)
                        .when(!self.editing, |this| {
                            this.child(
                                Button::new("edit")
                                    .ghost()
                                    .small()
                                    .icon(IconName::CaseSensitive)
                                    .disabled(sending)
                                    .tooltip("Edit the text")
                                    .accessibility_label("Edit message")
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.edit(window, cx)),
                                    ),
                            )
                        })
                        .child(div().flex_1())
                        .when(self.editing, |this| {
                            this.child(
                                div().text_caption().text_color(quiet).child(
                                    "Escape to finish, the main modifier with Enter to send",
                                ),
                            )
                        }),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .px_4()
                .py_2p5()
                .border_t_1()
                .border_color(line)
                .children(
                    self.author
                        .clone()
                        .map(|author| div().truncate().child(author)),
                )
                .child(
                    h_flex().flex_1().child(
                        h_flex()
                            .gap_1p5()
                            .px_1p5()
                            .rounded(px(5.))
                            .bg(if sending { chip } else { success.opacity(0.16) })
                            .text_label_small()
                            .when(sending, |this| this.child(Spinner::new().xsmall()))
                            .child(if sending {
                                "Sending"
                            } else {
                                "Ready for review"
                            }),
                    ),
                )
                .child(
                    Button::new("discard")
                        .outline()
                        .small()
                        .label("Discard")
                        .disabled(sending)
                        .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
                )
                .child(
                    Button::new("send")
                        .primary()
                        .small()
                        .label("Send")
                        .disabled(sending || to.is_none())
                        .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_greeting_uses_the_first_name() {
        assert_eq!(
            DraftContact::new("Imani Okafor", "i@x.example").first_name(),
            "Imani"
        );
        assert_eq!(
            DraftContact::new("  Accounts  team", "a@x.example").first_name(),
            "Accounts"
        );
        assert_eq!(DraftContact::new("", "a@x.example").first_name(), "");
    }

    #[test]
    fn the_receipt_says_what_happened() {
        assert_eq!(
            receipt(Phase::Sent, "Imani").as_deref(),
            Some("Sent to Imani")
        );
        assert_eq!(receipt(Phase::Sent, "").as_deref(), Some("Sent"));
        assert_eq!(
            receipt(Phase::Discarded, "Imani").as_deref(),
            Some("Draft discarded")
        );
        assert_eq!(receipt(Phase::Draft, "Imani"), None);
        assert_eq!(receipt(Phase::Sending, "Imani"), None);
    }
}
