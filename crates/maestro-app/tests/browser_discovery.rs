//! Local provider discovery through the application boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::browser::utils::model_discovery::*;
use maestro_app::{DiagnosticErrorInfo, Fetch, FetchError, HttpRequest, HttpResponse, Model};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::support;
use support::browser_providers::run;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict recorded reply.
struct Reply {
    /// Controlled response status.
    status: u16,
    /// Exact response body bytes.
    bytes: Vec<u8>,
    /// Whether the connection fails.
    drop: bool,
}
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
/// Strict recorded request.
struct Request {
    /// Observed HTTP method.
    method: String,
    /// Observed request path.
    path: String,
    /// Selected request headers.
    headers: BTreeMap<String, String>,
    /// Observed JSON request body.
    body: Option<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict recorded case.
struct Case {
    /// Case or model name.
    name: String,
    /// Provider identifier.
    provider: String,
    /// Optional or supplied test credential.
    key: Option<String>,
    /// Authored base with an origin placeholder.
    base: String,
    /// Controlled list and detail responses.
    replies: BTreeMap<String, Reply>,
    /// Recorded expected result.
    expected: Vec<Model>,
    /// Expected failure category, absent on success.
    error: Option<String>,
    /// Ordered recorded requests.
    requests: Vec<Request>,
    /// Ordered diagnostic prefixes.
    diagnostics: Vec<String>,
}
/// Read the strict discovery corpus.
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/browser_providers/discovery.json")).unwrap()
}
/// Select the recorded discovery operation.
fn provider(name: &str) -> DiscoveryProvider {
    match name {
        "llama.cpp" => DiscoveryProvider::LlamaCpp,
        "vllm" => DiscoveryProvider::Vllm,
        "ollama" => DiscoveryProvider::Ollama,
        _ => panic!("unknown provider"),
    }
}
/// Produce controlled HTTP response bytes or a connection error.
fn response(reply: &Reply) -> Result<HttpResponse, FetchError> {
    if reply.drop {
        return Err(FetchError::Connection(DiagnosticErrorInfo::error(
            "connection closed",
        )));
    }
    let bytes = reply.bytes.clone();
    let body: maestro_app::HttpBody = Box::pin(futures_util::stream::iter([Ok(bytes)]));
    Ok(HttpResponse {
        status: reply.status,
        status_text: "Fixture status".into(),
        headers: BTreeMap::new(),
        body,
    })
}
/// Complete one controlled response without further asynchronous effects.
fn ready_response(bytes: Vec<u8>) -> maestro_app::BoxFuture<Result<HttpResponse, FetchError>> {
    let result = response(&Reply {
        status: 200,
        bytes,
        drop: false,
    });
    Box::pin(async move { result })
}
/// Observe the selected request fields.
fn request(request: HttpRequest) -> Request {
    let url = url::Url::parse(&request.url).unwrap();
    Request {
        method: request.method,
        path: url.path().into(),
        headers: request.headers.into_iter().collect(),
        body: if request.body.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(&request.body).unwrap())
        },
    }
}
/// Compare complete models or the provider-attributed failure category.
fn compare_result(result: Result<Vec<Model>, DiscoveryError>, case: &Case) {
    match (result, case.error.as_deref()) {
        (Ok(models), None) => {
            let expected: Vec<Model> = case
                .expected
                .iter()
                .cloned()
                .map(|mut m| {
                    m.base_url = m.base_url.replace("$ORIGIN", "http://fixture.test:3210");
                    m
                })
                .collect();
            assert!(models == expected, "{}", case.name);
        }
        (Err(error), Some(kind)) => {
            assert_eq!(error.provider, provider(&case.provider));
            assert!(
                matches!(
                    (kind, &error.kind),
                    ("invalid_response", DiscoveryErrorKind::InvalidResponse)
                        | ("transport", DiscoveryErrorKind::Transport(_))
                        | ("http", DiscoveryErrorKind::Http(_))
                ),
                "{}",
                case.name
            );
        }
        _ => panic!("unexpected outcome: {}", case.name),
    }
}
/// Compare complete results and effects with one recorded operation.
async fn replay(mut case: Case) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let diagnostics = Arc::new(Mutex::new(Vec::new()));
    let replies = Arc::new(std::mem::take(&mut case.replies));
    let fetch: Fetch = {
        let requests = Arc::clone(&requests);
        Arc::new(move |input| {
            let input = request(input);
            let name = input
                .body
                .as_ref()
                .and_then(|b| b.get("model"))
                .and_then(Value::as_str)
                .unwrap_or("list");
            let result = response(replies.get(name).unwrap());
            requests.lock().unwrap().push(input);
            Box::pin(async move { result })
        })
    };
    let log: DiscoveryLog = {
        let diagnostics = Arc::clone(&diagnostics);
        Arc::new(move |diagnostic: DiscoveryDiagnostic<'_>| {
            let text = diagnostic.to_string();
            let prefix = match diagnostic {
                DiscoveryDiagnostic::SkippedWithoutTools { .. } => text,
                DiscoveryDiagnostic::DetailFailed { .. } | DiscoveryDiagnostic::Failed(_) => {
                    text.split_once(": ").unwrap().0.to_owned() + ":"
                }
            };
            diagnostics.lock().unwrap().push(prefix);
        })
    };
    let base = case.base.replace("$ORIGIN", "http://fixture.test:3210");
    let options = DiscoveryOptions {
        fetch,
        log,
        connect_web_socket: maestro_app::default_web_socket(),
    };
    let result = if case.provider == "ollama" {
        discover_ollama_models(&base, case.key.as_deref(), &options).await
    } else if case.provider == "vllm" {
        discover_vllm_models(&base, case.key.as_deref(), &options).await
    } else {
        discover_llama_cpp_models(&base, case.key.as_deref(), &options).await
    };
    compare_result(result, &case);
    assert!(*requests.lock().unwrap() == case.requests, "{}", case.name);
    assert_eq!(
        *diagnostics.lock().unwrap(),
        case.diagnostics,
        "{}",
        case.name
    );
}
#[test]
/// Llama discovery preserves order and defaults.
fn llama_discovery_preserves_order_and_defaults() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            c.name == "llama_defaults_order_duplicates" || c.name == "llama.cpp_falsy_limits"
        }) {
            replay(case).await;
        }
    });
}

