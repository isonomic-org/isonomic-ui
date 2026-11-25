// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

mod avatar;
mod avatar_group;

pub use avatar::*;
pub use avatar_group::*;

use crate::Size;
use gpui::{Pixels, px};

/// Returns the size of the avatar based on the given [`Size`].
pub(super) fn avatar_size(size: Size) -> Pixels {
    match size {
        Size::Large => px(80.),
        Size::Medium => px(48.),
        Size::Small => px(24.),
        Size::XSmall => px(16.),
        Size::Size(size) => size,
    }
}
