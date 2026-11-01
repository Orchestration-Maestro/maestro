//! Hidden execution through the retained native browser adapter.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::placement;
use maestro_app::Cancellation;
use placement::adapter;

/// Already-aborted execution never mounts a frame; direct empty code remains admitted.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_preflight_avoids_all_effects() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let trace = Trace::default();
    let provider: Provider = SharedHandle::new(Probe {
        trace: trace.clone(),
        ..Probe::default()
    });
    let signal = Cancellation::new();
    signal.abort();
    let mut input = request("return 9", vec![provider.clone()]);
    input.signal = Some(signal.clone());
    assert_eq!(
        sandbox
            .execute(input, vec![], false)
            .await
            .err()
            .unwrap()
            .message,
        "Execution aborted"
    );
    for (code, expected) in [
        ("", "Code parameter is required"),
        ("return 9", "Execution aborted"),
    ] {
        let error = maestro_app::browser::tools::javascript_repl::execute_javascript(
            code.into(),
            vec![provider.clone()],
            Some(signal.clone()),
            maestro_app::browser::components::sandboxed_iframe::SandboxEnvironment {
                host: std::sync::Arc::new(maestro_web::BrowserSandboxHost),
                sandbox_url_provider: None,
            },
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.message, expected);
        hidden_released();
    }
    assert!(trace.borrow().is_empty());
    fixture.closed(0);
    fixture.timers(0);
    let result = direct(&sandbox, "", vec![]).await;
    assert!(result.success);
    assert!(result.return_value.is_none());
    fixture.end();
}

use crate::sandbox_execution_support;
use maestro_app::SharedHandle;
use sandbox_execution_support::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast as _;

/// Console runtime precedes supplied providers; repeated entries are not deduplicated.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_prepends_console_and_notifies_providers() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for supplied_signal in [false, true] {
        let trace = Trace::default();
        let provider: Provider = SharedHandle::new(Probe { label: "same", trace: trace.clone(), runtime: "function(){if(!window.console.log.toString().includes('sendRuntimeMessage'))throw Error('console missing');}".into(), start: Some(Rc::new(move |signal| { assert_eq!(signal.is_some(), supplied_signal); Ok(()) })), ..Probe::default() });
        let mut input = request("return 7", vec![provider.clone(), provider]);
        input.signal = supplied_signal.then(Cancellation::new);
        let result = sandbox.execute(input, vec![], false).await.unwrap();
        assert!(result.success);
        assert_eq!(result.return_value.unwrap().0.as_f64(), Some(7.0));
        assert_eq!(
            &*trace.borrow(),
            &[
                "start:same:execution",
                "start:same:execution",
                "provider:same:execution-complete",
                "provider:same:execution-complete",
                "end:same:execution",
                "end:same:execution"
            ]
        );
    }
    fixture.end();
}

/// Providers run before both additional consumers, which precede terminal collection.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_routes_providers_before_all_consumers() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let trace = Trace::default();
    let provider: Provider = SharedHandle::new(Probe {
        label: "p",
        trace: trace.clone(),
        ..Probe::default()
    });
    let result = sandbox
        .execute(
            request(
                "await sendRuntimeMessage({type:'custom'}); console.log('line'); return 9",
                vec![provider],
            ),
            vec![
                SharedHandle::new(Consumer("a", trace.clone())),
                SharedHandle::new(Consumer("b", trace.clone())),
            ],
            false,
        )
        .await
        .unwrap();
    assert!(
        result.success,
        "{}",
        result
            .error
            .as_ref()
            .map_or("no error", |error| error.message.as_str())
    );
    assert_eq!(result.console[0].text, "line");
    assert_eq!(
        &*trace.borrow(),
        &[
            "start:p:execution",
            "provider:p:custom",
            "consumer:a:custom",
            "consumer:b:custom",
            "provider:p:console",
            "consumer:a:console",
            "consumer:b:console",
            "provider:p:execution-complete",
            "consumer:a:execution-complete",
            "consumer:b:execution-complete",
            "end:p:execution"
        ]
    );
    fixture.end();
}