#[test]
/// Vllm discovery caps tokens per model.
fn vllm_discovery_caps_tokens_per_model() {
    run(async {
        for case in cases()
            .into_iter()
            .filter(|c| c.name == "vllm_caps_order_duplicates" || c.name == "vllm_falsy_limits")
        {
            replay(case).await;
        }
    });
}

#[test]
/// Ollama filters tools without reordering results.
fn ollama_filters_tools_without_reordering_results() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            c.name == "ollama_order_duplicates_partial"
                || c.name == "ollama_empty"
                || c.name == "ollama_skip_before_metadata"
                || c.name.starts_with("ollama_capabilities_")
                || [
                    "ollama_detail_shape_82",
                    "ollama_detail_shape_83",
                    "ollama_detail_shape_84",
                    "ollama_detail_shape_86",
                ]
                .contains(&c.name.as_str())
        }) {
            replay(case).await;
        }
    });
}

#[test]
/// Discovery rejects invalid responses.
fn discovery_rejects_invalid_responses() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            [
                "_missing",
                "_null_data",
                "_object_data",
                "_null_root",
                "_array_root",
                "_null_row",
                "_unknown_fields",
            ]
            .iter()
            .any(|suffix| c.name.ends_with(suffix))
                || (c.name.starts_with("ollama_list_shape_") && c.name != "ollama_list_shape_92")
        }) {
            replay(case).await;
        }
    });
}

#[test]
/// Discovery attributes network http and decode failures.
fn discovery_attributes_network_http_and_decode_failures() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            c.name.contains("_failure_")
                || c.name.ends_with("_bad_json")
                || c.name.starts_with("ollama_list_http_")
                || c.name == "ollama_list_network"
        }) {
            replay(case).await;
        }
        let cause = FetchError::Connection(DiagnosticErrorInfo::error("body read failed"));
        let fetch: Fetch = Arc::new(move |_| {
            let cause = cause.clone();
            Box::pin(async {
                Ok(HttpResponse {
                    status: 200,
                    status_text: String::new(),
                    headers: BTreeMap::new(),
                    body: Box::pin(futures_util::stream::iter([Err(cause)])),
                })
            })
        });
        let error = discover_vllm_models(
            "http://fixture.test",
            None,
            &DiscoveryOptions {
                connect_web_socket: maestro_app::default_web_socket(),
                fetch,
                log: Arc::new(|_| {}),
            },
        )
        .await
        .err()
        .unwrap();
        assert!(
            std::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<FetchError>()
                .is_some()
        );
        assert!(matches!(error.kind, DiscoveryErrorKind::Transport(_)));
    });
}

