// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Setting toggle row: one on-or-off setting with a title, a line on what it does, and a switch, where the whole row flips it.
//!
//! While the owner saves a change the switch steps aside for a small
//! "Saving…" note, and a failed save shows its reason under the row. A row
//! can be locked with a reason, such as a plan that doesn't include it.

use isonomic_ui::base::PressableExt as _;
use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    switch::Switch, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

/// What a screen reader hears for the row, such as "Email receipts, on".
pub fn spoken_setting(title: &str, on: bool, saving: bool, locked: Option<&str>) -> String {
    let state = if on { "on" } else { "off" };
    match (saving, locked) {
        (true, _) => format!("{title}, {state}, saving"),
        (false, Some(reason)) => format!("{title}, {state}, locked: {reason}"),
        (false, None) => format!("{title}, {state}"),
    }
}

type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// One on-or-off setting.
#[derive(IntoElement)]
pub struct SettingToggleRow {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    description: Option<SharedString>,
    on: bool,
    saving: bool,
    error: Option<SharedString>,
    locked: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl SettingToggleRow {
    /// Create a row for a setting that is `on` or off.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, on: bool) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            description: None,
            on,
            saving: false,
            error: None,
            locked: None,
            on_change: None,
        }
    }

    /// Say what the setting does.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Show that a change is being saved.
    pub fn saving(mut self, saving: bool) -> Self {
        self.saving = saving;
        self
    }

    /// Show why the last change didn't save.
    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    /// Lock the setting and say why.
    pub fn locked(mut self, reason: impl Into<SharedString>) -> Self {
        self.locked = Some(reason.into());
        self
    }

    /// Run with the value asked for.
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for SettingToggleRow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SettingToggleRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let hover = theme.muted.opacity(0.5);
        let danger = theme.danger;
        let frozen = self.saving || self.locked.is_some();
        let on = self.on;
        let spoken = spoken_setting(&self.title, on, self.saving, self.locked.as_deref());
        let row_change = self.on_change.clone();
        let switch_change = self.on_change.clone();

        let control = if self.saving {
            div()
                .text_caption()
                .text_color(quiet)
                .child("Saving\u{2026}")
                .into_any_element()
        } else {
            Switch::new(ElementId::NamedChild(
                self.id.clone().into(),
                "switch".into(),
            ))
            .checked(on)
            .disabled(self.locked.is_some())
            .accessibility_label(self.title.clone())
            .on_change(move |value, window, cx| {
                if let Some(change) = &switch_change {
                    change(*value, window, cx);
                }
            })
            .into_any_element()
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_1()
            .px_3()
            .py_2p5()
            .rounded(px(6.))
            .relative()
            .role(Role::Group)
            .aria_label(spoken)
            .when(frozen, |this| this.pressable(false))
            .when(!frozen, |this| {
                // A click anywhere on the row flips the setting. It sits on a
                // layer with no role, so the switch stays the one control
                // that is announced.
                this.pressable(true).hover(move |s| s.bg(hover)).child(
                    div()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            "row-click".into(),
                        ))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .on_click(move |_, window, cx| {
                            if let Some(change) = &row_change {
                                change(!on, window, cx);
                            }
                        }),
                )
            })
            .child(
                h_flex()
                    .gap_4()
                    .justify_between()
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .text_label()
                                    .child(self.title.clone())
                                    .when(self.locked.is_some(), |this| {
                                        this.child(
                                            Icon::new(IconName::Ban).xsmall().text_color(quiet),
                                        )
                                    }),
                            )
                            .when_some(self.description, |this, text| {
                                this.child(div().text_caption().text_color(quiet).child(text))
                            })
                            .when_some(self.locked, |this, reason| {
                                this.child(div().text_caption().text_color(quiet).child(reason))
                            }),
                    )
                    .child(div().flex_none().child(control)),
            )
            .when_some(self.error, |this, error| {
                this.child(
                    h_flex()
                        .gap_1()
                        .text_caption()
                        .text_color(danger)
                        .child(Icon::new(IconName::CircleAlert).xsmall())
                        .child(error),
                )
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_read_their_state() {
        assert_eq!(
            spoken_setting("Email receipts", true, false, None),
            "Email receipts, on"
        );
        assert_eq!(
            spoken_setting("Email receipts", false, true, None),
            "Email receipts, off, saving"
        );
        assert_eq!(
            spoken_setting("Multi-currency", false, false, Some("Not on your plan")),
            "Multi-currency, off, locked: Not on your plan"
        );
    }
}