/// File fields are captured on arrival, while content handles remain live through hooks.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_collects_ordered_logs_and_original_downloads() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let (hook, original) = file_editor();
    let result = direct(&sandbox, "console.warn('first');console.log('second');console.warn('first');await returnDownloadableFile('same','雪\\0','text/plain');await returnDownloadableFile('same','','text/plain');await returnDownloadableFile('binary',new Uint8Array([1,2]),'application/octet-stream');return 3;", vec![downloads(),hook.clone()]).await;
    assert_eq!(
        result
            .console
            .iter()
            .map(|entry| (entry.r#type.as_str(), entry.text.as_str()))
            .collect::<Vec<_>>(),
        [("warn", "first"), ("log", "second"), ("warn", "first")]
    );
    let files = result.files.unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.file_name.as_str())
            .collect::<Vec<_>>(),
        ["same", "same", "binary"]
    );
    assert_eq!(files[0].content.0.as_string().unwrap(), "雪\0");
    assert_eq!(files[1].content.0.as_string().unwrap(), "");
    let returned = files[2]
        .content
        .0
        .clone()
        .unchecked_into::<js_sys::Uint8Array>();
    assert_eq!(returned.to_vec(), [255, 2]);
    let alias = original.borrow().clone().unwrap();
    alias.set_index(1, 99);
    assert_eq!(returned.to_vec(), [255, 99]);
    returned.set_index(0, 7);
    assert_eq!(alias.to_vec(), [7, 99]);
    let result = application(
        "await returnDownloadableFile('binary',new Uint8Array([1,2]),'application/octet-stream');",
        vec![downloads(), hook.clone()],
    )
    .await
    .unwrap();
    assert_eq!(
        result.files.unwrap()[0].content,
        maestro_app::browser::components::sandboxed_iframe::SandboxFileContent::Bytes(vec![255, 2])
    );
    maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| {
        *tool.runtime_providers_factory.write().unwrap() = Some(SharedHandle::new(move || {
            Ok(vec![downloads(), hook.clone()])
        }));
    });
    let execute =
        maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| tool.tool.read().unwrap().execute.clone());
    let result=execute("file".into(),serde_json::json!({"title":"download","code":"await returnDownloadableFile('binary',new Uint8Array([1,2]),'application/octet-stream');"}),None,None).await.unwrap();
    assert_eq!(
        result.details.as_ref().unwrap()["files"][0]["contentBase64"],
        "/wI="
    );
    assert_eq!(result.details.as_ref().unwrap()["files"][0]["size"], 2);

    fixture.end();
}

/// Runtime error keeps earlier console/files and the supplied error fields.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_error_keeps_accumulated_output_and_stack() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let code = "console.log('prior');await returnDownloadableFile('prior','file','text/plain');await complete({message:'failure',stack:'controlled stack'});";
    let result = direct(&sandbox, code, vec![downloads()]).await;
    assert!(!result.success);
    assert_eq!(result.console[0].text, "prior");
    assert_eq!(result.files.unwrap()[0].file_name, "prior");
    let error = result.error.unwrap();
    assert_eq!(
        (error.message.as_str(), error.stack.as_str()),
        ("failure", "controlled stack")
    );
    let error = application(code, vec![downloads()]).await.err().unwrap();
    assert_eq!(error.message, "prior\n\nError: failure\ncontrolled stack");
    hidden_released();
    fixture.end();
}

/// Running cancellation settles once after actual frame output and file collection.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_abort_releases_every_acquired_resource() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let trace = Trace::default();
    let signal = Cancellation::new();
    let observed = fixture.watch("running");
    let mut input = request(
        "console.log('before');await returnDownloadableFile('f','x');parent.postMessage({type:'running'},'*');await new Promise(()=>{});",
        vec![
            downloads(),
            SharedHandle::new(Probe {
                trace: trace.clone(),
                ..Probe::default()
            }),
        ],
    );
    input.signal = Some(signal.clone());
    let mut execution = sandbox.execute(input, vec![], false);
    pending(&mut execution);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    fixture.timers(1);
    signal.abort();
    assert_eq!(execution.await.err().unwrap().message, "Execution aborted");
    fixture.closed(0);
    fixture.timers(0);
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|entry| entry.starts_with("end:"))
            .count(),
        1
    );
    abort_during_settlement(&fixture, &sandbox).await;
    fixture.end();
}

