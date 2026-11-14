//! Message action bar: the quiet row of actions under an assistant's message.
//!
//! Copy, regenerate with a switcher between the versions of the answer, a vote
//! up or down that asks what was off, and read aloud. The bar stays faded until
//! it, or the message around it, is pointed at.

use std::{f32::consts::TAU, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt as _, App, ClickEvent, ElementId, Focusable as _, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, SharedString,
    StyleRefinement, Styled, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputState},
    v_flex,
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
    on_version_change: Option<VersionHandler>,
    on_copy: Option<Handler>,
    on_regenerate: Option<Handler>,
    on_feedback: Option<FeedbackHandler>,
    on_speak: Option<SpeakHandler>,
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
            on_version_change: None,
            on_copy: None,
            on_regenerate: None,
            on_feedback: None,
            on_speak: None,
        }
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

#[derive(Default)]
struct BarState {
    seeded: bool,
    version: usize,
    /// How many versions there were at the last draw.
    known_versions: usize,
    /// The vote on each version that has one.
    votes: Vec<(usize, Vote)>,
    row_open: bool,
    /// The picked reason. One past the last reason means something else.
    reason: Option<usize>,
    thanked: bool,
    playing: bool,
    _stop_playing: Option<Task<()>>,
    _close_row: Option<Task<()>>,
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
        let (kept, votes, row_open, reason, thanked, kept_playing) =
            state.update(cx, |state, _| {
                if !state.seeded {
                    state.seeded = true;
                    state.known_versions = count;
                    state.version = self.default_version.unwrap_or(count - 1);
                }
                state.version = settle_version(state.version, state.known_versions, count);
                state.known_versions = count;
                (
                    state.version,
                    state.votes.clone(),
                    state.row_open,
                    state.reason,
                    state.thanked,
                    state.playing,
                )
            });
        let version = self.version.unwrap_or(kept).min(count - 1);
        let vote = votes
            .iter()
            .find(|(voted, _)| *voted == version)
            .map(|(_, vote)| *vote);
        let playing = self.speaking.unwrap_or(kept_playing);
        let something_else = self.reasons.len();
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
        let cast = |pressed: Vote| {
            let state = state.clone();
            let on_feedback = self.on_feedback.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let next = vote_after(vote, pressed);
                state.update(cx, |state, cx| {
                    state.votes.retain(|(voted, _)| *voted != version);
                    state.votes.extend(next.map(|next| (version, next)));
                    // A vote down asks what was off.
                    state.row_open = next == Some(Vote::Down);
                    state.reason = None;
                    state.thanked = false;
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
            let reasons = self.reasons.clone();
            let on_feedback = self.on_feedback.clone();
            Rc::new(move |window, cx| {
                let why: Option<SharedString> = match reason {
                    Some(ix) if ix < reasons.len() => Some(reasons[ix].clone()),
                    Some(_) => {
                        let text = other.read(cx).value().trim().to_string();
                        (!text.is_empty()).then(|| text.into())
                    }
                    None => None,
                };
                let Some(why) = why else {
                    return;
                };
                other.update(cx, |input, cx| input.set_value("", window, cx));
                state.update(cx, |state, cx| {
                    state.thanked = true;
                    state._close_row = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(THANKS_FOR).await;
                        this.update(cx, |state, cx| {
                            state.row_open = false;
                            state.thanked = false;
                            state.reason = None;
                            cx.notify();
                        })
                        .ok();
                    }));
                    cx.notify();
                });
                if let Some(handler) = &on_feedback {
                    handler(Some(Vote::Down), Some(why), window, cx);
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

        let mut bar = h_flex().gap_0p5();
        for action in &self.actions {
            bar = match action {
                MessageAction::Copy => bar.child(
                    Clipboard::new((self.id.clone(), "copy"))
                        .value(self.text.clone())
                        .tooltip("Copy")
                        .when_some(self.on_copy.clone(), |this, on_copy| {
                            this.on_copied(move |_, window, cx| on_copy(window, cx))
                        }),
                ),
                MessageAction::Regenerate => bar
                    .child(
                        Button::new((self.id.clone(), "regenerate"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::RefreshCw)
                            .loading(self.regenerating)
                            .tooltip("Regenerate")
                            .on_click({
                                let on_regenerate = self.on_regenerate.clone();
                                move |_, window, cx| {
                                    if let Some(handler) = &on_regenerate {
                                        handler(window, cx);
                                    }
                                }
                            }),
                    )
                    .when(count > 1, |this| {
                        this.child(
                            h_flex()
                                .child(
                                    Button::new((self.id.clone(), "previous"))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::ChevronLeft)
                                        .disabled(version == 0)
                                        .tooltip("Previous version")
                                        .on_click(go(version.saturating_sub(1))),
                                )
                                .child(
                                    div()
                                        .px_0p5()
                                        .text_xs()
                                        .text_color(quiet)
                                        .child(format!("{} / {count}", version + 1)),
                                )
                                .child(
                                    Button::new((self.id.clone(), "next"))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::ChevronRight)
                                        .disabled(version + 1 >= count)
                                        .tooltip("Next version")
                                        .on_click(go((version + 1).min(count - 1))),
                                ),
                        )
                    }),
                MessageAction::Feedback => bar
                    .child(
                        Button::new((self.id.clone(), "up"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ThumbsUp)
                            .selected(vote == Some(Vote::Up))
                            .tooltip("Good answer")
                            .on_click(cast(Vote::Up)),
                    )
                    .child(
                        Button::new((self.id.clone(), "down"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ThumbsDown)
                            .selected(vote == Some(Vote::Down))
                            .tooltip("Poor answer")
                            .on_click(cast(Vote::Down)),
                    ),
                MessageAction::ReadAloud => {
                    let button = Button::new((self.id.clone(), "read-aloud"))
                        .ghost()
                        .xsmall()
                        .selected(playing)
                        .tooltip(if playing {
                            "Stop reading"
                        } else {
                            "Read aloud"
                        })
                        .on_click(toggle_speak.clone());
                    bar.child(if playing {
                        button.child(equalizer(accent, still))
                    } else {
                        button.icon(IconName::Play)
                    })
                }
            };
        }

        let can_send = match reason {
            Some(ix) if ix == something_else => !other.read(cx).value().trim().is_empty(),
            Some(_) => true,
            None => false,
        };
        let pick = |ix: usize| {
            let state = state.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    // Pressing the picked reason drops it.
                    state.reason = (state.reason != Some(ix)).then_some(ix);
                    cx.notify();
                })
            }
        };
        let row = row_open.then(|| {
            let labels = self
                .reasons
                .iter()
                .cloned()
                .chain([SharedString::from("Something else")]);
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
                        .child(div().font_medium().child(if thanked {
                            "Thanks, that helps."
                        } else {
                            "What was off?"
                        }))
                        .child(
                            Button::new((self.id.clone(), "close-row"))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .tooltip("Close")
                                .on_click(close_row),
                        ),
                )
                .when(!thanked, |this| {
                    this.child(
                        h_flex()
                            .flex_wrap()
                            .gap_1p5()
                            .children(labels.enumerate().map(|(ix, label)| {
                                Button::new((
                                    self.id.clone(),
                                    SharedString::from(format!("reason-{ix}")),
                                ))
                                .outline()
                                .xsmall()
                                .label(label)
                                .selected(reason == Some(ix))
                                .on_click(pick(ix))
                            })),
                    )
                    .when(reason == Some(something_else), |this| {
                        this.child(Input::new(&other).small())
                    })
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
            .text_sm()
            .line_height(relative(1.45))
            .child(bar.when(!lit, |this| {
                this.opacity(0.55)
                    .hover(|this| this.opacity(1.))
                    .when_some(self.hover_group.clone(), |this, group| {
                        this.group_hover(group, |this| this.opacity(1.))
                    })
            }))
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
