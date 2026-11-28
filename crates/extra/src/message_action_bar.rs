// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Message action bar: the quiet row of actions under an assistant's message.
//!
//! Copy, regenerate with a switcher between the versions of the answer, a vote
//! up or down that asks what was off, and read aloud. The bar stays faded until
//! it, or the message around it, is pointed at.
//!
//! With more than one version, the switcher's number slides in from the side
//! the switch came from, and a small map of dots, one per version, can sit
//! beside it: a dot is clicked to jump to its version, and the map takes Left,
//! Right, Home and End from the keyboard.
//!
//! A vote opens a row of reason chips, one or several, with an optional note,
//! and ends on a thank-you line that folds away or stays.

mod branches;

pub use branches::{SwapDirection, swap_direction, swap_in, swap_offset};

use std::{f32::consts::TAU, rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, Size,
    StyledExt as _,
    button::{Button, ButtonVariants as _},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputState},
    look, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, App, ClickEvent, ElementId, FocusHandle, Focusable as _, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

/// How long the row says thanks before it folds away.
const THANKS_FOR: Duration = Duration::from_millis(1600);

/// One of the actions the bar can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageAction {
    /// Copy the message.
    Copy,
    /// Ask for another answer, and switch between the answers so far.
    Regenerate,
    /// Vote the answer up or down.
    Feedback,
    /// Read the message aloud.
    ReadAloud,
}

/// A label the bar speaks and shows in a tooltip, which its owner can reword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageActionLabel {
    /// The copy button. "Copy" by default.
    Copy,
    /// The button that asks for another answer. "Regenerate" by default.
    Regenerate,
    /// The switcher's back button. "Previous version" by default.
    PreviousVersion,
    /// The switcher's forward button. "Next version" by default.
    NextVersion,
    /// The vote up. "Good answer" by default.
    GoodAnswer,
    /// The vote down. "Poor answer" by default.
    PoorAnswer,
    /// Read aloud while it is not reading. "Read aloud" by default.
    ReadAloud,
    /// Read aloud while it is reading. "Stop reading" by default.
    StopReading,
}

impl MessageActionLabel {
    /// The words used when the owner gives none.
    pub fn default_words(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Regenerate => "Regenerate",
            Self::PreviousVersion => "Previous version",
            Self::NextVersion => "Next version",
            Self::GoodAnswer => "Good answer",
            Self::PoorAnswer => "Poor answer",
            Self::ReadAloud => "Read aloud",
            Self::StopReading => "Stop reading",
        }
    }
}

/// A vote on an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vote {
    /// The answer was good.
    Up,
    /// The answer was not.
    Down,
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type VersionHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type FeedbackHandler = Rc<dyn Fn(Option<Vote>, Option<SharedString>, &mut Window, &mut App)>;
type DetailHandler = Rc<dyn Fn(&FeedbackDetail, &mut Window, &mut App)>;

/// Everything sent with a vote's reasons: the vote, the chips picked, in
/// the order they are offered, and the note, when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackDetail {
    /// The vote the reasons go with.
    pub vote: Vote,
    /// The reason chips picked.
    pub reasons: Vec<SharedString>,
    /// The note written, trimmed.
    pub note: Option<SharedString>,
}
type SpeakHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The actions under one assistant message.
#[derive(IntoElement)]
pub struct MessageActionBar {
    id: ElementId,
    style: StyleRefinement,
    text: SharedString,
    actions: Vec<MessageAction>,
    versions: usize,
    version: Option<usize>,
    default_version: Option<usize>,
    regenerating: bool,
    reasons: Vec<SharedString>,
    speaking: Option<bool>,
    speak_duration: Option<Duration>,
    quiet: bool,
    hover_group: Option<SharedString>,
    button_size: Size,
    icon_size: Option<Size>,
    pager_first: bool,
    pager_quiet: bool,
    labels: Vec<(MessageActionLabel, SharedString)>,
    on_version_change: Option<VersionHandler>,
    on_copy: Option<Handler>,
    on_regenerate: Option<Handler>,
    on_feedback: Option<FeedbackHandler>,
    on_speak: Option<SpeakHandler>,
    branch_map: bool,
    branch_labels: Vec<SharedString>,
    up_reasons: Vec<SharedString>,
    multi_reason: bool,
    note: bool,
    thanks: Option<SharedString>,
    keep_thanks: bool,
    thank_on_vote: bool,
    on_feedback_detail: Option<DetailHandler>,
}