/// Captured production deadline preserves output without claiming elapsed clock proof.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_timeout_preserves_output_and_uses_two_minutes() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let observed = fixture.watch("deadline-ready");
    let mut execution=sandbox.execute(request("console.log('before');await returnDownloadableFile('f','x');parent.postMessage({type:'deadline-ready'},'*');await new Promise(()=>{});",vec![downloads()]),vec![],false);
    pending(&mut execution);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    pending(&mut execution);
    fixture.timers(1);
    fixture.deadline();
    let result = execution.await.unwrap();
    assert!(!result.success);
    assert_eq!(result.console[0].text, "before");
    assert_eq!(result.files.unwrap()[0].file_name, "f");
    let error = result.error.unwrap();
    assert_eq!(error.message, "Execution timeout (120s)");
    assert_eq!(error.stack, "");
    fixture.closed(0);
    fixture.end();
}

/// HTML mode and URL selection are independent, with actual completion in every branch.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_honors_direct_consumers_and_html_mode() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for html in [false, true] {
        for url in [false, true] {
            let callback = fixture.bootstrap_callback(true);
            sandbox
                .sandbox_url_provider
                .replace(url.then_some(callback));
            let trace = Trace::default();
            let code = if html {
                "<body><script>complete(null,9)</script></body>"
            } else {
                "return 9"
            };
            let result = sandbox
                .execute(
                    request(code, vec![]),
                    vec![SharedHandle::new(Consumer("direct", trace.clone()))],
                    html,
                )
                .await
                .unwrap();
            assert_eq!(result.return_value.unwrap().0.as_f64(), Some(9.0));
            assert_eq!(&*trace.borrow(), &["consumer:direct:execution-complete"]);
        }
    }
    fixture.end();
}

/// URL selection is read after start hooks; one native handshake sends one document.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_url_handshake_uses_live_callback_once() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let calls = Rc::new(RefCell::new(Vec::new()));
    for replace_in_start in [false, true] {
        let initial = calls.clone();
        sandbox
            .sandbox_url_provider
            .replace(Some(SharedHandle::new({
                let fixture = fixture.clone();
                move || {
                    initial.borrow_mut().push("initial");
                    Ok(fixture.bootstrap(false))
                }
            })));
        let slot = sandbox.sandbox_url_provider.clone();
        let changed = calls.clone();
        let replacement_fixture = fixture.clone();
        let providers = if replace_in_start {
            vec![SharedHandle::new(Probe {
                start: Some(Rc::new(move |_| {
                    replace_bootstrap(&slot, &changed, &replacement_fixture);
                    Ok(())
                })),
                ..Probe::default()
            }) as Provider]
        } else {
            vec![]
        };
        let observed = fixture.watch("bootstrap-waiting");
        let mut execution = sandbox.execute(request("return 9", providers), vec![], false);
        pending(&mut execution);
        wasm_bindgen_futures::JsFuture::from(observed)
            .await
            .unwrap();
        fixture.dispatch("sandbox-ready", false);
        pending(&mut execution);
        fixture.dispatch("sandbox-ready", true);
        fixture.dispatch("sandbox-ready", true);
        fixture.dispatch("sandbox-error", true);
        assert_eq!(
            execution.await.unwrap().return_value.unwrap().0.as_f64(),
            Some(9.0)
        );
    }
    assert_eq!(&*calls.borrow(), &["initial", "replacement"]);
    fixture.end();
}

/// Bootstrap failures traverse the existing admitted router and both callback phases.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_url_bootstrap_error_reuses_admitted_router() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    sandbox
        .sandbox_url_provider
        .replace(Some(SharedHandle::new({
            let fixture = fixture.clone();
            move || Ok(fixture.bootstrap(false))
        })));
    let trace = Trace::default();
    let observed = fixture.watch("bootstrap-waiting");
    let mut execution = sandbox.execute(
        request(
            "return 9",
            vec![SharedHandle::new(Probe {
                label: "p",
                trace: trace.clone(),
                ..Probe::default()
            })],
        ),
        vec![SharedHandle::new(Consumer("c", trace.clone()))],
        false,
    );
    pending(&mut execution);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    fixture.dispatch("sandbox-error", false);
    pending(&mut execution);
    fixture.dispatch("sandbox-error", true);
    let result = execution.await.unwrap();
    assert!(!result.success);
    let error = result.error.unwrap();
    assert_eq!(
        (error.message.as_str(), error.stack.as_str()),
        ("bootstrap problem", "native stack")
    );
    assert_eq!(
        &*trace.borrow(),
        &[
            "start:p:execution",
            "provider:p:execution-error",
            "consumer:c:execution-error",
            "end:p:execution"
        ]
    );
    fixture.end();
}

