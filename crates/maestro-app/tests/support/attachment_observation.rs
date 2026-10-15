//! Complete attachment observations through the public boundary.
#![cfg(test)]
use maestro_app::browser::utils::attachment_utils::{Attachment, AttachmentError};
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
        Err(AttachmentError::Pptx(error)) => {
            let wrapped = AttachmentError::Pptx(error);
            let source = std::error::Error::source(&wrapped).unwrap();
            assert!(
                source
                    .downcast_ref::<maestro_app::browser::docx_preview::DocxError>()
                    .is_some()
            );
            assert!(!source.to_string().is_empty());
            let prefix = "Failed to process PPTX: ";
            assert!(wrapped.to_string().starts_with(prefix));
            json!({"error_kind":"Pptx", "prefix":prefix})
        }
        Err(AttachmentError::Unsupported(mime)) => {
            json!({"error":format!("Unsupported file type: {mime}")})
        }
        Err(AttachmentError::HttpStatus(_)) => json!({"error":"Failed to fetch file"}),
        Err(AttachmentError::Fetch(error)) => json!({"error":error.to_string()}),
    }
}
