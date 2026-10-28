//! PDF loading through the application boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_app::browser::utils::attachment_utils::{
    AttachmentSource, AttachmentType, load_attachment,
};
use std::sync::Arc;

#[test]
fn pdf_detection_prioritizes_mime_or_suffix() {
    let bytes = include_bytes!("fixtures/pdf/synthetic-wide.pdf");
    let fetch: maestro_app::Fetch = Arc::new(|_| panic!("local input fetched"));
    for row in support::pdf::routes().iter().take(5) {
        support::pdf::check_route(row);
    }
    let (name, mime) = ("a.pdf", "application/pdf");
    let attachment = crate::support::run(load_attachment(
        AttachmentSource::File {
            bytes,
            name,
            mime_type: mime,
        },
        None,
        &fetch,
    ))
    .unwrap();
    assert_eq!(attachment.r#type, AttachmentType::Document);
    assert_eq!(attachment.mime_type, "application/pdf");
    assert!(
        attachment
            .extracted_text
            .unwrap()
            .starts_with("<pdf filename=")
    );
}

#[test]
fn pdf_sources_retain_original_bytes_and_metadata() {
    for (index, row) in support::pdf::routes().iter().enumerate().take(12).skip(5) {
        if index != 9 {
            support::pdf::check_route(row);
        }
    }
}

#[test]
fn pdf_mime_detection_reads_exact_response_values() {
    for row in support::pdf::routes().iter().skip(12) {
        support::pdf::check_route(row);
    }
}

#[test]
fn pdf_page_wrappers_preserve_order_and_empty_pages() {
    let rows = support::pdf::documents();
    for name in [
        "synthetic-multipage.pdf",
        "synthetic-blank.pdf",
        "empty.pdf",
    ] {
        support::pdf::check_document(rows.iter().find(|row| row.input.file == name).unwrap());
    }
}

#[test]
fn pdf_literal_filenames_survive_wrapping() {
    support::pdf::check_route(&support::pdf::routes()[9]);
}

#[test]
fn pdf_preview_fits_160_and_upscales_small_pages() {
    for row in support::pdf::documents().iter().filter(|row| {
        row.input.file.starts_with("synthetic-")
            && ![
                "synthetic-multipage.pdf",
                "synthetic-blank.pdf",
                "synthetic-skinny.pdf",
            ]
            .contains(&row.input.file.as_str())
    }) {
        support::pdf::check_document(row);
    }
}

#[test]
fn pdf_preview_uses_first_page_without_later_content() {
    let attachment = support::pdf::load("synthetic-multipage.pdf").unwrap();
    let first = hayro::vello_cpu::Pixmap::from_png(std::io::Cursor::new(support::pdf::bytes(
        "synthetic-multipage.pdf.png",
    )))
    .unwrap();
    assert!(attachment.extracted_text.unwrap().contains("Second"));
    assert_eq!(
        support::pdf::pixels(attachment.preview.as_deref().unwrap()).data(),
        first.data()
    );
}

#[test]
fn pdf_preview_keeps_opaque_background_and_drawn_content() {
    for (name, drawn) in [("empty.pdf", false), ("synthetic-blank.pdf", true)] {
        let attachment = support::pdf::load(name).unwrap();
        let pixels = support::pdf::pixels(attachment.preview.as_deref().unwrap());
        assert!(pixels.data().iter().all(|pixel| pixel.a == 255));
        assert_eq!(
            pixels
                .data()
                .iter()
                .any(|pixel| [pixel.r, pixel.g, pixel.b] != [255, 255, 255]),
            drawn
        );
        assert!(
            pixels
                .data()
                .iter()
                .any(|pixel| pixel.to_u8_array() == [255, 255, 255, 255])
        );
    }
}

#[test]
fn pdf_processing_errors_are_categorized() {
    for row in support::pdf::documents().iter().filter(|row| {
        ["empty-data.pdf", "invalid.pdf", "broken.pdf"].contains(&row.input.file.as_str())
    }) {
        support::pdf::check_document(row);
    }
}

#[test]
fn pdf_url_failures_precede_parsing() {
    use maestro_models::{FetchError, HttpResponse};
    for status in [403, 500] {
        let fetch: maestro_app::Fetch = Arc::new(move |_| {
            let body = Box::pin(futures_util::stream::poll_fn(|_| {
                panic!("rejected body polled")
            }));
            Box::pin(async move {
                Ok(HttpResponse {
                    status,
                    status_text: String::new(),
                    headers: [("content-type".into(), "application/pdf".into())].into(),
                    body,
                })
            })
        });
        assert!(
            matches!(support::run(load_attachment(AttachmentSource::Url("https://example.test/a.pdf"), None, &fetch)), Err(maestro_app::browser::utils::attachment_utils::AttachmentError::HttpStatus(actual)) if actual == status)
        );
    }
    let fetch: maestro_app::Fetch = Arc::new(|_| Box::pin(async { Err(FetchError::Aborted) }));
    assert!(matches!(
        support::run(load_attachment(
            AttachmentSource::Url("https://example.test/a.pdf"),
            None,
            &fetch
        )),
        Err(
            maestro_app::browser::utils::attachment_utils::AttachmentError::Fetch(
                FetchError::Aborted
            )
        )
    ));
    let fetch: maestro_app::Fetch = Arc::new(|_| {
        Box::pin(async {
            Ok(HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: [].into(),
                body: Box::pin(futures_util::stream::once(async {
                    Err(FetchError::Aborted)
                })),
            })
        })
    });
    assert!(matches!(
        support::run(load_attachment(
            AttachmentSource::Url("https://example.test/a.pdf"),
            None,
            &fetch
        )),
        Err(
            maestro_app::browser::utils::attachment_utils::AttachmentError::Fetch(
                FetchError::Aborted
            )
        )
    ));
}

