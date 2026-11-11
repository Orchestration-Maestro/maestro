//! Generated runtime bootstrap qualification in isolated Chromium frames.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::sandbox_bootstrap_support::{
    Boot, Bundle, Mount, OBSERVER, Runtime, cases, digests, encode, loads_only_from_parent,
    observe, program, receipt, wrap,
};
use maestro_web::RuntimeMessageBridge;
use maestro_web::components::sandbox::routing::{
    RuntimeMessageBridgeOptions, RuntimeMessageContext,
};
use serde_json::{Value, json};

/// Sandbox identifier carried through configuration and provider invocations.
const ID: &str = "probe:\"雪]";
/// First authored head script: helper type, listing and installation trace.
const HEAD: &str = "window.S={first:typeof listAttachments,count:typeof listAttachments==='function'?listAttachments().length:-1,trace:self.__sandboxInstallTrace?[...__sandboxInstallTrace]:null,id:self.sandboxId}";
/// First authored body script: helper type and live global reads.
const BODY: &str = "S.second=typeof listAttachments;if(S.second==='function'){attachments.push({id:'b',fileName:'b.txt',mimeType:'text/plain',size:2});S.live=listAttachments().map(a=>a.id).join()}";
/// Script that keeps a reported runtime failure from reaching the driver as an uncaught error.
const MUTED: &str =
    "for(const t of['error','unhandledrejection'])addEventListener(t,e=>e.preventDefault())";
/// Supplied provider recording its own position in the installation trace.
const TRACE_PROVIDER: &str = "function(id){if(1<2&&id)(self.__sandboxInstallTrace??=[]).push('provider:'+id+':'+typeof listAttachments)}";
/// Supplied provider that turns Trusted Types to `none` inside the frame.
const TRUSTED_NONE: &str = "function(){const m=document.createElement('meta');m.httpEquiv='Content-Security-Policy';m.content=\"require-trusted-types-for 'script'; trusted-types 'none'\";document.head.append(m)}";
/// Supplied completion provider for asynchronous programs.
const COMPLETION: &str = "function(id){window['__maestro'+0]++;window.S={log:[],providerName:arguments.callee.name,providerId:id,providerThis:this===window,providerCurrent:document.currentScript.localName,capturedValue:window['__maestro'+0]};window.done=new Promise(r=>window.complete=async(error,value)=>{S.log.push('complete');S.error=error;S.value=value;S.returnName=typeof value==='function'?value.name:null;S.returnSame=value===window.expected;S.returnNegativeZero=Object.is(value,-0);S.returnUndefined=value===undefined;S.returnNull=value===null;r()})}";
/// Frame policy that requires Trusted Types without any frame policy object.
const STRICT: &str =
    "<meta http-equiv=\"Content-Security-Policy\" content=\"require-trusted-types-for 'script'\">";
/// Probe fields of an installed artifact.
const INSTALLED: &[&str] = &["first", "count", "trace", "id", "second", "live"];

/// Configuration with data globals, one of which the first built-in replaces.
fn config(selections: &[&str]) -> Value {
    json!({
        "sandboxId": ID,
        "globals": {
            "attachments": [{"id":"a","fileName":"a.txt","mimeType":"text/plain","size":1,"content":"YQ=="}],
            "listAttachments": "data value replaced by the runtime"
        },
        "runtime": selections
    })
}

/// Repeated built-ins around one supplied provider.
fn repeated(boot: Boot) -> Runtime {
    Runtime::new(
        boot,
        &config(&["attachments", "provider", "attachments"]),
        ID,
        &[TRACE_PROVIDER],
    )
}

/// The trace of [`repeated`] when every selection installs: the provider already finds the helper.
fn repeated_trace() -> Value {
    json!([format!("provider:{ID}:function")])
}

/// Hash sources for a runtime's loader, providers and the authored artifact scripts.
async fn artifact_hashes(runtime: &Runtime) -> String {
    let mut texts = vec![runtime.boot.as_str()];
    texts.extend(runtime.providers.iter().map(String::as_str));
    texts.extend([HEAD, BODY]);
    digests(&texts).await
}

