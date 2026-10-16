//! Actual browser document, event-source and subscription lifetime witnesses.
#![cfg(all(test, target_arch = "wasm32"))]
mod sandbox_loading_support;
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
use maestro_web::browser::sandbox_value::BrowserSandboxValue;
use sandbox_loading_support::*;
use std::sync::Arc;

/// Native values retain their stringify semantics at the public preparation boundary.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn prepared_documents_keep_native_data_and_failures() {
    let mut sandbox = adapter();
    for row in document_cases() {
        let prepared =
            sandbox.prepare_html_document(&row.id, &row.code, &row.providers, Some(row.options));
        if let Some(category) = js_sys::Reflect::get(&row.expected, &"error_category".into())
            .unwrap()
            .as_string()
        {
            let error = prepared.unwrap_err();
            assert_eq!(error.name.as_deref().unwrap_or("Error"), category);
        } else {
            wasm_bindgen_futures::JsFuture::from(observe_document(
                &row.name,
                &prepared.unwrap(),
                row.expected,
            ))
            .await
            .unwrap();
        }
    }
    sandbox.remove().unwrap();
    finish();
}

/// Download preparation keeps provider helpers without installing online routes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn standalone_runtime_keeps_helpers_without_parent_messaging() {
    let mut sandbox = adapter();
    let html = sandbox
        .prepare_html_document(
            "offline",
            "<body>offline</body>",
            &[provider("function(id){window.localHelper=()=>id;}")],
            Some(
                maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
                    is_html_artifact: true,
                    is_standalone: true,
                },
            ),
        )
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(standalone(&html))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    finish();
}

/// Completion waits for callbacks without replacing a user's result with callback failure.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn repl_completion_callbacks_preserve_order_and_errors() {
    let mut sandbox = adapter();
    for row in fixture_group("callbacks_") {
        let code = if row.input["outcome"] == "success" {
            "return 9;"
        } else {
            "throw new Error('user failure');"
        };
        let html = sandbox
            .prepare_html_document("callbacks", code, &[], None)
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(callbacks(
            &html,
            &row.input.to_string(),
            &row.output.to_string(),
        ))
        .await
        .unwrap();
    }
    sandbox.remove().unwrap();
    finish();
}