/// Pending ready/error observation is detached immediately on cancellation.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_abort_before_url_ready_detaches_handshake() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    sandbox
        .sandbox_url_provider
        .replace(Some(SharedHandle::new({
            let fixture = fixture.clone();
            move || Ok(fixture.bootstrap(false))
        })));
    let signal = Cancellation::new();
    let mut input = request("return 9", vec![]);
    input.signal = Some(signal.clone());
    let observed = fixture.watch("bootstrap-waiting");
    let mut execution = sandbox.execute(input, vec![], false);
    pending(&mut execution);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    let source = fixture.remember_frame();
    fixture.closed(2);
    signal.abort();
    assert_eq!(execution.await.err().unwrap().message, "Execution aborted");
    fixture.closed(0);
    fixture.dispatch_source("sandbox-ready", &source);
    fixture.dispatch_source("sandbox-error", &source);
    fixture.closed(0);
    fixture.end();
}

/// Each start/preparation failure preserves its original diagnostic and unwinds all entries.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_setup_failures_unwind_the_shared_owner() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for phase in ["first-start", "later-start", "data", "runtime"] {
        let trace = Trace::default();
        let providers = setup_providers(phase, &trace);
        let error = sandbox
            .execute(request("return 9", providers), vec![], false)
            .await
            .err()
            .unwrap();
        assert_eq!(error.message, format!("{phase} failure"));
        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|entry| entry.starts_with("end:"))
                .cloned()
                .collect::<Vec<_>>(),
            ["end:first:execution", "end:second:execution"]
        );
        fixture.closed(0);
        fixture.timers(0);
        assert!(
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .query_selector("iframe")
                .unwrap()
                .is_none()
        );
    }
    fixture.end();
}

/// Native construction, mounting, registration and URL failures release acquired resources.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_native_failures_unwind_the_shared_owner() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for mode in [
        "create",
        "append",
        "attribute",
        "register",
        "register-adapter",
        "url",
    ] {
        fixture.fault(mode);
        sandbox
            .sandbox_url_provider
            .replace((mode == "url").then(|| {
                SharedHandle::new(|| Err(maestro_app::DiagnosticErrorInfo::error("URL failure")))
                    as _
            }));
        let error = sandbox
            .execute(request("return 9", vec![]), vec![], false)
            .await
            .err()
            .unwrap();
        assert!(!error.message.is_empty());
        if mode == "url" {
            assert_eq!(error.message, "URL failure");
        }
        fixture.fault("");
        sandbox.remove().unwrap();
        fixture.closed(0);
        fixture.timers(0);
    }
    fixture.end();
}

/// Explicit validation failures reject without painting a loading error document.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_validation_rejects_without_a_frame_or_live_timer() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for (mode, expected) in [
        ("text", "controlled parser error"),
        ("empty", "Unknown parse error"),
        ("throw", "controlled parse failure"),
        ("empty-throw", "Unknown validation error"),
    ] {
        fixture.parser(mode);
        let error = sandbox
            .execute(request("return 9", vec![]), vec![], false)
            .await
            .err()
            .unwrap();
        fixture.restore_parser();
        assert_eq!(error.message, format!("HTML validation failed: {expected}"));
        fixture.closed(0);
        fixture.timers(0);
        assert!(
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .query_selector("iframe")
                .unwrap()
                .is_none()
        );
    }
    let result = sandbox
        .execute(
            request(
                "<body><p>recovered<script>complete(null,9)</script>",
                vec![],
            ),
            vec![],
            true,
        )
        .await
        .unwrap();
    assert_eq!(result.return_value.unwrap().0.as_f64(), Some(9.0));
    fixture.end();
}

