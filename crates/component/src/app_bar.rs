use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};
use isonomic_ui_base::{LogicalStyledExt as _, TestSupportExt as _};
use rust_i18n::t;
use smallvec::SmallVec;

use crate::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable, Size, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    toolbar::Toolbar,
    v_flex,
};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A command in an [`AppBar`]: a button while the bar shows it, and an item
/// in the bar's "More actions" menu once it overflows.
///
/// An action with an icon shows as an icon button, named and tooltipped by
/// its label; one without shows its label.
#[derive(Clone)]
pub struct AppBarAction {
    id: ElementId,
    label: SharedString,
    icon: Option<Icon>,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl AppBarAction {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            disabled: false,
            on_click: None,
        }
    }

    /// Show the action as an icon button. The label still names it, as its
    /// tooltip and to assistive technology, and in the overflow menu.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn into_button(self) -> Button {
        let label = self.label;
        let button = Button::new(self.id).disabled(self.disabled);
        let button = match self.icon {
            Some(icon) => button
                .icon(icon)
                .tooltip(label.clone())
                .accessibility_label(label),
            None => button.label(label),
        };
        match self.on_click {
            Some(handler) => button.on_click(move |event, window, cx| handler(event, window, cx)),
            None => button,
        }
    }

    fn into_menu_item(self) -> PopupMenuItem {
        let item = PopupMenuItem::new(self.label)
            .disabled(self.disabled)
            .when_some(self.icon, |item, icon| item.icon(icon));
        match self.on_click {
            Some(handler) => item.on_click(move |event, window, cx| handler(event, window, cx)),
            None => item,
        }
    }
}

/// The header of an application window or page: navigation at the start, a
/// title, optional content such as a search field or tabs, and commands at
/// the end.
///
/// ```text
/// [nav] Title            [content ··········]   [action] [action] [⋯]  [trailing]
///       Subtitle
/// ```
///
/// Commands added with [`AppBar::action`] sit in a [`Toolbar`], so the arrow
/// keys move between them. Past [`AppBar::max_visible_actions`], the rest
/// collapse into a "More actions" menu, in order. Content added with `child`
/// takes the space between the title and the commands; the title truncates
/// before the commands are squeezed. `trailing` content, such as an account
/// avatar, ends the bar.
///
/// The bar is exposed to assistive technology as the window's banner, and the
/// title as a heading.
///
/// ```
/// # mod isonomic_ui { pub extern crate isonomic_ui_component as component; }
/// use isonomic_ui::component::{IconName, app_bar::{AppBar, AppBarAction}};
///
/// let _ = AppBar::new("app-bar")
///     .title("Projects")
///     .action(AppBarAction::new("search", "Search").icon(IconName::Search))
///     .action(AppBarAction::new("new", "New project"));
/// ```
#[derive(IntoElement)]
pub struct AppBar {
    id: ElementId,
    style: StyleRefinement,
    size: Size,
    navigation: Option<AnyElement>,
    title: Option<SharedString>,
    subtitle: Option<SharedString>,
    children: SmallVec<[AnyElement; 1]>,
    actions: Vec<AppBarAction>,
    max_visible_actions: Option<usize>,
    trailing: SmallVec<[AnyElement; 1]>,
    bordered: bool,
}

