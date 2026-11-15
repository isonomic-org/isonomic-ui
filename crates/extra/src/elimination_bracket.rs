//! Elimination bracket: a judge picks the best of several answers, two at a
//! time.
//!
//! Neighbours meet in the first round; the one the judge prefers moves on
//! and the other stays behind, faded. Any match opens both answers side by
//! side with the scores and the judge's reason, and the final winner can be
//! put to use.

use std::{collections::HashMap, rc::Rc};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// The height of one contender's slot.
const SLOT: f32 = 34.;

/// One answer in the bracket.
#[derive(Clone)]
pub struct Contender {
    id: SharedString,
    name: SharedString,
    icon: Option<IconName>,
    meta: Option<SharedString>,
    output: Option<SharedString>,
}

impl Contender {
    /// Create a contender, named after what makes it different.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            meta: None,
            output: None,
        }
    }

    /// Set an icon in front of the name.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set a quiet line about it, such as its size or who wrote it.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Set what it produced, shown when its match is opened.
    pub fn output(mut self, output: impl Into<SharedString>) -> Self {
        self.output = Some(output.into());
        self
    }
}

/// Which of a match's two contenders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The one drawn first.
    Upper,
    /// The one drawn second.
    Lower,
}

/// What the judge decided about one match.
#[derive(Debug, Clone)]
pub struct Verdict {
    winner: Side,
    reason: Option<SharedString>,
    scores: Option<(f32, f32)>,
}

impl Verdict {
    /// Create a verdict.
    pub fn new(winner: Side) -> Self {
        Self {
            winner,
            reason: None,
            scores: None,
        }
    }

    /// Say why, in one sentence.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the scores of the upper and the lower contender.
    pub fn scores(mut self, upper: f32, lower: f32) -> Self {
        self.scores = Some((upper, lower));
        self
    }
}

/// A match, by round and position in the round, both from 0.
pub type MatchKey = (usize, usize);

type PickHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A knockout bracket of answers, judged two at a time.
#[derive(IntoElement)]
pub struct EliminationBracket {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    task: Option<SharedString>,
    judge: Option<SharedString>,
    contenders: Vec<Contender>,
    verdicts: HashMap<MatchKey, Verdict>,
    judging: Option<MatchKey>,
    on_pick: Option<PickHandler>,
}

impl EliminationBracket {
    /// Create a bracket with no contenders.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Judge".into(),
            task: None,
            judge: None,
            contenders: Vec::new(),
            verdicts: HashMap::new(),
            judging: None,
            on_pick: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Say what the answers are for.
    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Name who judges.
    pub fn judge(mut self, judge: impl Into<SharedString>) -> Self {
        self.judge = Some(judge.into());
        self
    }

    /// Add a contender. Neighbours meet first; use 2, 4, 8 or 16.
    pub fn contender(mut self, contender: Contender) -> Self {
        self.contenders.push(contender);
        self
    }

    /// Record a verdict. Only add one once both contenders are known.
    pub fn verdict(mut self, round: usize, position: usize, verdict: Verdict) -> Self {
        self.verdicts.insert((round, position), verdict);
        self
    }

    /// Mark the match the judge is working on.
    pub fn judging(mut self, round: usize, position: usize) -> Self {
        self.judging = Some((round, position));
        self
    }

