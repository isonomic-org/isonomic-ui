// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

pub(crate) mod adapter;
pub mod change;
mod delegate;
mod item;
pub mod state;
mod vec;

pub(crate) use adapter::SearchableListAdapter;
pub use change::SearchableListChange;
pub use delegate::{SearchableListDelegate, SearchableListItem};
pub use item::SearchableListItemElement;
pub use state::SearchableListState;
pub use vec::{SearchableGroup, SearchableVec};
