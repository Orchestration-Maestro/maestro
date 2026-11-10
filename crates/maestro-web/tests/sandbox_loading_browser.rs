//! Actual browser document, event-source and subscription lifetime witnesses.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::sandbox_bootstrap_support::loader_document;
use crate::sandbox_loading_support;
use maestro_app::SharedHandle;
use maestro_app::browser::components::sandbox::providers::SandboxRuntimeProvider;
use maestro_web::browser::values::BrowserSandboxValue;
use sandbox_loading_support::*;

/// Native values retain their stringify semantics at the public preparation boundary.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn prepared_documents_keep_native_data_and_failures() {
    let fixture = Fixture::default();
    let sandbox = adapter();
    for row in document_cases(&fixture) {
        let prepared =
            sandbox.prepare_html_document(&row.id, &row.code, &row.providers, Some(row.options));
        if let Some(category) = js_sys::Reflect::get(&row.expected, &"error_category".into())
            .unwrap()
            .as_string()
        {
            let error = prepared.unwrap_err();
            assert_eq!(error.name.as_deref().unwrap_or("Error"), category);
        } else {
            Fixture::observe_document(&row.name, &prepared.unwrap(), &row.expected).await;
        }
    }
    sandbox.remove().unwrap();
    Fixture::finish();
}

/// Download preparation keeps provider helpers without installing online routes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn standalone_runtime_keeps_helpers_without_parent_messaging() {
    let fixture = Fixture::default();
    let sandbox = adapter();
    let html = sandbox
        .prepare_html_document(
            "offline",
            "<body>offline</body>",
            &[provider(
                &fixture,
                "function(id){window.localHelper=()=>id;}",
            )],
            Some(
                maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
                    is_html_artifact: true,
                    is_standalone: true,
                },
            ),
        )
        .unwrap();
    Fixture::standalone(&html).await;
    sandbox.remove().unwrap();
    Fixture::finish();
}

/// Every built-in helper set installs, and none leaves a compiler-reserved global behind.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn builtin_helpers_leave_the_capture_namespace_unused() {
    use maestro_app::BuiltinRuntime::{Artifacts, Attachments, Console, FileDownload};
    let sandbox = adapter();
    let providers: Vec<_> = [Console, Attachments, FileDownload, Artifacts]
        .map(|kind| {
            SharedHandle::new(Builtin(kind))
                as SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>
        })
        .into();
    let html = sandbox
        .prepare_html_document(
            "builtins",
            "<head></head>",
            &providers,
            Some(
                maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
                    is_html_artifact: true,
                    is_standalone: false,
                },
            ),
        )
        .unwrap();
    let observed = Fixture::defined(
        &html,
        "^__maestro[0-9]+$",
        &[
            "complete",
            "listAttachments",
            "returnDownloadableFile",
            "getArtifact",
        ],
    )
    .await;
    assert_eq!(
        js_sys::JSON::stringify(&observed)
            .unwrap()
            .as_string()
            .unwrap(),
        r#"[[],["function","function","function","function"]]"#
    );
    sandbox.remove().unwrap();
    Fixture::finish();
}

/// Completion waits for callbacks without replacing a user's result with callback failure.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn repl_completion_callbacks_preserve_order_and_errors() {
    let sandbox = adapter();
    for row in fixture_group("callbacks_") {
        let code = if row.input["outcome"] == "success" {
            "return 9;"
        } else {
            "throw new Error('user failure');"
        };
        let html = sandbox
            .prepare_html_document("callbacks", code, &[], None)
            .unwrap();
        Fixture::callbacks(&html, &row.input, &row.output).await;
    }
    sandbox.remove().unwrap();
    Fixture::finish();
}

