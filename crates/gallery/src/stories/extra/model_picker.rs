//! Gallery story for the model picker.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use gpui_component::{ActiveTheme as _, IconName, h_flex, v_flex};
use isonomic_ui_extra::model_picker::{ModelOption, ModelPicker, PickerSetting, SettingChange};

/// Shows the picker in the toolbar of a composer and reports every change.
pub struct ModelPickerStory {
    last: SharedString,
}

impl ModelPickerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing changed yet".into(),
        })
    }
}

impl Render for ModelPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let radius = theme.radius_lg;
        let models = cx.entity();
        let settings = models.clone();

        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(420.))
                    .gap_4()
                    .p_3()
                    .rounded(radius)
                    .border_1()
                    .border_color(line)
                    .child(div().text_color(quiet).child("Ask about the books"))
                    .child(
                        h_flex().child(
                            ModelPicker::new("composer-model")
                                .columns(["Speed", "Depth", "Price"])
                                .model(
                                    ModelOption::new("atlas", "Atlas")
                                        .tagline("Month-end and audits")
                                        .ratings([2, 5, 5])
                                        .note("Starts in about 3 s, 18.00 per million words"),
                                )
                                .model(
                                    ModelOption::new("tally", "Tally")
                                        .tagline("Daily bookkeeping")
                                        .ratings([4, 4, 3])
                                        .note("Starts in about 1 s, 4.50 per million words"),
                                )
                                .model(
                                    ModelOption::new("dash", "Dash")
                                        .tagline("Quick lookups")
                                        .ratings([5, 2, 1])
                                        .note("Starts at once, 0.60 per million words"),
                                )
                                .model(
                                    ModelOption::new("switchboard", "Switchboard")
                                        .tagline("Chooses per request")
                                        .icon(IconName::Network)
                                        .note("Billed as whichever model answers"),
                                )
                                .default_value("tally")
                                .setting(
                                    PickerSetting::new("reasoning", "Reason it through")
                                        .icon(IconName::Cpu)
                                        .default_on(true)
                                        .levels(["Brief", "Standard", "Thorough"])
                                        .levels_label("How far")
                                        .default_level(1),
                                )
                                .setting(
                                    PickerSetting::new("rates", "Check live rates")
                                        .icon(IconName::Globe),
                                )
                                .on_change(move |id, _, cx| {
                                    let line = format!("Model: {id}");
                                    models.update(cx, |this, cx| {
                                        this.last = line.into();
                                        cx.notify();
                                    })
                                })
                                .on_setting_change(move |id, change, _, cx| {
                                    let line = match change {
                                        SettingChange::Switched(true) => format!("{id}: on"),
                                        SettingChange::Switched(false) => format!("{id}: off"),
                                        SettingChange::Level(level) => {
                                            format!("{id}: level {}", level + 1)
                                        }
                                    };
                                    settings.update(cx, |this, cx| {
                                        this.last = line.into();
                                        cx.notify();
                                    })
                                }),
                        ),
                    ),
            )
            .child(div().text_xs().text_color(quiet).child(self.last.clone()))
    }
}
