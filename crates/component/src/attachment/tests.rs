// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

#[test]
fn file_sizes_read_in_binary_units() {
    assert_eq!(format_file_size(0).as_ref(), "0 B");
    assert_eq!(format_file_size(812).as_ref(), "812 B");
    assert_eq!(format_file_size(1024).as_ref(), "1.0 KB");
    assert_eq!(format_file_size(4300).as_ref(), "4.2 KB");
    assert_eq!(format_file_size(12 * 1024 + 300).as_ref(), "12 KB");
    assert_eq!(format_file_size(2_516_582).as_ref(), "2.4 MB");
    // Rounding up carries into the next unit instead of "1024 KB".
    assert_eq!(format_file_size(1024 * 1024 - 10).as_ref(), "1.0 MB");
    assert_eq!(format_file_size(3 * 1024 * 1024 * 1024).as_ref(), "3.0 GB");
}

#[test]
fn file_kind_comes_from_the_extension() {
    assert_eq!(file_kind("report.pdf").as_deref(), Some("PDF"));
    assert_eq!(file_kind("archive.tar.gz").as_deref(), Some("GZ"));
    assert_eq!(file_kind("Makefile"), None);
    assert_eq!(file_kind(".env"), None);
    assert_eq!(file_kind("trailing."), None);
    assert_eq!(file_meta("photo.PNG", 2048).as_ref(), "PNG · 2.0 KB");
    assert_eq!(file_meta("README", 90).as_ref(), "90 B");
}

#[test]
fn file_family_picks_a_glyph_by_extension() {
    assert_eq!(FileFamily::of("a.JPG"), FileFamily::Image);
    assert_eq!(FileFamily::of("clip.mov"), FileFamily::Video);
    assert_eq!(FileFamily::of("memo.m4a"), FileFamily::Audio);
    assert_eq!(FileFamily::of("q3.pdf"), FileFamily::Pdf);
    assert_eq!(FileFamily::of("assets.zip"), FileFamily::Archive);
    assert_eq!(FileFamily::of("rows.csv"), FileFamily::Spreadsheet);
    assert_eq!(FileFamily::of("main.rs"), FileFamily::Code);
    assert_eq!(FileFamily::of("notes.md"), FileFamily::Text);
    assert_eq!(FileFamily::of("blob"), FileFamily::Other);
    assert_eq!(
        FileFamily::Pdf.icon(),
        isonomic_ui_assets::IconName::DocumentPdf
    );
}

#[test]
fn spoken_label_says_the_state_in_words() {
    assert_eq!(
        spoken_label("q3.pdf", AttachmentStatus::Uploading, Some(39.6)).as_ref(),
        "q3.pdf, uploading, 40 percent"
    );
    // Progress only counts while uploading.
    assert_eq!(
        spoken_label("q3.pdf", AttachmentStatus::Failed, Some(40.)).as_ref(),
        "q3.pdf, failed"
    );
    assert_eq!(
        spoken_label("q3.pdf", AttachmentStatus::Pending, None).as_ref(),
        "q3.pdf, waiting to upload"
    );
}

#[test]
fn test_attachment_builder() {
    let mut attachment = Attachment::new()
        .status(AttachmentStatus::Uploading)
        .axis(Axis::Vertical)
        .with_size(Size::Small)
        .media(AttachmentMedia::new().src("preview.png"))
        .content(
            AttachmentContent::new()
                .title(AttachmentTitle::new("report.pdf"))
                .description(AttachmentDescription::new("Uploading")),
        )
        .actions(AttachmentActions::new().child("Cancel"));

    assert_eq!(attachment.status, AttachmentStatus::Uploading);
    assert_eq!(attachment.axis, Axis::Vertical);
    assert_eq!(attachment.size, Size::Small);
    assert!(attachment.media.is_some());
    assert!(attachment.content.is_some());
    assert!(attachment.actions.is_some());

    attachment.layout_slots();
    assert_eq!(attachment.media.as_ref().unwrap().size, Some(Size::Small));
    assert_eq!(
        attachment.media.as_ref().unwrap().status,
        AttachmentStatus::Uploading
    );
    assert!(attachment.content.as_ref().unwrap().vertical_layout);
    assert!(attachment.actions.as_ref().unwrap().vertical_layout);

    let handle = ScrollHandle::new();
    let group = AttachmentGroup::new("group")
        .track_scroll(&handle)
        .with_edge_fade(gpui::black())
        .child("first")
        .child("second");
    assert!(group.scroll_handle.is_some());
    assert_eq!(group.edge_fade, Some(gpui::black()));
    assert_eq!(group.children.len(), 2);
    assert!(AttachmentGroup::new("plain").scroll_handle.is_none());
    assert!(AttachmentGroup::new("plain").edge_fade.is_none());
}