impl MessageActionBar {
    /// Create a bar with all four actions for a message with one version.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            text: SharedString::default(),
            actions: vec![
                MessageAction::Copy,
                MessageAction::Regenerate,
                MessageAction::Feedback,
                MessageAction::ReadAloud,
            ],
            versions: 1,
            version: None,
            default_version: None,
            regenerating: false,
            reasons: vec![
                "Wrong or made up".into(),
                "Too long".into(),
                "Missed what I asked".into(),
            ],
            speaking: None,
            speak_duration: None,
            quiet: true,
            hover_group: None,
            button_size: Size::XSmall,
            icon_size: None,
            pager_first: false,
            pager_quiet: false,
            labels: Vec::new(),
            on_version_change: None,
            on_copy: None,
            on_regenerate: None,
            on_feedback: None,
            on_speak: None,
            branch_map: false,
            branch_labels: Vec::new(),
            up_reasons: Vec::new(),
            multi_reason: false,
            note: false,
            thanks: None,
            keep_thanks: false,
            thank_on_vote: false,
            on_feedback_detail: None,
        }
    }

    /// Show a small map of the versions beside the switcher: a dot for
    /// each, the one showing drawn as a short pill. A dot jumps to its
    /// version, and the map takes Left, Right, Home and End. Off by default.
    pub fn branch_map(mut self, map: bool) -> Self {
        self.branch_map = map;
        self
    }

    /// Name the versions, in order, such as "Shorter" or "With a table".
    /// The names go in the map's tooltips and spoken labels.
    pub fn branch_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.branch_labels = labels.into_iter().map(Into::into).collect();
        self
    }

    /// Offer these reasons after a vote up, asking what was good. Without
    /// them a vote up opens nothing.
    pub fn up_reasons(
        mut self,
        reasons: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.up_reasons = reasons.into_iter().map(Into::into).collect();
        self
    }

    /// Let several reason chips be picked at once. Off by default, where
    /// picking one drops the other.
    pub fn multi_reason(mut self, multi: bool) -> Self {
        self.multi_reason = multi;
        self
    }

    /// Always offer a note under the reason chips, sent with them or on its
    /// own, in place of the "Something else" chip. Off by default.
    pub fn feedback_note(mut self, note: bool) -> Self {
        self.note = note;
        self
    }

    /// Reword the thank-you line shown once reasons are sent. "Thanks, that
    /// helps." by default.
    pub fn thanks_message(mut self, words: impl Into<SharedString>) -> Self {
        self.thanks = Some(words.into());
        self
    }

    /// Keep the thank-you line until the vote changes, instead of folding
    /// it away after a moment. Off by default.
    pub fn keep_thanks(mut self, keep: bool) -> Self {
        self.keep_thanks = keep;
        self
    }

    /// Say thanks as soon as a vote is cast that opens no reasons, such as
    /// a vote up with no up reasons. Off by default.
    pub fn thank_on_vote(mut self, thank: bool) -> Self {
        self.thank_on_vote = thank;
        self
    }

    /// Run with everything sent with a vote's reasons: the chips picked and
    /// the note. `on_feedback` still hears the same send, with the reasons
    /// joined into one line.
    pub fn on_feedback_detail(
        mut self,
        handler: impl Fn(&FeedbackDetail, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_feedback_detail = Some(Rc::new(handler));
        self
    }

    /// Set the size of the bar's buttons. The default is extra small.
    pub fn button_size(mut self, size: Size) -> Self {
        self.button_size = size;
        self
    }

    /// Set the size of the icons apart from the buttons' size, such as
    /// 16 px icons in 28 px buttons. Without it the icons follow the buttons.
    pub fn icon_size(mut self, size: impl Into<Size>) -> Self {
        self.icon_size = Some(size.into());
        self
    }

    /// Draw the version switcher quietly: its arrows at 12 px in the muted
    /// text colour, beside its numbers, whatever the bar's own icon size.
    /// Off by default, where the arrows follow the other actions.
    pub fn pager_quiet(mut self, quiet: bool) -> Self {
        self.pager_quiet = quiet;
        self
    }

    /// Put the version switcher at the start of the bar, before the actions,
    /// instead of after Regenerate. Off by default.
    pub fn pager_first(mut self, first: bool) -> Self {
        self.pager_first = first;
        self
    }

    /// Reword one of the bar's labels, such as "Try again" for Regenerate.
    /// The words are spoken by a screen reader and shown in the tooltip.
    pub fn label(mut self, which: MessageActionLabel, words: impl Into<SharedString>) -> Self {
        self.labels.retain(|(label, _)| *label != which);
        self.labels.push((which, words.into()));
        self
    }

    /// The message text. Copy writes it to the clipboard, and read aloud is
    /// timed by its length when nothing else drives it.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = text.into();
        self
    }

    /// Choose which actions show, in this order.
    pub fn actions(mut self, actions: impl IntoIterator<Item = MessageAction>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }

    /// How many versions of the message there are. From two on, a switcher
    /// shows next to Regenerate.
    pub fn versions(mut self, versions: usize) -> Self {
        self.versions = versions;
        self
    }

    /// Control which version shows, counted from zero. Pair it with
    /// `on_version_change`.
    pub fn version(mut self, version: usize) -> Self {
        self.version = Some(version);
        self
    }

    /// Set the version shown at first when the bar keeps track itself. It
    /// starts on the newest, and jumps to a new version as soon as one is added.
    pub fn default_version(mut self, version: usize) -> Self {
        self.default_version = Some(version);
        self
    }

    /// Keep Regenerate busy and the bar lit while a new version is written.
    pub fn regenerating(mut self, regenerating: bool) -> Self {
        self.regenerating = regenerating;
        self
    }

    /// Set the quick reasons offered after a vote down. A last choice for
    /// something else, with a text field, is always added.
    pub fn reasons(mut self, reasons: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.reasons = reasons.into_iter().map(Into::into).collect();
        self
    }

    /// Control whether the message is being read aloud, tied to a speech
    /// player. Without this the bar plays for a while and stops by itself.
    pub fn speaking(mut self, speaking: bool) -> Self {
        self.speaking = Some(speaking);
        self
    }

    /// How long read aloud plays when the bar times it itself. By default it
    /// is estimated from the text.
    pub fn speak_duration(mut self, duration: Duration) -> Self {
        self.speak_duration = Some(duration);
        self
    }

    /// Whether the bar is faded until it is pointed at. On by default.
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Light the bar up while this hover group is pointed at. Put the same
    /// name on the message's wrapper with `.group(name)`.
    pub fn hover_group(mut self, group: impl Into<SharedString>) -> Self {
        self.hover_group = Some(group.into());
        self
    }

    /// Run when another version is picked in the switcher.
    pub fn on_version_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_version_change = Some(Rc::new(handler));
        self
    }

    /// Run after the message was copied.
    pub fn on_copy(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }

    /// Run when Regenerate is pressed. Add a version and the switcher shows it.
    pub fn on_regenerate(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_regenerate = Some(Rc::new(handler));
        self
    }

    /// Run with the vote as soon as it is cast, with `None` when it is taken
    /// back, and once more with `Vote::Down` and a reason when one is sent.
    pub fn on_feedback(
        mut self,
        handler: impl Fn(Option<Vote>, Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_feedback = Some(Rc::new(handler));
        self
    }

    /// Run with `true` when read aloud starts and `false` when it stops.
    pub fn on_speak(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_speak = Some(Rc::new(handler));
        self
    }
}

