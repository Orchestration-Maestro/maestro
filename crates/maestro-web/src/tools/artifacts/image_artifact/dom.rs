//! Native image content sharing source and failure handling with the retained consumer.
use super::{Body, ImageArtifact};
use crate::{
    dom::{DomContent, DomRoot, DomView},
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use std::{cell::RefCell, rc::Rc};

/// Independent body state and its native error subscription.
struct Mounted {
    /// Operands and stable image node.
    body: Rc<RefCell<Body>>,
    /// Listener released together with this placement.
    _image: DomRoot,
}
impl ImageArtifact {
    /// Build an original-byte download independently of image loading.
    /// # Errors
    /// Returns decoding, localization or native failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        header::download(view, &self.filename, self.mime(), &self.bytes()?)
    }
}
impl DomContent for ImageArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let body = Rc::new(RefCell::new(Body {
            svg: self.body.borrow().svg,
            ..Body::default()
        }));
        let image = super::browser::nodes(&view.document, &body)?;
        let root = DomRoot {
            node: image.node.clone(),
            owner: Some(Box::new(Mounted {
                body,
                _image: image,
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("Image body is not mounted".into()))?;
        let presentation = view
            .pack
            .presentation(view.mode)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        let mut body = mounted.body.borrow_mut();
        self.assign(
            &mut body,
            presentation
                .colors
                .get("muted")
                .copied()
                .unwrap_or_default(),
        );
        super::browser::sync(&body).map_err(|error| ToolUiError::native(&error))
    }
}