impl AppBar {
    /// Create an empty bar. The id keeps its commands' keyboard state stable
    /// across frames; give each bar in a window a distinct id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            size: Size::default(),
            navigation: None,
            title: None,
            subtitle: None,
            children: SmallVec::new(),
            actions: Vec::new(),
            max_visible_actions: None,
            trailing: SmallVec::new(),
            bordered: true,
        }
    }

    /// The control before the title, such as a menu or back button. Give an
    /// icon-only button an accessible name.
    pub fn navigation(mut self, navigation: impl IntoElement) -> Self {
        self.navigation = Some(navigation.into_any_element());
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A second line under the title, such as a count or a location.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Append a command.
    pub fn action(mut self, action: AppBarAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Append commands.
    pub fn actions(mut self, actions: impl IntoIterator<Item = AppBarAction>) -> Self {
        self.actions.extend(actions);
        self
    }

    /// Show at most `count` commands as buttons and move the rest into a
    /// "More actions" menu. Default: all of them.
    pub fn max_visible_actions(mut self, count: usize) -> Self {
        self.max_visible_actions = Some(count);
        self
    }

    /// Append content after the commands, such as an account avatar.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing.push(trailing.into_any_element());
        self
    }

    /// Draw a border under the bar, default: `true`.
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Split the commands into those shown as buttons and those in the menu.
    fn split_actions(&mut self) -> (Vec<AppBarAction>, Vec<AppBarAction>) {
        let mut visible = std::mem::take(&mut self.actions);
        let shown = self.max_visible_actions.unwrap_or(visible.len());
        let overflow = visible.split_off(shown.min(visible.len()));
        (visible, overflow)
    }
}

/// `child` / `children` fill the space between the title and the commands.
impl ParentElement for AppBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for AppBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// [`Size::XSmall`] reads as [`Size::Small`]: a bar shorter than that leaves
/// no room for a title and subtitle.
impl Sizable for AppBar {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = match size.into() {
            Size::XSmall => Size::Small,
            size => size,
        };
        self
    }
}

