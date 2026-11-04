//! Browser-executed assertions through the actual native byte-download owner.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::browser_fixture_support::{Patch, dom_exception, error, get, global};
use base64::Engine as _;
use maestro_web::browser::attachment_content::{
    AttachmentHost, AttachmentHostError, BrowserAttachmentHost,
};
use maestro_web::export_html::template::{download_session_json, load_document};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::*};

/// Exercise both public routes, reached failures and idempotent anchor cleanup.
#[wasm_bindgen]
pub async fn session_and_attachment_downloads_share_native_cleanup() {
    let fixture = Fixture::default();
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
        fixture
            .finish(
                expected.as_bytes(),
                "application/x-ndjson",
                "雪<room>.jsonl",
                success,
            )
            .await;
    }
    attachment_download(&fixture, &host).await;
}

/// One controlled native download transaction at a time, recording the platform effects.
#[derive(Clone, Default)]
pub struct Fixture {
    /// The running transaction, if any.
    active: Rc<RefCell<Option<Transaction>>>,
}

/// An anchor click observer, which may refuse a click without live resources.
type ClickObserver = Closure<dyn Fn(web_sys::Event) -> Result<(), JsValue>>;

/// Effects one download transaction observed, in order.
#[derive(Default)]
struct Effects {
    /// Blobs given an object URL.
    blobs: Vec<web_sys::Blob>,
    /// Object URLs not yet revoked.
    live: Vec<String>,
    /// Created anchors.
    anchors: Vec<web_sys::HtmlAnchorElement>,
    /// Effect names in order.
    events: Vec<&'static str>,
    /// Anchor click observers, kept alive for the transaction.
    clicks: Vec<ClickObserver>,
}

/// A running transaction: its mode, its effects and the replaced page operations.
struct Transaction {
    /// The refused or altered step, or an ordinary mode.
    mode: String,
    /// Effects observed so far.
    effects: Rc<RefCell<Effects>>,
    /// Replaced page operations, restored when the transaction ends.
    _patches: Vec<Patch>,
}

impl Fixture {
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
        self.finish(bytes, mime, name, true).await;
    }
    /// Start one controlled native DOM transaction.
    pub fn begin(&self, mode: &str) {
        assert!(
            self.active.borrow().is_none(),
            "unfinished browser transaction"
        );
        let effects: Rc<RefCell<Effects>> = Rc::default();
        let document = global("document");
        let url = global("URL");
        let mut patches = vec![
            object_urls(&url, &effects),
            revocations(&url, &effects),
            anchors(&document, mode, &effects),
            appends(&get(&document, "body"), mode, &effects),
        ];
        if mode == "body" {
            patches.push(Patch::getter(&document, "body", |_| Ok(JsValue::NULL)));
        }
        *self.active.borrow_mut() = Some(Transaction {
            mode: mode.to_owned(),
            effects,
            _patches: patches,
        });
    }
    /// End the transaction, restoring the page and returning its mode and effects.
    fn end(&self) -> (String, Effects) {
        let transaction = self.active.borrow_mut().take().unwrap();
        let effects = std::mem::take(&mut *transaction.effects.borrow_mut());
        (transaction.mode, effects)
    }
    /// Observe the retained actual Blob and resources after synchronous return.
    pub async fn finish(&self, bytes: &[u8], mime: &str, name: &str, success: bool) {
        let (mode, effects) = self.end();
        assert!(
            effects.blobs.len() == 1 && effects.live.is_empty(),
            "Blob/URL ownership"
        );
        let blob = &effects.blobs[0];
        let actual = wasm_bindgen_futures::JsFuture::from(blob.array_buffer())
            .await
            .unwrap();
        let actual = js_sys::Uint8Array::new(&actual).to_vec();
        assert!(
            actual == bytes && blob.type_() == mime,
            "original bytes/MIME"
        );
        for anchor in &effects.anchors {
            assert!(
                anchor.download() == name && !anchor.is_connected(),
                "literal name/anchor cleanup"
            );
        }
        let expected: &[&str] = if success {
            &["create-url", "append", "click", "revoke-url"]
        } else {
            &["create-url", "revoke-url"]
        };
        assert_eq!(
            effects.events,
            expected,
            "effect order: {}",
            effects.events.join(",")
        );
        let order = effects.events.join(" -> ");
        web_sys::console::log_1(&format!("PASS native download {mode}: {order}").into());
    }
    /// Observe that an activation left the platform's download effects untouched.
    pub fn finish_none(&self) {
        let (mode, effects) = self.end();
        assert!(
            effects.blobs.is_empty() && effects.anchors.is_empty() && effects.events.is_empty(),
            "unexpected download effects: {} ({} Blobs)",
            effects.events.join(","),
            effects.blobs.len()
        );
        web_sys::console::log_1(&format!("PASS native download {mode}: none").into());
    }
}