#[test]
/// Discovery decodes utf8 before json.
fn discovery_decodes_utf8_before_json() {
    run(async {
        for case in cases()
            .into_iter()
            .filter(|c| c.name.ends_with("_bom") || c.name.ends_with("_bad_utf8"))
        {
            replay(case).await;
        }
    });
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict recorded Host case.
struct HostCase {
    /// Authored server host.
    host: String,
    /// Recorded expected result.
    expected: Option<String>,
}

#[test]
/// Ollama host selection matches requests.
fn ollama_host_selection_matches_requests() {
    run(async {
        let cases: Vec<HostCase> =
            serde_json::from_str(include_str!("fixtures/browser_providers/hosts.json")).unwrap();
        for case in cases {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let observed = Arc::clone(&requests);
            let options = DiscoveryOptions {
                connect_web_socket: maestro_app::default_web_socket(),
                fetch: Arc::new(move |request| {
                    observed.lock().unwrap().push(request.url);
                    ready_response(br#"{"models":[]}"#.to_vec())
                }),
                log: Arc::new(|_| {}),
            };
            let result = discover_ollama_models(&case.host, None, &options).await;
            if let Some(host) = case.expected {
                assert!(result.unwrap().is_empty());
                assert_eq!(*requests.lock().unwrap(), [format!("{host}/api/tags")]);
            } else {
                assert!(matches!(
                    result.err().unwrap().kind,
                    DiscoveryErrorKind::InvalidUrl
                ));
                assert!(requests.lock().unwrap().is_empty());
            }
        }
    });
}

#[test]
/// Ollama details fail independently.
fn ollama_details_fail_independently() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            c.name == "ollama_mixed_invalid_details" || c.name == "ollama_detail_shape_85"
        }) {
            replay(case).await;
        }
    });
}

#[test]
/// Ollama context uses architecture and decimal prefix.
fn ollama_context_uses_architecture_and_decimal_prefix() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            (c.name.starts_with("ollama_context_")
                && ![
                    "ollama_context_nel",
                    "ollama_context_bad",
                    "ollama_context_wrong_bool",
                ]
                .contains(&c.name.as_str()))
                || c.name.contains("_architecture")
                || c.name == "ollama_no_info"
        }) {
            replay(case).await;
        }
        let fetch: Fetch = Arc::new(|input| {
            let bytes = if input.url.ends_with("/tags") {
                br#"{"models":[{"name":"leading-zero"}]}"#.to_vec()
            } else {
                br#"{"capabilities":["tools"],"model_info":{".context_length":"-0012 tokens"}}"#
                    .to_vec()
            };
            Box::pin(async move {
                response(&Reply {
                    status: 200,
                    bytes,
                    drop: false,
                })
            })
        });
        let models = discover_ollama_models(
            "http://fixture.test",
            None,
            &DiscoveryOptions {
                connect_web_socket: maestro_app::default_web_socket(),
                fetch,
                log: Arc::new(|_| {}),
            },
        )
        .await
        .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].context_window.to_bits(), (-12.0_f64).to_bits());
        assert_eq!(models[0].max_tokens.to_bits(), (-120.0_f64).to_bits());
    });
}

#[test]
/// Discovery rejects unusable model fields.
fn discovery_rejects_unusable_model_fields() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            ["_wrong_id", "_missing_id", "_wrong_limit", "_row_array"]
                .iter()
                .any(|suffix| c.name.ends_with(suffix))
                || [
                    "ollama_list_shape_92",
                    "ollama_context_nel",
                    "ollama_context_bad",
                    "ollama_context_wrong_bool",
                ]
                .contains(&c.name.as_str())
        }) {
            replay(case).await;
        }
    });
}