/// The actual mounted frame runs the injected console before supplied runtimes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn srcdoc_load_injects_console_before_supplied_providers() {
    begin();
    let mut sandbox = adapter();
    arm("loaded");
    sandbox
        .load_content(
            "srcdoc",
            &content(),
            vec![
                provider("function(){window.firstProvider=typeof window.complete}"),
                provider("function(){window.secondProvider=window.firstProvider}"),
            ],
            vec![std::sync::Arc::new(Consumer)],
        )
        .unwrap();
    let loaded = wasm_bindgen_futures::JsFuture::from(waited())
        .await
        .unwrap();
    for name in ["first", "second"] {
        assert_eq!(
            js_sys::Reflect::get(&loaded, &name.into())
                .unwrap()
                .as_string()
                .as_deref(),
            Some("function")
        );
    }
    wasm_bindgen_futures::JsFuture::from(route_check("srcdoc"))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// Reload retains one native listener and explicit close revokes the actual callback.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn external_links_dispatch_once_across_reload_and_close() {
    begin();
    let mut sandbox = adapter();
    for _ in 0..2 {
        arm("loaded");
        sandbox
            .load_content("links", &content(), vec![], vec![])
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(waited())
            .await
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(route_check("click"))
            .await
            .unwrap();
    }
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// Current frame identity is read live after replacement and cannot be forged by another window.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn unrelated_or_stale_frames_cannot_open_urls() {
    begin();
    fault("sibling");
    let mut sandbox = adapter();
    for id in ["old", "new"] {
        arm("loaded");
        sandbox
            .load_content(id, &content(), vec![], vec![])
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(waited())
            .await
            .unwrap();
        dispatch("remember", "current");
    }
    wasm_bindgen_futures::JsFuture::from(route_check("unrelated"))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// The URL callback is read again on reload; only actual readiness forwards the document.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn url_load_uses_live_configuration_and_ready_handshake() {
    begin();
    let mut sandbox = adapter();
    for mode in ["first", "replacement"] {
        let expected = url(mode);
        sandbox.sandbox_url_provider = Some(std::sync::Arc::new(move || Ok(expected.clone())));
        arm("url-mounted");
        sandbox
            .load_content(
                &format!("url-{mode}"),
                "<body>url branch</body>",
                vec![],
                vec![],
            )
            .unwrap();
        let mounted = wasm_bindgen_futures::JsFuture::from(waited())
            .await
            .unwrap();
        assert_eq!(
            js_sys::Reflect::get(&mounted, &"mode".into())
                .unwrap()
                .as_string()
                .as_deref(),
            Some(mode)
        );
        wasm_bindgen_futures::JsFuture::from(route_check("ready"))
            .await
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(route_check("reset-opens"))
            .await
            .unwrap();
    }
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// Bootstrap errors use the registered consumer route without admitting a parent-origin forgery.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn url_error_reaches_registered_consumers_without_source_bypass() {
    begin();
    let mut sandbox = adapter();
    sandbox.sandbox_url_provider = Some(std::sync::Arc::new(|| Ok(url("error"))));
    arm("url-mounted");
    sandbox
        .load_content(
            "url-error",
            "<body>url branch</body>",
            vec![provider("function(){}")],
            vec![std::sync::Arc::new(Consumer)],
        )
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(waited())
        .await
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(route_check("error"))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// Tabs are used only for URL loading, with ordinary window fallback otherwise.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn extension_links_use_tabs_then_window_fallback() {
    begin();
    let mut sandbox = adapter();
    fault("tabs");
    sandbox.sandbox_url_provider = Some(std::sync::Arc::new(|| Ok(url("tabs"))));
    arm("url-mounted");
    sandbox
        .load_content("tabs", "<body>url branch</body>", vec![], vec![])
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(waited())
        .await
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(route_check("tabs"))
        .await
        .unwrap();
    sandbox.sandbox_url_provider = None;
    arm("loaded");
    sandbox
        .load_content("tabs-srcdoc", &content(), vec![], vec![])
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(waited())
        .await
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(route_check("click"))
        .await
        .unwrap();
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// Navigation decisions use real resolved anchors and form actions, including nested targets.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn navigation_intercepts_resolved_links_and_forms() {
    let mut sandbox = adapter();
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
        wasm_bindgen_futures::JsFuture::from(navigation(
            &html,
            &row.input.to_string(),
            &row.output.to_string(),
        ))
        .await
        .unwrap();
    }
    sandbox.remove().unwrap();
    finish();
}

/// Native parser recovery and reached parser failures select the exact fallback text.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn validation_uses_native_parser_and_fallback_messages() {
    begin();
    let mut sandbox = adapter();
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
        fault(mode);
        sandbox
            .load_content("validation", html, vec![], vec![])
            .unwrap();
        fault("");
        wasm_bindgen_futures::JsFuture::from(validation_check(expected, ""))
            .await
            .unwrap();
    }
    sandbox.remove().unwrap();
    closed(0);
    end();
    finish();
}

/// The literal error remains inert and uses each independently selected pack and mode.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn validation_document_keeps_literal_text_and_selected_pack() {
    begin();
    let literal = "literal <tag> & <script>window.executed=true</script>";
    for alternate in [false, true] {
        for light in [false, true] {
            let (mut sandbox, css) = adapter_pack(
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
            wasm_bindgen_futures::JsFuture::from(validation_check(literal, &css))
                .await
                .unwrap();
            sandbox.remove().unwrap();
            closed(0);
            finish();
        }
    }
    end();
}

/// Every URL settlement phase and final drop revoke the same native registration.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn close_removes_ready_error_and_external_routes() {
    begin();
    for phase in ["pending", "ready", "error", "invalid", "drop", "retry"] {
        let mut sandbox = adapter();
        sandbox.sandbox_url_provider = Some(std::sync::Arc::new(|| Ok(url("close"))));
        if phase == "invalid" {
            sandbox
                .load_content("closing", "<parsererror>bad</parsererror>", vec![], vec![])
                .unwrap();
            wasm_bindgen_futures::JsFuture::from(validation_check("bad", ""))
                .await
                .unwrap();
        } else {
            arm("url-mounted");
            sandbox
                .load_content("closing", "<body>closing</body>", vec![], vec![])
                .unwrap();
            wasm_bindgen_futures::JsFuture::from(waited())
                .await
                .unwrap();
            if phase == "ready" || phase == "error" {
                dispatch(
                    if phase == "ready" {
                        "sandbox-ready"
                    } else {
                        "sandbox-error"
                    },
                    "current",
                );
            }
        }
        if phase == "retry" {
            fault("detach");
            assert!(sandbox.remove().is_err());
            closed(2);
            fault("");
        }
        if phase != "drop" {
            sandbox.remove().unwrap();
            sandbox.remove().unwrap();
        }
        drop(sandbox);
        closed(0);
        finish();
    }
    end();
}

