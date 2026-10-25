//! Public preparation and production-mounted frame witnesses.
use super::{Provider, adapter};
use maestro_app::SharedHandle;
use maestro_web::components::sandboxed_iframe::PrepareHtmlOptions;
use wasm_bindgen::prelude::*;

/// Prepare through the public interface and take frame attributes from actual loading.
pub(crate) async fn program_witness() {
    let fixture = Fixture::load().await;
    let loading = super::Fixture::load().await;
    let sandbox = adapter();
    let providers = vec![SharedHandle::new(Provider {
        fixture: loading.clone(),
        data: js_sys::JSON::parse(&serde_json::json!({"obsData": fixture.program_input("raw")}).to_string())
            .unwrap(),
        runtime: fixture.program_input("provider"),
        failure: None,
    }) as SharedHandle<dyn maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider<super::BrowserSandboxValue>>];
    let html = sandbox
        .prepare_html_document("program", &fixture.program_input("user"), &providers, None)
        .unwrap();
    let tt_provider = super::provider(
        &loading,
        &fixture
            .program_input("provider")
            .replace("if(location.hash==='#tt')", "if(true)"),
    );
    let tt_html = sandbox
        .prepare_html_document(
            "policy",
            &fixture.program_input("user"),
            &[tt_provider],
            None,
        )
        .unwrap();
    for url in [false, true] {
        sandbox.sandbox_url_provider.replace(url.then(|| {
            SharedHandle::new({
                let fixture = fixture.clone();
                move || Ok(fixture.program_url())
            }) as _
        }));
        sandbox
            .load_content("prototype", "<head></head>", vec![], vec![])
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(fixture.program_checks(&html, &tt_html, url))
            .await
            .unwrap();
    }
    sandbox.remove().unwrap();
    loading.finish();
}

/// The prepared navigation runtime runs in a parsed opaque document.
pub(crate) async fn link_witness() {
    let fixture = Fixture::load().await;
    let loading = super::Fixture::load().await;
    let sandbox = adapter();
    let html = sandbox
        .prepare_html_document(
            "links",
            &fixture.program_input("click-document"),
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
    wasm_bindgen_futures::JsFuture::from(fixture.link_checks(&html))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    loading.finish();
}

/// All authored insertion points retain head policies on both public loading paths.
pub(crate) async fn artifact_witness() {
    let fixture = Fixture::load().await;
    let loading = super::Fixture::load().await;
    let sandbox = adapter();
    for url in [false, true] {
        sandbox.sandbox_url_provider.replace(url.then(|| {
            SharedHandle::new({
                let fixture = fixture.clone();
                move || Ok(fixture.program_url())
            }) as _
        }));
        for position in ["head", "html", "fragment"] {
            let mut code = fixture.artifact();
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
                    vec![super::provider(
                        &loading,
                        &fixture.program_input("provider"),
                    )],
                    vec![],
                )
                .unwrap();
            wasm_bindgen_futures::JsFuture::from(fixture.loaded_artifact(url))
                .await
                .unwrap();
        }
    }
    sandbox.remove().unwrap();
    loading.finish();
}

/// Marker-looking identifiers, provider records and programs remain literal data.
pub(crate) async fn marker_witness() {
    let fixture = Fixture::load().await;
    let loading = super::Fixture::load().await;
    let sandbox = adapter();
    let marker = "__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__";
    let providers = vec![SharedHandle::new(Provider {
        fixture: loading.clone(),
        data: js_sys::JSON::parse(&serde_json::json!({marker: marker, "attachment": {"text": marker}}).to_string()).unwrap(),
        runtime: fixture.program_input("marker-provider"),
        failure: None,
    }) as SharedHandle<dyn maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider<super::BrowserSandboxValue>>];
    let html = sandbox
        .prepare_html_document(
            marker,
            &fixture.program_input("marker-user"),
            &providers,
            None,
        )
        .unwrap();
    sandbox
        .load_content("prototype", "<head></head>", vec![], vec![])
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(fixture.marker_checks(&html))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    loading.finish();
}

/// Native namespaces retained for one browser test transaction.
#[derive(Clone)]
pub struct Fixture {
    /// Original packaged native namespace.
    module0: crate::browser_fixture_support::FixtureModule,
}
impl Fixture {
    /// Load each namespace before installing controlled host effects.
    pub(crate) async fn load() -> Self {
        Self {
            module0: crate::browser_fixture_support::FixtureModule::load(&wasm_bindgen::link_to!(
                module = "/tests/sandbox_loading_support/programs.js"
            ))
            .await
            .unwrap(),
        }
    }
    /// Run the exact-text, ordering and policy observations on a production frame's attributes.
    pub(crate) fn program_checks(&self, html: &str, tt_html: &str, url: bool) -> js_sys::Promise {
        self.module0
            .call(
                "program_checks",
                &js_sys::Array::of3(&html.into(), &tt_html.into(), &url.into()),
            )
            .unwrap()
            .unchecked_into()
    }
    /// Observe literal payload preservation and mounted completion.
    pub(crate) fn marker_checks(&self, html: &str) -> js_sys::Promise {
        self.module0
            .call("marker_checks", &js_sys::Array::of1(&html.into()))
            .unwrap()
            .unchecked_into()
    }
    /// Click parsed anchors through the browser driver and observe native routing.
    pub(crate) fn link_checks(&self, html: &str) -> js_sys::Promise {
        self.module0
            .call("link_checks", &js_sys::Array::of1(&html.into()))
            .unwrap()
            .unchecked_into()
    }
    /// Controlled HTTP bootstrap implementing the normal readiness handshake.
    #[must_use]
    pub(crate) fn program_url(&self) -> String {
        self.module0
            .call("program_url", &js_sys::Array::new())
            .unwrap()
            .as_string()
            .unwrap()
    }
    /// Authored document with a policy and a parser-time observation.
    #[must_use]
    pub(crate) fn artifact(&self) -> String {
        self.module0
            .call("artifact", &js_sys::Array::new())
            .unwrap()
            .as_string()
            .unwrap()
    }
    /// Observe the authored policy in the frame loaded by the public adapter.
    pub(crate) fn loaded_artifact(&self, url: bool) -> js_sys::Promise {
        self.module0
            .call("loaded_artifact", &js_sys::Array::of1(&url.into()))
            .unwrap()
            .unchecked_into()
    }
    /// Provider and module input used in the mounted exactness observations.
    #[must_use]
    pub(crate) fn program_input(&self, kind: &str) -> String {
        self.module0
            .call("program_input", &js_sys::Array::of1(&kind.into()))
            .unwrap()
            .as_string()
            .unwrap()
    }
}
