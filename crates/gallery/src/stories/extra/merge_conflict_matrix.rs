// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the merge conflict matrix.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::merge_conflict_matrix::{MatrixFile, MergeConflictMatrix, Resolution};

/// Three assistants' branches of a bookkeeping workspace, about to be
/// merged. The story keeps each resolution.
pub struct MergeConflictMatrixStory {
    files: Vec<MatrixFile>,
}

fn files() -> Vec<MatrixFile> {
    vec![
        MatrixFile::new(
            "ledger/march.csv",
            vec![Some((201, 201)), Some((61, 61)), None],
        ),
        MatrixFile::new(
            "rules/matching.toml",
            vec![Some((18, 0)), Some((2, 2)), None],
        ),
        MatrixFile::new("payables/friday-run.json", vec![None, None, Some((9, 0))]),
        MatrixFile::new(
            "notes/close-march.md",
            vec![Some((24, 0)), None, Some((6, 11))],
        ),
        MatrixFile::new(
            "vendors/balances.csv",
            vec![None, Some((40, 38)), Some((3, 3))],
        )
        .resolved(Resolution::Keep("bank".into())),
        MatrixFile::new("receipts/index.json", vec![None, None, Some((43, 0))]),
    ]
}

impl MergeConflictMatrixStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { files: files() })
    }
}

impl Story for MergeConflictMatrixStory {
    fn title() -> &'static str {
        "Merge conflict matrix"
    }

    fn description() -> &'static str {
        "Files down the side, branches across the top, and every place two branches touch the same file."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MergeConflictMatrixStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let resolve = cx.entity();
        MergeConflictMatrix::new(
            "close-merge",
            vec![
                ("cards".into(), "card-matcher".into()),
                ("bank".into(), "bank-matcher".into()),
                ("pay".into(), "payables".into()),
            ],
            self.files.clone(),
        )
        .on_resolve(move |path, resolution, _, cx| {
            resolve.update(cx, |this, cx| {
                if let Some(file) = this.files.iter_mut().find(|f| &f.path == path) {
                    file.resolution = Some(resolution);
                }
                cx.notify();
            })
        })
    }
}
