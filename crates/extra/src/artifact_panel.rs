// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Artifact panel: a generated piece of work shown beside a conversation.
//!
//! The header names the artifact and carries copy, download and close. Under
//! it, Preview and Source tabs switch between the rendered result and its
//! text, and a version picker steps back and forth through earlier versions
//! or jumps to one from a list. An older version says so above the content,
//! with a way back to the latest. Copying puts the shown version's source on
//! the clipboard; copy, download and close are reported as events.
//!
//! Keys: the tabs and buttons are ordinary tab stops; `[` and `]` step to the
//! previous and next version from anywhere inside the panel.

use std::rc::Rc;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    TextRole as _,
    button::{Button, ButtonVariants as _},
    clipboard::Clipboard,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    tab::{Tab, TabBar},
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, px,
};

use crate::common::{inset, panel, shell, shell_header};

type Preview = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;
type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type TabHandler = Rc<dyn Fn(ArtifactTab, &mut Window, &mut App)>;
type CopyHandler = Rc<dyn Fn(usize, SharedString, &mut Window, &mut App)>;
type DownloadHandler = Rc<dyn Fn(&ArtifactDownload, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Which face of the artifact is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArtifactTab {
    /// The rendered result.
    #[default]
    Preview,
    /// Its text.
    Source,
}

/// One saved version of the artifact.
#[derive(Clone)]
pub struct ArtifactVersion {
    label: SharedString,
    note: Option<SharedString>,
    time: Option<SharedString>,
    source: SharedString,
    preview: Option<Preview>,
}

impl ArtifactVersion {
    /// A version with a short label, such as "v3", and its source text.
    pub fn new(label: impl Into<SharedString>, source: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            note: None,
            time: None,
            source: source.into(),
            preview: None,
        }
    }

    /// What changed in this version, in a few words.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// When it was made, as the host writes it, such as "4 min ago".
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// How the Preview tab draws this version.
    pub fn preview(
        mut self,
        preview: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.preview = Some(Rc::new(preview));
        self
    }
}

/// What a download asks for: the version and the name to save it under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDownload {
    /// Which version, counted from the oldest.
    pub version: usize,
    /// The file name, marked with the version when it is not the latest.
    pub file_name: SharedString,
    /// The text to save.
    pub source: SharedString,
}

/// Step `delta` versions from `current` among `len`, stopping at the ends.
pub fn step_version(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(len - 1)
}

/// Where a version sits among all of them: "3 of 5".
pub fn position_text(version: usize, len: usize) -> String {
    format!("{} of {len}", version + 1)
}

/// The name to save a version under. The latest keeps `file_name`; an older
/// one gets its label before the extension: "plan.md" becomes "plan-v2.md".
pub fn versioned_file_name(file_name: &str, label: &str, latest: bool) -> String {
    if latest || label.is_empty() {
        return file_name.to_string();
    }
    let tag: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    match file_name.rfind('.').filter(|at| *at > 0) {
        Some(at) => format!("{}-{tag}{}", &file_name[..at], &file_name[at..]),
        None => format!("{file_name}-{tag}"),
    }
}

