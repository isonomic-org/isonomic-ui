use isonomic_ui::{
    App, AppContext, Context, Entity, Focusable, IntoElement, ParentElement, Render, Styled,
    Window, px,
};

use isonomic_ui::component::{ActiveTheme as _, text::TextView, v_flex};

use crate::section;

/// What an assistant's reply can hold: every block a chat has to draw.
const REPLY: &str = r#"# Reconciling the March statement

The statement and the ledger differ by **$412.60**. Three things explain it, and
one of them needs *your* decision.

## What I found

1. A duplicate payment to the landlord on 3 March.
2. Two card refunds that reached the bank but not the ledger:
   - `REF-2291` for $88.10
   - `REF-2304` for $124.50
3. A transfer that is still pending, so the bank has not counted it yet.

> The duplicate is the only one that changes what you owe. The rest is timing.

## The numbers

| Item | Bank | Ledger | Difference |
|---|---:|---:|---:|
| Rent | $2,400.00 | $2,200.00 | $200.00 |
| Refunds | $212.60 | $0.00 | $212.60 |
| Transfer | $0.00 | $0.00 | $0.00 |

## How to check it yourself

```sql
select posted_on, payee, amount
from payments
where payee = 'Northside Lettings'
  and posted_on between '2026-03-01' and '2026-03-31'
order by posted_on;
```

Run the query, then compare it with the [bank export](https://example.com/export).
A line with `amount = 200.00` twice on the same day is the duplicate.

---

### Next steps

- [x] Match the refunds
- [ ] Ask the landlord to return the duplicate
- [ ] Recheck once the transfer clears

Inline code such as `ledger.reconcile(month)` stays on the baseline, and a long
line of prose wraps inside the column instead of running past its edge, however
narrow the window gets.
"#;

/// Lists on their own, for the list styling options.
const LISTS: &str = r#"1. Match the refunds
2. Ask the landlord to return the duplicate
   - by email first
   - then by phone
3. Recheck once the transfer clears

   Its second paragraph lines up with the text above.

- A loose item

- Another loose item
"#;

pub struct MarkdownStory {
    focus_handle: isonomic_ui::FocusHandle,
}

impl super::Story for MarkdownStory {
    fn title() -> &'static str {
        "Markdown"
    }

    fn description() -> &'static str {
        "A reply drawn from Markdown: headings, lists, a quote, a table, code and links"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl MarkdownStory {
    pub(crate) fn new(_: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Focusable for MarkdownStory {
    fn focus_handle(&self, _: &isonomic_ui::App) -> isonomic_ui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MarkdownStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let marker = cx.theme().info;
        v_flex()
            .w_full()
            .items_center()
            .gap_6()
            .child(
                section("A reply")
                    .description("Every block a chat reply can hold, in a reading column.")
                    .w_full()
                    .max_w(px(720.))
                    // The section centres its content; the column takes the full
                    // width so the text wraps inside it.
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .child(TextView::markdown("markdown-reply", REPLY).selectable(true)),
                    ),
            )
            .child(
                section("List styling")
                    .description(
                        "Numbered text 28 in, bulleted text 21 in, more room after the marker, and coloured markers.",
                    )
                    .w_full()
                    .max_w(px(720.))
                    .child(
                        v_flex().w_full().min_w_0().child(
                            TextView::markdown("markdown-lists", LISTS)
                                .list_indent(px(28.))
                                .list_marker_gap(px(12.))
                                .bullet_list_indent(px(21.))
                                .list_marker_color(marker),
                        ),
                    ),
            )
    }
}
