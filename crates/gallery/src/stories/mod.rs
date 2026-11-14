use isonomic_ui::component::dock::PanelControl;
use isonomic_ui::{AnyView, App, AppContext as _, Entity, Hsla, Pixels, Render, Window, px};

mod alert_dialog_story;
mod alert_story;
mod attachment_story;
mod avatar_story;
mod badge_story;
mod breadcrumb_story;
mod button_story;
mod calendar_story;
mod carousel_story;
mod chart_story;
mod checkbox_story;
mod clipboard_story;
mod combobox_story;
mod command_story;
mod description_list_story;
mod diff_story;
mod dock_story;
mod dropdown_button_story;
mod editor_story;
mod empty_story;
mod hover_card_story;
mod icon_story;
mod image_story;
mod input_completion;
mod input_story;
mod input_tokens;
mod kbd_story;
mod label_story;
mod list_story;
mod marker_story;
mod menu_story;
mod message_scroller_story;
mod message_story;
mod native_menu_story;
mod welcome_story;

pub use alert_dialog_story::AlertDialogStory;
pub use alert_story::AlertStory;
pub use attachment_story::AttachmentStory;
pub use avatar_story::AvatarStory;
pub use badge_story::BadgeStory;
pub use breadcrumb_story::BreadcrumbStory;
pub use button_story::ButtonStory;
pub use calendar_story::CalendarStory;
pub use carousel_story::CarouselStory;
pub use chart_story::ChartStory;
pub use checkbox_story::CheckboxStory;
pub use clipboard_story::ClipboardStory;
pub use combobox_story::ComboboxStory;
pub use command_story::CommandStory;
pub use description_list_story::DescriptionListStory;
pub use diff_story::DiffStory;
pub use dock_story::DockStory;
pub use dropdown_button_story::DropdownButtonStory;
pub use editor_story::EditorStory;
pub use empty_story::EmptyStory;
pub use hover_card_story::HoverCardStory;
pub use icon_story::IconStory;
pub use image_story::ImageStory;
pub use input_story::InputStory;
pub use kbd_story::KbdStory;
pub use label_story::LabelStory;
pub use list_story::ListStory;
pub use marker_story::MarkerStory;
pub use menu_story::MenuStory;
pub use message_scroller_story::MessageScrollerStory;
pub use message_story::MessageStory;
pub use native_menu_story::NativeMenuStory;

pub use welcome_story::WelcomeStory;

pub(crate) fn init(cx: &mut App) {
    input_story::init(cx);
    combobox_story::init(cx);
    menu_story::init(cx);
}

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
