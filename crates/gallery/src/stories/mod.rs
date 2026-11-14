use isonomic_ui::component::dock::PanelControl;
use isonomic_ui::{AnyView, App, AppContext as _, Entity, Hsla, Pixels, Render, Window, px};

mod alert_dialog_story;
mod alert_story;
mod attachment_story;
mod avatar_story;
mod welcome_story;

pub use alert_dialog_story::AlertDialogStory;
pub use alert_story::AlertStory;
pub use attachment_story::AttachmentStory;
pub use avatar_story::AvatarStory;

pub use welcome_story::WelcomeStory;

pub(crate) fn init(cx: &mut App) {}

pub trait Story: Render + Sized {
    fn klass() -> &'static str {
        std::any::type_name::<Self>().split("::").last().unwrap()
    }

    fn title() -> &'static str;

    fn description() -> &'static str {
        ""
    }

    fn closable() -> bool {
        true
    }

    fn zoomable() -> Option<PanelControl> {
        Some(PanelControl::default())
    }

    fn title_bg() -> Option<Hsla> {
        None
    }

    fn paddings() -> Pixels {
        px(16.)
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render>;

    fn on_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
        let _ = active;
        let _ = window;
        let _ = cx;
    }

    fn on_active_any(view: AnyView, active: bool, window: &mut Window, cx: &mut App)
    where
        Self: 'static,
    {
        if let Some(story) = view.downcast::<Self>().ok() {
            cx.update_entity(&story, |story, cx| {
                story.on_active(active, window, cx);
            });
        }
    }
}