#[test]
fn test_attachment_whole_card_click_builder() {
    assert!(Attachment::new().id.is_none());
    assert!(Attachment::new().on_click.is_none());

    let clickable = Attachment::new()
        .id("report-attachment")
        .on_click(|_, _, _| {});
    assert_eq!(clickable.id, Some("report-attachment".into()));
    assert!(clickable.on_click.is_some());
}

#[test]
fn test_attachment_corner_controls_builder() {
    assert!(Attachment::new().on_remove.is_none());
    assert!(Attachment::new().on_retry.is_none());

    let attachment = Attachment::new()
        .id("upload")
        .on_remove(|_, _, _| {})
        .on_retry(|_, _, _| {});
    assert!(attachment.on_remove.is_some());
    assert!(attachment.on_retry.is_some());
}

#[test]
fn test_attachment_retry_control_reaches_slots_only_while_failed() {
    let mut failed = Attachment::new()
        .id("upload")
        .status(AttachmentStatus::Failed)
        .on_retry(|_, _, _| {})
        .media(AttachmentMedia::new().src("preview.png"))
        .content(AttachmentContent::new().description(AttachmentDescription::new("Failed")));
    failed.layout_slots();
    assert!(failed.media.as_ref().unwrap().retry.is_some());
    assert!(failed.content.as_ref().unwrap().retry.is_some());

    // A completed attachment offers no retry, and one without an identity
    // has nothing to key the control on.
    let mut complete = Attachment::new()
        .id("upload")
        .on_retry(|_, _, _| {})
        .media(AttachmentMedia::new().src("preview.png"));
    complete.layout_slots();
    assert!(complete.media.as_ref().unwrap().retry.is_none());

    let mut anonymous = Attachment::new()
        .status(AttachmentStatus::Failed)
        .on_retry(|_, _, _| {})
        .media(AttachmentMedia::new().src("preview.png"));
    anonymous.layout_slots();
    assert!(anonymous.media.as_ref().unwrap().retry.is_none());
}

#[test]
fn test_attachment_progress_and_tooltip_builder() {
    assert!(Attachment::new().progress.is_none());
    assert!(Attachment::new().tooltip.is_none());
    assert_eq!(Attachment::new().progress(130.).progress, Some(100.));
    assert_eq!(Attachment::new().progress(-5.).progress, Some(0.));
    assert_eq!(
        Attachment::new().tooltip("Network error").tooltip,
        Some("Network error".into())
    );

    // Progress reaches the slots only while uploading.
    let mut uploading = Attachment::new()
        .id("upload")
        .status(AttachmentStatus::Uploading)
        .progress(62.)
        .media(AttachmentMedia::new().src("preview.png"))
        .content(AttachmentContent::new().description(AttachmentDescription::new("Uploading")));
    uploading.layout_slots();
    assert_eq!(uploading.media.as_ref().unwrap().progress, Some(62.));
    assert_eq!(uploading.content.as_ref().unwrap().progress, Some(62.));

    let mut processing = Attachment::new()
        .status(AttachmentStatus::Processing)
        .progress(62.)
        .media(AttachmentMedia::new().src("preview.png"));
    processing.layout_slots();
    assert!(processing.media.as_ref().unwrap().progress.is_none());
}

#[test]
fn test_attachment_image_tile_media_is_flush() {
    let mut tile = Attachment::new()
        .axis(Axis::Vertical)
        .media(AttachmentMedia::new().src("preview.png"));
    tile.layout_slots();
    assert!(tile.media.as_ref().unwrap().flush);

    let mut card = Attachment::new()
        .axis(Axis::Vertical)
        .media(AttachmentMedia::new().src("preview.png"))
        .content(AttachmentContent::new().title(AttachmentTitle::new("preview.png")));
    card.layout_slots();
    assert!(!card.media.as_ref().unwrap().flush);
}

#[test]
fn test_attachment_defaults_and_status_helpers() {
    assert_eq!(Attachment::new().status, AttachmentStatus::Complete);
    assert_eq!(AttachmentStatus::default(), AttachmentStatus::Complete);
    assert!(AttachmentStatus::Pending.is_pending());
    assert!(AttachmentStatus::Uploading.is_in_progress());
    assert!(AttachmentStatus::Processing.is_processing());
    assert!(AttachmentStatus::Failed.is_failed());
    assert!(AttachmentStatus::Complete.is_complete());
    assert!(!AttachmentStatus::Complete.is_in_progress());
}

