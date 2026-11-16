//! Every macro here must expand without this crate naming GPUI or
//! `isonomic-ui`: there is no `use … as gpui;` anywhere in the file.

use isonomic_app::ui::{
    App, Context, IntoElement, ParentElement as _, Render, RenderOnce, Styled as _, Window, div,
};

/// A derived element.
#[derive(IntoElement)]
pub struct Badge {
    label: &'static str,
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().px_2().child(self.label)
    }
}

/// A view.
pub struct Screen;

impl Render for Screen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Badge { label: "Ready" })
    }
}

isonomic_app::ui::actions!(app_facade, [Refresh]);

#[cfg(test)]
mod tests {
    use super::*;
    use isonomic_app::ui::{AppContext as _, TestAppContext};

    #[isonomic_app::ui::test]
    fn the_view_draws(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Screen);
        cx.run_until_parked();
        window
            .update(cx, |_, window, cx| {
                window.dispatch_action(Box::new(Refresh), cx);
            })
            .unwrap();
        let _ = cx.new(|_| Screen);
    }
}