/// An end-hook failure cannot skip later entries or replace an earlier setup failure.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_end_failure_cannot_prevent_resource_release() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for earlier in [false, true] {
        let trace = Trace::default();
        let providers = vec![
            SharedHandle::new(Probe {
                label: "first",
                trace: trace.clone(),
                end: Some(Rc::new(|| {
                    Err(maestro_app::DiagnosticErrorInfo::error("end failure"))
                })),
                failure: earlier.then_some("data"),
                ..Probe::default()
            }) as Provider,
            SharedHandle::new(Probe {
                label: "second",
                trace: trace.clone(),
                ..Probe::default()
            }),
        ];
        let error = sandbox
            .execute(request("return 9", providers), vec![], false)
            .await
            .err()
            .unwrap();
        assert_eq!(
            error.message,
            if earlier {
                "data failure"
            } else {
                "end failure"
            }
        );
        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|entry| entry.starts_with("end:"))
                .cloned()
                .collect::<Vec<_>>(),
            ["end:first:execution", "end:second:execution"]
        );
        fixture.closed(0);
        fixture.timers(0);
    }
    fixture.end();
}

/// Cancellation raised synchronously by a start callback is observed before mounting.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_observes_abort_raised_by_start_hook() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for abort in [true, false] {
        let mut input = request(
            "return 9",
            vec![SharedHandle::new(Probe {
                start: Some(Rc::new(move |signal| {
                    let signal = signal.unwrap();
                    if abort {
                        signal.abort();
                    }
                    Ok(())
                })),
                ..Probe::default()
            })],
        );
        input.signal = Some(Cancellation::new());
        let result = sandbox.execute(input, vec![], false).await;
        if abort {
            assert_eq!(result.err().unwrap().message, "Execution aborted");
        } else {
            assert_eq!(result.unwrap().return_value.unwrap().0.as_f64(), Some(9.0));
        }
        fixture.closed(0);
        fixture.timers(0);
    }
    fixture.end();
}

/// Completion awaits gated callbacks and keeps their ordered console output on both outcomes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_waits_for_completion_callbacks_and_console_flush() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for success in [true, false] {
        let code = format!(
            "onCompleted(async ok=>{{console.log('one '+ok);parent.postMessage({{type:'callback-ready'}},'*');await new Promise(resolve=>addEventListener('message',e=>{{if(e.data==='release')resolve();}}));console.log('done');}});onCompleted(ok=>console.warn('two '+ok));console.log('before');{}",
            if success {
                "return 7"
            } else {
                "throw Error('user failure')"
            }
        );
        let observed = fixture.watch("callback-ready");
        let mut execution = sandbox.execute(request(&code, vec![]), vec![], false);
        pending(&mut execution);
        wasm_bindgen_futures::JsFuture::from(observed)
            .await
            .unwrap();
        pending(&mut execution);
        fixture.release_frame();
        let result = execution.await.unwrap();
        assert_eq!(result.success, success);
        assert_eq!(
            result
                .console
                .iter()
                .map(|entry| entry.text.clone())
                .collect::<Vec<_>>(),
            [
                "before".into(),
                format!("one {success}"),
                format!("two {success}"),
                "done".into()
            ]
        );
        let code = format!(
            "onCompleted(async()=>{{throw Error('callback failed')}});{}",
            if success {
                "return 7"
            } else {
                "throw Error('user failure')"
            }
        );
        let result = direct(&sandbox, &code, vec![]).await;
        assert_eq!(result.success, success);
        assert!(
            result
                .console
                .iter()
                .any(|entry| entry.text.starts_with("Completion callback error:"))
        );
    }
    fixture.end();
}

/// Native serialization failures remain errors after actual execution and owned cleanup.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_serialization_failures_stay_native() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for code in [
        "const value={};value.self=value;return value;",
        "return {nested:9n};",
    ] {
        let result = direct(&sandbox, code, vec![]).await;
        assert!(result.success);
        assert!(result.return_value.unwrap().0.is_object());
        let error = application(code, vec![]).await.err().unwrap();
        assert!(!error.message.is_empty());
        assert!(!error.message.contains("=> null"));
        hidden_released();
        fixture.closed(0);
    }
    fixture.end();
}