/// The actual mounted frame runs the injected console before supplied runtimes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn srcdoc_load_injects_console_before_supplied_providers() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    fixture.arm("loaded");
    sandbox
        .load_content(
            "srcdoc",
            &Fixture::content(),
            vec![
                provider(
                    &fixture,
                    "function(){window.firstProvider=typeof window.complete}",
                ),
                provider(
                    &fixture,
                    "function(){window.secondProvider=window.firstProvider}",
                ),
            ],
            vec![maestro_app::SharedHandle::new(Consumer(fixture.clone()))],
        )
        .unwrap();
    let loaded = fixture.waited().await;
    for name in ["first", "second"] {
        assert_eq!(
            js_sys::Reflect::get(&loaded, &name.into())
                .unwrap()
                .as_string()
                .as_deref(),
            Some("function")
        );
    }
    fixture.route_check("srcdoc").await;
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Reload retains one native listener and explicit close revokes the actual callback.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn external_links_dispatch_once_across_reload_and_close() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    for _ in 0..2 {
        fixture.arm("loaded");
        sandbox
            .load_content("links", &Fixture::content(), vec![], vec![])
            .unwrap();
        fixture.waited().await;
        fixture.route_check("click").await;
    }
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Current frame identity is read live after replacement and cannot be forged by another window.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn unrelated_or_stale_frames_cannot_open_urls() {
    let fixture = Fixture::default();
    fixture.begin();
    fixture.fault("sibling");
    let sandbox = adapter();
    for id in ["old", "new"] {
        fixture.arm("loaded");
        sandbox
            .load_content(id, &Fixture::content(), vec![], vec![])
            .unwrap();
        fixture.waited().await;
        fixture.dispatch("remember", "current");
    }
    fixture.route_check("unrelated").await;
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// The URL callback is read again on reload; only actual readiness forwards the document.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn url_load_uses_live_configuration_and_ready_handshake() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    for mode in ["first", "replacement"] {
        let expected = Fixture::url(mode);
        sandbox
            .sandbox_url_provider
            .replace(Some(maestro_app::SharedHandle::new(move || {
                Ok(expected.clone())
            })));
        fixture.arm("url-mounted");
        sandbox
            .load_content(
                &format!("url-{mode}"),
                "<body>url branch</body>",
                vec![],
                vec![],
            )
            .unwrap();
        let mounted = fixture.waited().await;
        assert_eq!(
            js_sys::Reflect::get(&mounted, &"mode".into())
                .unwrap()
                .as_string()
                .as_deref(),
            Some(mode)
        );
        fixture.route_check("ready").await;
        fixture.route_check("reset-opens").await;
    }
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Bootstrap errors use the registered consumer route without admitting a parent-origin forgery.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn url_error_reaches_registered_consumers_without_source_bypass() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    sandbox
        .sandbox_url_provider
        .replace(Some(maestro_app::SharedHandle::new({
            move || Ok(Fixture::url("error"))
        })));
    fixture.arm("url-mounted");
    sandbox
        .load_content(
            "url-error",
            "<body>url branch</body>",
            vec![provider(&fixture, "function(){}")],
            vec![maestro_app::SharedHandle::new(Consumer(fixture.clone()))],
        )
        .unwrap();
    fixture.waited().await;
    fixture.route_check("error").await;
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Tabs are used only for URL loading, with ordinary window fallback otherwise.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn extension_links_use_tabs_then_window_fallback() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    fixture.fault("tabs");
    sandbox
        .sandbox_url_provider
        .replace(Some(maestro_app::SharedHandle::new({
            move || Ok(Fixture::url("tabs"))
        })));
    fixture.arm("url-mounted");
    sandbox
        .load_content("tabs", "<body>url branch</body>", vec![], vec![])
        .unwrap();
    fixture.waited().await;
    fixture.route_check("tabs").await;
    sandbox.sandbox_url_provider.replace(None);
    fixture.arm("loaded");
    sandbox
        .load_content("tabs-srcdoc", &Fixture::content(), vec![], vec![])
        .unwrap();
    fixture.waited().await;
    fixture.route_check("click").await;
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Navigation decisions use real resolved anchors and form actions, including nested targets.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn navigation_intercepts_resolved_links_and_forms() {
    let sandbox = adapter();
    let html = sandbox
        .prepare_html_document(
            "navigation",
            "<head></head>",
            &[],
            Some(
                maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
                    is_html_artifact: true,
                    is_standalone: false,
                },
            ),
        )
        .unwrap();
    for row in fixture_group("navigation_") {
        Fixture::navigation(&html, &row.input, &row.output).await;
    }
    sandbox.remove().unwrap();
    Fixture::finish();
}