/// Authored HTML artifact with the runtime inserted after its head prefix.
fn artifact(prefix: &str, runtime: &Runtime) -> String {
    format!(
        "<!DOCTYPE html>{prefix}<html><head>{}<script>{HEAD}</script></head><body><script>{BODY}</script></body></html>",
        runtime.markup
    )
}

/// Expected observation when every selection installs before the first classic scripts.
fn installed(trace: &Value) -> Value {
    json!({"first":"function","count":1,"trace":trace,"id":ID,"second":"function","live":"a,b"})
}

/// Selected probe fields of one frame observation.
fn fields(state: &Value, keys: &[&str]) -> Value {
    keys.iter()
        .map(|key| ((*key).to_owned(), state[*key].clone()))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// The actual bundle installs every selection before the first classic scripts.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_installs_selections_before_first_classic_scripts() {
    let bundle = Bundle::load();
    let runtime = repeated(bundle.boot());
    let policy = format!(
        "script-src 'self' {} 'wasm-unsafe-eval'",
        artifact_hashes(&runtime).await
    );
    let html = artifact("", &runtime);
    let mount = Mount {
        policy: &policy,
        ..Mount::default()
    };
    let (state, _) = observe(&html, mount).await;
    receipt(&format!(
        "install bundle={} document={} state={state}",
        bundle.script.len(),
        html.len()
    ));
    assert_eq!(fields(&state, INSTALLED), installed(&repeated_trace()));
    assert_eq!(
        fields(&state, &["head", "scaffold", "origin"]),
        json!({"head":"script","scaffold":0,"origin":"null"})
    );
}

/// Compare one settled program with its expected log, error message, value and import.
fn check_program(state: &Value, expected: &Value) {
    let error = match &state["error"] {
        Value::Object(error) => {
            let stack = error["stack"].as_str().unwrap();
            if expected["error"].get("name").is_some() {
                json!({"name": stack.split(':').next().unwrap()})
            } else {
                assert!(stack.contains("TypeError: user failure"), "{stack}");
                json!({"message": error["message"]})
            }
        }
        other => other.clone(),
    };
    let observed =
        json!({"log":state["log"],"error":error,"value":state["value"],"helper":state["helper"]});
    assert_eq!(&observed, expected);
    if state.get("metaUrl").is_some() {
        assert_eq!(state["metaUrl"], state["base"]);
    }
    if let Some(carriage_return) = state.get("cr") {
        assert_eq!(carriage_return.as_bool(), Some(true));
    }
    assert_eq!(
        fields(state, &["body", "scaffold"]),
        json!({"body":"","scaffold":0})
    );
}

/// Asynchronous programs settle through Rust: await, return, throw, imports and callbacks.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_settles_asynchronous_programs_in_rust() {
    run_programs(cases::measured(ID)).await;
}

/// Native abrupt completions retain finally cancellation, iterator cleanup and binding scopes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_preserves_completion_control_flow() {
    run_programs(
        cases::finalizer_restoration()
            .into_iter()
            .chain(cases::control_flow())
            .chain(cases::cleanup())
            .collect(),
    )
    .await;
}

/// Actual tool content still executes, while native module early errors settle as SyntaxErrors.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_keeps_native_module_admission() {
    run_programs(cases::admission()).await;
}

/// Captured providers keep anonymous names, invocation context, script timing and global changes.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_captures_native_provider_expressions() {
    let bundle = Bundle::load();
    provider_failures(&bundle).await;
    native_values(&bundle).await;
    let anonymous = json!({"log":["complete"], "error":null, "value":{}, "helper":null});
    for source in ["return function(){}", "return ()=>42", "return class{}"] {
        let state = run_program(&bundle, source, &anonymous, COMPLETION).await;
        assert_eq!(state["returnName"], "", "{source}");
    }
    let named = run_program(
        &bundle,
        "function named(){}return named",
        &anonymous,
        COMPLETION,
    )
    .await;
    assert_eq!(named["returnName"], "named");
    let expected = json!({"log":["complete"], "error":null, "value":3, "helper":null});
    let providers = [
        COMPLETION.to_owned(),
        format!("Object.assign({COMPLETION},{{call:null}})"),
    ];
    for provider in providers {
        let state = run_program(
            &bundle,
            "window['__maestro'+0]++;return window['__maestro'+0]",
            &expected,
            &provider,
        )
        .await;
        assert_eq!(
            fields(
                &state,
                &[
                    "providerName",
                    "providerId",
                    "providerThis",
                    "providerCurrent",
                    "capturedValue"
                ]
            ),
            json!({"providerName":"", "providerId":ID, "providerThis":true, "providerCurrent":"script", "capturedValue":2})
        );
    }
}

