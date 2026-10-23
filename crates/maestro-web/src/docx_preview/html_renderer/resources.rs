//! Package resources a rendering names: images, picture bullets, fonts and alternate chunks.
use super::HtmlRenderer;
use crate::docx_preview::PartRecords;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

impl HtmlRenderer<'_> {
    /// Load an image against the part being rendered, the document part by default.
    pub(super) fn load_document_image(&mut self, id: &str) -> Option<String> {
        let part = self
            .current_part
            .or_else(|| self.preview.document.document_part())?;
        let bytes = self.preview.document.load_resource(part, id).ok()??;
        self.blob_to_url(&bytes)
    }

    /// Load a picture bullet's image against the numbering part.
    pub(super) fn load_numbering_image<T>(
        &mut self,
        numbering: &PartRecords<T>,
        id: Option<&str>,
    ) -> Option<String> {
        let part = self.preview.part(Some(numbering))?;
        let bytes = self.preview.document.load_resource(part, id?).ok()??;
        self.blob_to_url(&bytes)
    }

    /// Load an embedded font against the font table part, de-obfuscated with its key.
    pub(super) fn load_font<T>(
        &mut self,
        fonts: &PartRecords<T>,
        id: Option<&str>,
        key: Option<&str>,
    ) -> Option<String> {
        let part = self.preview.part(Some(fonts))?;
        let bytes = self.preview.document.load_font(part, id?, key).ok()??;
        self.blob_to_url(&bytes)
    }

    /// Load an alternate format chunk's text against the part being rendered.
    pub(super) fn load_alt_chunk(&self, id: &str) -> Option<String> {
        let part = self
            .current_part
            .or_else(|| self.preview.document.document_part())?;
        self.preview.document.load_alt_chunk(part, id).ok()?
    }

    /// Name loaded bytes by data URL or, when the package was read with base64 URLs, by an
    /// object URL the rendering keeps. The URL kind follows the options the package was read
    /// with, which the resource loader keeps, never the options of a later rendering.
    fn blob_to_url(&mut self, bytes: &[u8]) -> Option<String> {
        if self.preview.options.resources.use_base64_url {
            return Some(format!(
                "data:application/octet-stream;base64,{}",
                STANDARD.encode(bytes)
            ));
        }
        object_url(self, bytes)
    }
}

/// Create an object URL the rendering keeps until it is dropped.
#[cfg(target_arch = "wasm32")]
fn object_url(renderer: &mut HtmlRenderer<'_>, bytes: &[u8]) -> Option<String> {
    let url = crate::utils::download::ObjectUrl::new(bytes, "").ok()?;
    let text = url.as_str().to_owned();
    renderer.object_urls.push(url);
    Some(text)
}

/// Leave the reference unset: a build without a browser has no object URLs.
#[cfg(not(target_arch = "wasm32"))]
fn object_url(_renderer: &mut HtmlRenderer<'_>, _bytes: &[u8]) -> Option<String> {
    None
}
