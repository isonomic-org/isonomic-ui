//! Gallery story for the labelled icon button.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::labelled_icon_button::LabelledIconButton;

/// A toolbar over a ledger: plain buttons with shortcuts, two toggles and an
/// inbox with a count. The story flips the toggles and clears the inbox.
pub struct LabelledIconButtonStory {
    pinned: bool,
    muted: bool,
    inbox: u32,
}

impl LabelledIconButtonStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            pinned: true,
            muted: false,
            inbox: 12,
        })
    }
}

impl Story for LabelledIconButtonStory {
    fn title() -> &'static str {
        "Labelled icon button"
    }

    fn description() -> &'static str {
        "A square icon button that can't be built without a name, shows that name on hover, can stay pressed, and can carry a count."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for LabelledIconButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (pin, mute, inbox) = (cx.entity(), cx.entity(), cx.entity());
        let theme = cx.theme();
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_1()
                    .p_1()
                    .rounded(isonomic_ui::px(9.))
                    .border_1()
                    .border_color(theme.border)
                    .w_auto()
                    .child(
                        LabelledIconButton::new("search", IconName::Search, "Search the ledger")
                            .shortcut("cmd-k"),
                    )
                    .child(
                        LabelledIconButton::new("copy", IconName::Copy, "Copy selected lines")
                            .shortcut("cmd-c"),
                    )
                    .child(LabelledIconButton::new(
                        "refresh",
                        IconName::RefreshCw,
                        "Refresh from the bank",
                    ))
                    .child(
                        div()
                            .w(isonomic_ui::px(1.))
                            .h(isonomic_ui::px(18.))
                            .bg(theme.border),
                    )
                    .child(
                        LabelledIconButton::new("pin", IconName::Star, "Pin this view")
                            .pressed(self.pinned)
                            .on_click(move |_, cx| {
                                pin.update(cx, |this, cx| {
                                    this.pinned = !this.pinned;
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        LabelledIconButton::new("mute", IconName::Bell, "Notifications")
                            .pressed(!self.muted)
                            .on_click(move |_, cx| {
                                mute.update(cx, |this, cx| {
                                    this.muted = !this.muted;
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        LabelledIconButton::new("inbox", IconName::Inbox, "Review inbox")
                            .count(self.inbox)
                            .on_click(move |_, cx| {
                                inbox.update(cx, |this, cx| {
                                    this.inbox = 0;
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Hover a button for its name and shortcut."),
            )
    }
}
