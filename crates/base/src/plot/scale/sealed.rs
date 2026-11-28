// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

/// Keeps [`PlotValue`](super::PlotValue) closed to the types it lists.
pub trait Sealed {}

impl Sealed for f32 {}
impl Sealed for f64 {}
#[cfg(feature = "decimal")]
impl Sealed for rust_decimal::Decimal {}
