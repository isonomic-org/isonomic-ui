// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the artifact panel.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, TextRole as _, h_flex, v_flex};
use isonomic_ui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div, px,
};
use isonomic_ui_extra::artifact_panel::{ArtifactPanel, ArtifactTab, ArtifactVersion};

const V1: &str = "# Picnic plan\n\n- Saturday at the lake\n- Bring blankets\n";
const V2: &str = "# Picnic plan\n\n- Saturday at the lake, noon\n- Bring blankets and a kite\n- Sandwiches: cheese, egg\n";
const V3: &str = "# Picnic plan\n\nWhen: Saturday, noon\nWhere: the north shore of the lake\n\n## Bring\n- Blankets\n- A kite\n- Sandwiches: cheese, egg, cucumber\n- Lemonade\n\n## If it rains\nMove to the boathouse.\n";

/// A made-up rendered plan: a title and a short checklist.
fn plan_preview(title: &'static str, items: &'static [&'static str], cx: &App) -> AnyElement {
    let quiet = cx.theme().muted_foreground;
    v_flex()
        .p_4()
        .gap_2()
        .child(div().text_title().child(title))
        .children(items.iter().map(move |item| {
            h_flex()
                .gap_2()
                .child(div().size(px(6.)).rounded_full().bg(quiet))
                .child(div().text_body().child(*item))
        }))
        .into_any_element()
}

/// A plan with three versions, beside a narrow one with no versions yet.
pub struct ArtifactPanelStory {
    last: SharedString,
}

impl ArtifactPanelStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Copy, download or step through the versions".into(),
        })
    }
}

impl Story for ArtifactPanelStory {
    fn title() -> &'static str {
        "Artifact panel"
    }

    fn description() -> &'static str {
        "A generated artifact beside the conversation, with Preview and Source tabs, its versions, and copy and download."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ArtifactPanelStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let say = |cx: &mut Context<Self>| {
            let this = cx.entity();
            move |text: String, cx: &mut App| {
                this.update(cx, |this, cx| {
                    this.last = text.into();
                    cx.notify();
                })
            }
        };
        let on_copy = say(cx);
        let on_download = say(cx);
        let on_version = say(cx);
        let on_tab = say(cx);

        v_flex()
            .gap_6()
            .child(div().text_sm().text_color(quiet).child(self.last.clone()))
            .child(
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .gap_6()
                    .child(
                        ArtifactPanel::new("plan", "Picnic plan", "picnic-plan.md")
                            .kind("Document")
                            .w(px(440.))
                            .min_height(px(260.))
                            .version(
                                ArtifactVersion::new("v1", V1)
                                    .note("First draft")
                                    .time("12 min ago")
                                    .preview(|_, cx| {
                                        plan_preview(
                                            "Picnic plan",
                                            &["Saturday at the lake", "Bring blankets"],
                                            cx,
                                        )
                                    }),
                            )
                            .version(
                                ArtifactVersion::new("v2", V2)
                                    .note("Added food")
                                    .time("7 min ago")
                                    .preview(|_, cx| {
                                        plan_preview(
                                            "Picnic plan",
                                            &[
                                                "Saturday at the lake, noon",
                                                "Bring blankets and a kite",
                                                "Sandwiches: cheese, egg",
                                            ],
                                            cx,
                                        )
                                    }),
                            )
                            .version(
                                ArtifactVersion::new("v3", V3)
                                    .note("Rain plan")
                                    .time("Just now")
                                    .preview(|_, cx| {
                                        plan_preview(
                                            "Picnic plan",
                                            &[
                                                "Saturday, noon, on the north shore",
                                                "Blankets, a kite, sandwiches, lemonade",
                                                "If it rains, the boathouse",
                                            ],
                                            cx,
                                        )
                                    }),
                            )
                            .on_copy(move |ix, _, _, cx| {
                                on_copy(format!("Copied version {}", ix + 1), cx)
                            })
                            .on_download(move |request, _, cx| {
                                on_download(format!("Download {}", request.file_name), cx)
                            })
                            .on_version_change(move |ix, _, cx| {
                                on_version(format!("Showing version {}", ix + 1), cx)
                            })
                            .on_tab_change(move |tab, _, cx| {
                                on_tab(format!("{tab:?} tab"), cx)
                            })
                            .on_close(|_, _| {}),
                    )
                    .child(
                        ArtifactPanel::new("styles", "Button styles", "buttons.css")
                            .kind("Stylesheet")
                            .w(px(360.))
                            .min_height(px(200.))
                            .tab(ArtifactTab::Source)
                            .selected_version(0)
                            .version(
                                ArtifactVersion::new(
                                    "v1",
                                    ".button {\n  height: 28px;\n  padding: 0 10px;\n  border-radius: 6px;\n}\n",
                                )
                                .note("Sizes")
                                .time("Yesterday"),
                            )
                            .version(
                                ArtifactVersion::new(
                                    "v2",
                                    ".button {\n  height: 28px;\n  padding: 0 10px;\n  border-radius: 6px;\n  font-weight: 500;\n}\n\n.button:hover {\n  opacity: 0.9;\n}\n",
                                )
                                .note("Hover")
                                .time("Today"),
                            ),
                    ),
            )
            .font_family(cx.theme().font_family.clone())
    }
}