/// Protected capture properties cause setup refusal without overwriting their values.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_refuses_protected_capture_properties() {
    let bundle = Bundle::load();
    let supplied = "function(){window.ran=true}";
    let candidate =
        maestro_web::components::sandboxed_iframe::lowering::provider(supplied).unwrap();
    for selections in [
        vec!["provider", "provider", "attachments"],
        vec!["provider"],
    ] {
        let key = candidate.capture();
        let protected = format!(
            "function(){{Object.defineProperty(window,{},{{value:'kept',configurable:false}})}}",
            json!(key)
        );
        let mut configuration = config(&selections);
        if selections.len() == 1 {
            configuration["programCapture"] = key.into();
        }
        let providers = if selections.len() == 1 {
            vec![protected.as_str()]
        } else {
            vec![protected.as_str(), supplied]
        };
        let probe = format!(
            "S.kept=window[{}];S.ran=typeof window.ran;S.helper=typeof listAttachments",
            json!(key)
        );
        let state = provider_document(&bundle, &configuration, &providers, &probe).await;
        receipt(&format!(
            "protected capture selections={selections:?} state={state}"
        ));
        assert_eq!(
            fields(&state, &["kept", "ran", "helper"]),
            json!({"kept":"kept", "ran":"undefined", "helper":"string"})
        );
        let errors = state["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0]
                .as_str()
                .unwrap()
                .starts_with("TypeError: cannot install capture accessor")
        );
    }
}

/// Native identity, signed zero, undefined and null survive outcome capture without JSON conversion.
async fn native_values(bundle: &Bundle) {
    let rows = [
        (
            "return -0",
            json!(0),
            json!({"returnSame":false,"returnNegativeZero":true,"returnUndefined":false,"returnNull":false}),
        ),
        (
            "return undefined",
            Value::Null,
            json!({"returnSame":true,"returnNegativeZero":false,"returnUndefined":true,"returnNull":false}),
        ),
        (
            "return null",
            Value::Null,
            json!({"returnSame":false,"returnNegativeZero":false,"returnUndefined":false,"returnNull":true}),
        ),
        (
            "window.expected={answer:42};return window.expected",
            json!({"answer":42}),
            json!({"returnSame":true,"returnNegativeZero":false,"returnUndefined":false,"returnNull":false}),
        ),
    ];
    for (source, value, native) in rows {
        let expected = json!({"log":["complete"],"error":null,"value":value,"helper":null});
        let state = run_program(bundle, source, &expected, COMPLETION).await;
        assert_eq!(
            fields(
                &state,
                &[
                    "returnSame",
                    "returnNegativeZero",
                    "returnUndefined",
                    "returnNull"
                ]
            ),
            native
        );
    }
}

/// A thrown provider or non-callable expression stops later installation with one native TypeError.
async fn provider_failures(bundle: &Bundle) {
    for source in ["function(){throw new TypeError('provider failure')}", "42"] {
        let state = provider_document(
            bundle,
            &config(&["provider", "attachments"]),
            &[source],
            "S.helper=typeof listAttachments",
        )
        .await;
        assert_eq!(state["helper"], "string");
        let errors = state["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].as_str().unwrap().starts_with("TypeError:"));
    }
}

/// A provider expression that throws before it yields a function stops later installation and is
/// reported once, and a provider the browser refuses to run stops it after the browser's report.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_provider_expression_failure_stops_installation() {
    let bundle = Bundle::load();
    let configuration = config(&["provider", "attachments"]);
    let state = provider_document(
        &bundle,
        &configuration,
        &["(()=>{throw new TypeError('expression failure')})()"],
        "S.helper=typeof listAttachments",
    )
    .await;
    assert_eq!(state["helper"], "string");
    assert_eq!(state["errors"], json!(["TypeError: expression failure"]));
    let refused = provider_document(
        &bundle,
        &configuration,
        &["function(){let a;let a}"],
        "S.helper=typeof listAttachments",
    )
    .await;
    assert_eq!(refused["helper"], "string");
    let errors = refused["errors"].as_array().unwrap();
    assert!(errors[0].as_str().unwrap().starts_with("SyntaxError:"));
    assert_eq!(
        errors.get(1).and_then(Value::as_str),
        Some("TypeError: provider did not complete")
    );
}

