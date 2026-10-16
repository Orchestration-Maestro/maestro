//! Artifact view construction using application-owned filename policy.
pub mod artifact_element;
pub mod generic_artifact;
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
    if get_file_type(filename) != FileType::Generic {
        return Err(AttachmentUiError::Presentation(
            "Artifact view is not installed".into(),
        ));
    }
    let mut element = generic_artifact::GenericArtifact::new(host);
    element.set_filename(filename.into());
    element.set_content(content.into());
    Ok(Box::new(element))
}
