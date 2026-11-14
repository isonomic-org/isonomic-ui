use isonomic_ui::assets::AllAssets;
use isonomic_ui_gallery::{Gallery, create_new_window, init};

fn main() {
    let app = isonomic_ui::application().with_assets(AllAssets);

    // Parse `cargo run -- <story_name>`
    let name = std::env::args().nth(1);

    app.run(move |cx| {
        init(cx);
        // Required for system notifications on Windows; macOS only shows them
        // when running from a bundled .app.
        cx.set_app_identity("com.isonomic.ui.gallery", "Isonomic UI");
        cx.activate(true);

        create_new_window(
            "Isonomic UI",
            move |window, cx| Gallery::view(name.as_deref(), window, cx),
            cx,
        );
    });
}