/// A provider that installs its own error handler, by assignment or by defining the property, keeps
/// it, and one that does so before throwing still stops installation.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_provider_error_handler_writes_survive_installation() {
    let bundle = Bundle::load();
    let configuration = config(&["provider", "attachments"]);
    for write in [
        "window.onerror=window.savedHandler",
        "Object.defineProperty(window,'onerror',{value:window.savedHandler,writable:true,configurable:true})",
    ] {
        let kept = provider_document(
            &bundle,
            &configuration,
            &[&format!(
                "function(){{window.savedHandler=()=>true;{write}}}"
            )],
            "S.helper=typeof listAttachments;S.kept=window.onerror===window.savedHandler",
        )
        .await;
        assert_eq!(kept["helper"], "function", "{write}");
        assert_eq!(kept["kept"], true, "{write}");
    }
    let thrown = provider_document(
        &bundle,
        &configuration,
        &["(()=>{window.onerror=()=>true;throw new TypeError('expression failure')})()"],
        "S.helper=typeof listAttachments",
    )
    .await;
    assert_eq!(thrown["helper"], "string");
}

/// Install supplied providers and inspect a following classic script under exact hashes.
async fn provider_document(
    bundle: &Bundle,
    configuration: &Value,
    providers: &[&str],
    probe: &str,
) -> Value {
    let runtime = Runtime::new(bundle.boot(), configuration, ID, providers);
    let html = format!(
        "<!DOCTYPE html><script>{OBSERVER}</script>{}<script>{probe}</script>",
        runtime.markup
    );
    let mut texts = vec![OBSERVER, runtime.boot.as_str(), probe];
    texts.extend(runtime.providers.iter().map(String::as_str));
    let policy = format!(
        "script-src 'self' {} 'wasm-unsafe-eval'",
        digests(&texts).await
    );
    let (state, _) = observe(
        &html,
        Mount {
            policy: &policy,
            ..Mount::default()
        },
    )
    .await;
    receipt(&format!("provider state={state}"));
    state
}

/// Run each supplied body in a fresh isolated realm with an exact input-hash policy.
async fn run_programs(rows: Vec<(&str, Value)>) {
    let bundle = Bundle::load();
    for (code, expected) in rows {
        run_program(&bundle, code, &expected, COMPLETION).await;
    }
}

/// Compile, mount and inspect one program through the retained qualification boundary.
async fn run_program(bundle: &Bundle, code: &str, expected: &Value, provider: &str) -> Value {
    let text = program(code);
    let mut configuration = config(&["provider", "attachments"]);
    configuration["programCapture"] = text.capture().into();
    configuration["globals"]["__maestro0"] = 1.into();
    let runtime = Runtime::new(bundle.boot(), &configuration, ID, &[provider]);
    let policy = format!(
        "script-src 'self' {} 'wasm-unsafe-eval'",
        digests(&[&runtime.boot, &runtime.providers[0], text.code()]).await
    );
    let mount = Mount {
        policy: &policy,
        ..Mount::default()
    };
    let (state, _) = observe(&wrap(&runtime.markup, text.code()), mount).await;
    receipt(&format!("program input={code:?} state={state}"));
    check_program(&state, expected);
    state
}

/// The supplied sandbox URL writes the same document and keeps first-script access.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_supplied_url_installs_before_first_classic_scripts() {
    let bundle = Bundle::load();
    let runtime = repeated(bundle.boot());
    let policy = format!(
        "script-src {} 'wasm-unsafe-eval'",
        artifact_hashes(&runtime).await
    );
    let prefix = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">");
    let mount = Mount {
        url: true,
        ..Mount::default()
    };
    let (state, _) = observe(&artifact(&prefix, &runtime), mount).await;
    receipt(&format!("url state={state}"));
    assert_eq!(fields(&state, INSTALLED), installed(&repeated_trace()));
    assert_eq!(
        fields(&state, &["head", "scaffold"]),
        json!({"head":"meta,script","scaffold":0})
    );
}