/// The source split into lines, keeping a trailing empty one out.
pub fn source_lines(source: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = source.split('\n').collect();
    if lines.len() > 1 && lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// How many digits the widest line number needs.
pub fn gutter_digits(lines: usize) -> usize {
    lines.max(1).to_string().len()
}

/// The menu line for a version: "v2 · Added totals · 4 min ago".
pub fn version_line(version: &ArtifactVersion) -> String {
    let mut parts = vec![version.label.to_string()];
    parts.extend(version.note.iter().map(|note| note.to_string()));
    parts.extend(version.time.iter().map(|time| time.to_string()));
    parts.join(" · ")
}

/// The version the caller gave last, and the one on screen.
#[derive(Clone, Copy)]
struct Picked<T: Copy> {
    given: T,
    shown: T,
}

/// A side panel for a generated artifact, with its versions.
#[derive(IntoElement)]
pub struct ArtifactPanel {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    kind: Option<SharedString>,
    file_name: SharedString,
    versions: Vec<ArtifactVersion>,
    version: Option<usize>,
    tab: ArtifactTab,
    min_height: Pixels,
    on_version: Option<IndexHandler>,
    on_tab: Option<TabHandler>,
    on_copy: Option<CopyHandler>,
    on_download: Option<DownloadHandler>,
    on_close: Option<CloseHandler>,
}

impl ArtifactPanel {
    /// A panel for an artifact saved under `file_name`.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        file_name: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            kind: None,
            file_name: file_name.into(),
            versions: Vec::new(),
            version: None,
            tab: ArtifactTab::Preview,
            min_height: px(320.),
            on_version: None,
            on_tab: None,
            on_copy: None,
            on_download: None,
            on_close: None,
        }
    }

    /// What sort of thing it is, such as "Page" or "Document".
    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// Add a version. Versions go oldest first.
    pub fn version(mut self, version: ArtifactVersion) -> Self {
        self.versions.push(version);
        self
    }

    /// Show the version at `index`, counted from the oldest. The latest by
    /// default. Changing it later moves the panel to it.
    pub fn selected_version(mut self, index: usize) -> Self {
        self.version = Some(index);
        self
    }

    /// The tab to show. Preview by default.
    pub fn tab(mut self, tab: ArtifactTab) -> Self {
        self.tab = tab;
        self
    }

    /// The least height of the content area. 320px by default.
    pub fn min_height(mut self, height: impl Into<Pixels>) -> Self {
        self.min_height = height.into();
        self
    }

    /// Run when the reader moves to another version.
    pub fn on_version_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_version = Some(Rc::new(handler));
        self
    }

    /// Run when the reader switches between Preview and Source.
    pub fn on_tab_change(
        mut self,
        handler: impl Fn(ArtifactTab, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_tab = Some(Rc::new(handler));
        self
    }

    /// Run after the shown version's source is copied, with its index.
    pub fn on_copy(
        mut self,
        handler: impl Fn(usize, SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }

    /// Show a Download button that asks the host to save the shown version.
    pub fn on_download(
        mut self,
        handler: impl Fn(&ArtifactDownload, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_download = Some(Rc::new(handler));
        self
    }

    /// Show a Close button.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl Styled for ArtifactPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ArtifactPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: &str| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let info = theme.info;
        let count = self.versions.len();
        let last = count.saturating_sub(1);

        let given = self.version.unwrap_or(last).min(last);
        let version_state = window.use_keyed_state(part("version"), cx, |_, _| Picked {
            given,
            shown: given,
        });
        if version_state.read(cx).given != given {
            version_state.update(cx, |state, _| {
                *state = Picked {
                    given,
                    shown: given,
                }
            });
        }
        let shown = version_state.read(cx).shown.min(last);

        let tab_state = window.use_keyed_state(part("tab"), cx, |_, _| Picked {
            given: self.tab,
            shown: self.tab,
        });
        if tab_state.read(cx).given != self.tab {
            let tab = self.tab;
            tab_state.update(cx, |state, _| {
                *state = Picked {
                    given: tab,
                    shown: tab,
                }
            });
        }
        let tab = tab_state.read(cx).shown;

        // Moves to a version and tells the host.
        let go: Rc<dyn Fn(usize, &mut Window, &mut App)> = {
            let state = version_state.clone();
            let handler = self.on_version.clone();
            Rc::new(move |to: usize, window: &mut Window, cx: &mut App| {
                if state.read(cx).shown == to {
                    return;
                }
                state.update(cx, |state, cx| {
                    state.shown = to;
                    cx.notify();
                });
                if let Some(handler) = &handler {
                    handler(to, window, cx);
                }
            })
        };

        let current = self.versions.get(shown).cloned();
        let source = current
            .as_ref()
            .map(|v| v.source.clone())
            .unwrap_or_default();
        let label = current
            .as_ref()
            .map(|v| v.label.clone())
            .unwrap_or_default();

        // Header actions: copy, download, close.
        let copy = (count > 0).then(|| {
            let handler = self.on_copy.clone();
            Clipboard::new(part("copy"))
                .value(source.clone())
                .tooltip("Copy source")
                .accessibility_label(format!("Copy source of {label}"))
                .on_copied(move |text, window, cx| {
                    if let Some(handler) = &handler {
                        handler(shown, text, window, cx);
                    }
                })
        });
        let download = self
            .on_download
            .clone()
            .filter(|_| count > 0)
            .map(|handler| {
                let request = ArtifactDownload {
                    version: shown,
                    file_name: versioned_file_name(&self.file_name, &label, shown == last).into(),
                    source: source.clone(),
                };
                Button::new(part("download"))
                    .ghost()
                    .small()
                    .icon(Icon::new(CatalogIcon::Download))
                    .tooltip("Download")
                    .accessibility_label(format!("Download {}", request.file_name))
                    .on_click(move |_, window, cx| handler(&request, window, cx))
            });
        let close = self.on_close.clone().map(|handler| {
            Button::new(part("close"))
                .ghost()
                .small()
                .icon(IconName::Close)
                .tooltip("Close")
                .accessibility_label(format!("Close {}", self.title))
                .on_click(move |_, window, cx| handler(window, cx))
        });
        let aside = h_flex()
            .gap_1()
            .children(copy)
            .children(download)
            .children(close);

        let subtitle = match &self.kind {
            Some(kind) => format!("{kind} · {}", self.file_name),
            None => self.file_name.to_string(),
        };

        // Preview and Source.
        let tabs = {
            let state = tab_state.clone();
            let handler = self.on_tab.clone();
            TabBar::new(part("tabs"))
                .segmented()
                .small()
                .selected_index(match tab {
                    ArtifactTab::Preview => 0,
                    ArtifactTab::Source => 1,
                })
                .child(
                    Tab::new()
                        .icon(Icon::new(CatalogIcon::View))
                        .label("Preview"),
                )
                .child(
                    Tab::new()
                        .icon(Icon::new(CatalogIcon::Code))
                        .label("Source"),
                )
                .on_click(move |ix: &usize, window, cx| {
                    let next = if *ix == 0 {
                        ArtifactTab::Preview
                    } else {
                        ArtifactTab::Source
                    };
                    state.update(cx, |state, cx| {
                        state.shown = next;
                        cx.notify();
                    });
                    if let Some(handler) = &handler {
                        handler(next, window, cx);
                    }
                })
        };

        // The version picker: back, a list, forward.
        let picker = (count > 0).then(|| {
            let back = go.clone();
            let forward = go.clone();
            let versions = self.versions.clone();
            let list_go = go.clone();
            h_flex()
                .gap_0p5()
                .child(
                    Button::new(part("older"))
                        .ghost()
                        .small()
                        .icon(IconName::ChevronLeft)
                        .disabled(shown == 0)
                        .tooltip("Older version")
                        .accessibility_label("Older version")
                        .on_click(move |_, window, cx| {
                            back(step_version(shown, -1, count), window, cx)
                        }),
                )
                .child(
                    Button::new(part("versions"))
                        .ghost()
                        .small()
                        .dropdown_caret(true)
                        .accessibility_label(format!(
                            "Version {label}, {}{}. Choose a version",
                            position_text(shown, count),
                            if shown == last { ", latest" } else { "" }
                        ))
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .child(Icon::new(CatalogIcon::Version).size(px(14.)))
                                .child(div().text_label_small().child(label.clone()))
                                .child(
                                    div()
                                        .text_caption()
                                        .font_family(mono.clone())
                                        .text_color(quiet)
                                        .child(position_text(shown, count)),
                                ),
                        )
                        .dropdown_menu(move |mut menu, _, _| {
                            menu = menu.min_w(px(240.)).label("Versions");
                            for (ix, version) in versions.iter().enumerate().rev() {
                                let go = list_go.clone();
                                let mut line = version_line(version);
                                if ix == versions.len() - 1 {
                                    line.push_str(" · latest");
                                }
                                menu = menu.item(
                                    PopupMenuItem::new(line)
                                        .checked(ix == shown)
                                        .on_click(move |_, window, cx| go(ix, window, cx)),
                                );
                            }
                            menu
                        }),
                )
                .child(
                    Button::new(part("newer"))
                        .ghost()
                        .small()
                        .icon(IconName::ChevronRight)
                        .disabled(shown >= last)
                        .tooltip("Newer version")
                        .accessibility_label("Newer version")
                        .on_click(move |_, window, cx| {
                            forward(step_version(shown, 1, count), window, cx)
                        }),
                )
        });

        // An older version says so, with the way back.
        let older_note = (count > 0 && shown < last).then(|| {
            let latest = go.clone();
            h_flex()
                .gap_2()
                .px_3()
                .py(px(6.))
                .rounded(px(10.))
                .bg(info.opacity(0.1))
                .text_caption()
                .child(
                    Icon::new(CatalogIcon::RecentlyViewed)
                        .size(px(14.))
                        .text_color(info),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(format!("Older version, {label}")),
                )
                .child(
                    Button::new(part("latest"))
                        .ghost()
                        .xsmall()
                        .label("Back to latest")
                        .on_click(move |_, window, cx| latest(last, window, cx)),
                )
        });

        let body: AnyElement = match (&current, tab) {
            (None, _) => empty(quiet, "Nothing has been made yet.").into_any_element(),
            (Some(version), ArtifactTab::Preview) => match &version.preview {
                Some(preview) => div()
                    .id(part("preview"))
                    .size_full()
                    .overflow_y_scroll()
                    .child(preview(window, cx))
                    .into_any_element(),
                None => empty(
                    quiet,
                    "This version has no preview. Its text is under Source.",
                )
                .into_any_element(),
            },
            (Some(version), ArtifactTab::Source) => {
                let lines = source_lines(&version.source);
                let digits = gutter_digits(lines.len());
                let numbers = v_flex()
                    .flex_none()
                    .pr_3()
                    .text_color(look_faint(cx))
                    .children((1..=lines.len()).map(move |n| {
                        div()
                            .flex()
                            .justify_end()
                            .w(px(8. * digits as f32))
                            .child(n.to_string())
                    }));
                let text = v_flex()
                    .flex_1()
                    .min_w_0()
                    .children(lines.iter().map(|line| {
                        div().whitespace_nowrap().child(if line.is_empty() {
                            SharedString::from(" ")
                        } else {
                            SharedString::from(line.to_string())
                        })
                    }));
                inset(cx)
                    .id(part("source"))
                    .size_full()
                    .p_3()
                    .overflow_y_scroll()
                    .role(Role::Group)
                    .aria_label(format!("Source of {label}, {} lines", lines.len()))
                    .font_family(mono.clone())
                    .text_size(px(12.))
                    .line_height(px(20.))
                    .child(h_flex().items_start().child(numbers).child(text))
                    .into_any_element()
            }
        };

        let footer = current.as_ref().map(|version| {
            h_flex()
                .gap_2()
                .text_caption()
                .text_color(quiet)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(version_line(version)),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(mono.clone())
                        .child(format!("{} lines", source_lines(&version.source).len())),
                )
        });

        let key_go = go.clone();
        shell(cx)
            .id(self.id.clone())
            .test_support()
            .flex()
            .flex_col()
            .role(Role::Region)
            .aria_label(format!(
                "{}, {}, {}",
                self.title,
                if count == 0 {
                    "no versions".to_string()
                } else {
                    format!("version {label}, {}", position_text(shown, count))
                },
                match tab {
                    ArtifactTab::Preview => "preview",
                    ArtifactTab::Source => "source",
                }
            ))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let delta = match event.keystroke.key.as_str() {
                    "[" => -1,
                    "]" => 1,
                    _ => return,
                };
                if count == 0 {
                    return;
                }
                key_go(step_version(shown, delta, count), window, cx);
                cx.stop_propagation();
            })
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Document)),
                self.title.clone(),
                Some(subtitle.into_any_element()),
                Some(aside.into_any_element()),
                cx,
            ))
            .child(
                panel(cx)
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .flex_wrap()
                            .gap_2()
                            .child(tabs)
                            .children(picker),
                    )
                    .children(older_note)
                    .child(
                        div()
                            .flex_1()
                            .min_h(self.min_height)
                            .flex()
                            .flex_col()
                            .child(body),
                    )
                    .children(footer),
            )
            .refine_style(&self.style)
    }
}

