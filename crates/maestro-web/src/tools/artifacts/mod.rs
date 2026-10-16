//! Artifact view construction using application-owned filename policy.
pub mod artifact_element;
pub mod generic_artifact;
pub mod pdf_artifact;
use crate::browser::attachment_content::{AttachmentHost, AttachmentUiError};
use artifact_element::ArtifactElement;
use maestro_app::browser::tools::artifacts::{FileType, get_file_type};
use std::rc::Rc;
/// Construct the installed view without disguising recognized specialized files.
/// # Errors
/// Returns a presentation error when the selected specialized view is unavailable.
pub fn create_artifact_element(
    filename: &str,
    content: &str,
    host: Rc<dyn AttachmentHost>,
) -> Result<Box<dyn ArtifactElement>, AttachmentUiError> {
    let mut element: Box<dyn ArtifactElement> = match get_file_type(filename) {
        FileType::Pdf => Box::new(pdf_artifact::PdfArtifact::new(host)),
        FileType::Generic => Box::new(generic_artifact::GenericArtifact::new(host)),
        _ => {
            return Err(AttachmentUiError::Presentation(
                "Artifact view is not installed".into(),
            ));
        }
    };
    element.set_filename(filename.into());
    element.set_content(content.into());
    Ok(element)
}