#[test]
/// Discovery keeps last selected json member.
fn discovery_keeps_last_selected_json_member() {
    run(async {
        for case in cases()
            .into_iter()
            .filter(|c| c.name.ends_with("_duplicate_selected"))
        {
            replay(case).await;
        }
    });
}

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// Read one native request including its JSON body.
fn read_http(stream: &mut TcpStream) -> Request {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
    let head = String::from_utf8(bytes).unwrap();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap().split_whitespace();
    let method = first.next().unwrap().to_owned();
    let path = first.next().unwrap().to_owned();
    let all: BTreeMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length = all
        .get("content-length")
        .map_or(0, |value| value.parse().unwrap());
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    let headers = all
        .into_iter()
        .filter(|(key, _)| {
            ["content-type", "authorization"].contains(&key.as_str())
                || (path.contains("/api/") && key == "accept")
        })
        .collect();
    Request {
        method,
        path,
        headers,
        body: if body.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(&body).unwrap())
        },
    }
}
/// Complete or drop one native response.
fn write_http(stream: &mut TcpStream, reply: &Reply) {
    if reply.drop {
        return;
    }
    write!(stream, "HTTP/1.1 {} Fixture status\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", reply.status, reply.bytes.len()).unwrap();
    stream.write_all(&reply.bytes).unwrap();
}
/// Serve the expected requests, optionally withholding all detail responses.
fn server(
    replies: BTreeMap<String, Reply>,
    count: usize,
    barrier: bool,
) -> (String, std::thread::JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let mut observed = Vec::new();
        let mut pending = Vec::new();
        for _ in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http(&mut stream);
            let name = request
                .body
                .as_ref()
                .and_then(|body| body.get("model"))
                .and_then(Value::as_str)
                .unwrap_or("list")
                .to_owned();
            if barrier && name != "list" {
                pending.push((stream, name));
            } else {
                write_http(&mut stream, replies.get(&name).unwrap());
            }
            observed.push(request);
        }
        for (mut stream, name) in pending.into_iter().rev() {
            write_http(&mut stream, replies.get(&name).unwrap());
        }
        observed
    });
    (base, thread)
}
/// Compare one native exchange with its recorded operation.
async fn native(case: Case, barrier: bool) {
    let count = case.requests.len();
    let (origin, server) = server(case.replies, count, barrier);
    let base = case.base.replace("$ORIGIN", &origin);
    let options = DiscoveryOptions {
        connect_web_socket: maestro_app::default_web_socket(),
        fetch: maestro_app::default_fetch(),
        log: Arc::new(|_| panic!("unexpected diagnostic")),
    };
    let models = match provider(&case.provider) {
        DiscoveryProvider::Ollama => {
            discover_ollama_models(&base, case.key.as_deref(), &options).await
        }
        DiscoveryProvider::LlamaCpp => {
            discover_llama_cpp_models(&base, case.key.as_deref(), &options).await
        }
        DiscoveryProvider::Vllm => discover_vllm_models(&base, case.key.as_deref(), &options).await,
        DiscoveryProvider::Lmstudio => panic!("not an HTTP fixture"),
    }
    .unwrap();
    let expected: Vec<Model> = case
        .expected
        .into_iter()
        .map(|mut m| {
            m.base_url = m.base_url.replace("$ORIGIN", &origin);
            m
        })
        .collect();
    assert!(models == expected, "{}", case.name);
    let observed = server.join().unwrap();
    if barrier {
        assert!(observed[0] == case.requests[0], "listing request");
        // Requests may arrive in any order; responses were withheld until every detail arrived.
        let mut observed: Vec<_> = observed.into_iter().skip(1).collect();
        let mut expected: Vec<_> = case.requests.into_iter().skip(1).collect();
        observed.sort_by_key(|request| request.body.as_ref().unwrap().to_string());
        expected.sort_by_key(|request| request.body.as_ref().unwrap().to_string());
        assert!(observed == expected, "all detail requests");
    } else {
        assert!(observed == case.requests, "{}", case.name);
    }
    assert!(case.error.is_none());
    assert!(case.diagnostics.is_empty());
}
#[test]
/// Discovery routes http and auth.
fn discovery_routes_http_and_auth() {
    run(async {
        for case in cases().into_iter().filter(|c| {
            c.name.contains("_auth_")
                || c.name.ends_with("_trailing_slash")
                || c.name == "ollama_ignored_key"
                || c.name == "ollama_prefix_path"
        }) {
            native(case, false).await;
        }
    });
}

#[test]
/// Ollama details run concurrently.
fn ollama_details_run_concurrently() {
    run(async {
        let case = cases()
            .into_iter()
            .find(|case| case.name == "ollama_concurrent")
            .unwrap();
        native(case, true).await;
    });
}

