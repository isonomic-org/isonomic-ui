//! Stories for the components of `isonomic-ui-extra`, one file per
//! component. `build.rs` finds the files here and registers each story with
//! the gallery, so a new story needs nothing beyond its own file.

include!(concat!(env!("OUT_DIR"), "/extra_stories.rs"));