impl Styled for MessageActionBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The vote after a thumb is pressed: pressing the cast vote takes it back.
fn vote_after(current: Option<Vote>, pressed: Vote) -> Option<Vote> {
    (current != Some(pressed)).then_some(pressed)
}

/// How long reading a text aloud takes, at a calm pace and never too short to
/// notice.
fn speak_duration(text: &str) -> Duration {
    let words = text.split_whitespace().count() as u64;
    Duration::from_millis((words * 380).max(2600))
}

/// Which version shows once the number of versions is known: the kept one,
/// or the newest when one was just added.
fn settle_version(kept: usize, known: usize, count: usize) -> usize {
    let last = count.saturating_sub(1);
    if count > known { last } else { kept.min(last) }
}

/// The chips picked after pressing chip `ix`: pressing a picked chip drops
/// it, and with one at a time a new pick replaces the old.
fn toggle_pick(picked: &[usize], ix: usize, multi: bool) -> Vec<usize> {
    if picked.contains(&ix) {
        return picked.iter().copied().filter(|p| *p != ix).collect();
    }
    if !multi {
        return vec![ix];
    }
    let mut next = picked.to_vec();
    next.push(ix);
    next.sort_unstable();
    next
}

/// What a send carries: the picked chips by name, in order, and the note.
/// `None` when there is nothing to send. A chip past the last reason is
/// "Something else", which stands for the note.
fn gather(
    reasons: &[SharedString],
    picked: &[usize],
    note: &str,
) -> Option<(Vec<SharedString>, Option<SharedString>)> {
    let chips: Vec<SharedString> = picked
        .iter()
        .filter_map(|ix| reasons.get(*ix).cloned())
        .collect();
    let note = note.trim();
    let note = (!note.is_empty()).then(|| SharedString::from(note.to_string()));
    (!chips.is_empty() || note.is_some()).then_some((chips, note))
}

