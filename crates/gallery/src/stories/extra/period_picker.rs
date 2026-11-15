//! Gallery story for the period picker.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::period_picker::{PeriodPicker, Ymd};

const TODAY: Ymd = (2026, 4, 14);

/// Picking the period for a profit and loss report. The story keeps the
/// span, the pointer and the months shown.
pub struct PeriodPickerStory {
    start: Option<Ymd>,
    end: Option<Ymd>,
    hover: Option<Ymd>,
    first: (i32, u32),
}

impl PeriodPickerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            start: Some((2026, 3, 1)),
            end: Some((2026, 3, 31)),
            hover: None,
            first: (2026, 3),
        })
    }
}

impl Story for PeriodPickerStory {
    fn title() -> &'static str {
        "Period picker"
    }

    fn description() -> &'static str {
        "Pick a span of days on two months side by side, or take one of the reporting periods people use most."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for PeriodPickerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (span, hover, month) = (cx.entity(), cx.entity(), cx.entity());
        PeriodPicker::new("pnl-period", TODAY)
            .first_month(self.first.0, self.first.1)
            .span(self.start, self.end)
            .hover(self.hover)
            .on_span(move |a, b, _, cx| {
                span.update(cx, |this, cx| {
                    this.start = a;
                    this.end = b;
                    if let Some((y, m, _)) = a.filter(|_| b.is_some()) {
                        this.first = (y, m);
                    }
                    cx.notify();
                })
            })
            .on_hover(move |day, _, cx| {
                hover.update(cx, |this, cx| {
                    if this.end.is_none() {
                        this.hover = day;
                        cx.notify();
                    }
                })
            })
            .on_month(move |n, _, cx| {
                month.update(cx, |this, cx| {
                    let total = this.first.0 * 12 + this.first.1 as i32 - 1 + n;
                    this.first = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
                    cx.notify();
                })
            })
    }
}
