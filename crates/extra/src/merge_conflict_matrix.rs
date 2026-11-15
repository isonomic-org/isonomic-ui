//! Merge conflict matrix: files down the side, branches across the top, and every place two branches touch the same file.
//!
//! A cell is filled when a branch changes a file, with how many lines.
//! A file changed by more than one branch is a conflict row: it's marked,
//! and a choice at the end of the row says how to resolve it: keep one
//! branch's version, combine them, or leave it for a person. A summary
//! counts what's resolved. The choices go to the owner.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::{Tone, resting_shadow};

/// How a conflict is to be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Keep one branch's version, by branch id.
    Keep(SharedString),
    /// Combine the changes.
    Combine,
    /// Leave it for a person.
    Manual,
}

/// One branch's change to one file: lines added and removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Touch {
    /// Lines added.
    pub added: u32,
    /// Lines removed.
    pub removed: u32,
}

/// A file and what each branch did to it.
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixFile {
    /// Its path.
    pub path: SharedString,
    /// Per branch, in branch order: the change, if any.
    pub touches: Vec<Option<Touch>>,
    /// How its conflict is to be resolved, once chosen.
    pub resolution: Option<Resolution>,
}

impl MatrixFile {
    /// Create a file row.
    pub fn new(path: impl Into<SharedString>, touches: Vec<Option<(u32, u32)>>) -> Self {
        Self {
            path: path.into(),
            touches: touches
                .into_iter()
                .map(|t| t.map(|(added, removed)| Touch { added, removed }))
                .collect(),
            resolution: None,
        }
    }

    /// Set how it's resolved.
    pub fn resolved(mut self, resolution: Resolution) -> Self {
        self.resolution = Some(resolution);
        self
    }

    /// Whether more than one branch changes it.
    pub fn conflicts(&self) -> bool {
        self.touches.iter().filter(|t| t.is_some()).count() > 1
    }
}

/// Conflicts and how many are resolved.
pub fn conflict_counts(files: &[MatrixFile]) -> (usize, usize) {
    let conflicts: Vec<_> = files.iter().filter(|f| f.conflicts()).collect();
    (
        conflicts.len(),
        conflicts.iter().filter(|f| f.resolution.is_some()).count(),
    )
}

type ResolveHandler = Rc<dyn Fn(&SharedString, Resolution, &mut Window, &mut App)>;

/// Files by branch, with conflicts to resolve.
#[derive(IntoElement)]
pub struct MergeConflictMatrix {
    id: ElementId,
    style: StyleRefinement,
    branches: Vec<(SharedString, SharedString)>,
    files: Vec<MatrixFile>,
    on_resolve: Option<ResolveHandler>,
}

impl MergeConflictMatrix {
    /// Create a matrix for branches, each given as id and name.
    pub fn new(
        id: impl Into<ElementId>,
        branches: Vec<(SharedString, SharedString)>,
        files: Vec<MatrixFile>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            branches,
            files,
            on_resolve: None,
        }
    }

    /// Run with a file's path and the resolution chosen.
    pub fn on_resolve(
        mut self,
        handler: impl Fn(&SharedString, Resolution, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolve = Some(Rc::new(handler));
        self
    }
}

impl Styled for MergeConflictMatrix {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MergeConflictMatrix {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let warning = Tone::Warning.color(cx);
        let success = Tone::Success.color(cx);
        let danger = Tone::Danger.color(cx);
        let (conflicts, resolved) = conflict_counts(&self.files);
        let cell_w = px(92.);
        let path_w = px(220.);
        let choice_w = px(160.);

        let header = h_flex()
            .h(px(34.))
            .border_b_1()
            .border_color(line)
            .text_xs()
            .font_medium()
            .text_color(quiet)
            .child(div().w(path_w).flex_none().px_3().child("File"))
            .children(self.branches.iter().map(|(_, name)| {
                div()
                    .w(cell_w)
                    .flex_none()
                    .px_2()
                    .truncate()
                    .font_family(mono.clone())
                    .child(name.clone())
            }))
            .child(div().w(choice_w).flex_none().px_2().child("Resolution"));