/// Record each object URL the page creates.
fn object_urls(url: &JsValue, effects: &Rc<RefCell<Effects>>) -> Patch {
    let effects = Rc::clone(effects);
    Patch::method(url, "createObjectURL", move |original, this, args| {
        let created = original.apply(&this, &args)?;
        let mut effects = effects.borrow_mut();
        effects.blobs.push(args.get(0).unchecked_into());
        effects.live.push(created.as_string().unwrap());
        effects.events.push("create-url");
        Ok(created)
    })
}

/// Require each revocation to name a live object URL.
fn revocations(url: &JsValue, effects: &Rc<RefCell<Effects>>) -> Patch {
    let effects = Rc::clone(effects);
    Patch::method(url, "revokeObjectURL", move |original, this, args| {
        let revoked = args.get(0).as_string();
        {
            let mut effects = effects.borrow_mut();
            let position = effects
                .live
                .iter()
                .position(|live| Some(live) == revoked.as_ref());
            let Some(position) = position else {
                return Err(error("wrong URL revoked"));
            };
            effects.live.remove(position);
            effects.events.push("revoke-url");
        }
        original.apply(&this, &args)
    })
}

/// Record created anchors and their clicks, refusing creation in `create` mode.
fn anchors(document: &JsValue, mode: &str, effects: &Rc<RefCell<Effects>>) -> Patch {
    let (mode, effects) = (mode.to_owned(), Rc::clone(effects));
    Patch::method(document, "createElement", move |original, this, args| {
        let anchor = args.get(0) == "a";
        if anchor && mode == "create" {
            return Err(dom_exception("controlled create", "InvalidStateError"));
        }
        let element = original.apply(&this, &args)?;
        if anchor {
            let anchor: web_sys::HtmlAnchorElement = element.clone().unchecked_into();
            let click = clicks(&anchor, mode == "removed", &effects);
            anchor
                .add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
                .unwrap();
            let mut effects = effects.borrow_mut();
            effects.anchors.push(anchor);
            effects.clicks.push(click);
        }
        Ok(element)
    })
}

/// Observe one anchor click without navigating, requiring its live resources.
fn clicks(
    anchor: &web_sys::HtmlAnchorElement,
    remove: bool,
    effects: &Rc<RefCell<Effects>>,
) -> ClickObserver {
    let (anchor, effects) = (anchor.clone(), Rc::clone(effects));
    Closure::new(move |event: web_sys::Event| {
        event.prevent_default();
        let live = effects.borrow().live.contains(&anchor.href());
        if !live || !anchor.is_connected() {
            return Err(error("click without live resources"));
        }
        effects.borrow_mut().events.push("click");
        if remove {
            anchor.remove();
        }
        Ok(())
    })
}

/// Record each append to the body, refusing it in `append` mode.
fn appends(body: &JsValue, mode: &str, effects: &Rc<RefCell<Effects>>) -> Patch {
    let (refuse, effects) = (mode == "append", Rc::clone(effects));
    Patch::method(body, "appendChild", move |original, this, args| {
        if refuse {
            return Err(dom_exception("controlled append", "HierarchyRequestError"));
        }
        effects.borrow_mut().events.push("append");
        original.apply(&this, &args)
    })
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
    fixture
        .finish(&bytes, "application/octet-stream", "binary<雪>.dat", true)
        .await;
}
