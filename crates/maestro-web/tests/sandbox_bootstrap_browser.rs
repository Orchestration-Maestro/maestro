//! Prototype runtime bootstrap qualification in isolated Chromium frames.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::sandbox_bootstrap_support::{
    Bundle, Mount, OBSERVER, Runtime, digests, encode, json as native_json, observe, padded,
    program, receipt, synchronous_limit, wrap,
};
use serde_json::{Value, json};

/// Sandbox identifier carried through configuration and provider invocations.
const ID: &str = "probe:\"雪]";
/// First authored head script: helper type, listing and installation trace.
const HEAD: &str = "window.S={first:typeof listAttachments,count:typeof listAttachments==='function'?listAttachments().length:-1,trace:self.__sandboxInstallTrace?[...__sandboxInstallTrace]:null,id:self.sandboxId}";
/// First authored body script: helper type and live global reads.
const BODY: &str = "S.second=typeof listAttachments;if(S.second==='function'){attachments.push({id:'b',fileName:'b.txt',mimeType:'text/plain',size:2});S.live=listAttachments().map(a=>a.id).join()}";
/// Supplied provider recording its own position in the installation trace.
const TRACE_PROVIDER: &str =
    "function(id){__sandboxInstallTrace.push('provider:'+id+':'+typeof listAttachments)}";
/// Supplied provider that turns Trusted Types to `none` inside the frame.
const TRUSTED_NONE: &str = "function(){const m=document.createElement('meta');m.httpEquiv='Content-Security-Policy';m.content=\"require-trusted-types-for 'script'; trusted-types 'none'\";document.head.append(m)}";
/// Supplied completion provider for asynchronous programs.
const COMPLETION: &str = "function(){window.S={log:[]};window.done=new Promise(r=>window.complete=async(error,value)=>{S.log.push('complete');S.error=error;S.value=value;r()})}";
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
fn repeated(boot: String) -> Runtime {
    Runtime::new(
        boot,
        &config(&["attachments", "provider", "attachments"]),
        ID,
        &[TRACE_PROVIDER],
    )
}

/// The trace of [`repeated`] when every selection installs.
fn repeated_trace() -> Value {
    json!([
        "attachments:object",
        format!("provider:{ID}:function"),
        "attachments:object"
    ])
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

/// The parser-blocking loading script's synchronous compilation limit on the main thread.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_measures_synchronous_compilation_limit() {
    let bundle = Bundle::load().await;
    receipt(&format!(
        "bundle wasm={} glue={} blocking-script={}",
        bundle.wasm.len(),
        bundle.glue.len(),
        bundle.blocking(&bundle.wasm).len()
    ));
    let lower = bundle.glue.to_ascii_lowercase();
    assert!(!lower.contains("</script") && !lower.contains("<!--"));
    let (largest, refused) = synchronous_limit(&bundle.wasm);
    receipt(&format!(
        "sync-compile largest={largest} refused={refused:?}"
    ));
    let (first, error) = refused.expect("a refused size below the search limit");
    assert_eq!(first, largest + 1);
    assert!(error.starts_with("RangeError: "), "{error}");
    assert!(largest >= bundle.wasm.len());
}

/// Real-size and limit-size modules install every selection before the first classic scripts.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_installs_selections_before_first_classic_scripts() {
    let bundle = Bundle::load().await;
    let (largest, _) = synchronous_limit(&bundle.wasm);
    for size in [bundle.wasm.len(), largest] {
        let runtime = repeated(bundle.blocking(&padded(&bundle.wasm, size)));
        let policy = format!(
            "script-src {} 'wasm-unsafe-eval'",
            artifact_hashes(&runtime).await
        );
        let html = artifact("", &runtime);
        let mount = Mount {
            policy: &policy,
            ..Mount::default()
        };
        let (state, _) = observe(&html, mount).await;
        receipt(&format!(
            "install size={size} document={} state={state}",
            html.len()
        ));
        assert_eq!(
            fields(&state, INSTALLED),
            installed(&repeated_trace()),
            "{size}"
        );
        assert_eq!(
            fields(&state, &["head", "scaffold", "origin"]),
            json!({"head":"script","scaffold":0,"origin":"null"})
        );
    }
}

