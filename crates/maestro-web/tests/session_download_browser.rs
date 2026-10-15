//! Browser-executed assertions through the actual native byte-download owner.
#![cfg(all(test, target_arch = "wasm32"))]
use base64::Engine as _;
use maestro_web::browser::attachment_content::{
    AttachmentHost, AttachmentUiError, BrowserAttachmentHost,
};
use maestro_web::export_html::template::{download_session_json, load_document};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::*};

#[wasm_bindgen(inline_js = r#"
let active;
export function begin(mode) {
    if (active) throw new Error('unfinished browser transaction');
    const create = document.createElement;
    const append = document.body.appendChild;
    const body = document.body;
    const createUrl = URL.createObjectURL;
    const revoke = URL.revokeObjectURL;
    const state = { mode, blobs: [], anchors: [], live: new Set(), events: [] };
    URL.createObjectURL = function(blob) {
        const url = createUrl.call(this, blob);
        state.blobs.push(blob); state.live.add(url); state.events.push('create-url');
        return url;
    };
    URL.revokeObjectURL = function(url) {
        if (!state.live.delete(url)) throw new Error('wrong URL revoked');
        state.events.push('revoke-url'); return revoke.call(this, url);
    };
    document.createElement = function(tag, ...rest) {
        if (tag === 'a' && mode === 'create') throw new DOMException('controlled create', 'InvalidStateError');
        const element = create.call(this, tag, ...rest);
        if (tag === 'a') {
            state.anchors.push(element);
            element.addEventListener('click', event => {
                event.preventDefault();
                if (!state.live.has(element.href) || !element.isConnected) throw new Error('click without live resources');
                state.events.push('click');
                if (mode === 'removed') element.remove();
            });
        }
        return element;
    };
    body.appendChild = function(child) {
        if (mode === 'append') throw new DOMException('controlled append', 'HierarchyRequestError');
        state.events.push('append'); return append.call(this, child);
    };
    if (mode === 'body') Object.defineProperty(document, 'body', { configurable: true, get: () => null });
    state.restore = () => {
        URL.createObjectURL = createUrl; URL.revokeObjectURL = revoke;
        document.createElement = create; body.appendChild = append;
        if (mode === 'body') delete document.body;
        active = undefined;
    };
    active = state;
}
export async function finish(bytes, mime, name, success) {
    const state = active;
    try {
        if (state.blobs.length !== 1 || state.live.size !== 0) throw new Error('Blob/URL ownership');
        const blob = state.blobs[0];
        const actual = new Uint8Array(await blob.arrayBuffer());
        if (actual.length !== bytes.length || actual.some((b, i) => b !== bytes[i]) || blob.type !== mime) throw new Error('original bytes/MIME');
        for (const anchor of state.anchors) {
            if (anchor.download !== name || anchor.isConnected) throw new Error('literal name/anchor cleanup');
        }
        const expected = success ? ['create-url', 'append', 'click', 'revoke-url'] : ['create-url', 'revoke-url'];
        if (JSON.stringify(state.events) !== JSON.stringify(expected)) throw new Error('effect order: ' + state.events.join(','));
        console.log('PASS native download ' + state.mode + ': ' + state.events.join(' -> '));
    } finally { state.restore(); }
}
"#)]
extern "C" {
    /// Start one controlled native DOM transaction.
    fn begin(mode: &str);
    /// Observe the retained actual Blob and resources after synchronous return.
    fn finish(bytes: &[u8], mime: &str, name: &str, success: bool) -> js_sys::Promise;
}

/// Exercise both public routes, reached failures and idempotent anchor cleanup.
#[wasm_bindgen]
pub async fn session_and_attachment_downloads_share_native_cleanup() {
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
        begin(mode);
        let result = download_session_json(&document, &host);
        let success = matches!(mode, "success" | "removed");
        if success {
            result.unwrap();
        } else {
            assert!(matches!(result, Err(AttachmentUiError::Host(_))));
        }
        wasm_bindgen_futures::JsFuture::from(finish(
            expected.as_bytes(),
            "application/x-ndjson",
            "雪<room>.jsonl",
            success,
        ))
        .await
        .unwrap();
    }
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
    begin("attachment");
    host.download(Rc::new(RefCell::new(attachment))).unwrap();
    wasm_bindgen_futures::JsFuture::from(finish(
        &bytes,
        "application/octet-stream",
        "binary<雪>.dat",
        true,
    ))
    .await
    .unwrap();
}