/// Shared application whitespace and native numeric spelling reach the final output unchanged.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_preserves_shared_whitespace_and_number_spelling() {
    let fixture = Fixture::load().await;
    fixture.begin();
    for (code, expected) in [
        (" \n\t", "Code executed successfully (no output)"),
        ("return '\\uFEFF value \\uFEFF';", "=> ﻿ value"),
        ("return '\\u0085';", "=> \u{85}"),
        ("return -0;", "=> 0"),
        ("return 1e-7;", "=> 1e-7"),
        ("return 1e21;", "=> 1e+21"),
        (
            "return {z:1,'10':10,'2':2,a:[true,null]};",
            "=> {\n  \"2\": 2,\n  \"10\": 10,\n  \"z\": 1,\n  \"a\": [\n    true,\n    null\n  ]\n}",
        ),
    ] {
        assert_eq!(application(code, vec![]).await.unwrap().output, expected);
        hidden_released();
    }
    fixture.end();
}

/// Default registration is lazy, configurable, and executes the same real hidden adapter.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn hidden_host_and_default_tool_use_the_same_real_adapter() {
    use maestro_web::browser::sandbox_value::BrowserSandboxValue;
    use maestro_web::{JAVASCRIPT_REPL_TOOL, SandboxRuntimeProvider, SandboxUrlProvider};

    let fixture = Fixture::load().await;
    fixture.begin();
    let first = JAVASCRIPT_REPL_TOOL.with(|tool| tool.tool.clone());
    let second = JAVASCRIPT_REPL_TOOL.with(|tool| tool.tool.clone());
    assert!(SharedHandle::ptr_eq(&first, &second));
    hidden_released();
    let expected = tool_defaults();
    {
        let tool = first.read().unwrap();
        assert_eq!(tool.definition.name, expected.name);
        assert_eq!(tool.label, expected.label);
        assert_eq!(*tool.definition.parameters, expected.parameters);
    }
    let trace = Trace::default();
    let provider: SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>> =
        SharedHandle::new(Probe {
            trace: trace.clone(),
            runtime: "function(){window.configured=41;}".into(),
            ..Probe::default()
        });
    JAVASCRIPT_REPL_TOOL.with(|tool| {
        assert!(tool.sandbox_url_provider.read().unwrap().is_none());
        assert!(
            tool.runtime_providers_factory
                .read()
                .unwrap()
                .as_ref()
                .unwrap()()
            .unwrap()
            .is_empty()
        );
        *tool.runtime_providers_factory.write().unwrap() =
            Some(SharedHandle::new(move || Ok(vec![provider.clone()])));
        let url: SandboxUrlProvider = SharedHandle::new({
            let fixture = fixture.clone();
            move || Ok(fixture.bootstrap(true))
        });
        *tool.sandbox_url_provider.write().unwrap() = Some(url);
    });
    let execute = first.read().unwrap().execute.clone();
    let result = execute(
        "configured".into(),
        serde_json::json!({"title":"run","code":"return window.configured+1"}),
        None,
        None,
    )
    .await
    .unwrap();
    let maestro_app::UserBlock::Text(text) = &result.content[0] else {
        panic!("text result")
    };
    assert_eq!(text.text, "=> 42");
    assert!(trace.borrow().iter().any(|entry| entry.starts_with("end:")));
    hidden_released();
    root_records().await;
    fixture.end();
}

/// Settling one native execution cannot release its still-running sibling.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_instances_do_not_share_cleanup_or_results() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let first = adapter();
    let second = adapter();
    let observed = fixture.watch("first-running");
    let mut a=first.execute(request("parent.postMessage({type:'first-running'},'*');await new Promise(resolve=>addEventListener('message',e=>{if(e.data==='release')resolve()}));return 7;",vec![]),vec![],false);
    pending(&mut a);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    let source = fixture.remember_frame();
    let observed = fixture.watch("second-running");
    let signal = Cancellation::new();
    let mut input = request(
        "parent.postMessage({type:'second-running'},'*');await new Promise(()=>{});",
        vec![],
    );
    input.sandbox_id = "sibling".into();
    input.signal = Some(signal.clone());
    let mut b = second.execute(input, vec![], false);
    pending(&mut b);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    fixture.timers(2);
    fixture.release_source(&source);
    assert_eq!(a.await.unwrap().return_value.unwrap().0.as_f64(), Some(7.0));
    fixture.closed(2);
    fixture.timers(1);
    pending(&mut b);
    signal.abort();
    assert_eq!(b.await.err().unwrap().message, "Execution aborted");
    fixture.closed(0);
    fixture.end();
}