/// The served sandbox page writes a load message only when its frame's parent sent it.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_supplied_url_loads_only_from_its_parent() {
    loads_only_from_parent("src", "/sandbox.html", "sandbox-ready").await;
}

/// One self-contained document runs with every network and resource directive refused.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_offline_document_needs_no_resource() {
    let bundle = Bundle::load();
    let runtime = Runtime::new(bundle.boot(), &config(&["attachments"]), ID, &[]);
    let policy = format!(
        "default-src 'none'; script-src {} 'wasm-unsafe-eval'",
        artifact_hashes(&runtime).await
    );
    let html = artifact(
        &format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">"),
        &runtime,
    );
    let mount = Mount {
        url: true,
        ..Mount::default()
    };
    let (state, _) = observe(&html, mount).await;
    receipt(&format!("offline document={} state={state}", html.len()));
    assert_eq!(fields(&state, INSTALLED), installed(&Value::Null));
}

/// Observe the repeated runtime under one inherited policy row.
async fn policy_row(html: &str, label: &str, policy: &str, trusted_default: bool) -> Value {
    let mount = Mount {
        policy,
        trusted_default,
        ..Mount::default()
    };
    let (state, _) = observe(html, mount).await;
    receipt(&format!("policy {label}: {state}"));
    fields(&state, &["first", "trace", "scaffold"])
}

/// A supplied provider turns Trusted Types to `none`; later selections still install.
async fn in_frame_none(bundle: &Bundle) -> Value {
    let none = Runtime::new(
        bundle.boot(),
        &config(&["provider", "attachments", "provider"]),
        ID,
        &[TRUSTED_NONE, TRACE_PROVIDER],
    );
    let policy = format!(
        "script-src 'self' {} 'wasm-unsafe-eval'",
        artifact_hashes(&none).await
    );
    policy_row(&artifact("", &none), "in-frame none", &policy, false).await
}

/// Hash admission, refusal and Trusted Types rows of the parser-blocking path.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_policy_matrix_keeps_hash_and_trusted_types_results() {
    let bundle = Bundle::load();
    let runtime = repeated(bundle.boot());
    let html = artifact("", &runtime);
    let all = artifact_hashes(&runtime).await;
    let authored = digests(&[HEAD, BODY]).await;
    let encoded = encode(&runtime.providers[0]);
    let encoded = digests(&[MUTED, &runtime.boot, &encoded, HEAD, BODY]).await;
    let muted = artifact(&format!("<script>{MUTED}</script>"), &runtime);
    let strict =
        format!("script-src 'self' {all} 'wasm-unsafe-eval'; require-trusted-types-for 'script'");
    let mut observed = vec![
        policy_row(
            &html,
            "matched",
            &format!("script-src 'self' {all} 'wasm-unsafe-eval'"),
            false,
        )
        .await,
        policy_row(
            &html,
            "unmatched loader",
            &format!("script-src 'self' {authored} 'wasm-unsafe-eval'"),
            false,
        )
        .await,
        policy_row(&html, "inherited HTML-only default", &strict, true).await,
        policy_row(
            &artifact(STRICT, &runtime),
            "frame strict without default",
            &format!("script-src 'self' {all} 'wasm-unsafe-eval'"),
            false,
        )
        .await,
    ];
    observed.push(in_frame_none(&bundle).await);
    let full = json!({"first":"function","trace":repeated_trace(),"scaffold":0});
    assert_eq!(
        Value::from(observed),
        json!([
            full,
            {"first":"undefined","trace":null,"scaffold":1},
            full,
            full,
            {"first":"function","trace":[format!("provider:{ID}:function")],"scaffold":0},
        ])
    );
    // A provider the policy blocks never reports its completion, so installation stops there, as
    // it would inside one runtime script: the configuration block and the runtime script stay.
    let mount = Mount {
        policy: &format!("script-src 'self' {encoded} 'wasm-unsafe-eval'"),
        ..Mount::default()
    };
    let (blocked, _) = observe(&muted, mount).await;
    receipt(&format!("policy encoded provider markup: {blocked}"));
    assert_eq!(
        fields(&blocked, &["first", "trace", "head"]),
        json!({"first":"function","trace":null,"head":"script,script,script,script"})
    );
}