/// The one line `on_feedback` hears for a send: the chips joined, then the
/// note.
fn joined_reason(chips: &[SharedString], note: Option<&SharedString>) -> SharedString {
    let mut parts: Vec<&str> = chips.iter().map(|chip| chip.as_ref()).collect();
    parts.extend(note.map(|note| note.as_ref()));
    parts.join(", ").into()
}

#[derive(Default)]
struct BarState {
    seeded: bool,
    version: usize,
    /// How many versions there were at the last draw.
    known_versions: usize,
    /// The vote on each version that has one.
    votes: Vec<(usize, Vote)>,
    row_open: bool,
    /// The vote the open row asks about.
    row_vote: Option<Vote>,
    /// The picked reasons. One past the last reason means something else.
    picked: Vec<usize>,
    thanked: bool,
    playing: bool,
    /// The version drawn last, to tell which way a switch went.
    shown: Option<usize>,
    direction: SwapDirection,
    /// Counts the switches, so each one slides in afresh.
    swaps: usize,
    map_focus: Option<FocusHandle>,
    hovered: bool,
    _stop_playing: Option<Task<()>>,
    _close_row: Option<Task<()>>,
}

/// Fold the feedback row away once the thank-you line has been read.
fn fold_later(cx: &mut isonomic_ui::Context<BarState>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(THANKS_FOR).await;
        this.update(cx, |state, cx| {
            state.row_open = false;
            state.thanked = false;
            state.picked.clear();
            cx.notify();
        })
        .ok();
    })
}

/// Four bars that bounce while the message is read aloud.
fn equalizer(color: Hsla, still: bool) -> impl IntoElement {
    h_flex()
        .items_end()
        .gap(px(2.))
        .h(px(12.))
        .children((0..4usize).map(|bar| {
            let phase = bar as f32 * 0.23;
            let base = div().w(px(2.)).rounded(px(1.)).bg(color);
            if still {
                return base.h(px(5. + (bar % 2) as f32 * 4.)).into_any_element();
            }
            base.with_animation(
                ("bar", bar),
                Animation::new(Duration::from_millis(900)).repeat(),
                move |this, time| {
                    let wave = ((time + phase) * TAU).sin() * 0.5 + 0.5;
                    this.h(px(3. + 9. * wave))
                },
            )
            .into_any_element()
        }))
}