/// Closing either registered sibling leaves the other's router and opener live.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn removing_one_sandbox_preserves_its_sibling() {
    begin();
    for first in [0, 1] {
        let mut sandboxes = [adapter(), adapter()];
        for (index, sandbox) in sandboxes.iter_mut().enumerate() {
            arm("loaded");
            sandbox
                .load_content(
                    &format!("sibling-{index}"),
                    &content(),
                    vec![],
                    vec![std::sync::Arc::new(Consumer)],
                )
                .unwrap();
            wasm_bindgen_futures::JsFuture::from(waited())
                .await
                .unwrap();
        }
        sandboxes[first].remove().unwrap();
        closed(2);
        wasm_bindgen_futures::JsFuture::from(route_check("click"))
            .await
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(route_check("sibling-probe"))
            .await
            .unwrap();
        sandboxes[1 - first].remove().unwrap();
        closed(0);
        finish();
    }
    end();
}

/// Reached native/provider failures release acquired resources and permit an ordinary next load.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn failed_load_releases_native_registration_and_frame() {
    begin();
    let mut sandbox = adapter();
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
        fault(mode);
        let providers: Vec<Arc<dyn SandboxRuntimeProvider<BrowserSandboxValue>>> =
            if mode == "data" || mode == "runtime" {
                vec![std::sync::Arc::new(Provider {
                    data: js_sys::Object::new().into(),
                    runtime: "function(){}".into(),
                    failure: Some(mode.into()),
                })]
            } else if mode == "serializer" {
                let data = js_sys::Object::new();
                js_sys::Reflect::set(&data, &"value".into(), &wasm_bindgen::JsValue::UNDEFINED)
                    .unwrap();
                vec![std::sync::Arc::new(Provider {
                    data: data.into(),
                    runtime: "function(){}".into(),
                    failure: None,
                })]
            } else {
                vec![]
            };
        sandbox.sandbox_url_provider = (mode == "url").then(|| {
            std::sync::Arc::new(|| Err(maestro_app::DiagnosticErrorInfo::error("URL failure")))
                as maestro_app::browser::components::sandboxed_iframe::SandboxUrlProvider
        });
        let error = sandbox
            .load_content("failed", "<body>attempt</body>", providers, vec![])
            .unwrap_err();
        if mode == "url" {
            assert_eq!(error.message, "URL failure");
        }
        if mode == "data" || mode == "runtime" {
            assert_eq!(error.message, "provider failed");
        }
        fault("");
        wasm_bindgen_futures::JsFuture::from(route_check("empty"))
            .await
            .unwrap();
        sandbox.sandbox_url_provider = None;
        arm("loaded");
        sandbox
            .load_content("recovered", &content(), vec![], vec![])
            .unwrap();
        wasm_bindgen_futures::JsFuture::from(waited())
            .await
            .unwrap();
        sandbox.remove().unwrap();
        closed(0);
    }
    end();
    finish();
}

/// External resources preserve raw template values and script document boundaries.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn program_resources_preserve_raw_templates_and_document_boundaries() {
    let mut sandbox = adapter();
    let html = sandbox.prepare_html_document(
        "resource-values",
        "return String.raw`</ScRiPt>雪<!--<script>`;",
        &[provider("function(){window.providerValue=String.raw`</ScRiPt>雪<!--<script>`;window.complete=(error,value)=>parent.postMessage({type:'resource-result',error,value,provider:window.providerValue,body:!!document.body},'*');}")],
        Some(maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
            is_html_artifact: false, is_standalone: true,
        }),
    ).unwrap();
    wasm_bindgen_futures::JsFuture::from(resource_values(&html))
        .await
        .unwrap();
    sandbox.remove().unwrap();
}

/// Vector links use their native resolved address during a bubbling click.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn vector_links_dispatch_resolved_addresses() {
    let mut sandbox = adapter();
    let html = sandbox
        .prepare_html_document("vector", "", &[], None)
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(navigation(
        &html,
        r#"{"kind":"svg","href":"../diagram"}"#,
        r#"{"events":[["prevent"],["stop"],["post",{"type":"open-external-url","url":"https://example.org/diagram"},"*"]]}"#,
    )).await.unwrap();
    sandbox.remove().unwrap();
}
