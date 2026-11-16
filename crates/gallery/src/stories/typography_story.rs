use isonomic_ui::{
    App, AppContext, Context, Entity, Focusable, FontWeight, IntoElement, ParentElement, Render,
    Styled, Window, div, px,
};

use isonomic_ui::component::{ActiveTheme as _, v_flex};

use crate::section;

pub struct TypographyStory {
    focus_handle: isonomic_ui::FocusHandle,
}

impl super::Story for TypographyStory {
    fn title() -> &'static str {
        "Typography"
    }

    fn description() -> &'static str {
        "The three bundled families: sans for the interface and wordmarks, mono for code and figures, serif for long-form reading"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl TypographyStory {
    pub(crate) fn new(_: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Focusable for TypographyStory {
    fn focus_handle(&self, _: &isonomic_ui::App) -> isonomic_ui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TypographyStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sans = theme.font_family.clone();
        let mono = theme.mono_font_family.clone();
        let serif = theme.serif_font_family.clone();
        let quiet = theme.muted_foreground;
        let sample = "The quick brown fox jumps over the lazy dog 0123456789";

        v_flex()
            .w_full()
            .items_center()
            .gap_6()
            .child(
                section("Sans, for the interface and wordmarks")
                    .description(
                        "The theme's font_family. Light is for the product word of a wordmark.",
                    )
                    .w_full()
                    .max_w(px(720.))
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .font_family(sans)
                            .child(
                                div()
                                    .font_weight(FontWeight::LIGHT)
                                    .child(format!("Light: {sample}")),
                            )
                            .child(div().child(format!("Regular: {sample}")))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("Medium: {sample}")),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("Semibold: {sample}")),
                            ),
                    ),
            )
            .child(
                section("Mono, for code and figures")
                    .description("The theme's mono_font_family.")
                    .w_full()
                    .max_w(px(720.))
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .font_family(mono)
                            .text_size(px(13.))
                            .child(div().child(format!("Regular: {sample}")))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("Medium: {sample}")),
                            ),
                    ),
            )
            .child(
                section("Serif, for long-form reading")
                    .description(
                        "An article in a read mode, and rarely a chat bubble. Never a wordmark, \
                         a heading, a greeting or interface text. The theme's serif_font_family.",
                    )
                    .w_full()
                    .max_w(px(720.))
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(560.))
                            .gap_3()
                            .font_family(serif)
                            .line_height(isonomic_ui::relative(1.6))
                            .child(div().child(
                                "A ledger is a record kept so that someone else can check it. \
                                 That is its whole purpose, and it shapes everything about how \
                                 one is written: each entry dated, each amount set against its \
                                 source, nothing rubbed out.",
                            ))
                            .child(div().font_weight(FontWeight::MEDIUM).child(
                                "The medium weight marks the few words in a passage that must \
                                 not be missed.",
                            ))
                            .child(div().text_color(quiet).child(format!("Regular: {sample}"))),
                    ),
            )
    }
}
