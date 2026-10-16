//! Public preparation and production-mounted frame witnesses.
use super::{Provider, adapter, finish};
use maestro_web::components::sandboxed_iframe::PrepareHtmlOptions;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/tests/sandbox_loading_support/programs.js")]
extern "C" {
    /// Run the exact-text, ordering and policy observations on a production frame's attributes.
    fn program_checks(html: &str, tt_html: &str, url: bool) -> js_sys::Promise;
    /// Observe literal payload preservation and mounted completion.
    fn marker_checks(html: &str) -> js_sys::Promise;
    /// Click parsed anchors through the browser driver and observe native routing.
    fn link_checks(html: &str) -> js_sys::Promise;
    /// Controlled HTTP bootstrap implementing the normal readiness handshake.
    fn program_url() -> String;
    /// Authored document with a policy and a parser-time observation.
    fn artifact() -> String;
    /// Observe the authored policy in the frame loaded by the public adapter.
    fn loaded_artifact(url: bool) -> js_sys::Promise;
    /// Provider and module input used in the mounted exactness observations.
    fn program_input(kind: &str) -> String;
}

/// Prepare through the public interface and take frame attributes from actual loading.
pub async fn program_witness() {
    let mut sandbox = adapter();
    let providers = vec![Arc::new(Provider {
        data: js_sys::JSON::parse(&serde_json::json!({"obsData": program_input("raw")}).to_string())
            .unwrap(),
        runtime: program_input("provider"),
        failure: None,
    }) as Arc<dyn maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider<super::BrowserSandboxValue>>];
    let html = sandbox
        .prepare_html_document("program", &program_input("user"), &providers, None)
        .unwrap();
    let tt_provider = super::provider(
        &program_input("provider").replace("if(location.hash==='#tt')", "if(true)"),
    );
    let tt_html = sandbox
        .prepare_html_document("policy", &program_input("user"), &[tt_provider], None)
        .unwrap();
    for url in [false, true] {
        sandbox.sandbox_url_provider = url.then(|| Arc::new(|| Ok(program_url())) as _);
        sandbox
            .load_content("prototype", "<head></head>", vec![], vec![])
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(program_checks(&html, &tt_html, url))
            .await
            .unwrap();
    }
    sandbox.remove().unwrap();
    finish();
}

/// The prepared navigation runtime runs in a parsed opaque document.
pub async fn link_witness() {
    let mut sandbox = adapter();
    let html = sandbox
        .prepare_html_document(
            "links",
            &program_input("click-document"),
            &[],
            Some(PrepareHtmlOptions {
                is_html_artifact: true,
                is_standalone: false,
            }),
        )
        .unwrap();
    sandbox
        .load_content("prototype", "<head></head>", vec![], vec![])
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(link_checks(&html))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    finish();
}

/// All authored insertion points retain head policies on both public loading paths.
pub async fn artifact_witness() {
    let mut sandbox = adapter();
    for url in [false, true] {
        sandbox.sandbox_url_provider = url.then(|| Arc::new(|| Ok(program_url())) as _);
        for position in ["head", "html", "fragment"] {
            let mut code = artifact();
            if position != "head" {
                code = code.replace("<head>", "").replace("</head>", "");
            }
            if position == "fragment" {
                code = code.replace("<html>", "").replace("</html>", "");
            }
            sandbox
                .load_content(
                    "artifact",
                    &code,
                    vec![super::provider(&program_input("provider"))],
                    vec![],
                )
                .unwrap();
            wasm_bindgen_futures::JsFuture::from(loaded_artifact(url))
                .await
                .unwrap();
        }
    }
    sandbox.remove().unwrap();
    finish();
}

/// Marker-looking identifiers, provider records and programs remain literal data.
pub async fn marker_witness() {
    let mut sandbox = adapter();
    let marker = "__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__";
    let providers = vec![Arc::new(Provider {
        data: js_sys::JSON::parse(&serde_json::json!({marker: marker, "attachment": {"text": marker}}).to_string()).unwrap(),
        runtime: program_input("marker-provider"),
        failure: None,
    }) as Arc<dyn maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider<super::BrowserSandboxValue>>];
    let html = sandbox
        .prepare_html_document(marker, &program_input("marker-user"), &providers, None)
        .unwrap();
    sandbox
        .load_content("prototype", "<head></head>", vec![], vec![])
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(marker_checks(&html))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    finish();
}