/// A refused module compilation surfaces its error and leaves helpers absent.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_refused_compilation_leaves_helpers_absent() {
    let bundle = Bundle::load();
    let probe = "S.first=typeof listAttachments";
    let runtime = Runtime::new(bundle.boot(), &config(&["attachments"]), ID, &[]);
    let html = format!(
        "<!DOCTYPE html><script>{OBSERVER}</script><head>{}<script>{probe}</script></head><body></body>",
        runtime.markup
    );
    let policy = format!(
        "script-src 'self' {}",
        digests(&[OBSERVER, &runtime.boot, probe]).await
    );
    let mount = Mount {
        policy: &policy,
        ..Mount::default()
    };
    let (state, _) = observe(&html, mount).await;
    receipt(&format!("refusal no wasm permission: {state}"));
    assert_eq!(state["first"], "undefined");
    let errors = state["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].as_str().unwrap().starts_with("CompileError: "));
}

/// Records uncaught errors, then every listener, timer and observer the frame registers while recording.
///
/// A call from code with no script URL goes to `outside`, not `registered`: the policy admits no
/// eval, so only the test driver's own evaluations run without one, and the driver may install its
/// listeners at any point of the document.
const RECORDER: &str = "window.S={errors:[],registered:[],outside:[],recording:true};addEventListener('error',e=>{S.errors.push(e.error?e.error.name+': '+e.error.message:'muted: '+e.message);e.preventDefault()});const caller=()=>{const p=Error.prepareStackTrace;Error.prepareStackTrace=(_,s)=>s;const s=new Error().stack;Error.prepareStackTrace=p;return s[2].getScriptNameOrSourceURL()};for(const[o,k]of[[EventTarget.prototype,'addEventListener'],[window,'setTimeout'],[window,'setInterval'],[window,'requestAnimationFrame'],[window,'requestIdleCallback'],[window,'queueMicrotask'],[MutationObserver.prototype,'observe']]){const f=o[k];o[k]=function(...a){if(S.recording)(caller()?S.registered:S.outside).push(typeof a[0]==='string'?k+':'+a[0]:k);return f.apply(this,a)}}";
/// Ends recording after the loader and reads the helpers and scaffold it left.
const RECORDED: &str = "S.recording=false;S.first=typeof listAttachments;S.config=document.querySelectorAll('script[data-maestro-sandbox]').length;S.trace=self.__sandboxInstallTrace?[...__sandboxInstallTrace]:null";

/// Startup and refused startup register no frame listener, timer or observer; only refusal leaves inert scaffold.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_startup_leaves_no_frame_registrations() {
    let bundle = Bundle::load();
    let mut observed = Vec::new();
    for (label, permission) in [("started", " 'wasm-unsafe-eval'"), ("refused", "")] {
        let runtime = Runtime::new(
            bundle.boot(),
            &config(&["attachments", "provider"]),
            ID,
            &[TRACE_PROVIDER],
        );
        let html = format!(
            "<!DOCTYPE html><html><head><script>{RECORDER}</script>{}<script>{RECORDED}</script></head><body></body></html>",
            runtime.markup
        );
        let policy = format!(
            "script-src 'self' {}{permission}",
            digests(&[RECORDER, &runtime.boot, &runtime.providers[0], RECORDED]).await
        );
        let mount = Mount {
            policy: &policy,
            ..Mount::default()
        };
        let (state, _) = observe(&html, mount).await;
        receipt(&format!("registrations {label}: {state}"));
        let mut row = fields(
            &state,
            &["registered", "first", "config", "scaffold", "trace"],
        );
        row["errors"] = state["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|error| Value::from(error.as_str().unwrap().split(':').next().unwrap()))
            .collect();
        observed.push(row);
    }
    assert_eq!(
        Value::from(observed),
        json!([
            {"registered":[],"first":"function","config":0,"scaffold":0,"trace":[format!("provider:{ID}:function")],"errors":[]},
            {"registered":[],"first":"undefined","config":1,"scaffold":1,"trace":null,"errors":["CompileError"]},
        ])
    );
}

