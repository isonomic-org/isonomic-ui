//! Gallery story for the prompt template library.

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use gpui_component::{ActiveTheme as _, v_flex};
use isonomic_ui_extra::prompt_template_library::{
    PromptLibraryEvent, PromptTemplate, PromptTemplateLibrary, PromptVariable,
};

/// Four made-up bookkeeping prompts, one of them filled in already.
pub struct PromptTemplateLibraryStory {
    library: Entity<PromptTemplateLibrary>,
    last: SharedString,
    _events: Subscription,
}

impl PromptTemplateLibraryStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let library = cx.new(|cx| {
                PromptTemplateLibrary::new(window, cx)
                    .template(
                        PromptTemplate::new(
                            "chase",
                            "Chase a late invoice",
                            "Write a {tone} reminder to {client} about invoice {number}, now {days} days late. Offer a short call if anything is unclear.",
                        )
                        .category("Billing")
                        .variable(PromptVariable::new("tone").description("tone").default("polite"))
                        .variable(PromptVariable::new("client").description("client"))
                        .variable(PromptVariable::new("number").description("invoice no."))
                        .variable(PromptVariable::new("days").description("days")),
                    )
                    .template(
                        PromptTemplate::new(
                            "variance",
                            "Explain a variance",
                            "Explain why {account} moved by {amount} between {from} and {to}, in two short paragraphs for {reader}.",
                        )
                        .category("Reports")
                        .variable(PromptVariable::new("account").default("Software subscriptions"))
                        .variable(PromptVariable::new("amount").default("+18%"))
                        .variable(PromptVariable::new("from").default("August"))
                        .variable(PromptVariable::new("to").default("September"))
                        .variable(PromptVariable::new("reader").default("the founders")),
                    )
                    .template(
                        PromptTemplate::new(
                            "close",
                            "Sum up the month end",
                            "Sum up the {month} close for {reader}: what reconciled, what is still open and what needs a decision.",
                        )
                        .category("Month end")
                        .variable(PromptVariable::new("month").description("month"))
                        .variable(PromptVariable::new("reader").description("who reads it")),
                    )
                    .template(
                        PromptTemplate::new(
                            "categorise",
                            "Sort bank lines",
                            "Suggest a category for each bank line below using the chart of accounts for {company}. Flag anything above {limit}.",
                        )
                        .category("Ledger")
                        .variable(PromptVariable::new("company").description("company"))
                        .variable(PromptVariable::new("limit").description("amount")),
                    )
                    .chosen("variance")
            });
            let events = cx.subscribe(&library, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    PromptLibraryEvent::Selected(id) => format!("Chose {id}"),
                    PromptLibraryEvent::Copied(prompt) => {
                        format!("Copied {} characters", prompt.chars().count())
                    }
                    PromptLibraryEvent::Used { id, values, .. } => {
                        format!("Used {id} with {} blanks filled", values.len())
                    }
                }
                .into();
                cx.notify();
            });
            Self {
                library,
                last: "Nothing yet".into(),
                _events: events,
            }
        })
    }
}

impl Render for PromptTemplateLibraryStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.library.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