/// The faint ink line numbers are drawn in.
fn look_faint(cx: &App) -> isonomic_ui::Hsla {
    isonomic_ui::component::look::ink::faint(cx)
}

/// A quiet message where content would be.
fn empty(quiet: isonomic_ui::Hsla, text: &'static str) -> impl IntoElement {
    v_flex()
        .size_full()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_2()
        .text_caption()
        .text_color(quiet)
        .child(Icon::new(CatalogIcon::Document).size(px(20.)))
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_stops_at_the_oldest_and_the_latest() {
        assert_eq!(step_version(2, -1, 5), 1);
        assert_eq!(step_version(0, -1, 5), 0);
        assert_eq!(step_version(4, 1, 5), 4);
        assert_eq!(step_version(1, 1, 5), 2);
        assert_eq!(step_version(3, 1, 0), 0);
        assert_eq!(position_text(2, 5), "3 of 5");
    }

    #[test]
    fn older_versions_save_under_a_marked_name() {
        assert_eq!(versioned_file_name("plan.md", "v2", false), "plan-v2.md");
        assert_eq!(versioned_file_name("plan.md", "v5", true), "plan.md");
        assert_eq!(versioned_file_name("notes", "v1", false), "notes-v1");
        assert_eq!(versioned_file_name(".env", "v1", false), ".env-v1");
        assert_eq!(
            versioned_file_name("site.page.html", "draft 2", false),
            "site.page-draft-2.html"
        );
    }

    #[test]
    fn source_splits_into_numbered_lines() {
        assert_eq!(source_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(source_lines("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(source_lines(""), vec![""]);
        assert_eq!(gutter_digits(9), 1);
        assert_eq!(gutter_digits(10), 2);
        assert_eq!(gutter_digits(0), 1);
    }

    #[test]
    fn a_version_line_joins_what_it_has() {
        let full = ArtifactVersion::new("v2", "x")
            .note("Added totals")
            .time("4 min ago");
        assert_eq!(version_line(&full), "v2 · Added totals · 4 min ago");
        assert_eq!(version_line(&ArtifactVersion::new("v1", "x")), "v1");
    }
}
