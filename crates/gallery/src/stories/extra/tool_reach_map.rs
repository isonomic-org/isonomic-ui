//! Gallery story for the tool reach map.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::tool_reach_map::{ReachStanding, ReachTool, ToolCall, ToolReachMap};

/// What a reconciler can reach during a close. The story keeps the
/// filter and selection and notes replays.
pub struct ToolReachMapStory {
    filter: Option<ReachStanding>,
    selected: Option<SharedString>,
    replayed: Option<String>,
}

fn tools() -> Vec<ReachTool> {
    vec![
        ReachTool::new("l-list", "Ledger", "list_unmatched").calls(vec![
            ToolCall::ok("c1", "1:03 pm", "account=cards \u{2192} 214 lines"),
            ToolCall::ok("c2", "1:41 pm", "account=operating \u{2192} 61 lines"),
        ]),
        ReachTool::new("l-post", "Ledger", "post_matches").calls(vec![
            ToolCall::ok("c3", "1:44 pm", "61 matches \u{2192} posted"),
            ToolCall::failed("c4", "1:55 pm", "201 matches \u{2192} period locked"),
        ]),
        ReachTool::new("l-unlock", "Ledger", "unlock_period").blocked(),
        ReachTool::new("l-journal", "Ledger", "create_journal"),
        ReachTool::new("b-stmt", "Bank feed", "statement").calls(vec![ToolCall::ok(
            "c5",
            "1:39 pm",
            "operating, March \u{2192} 61 lines",
        )]),
        ReachTool::new("b-pay", "Bank feed", "send_payment").blocked(),
        ReachTool::new("m-draft", "Mail", "draft").calls(vec![ToolCall::ok(
            "c6",
            "1:57 pm",
            "to Maya \u{2192} draft saved",
        )]),
        ReachTool::new("m-send", "Mail", "send"),
        ReachTool::new("f-read", "Files", "read").calls(vec![ToolCall::ok(
            "c7",
            "1:02 pm",
            "matching-rules.toml",
        )]),
        ReachTool::new("f-write", "Files", "write").calls(vec![ToolCall::failed(
            "c8",
            "1:58 pm",
            "close-note.md \u{2192} file is open elsewhere",
        )]),
    ]
}

impl ToolReachMapStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            filter: None,
            selected: Some("l-post".into()),
            replayed: None,
        })
    }
}

impl Story for ToolReachMapStory {
    fn title() -> &'static str {
        "Tool reach map"
    }

    fn description() -> &'static str {
        "Every tool an assistant can reach, grouped by service, with what it used, what failed and each call ready to replay."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ToolReachMapStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let filter = cx.entity();
        let replay = cx.entity();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                ToolReachMap::new("reconciler-reach", "the Reconciler", tools())
                    .filter(self.filter)
                    .selected(self.selected.clone())
                    .on_select(move |id, _, cx| {
                        select.update(cx, |this, cx| {
                            this.selected = id.cloned();
                            cx.notify();
                        })
                    })
                    .on_filter(move |f, _, cx| {
                        filter.update(cx, |this, cx| {
                            this.filter = f;
                            cx.notify();
                        })
                    })
                    .on_replay(move |tool, call, _, cx| {
                        replay.update(cx, |this, cx| {
                            this.replayed = Some(format!("Replaying {call} on {tool}"));
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        self.replayed
                            .clone()
                            .unwrap_or_else(|| "Pick a tool to see its calls.".into()),
                    ),
            )
    }
}
