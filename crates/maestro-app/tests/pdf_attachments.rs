//! PDF loading through the application boundary.
#![cfg(test)]
use maestro_app::browser::utils::attachment_utils::{
    AttachmentSource, AttachmentType, load_attachment,
};
use std::sync::Arc;
mod support {
    /// Complete identifier-aware observations.
    pub mod attachment_observation;
    /// Strict PDF fixture consumers.
    pub mod pdf;
}

#[test]
fn pdf_detection_prioritizes_mime_or_suffix() {
    let bytes = include_bytes!("fixtures/pdf/synthetic-wide.pdf");
    let fetch: maestro_app::Fetch = Arc::new(|_| panic!("local input fetched"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for row in support::pdf::routes().iter().take(5) {
        support::pdf::check_route(row);
    }
    let (name, mime) = ("a.pdf", "application/pdf");
    let attachment = runtime
        .block_on(load_attachment(
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
            matches!(support::pdf::run(load_attachment(AttachmentSource::Url("https://example.test/a.pdf"), None, &fetch)), Err(maestro_app::browser::utils::attachment_utils::AttachmentError::HttpStatus(actual)) if actual == status)
        );
    }
    let fetch: maestro_app::Fetch = Arc::new(|_| Box::pin(async { Err(FetchError::Aborted) }));
    assert!(matches!(
        support::pdf::run(load_attachment(
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
        support::pdf::run(load_attachment(
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
        .chain([
            "documents.json".into(),
            "routes.json".into(),
            "runs.json".into(),
            "LICENSE".into(),
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
    for mode in [1, 2, 3, 6] {
        let bytes = support::pdf::synthetic(
            &format!("BT /F1 12 Tf {mode} Tr 5 30 Td (Page A) Tj (Page A) Tj ET"),
            false,
        );
        let fetch: maestro_app::Fetch = Arc::new(|_| panic!("local input fetched"));
        let attachment = support::pdf::run(load_attachment(
            AttachmentSource::Buffer(&bytes),
            Some("runs.pdf"),
            &fetch,
        ))
        .unwrap();
        assert_eq!(
            attachment.extracted_text.as_deref(),
            Some(
                "<pdf filename=\"runs.pdf\">\n<page number=\"1\">\nPage A Page A\n</page>\n</pdf>"
            ),
            "mode {mode}"
        );
    }
}

#[test]
fn pdf_preview_preserves_full_precision_direct_and_inherited_boxes() {
    for inherited in [false, true] {
        let bytes = support::pdf::synthetic("", inherited);
        let fetch: maestro_app::Fetch = Arc::new(|_| panic!("local input fetched"));
        let attachment = support::pdf::run(load_attachment(
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