impl RenderOnce for MessageActionBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let accent = theme.info;
        let still = cx.reduce_motion();

        let count = self.versions.max(1);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| BarState::default());
        let other = window.use_keyed_state((self.id.clone(), "other"), cx, |window, cx| {
            InputState::new(window, cx).placeholder("Tell us more")
        });
        let (kept, votes, row_open, row_vote, picked, thanked, kept_playing) =
            state.update(cx, |state, cx| {
                if !state.seeded {
                    state.seeded = true;
                    state.known_versions = count;
                    state.version = self.default_version.unwrap_or(count - 1);
                }
                state.version = settle_version(state.version, state.known_versions, count);
                state.known_versions = count;
                if state.map_focus.is_none() {
                    state.map_focus = Some(cx.focus_handle().tab_stop(true));
                }
                (
                    state.version,
                    state.votes.clone(),
                    state.row_open,
                    state.row_vote,
                    state.picked.clone(),
                    state.thanked,
                    state.playing,
                )
            });
        let version = self.version.unwrap_or(kept).min(count - 1);
        // Tell which way the shown version moved since the last draw, so the
        // switcher's number slides in from that side.
        let (direction, swaps, map_focus) = state.update(cx, |state, _| {
            if let Some(shown) = state.shown
                && shown != version
            {
                state.direction = swap_direction(shown, version);
                state.swaps += 1;
            }
            state.shown = Some(version);
            (
                state.direction,
                state.swaps,
                state.map_focus.clone().expect("set above"),
            )
        });
        let hovered = state.read(cx).hovered;
        let lit_fade = look::hover_fade((self.id.clone(), "lit"), hovered, window, cx);
        let vote = votes
            .iter()
            .find(|(voted, _)| *voted == version)
            .map(|(_, vote)| *vote);
        let playing = self.speaking.unwrap_or(kept_playing);
        // The row asks about the vote that opened it, with that vote's chips.
        let row_reasons: Vec<SharedString> = if row_vote == Some(Vote::Up) {
            self.up_reasons.clone()
        } else {
            self.reasons.clone()
        };
        let something_else = row_reasons.len();
        let note_open = self.note || picked.contains(&something_else);
        let lit = !self.quiet || self.regenerating || playing || row_open;

        let go = |to: usize| {
            let state = state.clone();
            let on_version_change = self.on_version_change.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.version = to;
                    state.row_open = false;
                    cx.notify();
                });
                if let Some(handler) = &on_version_change {
                    handler(to, window, cx);
                }
            }
        };
        let has_up_reasons = !self.up_reasons.is_empty();
        let thank_on_vote = self.thank_on_vote;
        let keep_thanks = self.keep_thanks;
        let cast = |pressed: Vote| {
            let state = state.clone();
            let on_feedback = self.on_feedback.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let next = vote_after(vote, pressed);
                state.update(cx, |state, cx| {
                    state.votes.retain(|(voted, _)| *voted != version);
                    state.votes.extend(next.map(|next| (version, next)));
                    // A vote down asks what was off, and a vote up what was
                    // good when there are reasons for it; otherwise a vote
                    // can say thanks at once.
                    let asks = match next {
                        Some(Vote::Down) => true,
                        Some(Vote::Up) => has_up_reasons,
                        None => false,
                    };
                    state.row_open = asks || (next.is_some() && thank_on_vote);
                    state.row_vote = next;
                    state.picked.clear();
                    state.thanked = !asks && next.is_some() && thank_on_vote;
                    state._close_row = None;
                    if state.thanked && !keep_thanks {
                        state._close_row = Some(fold_later(cx));
                    }
                    cx.notify();
                });
                if let Some(handler) = &on_feedback {
                    handler(next, None, window, cx);
                }
            }
        };
        let close_row = {
            let state = state.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.row_open = false;
                    cx.notify();
                })
            }
        };
        let send: Handler = {
            let state = state.clone();
            let other = other.clone();
            let reasons = row_reasons.clone();
            let picked = picked.clone();
            let on_feedback = self.on_feedback.clone();
            let on_detail = self.on_feedback_detail.clone();
            let vote = row_vote.unwrap_or(Vote::Down);
            Rc::new(move |window, cx| {
                let note = if note_open {
                    other.read(cx).value().to_string()
                } else {
                    String::new()
                };
                let Some((chips, note)) = gather(&reasons, &picked, &note) else {
                    return;
                };
                other.update(cx, |input, cx| input.set_value("", window, cx));
                state.update(cx, |state, cx| {
                    state.thanked = true;
                    state._close_row = (!keep_thanks).then(|| fold_later(cx));
                    cx.notify();
                });
                if let Some(handler) = &on_feedback {
                    handler(
                        Some(vote),
                        Some(joined_reason(&chips, note.as_ref())),
                        window,
                        cx,
                    );
                }
                if let Some(handler) = &on_detail {
                    handler(
                        &FeedbackDetail {
                            vote,
                            reasons: chips,
                            note,
                        },
                        window,
                        cx,
                    );
                }
            })
        };
        let toggle_speak = {
            let state = state.clone();
            let on_speak = self.on_speak.clone();
            let controlled = self.speaking.is_some();
            let duration = self
                .speak_duration
                .unwrap_or_else(|| speak_duration(&self.text));
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let next = !playing;
                if !controlled {
                    let on_speak = on_speak.clone();
                    state.update(cx, |state, cx| {
                        state.playing = next;
                        // Left to itself, reading stops when the text is through.
                        state._stop_playing = next.then(|| {
                            cx.spawn_in(window, async move |this, cx| {
                                cx.background_executor().timer(duration).await;
                                cx.update(|window, cx| {
                                    let Some(this) = this.upgrade() else {
                                        return;
                                    };
                                    this.update(cx, |state: &mut BarState, cx| {
                                        state.playing = false;
                                        cx.notify();
                                    });
                                    if let Some(handler) = &on_speak {
                                        handler(false, window, cx);
                                    }
                                })
                                .ok();
                            })
                        });
                        cx.notify();
                    });
                }
                if let Some(handler) = &on_speak {
                    handler(next, window, cx);
                }
            }
        };

        let size = self.button_size;
        let icon_size = self.icon_size;
        let labels = self.labels.clone();
        let words = move |which: MessageActionLabel| -> SharedString {
            labels
                .iter()
                .find(|(label, _)| *label == which)
                .map(|(_, words)| words.clone())
                .unwrap_or_else(|| which.default_words().into())
        };
        // The switcher between the versions of the answer.
        let pager_quiet = self.pager_quiet;
        let arrow = move |name: IconName| {
            let icon = Icon::new(name);
            if pager_quiet {
                icon.text_color(quiet)
            } else {
                icon
            }
        };
        let arrow_size = if pager_quiet {
            Some(Size::Size(px(12.)))
        } else {
            icon_size
        };
        let pager = (count > 1 && self.actions.contains(&MessageAction::Regenerate)).then(|| {
            h_flex()
                .child(
                    Button::new((self.id.clone(), "previous"))
                        .ghost()
                        .with_size(size)
                        .when_some(arrow_size, |this, icon| this.icon_size(icon))
                        .icon(arrow(IconName::ChevronLeft))
                        .disabled(version == 0)
                        .accessibility_label(words(MessageActionLabel::PreviousVersion))
                        .tooltip(words(MessageActionLabel::PreviousVersion))
                        .on_click(go(version.saturating_sub(1))),
                )
                .child(swap_in(
                    (self.id.clone(), SharedString::from(format!("swap-{swaps}"))),
                    direction,
                    still,
                    div()
                        .px_0p5()
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{} / {count}", version + 1)),
                ))
                .child(
                    Button::new((self.id.clone(), "next"))
                        .ghost()
                        .with_size(size)
                        .when_some(arrow_size, |this, icon| this.icon_size(icon))
                        .icon(arrow(IconName::ChevronRight))
                        .disabled(version + 1 >= count)
                        .accessibility_label(words(MessageActionLabel::NextVersion))
                        .tooltip(words(MessageActionLabel::NextVersion))
                        .on_click(go((version + 1).min(count - 1))),
                )
                .when(self.branch_map, |this| {
                    let label = |n: usize| self.branch_labels.get(n).map(|label| label.as_ref());
                    let slots = branches::map_slots(count, version, branches::MAP_SLOTS);
                    let step = {
                        let state = state.clone();
                        let on_version_change = self.on_version_change.clone();
                        move |event: &isonomic_ui::KeyDownEvent,
                              window: &mut Window,
                              cx: &mut App| {
                            if event.keystroke.modifiers.modified() {
                                return;
                            }
                            let Some(to) = branches::map_step(version, count, &event.keystroke.key)
                            else {
                                return;
                            };
                            cx.stop_propagation();
                            if to == version {
                                return;
                            }
                            state.update(cx, |state, cx| {
                                state.version = to;
                                state.row_open = false;
                                cx.notify();
                            });
                            if let Some(handler) = &on_version_change {
                                handler(to, window, cx);
                            }
                        }
                    };
                    this.child(
                        h_flex()
                            .id((self.id.clone(), "branch-map"))
                            .track_focus(&map_focus)
                            .role(Role::Group)
                            .aria_label(branches::map_words(version, count, label(version)))
                            .on_key_down(step)
                            .ml_1()
                            .px_1()
                            .h(px(16.))
                            .gap(px(3.))
                            .rounded(px(4.))
                            .focus_visible(|style| style.border_1().border_color(accent))
                            .children(slots.into_iter().map(|slot| match slot {
                                branches::Slot::Gap => branches::gap_mark(quiet.opacity(0.6)),
                                branches::Slot::Version(n) => {
                                    let current = n == version;
                                    let words = branches::dot_words(n, label(n), current);
                                    div()
                                        .id((
                                            self.id.clone(),
                                            SharedString::from(format!("branch-{n}")),
                                        ))
                                        .role(Role::Button)
                                        .aria_label(words.clone())
                                        .flex()
                                        .items_center()
                                        .h(px(12.))
                                        .cursor_pointer()
                                        .on_click(go(n))
                                        .child(
                                            div()
                                                .w(px(branches::dot_width(current)))
                                                .h(px(6.))
                                                .rounded_full()
                                                .bg(if current {
                                                    accent
                                                } else {
                                                    quiet.opacity(0.45)
                                                }),
                                        )
                                        .into_any_element()
                                }
                            })),
                    )
                })
        });
        let (pager_first, mut pager_after) = if self.pager_first {
            (pager, None)
        } else {
            (None, pager)
        };

        let mut bar = h_flex().gap_0p5().children(pager_first);
        for action in &self.actions {
            bar = match action {
                MessageAction::Copy => bar.child(
                    Clipboard::new((self.id.clone(), "copy"))
                        .value(self.text.clone())
                        .with_size(size)
                        .when_some(icon_size, |this, icon| this.icon_size(icon))
                        .tooltip(words(MessageActionLabel::Copy))
                        .when_some(self.on_copy.clone(), |this, on_copy| {
                            this.on_copied(move |_, window, cx| on_copy(window, cx))
                        }),
                ),
                MessageAction::Regenerate => bar
                    .child(
                        Button::new((self.id.clone(), "regenerate"))
                            .ghost()
                            .with_size(size)
                            .when_some(icon_size, |this, icon| this.icon_size(icon))
                            .icon(IconName::RefreshCw)
                            .loading(self.regenerating)
                            .accessibility_label(words(MessageActionLabel::Regenerate))
                            .tooltip(words(MessageActionLabel::Regenerate))
                            .on_click({
                                let on_regenerate = self.on_regenerate.clone();
                                move |_, window, cx| {
                                    if let Some(handler) = &on_regenerate {
                                        handler(window, cx);
                                    }
                                }
                            }),
                    )
                    .children(pager_after.take()),
                MessageAction::Feedback => bar
                    .child(
                        Button::new((self.id.clone(), "up"))
                            .ghost()
                            .with_size(size)
                            .when_some(icon_size, |this, icon| this.icon_size(icon))
                            .icon(IconName::ThumbsUp)
                            .selected(vote == Some(Vote::Up))
                            .accessibility_label(words(MessageActionLabel::GoodAnswer))
                            .tooltip(words(MessageActionLabel::GoodAnswer))
                            .on_click(cast(Vote::Up)),
                    )
                    .child(
                        Button::new((self.id.clone(), "down"))
                            .ghost()
                            .with_size(size)
                            .when_some(icon_size, |this, icon| this.icon_size(icon))
                            .icon(IconName::ThumbsDown)
                            .selected(vote == Some(Vote::Down))
                            .accessibility_label(words(MessageActionLabel::PoorAnswer))
                            .tooltip(words(MessageActionLabel::PoorAnswer))
                            .on_click(cast(Vote::Down)),
                    ),
                MessageAction::ReadAloud => {
                    let label = words(if playing {
                        MessageActionLabel::StopReading
                    } else {
                        MessageActionLabel::ReadAloud
                    });
                    let button = Button::new((self.id.clone(), "read-aloud"))
                        .ghost()
                        .with_size(size)
                        .when_some(icon_size, |this, icon| this.icon_size(icon))
                        .selected(playing)
                        .accessibility_label(label.clone())
                        .tooltip(label)
                        .on_click(toggle_speak.clone());
                    bar.child(if playing {
                        button.child(equalizer(accent, still))
                    } else {
                        button.icon(IconName::Play)
                    })
                }
            };
        }

        let typed = if note_open {
            other.read(cx).value().to_string()
        } else {
            String::new()
        };
        let can_send = gather(&row_reasons, &picked, &typed).is_some();
        let multi = self.multi_reason;
        let pick = |ix: usize| {
            let state = state.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.picked = toggle_pick(&state.picked, ix, multi);
                    cx.notify();
                })
            }
        };
        let thanks_words = self
            .thanks
            .clone()
            .unwrap_or_else(|| "Thanks, that helps.".into());
        let question = if row_vote == Some(Vote::Up) {
            "What was good?"
        } else {
            "What was off?"
        };
        let row = row_open.then(|| {
            // With a note always on offer, "Something else" is not needed.
            let labels: Vec<SharedString> = if self.note {
                row_reasons.clone()
            } else {
                row_reasons
                    .iter()
                    .cloned()
                    .chain([SharedString::from("Something else")])
                    .collect()
            };
            v_flex()
                .gap_2()
                .pt_2()
                .on_key_down({
                    let send = send.clone();
                    let close_row = close_row.clone();
                    let other = other.clone();
                    move |event, window, cx| match event.keystroke.key.as_str() {
                        "escape" => {
                            close_row(&ClickEvent::default(), window, cx);
                            cx.stop_propagation();
                        }
                        "enter" if other.read(cx).focus_handle(cx).is_focused(window) => {
                            send(window, cx);
                            cx.stop_propagation();
                        }
                        _ => {}
                    }
                })
                .child(
                    h_flex()
                        .justify_between()
                        .child(if thanked {
                            let line = h_flex()
                                .gap_1p5()
                                .font_medium()
                                .child(Icon::new(IconName::CircleCheck).small().text_color(accent))
                                .child(thanks_words.clone());
                            if still {
                                line.into_any_element()
                            } else {
                                line.with_animation(
                                    (self.id.clone(), "thanks"),
                                    Animation::new(Duration::from_millis(220)),
                                    |this, delta| this.opacity(delta),
                                )
                                .into_any_element()
                            }
                        } else {
                            div().font_medium().child(question).into_any_element()
                        })
                        .child(
                            Button::new((self.id.clone(), "close-row"))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .accessibility_label("Close")
                                .tooltip("Close")
                                .on_click(close_row),
                        ),
                )
                .when(!thanked, |this| {
                    this.child(h_flex().flex_wrap().gap_1p5().children(
                        labels.into_iter().enumerate().map(|(ix, label)| {
                            Button::new((
                                self.id.clone(),
                                SharedString::from(format!("reason-{ix}")),
                            ))
                            .outline()
                            .xsmall()
                            .label(label)
                            .selected(picked.contains(&ix))
                            .on_click(pick(ix))
                        }),
                    ))
                    .when(note_open, |this| this.child(Input::new(&other).small()))
                    .child(
                        h_flex().justify_end().child(
                            Button::new((self.id.clone(), "send"))
                                .primary()
                                .small()
                                .label("Send")
                                .disabled(!can_send)
                                .on_click(move |_, window, cx| send(window, cx)),
                        ),
                    )
                })
        });

        v_flex()
            .id(self.id.clone())
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .child(
                bar.id((self.id.clone(), "actions"))
                    .on_hover({
                        let state = state.clone();
                        move |over, _, cx| {
                            state.update(cx, |state, cx| {
                                if state.hovered != *over {
                                    state.hovered = *over;
                                    cx.notify();
                                }
                            })
                        }
                    })
                    .when(!lit, |this| {
                        // Pointing at the bar fades it up; the message's own
                        // hover group still lights it as before.
                        this.opacity(0.55 + 0.45 * lit_fade)
                            .when_some(self.hover_group.clone(), |this, group| {
                                this.group_hover(group, |this| this.opacity(1.))
                            })
                    }),
            )
            .children(row)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressing_the_cast_vote_takes_it_back() {
        assert_eq!(vote_after(None, Vote::Up), Some(Vote::Up));
        assert_eq!(vote_after(Some(Vote::Up), Vote::Up), None);
        assert_eq!(vote_after(Some(Vote::Up), Vote::Down), Some(Vote::Down));
    }

    #[test]
    fn reading_takes_longer_for_longer_text() {
        assert_eq!(speak_duration("Short."), Duration::from_millis(2600));
        let twenty_words = "word ".repeat(20);
        assert_eq!(speak_duration(&twenty_words), Duration::from_millis(7600));
    }

    #[test]
    fn reason_chips_pick_one_or_several() {
        assert_eq!(toggle_pick(&[], 1, false), [1]);
        assert_eq!(toggle_pick(&[1], 2, false), [2]);
        assert_eq!(toggle_pick(&[1], 1, false), Vec::<usize>::new());
        assert_eq!(toggle_pick(&[2], 0, true), [0, 2]);
        assert_eq!(toggle_pick(&[0, 2], 2, true), [0]);
    }

    #[test]
    fn a_send_carries_the_chips_and_the_note() {
        let reasons: Vec<SharedString> = vec!["Too long".into(), "Wrong".into()];
        assert_eq!(gather(&reasons, &[], "  "), None);
        assert_eq!(
            gather(&reasons, &[1, 0], ""),
            Some((vec!["Wrong".into(), "Too long".into()], None))
        );
        // "Something else" sends only its note.
        assert_eq!(gather(&reasons, &[2], ""), None);
        assert_eq!(
            gather(&reasons, &[2], " the totals are off "),
            Some((Vec::new(), Some("the totals are off".into())))
        );
        assert_eq!(
            joined_reason(
                &["Too long".into(), "Wrong".into()],
                Some(&"see row 4".into())
            ),
            "Too long, Wrong, see row 4"
        );
    }

    #[test]
    fn a_new_version_is_shown_as_soon_as_it_is_added() {
        // Nothing new: the kept version stays.
        assert_eq!(settle_version(0, 3, 3), 0);
        // One was added: jump to it.
        assert_eq!(settle_version(0, 3, 4), 3);
        // Versions went away: stay inside what is left.
        assert_eq!(settle_version(3, 4, 2), 1);
        assert_eq!(settle_version(0, 0, 0), 0);
    }
}