#[test]
fn pdf_font_and_text_corpus_keeps_approved_engine_results() {
    let rows = support::pdf::documents();
    assert_eq!(rows.len(), 48);
    for row in rows
        .iter()
        .take(32)
        .filter(|row| row.input.file != "empty.pdf")
    {
        support::pdf::check_document(row);
    }
    let known: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|row| row.input.file.clone())
        .chain(rows.iter().filter_map(|row| match &row.expected {
            support::pdf::DocumentExpected::Success {
                preview: Some(preview),
                ..
            } => Some(preview.file.clone()),
            _ => None,
        }))
        .chain(support::pdf::text_show_files())
        .chain(support::pdf::AARCH64_PREVIEWS.map(support::pdf::aarch64_preview))
        .chain([
            "documents.json".into(),
            "routes.json".into(),
            "runs.json".into(),
            "text-shows.json".into(),
            "LICENSE".into(),
            "owned-inherited-form-cycle.pdf".into(),
            "owned-shared-form-cycle.pdf".into(),
        ])
        .collect();
    for entry in std::fs::read_dir(support::pdf::path("")).unwrap() {
        assert!(known.contains(entry.unwrap().file_name().to_str().unwrap()));
    }
}

#[test]
fn pdf_preview_encoding_failure_keeps_text() {
    let row = support::pdf::documents()
        .into_iter()
        .find(|row| row.input.file == "synthetic-skinny.pdf")
        .unwrap();
    support::pdf::check_document(&row);
    let pdf = hayro::hayro_syntax::Pdf::new(support::pdf::bytes("synthetic-skinny.pdf")).unwrap();
    let page = &pdf.pages()[0];
    let (width, height) = page.render_dimensions();
    assert_eq!((width, height), (1.0, 1000.0));
    assert!(hayro::vello_cpu::Pixmap::new(0, 160).into_png().is_err());
}

#[test]
fn pdf_text_modes_keep_one_run_and_repeated_show_operations() {
    support::pdf::check_text_shows(
        "pdf_text_modes_keep_one_run_and_repeated_show_operations",
        16,
    );
}

#[test]
fn pdf_translated_fill_and_stroke_shows_keep_both_texts() {
    let bytes = support::pdf::synthetic(
        "BT /F1 12 Tf 0 Tr 5 30 Td (Page A) Tj ET\n\
         1 0 0 1 0 100 cm\n\
         BT /F1 12 Tf 1 Tr 5 30 Td (Page A) Tj ET",
        false,
    );
    let fetch: maestro_app::Fetch = Arc::new(|_| panic!("local input fetched"));
    let attachment = support::run(load_attachment(
        AttachmentSource::Buffer(&bytes),
        Some("translated.pdf"),
        &fetch,
    ))
    .unwrap();
    assert_eq!(
        attachment.extracted_text.as_deref(),
        Some(
            "<pdf filename=\"translated.pdf\">\n<page number=\"1\">\nPage A Page A\n</page>\n</pdf>"
        )
    );
}

#[test]
fn pdf_preview_preserves_full_precision_direct_and_inherited_boxes() {
    for inherited in [false, true] {
        let bytes = support::pdf::synthetic("", inherited);
        let fetch: maestro_app::Fetch = Arc::new(|_| panic!("local input fetched"));
        let attachment = support::run(load_attachment(
            AttachmentSource::Buffer(&bytes),
            Some("geometry.pdf"),
            &fetch,
        ))
        .unwrap();
        let pixels = support::pdf::pixels(attachment.preview.as_deref().unwrap());
        assert_eq!(
            (pixels.width(), pixels.height()),
            (64, 160),
            "inherited {inherited}"
        );
    }
}

#[test]
fn pdf_clip_only_text_is_extracted() {
    support::pdf::check_text_shows("pdf_clip_only_text_is_extracted", 1);
}

#[test]
fn pdf_separate_fill_then_stroke_runs_are_retained() {
    support::pdf::check_text_shows("pdf_separate_fill_then_stroke_runs_are_retained", 1);
}