/// Dropping a polled future releases the containing adapter's run and callback captures.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn executing_future_drop_releases_the_retained_owner() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let trace = Trace::default();
    let provider: Provider = SharedHandle::new(Probe {
        trace: trace.clone(),
        ..Probe::default()
    });
    let weak = SharedHandle::downgrade(&provider);
    let observed = fixture.watch("drop-ready");
    let mut execution = sandbox.execute(
        request(
            "parent.postMessage({type:'drop-ready'},'*');await new Promise(()=>{});",
            vec![provider],
        ),
        vec![],
        false,
    );
    pending(&mut execution);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    fixture.timers(1);
    drop(execution);
    drop(sandbox);
    fixture.closed(0);
    fixture.timers(0);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|entry| entry.starts_with("end:"))
            .count(),
        1
    );
    fixture.end();
}

/// Failed native detach remains owned for retry on both execution and loading paths.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn lifecycle_removal_failures_retain_retryable_native_handles() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    let trace = Trace::default();
    let observed = fixture.watch("retry-ready");
    let mut execution = sandbox.execute(
        request(
            "parent.postMessage({type:'retry-ready'},'*');await new Promise(()=>{});",
            vec![SharedHandle::new(Probe {
                trace: trace.clone(),
                ..Probe::default()
            })],
        ),
        vec![],
        false,
    );
    pending(&mut execution);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    let source = fixture.remember_frame();
    fixture.fault("detach");
    assert!(sandbox.remove().is_err());
    assert_eq!(execution.await.err().unwrap().message, "Execution aborted");
    fixture.timers(0);
    fixture.closed(2);
    fixture.dispatch_source("sandbox-error", &source);
    fixture.fault("");
    sandbox.remove().unwrap();
    fixture.closed(0);
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|entry| entry.starts_with("end:"))
            .count(),
        1
    );
    sandbox
        .load_content("loading-retry", "<body>loading</body>", vec![], vec![])
        .unwrap();
    fixture.fault("detach");
    assert!(sandbox.remove().is_err());
    fixture.closed(2);
    fixture.fault("");
    sandbox.remove().unwrap();
    fixture.closed(0);
    fixture.end();
}

/// Raw native return kinds survive actual frames and reach the registered tool formatter.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_retains_native_returns_through_repl_formatting() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for row in return_cases() {
        let result = direct(&sandbox, &row.code, vec![]).await;
        assert!(result.success);
        if row.name == "native_missing" {
            assert!(result.return_value.is_none());
        } else {
            let value = result.return_value.unwrap();
            fixture.assert_native(&row.name, &value.0);
        }
        assert_eq!(tool_output(&row.code).await.unwrap(), row.output);
        hidden_released();
    }
    let provider = terminal_editor("execution-complete", "m.returnValue.answer=2");
    let result = direct(&sandbox, "return {answer:1};", vec![provider.clone()]).await;
    assert_eq!(
        field(&result.return_value.unwrap(), "answer").as_f64(),
        Some(2.0)
    );
    maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| {
        *tool.runtime_providers_factory.write().unwrap() =
            Some(SharedHandle::new(move || Ok(vec![provider.clone()])));
    });
    assert_eq!(
        tool_output("return {answer:1};").await.unwrap(),
        "=> {\n  \"answer\": 2\n}"
    );
    fixture.end();
}

