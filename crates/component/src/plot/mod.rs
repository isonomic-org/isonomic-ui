// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Plotting for Isonomic UI's charts.
//!
//! The unstyled primitives — scales, shapes, axes, grids, labels, [`Plot`] and
//! [`PlotElement`] — live in [`isonomic_ui_base::plot`] and are re-exported here.
//! This module adds the styled [`tooltip`] overlay and the `IntoPlot` derive.
pub use isonomic_ui_base::plot::*;
pub use isonomic_ui_macros::IntoPlot;

pub mod tooltip;