#[test]
fn pdf_text_show_operators_keep_their_boundaries() {
    support::pdf::check_text_shows("pdf_text_show_operators_keep_their_boundaries", 4);
}

#[test]
fn pdf_text_show_order_retains_distinct_and_repeated_runs() {
    support::pdf::check_text_shows("pdf_text_show_order_retains_distinct_and_repeated_runs", 1);
}

#[test]
fn pdf_empty_text_shows_leave_no_output() {
    support::pdf::check_text_shows("pdf_empty_text_shows_leave_no_output", 1);
}

#[test]
fn pdf_form_text_shows_use_the_same_extractor() {
    support::pdf::check_text_shows("pdf_form_text_shows_use_the_same_extractor", 2);
}

#[test]
fn pdf_compressed_stream_keeps_clip_text() {
    support::pdf::check_text_shows("pdf_compressed_stream_keeps_clip_text", 1);
}

#[test]
fn pdf_text_show_preserves_multi_scalar_unicode_mapping() {
    support::pdf::check_text_shows("pdf_text_show_preserves_multi_scalar_unicode_mapping", 1);
}

#[test]
fn pdf_clip_only_text_does_not_paint() {
    let attachments = support::pdf::check_text_shows("pdf_clip_only_text_does_not_paint", 1);
    let pixels = support::pdf::pixels(attachments[0].preview.as_deref().unwrap());
    assert_eq!((pixels.width(), pixels.height()), (64, 160));
    assert!(
        pixels
            .data()
            .iter()
            .all(|pixel| pixel.to_u8_array() == [255, 255, 255, 255])
    );
}

#[test]
fn pdf_stroke_then_fill_runs_are_retained() {
    support::pdf::check_text_shows("pdf_stroke_then_fill_runs_are_retained", 1);
}

#[test]
fn pdf_outlined_shows_at_identical_positions_remain_distinct() {
    support::pdf::check_text_shows(
        "pdf_outlined_shows_at_identical_positions_remain_distinct",
        2,
    );
}

#[test]
fn pdf_text_extraction_includes_hidden_optional_content() {
    support::pdf::check_text_shows("pdf_text_extraction_includes_hidden_optional_content", 1);
}

#[test]
fn pdf_text_bytes_survive_native_decoding_serialization() {
    support::pdf::check_text_shows("pdf_text_bytes_survive_native_decoding_serialization", 3);
}

#[test]
fn pdf_font_state_survives_save_restore_and_text_objects() {
    support::pdf::check_text_shows("pdf_font_state_survives_save_restore_and_text_objects", 1);
}

#[test]
fn pdf_graphics_state_selects_font_without_resetting_it() {
    support::pdf::check_text_shows("pdf_graphics_state_selects_font_without_resetting_it", 1);
}

#[test]
fn pdf_form_fonts_keep_resource_scope_and_restore_parent() {
    support::pdf::check_text_shows("pdf_form_fonts_keep_resource_scope_and_restore_parent", 2);
}

#[test]
fn pdf_font_names_retain_escaped_bytes() {
    support::pdf::check_text_shows("pdf_font_names_retain_escaped_bytes", 1);
}

#[test]
fn pdf_font_selection_controls_default_show_fallbacks() {
    support::pdf::check_text_shows("pdf_font_selection_controls_default_show_fallbacks", 4);
}

#[test]
fn pdf_form_cycles_stop_without_dropping_repeated_finite_forms() {
    support::pdf::check_text_shows(
        "pdf_form_cycles_stop_without_dropping_repeated_finite_forms",
        1,
    );
    let repeated = support::pdf::load("show-form-repeat.pdf").unwrap();
    assert_eq!(
        repeated.extracted_text.as_deref(),
        Some(
            "<pdf filename=\"show-form-repeat.pdf\">\n<page number=\"1\">\nPage A Page A\n</page>\n</pdf>"
        )
    );
}

#[test]
fn pdf_inherited_resource_form_cycles_stop_and_allow_later_invocations() {
    let attachment = support::pdf::load("owned-inherited-form-cycle.pdf").unwrap();
    assert_eq!(
        attachment.extracted_text.as_deref(),
        Some(
            "<pdf filename=\"owned-inherited-form-cycle.pdf\">\n<page number=\"1\">\nP P\n</page>\n</pdf>"
        )
    );
}

#[test]
fn pdf_shared_resource_form_cycles_stop_and_allow_later_invocations() {
    let attachment = support::pdf::load("owned-shared-form-cycle.pdf").unwrap();
    assert_eq!(
        attachment.extracted_text.as_deref(),
        Some(
            "<pdf filename=\"owned-shared-form-cycle.pdf\">\n<page number=\"1\">\nP P P P P P\n</page>\n</pdf>"
        )
    );
}

#[test]
fn pdf_deep_finite_forms_do_not_inherit_raster_depth_limit() {
    support::pdf::check_text_shows("pdf_deep_finite_forms_do_not_inherit_raster_depth_limit", 1);
}