impl RenderOnce for AppBar {
    fn render(mut self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size;
        let (visible, overflow) = self.split_actions();
        // Commands are one step smaller than the bar, as in a toolbar.
        let command_size = match size {
            Size::Large => Size::Medium,
            _ => Size::Small,
        };
        let more = (!overflow.is_empty()).then(|| {
            let label = SharedString::from(t!("AppBar.more"));
            let overflow = Rc::new(overflow);
            Button::new("more-actions")
                .ghost()
                .compact()
                .with_size(command_size)
                .icon(IconName::Ellipsis)
                .tooltip(label.clone())
                .accessibility_label(label)
                .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                    for action in overflow.iter().cloned() {
                        menu = menu.item(action.into_menu_item());
                    }
                    menu
                })
        });
        let has_commands = !visible.is_empty() || more.is_some();
        let title = self.title.map(|title| {
            v_flex()
                .min_w_0()
                .child(
                    div()
                        .id("title")
                        .test_support()
                        .role(Role::Heading)
                        .aria_level(1)
                        .aria_label(title.clone())
                        .truncate()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .map(|this| match size {
                            Size::Small => this.text_sm(),
                            Size::Large => this.text_lg(),
                            _ => this.text_base(),
                        })
                        .child(title),
                )
                .when_some(self.subtitle, |this, subtitle| {
                    this.child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(subtitle),
                    )
                })
        });

        h_flex()
            .id(self.id.clone())
            .test_support()
            .role(Role::Banner)
            .flex_row_dir(cx)
            .w_full()
            .flex_shrink_0()
            .items_center()
            .bg(cx.theme().background)
            .when(self.bordered, |this| {
                this.border_b_1().border_color(cx.theme().border)
            })
            .map(|this| match size {
                Size::Small => this.h_10().px_2().gap_2(),
                Size::Large => this.h_16().px_4().gap_3(),
                _ => this.h_12().px_3().gap_2(),
            })
            .refine_style(&self.style)
            .when_some(self.navigation, |this, navigation| {
                this.child(div().flex_none().child(navigation))
            })
            .children(title)
            // The content takes the free space, so the commands stay at the end even
            // without any.
            .child(
                h_flex()
                    .flex_row_dir(cx)
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .children(self.children),
            )
            .when(has_commands, |this| {
                this.child(
                    Toolbar::new((self.id.clone(), "actions"))
                        .with_size(command_size)
                        .flex_none()
                        .p_0()
                        .children(visible.into_iter().map(AppBarAction::into_button))
                        .when_some(more, |this, more| this.content(more)),
                )
            })
            .children(self.trailing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(count: usize) -> AppBar {
        AppBar::new("bar")
            .actions((0..count).map(|ix| AppBarAction::new(("action", ix), format!("Action {ix}"))))
    }

    #[test]
    fn every_action_shows_by_default() {
        let (visible, overflow) = bar(3).split_actions();
        assert_eq!(visible.len(), 3);
        assert!(overflow.is_empty());
    }

    #[test]
    fn actions_past_the_limit_overflow_in_order() {
        let (visible, overflow) = bar(4).max_visible_actions(1).split_actions();
        let labels = |actions: &[AppBarAction]| {
            actions
                .iter()
                .map(|action| action.label.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(&visible), ["Action 0"]);
        assert_eq!(labels(&overflow), ["Action 1", "Action 2", "Action 3"]);
        // A limit above the count shows everything.
        assert_eq!(bar(2).max_visible_actions(5).split_actions().1.len(), 0);
        // Zero moves every command into the menu.
        assert_eq!(bar(2).max_visible_actions(0).split_actions().0.len(), 0);
    }

    #[test]
    fn extra_small_reads_as_small() {
        assert_eq!(AppBar::new("bar").xsmall().size, Size::Small);
        assert_eq!(AppBar::new("bar").large().size, Size::Large);
    }

    #[cfg(feature = "test-support")]
    mod rendered {
        use super::*;
        use gpui::{Context, Render, px};

        struct Harness {
            archived: Rc<std::cell::Cell<bool>>,
        }

        impl Render for Harness {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let archived = self.archived.clone();
                div().w(px(360.)).child(
                    AppBar::new("bar")
                        .navigation(
                            Button::new("menu")
                                .icon(IconName::Menu)
                                .accessibility_label("Menu"),
                        )
                        .title("A title long enough that it cannot fit beside its commands")
                        .subtitle("12 open")
                        .action(AppBarAction::new("search", "Search").icon(IconName::Search))
                        .action(AppBarAction::new("share", "Share"))
                        .action(
                            AppBarAction::new("archive", "Archive")
                                .on_click(move |_, _, _| archived.set(true)),
                        )
                        .max_visible_actions(2),
                )
            }
        }

        #[gpui::test]
        fn a_narrow_bar_truncates_the_title_and_keeps_its_commands(cx: &mut gpui::TestAppContext) {
            cx.update(crate::init);
            let archived = Rc::new(std::cell::Cell::new(false));
            let (_, cx) = cx.add_window_view({
                let archived = archived.clone();
                move |_, _| Harness { archived }
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            cx.update(|window, _| {
                let find = |id: &'static str| {
                    isonomic_ui_base::test_support::find(window, &[], &ElementId::from(id))
                        .unwrap_or_else(|| panic!("{id} is painted"))
                };
                let bar = find("bar");
                assert_eq!(bar.role(), Some(Role::Banner));
                let title = find("title");
                assert_eq!(title.role(), Some(Role::Heading));
                assert!(title.bounds().right() <= find("search").bounds().left());

                // Two commands show, the third is in the menu, and all fit the bar.
                assert!(find("share").bounds().right() <= find("more-actions").bounds().left());
                assert!(
                    isonomic_ui_base::test_support::find(window, &[], &"archive".into()).is_none()
                );
                assert_eq!(find("search").label(), Some("Search"));
                assert_eq!(find("more-actions").label(), Some("More actions"));
                assert!(find("more-actions").bounds().right() <= bar.bounds().right());
            });

            // The menu lists the overflowed command, which runs its handler.
            let more = cx.update(|window, _| {
                isonomic_ui_base::test_support::find(window, &[], &"more-actions".into())
                    .unwrap()
                    .bounds()
                    .center()
            });
            cx.simulate_click(more, gpui::Modifiers::default());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let archive = cx.update(|window, _| {
                isonomic_ui_base::test_support::snapshots(window)
                    .into_iter()
                    .find(|item| {
                        item.role() == Some(Role::MenuItem) && item.label() == Some("Archive")
                    })
                    .expect("Archive is in the menu")
                    .bounds()
                    .center()
            });
            cx.simulate_click(archive, gpui::Modifiers::default());
            assert!(archived.get());
        }
    }
}