/// Native parser recovery and reached parser failures select the exact fallback text.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn validation_uses_native_parser_and_fallback_messages() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    for (html, mode, expected) in [
        ("<html><body>valid</body></html>", "", ""),
        ("<div><p>unclosed", "", ""),
        (
            "<parsererror>fixture parse problem</parsererror>",
            "",
            "fixture parse problem",
        ),
        ("<parsererror></parsererror>", "", "Unknown parse error"),
        ("x", "parse-throw", "controlled parse failure"),
        ("x", "parse-no-message", "Unknown validation error"),
    ] {
        fixture.fault(mode);
        sandbox
            .load_content("validation", html, vec![], vec![])
            .unwrap();
        fixture.fault("");
        fixture.validation_check(expected, "").await;
    }
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// The literal error remains inert and uses each independently selected pack and mode.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn validation_document_keeps_literal_text_and_selected_pack() {
    let fixture = Fixture::default();
    fixture.begin();
    let literal = "literal <tag> & <script>window.executed=true</script>";
    for alternate in [false, true] {
        for light in [false, true] {
            let (sandbox, css) = adapter_pack(
                alternate,
                if light {
                    maestro_theme::BrandMode::Light
                } else {
                    maestro_theme::BrandMode::Dark
                },
            );
            sandbox
                .load_content("replace", "<body>previous</body>", vec![], vec![])
                .unwrap();
            sandbox.load_content("invalid", "<parsererror>literal &lt;tag&gt; &amp; &lt;script&gt;window.executed=true&lt;/script&gt;</parsererror>", vec![], vec![]).unwrap();
            fixture.validation_check(literal, &css).await;
            sandbox.remove().unwrap();
            fixture.closed(0);
            Fixture::finish();
        }
    }
    fixture.end();
}

/// Every URL settlement phase and final drop revoke the same native registration.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn close_removes_ready_error_and_external_routes() {
    let fixture = Fixture::default();
    fixture.begin();
    for phase in ["pending", "ready", "error", "invalid", "drop", "retry"] {
        let sandbox = adapter();
        sandbox
            .sandbox_url_provider
            .replace(Some(maestro_app::SharedHandle::new({
                move || Ok(Fixture::url("close"))
            })));
        if phase == "invalid" {
            sandbox
                .load_content("closing", "<parsererror>bad</parsererror>", vec![], vec![])
                .unwrap();
            fixture.validation_check("bad", "").await;
        } else {
            fixture.arm("url-mounted");
            sandbox
                .load_content("closing", "<body>closing</body>", vec![], vec![])
                .unwrap();
            fixture.waited().await;
            if phase == "ready" || phase == "error" {
                let event = match phase {
                    "ready" => "sandbox-ready",
                    _ => "sandbox-error",
                };
                fixture.dispatch(event, "current");
            }
        }
        if phase == "retry" {
            fixture.fault("detach");
            assert!(sandbox.remove().is_err());
            fixture.closed(2);
            fixture.fault("");
        }
        if phase != "drop" {
            sandbox.remove().unwrap();
            sandbox.remove().unwrap();
        }
        drop(sandbox);
        fixture.closed(0);
        Fixture::finish();
    }
    fixture.end();
}

/// Closing either registered sibling leaves the other's router and opener live.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn removing_one_sandbox_preserves_its_sibling() {
    let fixture = Fixture::default();
    fixture.begin();
    for first in [0, 1] {
        let mut sandboxes = [adapter(), adapter()];
        for (index, sandbox) in sandboxes.iter_mut().enumerate() {
            fixture.arm("loaded");
            sandbox
                .load_content(
                    &format!("sibling-{index}"),
                    &Fixture::content(),
                    vec![],
                    vec![maestro_app::SharedHandle::new(Consumer(fixture.clone()))],
                )
                .unwrap();
            fixture.waited().await;
        }
        sandboxes[first].remove().unwrap();
        fixture.closed(2);
        fixture.route_check("click").await;
        fixture.route_check("sibling-probe").await;
        sandboxes[1 - first].remove().unwrap();
        fixture.closed(0);
        Fixture::finish();
    }
    fixture.end();
}

