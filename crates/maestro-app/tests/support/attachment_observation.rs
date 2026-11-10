//! Complete attachment observations through the public boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::browser::docx_preview::DocxError;
use maestro_app::browser::ssf::WorkbookError;
use maestro_app::browser::utils::attachments::{Attachment, AttachmentError};
use serde_json::{Value, json};
/// Compare full results, replacing only a checked nondeterministic identifier.
pub fn observed(result: Result<Attachment, AttachmentError>) -> Value {
    match result {
        Ok(mut value) => {
            let suffix = value
                .id
                .strip_prefix(&format!("{}_", value.file_name))
                .unwrap();
            let (time, random) = suffix.split_once('_').unwrap();
            let time: f64 = time.parse().unwrap();
            let random: f64 = random.parse().unwrap();
            assert!(time > 0.0 && time.fract() == 0.0);
            assert!((0.0..1.0).contains(&random));
            assert!(value.id.ends_with(ryu_js::Buffer::new().format(random)));
            value.id = format!("{}_<generated>", value.file_name);
            json!({"attachment":value})
        }
        Err(AttachmentError::Pdf(error)) => {
            assert!(!error.is_empty());
            json!({"error_kind":"Pdf", "prefix":"Failed to process PDF: "})
        }
        Err(error @ AttachmentError::Pptx(_)) => {
            package_failure::<DocxError>(&error, "Pptx", "Failed to process PPTX: ")
        }
        Err(error @ AttachmentError::Docx(_)) => {
            package_failure::<DocxError>(&error, "Docx", "Failed to process DOCX: ")
        }
        Err(error @ AttachmentError::Excel(_)) => {
            package_failure::<WorkbookError>(&error, "Excel", "Failed to process Excel: ")
        }
        Err(AttachmentError::Unsupported(mime)) => {
            json!({"error":format!("Unsupported file type: {mime}")})
        }
        Err(AttachmentError::HttpStatus(_)) => json!({"error":"Failed to fetch file"}),
        Err(AttachmentError::Fetch(error)) => json!({"error":error.to_string()}),
    }
}
/// Project a package-processing failure with its prefix and retained source.
fn package_failure<Source: std::error::Error + 'static>(
    wrapped: &AttachmentError,
    kind: &str,
    prefix: &str,
) -> Value {
    let source = std::error::Error::source(wrapped).unwrap();
    assert!(source.downcast_ref::<Source>().is_some());
    assert!(!source.to_string().is_empty());
    assert!(wrapped.to_string().starts_with(prefix));
    json!({"error_kind":kind, "prefix":prefix})
}
