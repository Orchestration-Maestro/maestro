//! Browser-executed assertions through the actual native byte-download owner.
#![cfg(all(test, target_arch = "wasm32"))]
use base64::Engine as _;
use maestro_web::browser::attachment_content::{
    AttachmentHost, AttachmentHostError, BrowserAttachmentHost,
};
use maestro_web::export_html::template::{download_session_json, load_document};
use wasm_bindgen::{JsCast as _, prelude::*};

/// Exercise both public routes, reached failures and idempotent anchor cleanup.
#[wasm_bindgen]
pub async fn session_and_attachment_downloads_share_native_cleanup() {
    let fixture = Fixture::load().await;
    let canvas = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into()
        .unwrap();
    let host = BrowserAttachmentHost::new(canvas).unwrap();
    let document = load_document(
        &base64::engine::general_purpose::STANDARD
            .encode(r#"{"header":{"id":"雪<room>"},"entries":[{"type":"custom","data":"é"}]}"#),
        "",
        None,
    )
    .unwrap();
    let expected =
        "{\"type\":\"header\",\"id\":\"雪<room>\"}\n{\"type\":\"custom\",\"data\":\"é\"}";
    for mode in ["success", "create", "body", "append", "removed"] {
        fixture.begin(mode);
        let result = download_session_json(&document, &host);
        let success = matches!(mode, "success" | "removed");
        if success {
            result.unwrap();
        } else {
            assert!(result.is_err_and(|error| matches!(
                error.get_ref().and_then(|cause| cause.downcast_ref::<AttachmentHostError>()),
                Some(AttachmentHostError::Host(_))
            )));
        }
        wasm_bindgen_futures::JsFuture::from(fixture.finish(
            expected.as_bytes(),
            "application/x-ndjson",
            "雪<room>.jsonl",
            success,
        ))
        .await
        .unwrap();
    }
    attachment_download(&fixture, &host).await;
}

/// Native namespaces retained for one browser test transaction.
#[derive(Clone)]
pub struct Fixture {
    /// Original packaged native namespace.
    module0: crate::browser_fixture_support::FixtureModule,
}
/// Packaged URL of the download fixture module.
fn fixture_module() -> String {
    wasm_bindgen::link_to!(module = "/tests/session_download_support/browser.js")
}

impl Fixture {
    /// Load the namespace through a page's own module import, which its policy allows.
    pub async fn import(import: &js_sys::Function) -> Self {
        Self {
            module0: crate::browser_fixture_support::FixtureModule::import(
                import,
                &fixture_module(),
            )
            .await
            .unwrap(),
        }
    }
    /// Load each namespace before installing controlled host effects.
    pub async fn load() -> Self {
        Self {
            module0: crate::browser_fixture_support::FixtureModule::load(&fixture_module())
                .await
                .unwrap(),
        }
    }
    /// Activate `control` and observe one complete native download of `bytes`.
    pub async fn saved(
        &self,
        control: &web_sys::HtmlElement,
        bytes: &[u8],
        mime: &str,
        name: &str,
    ) {
        self.begin("success");
        control.click();
        wasm_bindgen_futures::JsFuture::from(self.finish(bytes, mime, name, true))
            .await
            .unwrap();
    }
    /// Start one controlled native DOM transaction.
    pub fn begin(&self, mode: &str) {
        self.module0
            .call("begin", &js_sys::Array::of1(&mode.into()))
            .unwrap();
    }
    /// Observe the retained actual Blob and resources after synchronous return.
    pub fn finish(&self, bytes: &[u8], mime: &str, name: &str, success: bool) -> js_sys::Promise {
        self.module0
            .call(
                "finish",
                &js_sys::Array::of4(
                    &js_sys::Uint8Array::from(bytes),
                    &mime.into(),
                    &name.into(),
                    &success.into(),
                ),
            )
            .unwrap()
            .unchecked_into()
    }
    /// Observe that an activation left the platform's download effects untouched.
    pub fn finish_none(&self) -> js_sys::Promise {
        self.module0
            .call("finishNone", &js_sys::Array::new())
            .unwrap()
            .unchecked_into()
    }
}

/// Observe the original attachment bytes through the same native cleanup fixture.
async fn attachment_download(fixture: &Fixture, host: &BrowserAttachmentHost) {
    let bytes = [0, 255, 128, 13, 10, 42];
    let attachment = maestro_web::Attachment {
        id: "binary".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "binary<雪>.dat".into(),
        mime_type: "application/octet-stream".into(),
        size: bytes.len(),
        content: base64::engine::general_purpose::STANDARD.encode(bytes),
        extracted_text: Some("not original".into()),
        preview: None,
    };
    fixture.begin("attachment");
    host.download(std::sync::Arc::new(std::sync::RwLock::new(attachment)))
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(fixture.finish(
        &bytes,
        "application/octet-stream",
        "binary<雪>.dat",
        true,
    ))
    .await
    .unwrap();
}