/// Reached native/provider failures release acquired resources and permit an ordinary next load.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn failed_load_releases_native_registration_and_frame() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    for mode in [
        "url",
        "data",
        "runtime",
        "serializer",
        "register",
        "register-adapter",
        "create",
        "append",
    ] {
        fixture.fault(mode);
        let providers = failing_providers(&fixture, mode);
        sandbox
            .sandbox_url_provider
            .replace((mode == "url").then(|| {
                maestro_app::SharedHandle::new(|| {
                    Err(maestro_app::DiagnosticErrorInfo::error("URL failure"))
                })
                    as maestro_app::browser::components::sandboxed_iframe::SandboxUrlProvider
            }));
        let error = sandbox
            .load_content("failed", "<body>attempt</body>", providers, vec![])
            .unwrap_err();
        if mode == "url" {
            assert_eq!(error.message, "URL failure");
        }
        if mode == "data" || mode == "runtime" {
            assert_eq!(error.message, "provider failed");
        }
        fixture.fault("");
        fixture.route_check("empty").await;
        sandbox.sandbox_url_provider.replace(None);
        fixture.arm("loaded");
        sandbox
            .load_content("recovered", &Fixture::content(), vec![], vec![])
            .unwrap();
        fixture.waited().await;
        sandbox.remove().unwrap();
        fixture.closed(0);
    }
    fixture.end();
    Fixture::finish();
}

/// Inline programs keep exact text, head semantics, policies and module ordering.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn inline_programs_preserve_text_policies_and_order() {
    sandbox_loading_support::programs::program_witness().await;
}

/// Real pointer clicks follow native HTML and animated vector hyperlink state.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn effective_links_follow_native_hyperlink_state() {
    sandbox_loading_support::programs::link_witness().await;
}

/// Authored policies remain in the head at every runtime insertion point.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn html_artifacts_keep_head_policies_at_each_insertion_point() {
    sandbox_loading_support::programs::artifact_witness().await;
}

/// Public preparation preserves marker-looking payloads through mounted completion.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn marker_looking_payloads_survive_preparation_and_completion() {
    sandbox_loading_support::programs::marker_witness().await;
}

/// Select the packaged writer page for URL loads, or srcdoc.
fn writer(sandbox: &maestro_web::SandboxIframe, url: bool) {
    let provider = url.then(crate::contributions_support::sandbox_url);
    drop(sandbox.sandbox_url_provider.replace(provider));
}

/// Head script that reports the document has started, ahead of the loader.
const STARTING: &str = "parent.postMessage({type:'bootstrap-starting'},'*')";

/// Removal at each point of the runtime's startup settles the host once.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_removal_during_startup_settles_once() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    // Before the host reads readiness, after it replies, before the runtime and after writing.
    // For `url-pending` the fixture arms before loading, so its listener runs ahead of the host's.
    for (phase, signal) in [
        ("url-pending", "sandbox-ready"),
        ("url-replied", "sandbox-ready"),
        ("srcdoc-starting", "bootstrap-starting"),
        ("url-installed", "sandbox-installed"),
    ] {
        let url = phase.starts_with("url");
        writer(&sandbox, url);
        let html = loader_document(STARTING, "");
        if phase == "url-pending" {
            fixture.arm(signal);
        }
        sandbox.load_content(phase, &html, vec![], vec![]).unwrap();
        if phase != "url-pending" {
            fixture.arm(signal);
        }
        fixture.waited().await;
        sandbox.remove().unwrap();
        sandbox.remove().unwrap();
        fixture.route_check("empty").await;
    }
    writer(&sandbox, false);
    fixture.arm("loaded");
    sandbox
        .load_content("recovered", &Fixture::content(), vec![], vec![])
        .unwrap();
    fixture.waited().await;
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Head script recording the runtime's uncaught error without reporting it.
const REFUSAL: &str = "addEventListener('error',e=>{self.refused=e.error.name;e.preventDefault()})";
/// Body script reporting the installed helper and any refusal.
const LOADED: &str =
    "parent.postMessage({type:'loaded',first:typeof listAttachments,refused:self.refused},'*')";