/// Compare one settled program with its expected log, error message, value and import.
fn check_program(state: &Value, expected: &Value) {
    let error = match &state["error"] {
        Value::Object(error) => {
            let stack = error["stack"].as_str().unwrap();
            assert!(stack.contains("TypeError: user failure"), "{stack}");
            json!({"message": error["message"]})
        }
        other => other.clone(),
    };
    let observed =
        json!({"log":state["log"],"error":error,"value":state["value"],"helper":state["helper"]});
    assert_eq!(&observed, expected);
    if state.get("metaUrl").is_some() {
        assert_eq!(state["metaUrl"], state["base"]);
    }
    assert_eq!(
        fields(state, &["body", "scaffold"]),
        json!({"body":"","scaffold":0})
    );
}

/// Asynchronous programs settle through Rust: await, return, throw, imports and callbacks.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_settles_asynchronous_programs_in_rust() {
    let bundle = Bundle::load().await;
    let rows = [
        (
            "S.started=true;window.__completionCallbacks=[ok=>{S.log.push('callback:'+ok)}];S.metaUrl=import.meta.url;S.base=document.baseURI;S.helper=(await import('./helper.js')).answer;await null;return [sandboxId,42];",
            json!({"log":["callback:true","complete"],"error":null,"value":[ID,42],"helper":"helper-ok"}),
        ),
        (
            "S.started=true;window.__completionCallbacks=[ok=>{S.log.push('callback:'+ok)}];throw new TypeError('user failure');",
            json!({"log":["callback:false","complete"],"error":{"message":"user failure"},"value":null,"helper":null}),
        ),
        (
            "S.started=true;window.__completionCallbacks=[()=>{throw Error('callback failure')},ok=>{S.log.push('second:'+ok)}];return 'kept';",
            json!({"log":["complete"],"error":null,"value":"kept","helper":null}),
        ),
    ];
    for (code, expected) in rows {
        let runtime = Runtime::new(
            bundle.blocking(&bundle.wasm),
            &config(&["provider", "attachments"]),
            ID,
            &[COMPLETION],
        );
        let text = program(code);
        let policy = format!(
            "script-src 'self' {} 'wasm-unsafe-eval'",
            digests(&[&runtime.boot, &runtime.providers[0], &text]).await
        );
        let mount = Mount {
            policy: &policy,
            ..Mount::default()
        };
        let (state, _) = observe(&wrap(&runtime.markup, &text), mount).await;
        receipt(&format!("program state={state}"));
        check_program(&state, &expected);
    }
}

/// The supplied sandbox URL writes the same document and keeps first-script access.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_supplied_url_installs_before_first_classic_scripts() {
    let bundle = Bundle::load().await;
    let runtime = repeated(bundle.blocking(&bundle.wasm));
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

/// One self-contained document runs with every network and resource directive refused.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_offline_document_needs_no_resource() {
    let bundle = Bundle::load().await;
    let runtime = Runtime::new(
        bundle.blocking(&bundle.wasm),
        &config(&["attachments"]),
        ID,
        &[],
    );
    let policy = format!(
        "default-src 'none'; script-src {} 'wasm-unsafe-eval'",
        artifact_hashes(&runtime).await
    );
    let html = artifact("", &runtime);
    let mount = Mount {
        policy: &policy,
        ..Mount::default()
    };
    let (state, _) = observe(&html, mount).await;
    receipt(&format!("offline document={} state={state}", html.len()));
    assert_eq!(
        fields(&state, INSTALLED),
        installed(&json!(["attachments:object"]))
    );
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