#[test]
fn test_attachment_slots_are_composable() {
    let media = AttachmentMedia::new().child("icon");
    assert_eq!(media.children.len(), 1);

    let content = AttachmentContent::new()
        .title(AttachmentTitle::new("name"))
        .description(AttachmentDescription::new("Details"))
        .child("Custom progress");
    assert_eq!(content.children.len(), 3);
    assert!(matches!(
        content.children[0],
        AttachmentContentChild::Title(_)
    ));
    assert!(matches!(
        content.children[1],
        AttachmentContentChild::Description(_)
    ));
    assert!(matches!(
        content.children[2],
        AttachmentContentChild::Element(_)
    ));

    let legacy = AttachmentContent::new().child(AttachmentTitle::new("legacy"));
    assert!(matches!(
        legacy.children[0],
        AttachmentContentChild::Element(_)
    ));

    let actions = AttachmentActions::new().child("remove");
    assert_eq!(actions.children.len(), 1);
}

#[test]
fn test_attachment_typed_content_inherits_status() {
    let mut attachment = Attachment::new()
        .status(AttachmentStatus::Uploading)
        .content(
            AttachmentContent::new()
                .title(AttachmentTitle::new("report.pdf"))
                .description(AttachmentDescription::new("Uploading")),
        );

    attachment.layout_slots();

    let content = attachment.content.unwrap();
    let AttachmentContentChild::Title(title) = &content.children[0] else {
        panic!("expected the typed title slot");
    };
    assert_eq!(title.status, Some(AttachmentStatus::Uploading));

    let AttachmentContentChild::Description(description) = &content.children[1] else {
        panic!("expected the typed description slot");
    };
    assert_eq!(description.status, Some(AttachmentStatus::Uploading));
}

#[test]
fn test_attachment_explicit_child_status_overrides_parent() {
    let mut attachment = Attachment::new().status(AttachmentStatus::Failed).content(
        AttachmentContent::new()
            .title(AttachmentTitle::new("report.pdf").status(AttachmentStatus::Processing))
            .description(
                AttachmentDescription::new("Previous upload completed")
                    .status(AttachmentStatus::Complete),
            ),
    );

    attachment.layout_slots();

    let content = attachment.content.unwrap();
    let AttachmentContentChild::Title(title) = &content.children[0] else {
        panic!("expected the typed title slot");
    };
    assert_eq!(title.status, Some(AttachmentStatus::Processing));

    let AttachmentContentChild::Description(description) = &content.children[1] else {
        panic!("expected the typed description slot");
    };
    assert_eq!(description.status, Some(AttachmentStatus::Complete));
}

#[test]
fn test_attachment_title_keeps_custom_shimmer_style() {
    let mut attachment = Attachment::new()
        .status(AttachmentStatus::Processing)
        .content(
            AttachmentContent::new().title(
                AttachmentTitle::new("report.pdf")
                    .with_shimmer_style(ShimmerStyle::new().spread(0.45).reverse(true)),
            ),
        );

    attachment.layout_slots();

    let content = attachment.content.unwrap();
    let AttachmentContentChild::Title(title) = &content.children[0] else {
        panic!("expected the typed title slot");
    };
    assert_eq!(title.status, Some(AttachmentStatus::Processing));
    assert!(title.shimmer_style.is_some());
}

#[test]
fn test_attachment_media_preview_keeps_children_and_overlays() {
    let media = AttachmentMedia::new()
        .src("preview.png")
        .child("Existing overlay")
        .overlay("Centered overlay");

    assert!(media.source.is_some());
    assert_eq!(media.children.len(), 1);
    assert_eq!(media.overlays.len(), 1);
}

#[test]
fn test_attachment_media_size_inherits_root_unless_explicit() {
    let slot = |size, status, axis, flush| SlotLayout {
        size,
        status,
        axis,
        flush,
        retry: None,
        progress: None,
        id: None,
    };
    let inherited = AttachmentMedia::new().layout(slot(
        Size::Small,
        AttachmentStatus::Complete,
        Axis::Vertical,
        true,
    ));
    assert_eq!(inherited.size, Some(Size::Small));
    assert_eq!(inherited.axis, Axis::Vertical);
    assert!(inherited.flush);

    let explicit = AttachmentMedia::new().with_size(Size::XSmall).layout(slot(
        Size::Large,
        AttachmentStatus::Failed,
        Axis::Horizontal,
        false,
    ));
    assert_eq!(explicit.size, Some(Size::XSmall));
    assert_eq!(explicit.status, AttachmentStatus::Failed);
}