/// Started and refused runtime startups hold only the host's registrations and release them.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_outcomes_release_host_registrations() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    // A frame policy without the WebAssembly permission refuses the module's compilation. The
    // policy and the error recorder come first, so the inserted runtime meets both.
    let refusing = format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'unsafe-inline'\"><script>{REFUSAL}</script>"
    );
    for (prefix, outcome) in [
        (
            format!("<script>{REFUSAL}</script>"),
            [Some("function"), None],
        ),
        (refusing, [Some("undefined"), Some("CompileError")]),
    ] {
        fixture.arm("loaded");
        let helper = SharedHandle::new(Builtin(maestro_app::BuiltinRuntime::Attachments))
            as SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>;
        sandbox
            .load_content(
                "outcome",
                &format!("{prefix}{}", loader_document("", LOADED)),
                vec![helper],
                vec![],
            )
            .unwrap();
        let loaded = fixture.waited().await;
        let field = |key: &str| {
            js_sys::Reflect::get(&loaded, &key.into())
                .unwrap()
                .as_string()
        };
        assert_eq!(
            [field("first"), field("refused")],
            outcome.map(|value| value.map(str::to_owned))
        );
        fixture.closed(2);
        sandbox.remove().unwrap();
        fixture.route_check("empty").await;
    }
    fixture.end();
    Fixture::finish();
}

/// Written-document script that replays readiness and an error, then answers a flush.
const REPLAY: &str = "addEventListener('message',e=>{const d=e.data;if(d?.type==='sandbox-load')parent.postMessage({type:'code-received',id:d.sandboxId,code:d.code},'*');if(d?.type==='flush')parent.postMessage({type:'flushed'},'*')});parent.postMessage({type:'sandbox-ready'},'*');parent.postMessage({type:'sandbox-error',error:'late',stack:'late'},'*');parent.postMessage({type:'replayed',first:typeof listAttachments},'*')";

/// The current frame's ready or error handshake settles once, ignoring replays and a replaced frame.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_url_handshake_settles_once() {
    let fixture = Fixture::default();
    fixture.begin();
    let sandbox = adapter();
    writer(&sandbox, true);
    for load in ["first", "replacement"] {
        fixture.arm("replayed");
        sandbox
            .load_content(
                &format!("handshake-{load}"),
                &loader_document("", REPLAY),
                vec![
                    SharedHandle::new(Builtin(maestro_app::BuiltinRuntime::Attachments))
                        as SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>,
                ],
                vec![SharedHandle::new(Consumer(fixture.clone()))],
            )
            .unwrap();
        if load == "replacement" {
            fixture.dispatch("sandbox-error", "previous");
        }
        let replayed = fixture.waited().await;
        assert_eq!(
            js_sys::Reflect::get(&replayed, &"first".into())
                .unwrap()
                .as_string()
                .as_deref(),
            Some("function")
        );
        fixture.dispatch("sandbox-ready", "current");
        fixture.route_check("replayed").await;
    }
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
    Fixture::finish();
}

/// Prepare only the provider failure selected by the existing native fixture.
fn failing_providers(
    fixture: &Fixture,
    mode: &str,
) -> Vec<SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>> {
    if mode == "data" || mode == "runtime" {
        vec![maestro_app::SharedHandle::new(Provider {
            fixture: fixture.clone(),
            data: js_sys::Object::new().into(),
            runtime: "function(){}".into(),
            failure: Some(mode.into()),
        })]
    } else if mode == "serializer" {
        let data = js_sys::Object::new();
        js_sys::Reflect::set(&data, &"value".into(), &wasm_bindgen::JsValue::UNDEFINED).unwrap();
        vec![maestro_app::SharedHandle::new(Provider {
            fixture: fixture.clone(),
            data: data.into(),
            runtime: "function(){}".into(),
            failure: None,
        })]
    } else {
        vec![]
    }
}