/// Hash admission, refusal and Trusted Types rows of the parser-blocking path.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_policy_matrix_keeps_hash_and_trusted_types_results() {
    let bundle = Bundle::load().await;
    let runtime = repeated(bundle.blocking(&bundle.wasm));
    let html = artifact("", &runtime);
    let all = artifact_hashes(&runtime).await;
    let authored = digests(&[HEAD, BODY]).await;
    let encoded = encode(&runtime.providers[0]);
    let encoded = digests(&[&runtime.boot, &encoded, HEAD, BODY]).await;
    let strict = format!("script-src {all} 'wasm-unsafe-eval'; require-trusted-types-for 'script'");
    let mut observed = vec![
        policy_row(
            &html,
            "matched",
            &format!("script-src {all} 'wasm-unsafe-eval'"),
            false,
        )
        .await,
        policy_row(
            &html,
            "unmatched loader",
            &format!("script-src {authored} 'wasm-unsafe-eval'"),
            false,
        )
        .await,
        policy_row(
            &html,
            "encoded provider markup",
            &format!("script-src {encoded} 'wasm-unsafe-eval'"),
            false,
        )
        .await,
        policy_row(&html, "inherited HTML-only default", &strict, true).await,
        policy_row(&html, "inherited strict without default", &strict, false).await,
    ];
    let none = Runtime::new(
        bundle.blocking(&bundle.wasm),
        &config(&["provider", "attachments", "provider"]),
        ID,
        &[TRUSTED_NONE, TRACE_PROVIDER],
    );
    let policy = format!(
        "script-src {} 'wasm-unsafe-eval'",
        artifact_hashes(&none).await
    );
    observed.push(policy_row(&artifact("", &none), "in-frame none", &policy, false).await);
    let full = json!({"first":"function","trace":repeated_trace(),"scaffold":0});
    assert_eq!(
        Value::from(observed),
        json!([
            full,
            {"first":"undefined","trace":null,"scaffold":1},
            {"first":"function","trace":["attachments:object","attachments:object"],"scaffold":0},
            full,
            full,
            {"first":"function","trace":["attachments:object",format!("provider:{ID}:function")],"scaffold":0},
        ])
    );
}

/// Refused loading scripts surface their compile errors and leave helpers absent.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_refused_compilation_leaves_helpers_absent() {
    let bundle = Bundle::load().await;
    let (largest, _) = synchronous_limit(&bundle.wasm);
    let probe = "S.first=typeof listAttachments";
    let rows = [
        (
            "over limit",
            padded(&bundle.wasm, largest + 1),
            " 'wasm-unsafe-eval'",
            "RangeError: ",
        ),
        (
            "no wasm permission",
            bundle.wasm.clone(),
            "",
            "CompileError: ",
        ),
    ];
    for (label, wasm, permission, error) in rows {
        let runtime = Runtime::new(bundle.blocking(&wasm), &config(&["attachments"]), ID, &[]);
        let html = format!(
            "<!DOCTYPE html><script>{OBSERVER}</script><head>{}<script>{probe}</script></head><body></body>",
            runtime.markup
        );
        let policy = format!(
            "script-src {}{permission}",
            digests(&[OBSERVER, &runtime.boot, probe]).await
        );
        let mount = Mount {
            policy: &policy,
            ..Mount::default()
        };
        let (state, _) = observe(&html, mount).await;
        receipt(&format!("refusal {label}: {state}"));
        assert_eq!(state["first"], "undefined", "{label}");
        let errors = state["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1, "{label}");
        assert!(errors[0].as_str().unwrap().starts_with(error), "{label}");
    }
}

/// The staging alternative installs before authored scripts but needs an HTML sink.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn bootstrap_staging_alternative_needs_a_trusted_html_sink() {
    let bundle = Bundle::load().await;
    let mut config = config(&["attachments"]);
    config["staged"] = format!(
        "<!DOCTYPE html><html><head><script>{HEAD}</script></head><body><script>{BODY}</script></body></html>"
    )
    .into();
    let boot = bundle.staged();
    let staging = format!(
        "<!DOCTYPE html><html><head><script type=\"application/json\" data-maestro-sandbox>{}</script><script>{boot}</script></head></html>",
        config.to_string().replace('<', "\\u003c")
    );
    let hashes = digests(&[&boot, HEAD, BODY]).await;
    let mut observed = Vec::new();
    for (label, trusted) in [
        ("open", ""),
        ("strict", "; require-trusted-types-for 'script'"),
    ] {
        let policy = format!("script-src {hashes} 'wasm-unsafe-eval'{trusted}");
        let mount = Mount {
            policy: &policy,
            staged: true,
            ..Mount::default()
        };
        let (state, staged) = observe(&staging, mount).await;
        receipt(&format!("staging {label}: {state}"));
        let mut row = fields(&state, &["first", "trace", "second"]);
        row["posted"] = native_json(&staged)["trace"].clone();
        observed.push(row);
    }
    assert_eq!(
        Value::from(observed),
        json!([
            {"first":"function","trace":["attachments:object","staged"],"second":"function","posted":["attachments:object","staged"]},
            {"first":null,"trace":null,"second":null,"posted":["attachments:object","staged","write-refused:TypeError"]},
        ])
    );
}