#[test]
fn test_attachment_group_builder() {
    let group = AttachmentGroup::new("attachments")
        .child("First")
        .child("Second");

    assert_eq!(group.children.len(), 2);
}

mod click_dispatch {
    use std::{cell::Cell, rc::Rc};

    use gpui::{Context, Modifiers, Render, TestAppContext, point, px};

    use super::super::*;
    use crate::button::Button;

    struct AttachmentClickHarness {
        card_clicks: Rc<Cell<usize>>,
        action_clicks: Rc<Cell<usize>>,
    }

    impl Render for AttachmentClickHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let card_clicks = self.card_clicks.clone();
            let action_clicks = self.action_clicks.clone();

            Attachment::new()
                .id("attachment")
                .w(px(200.))
                .h(px(60.))
                .on_click(move |_, _, _| card_clicks.set(card_clicks.get() + 1))
                .actions(
                    AttachmentActions::new().child(
                        Button::new("open")
                            .w(px(40.))
                            .h(px(40.))
                            .on_click(move |_, _, _| action_clicks.set(action_clicks.get() + 1)),
                    ),
                )
        }
    }

    #[gpui::test]
    fn whole_card_click_stays_below_the_actions(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let card_clicks = Rc::new(Cell::new(0));
        let action_clicks = Rc::new(Cell::new(0));
        let (_, cx) = cx.add_window_view({
            let card_clicks = card_clicks.clone();
            let action_clicks = action_clicks.clone();
            move |_, _| AttachmentClickHarness {
                card_clicks,
                action_clicks,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        // A click on an action must not also fire the whole-card handler.
        cx.simulate_click(point(px(20.), px(30.)), Modifiers::default());
        assert_eq!(action_clicks.get(), 1);
        assert_eq!(card_clicks.get(), 0);

        // A click elsewhere on the card fires the whole-card handler.
        cx.simulate_click(point(px(150.), px(30.)), Modifiers::default());
        assert_eq!(action_clicks.get(), 1);
        assert_eq!(card_clicks.get(), 1);
    }
}

mod retry_dispatch {
    use std::{cell::Cell, rc::Rc};

    use gpui::{
        Context, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, Render, TestAppContext,
        VisualTestContext, point, px,
    };

    use super::super::*;

    struct FailedMediaHarness {
        retry: bool,
        retries: Rc<Cell<usize>>,
    }

    impl Render for FailedMediaHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let retries = self.retries.clone();
            Attachment::new()
                .id("failed-attachment")
                .status(AttachmentStatus::Failed)
                .media(AttachmentMedia::new())
                .when(self.retry, |attachment| {
                    attachment.on_retry(move |_, _, _| retries.set(retries.get() + 1))
                })
        }
    }

    fn harness(cx: &mut TestAppContext, retry: bool) -> (&mut VisualTestContext, Rc<Cell<usize>>) {
        cx.update(crate::init);
        let retries = Rc::new(Cell::new(0));
        let (_, cx) = cx.add_window_view({
            let retries = retries.clone();
            move |_, _| FailedMediaHarness { retry, retries }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (cx, retries)
    }

    #[gpui::test]
    fn failed_media_without_source_retries_by_pointer_and_keyboard(cx: &mut TestAppContext) {
        let (cx, retries) = harness(cx, true);
        cx.simulate_click(point(px(20.), px(20.)), Modifiers::default());
        assert_eq!(retries.get(), 1);

        cx.update(|window, cx| window.focus_next(cx));
        cx.update(|window, cx| assert!(window.focused(cx).is_some()));
        let keystroke = Keystroke::parse("enter").unwrap();
        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
        assert_eq!(retries.get(), 2);
    }

    #[gpui::test]
    fn failed_media_without_retry_has_no_action(cx: &mut TestAppContext) {
        let (cx, retries) = harness(cx, false);
        cx.simulate_click(point(px(20.), px(20.)), Modifiers::default());
        assert_eq!(retries.get(), 0);
        cx.update(|window, cx| window.focus_next(cx));
        cx.update(|window, cx| assert!(window.focused(cx).is_none()));
    }
}