#[test]
/// Discovery dispatches three provider types.
fn discovery_dispatches_three_provider_types() {
    run(async {
        for provider in [
            DiscoveryProvider::Ollama,
            DiscoveryProvider::LlamaCpp,
            DiscoveryProvider::Vllm,
        ] {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let observed = Arc::clone(&requests);
            let fetch: Fetch = Arc::new(move |input| {
                observed.lock().unwrap().push(input.url.clone());
                let bytes = match input.url.rsplit('/').next().unwrap() {
                "tags" => br#"{"models":[{"name":"dispatch-ollama"}]}"#.to_vec(),
                "show" => br#"{"capabilities":["tools"],"model_info":{".context_length":700}}"#.to_vec(),
                "models" => br#"{"data":[{"id":"dispatch-completion","context_length":900,"max_model_len":1100}]}"#.to_vec(),
                _ => panic!("wrong endpoint"),
            };
                ready_response(bytes)
            });
            let models = discover_models(
                provider,
                "http://fixture.test",
                None,
                &DiscoveryOptions {
                    connect_web_socket: maestro_app::default_web_socket(),
                    fetch,
                    log: Arc::new(|_| panic!("unexpected diagnostic")),
                },
            )
            .await
            .unwrap();
            assert_eq!(models.len(), 1);
            match provider {
                DiscoveryProvider::Lmstudio => panic!("not an HTTP fixture"),
                DiscoveryProvider::Ollama => {
                    assert_eq!(models[0].id, "dispatch-ollama");
                    assert_eq!(models[0].context_window.to_bits(), 700.0_f64.to_bits());
                    assert_eq!(
                        *requests.lock().unwrap(),
                        [
                            "http://fixture.test:80/api/tags",
                            "http://fixture.test:80/api/show"
                        ]
                    );
                }
                DiscoveryProvider::LlamaCpp => {
                    assert_eq!(models[0].id, "dispatch-completion");
                    assert_eq!(models[0].context_window.to_bits(), 900.0_f64.to_bits());
                    assert_eq!(*requests.lock().unwrap(), ["http://fixture.test/v1/models"]);
                }
                DiscoveryProvider::Vllm => {
                    assert_eq!(models[0].id, "dispatch-completion");
                    assert_eq!(models[0].context_window.to_bits(), 1100.0_f64.to_bits());
                    assert_eq!(*requests.lock().unwrap(), ["http://fixture.test/v1/models"]);
                }
            }
        }
    });
}

#[test]
/// Discovery diagnostics preserve call boundaries.
fn discovery_diagnostics_preserve_call_boundaries() {
    run(async {
        let transcript = Arc::new(Mutex::new(Vec::new()));
        let log: DiscoveryLog = {
            let transcript = Arc::clone(&transcript);
            Arc::new(move |diagnostic: DiscoveryDiagnostic<'_>| {
                let text = diagnostic.to_string();
                transcript.lock().unwrap().push(text);
            })
        };
        let fetch: Fetch = Arc::new(|input| {
            let body = if input.url.ends_with("/tags") {
                br#"{"models":[{"name":"skipped"},{"name":"failed"}]}"#.to_vec()
            } else if serde_json::from_slice::<Value>(&input.body).unwrap()["model"] == "skipped" {
                br#"{"capabilities":[]}"#.to_vec()
            } else {
                b"{".to_vec()
            };
            Box::pin(async move {
                response(&Reply {
                    status: 200,
                    bytes: body,
                    drop: false,
                })
            })
        });
        let models = discover_ollama_models(
            "http://fixture.test",
            None,
            &DiscoveryOptions {
                connect_web_socket: maestro_app::default_web_socket(),
                fetch,
                log: Arc::clone(&log),
            },
        )
        .await
        .unwrap();
        assert!(models.is_empty());
        {
            let transcript = transcript.lock().unwrap();
            assert_eq!(transcript.len(), 2);
            assert_eq!(
                transcript[0],
                "Skipping model skipped: does not support tools"
            );
            assert!(transcript[1].starts_with("Failed to fetch details for model failed:"));
        }
        failure_diagnostics(&transcript, log).await;
    });
}

/// Observe each whole-operation failure only within its invocation's transcript slice.
async fn failure_diagnostics(transcript: &Arc<Mutex<Vec<String>>>, log: DiscoveryLog) {
    for provider in [
        DiscoveryProvider::Ollama,
        DiscoveryProvider::LlamaCpp,
        DiscoveryProvider::Vllm,
    ] {
        let start = transcript.lock().unwrap().len();
        let fetch: Fetch = Arc::new(|_| Box::pin(async { Err(FetchError::Timeout) }));
        let error = discover_models(
            provider,
            "http://fixture.test",
            None,
            &DiscoveryOptions {
                connect_web_socket: maestro_app::default_web_socket(),
                fetch,
                log: Arc::clone(&log),
            },
        )
        .await
        .err()
        .unwrap();
        assert!(matches!(
            error.kind,
            DiscoveryErrorKind::Transport(ref cause) if matches!(cause.as_ref(), FetchError::Timeout)
        ));
        let transcript = transcript.lock().unwrap();
        assert_eq!(transcript.len(), start + 1);
        assert!(transcript[start].starts_with(&format!("Failed to discover {provider} models:")));
    }
}