/// Authored scripts that log their order beside the completion provider.
const ORDERED: [&str; 4] = [
    "S.log.push('head:'+typeof complete)",
    "S.log.push('module')",
    "S.log.push('body')",
    "S.log.push('trailing')",
];
/// The completion signal an HTML artifact appends after its whole source.
const SUFFIX: &str = "window.__maestroComplete";

/// The appended completion signal runs after source following `</html>` and before module scripts.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_completion_follows_trailing_source() {
    let bundle = Bundle::load();
    let runtime = Runtime::new(bundle.boot(), &config(&["provider"]), ID, &[COMPLETION]);
    let [head, module, body, trailing] = ORDERED;
    let html = format!(
        "<!DOCTYPE html><html><head>{}<script>{head}</script><script type=\"module\">{module}</script></head><body><script>{body}</script></body></html><script>{trailing}</script><script>{SUFFIX}</script>",
        runtime.markup
    );
    let mut texts = vec![runtime.boot.as_str(), runtime.providers[0].as_str(), SUFFIX];
    texts.extend(ORDERED);
    let policy = format!(
        "script-src 'self' {} 'wasm-unsafe-eval'",
        digests(&texts).await
    );
    let mount = Mount {
        policy: &policy,
        ..Mount::default()
    };
    let (state, _) = observe(&html, mount).await;
    receipt(&format!("completion state={state}"));
    assert_eq!(
        fields(&state, &["log", "body"]),
        json!({"log":["head:function","body","trailing","complete","module"],"body":"script,script,script"})
    );
}

/// Authored scripts declare the names the generated runtime is built from, and none leaks onto the window.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_leaves_authored_global_names_free() {
    let bundle = Bundle::load();
    let probe = "const Module={own:1};const abort=2;const preRun=3;const ENVIRONMENT_IS_WEB=4;const updateMemoryViews=5;S.names=[Module.own,abort,preRun,ENVIRONMENT_IS_WEB,updateMemoryViews].join();S.helper=typeof listAttachments;S.leaked=['Module','abort','preRun','postRun','ENVIRONMENT_IS_WEB','updateMemoryViews','createWasm','handleError','HEAP8','HEAPU8','wasmExports','wasmMemory'].filter(name=>Object.hasOwn(window,name))";
    let state = provider_document(&bundle, &config(&["attachments"]), &[], probe).await;
    assert_eq!(
        fields(&state, &["names", "helper", "leaked"]),
        json!({"names":"1,2,3,4,5", "helper":"function", "leaked":[]})
    );
    assert_eq!(state["errors"], json!([]));
}

/// Stand-in for the extension runtime, installed before the generated script.
const EXTENSION: &str = "window.S={started:true,sent:[]};window.chrome={runtime:{sendMessage:async m=>{S.sent.push(m);return{received:m.type}}}}";
/// Uses the two globals the user-script script installs.
const USER_SCRIPT_PROBE: &str = "window.done=(async()=>{S.reply=await sendRuntimeMessage({type:'custom',n:1,sandboxId:'caller'});onCompleted(()=>0);S.shape=[typeof sendRuntimeMessage,typeof onCompleted,__completionCallbacks.length]})()";

/// The user-script script sends through the extension runtime with its identifier last and
/// registers completion callbacks, from the same generated bundle as the frame channel.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_user_script_role_sends_through_the_extension_runtime() {
    let script = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
        context: RuntimeMessageContext::UserScript,
        sandbox_id: ID,
    })
    .unwrap();
    let html = format!(
        "<!DOCTYPE html><html><head><script>{EXTENSION}</script><script>{script}</script></head><body><script>{USER_SCRIPT_PROBE}</script></body></html>"
    );
    let (state, _) = observe(&html, Mount::default()).await;
    receipt(&format!("user-script state={state}"));
    assert_eq!(
        fields(&state, &["sent", "reply", "shape"]),
        json!({
            "sent": [{"type":"custom","n":1,"sandboxId":ID}],
            "reply": {"received":"custom"},
            "shape": ["function","function",1],
        })
    );
}
