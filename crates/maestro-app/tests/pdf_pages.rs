//! Owned PDF document rendering through the application adapter.
use maestro_app::browser::utils::attachments::portable::PdfDocument;

#[test]
fn pdf_pages_keep_scale_order_and_full_precision() {
    for (name, dimensions) in [
        ("synthetic-wide.pdf", (300, 150)),
        ("synthetic-tall.pdf", (150, 300)),
        ("synthetic-square.pdf", (240, 240)),
        ("synthetic-small.pdf", (60, 30)),
        ("synthetic-fractional-tall.pdf", (499, 750)),
        ("synthetic-fractional-wide.pdf", (750, 499)),
        ("synthetic-rotated.pdf", (150, 300)),
        ("synthetic-multipage.pdf", (300, 150)),
        ("synthetic-blank.pdf", (300, 150)),
        ("synthetic-f64-up.pdf", (399, 300)),
        ("synthetic-f64-down.pdf", (454, 445)),
        ("synthetic-decimal.pdf", (150, 375)),
        ("synthetic-skinny.pdf", (1, 1500)),
        ("empty.pdf", (918, 1188)),
    ] {
        let bytes = std::fs::read(format!(
            "{}/tests/fixtures/pdf/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let doc = PdfDocument::new(bytes).unwrap();
        assert_eq!(
            doc.num_pages(),
            if name == "synthetic-multipage.pdf" {
                3
            } else {
                1
            }
        );
        let mut pages = vec![];
        for number in 1..=doc.num_pages() {
            let page = doc.render_page(number).unwrap();
            assert_eq!((page.width, page.height), dimensions, "{name}");
            pages.push(page.png);
        }
        if pages.len() == 3 {
            assert_ne!(pages[0], pages[1]);
            assert_eq!(pages[0], pages[2]);
            let source =
                String::from_utf8(include_bytes!("fixtures/pdf/synthetic-multipage.pdf").to_vec())
                    .unwrap();
            for (index, id) in [3, 5, 7].into_iter().enumerate() {
                // Keep byte offsets intact while selecting a single original page tree child.
                let single = source.replace("/Count 3", "/Count 1").replace(
                    "/Kids [3 0 R 5 0 R 7 0 R]",
                    &format!("/Kids [{id} 0 R]            "),
                );
                let selected = PdfDocument::new(single.into_bytes()).unwrap();
                assert_eq!(selected.num_pages(), 1);
                assert_eq!(pages[index], selected.render_page(1).unwrap().png);
            }
        }
    }
}

#[test]
fn pdf_page_raster_is_opaque_and_keeps_drawn_content() {
    for (name, drawn) in [("empty.pdf", false), ("synthetic-blank.pdf", true)] {
        let bytes = std::fs::read(format!(
            "{}/tests/fixtures/pdf/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let page = PdfDocument::new(bytes).unwrap().render_page(1).unwrap();
        let pixels = hayro::vello_cpu::Pixmap::from_png(std::io::Cursor::new(page.png)).unwrap();
        assert!(pixels.data().iter().all(|pixel| pixel.a == 255));
        assert_eq!(
            pixels
                .data()
                .iter()
                .any(|pixel| [pixel.r, pixel.g, pixel.b] != [255, 255, 255]),
            drawn
        );
    }
}

#[test]
fn pdf_document_failures_preserve_categories() {
    use maestro_app::browser::utils::attachments::AttachmentError;
    for name in ["empty-data.pdf", "invalid.pdf"] {
        let bytes = std::fs::read(format!(
            "{}/tests/fixtures/pdf/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert!(matches!(
            PdfDocument::new(bytes),
            Err(AttachmentError::Pdf(_))
        ));
    }
    let doc =
        PdfDocument::new(include_bytes!("fixtures/pdf/synthetic-multipage.pdf").to_vec()).unwrap();
    for page in [0, 4] {
        assert!(matches!(
            doc.render_page(page),
            Err(AttachmentError::Pdf(_))
        ));
    }
    for page in [1, 3] {
        assert!(doc.render_page(page).is_ok());
    }
}