    /// Offer to use the final winner, and run with its id.
    pub fn on_pick(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl Styled for EliminationBracket {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// How many rounds a bracket of `contenders` has.
fn round_count(contenders: usize) -> usize {
    let mut rounds = 0;
    while (1 << rounds) < contenders {
        rounds += 1;
    }
    rounds
}

/// The name of a round, counted back from the final.
fn round_name(round: usize, rounds: usize) -> String {
    match rounds - round {
        1 => "Final".into(),
        2 => "Semifinal".into(),
        3 => "Quarterfinal".into(),
        left => format!("Round of {}", 1 << left),
    }
}

/// Who sits in each slot of each round, by contender position. A slot is
/// empty until the match feeding it is decided.
fn slots(contenders: usize, verdicts: &HashMap<MatchKey, Verdict>) -> Vec<Vec<Option<usize>>> {
    let rounds = round_count(contenders);
    let mut slots = vec![
        (0..1 << rounds)
            .map(|ix| (ix < contenders).then_some(ix))
            .collect(),
    ];
    for round in 0..rounds {
        let previous: &Vec<Option<usize>> = &slots[round];
        let next = (0..previous.len() / 2)
            .map(|position| {
                let verdict = verdicts.get(&(round, position))?;
                match verdict.winner {
                    Side::Upper => previous[position * 2],
                    Side::Lower => previous[position * 2 + 1],
                }
            })
            .collect();
        slots.push(next);
    }
    slots
}

/// The match an arrow key moves to.
fn step_match(at: MatchKey, rounds: usize, key: &str) -> Option<MatchKey> {
    let (round, position) = at;
    let in_round = (1usize << rounds) >> (round + 1);
    match key {
        "right" if round + 1 < rounds => Some((round + 1, position / 2)),
        "left" if round > 0 => Some((round - 1, position * 2)),
        "down" if position + 1 < in_round => Some((round, position + 1)),
        "up" if position > 0 => Some((round, position - 1)),
        _ => None,
    }
}

/// A score with one decimal.
fn score_words(score: f32) -> String {
    format!("{score:.1}")
}

struct BracketState {
    /// The match opened by hand.
    opened: Option<MatchKey>,
    focus: FocusHandle,
}

impl RenderOnce for EliminationBracket {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| BracketState {
            opened: None,
            focus: cx.focus_handle().tab_stop(true),
        });
        let (opened, focus) = {
            let state = state.read(cx);
            (state.opened, state.focus.clone())
        };
        let rounds = round_count(self.contenders.len());
        let slots = slots(self.contenders.len(), &self.verdicts);
        let total = (1usize << rounds).saturating_sub(1);
        let decided = self.verdicts.len().min(total);
        let champion = slots
            .last()
            .and_then(|last| last.first().copied().flatten());
        let champion = champion.filter(|_| rounds > 0);
        // The latest decided match, for when nothing is being judged.
        let latest = self.verdicts.keys().copied().max();
        let shown = opened
            .or(self.judging)
            .or(latest)
            .filter(|(round, _)| *round < rounds);

        // A match in words: "Semi-final: Draft A against Draft B, Draft A won".
        let match_words = |key: MatchKey| {
            let (round, position) = key;
            let name = |slot: Option<usize>| {
                slot.and_then(|ix| self.contenders.get(ix))
                    .map(|c| c.name.to_string())
                    .unwrap_or_else(|| "waiting".into())
            };
            let upper = slots[round].get(position * 2).copied().flatten();
            let lower = slots[round].get(position * 2 + 1).copied().flatten();
            let result = match self.verdicts.get(&key) {
                Some(verdict) => {
                    let winner = match verdict.winner {
                        Side::Upper => name(upper),
                        Side::Lower => name(lower),
                    };
                    format!(", {winner} won")
                }
                None if self.judging == Some(key) => ", being judged".to_string(),
                None => String::new(),
            };
            format!("{}: {} against {}{result}", round_name(round, rounds), name(upper), name(lower))
        };
        let open = |key: MatchKey| {
            let state = state.clone();
            move |_: &isonomic_ui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |state, cx| {
                    state.opened = (state.opened != Some(key)).then_some(key);
                    cx.notify();
                })
            }
        };
        let contender_row = |slot: Option<usize>, side: Side, key: MatchKey| {
            let verdict = self.verdicts.get(&key);
            let lost = verdict.is_some_and(|verdict| verdict.winner != side);
            let won = verdict.is_some_and(|verdict| verdict.winner == side);
            let score = verdict
                .and_then(|verdict| verdict.scores)
                .map(|scores| match side {
                    Side::Upper => scores.0,
                    Side::Lower => scores.1,
                });
            let contender = slot.and_then(|ix| self.contenders.get(ix));
            h_flex()
                .gap_2()
                .h(px(SLOT))
                .px_2()
                .when(lost, |this| this.opacity(0.45))
                .map(|this| match contender {
                    Some(contender) => this
                        .children(
                            contender
                                .icon
                                .clone()
                                .map(|icon| Icon::new(icon).xsmall().text_color(quiet)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .when(won, |this| this.font_medium())
                                .child(contender.name.clone()),
                        )
                        .children(score.map(|score| {
                            div()
                                .flex_none()
                                .text_xs()
                                .when(!won, |this| this.text_color(quiet))
                                .child(score_words(score))
                        }))
                        .when(won && score.is_none(), |this| {
                            this.child(Icon::new(IconName::Check).xsmall().text_color(success))
                        }),
                    None => this.child(div().text_xs().text_color(quiet).child("Waiting")),
                })
        };

        let columns = (0..rounds).map(|round| {
            let matches = slots[round].len() / 2;
            let column = (0..matches).map(|position| {
                let key = (round, position);
                let is_shown = shown == Some(key);
                let is_judging = self.judging == Some(key);
                let upper = slots[round][position * 2];
                let lower = slots[round][position * 2 + 1];
                div()
                    .id(("match", round * 64 + position))
                    .role(Role::Button)
                    .aria_label(match_words(key))
                    .aria_selected(is_shown)
                    .relative()
                    .rounded(radius)
                    .border_1()
                    .border_color(if is_shown { accent } else { line })
                    .bg(fill)
                    .cursor_pointer()
                    .on_click(open(key))
                    .child(contender_row(upper, Side::Upper, key))
                    .child(div().h(px(1.)).bg(line))
                    .child(contender_row(lower, Side::Lower, key))
                    .when(is_judging, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(-6.))
                                .right(px(-6.))
                                .p_0p5()
                                .rounded_full_style(cx)
                                .bg(fill)
                                .child(Spinner::new().xsmall().color(accent)),
                        )
                    })
            });
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child(round_name(round, rounds).to_uppercase()),
                )
                .child(v_flex().flex_1().justify_around().gap_2().children(column))
        });
        let columns: Vec<_> = columns.collect();
        let winner_column = champion
            .and_then(|ix| self.contenders.get(ix))
            .map(|winner| {
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .id("winner")
                    .role(Role::Status)
                    .aria_label(format!("Winner: {}", winner.name))
                    .child(div().text_xs().text_color(quiet).child("WINNER"))
                    .child(
                        v_flex().flex_1().justify_center().child(
                            h_flex()
                                .gap_2()
                                .h(px(SLOT + 6.))
                                .px_2()
                                .rounded(radius)
                                .border_1()
                                .border_color(success.opacity(0.5))
                                .bg(success.opacity(0.08))
                                .child(Icon::new(IconName::StarFill).xsmall().text_color(success))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .font_medium()
                                        .child(winner.name.clone()),
                                ),
                        ),
                    )
            });

        let is_focused = focus.is_focused(window);
        let bracket_name = match shown {
            Some(key) => format!("Bracket, {decided} of {total} matches decided. {}", match_words(key)),
            None => format!("Bracket, {decided} of {total} matches decided"),
        };
        let bracket = h_flex()
            .id((self.id.clone(), "bracket"))
            .role(Role::Group)
            .aria_label(bracket_name)
            .track_focus(&focus)
            .on_key_down({
                let state = state.clone();
                let judging = self.judging;
                move |event, _, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let next = match event.keystroke.key.as_str() {
                        "escape" => None,
                        key => match shown.and_then(|at| step_match(at, rounds, key)) {
                            Some(next) => Some(next),
                            None if shown.is_none() && rounds > 0 => Some((0, 0)),
                            None => return,
                        },
                    };
                    state.update(cx, |state, cx| {
                        state.opened = next.filter(|next| Some(*next) != judging);
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            })
            .items_stretch()
            .gap_4()
            .p_1()
            .rounded(radius)
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .children(columns)
            .children(winner_column);

        let panel = shown.map(|key| {
            let (round, position) = key;
            let verdict = self.verdicts.get(&key);
            let pair = [
                (slots[round][position * 2], Side::Upper),
                (slots[round][position * 2 + 1], Side::Lower),
            ];
            let status = if self.judging == Some(key) {
                match &self.judge {
                    Some(judge) => format!("{judge} is comparing these"),
                    None => "Being compared".to_string(),
                }
            } else if verdict.is_some() {
                "Decided".to_string()
            } else {
                "Not judged yet".to_string()
            };
            let cards = pair.map(|(slot, side)| {
                let contender = slot.and_then(|ix| self.contenders.get(ix));
                let won = verdict.is_some_and(|verdict| verdict.winner == side);
                let score = verdict
                    .and_then(|verdict| verdict.scores)
                    .map(|scores| match side {
                        Side::Upper => scores.0,
                        Side::Lower => scores.1,
                    });
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .p_3()
                    .rounded(radius)
                    .border_1()
                    .border_color(if won { success.opacity(0.5) } else { line })
                    .bg(chip.opacity(0.5))
                    .map(|this| match contender {
                        Some(contender) => this
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .font_medium()
                                            .child(contender.name.clone()),
                                    )
                                    .children(score.map(|score| {
                                        div().flex_none().font_medium().child(score_words(score))
                                    })),
                            )
                            .children(contender.meta.clone().map(|meta| {
                                div().truncate().text_xs().text_color(quiet).child(meta)
                            }))
                            .children(contender.output.clone().map(|output| {
                                div().text_xs().line_height(relative(1.5)).child(output)
                            })),
                        None => this.child(div().text_color(quiet).child("Not known yet")),
                    })
            });
            let use_winner = self
                .on_pick
                .clone()
                .zip(champion.and_then(|ix| self.contenders.get(ix)))
                .filter(|_| round + 1 == rounds)
                .map(|(handler, winner)| {
                    let id = winner.id.clone();
                    Button::new((self.id.clone(), "use"))
                        .primary()
                        .small()
                        .label(format!("Use {}", winner.name))
                        .on_click(move |_, window, cx| handler(&id, window, cx))
                });
            let [upper, lower] = cards;
            v_flex()
                .gap_2()
                .p_3()
                .rounded(radius_lg)
                .border_1()
                .border_color(line)
                .bg(fill)
                .shadow(resting_shadow(cx))
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().font_medium().child(match rounds - round {
                            1 => "Final".to_string(),
                            _ => format!("{} {}", round_name(round, rounds), position + 1),
                        }))
                        .child(div().flex_1())
                        .child(div().text_xs().text_color(quiet).child(status)),
                )
                .child(h_flex().items_start().gap_2().child(upper).child(lower))
                .children(
                    verdict
                        .and_then(|verdict| verdict.reason.clone())
                        .map(|reason| div().text_xs().text_color(quiet).child(reason)),
                )
                .children(use_winner.map(|button| h_flex().justify_end().child(button)))
        });

        let progress = match shown {
            Some((round, _)) => format!("{}, {decided} of {total}", round_name(round, rounds)),
            None => format!("{decided} of {total}"),
        };

        v_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label(self.title.clone())
            .w_full()
            .gap_3()
            .text_sm()
            .line_height(relative(1.4))
            .text_color(ink)
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Network).small().text_color(quiet))
                    .child(div().flex_none().font_medium().child(self.title))
                    .children(
                        self.task
                            .map(|task| div().min_w_0().truncate().text_color(quiet).child(task)),
                    )
                    .child(div().flex_1())
                    .when(self.judging.is_some(), |this| {
                        this.child(
                            h_flex()
                                .flex_none()
                                .gap_1()
                                .px_1p5()
                                .rounded(px(5.))
                                .bg(accent.opacity(0.12))
                                .text_xs()
                                .child(Spinner::new().xsmall().color(accent))
                                .child("Judging"),
                        )
                    })
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(quiet)
                            .child(progress),
                    ),
            )
            .child(bracket)
            .children(panel)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_cover_every_contender() {
        assert_eq!(round_count(2), 1);
        assert_eq!(round_count(4), 2);
        assert_eq!(round_count(8), 3);
        assert_eq!(round_count(5), 3);
    }

    #[test]
    fn rounds_are_named_back_from_the_final() {
        assert_eq!(round_name(2, 3), "Final");
        assert_eq!(round_name(1, 3), "Semifinal");
        assert_eq!(round_name(0, 3), "Quarterfinal");
        assert_eq!(round_name(0, 4), "Round of 16");
    }

    #[test]
    fn winners_move_on_and_undecided_slots_wait() {
        let mut verdicts = HashMap::new();
        verdicts.insert((0, 0), Verdict::new(Side::Lower));
        verdicts.insert((0, 1), Verdict::new(Side::Upper));
        verdicts.insert((1, 0), Verdict::new(Side::Upper));
        let slots = slots(4, &verdicts);
        assert_eq!(slots[1], [Some(1), Some(2)]);
        assert_eq!(slots[2], [Some(1)]);

        let open = slots_without_final();
        assert_eq!(open[1], [Some(0), None]);
    }

    fn slots_without_final() -> Vec<Vec<Option<usize>>> {
        let mut verdicts = HashMap::new();
        verdicts.insert((0, 0), Verdict::new(Side::Upper));
        slots(4, &verdicts)
    }

    #[test]
    fn arrows_follow_the_bracket() {
        assert_eq!(step_match((0, 3), 3, "right"), Some((1, 1)));
        assert_eq!(step_match((1, 1), 3, "left"), Some((0, 2)));
        assert_eq!(step_match((0, 3), 3, "down"), None);
        assert_eq!(step_match((0, 2), 3, "down"), Some((0, 3)));
        assert_eq!(step_match((2, 0), 3, "right"), None);
        assert_eq!(step_match((0, 0), 3, "up"), None);
    }
}