        let rows: Vec<_> = self
            .files
            .iter()
            .enumerate()
            .map(|(ix, file)| {
                let conflict = file.conflicts();
                let cells: Vec<_> = file
                    .touches
                    .iter()
                    .map(|touch| {
                        div().w(cell_w).flex_none().px_1().child(match touch {
                            Some(t) => h_flex()
                                .h(px(22.))
                                .gap_1()
                                .px_1p5()
                                .rounded(px(4.))
                                .bg(if conflict {
                                    warning.opacity(0.16)
                                } else {
                                    accent.opacity(0.1)
                                })
                                .text_xs()
                                .child(div().text_color(success).child(format!("+{}", t.added)))
                                .child(
                                    div()
                                        .text_color(danger)
                                        .child(format!("\u{2212}{}", t.removed)),
                                )
                                .into_any_element(),
                            None => div()
                                .h(px(22.))
                                .flex()
                                .items_center()
                                .px_1p5()
                                .text_xs()
                                .text_color(line)
                                .child("\u{b7}")
                                .into_any_element(),
                        })
                    })
                    .collect();
                let choice = if conflict {
                    let current = match &file.resolution {
                        Some(Resolution::Keep(b)) => format!(
                            "Keep {}",
                            self.branches
                                .iter()
                                .find(|(id, _)| id == b)
                                .map(|(_, n)| n.clone())
                                .unwrap_or_else(|| b.clone())
                        ),
                        Some(Resolution::Combine) => "Combine".to_string(),
                        Some(Resolution::Manual) => "Leave for a person".to_string(),
                        None => "Choose\u{2026}".to_string(),
                    };
                    let options: Vec<(String, Resolution)> = file
                        .touches
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| t.is_some())
                        .filter_map(|(bx, _)| self.branches.get(bx))
                        .map(|(id, name)| (format!("Keep {name}"), Resolution::Keep(id.clone())))
                        .chain([
                            ("Combine".to_string(), Resolution::Combine),
                            ("Leave for a person".to_string(), Resolution::Manual),
                        ])
                        .collect();
                    let on_resolve = self.on_resolve.clone();
                    let path = file.path.clone();
                    let chosen = file.resolution.clone();
                    Button::new(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("resolve-{ix}").into(),
                    ))
                    .xsmall()
                    .map(|b| {
                        if chosen.is_some() {
                            b.ghost()
                        } else {
                            b.outline()
                        }
                    })
                    .label(current)
                    .dropdown_caret(true)
                    .accessibility_label(format!("Resolve {}", file.path))
                    .dropdown_menu(move |menu, _, _| {
                        options.iter().fold(menu, |menu, (label, resolution)| {
                            let on_resolve = on_resolve.clone();
                            let path = path.clone();
                            let pick = resolution.clone();
                            menu.item(
                                PopupMenuItem::new(label.clone())
                                    .checked(chosen.as_ref() == Some(resolution))
                                    .on_click(move |_, window, cx| {
                                        if let Some(on_resolve) = &on_resolve {
                                            on_resolve(&path, pick.clone(), window, cx);
                                        }
                                    }),
                            )
                        })
                    })
                    .into_any_element()
                } else {
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child("No conflict")
                        .into_any_element()
                };
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("file-{ix}").into(),
                    ))
                    .h(px(36.))
                    .when(ix > 0, |this| this.border_t_1().border_color(line))
                    .when(conflict && file.resolution.is_none(), |this| {
                        this.bg(warning.opacity(0.05))
                    })
                    .role(Role::Row)
                    .aria_label(format!(
                        "{}, changed on {} branches{}",
                        file.path,
                        file.touches.iter().filter(|t| t.is_some()).count(),
                        if conflict { ", conflict" } else { "" }
                    ))
                    .child(
                        h_flex()
                            .w(path_w)
                            .flex_none()
                            .gap_1p5()
                            .px_3()
                            .child(if conflict {
                                Icon::new(if file.resolution.is_some() {
                                    IconName::CircleCheck
                                } else {
                                    IconName::TriangleAlert
                                })
                                .xsmall()
                                .text_color(if file.resolution.is_some() {
                                    success
                                } else {
                                    warning
                                })
                                .into_any_element()
                            } else {
                                div().w(px(12.)).into_any_element()
                            })
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .font_family(mono.clone())
                                    .child(file.path.clone()),
                            ),
                    )
                    .children(cells)
                    .child(div().w(choice_w).flex_none().px_2().child(choice))
            })
            .collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(900.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(div().flex_1().font_medium().child(format!(
                        "{} files across {} branches",
                        self.files.len(),
                        self.branches.len()
                    )))
                    .child(
                        div()
                            .px_1p5()
                            .rounded(px(4.))
                            .bg(if resolved == conflicts {
                                success.opacity(0.14)
                            } else {
                                chip
                            })
                            .text_xs()
                            .text_color(if resolved == conflicts { success } else { ink })
                            .child(if conflicts == 0 {
                                "No conflicts".to_string()
                            } else {
                                format!("{resolved} of {conflicts} conflicts resolved")
                            }),
                    ),
            )
            .child(
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "scroll".into(),
                    ))
                    .overflow_x_scroll()
                    .child(
                        v_flex()
                            .min_w(px(220. + 160. + 92. * self.branches.len() as f32))
                            .child(header)
                            .children(rows),
                    ),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflicts_are_files_two_branches_touch() {
        let files = vec![
            MatrixFile::new("a", vec![Some((1, 0)), None]),
            MatrixFile::new("b", vec![Some((1, 0)), Some((2, 2))]),
            MatrixFile::new("c", vec![Some((3, 1)), Some((1, 0))]).resolved(Resolution::Combine),
        ];
        assert!(!files[0].conflicts());
        assert_eq!(conflict_counts(&files), (2, 1));
    }
}