/// End hooks may replace, add or remove terminal fields before SDK projection.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_reads_terminal_fields_after_end_hooks() {
    let fixture = Fixture::load().await;
    fixture.begin();
    let sandbox = adapter();
    for (code, edit, expected) in [
        ("return 1;", "m.returnValue=2", Some(2.0)),
        ("return;", "m.returnValue=2", Some(2.0)),
        ("return 1;", "delete m.returnValue", None),
    ] {
        let provider = terminal_editor("execution-complete", edit);
        let result = direct(&sandbox, code, vec![provider.clone()]).await;
        assert_eq!(result.return_value.map(|v| v.0.as_f64().unwrap()), expected);
        maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| {
            *tool.runtime_providers_factory.write().unwrap() =
                Some(SharedHandle::new(move || Ok(vec![provider.clone()])));
        });
        assert_eq!(
            tool_output(code).await.unwrap(),
            expected.map_or("Code executed successfully (no output)", |_| "=> 2")
        );
        hidden_released();
    }
    for edit in [
        "m.error={message:'edited message',stack:'edited stack'}",
        "m.error.message='edited message';m.error.stack='edited stack'",
    ] {
        let provider = terminal_editor("execution-error", edit);
        let result = direct(&sandbox, "throw Error('original');", vec![provider.clone()]).await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.message, "edited message");
        assert_eq!(error.stack, "edited stack");
        let error = application("throw Error('original');", vec![provider])
            .await
            .err()
            .unwrap();
        assert_eq!(error.message, "Error: edited message\nedited stack");
        hidden_released();
    }
    fixture.end();
}

/// Open download content reaches the shared native string fallback and tool encoder.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn execution_encodes_nonbyte_download_content() {
    use maestro_app::browser::components::sandboxed_iframe::SandboxFileContent;

    let fixture = Fixture::load().await;
    fixture.begin();
    let code = "await returnDownloadableFile('missing.json', undefined);";
    let result = application(code, vec![downloads()]).await.unwrap();
    let file = result.files.unwrap().remove(0);
    let SandboxFileContent::Text(text) = file.content else {
        panic!("native string fallback");
    };
    assert_eq!(text.as_bytes(), b"undefined");
    maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| {
        *tool.runtime_providers_factory.write().unwrap() =
            Some(SharedHandle::new(|| Ok(vec![downloads()])));
    });
    let execute =
        maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| tool.tool.read().unwrap().execute.clone());
    let result = execute(
        "fallback".into(),
        serde_json::json!({"title":"fallback", "code":code}),
        None,
        None,
    )
    .await
    .unwrap();
    let details = result.details.unwrap();
    assert_eq!(details["files"][0]["fileName"], "missing.json");
    assert_eq!(details["files"][0]["contentBase64"], "dW5kZWZpbmVk");
    assert_eq!(details["files"][0]["size"], 9);
    hidden_released();
    fixture.end();
}

/// Install the live replacement only when its start callback is reached.
fn replace_bootstrap(
    slot: &Rc<
        RefCell<Option<maestro_app::browser::components::sandboxed_iframe::SandboxUrlProvider>>,
    >,
    calls: &Rc<RefCell<Vec<&'static str>>>,
    fixture: &Fixture,
) {
    let calls = calls.clone();
    let fixture = fixture.clone();
    slot.replace(Some(SharedHandle::new(move || {
        calls.borrow_mut().push("replacement");
        Ok(fixture.bootstrap(false))
    })));
}

/// Keep cancellation outcomes distinct during completion and after end hooks.
async fn abort_during_settlement(fixture: &Fixture, sandbox: &maestro_web::SandboxIframe) {
    let during = Cancellation::new();
    let abort = during.clone();
    let provider: Provider = SharedHandle::new(Probe {
        observe: Some(Rc::new(move |value| {
            if tag(value) == "execution-complete" {
                abort.abort();
            }
        })),
        ..Probe::default()
    });
    let mut input = request("return 9", vec![provider]);
    input.signal = Some(during);
    assert_eq!(
        sandbox
            .execute(input, vec![], false)
            .await
            .err()
            .unwrap()
            .message,
        "Execution aborted"
    );
    fixture.closed(0);
    fixture.timers(0);
    let late = Cancellation::new();
    let abort = late.clone();
    let mut input = request(
        "return 9",
        vec![SharedHandle::new(Probe {
            end: Some(Rc::new(move || {
                abort.abort();
                Ok(())
            })),
            ..Probe::default()
        })],
    );
    input.signal = Some(late.clone());
    let result = sandbox.execute(input, vec![], false).await.unwrap();
    late.abort();
    assert_eq!(result.return_value.unwrap().0.as_f64(), Some(9.0));
}
