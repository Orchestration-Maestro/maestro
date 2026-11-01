//! Public proxy request and stream observations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_models::FetchError;
use serde_json::{Value, json};
use support::proxy::{
    Reply, USAGE, ZERO, canonical_events, canonical_lines, canonical_message, done, exchange, join,
    line, message, run_chunks, run_lines,
};

/// The diagnostic that settles a body ending without a terminal event.
const PREMATURE: &str = "Proxy stream ended before a terminal event";

/// A content-less failure message with the given text.
fn failure(text: &str) -> Value {
    message(&json!([]), "error", Some(text), ZERO)
}
/// Assert a run ended in one error event whose native diagnostic is not an authored one.
fn assert_native_error(run: &support::proxy::Run, prefix: usize) {
    assert_eq!(run.events.len(), prefix + 1);
    assert_eq!(
        run.events[prefix],
        json!({"type": "error", "reason": "error"})
    );
    let text = run.message()["errorMessage"].as_str().unwrap().to_owned();
    assert!(!text.is_empty() && text != PREMATURE, "{text}");
}

/// One request reconstructs every compact event of a single-text response.
#[test]
fn proxy_lf_frames_reconstruct_all_events() {
    support::run_io(proxy_lf_frames_reconstruct_all_events_case());
}
/// Drive the scenario of `proxy_lf_frames_reconstruct_all_events` on the test runtime.
async fn proxy_lf_frames_reconstruct_all_events_case() {
    let run = run_lines(&canonical_lines()).await;
    assert_eq!(run.events, canonical_events());
    assert_eq!(run.message(), canonical_message());
}

/// Success statuses other than 200 are accepted up to the end of the 2xx range.
#[test]
fn proxy_accepts_all_success_statuses() {
    support::run_io(proxy_accepts_all_success_statuses_case());
}
/// Drive the scenario of `proxy_accepts_all_success_statuses` on the test runtime.
async fn proxy_accepts_all_success_statuses_case() {
    for status in [201, 299] {
        let mut reply = Reply::chunks(canonical_lines());
        reply.status = status;
        let (run, _) = exchange(reply).await;
        assert_eq!(run.events, canonical_events(), "{status}");
        assert_eq!(run.message(), canonical_message());
    }
}

/// Lines are processed per LF only; a CR, or a missing LF at the end, never completes a line.
#[test]
fn proxy_lf_only_framing_rejects_sse_shortcuts() {
    support::run_io(proxy_lf_only_framing_rejects_sse_shortcuts_case());
}
/// Drive the scenario of `proxy_lf_only_framing_rejects_sse_shortcuts` on the test runtime.
async fn proxy_lf_only_framing_rejects_sse_shortcuts_case() {
    let crlf: Vec<u8> = canonical_lines()
        .iter()
        .flat_map(|bytes| [&bytes[..bytes.len() - 1], b"\r\n"].concat())
        .collect();
    assert_eq!(run_chunks(vec![crlf]).await.events, canonical_events());
    let mut prefixed = join(&[
        b"data:{\"type\":\"start\"}\n".to_vec(),
        b" data: {\"type\":\"start\"}\nevent: start\n: ping\ndata: \t \r\n".to_vec(),
    ]);
    prefixed.extend(join(&canonical_lines()));
    assert_eq!(run_chunks(vec![prefixed]).await.events, canonical_events());
    for rejected in ["data: {\"type\":\ndata: \"start\"}\n", "data: [DONE]\n"] {
        let run = run_chunks(vec![rejected.as_bytes().to_vec()]).await;
        assert_native_error(&run, 0);
    }
}

/// A body without any LF never produces an event and settles as a premature end.
#[test]
fn proxy_eof_without_terminal_settles_as_error() {
    support::run_io(proxy_eof_without_terminal_settles_as_error_case());
}
/// Drive the scenario of `proxy_eof_without_terminal_settles_as_error` on the test runtime.
async fn proxy_eof_without_terminal_settles_as_error_case() {
    let body: Vec<u8> = join(&canonical_lines())
        .into_iter()
        .map(|byte| if byte == b'\n' { b'\r' } else { byte })
        .collect();
    let run = run_chunks(vec![body]).await;
    assert_eq!(
        run.events,
        vec![json!({"type": "error", "reason": "error"})]
    );
    assert_eq!(run.message(), failure(PREMATURE));
    let empty = run_chunks(vec![]).await;
    assert_eq!(
        empty.events,
        vec![json!({"type": "error", "reason": "error"})]
    );
    assert_eq!(empty.message(), failure(PREMATURE));
    let lines = canonical_lines();
    let partial = run_lines(&lines[..5]).await;
    assert_eq!(partial.events[..5], canonical_events()[..5]);
    assert_eq!(
        partial.events[5],
        json!({"type": "error", "reason": "error"})
    );
    let mut expected = canonical_message();
    expected["stopReason"] = json!("error");
    expected["errorMessage"] = json!(PREMATURE);
    expected["usage"] = serde_json::from_str(ZERO).unwrap();
    assert_eq!(partial.message(), expected);
}

/// A byte-order mark is dropped only at the very start of the stream.
#[test]
fn proxy_decoder_strips_only_stream_leading_bom() {
    support::run_io(proxy_decoder_strips_only_stream_leading_bom_case());
}
/// Drive the scenario of `proxy_decoder_strips_only_stream_leading_bom` on the test runtime.
async fn proxy_decoder_strips_only_stream_leading_bom_case() {
    let mut lines = canonical_lines();
    let first = [b"\xEF\xBB\xBF".to_vec(), lines[0].clone()].concat();
    lines[0] = first;
    assert_eq!(run_lines(&lines).await.events, canonical_events());
    let mut later = vec![b"\n\xEF\xBB\xBF".to_vec(), line(r#"{"type":"start"}"#)];
    later.extend(canonical_lines());
    assert_eq!(run_lines(&later).await.events, canonical_events());
}

/// Payload edges lose U+FEFF and Unicode whitespace, but not the next-line character.
#[test]
fn proxy_payload_trim_accepts_feff() {
    support::run_io(proxy_payload_trim_accepts_feff_case());
}
/// Drive the scenario of `proxy_payload_trim_accepts_feff` on the test runtime.
async fn proxy_payload_trim_accepts_feff_case() {
    let wrapped = "data: \u{feff}{\"type\":\"start\"}\u{feff}\n"
        .as_bytes()
        .to_vec();
    let mut chunks = vec![wrapped];
    chunks.extend(canonical_lines());
    let run = run_chunks(chunks).await;
    let mut expected = canonical_events();
    expected.insert(0, json!({"type": "start"}));
    assert_eq!(run.events, expected);
}

/// U+0085 is not payload whitespace, at either edge.
#[test]
fn proxy_payload_trim_does_not_accept_nel() {
    support::run_io(proxy_payload_trim_does_not_accept_nel_case());
}
/// Drive the scenario of `proxy_payload_trim_does_not_accept_nel` on the test runtime.
async fn proxy_payload_trim_does_not_accept_nel_case() {
    for payload in [
        "data: \u{85}{\"type\":\"start\"}\n",
        "data: {\"type\":\"start\"}\u{85}\n",
    ] {
        assert_native_error(&run_chunks(vec![payload.as_bytes().to_vec()]).await, 0);
    }
}

/// Invalid UTF-8 inside a payload becomes the replacement character.
#[test]
fn proxy_invalid_utf8_uses_replacement() {
    support::run_io(proxy_invalid_utf8_uses_replacement_case());
}
/// Drive the scenario of `proxy_invalid_utf8_uses_replacement` on the test runtime.
async fn proxy_invalid_utf8_uses_replacement_case() {
    let mut lines = vec![line(r#"{"type":"text_start","contentIndex":0}"#)];
    lines.push(
        [&b"data: {\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"\xFF\"}\n"[..]].concat(),
    );
    lines.push(done("stop"));
    let run = run_lines(&lines).await;
    assert_eq!(run.events[1]["delta"], "\u{fffd}");
    assert_eq!(run.message()["content"][0]["text"], "\u{fffd}");
}

/// UTF-8 sequences and lines may be split across arbitrary chunk boundaries.
#[test]
fn proxy_split_utf8_and_data_lines_survive() {
    support::run_io(proxy_split_utf8_and_data_lines_survive_case());
}
/// Drive the scenario of `proxy_split_utf8_and_data_lines_survive` on the test runtime.
async fn proxy_split_utf8_and_data_lines_survive_case() {
    let bytes = join(&canonical_lines());
    let run = run_chunks(bytes.iter().map(|byte| vec![*byte]).collect()).await;
    assert_eq!(run.events, canonical_events());
    assert_eq!(run.message(), canonical_message());
}

/// An unfinished final line and an unfinished final character are never processed.
#[test]
fn proxy_unterminated_tail_is_not_processed() {
    support::run_io(proxy_unterminated_tail_is_not_processed_case());
}
/// Drive the scenario of `proxy_unterminated_tail_is_not_processed` on the test runtime.
async fn proxy_unterminated_tail_is_not_processed_case() {
    for tail in [&b"data: not-json"[..], b"\xC3"] {
        let chunks = vec![join(&canonical_lines()), tail.to_vec()];
        let run = run_chunks(chunks).await;
        assert_eq!(run.events, canonical_events());
        assert_eq!(run.message(), canonical_message());
    }
}

/// Malformed JSON after received content settles an error and keeps that content.
#[test]
fn proxy_parse_failure_retains_received_content() {
    support::run_io(proxy_parse_failure_retains_received_content_case());
}
/// Drive the scenario of `proxy_parse_failure_retains_received_content` on the test runtime.
async fn proxy_parse_failure_retains_received_content_case() {
    let mut lines = canonical_lines();
    lines[5] = b"data: {\n".to_vec();
    let run = run_lines(&lines).await;
    assert_native_error(&run, 5);
    assert_eq!(run.events[..5], canonical_events()[..5]);
    let content = json!([{"type": "text", "text": "hé🙂!", "textSignature": "signature"}]);
    assert_eq!(run.message()["content"], content);
    assert_eq!(run.message()["stopReason"], "error");
}

/// The nonempty error of a failure body replaces the status text; anything else keeps it.
#[test]
fn proxy_http_failure_prefers_nonempty_json_error() {
    support::run_io(proxy_http_failure_prefers_nonempty_json_error_case());
}
/// Drive the scenario of `proxy_http_failure_prefers_nonempty_json_error` on the test runtime.
async fn proxy_http_failure_prefers_nonempty_json_error_case() {
    let cases: [(&[u8], &str); 8] = [
        (br#"{"error":"specific"}"#, "Proxy error: specific"),
        (br#"{"error":""}"#, "Proxy error: 429 Custom reason"),
        (b"{}", "Proxy error: 429 Custom reason"),
        (br#"{"error":null}"#, "Proxy error: 429 Custom reason"),
        (b"bad-json", "Proxy error: 429 Custom reason"),
        (br#"{"error":"first","error":"last"}"#, "Proxy error: last"),
        (
            b"\xEF\xBB\xBF{\"error\":\"specific\"}",
            "Proxy error: specific",
        ),
        (
            b"{\"error\":\"sp\xFFcific\"}",
            "Proxy error: sp\u{fffd}cific",
        ),
    ];
    for (body, expected) in cases {
        let (run, _) = exchange(Reply::status(429, "Custom reason", body)).await;
        assert_eq!(
            run.events,
            vec![json!({"type": "error", "reason": "error"})]
        );
        assert_eq!(run.message(), failure(expected), "{expected}");
    }
}

/// Statuses outside 2xx fall back to their code and phrase, even without a body.
#[test]
fn proxy_http_failure_retains_status_text() {
    support::run_io(proxy_http_failure_retains_status_text_case());
}
/// Drive the scenario of `proxy_http_failure_retains_status_text` on the test runtime.
async fn proxy_http_failure_retains_status_text_case() {
    for (status, text, body, expected) in [
        (599, "", &b""[..], "Proxy error: 599 "),
        (
            300,
            "Redirect boundary",
            b"{}",
            "Proxy error: 300 Redirect boundary",
        ),
    ] {
        let (run, _) = exchange(Reply::status(status, text, body)).await;
        assert_eq!(run.message(), failure(expected));
    }
    let mut broken = Reply::status(429, "Custom reason", b"");
    broken.body = vec![Err(FetchError::Timeout)];
    let (run, _) = exchange(broken).await;
    assert_eq!(run.message(), failure("Proxy error: 429 Custom reason"));
}

/// Transport and body failures settle an error that keeps what was already received.
#[test]
fn proxy_transport_failure_retains_partial() {
    support::run_io(proxy_transport_failure_retains_partial_case());
}
/// Drive the scenario of `proxy_transport_failure_retains_partial` on the test runtime.
async fn proxy_transport_failure_retains_partial_case() {
    let (fetch, requests) = support::proxy::failing("network failed");
    let run = support::proxy::run_with(support::proxy::with_fetch(fetch)).await;
    assert_eq!(
        run.events,
        vec![json!({"type": "error", "reason": "error"})]
    );
    assert_eq!(run.message(), failure("network failed"));
    assert_eq!(requests.lock().unwrap().len(), 1);
    let mut reply = Reply::chunks(vec![join(&canonical_lines()[..5])]);
    let cause = maestro_models::DiagnosticErrorInfo::new(None, "body failed");
    reply.body.push(Err(FetchError::Connection(cause)));
    let (run, _) = exchange(reply).await;
    assert_eq!(run.events[..5], canonical_events()[..5]);
    assert_eq!(run.events[5], json!({"type": "error", "reason": "error"}));
    let mut expected = canonical_message();
    expected["stopReason"] = json!("error");
    expected["errorMessage"] = json!("body failed");
    expected["usage"] = serde_json::from_str(ZERO).unwrap();
    assert_eq!(run.message(), expected);
}

/// A usage object is kept exactly; its cost is never recomputed from the token counts.
#[test]
fn proxy_terminal_usage_is_not_repriced() {
    support::run_io(proxy_terminal_usage_is_not_repriced_case());
}
/// Drive the scenario of `proxy_terminal_usage_is_not_repriced` on the test runtime.
async fn proxy_terminal_usage_is_not_repriced_case() {
    for (reason, stop) in [
        ("stop", "stop"),
        ("length", "length"),
        ("toolUse", "toolUse"),
    ] {
        let mut lines = canonical_lines();
        lines[5] = done(reason);
        let run = run_lines(&lines).await;
        assert_eq!(run.events[5], json!({"type": "done", "reason": reason}));
        let mut expected = canonical_message();
        expected["stopReason"] = json!(stop);
        assert_eq!(run.message(), expected);
        assert_eq!(
            run.message()["usage"],
            serde_json::from_str::<Value>(USAGE).unwrap()
        );
    }
}

/// Lines that build a block of some other kind.
type Setup = fn() -> Vec<Vec<u8>>;

/// A `thinking` block of two deltas, closed with a signature.
fn thinking_lines() -> Vec<Vec<u8>> {
    vec![
        line(r#"{"type":"thinking_start","contentIndex":0}"#),
        line(r#"{"type":"thinking_delta","contentIndex":0,"delta":"reason "}"#),
        line(r#"{"type":"thinking_delta","contentIndex":0,"delta":"twice"}"#),
        line(r#"{"type":"thinking_end","contentIndex":0,"contentSignature":"thought"}"#),
    ]
}
/// Event shapes of [`thinking_lines`].
fn thinking_events() -> Vec<Value> {
    vec![
        json!({"type": "thinking_start", "contentIndex": 0}),
        json!({"type": "thinking_delta", "contentIndex": 0, "delta": "reason "}),
        json!({"type": "thinking_delta", "contentIndex": 0, "delta": "twice"}),
        json!({"type": "thinking_end", "contentIndex": 0, "content": "reason twice"}),
    ]
}
/// The thinking block those lines build.
fn thinking_block() -> Value {
    json!({"type": "thinking", "thinking": "reason twice", "thinkingSignature": "thought"})
}
/// A terminal line for a tool-using response.
fn terminal() -> Vec<u8> {
    done("stop")
}

/// Text blocks accumulate, can be closed repeatedly with or without a signature, and restart empty.
#[test]
fn proxy_text_indices_signatures_and_replacement() {
    support::run_io(proxy_text_indices_signatures_and_replacement_case());
}
/// Drive the scenario of `proxy_text_indices_signatures_and_replacement` on the test runtime.
async fn proxy_text_indices_signatures_and_replacement_case() {
    let base = canonical_lines();
    let before_end = base[..4].to_vec();
    let cases = [
        (
            vec![base[4].clone()],
            json!({"type": "text", "text": "hé🙂!", "textSignature": "signature"}),
        ),
        (
            vec![
                base[4].clone(),
                line(r#"{"type":"text_end","contentIndex":0}"#),
            ],
            json!({"type": "text", "text": "hé🙂!"}),
        ),
        (
            vec![
                base[4].clone(),
                line(r#"{"type":"text_end","contentIndex":0,"contentSignature":""}"#),
            ],
            json!({"type": "text", "text": "hé🙂!", "textSignature": ""}),
        ),
        (
            vec![
                base[4].clone(),
                line(r#"{"type":"text_start","contentIndex":0}"#),
                line(r#"{"type":"text_delta","contentIndex":0,"delta":""}"#),
            ],
            json!({"type": "text", "text": ""}),
        ),
    ];
    for (tail, block) in cases {
        let lines = [before_end.clone(), tail, vec![terminal()]].concat();
        let run = run_lines(&lines).await;
        assert_eq!(run.message()["content"], json!([block]), "{block}");
        assert_eq!(run.message()["stopReason"], "stop");
    }
}

/// Thinking blocks follow the same rules as text blocks.
#[test]
fn proxy_thinking_indices_signatures_and_replacement() {
    support::run_io(proxy_thinking_indices_signatures_and_replacement_case());
}
/// Drive the scenario of `proxy_thinking_indices_signatures_and_replacement` on the test runtime.
async fn proxy_thinking_indices_signatures_and_replacement_case() {
    let base = thinking_lines();
    let cases = [
        (vec![], thinking_block()),
        (
            vec![line(r#"{"type":"thinking_end","contentIndex":0}"#)],
            json!({"type": "thinking", "thinking": "reason twice"}),
        ),
        (
            vec![line(
                r#"{"type":"thinking_end","contentIndex":0,"contentSignature":""}"#,
            )],
            json!({"type": "thinking", "thinking": "reason twice", "thinkingSignature": ""}),
        ),
        (
            vec![
                line(r#"{"type":"thinking_start","contentIndex":0}"#),
                line(r#"{"type":"thinking_delta","contentIndex":0,"delta":""}"#),
            ],
            json!({"type": "thinking", "thinking": ""}),
        ),
    ];
    for (tail, block) in cases {
        let lines = [base.clone(), tail, vec![terminal()]].concat();
        let run = run_lines(&lines).await;
        assert_eq!(run.events[..4], thinking_events()[..], "{block}");
        assert_eq!(run.message()["content"], json!([block]));
    }
}

/// Blocks live at the indices the proxy names, however their updates interleave.
#[test]
fn proxy_blocks_keep_supplied_indices() {
    support::run_io(proxy_blocks_keep_supplied_indices_case());
}
/// Drive the scenario of `proxy_blocks_keep_supplied_indices` on the test runtime.
async fn proxy_blocks_keep_supplied_indices_case() {
    let lines = [
        canonical_lines()[..5].to_vec(),
        vec![
            line(r#"{"type":"thinking_start","contentIndex":1}"#),
            line(r#"{"type":"toolcall_start","contentIndex":2,"id":"b","toolName":"calc"}"#),
            line(r#"{"type":"text_delta","contentIndex":0,"delta":"late"}"#),
            line(r#"{"type":"thinking_delta","contentIndex":1,"delta":"think"}"#),
            line(r#"{"type":"toolcall_delta","contentIndex":2,"delta":"{\"x\":2}"}"#),
            line(r#"{"type":"toolcall_end","contentIndex":2}"#),
            terminal(),
        ],
    ]
    .concat();
    let run = run_lines(&lines).await;
    let tool = json!({"type": "toolCall", "id": "b", "name": "calc", "arguments": {"x": 2}});
    assert_eq!(
        run.message()["content"],
        json!([
            {"type": "text", "text": "hé🙂!late", "textSignature": "signature"},
            {"type": "thinking", "thinking": "think"},
            tool,
        ])
    );
    assert_eq!(
        run.events[10],
        json!({"type": "toolcall_end", "contentIndex": 2, "toolCall": tool})
    );
}

/// Tool arguments come from the shared streaming parser on every delta.
#[test]
fn proxy_tool_arguments_use_shared_parser() {
    support::run_io(proxy_tool_arguments_use_shared_parser_case());
}
/// Drive the scenario of `proxy_tool_arguments_use_shared_parser` on the test runtime.
async fn proxy_tool_arguments_use_shared_parser_case() {
    let cases: [(&[&str], Value); 6] = [
        (
            &[r#"{\"expression\":\"5"#, r#" + 3\"}"#],
            json!({"expression": "5 + 3"}),
        ),
        (&["not json"], json!({})),
        (
            &[r#"{\"path\":\"C:\\work\",\"line\":\"a\nb\"}"#],
            json!({"path": "C:\\work", "line": "a\nb"}),
        ),
        (&[r#"{\"x\":1,\"x\":2}"#], json!({"x": 2})),
        (&[""], json!({})),
        (
            &[r#"{\"o\":{\"a\":[1,2],\"b\":true}}"#],
            json!({"o": {"a": [1, 2], "b": true}}),
        ),
    ];
    for (deltas, arguments) in cases {
        let mut lines = vec![line(
            r#"{"type":"toolcall_start","contentIndex":0,"id":"call-α","toolName":"calculate"}"#,
        )];
        for delta in deltas {
            lines.push(line(&format!(
                r#"{{"type":"toolcall_delta","contentIndex":0,"delta":"{delta}"}}"#
            )));
        }
        lines.push(line(r#"{"type":"toolcall_end","contentIndex":0}"#));
        lines.push(terminal());
        let run = run_lines(&lines).await;
        let tool = json!({"type": "toolCall", "id": "call-α", "name": "calculate", "arguments": arguments});
        assert_eq!(run.message()["content"], json!([tool]), "{deltas:?}");
        let end = &run.events[run.events.len() - 2];
        assert_eq!(end["toolCall"], tool);
    }
}

/// Each tool-call delta re-parses the accumulated text into the live message before the next read.
#[test]
fn proxy_tool_arguments_parse_on_each_delta() {
    support::run_io(proxy_tool_arguments_parse_on_each_delta_case());
}
/// Drive the scenario of `proxy_tool_arguments_parse_on_each_delta` on the test runtime.
async fn proxy_tool_arguments_parse_on_each_delta_case() {
    let (stream, producer, mut gate, _) =
        spawn_gated(maestro_models::SimpleStreamOptions::default());
    gate.reads.recv().await.unwrap();
    gate.send
        .send(Some(Ok(line(r#"{"type":"start"}"#))))
        .unwrap();
    gate.reads.recv().await.unwrap();
    let Some(maestro_models::AssistantMessageEvent::Start { partial }) = stream.next().await else {
        panic!("expected the start event first");
    };
    let start = r#"{"type":"toolcall_start","contentIndex":0,"id":"c","toolName":"t"}"#;
    gate.send.send(Some(Ok(line(start)))).unwrap();
    gate.reads.recv().await.unwrap();
    let observed = |partial: &maestro_models::SharedAssistantMessage| {
        support::proxy::snapshot(partial)["content"][0]["arguments"].clone()
    };
    assert_eq!(observed(&partial), json!({}));
    for (delta, expected) in [
        (r#"{\"n\":\"5"#, json!({"n": "5"})),
        (r#" + 3\",\"m\":2"#, json!({"n": "5 + 3", "m": 2})),
    ] {
        let payload = format!(r#"{{"type":"toolcall_delta","contentIndex":0,"delta":"{delta}"}}"#);
        gate.send.send(Some(Ok(line(&payload)))).unwrap();
        gate.reads.recv().await.unwrap();
        assert_eq!(observed(&partial), expected, "{delta}");
    }
    gate.send.send(Some(Ok(done("stop")))).unwrap();
    gate.send.send(None).unwrap();
    producer.await.unwrap();
    assert_eq!(observed(&partial), json!({"n": "5 + 3", "m": 2}));
}

/// A restarted tool call forgets the argument text of the call it replaces.
#[test]
fn proxy_tool_restart_clears_pending_arguments() {
    support::run_io(proxy_tool_restart_clears_pending_arguments_case());
}
/// Drive the scenario of `proxy_tool_restart_clears_pending_arguments` on the test runtime.
async fn proxy_tool_restart_clears_pending_arguments_case() {
    let start =
        line(r#"{"type":"toolcall_start","contentIndex":0,"id":"call-α","toolName":"calculate"}"#);
    let lines = [
        start.clone(),
        line(r#"{"type":"toolcall_delta","contentIndex":0,"delta":"{\"expression\":\"5"}"#),
        start,
        line(r#"{"type":"toolcall_delta","contentIndex":0,"delta":"{\"new\":true}"}"#),
        line(r#"{"type":"toolcall_end","contentIndex":0}"#),
        terminal(),
    ];
    let run = run_lines(&lines).await;
    let content = &run.message()["content"];
    assert_eq!(content[0]["arguments"], json!({"new": true}));
    assert_eq!(content.as_array().unwrap().len(), 1);
}

/// Ending a tool call that is absent or of another kind publishes nothing.
#[test]
fn proxy_wrong_kind_end_is_ignored() {
    support::run_io(proxy_wrong_kind_end_is_ignored_case());
}
/// Drive the scenario of `proxy_wrong_kind_end_is_ignored` on the test runtime.
async fn proxy_wrong_kind_end_is_ignored_case() {
    let stray = line(r#"{"type":"toolcall_end","contentIndex":0}"#);
    let canonical = canonical_lines();
    let lines = [
        vec![stray.clone()],
        canonical[..5].to_vec(),
        vec![stray, canonical[5].clone()],
    ]
    .concat();
    let run = run_lines(&lines).await;
    assert_eq!(run.events, canonical_events());
}

/// Each content update that names a block of the wrong kind fails with its authored text.
#[test]
fn proxy_mismatched_content_reports_authored_errors() {
    support::run_io(proxy_mismatched_content_reports_authored_errors_case());
}
/// Drive the scenario of `proxy_mismatched_content_reports_authored_errors` on the test runtime.
async fn proxy_mismatched_content_reports_authored_errors_case() {
    let text: Setup = || canonical_lines()[..5].to_vec();
    let cases: [(&str, Setup, &str); 5] = [
        (
            r#"{"type":"text_delta","contentIndex":0,"delta":"x"}"#,
            thinking_lines,
            "Received text_delta for non-text content",
        ),
        (
            r#"{"type":"text_end","contentIndex":0}"#,
            thinking_lines,
            "Received text_end for non-text content",
        ),
        (
            r#"{"type":"thinking_delta","contentIndex":0,"delta":"x"}"#,
            text,
            "Received thinking_delta for non-thinking content",
        ),
        (
            r#"{"type":"thinking_end","contentIndex":0}"#,
            text,
            "Received thinking_end for non-thinking content",
        ),
        (
            r#"{"type":"toolcall_delta","contentIndex":0,"delta":"x"}"#,
            text,
            "Received toolcall_delta for non-toolCall content",
        ),
    ];
    for (payload, wrong_kind, expected) in cases {
        for setup in [vec![], wrong_kind()] {
            assert_mismatch(&setup, payload, expected).await;
        }
    }
}
/// Run `setup` then the offending payload and expect the authored failure.
async fn assert_mismatch(setup: &[Vec<u8>], offending: &str, expected: &str) {
    let lines = [setup.to_vec(), vec![line(offending)]].concat();
    let run = run_lines(&lines).await;
    assert_eq!(run.events.len(), setup.len() + 1, "{expected}");
    assert_eq!(
        run.events[setup.len()],
        json!({"type": "error", "reason": "error"})
    );
    let message = run.message();
    assert_eq!(message["errorMessage"], expected);
    assert_eq!(message["stopReason"], "error");
    assert_eq!(
        message["content"].as_array().unwrap().len(),
        usize::from(!setup.is_empty())
    );
}

/// A remote failure keeps its reason and keeps the optional message exactly as sent.
#[test]
fn proxy_remote_error_preserves_optional_message() {
    support::run_io(proxy_remote_error_preserves_optional_message_case());
}
/// Drive the scenario of `proxy_remote_error_preserves_optional_message` on the test runtime.
async fn proxy_remote_error_preserves_optional_message_case() {
    for reason in ["error", "aborted"] {
        for sent in [None, Some(""), Some("remote failure")] {
            let extra = sent.map_or(String::new(), |text| format!(r#","errorMessage":"{text}""#));
            let failure = line(&format!(
                r#"{{"type":"error","reason":"{reason}","usage":{USAGE}{extra}}}"#
            ));
            let run = run_lines(&[thinking_lines(), vec![failure]].concat()).await;
            let mut events = thinking_events();
            events.push(json!({"type": "error", "reason": reason}));
            assert_eq!(run.events, events);
            let content = json!([thinking_block()]);
            assert_eq!(
                run.message(),
                message(&content, reason, sent, USAGE),
                "{reason} {sent:?}"
            );
        }
    }
}

/// A remote terminal followed by the end of the body settles exactly once.
#[test]
fn proxy_terminal_then_eof_settles_once() {
    support::run_io(proxy_terminal_then_eof_settles_once_case());
}
/// Drive the scenario of `proxy_terminal_then_eof_settles_once` on the test runtime.
async fn proxy_terminal_then_eof_settles_once_case() {
    let failure = line(&format!(
        r#"{{"type":"error","reason":"error","usage":{USAGE},"errorMessage":"remote"}}"#
    ));
    let run = run_lines(&[failure]).await;
    assert_eq!(
        run.events,
        vec![json!({"type": "error", "reason": "error"})]
    );
    assert_eq!(
        run.message(),
        message(&json!([]), "error", Some("remote"), USAGE)
    );
}

/// Input after a terminal event still updates the retained message; it never becomes an event.
#[test]
fn proxy_terminal_does_not_stop_source_reduction() {
    support::run_io(proxy_terminal_does_not_stop_source_reduction_case());
}
/// Drive the scenario of `proxy_terminal_does_not_stop_source_reduction` on the test runtime.
async fn proxy_terminal_does_not_stop_source_reduction_case() {
    let trailing = [
        line(r#"{"type":"text_delta","contentIndex":0,"delta":"AFTER"}"#),
        line(&format!(
            r#"{{"type":"done","reason":"length","usage":{USAGE}}}"#
        )),
    ];
    let run = run_lines(&[canonical_lines(), trailing.to_vec()].concat()).await;
    assert_eq!(run.events, canonical_events());
    let mut expected = canonical_message();
    expected["stopReason"] = json!("length");
    expected["content"][0]["text"] = json!("hé🙂!AFTER");
    assert_eq!(run.message(), expected);
    let broken = run_lines(&[canonical_lines(), vec![b"data: {\n".to_vec()]].concat()).await;
    assert_eq!(broken.events, canonical_events());
    let message = broken.message();
    assert_eq!(message["stopReason"], "error");
    assert!(!message["errorMessage"].as_str().unwrap().is_empty());
    assert_eq!(message["content"], canonical_message()["content"]);
}

/// Repeated members select their last value at every depth, before the event is decoded.
#[test]
fn proxy_wire_decoding_selects_last_members() {
    support::run_io(proxy_wire_decoding_selects_last_members_case());
}
/// Drive the scenario of `proxy_wire_decoding_selects_last_members` on the test runtime.
async fn proxy_wire_decoding_selects_last_members_case() {
    let start = r#"{"type":"future","type":"text_start","contentIndex":1,"contentIndex":0,"ignored":{"deep":[1]}}"#;
    let usage = r#"{"input":99,"input":7,"output":3,"cacheRead":2,"cacheWrite":1,"totalTokens":13,"cost":{"input":99,"input":0.7,"output":0.6,"cacheRead":0.2,"cacheWrite":0.1,"total":9.5}}"#;
    let lines = [
        line(start),
        line(r#"{"type":"text_delta","contentIndex":0,"delta":"first","delta":"last"}"#),
        line(&format!(
            r#"{{"type":"done","reason":"length","reason":"stop","usage":{usage}}}"#
        )),
    ];
    let run = run_lines(&lines).await;
    let expected = message(
        &json!([{"type": "text", "text": "last"}]),
        "stop",
        None,
        USAGE,
    );
    assert_eq!(run.message(), expected);
    assert_eq!(run.kinds(), ["text_start", "text_delta", "done"]);
}

/// Start a gated exchange whose producer runs on the test runtime.
fn spawn_gated(
    mut options: maestro_models::SimpleStreamOptions,
) -> (
    maestro_models::AssistantMessageEventStream,
    tokio::task::JoinHandle<()>,
    support::proxy::Gate,
    support::proxy::Requests,
) {
    let (fetch, requests, gate) = support::proxy::gated();
    options.common.fetch = Some(fetch);
    let (stream, tasks) = support::proxy::start(options);
    (stream, tokio::spawn(tasks.take()), gate, requests)
}
/// Options carrying a fresh cancellation signal.
fn cancellable() -> (
    maestro_models::SimpleStreamOptions,
    maestro_models::Cancellation,
) {
    let signal = maestro_models::Cancellation::new();
    let mut options = maestro_models::SimpleStreamOptions::default();
    options.common.signal = Some(signal.clone());
    (options, signal)
}
/// The failure events of an aborted exchange.
fn aborted_message(content: &Value) -> Value {
    message(content, "aborted", Some("Request aborted by user"), ZERO)
}

/// An abort that becomes visible together with the end of the body beats the premature-end text.
#[test]
fn proxy_abort_precedes_premature_eof() {
    support::run_io(proxy_abort_precedes_premature_eof_case());
}
/// Drive the scenario of `proxy_abort_precedes_premature_eof` on the test runtime.
async fn proxy_abort_precedes_premature_eof_case() {
    let (options, signal) = cancellable();
    let (stream, producer, mut gate, _) = spawn_gated(options);
    gate.reads.recv().await.unwrap();
    gate.send
        .send(Some(Ok(line(r#"{"type":"start"}"#))))
        .unwrap();
    gate.reads.recv().await.unwrap();
    signal.abort();
    gate.send.send(None).unwrap();
    producer.await.unwrap();
    let run = support::proxy::collect(&stream).await;
    assert_eq!(
        run.events,
        [
            json!({"type": "start"}),
            json!({"type": "error", "reason": "aborted"})
        ]
    );
    assert_eq!(run.message(), aborted_message(&json!([])));
}

/// The first event shares the message every later update and the final result expose.
#[test]
fn proxy_initial_partial_is_shared() {
    support::run_io(proxy_initial_partial_is_shared_case());
}
/// Drive the scenario of `proxy_initial_partial_is_shared` on the test runtime.
async fn proxy_initial_partial_is_shared_case() {
    let before = maestro_models::timestamp_now();
    let (stream, producer, mut gate, _) =
        spawn_gated(maestro_models::SimpleStreamOptions::default());
    gate.reads.recv().await.unwrap();
    gate.send
        .send(Some(Ok(line(r#"{"type":"start"}"#))))
        .unwrap();
    gate.reads.recv().await.unwrap();
    let Some(maestro_models::AssistantMessageEvent::Start { partial }) = stream.next().await else {
        panic!("expected the start event first");
    };
    assert_eq!(
        support::proxy::snapshot(&partial),
        message(&json!([]), "stop", None, ZERO)
    );
    let stamp = partial.read().unwrap().timestamp;
    assert!(before <= stamp && stamp <= maestro_models::timestamp_now());
    gate.send.send(Some(Ok(done("stop")))).unwrap();
    gate.send.send(None).unwrap();
    producer.await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&partial, &stream.result().await));
    assert_eq!(
        support::proxy::snapshot(&partial)["usage"],
        serde_json::from_str::<Value>(USAGE).unwrap()
    );
}

/// A signal that is already aborted, or aborts while reading, settles an aborted result.
#[test]
fn proxy_preabort_and_inflight_abort_settle() {
    support::run_io(proxy_preabort_and_inflight_abort_settle_case());
}
/// Drive the scenario of `proxy_preabort_and_inflight_abort_settle` on the test runtime.
async fn proxy_preabort_and_inflight_abort_settle_case() {
    let (mut options, signal) = cancellable();
    signal.abort();
    let (fetch, requests) = support::proxy::replying(Reply::chunks(canonical_lines()));
    options.common.fetch = Some(fetch);
    let run = support::proxy::run_with(options).await;
    assert_eq!(run.events, [json!({"type": "error", "reason": "aborted"})]);
    assert_eq!(run.message(), aborted_message(&json!([])));
    assert!(
        requests.lock().unwrap()[0]
            .signal
            .as_ref()
            .unwrap()
            .is_aborted()
    );

    let (options, signal) = cancellable();
    let (stream, producer, mut gate, _) = spawn_gated(options);
    gate.reads.recv().await.unwrap();
    gate.send
        .send(Some(Ok(line(r#"{"type":"start"}"#))))
        .unwrap();
    gate.reads.recv().await.unwrap();
    signal.abort();
    producer.await.unwrap();
    gate.dropped.await.unwrap();
    let run = support::proxy::collect(&stream).await;
    assert_eq!(
        run.events,
        [
            json!({"type": "start"}),
            json!({"type": "error", "reason": "aborted"})
        ]
    );
    assert_eq!(run.message(), aborted_message(&json!([])));
}

/// The producer is submitted before the stream is returned and finishes without any observer.
#[test]
fn proxy_producer_outlives_dropped_observers() {
    support::run_io(proxy_producer_outlives_dropped_observers_case());
}
/// Drive the scenario of `proxy_producer_outlives_dropped_observers` on the test runtime.
async fn proxy_producer_outlives_dropped_observers_case() {
    let (fetch, requests, gate) = support::proxy::gated();
    let (stream, tasks) = support::proxy::start(support::proxy::with_fetch(fetch));
    assert_eq!(tasks.0.lock().unwrap().len(), 1);
    assert!(requests.lock().unwrap().is_empty());
    drop(stream);
    for bytes in canonical_lines() {
        gate.send.send(Some(Ok(bytes))).unwrap();
    }
    gate.send.send(None).unwrap();
    tasks.take().await;
    assert_eq!(requests.lock().unwrap().len(), 1);
    gate.dropped.await.unwrap();
}

/// Aborting drops the pending request or the open body, and a later request is unaffected.
#[test]
fn proxy_cancellation_drops_inflight_transport() {
    support::run_io(proxy_cancellation_drops_inflight_transport_case());
}
/// Drive the scenario of `proxy_cancellation_drops_inflight_transport` on the test runtime.
async fn proxy_cancellation_drops_inflight_transport_case() {
    let (mut options, signal) = cancellable();
    let (fetch, requests, mut held) = support::proxy::held();
    options.common.fetch = Some(fetch);
    let (stream, tasks) = support::proxy::start(options);
    let producer = tokio::spawn(tasks.take());
    held.started.recv().await.unwrap();
    signal.abort();
    producer.await.unwrap();
    held.dropped.await.unwrap();
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert!(
        requests.lock().unwrap()[0]
            .signal
            .as_ref()
            .unwrap()
            .is_aborted()
    );
    let run = support::proxy::collect(&stream).await;
    assert_eq!(run.events, [json!({"type": "error", "reason": "aborted"})]);

    let (options, signal) = cancellable();
    let (_, producer, mut gate, _) = spawn_gated(options);
    gate.reads.recv().await.unwrap();
    signal.abort();
    producer.await.unwrap();
    gate.dropped.await.unwrap();
    assert_eq!(
        support::proxy::run_lines(&canonical_lines()).await.events,
        canonical_events()
    );
}

/// An abort after the terminal event rewrites the retained message but publishes nothing new.
#[test]
fn proxy_abort_after_terminal_mutates_retained_result() {
    support::run_io(proxy_abort_after_terminal_mutates_retained_result_case());
}
/// Drive the scenario of `proxy_abort_after_terminal_mutates_retained_result` on the test runtime.
async fn proxy_abort_after_terminal_mutates_retained_result_case() {
    let (options, signal) = cancellable();
    let (stream, producer, mut gate, _) = spawn_gated(options);
    gate.reads.recv().await.unwrap();
    gate.send.send(Some(Ok(join(&canonical_lines())))).unwrap();
    gate.reads.recv().await.unwrap();
    let result = stream.result().await;
    signal.abort();
    producer.await.unwrap();
    gate.dropped.await.unwrap();
    let run = support::proxy::collect(&stream).await;
    assert_eq!(run.events, canonical_events());
    let mut expected = aborted_message(&canonical_message()["content"]);
    expected["usage"] = serde_json::from_str(USAGE).unwrap();
    assert_eq!(support::proxy::snapshot(&result), expected);
    assert!(std::sync::Arc::ptr_eq(&result, &run.result));
}

/// Options setting every allowlisted field and every field the proxy must not forward.
fn full_options() -> maestro_models::SimpleStreamOptions {
    let map = |value: Value| value.as_object().unwrap().clone();
    let mut options = maestro_models::SimpleStreamOptions::default();
    let common = &mut options.common;
    common.temperature = Some(-0.0);
    common.max_tokens = Some(0.0);
    common.cache_retention = Some(maestro_models::CacheRetention::None);
    common.session_id = Some(String::new());
    common.headers = Some(
        [("z", "last"), ("2", "second"), ("1", "first"), ("a", "")]
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .into(),
    );
    common.metadata = Some(map(json!({
        "z": -0.0, "2": 2, "1": 1, "empty": null, "nested": {"z": 1e21, "a": 1e-7}
    })));
    common.transport = Some(maestro_models::Transport::Auto);
    common.max_retry_delay_ms = Some(-7.0);
    common.api_key = Some("provider-secret".into());
    common.timeout_ms = Some(123.0);
    common.max_retries = Some(9.0);
    options.reasoning = Some(maestro_models::ThinkingLevel::High);
    options.thinking_budgets = Some(maestro_models::ThinkingBudgets {
        minimal: Some(0.0),
        high: Some(128.0),
        ..Default::default()
    });
    options
}
/// The body a run sent, as text.
fn body_of(requests: &support::proxy::Requests) -> String {
    String::from_utf8(requests.lock().unwrap()[0].body.clone()).unwrap()
}

/// Only the allowlisted options are sent, in wire order, omitting absent ones and keeping empty ones.
#[test]
fn proxy_request_allowlist_preserves_omission_and_wire_order() {
    support::run_io(proxy_request_allowlist_preserves_omission_and_wire_order_case());
}
/// Drive the scenario of `proxy_request_allowlist_preserves_omission_and_wire_order` on the test runtime.
async fn proxy_request_allowlist_preserves_omission_and_wire_order_case() {
    let expected = concat!(
        r#","options":{"temperature":0,"maxTokens":0,"reasoning":"high","cacheRetention":"none","#,
        r#""sessionId":"","headers":{"1":"first","2":"second","z":"last","a":""},"#,
        r#""metadata":{"1":1,"2":2,"z":0,"empty":null,"nested":{"z":1e+21,"a":1e-7}},"#,
        r#""transport":"auto","thinkingBudgets":{"minimal":0,"high":128},"maxRetryDelayMs":-7}}"#,
    );
    let (fetch, requests) = support::proxy::replying(Reply::chunks(canonical_lines()));
    let mut options = full_options();
    options.common.fetch = Some(fetch);
    support::proxy::run_with(options).await;
    let body = body_of(&requests);
    assert!(body.starts_with(r#"{"model":{"#));
    assert!(body.contains(r#","context":{"#));
    assert!(body.ends_with(expected), "{body}");

    let mut empty = maestro_models::SimpleStreamOptions::default();
    empty.common.headers = Some(std::iter::empty().collect());
    empty.common.metadata = Some(serde_json::Map::default());
    empty.common.session_id = Some(String::new());
    empty.common.temperature = Some(-0.0);
    empty.common.max_tokens = Some(1e21);
    empty.common.max_retry_delay_ms = Some(1e-7);
    empty.thinking_budgets = Some(maestro_models::ThinkingBudgets::default());
    let (fetch, requests) = support::proxy::replying(Reply::chunks(canonical_lines()));
    empty.common.fetch = Some(fetch);
    support::proxy::run_with(empty).await;
    let tail = r#","options":{"temperature":0,"maxTokens":1e+21,"sessionId":"","headers":{},"metadata":{},"thinkingBudgets":{},"maxRetryDelayMs":1e-7}}"#;
    assert!(body_of(&requests).ends_with(tail));

    let (fetch, requests) = support::proxy::replying(Reply::chunks(canonical_lines()));
    support::proxy::run_with(support::proxy::with_fetch(fetch)).await;
    assert!(body_of(&requests).ends_with(r#","options":{}}"#));
}

/// The transport gets the literal address, both headers, the exact body and the caller's signal.
#[test]
fn proxy_adapter_receives_unmodified_request_and_signal() {
    support::run_io(proxy_adapter_receives_unmodified_request_and_signal_case());
}
/// Drive the scenario of `proxy_adapter_receives_unmodified_request_and_signal` on the test runtime.
async fn proxy_adapter_receives_unmodified_request_and_signal_case() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let (mut options, signal) = cancellable();
    let counter = std::sync::Arc::clone(&calls);
    options.common.on_payload = Some(std::sync::Arc::new(move |payload, _| {
        counter.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(payload)))
    }));
    let counter = std::sync::Arc::clone(&calls);
    options.common.on_response = Some(std::sync::Arc::new(move |_, _| {
        counter.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(())))
    }));
    options.common.api_key = Some("provider-secret".into());
    let (fetch, requests) = support::proxy::replying(Reply::chunks(canonical_lines()));
    options.common.fetch = Some(fetch);
    let run = support::proxy::run_with(options).await;
    assert_eq!(run.events, canonical_events());
    let seen = requests.lock().unwrap()[0].clone();
    assert_eq!(seen.url, "https://proxy.example//api/stream");
    assert_eq!(seen.method, "POST");
    assert_eq!(
        seen.header("authorization"),
        Some("Bearer local-credential")
    );
    assert_eq!(seen.header("content-type"), Some("application/json"));
    assert_eq!(seen.headers.len(), 2);
    let body: Value = serde_json::from_slice(&seen.body).unwrap();
    assert_eq!(
        body.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["model", "context", "options"]
    );
    assert_eq!(body["options"], json!({}));
    let text = String::from_utf8(seen.body.clone()).unwrap();
    assert!(!text.contains("local-credential") && !text.contains("provider-secret"));
    assert!(!seen.signal.as_ref().unwrap().is_aborted());
    signal.abort();
    assert!(seen.signal.unwrap().is_aborted());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

/// Known discriminants decode, unknown members are ignored, and wrong field types fail natively.
#[test]
fn proxy_wire_field_domains_keep_selection_and_failures() {
    support::run_io(proxy_wire_field_domains_keep_selection_and_failures_case());
}
/// Drive the scenario of `proxy_wire_field_domains_keep_selection_and_failures` on the test runtime.
async fn proxy_wire_field_domains_keep_selection_and_failures_case() {
    let accepted = [
        line(r#"{"type":"start","extra":{"a":[1]}}"#),
        line(r#"{"type":"toolcall_start","contentIndex":0,"id":"","toolName":""}"#),
        line(r#"{"type":"toolcall_delta","contentIndex":0,"delta":""}"#),
        line(r#"{"type":"toolcall_end","contentIndex":0}"#),
        line(r#"{"type":"text_start","contentIndex":1}"#),
        line(r#"{"type":"text_end","contentIndex":1,"contentSignature":null}"#),
        done("stop"),
    ];
    let run = run_lines(&accepted).await;
    let tool = json!({"type": "toolCall", "id": "", "name": "", "arguments": {}});
    assert_eq!(
        run.message()["content"],
        json!([tool, {"type": "text", "text": ""}])
    );
    let broken = [
        r#"{"type":"text_start","contentIndex":"0"}"#,
        r#"{"type":"text_start","contentIndex":-1}"#,
        r#"{"type":"text_start"}"#,
        r#"{"type":"text_delta","contentIndex":0,"delta":5}"#,
        r#"{"type":"toolcall_start","contentIndex":0,"id":1,"toolName":"x"}"#,
        r#"{"type":"done","reason":"bogus","usage":{}}"#,
        r#"{"type":"done","reason":"stop"}"#,
        r#"{"type":"error","reason":"error","usage":{"input":1},"errorMessage":"x"}"#,
        r#"{"type":"text_start","contentIndex":3}"#,
    ];
    for payload in broken {
        let lines = [canonical_lines()[..5].to_vec(), vec![line(payload)]].concat();
        let run = run_lines(&lines).await;
        assert_native_error(&run, 5);
        assert_eq!(
            run.message()["content"],
            canonical_message()["content"],
            "{payload}"
        );
    }
}

/// Integral numbers select a content index whatever their JSON spelling; others fail natively.
#[test]
fn proxy_content_index_accepts_integral_number_spellings() {
    support::run_io(proxy_content_index_accepts_integral_number_spellings_case());
}
/// Drive the scenario of `proxy_content_index_accepts_integral_number_spellings` on the test runtime.
async fn proxy_content_index_accepts_integral_number_spellings_case() {
    let lines = [
        line(r#"{"type":"text_start","contentIndex":0.0}"#),
        line(r#"{"type":"text_delta","contentIndex":0e0,"delta":"a"}"#),
        line(r#"{"type":"text_end","contentIndex":-0.0}"#),
        line(r#"{"type":"thinking_start","contentIndex":3,"contentIndex":1.0}"#),
        line(r#"{"type":"thinking_delta","contentIndex":1E0,"delta":"b"}"#),
        line(r#"{"type":"thinking_end","contentIndex":10e-1}"#),
        line(r#"{"type":"toolcall_start","contentIndex":2.0,"id":"t","toolName":"n"}"#),
        line(r#"{"type":"toolcall_delta","contentIndex":0.2e1,"delta":"{}"}"#),
        line(r#"{"type":"toolcall_end","contentIndex":2e0}"#),
        done("stop"),
    ];
    let run = run_lines(&lines).await;
    assert_eq!(
        run.message()["content"],
        json!([
            {"type": "text", "text": "a"},
            {"type": "thinking", "thinking": "b"},
            {"type": "toolCall", "id": "t", "name": "n", "arguments": {}}
        ])
    );
    for payload in [
        r#"{"type":"text_start","contentIndex":1.5}"#,
        r#"{"type":"text_start","contentIndex":-1.0}"#,
        r#"{"type":"text_start","contentIndex":1e300}"#,
        r#"{"type":"text_start","contentIndex":0,"contentIndex":0.5}"#,
    ] {
        assert_native_error(&run_lines(&[line(payload)]).await, 0);
    }
}

/// Handles stay shared from the first event to the result, and reported numbers are not altered.
#[test]
fn proxy_borrowed_result_and_final_usage_keep_identity() {
    support::run_io(proxy_borrowed_result_and_final_usage_keep_identity_case());
}
/// Drive the scenario of `proxy_borrowed_result_and_final_usage_keep_identity` on the test runtime.
async fn proxy_borrowed_result_and_final_usage_keep_identity_case() {
    let usage = USAGE.replace("\"input\":7", "\"input\":-0.0");
    let finish = line(&format!(
        r#"{{"type":"done","reason":"stop","usage":{usage}}}"#
    ));
    let late = line(r#"{"type":"text_delta","contentIndex":0,"delta":"late"}"#);
    let lines = [canonical_lines()[..5].to_vec(), vec![finish, late]].concat();
    let run = run_lines(&lines).await;
    let maestro_models::AssistantMessageEvent::Start { partial } = &run.raw[0] else {
        panic!("expected start first");
    };
    let maestro_models::AssistantMessageEvent::Done { message: last, .. } = &run.raw[5] else {
        panic!("expected done last");
    };
    assert!(std::sync::Arc::ptr_eq(partial, last) && std::sync::Arc::ptr_eq(partial, &run.result));
    let seen = partial.read().unwrap();
    assert!(seen.usage.input.is_sign_negative() && seen.usage.input == 0.0);
    assert_eq!(seen.usage.cost.total.to_bits(), 9.5_f64.to_bits());
    assert!(
        matches!(&seen.content[..], [maestro_models::AssistantContent::Text(text)] if text.text == "hé🙂!late")
    );
    drop(seen);
    let weak = std::sync::Arc::downgrade(partial);
    drop(run);
    assert!(weak.upgrade().is_none());
}

#[allow(
    dead_code,
    reason = "This binary uses part of the shared loopback server."
)]
#[path = "../../maestro-models/tests/support/loopback.rs"]
mod loopback;

/// How a proxy request reaches its server.
#[derive(Clone, Copy, Debug)]
enum Route {
    /// A replacement transport answering in process.
    Controlled,
    /// The default transport against a loopback server.
    Http,
}
/// What the server side saw of one exchange.
#[derive(Debug, PartialEq)]
struct Exchange {
    /// Request target after the authority.
    target: String,
    /// Authorization header.
    authorization: Option<String>,
    /// Content type header.
    content_type: Option<String>,
    /// Request body.
    body: Vec<u8>,
}
/// One scripted server answer.
struct Answer {
    /// Status code.
    status: u16,
    /// Status phrase.
    phrase: &'static str,
    /// Body pieces in order.
    pieces: Vec<Vec<u8>>,
    /// Whether the connection breaks after the pieces.
    truncated: bool,
}
impl Answer {
    /// A complete, successful body.
    fn ok(pieces: Vec<Vec<u8>>) -> Self {
        Self {
            status: 200,
            phrase: "OK",
            pieces,
            truncated: false,
        }
    }
}
/// Run one request against `answer` over `route`.
async fn exchange_over(route: Route, answer: Answer) -> (support::proxy::Run, Exchange) {
    match route {
        Route::Controlled => {
            let mut reply = Reply::chunks(answer.pieces);
            reply.status = answer.status;
            reply.status_text = answer.phrase.into();
            if answer.truncated {
                reply.body.push(Err(FetchError::Timeout));
            }
            let (fetch, requests) = support::proxy::replying(reply);
            let run = support::proxy::run_with(support::proxy::with_fetch(fetch)).await;
            let seen = requests.lock().unwrap()[0].clone();
            let target = seen
                .url
                .strip_prefix("https://proxy.example")
                .unwrap()
                .to_owned();
            let header = |name| seen.header(name).map(str::to_owned);
            let exchange = Exchange {
                target,
                authorization: header("authorization"),
                content_type: header("content-type"),
                body: seen.body.clone(),
            };
            (run, exchange)
        }
        Route::Http => {
            let length = if answer.truncated {
                "content-length: 100000\r\n"
            } else {
                ""
            };
            let head = format!(
                "HTTP/1.1 {} {}\r\ncontent-type: text/event-stream\r\nconnection: close\r\n{length}\r\n",
                answer.status, answer.phrase
            );
            let server = loopback::serve(&head, loopback::Step::writes(answer.pieces)).unwrap();
            let address = format!("{}/", server.url);
            let proxy = maestro_agent::ProxyStreamOptions {
                options: maestro_models::SimpleStreamOptions::default(),
                auth_token: support::proxy::TOKEN.into(),
                proxy_url: address,
            };
            let (stream, tasks) = support::proxy::start_at(proxy);
            let run = support::proxy::finish(&stream, &tasks).await;
            let received = server.finish().unwrap();
            let target = received.request_line.strip_prefix("POST ").unwrap();
            let target = target.strip_suffix(" HTTP/1.1").unwrap().to_owned();
            let exchange = Exchange {
                target,
                authorization: received.header("authorization").map(str::to_owned),
                content_type: received.header("content-type").map(str::to_owned),
                body: received.body,
            };
            (run, exchange)
        }
    }
}

/// The server answers the conformance contract compares across routes.
fn contract_answers() -> Vec<Answer> {
    let status = |code, phrase, body: &[u8]| Answer {
        status: code,
        phrase,
        pieces: vec![body.to_vec()],
        truncated: false,
    };
    let mut malformed = canonical_lines()[..5].to_vec();
    malformed.push(b"data: {\n".to_vec());
    vec![
        Answer::ok(canonical_lines()),
        status(429, "Custom reason", br#"{"error":"specific"}"#),
        status(429, "Custom reason", b"{}"),
        Answer::ok(malformed),
        Answer {
            truncated: true,
            ..Answer::ok(canonical_lines()[..5].to_vec())
        },
    ]
}
/// Abort an exchange whose body is open after one event and return what the stream then holds.
async fn cancel_over(route: Route) -> support::proxy::Run {
    let (options, signal) = cancellable();
    let start = line(r#"{"type":"start"}"#);
    let (stream, producer, release) = match route {
        Route::Controlled => {
            let (stream, producer, gate, _) = spawn_gated(options);
            gate.send.send(Some(Ok(start))).unwrap();
            (stream, producer, Some(gate))
        }
        Route::Http => {
            let (hold, wait) = std::sync::mpsc::channel();
            let steps = vec![loopback::Step::Write(start), loopback::Step::Wait(wait)];
            let server = loopback::serve(loopback::EVENT_STREAM_HEAD, steps).unwrap();
            let proxy = maestro_agent::ProxyStreamOptions {
                options,
                auth_token: support::proxy::TOKEN.into(),
                proxy_url: server.url.clone(),
            };
            let (stream, tasks) = support::proxy::start_at(proxy);
            let producer = tokio::spawn(tasks.take());
            let first = stream.next().await.unwrap();
            assert_eq!(support::proxy::shape(&first), json!({"type": "start"}));
            signal.abort();
            producer.await.unwrap();
            hold.send(()).unwrap();
            server.finish().unwrap();
            return support::proxy::collect(&stream).await;
        }
    };
    let first = stream.next().await.unwrap();
    assert_eq!(support::proxy::shape(&first), json!({"type": "start"}));
    signal.abort();
    producer.await.unwrap();
    release.unwrap().dropped.await.unwrap();
    support::proxy::collect(&stream).await
}

/// Both transports yield the same requests, results and failures for the same server answers.
#[test]
fn proxy_controlled_and_http_transports_conform() {
    support::run_io(proxy_controlled_and_http_transports_conform_case());
}
/// Drive the scenario of `proxy_controlled_and_http_transports_conform` on the test runtime.
async fn proxy_controlled_and_http_transports_conform_case() {
    let (mut controlled, mut http) = (vec![], vec![]);
    for (route, seen) in [
        (Route::Controlled, &mut controlled),
        (Route::Http, &mut http),
    ] {
        for answer in contract_answers() {
            seen.push(exchange_over(route, answer).await);
        }
    }
    for ((left, left_exchange), (right, right_exchange)) in controlled.iter().zip(&http) {
        assert_eq!(left_exchange, right_exchange);
        assert_eq!(left.kinds(), right.kinds());
    }
    assert_request_contract(&controlled[0].1);
    for outcomes in [&controlled, &http] {
        assert_outcome_contract(outcomes);
    }
    for route in [Route::Controlled, Route::Http] {
        let run = cancel_over(route).await;
        assert_eq!(
            run.events,
            [json!({"type": "error", "reason": "aborted"})],
            "{route:?}"
        );
        assert_eq!(run.message(), aborted_message(&json!([])), "{route:?}");
    }
}
/// The request every route must have carried for the first answer.
fn assert_request_contract(first: &Exchange) {
    assert_eq!(first.target, "//api/stream");
    assert_eq!(
        first.authorization.as_deref(),
        Some("Bearer local-credential")
    );
    assert_eq!(first.content_type.as_deref(), Some("application/json"));
    assert!(first.body.ends_with(br#","options":{}}"#));
}
/// The results every route must have produced for the contract answers.
fn assert_outcome_contract(outcomes: &[(support::proxy::Run, Exchange)]) {
    assert_eq!(outcomes[0].0.message(), canonical_message());
    assert_eq!(outcomes[1].0.message(), failure("Proxy error: specific"));
    assert_eq!(
        outcomes[2].0.message(),
        failure("Proxy error: 429 Custom reason")
    );
    for outcome in &outcomes[3..] {
        assert_native_error(&outcome.0, 5);
        assert_eq!(
            outcome.0.message()["content"],
            canonical_message()["content"]
        );
    }
}

/// Unknown discriminants, and events without one, warn on stderr and are skipped.
#[test]
fn proxy_unknown_event_warns_and_continues() {
    support::run_io(proxy_unknown_event_warns_and_continues_case());
}
/// Drive the scenario of `proxy_unknown_event_warns_and_continues` on the test runtime.
async fn proxy_unknown_event_warns_and_continues_case() {
    const TEST: &str = "proxy_unknown_event_warns_and_continues";
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(TEST)
            .unwrap()
            .nocapture()
            .env_clear()
            .case("*")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        for warning in ["future_event", "undefined"] {
            let text = format!("Unhandled proxy event type: {warning}\n");
            assert_eq!(stderr.matches(&text).count(), 1, "{warning}: {stderr}");
        }
        return;
    }
    let skipped = [
        line(r#"{"type":"future_event"}"#),
        line(r#"{"contentIndex":0}"#),
    ];
    let run = run_lines(&[skipped.to_vec(), canonical_lines()].concat()).await;
    assert_eq!(run.events, canonical_events());
    assert_eq!(run.message(), canonical_message());
}

/// The last assistant entry of an Agent, as text, stop reason and error message.
fn last_assistant(agent: &maestro_agent::Agent) -> (Value, Option<String>, String) {
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    let Some(maestro_agent::AgentMessage::Assistant(message)) = messages.last() else {
        panic!("no assistant entry");
    };
    let message = message.read().unwrap();
    let stop = serde_json::to_value(&message.stop_reason).unwrap();
    let text = message
        .content
        .iter()
        .filter_map(|block| match block {
            maestro_models::AssistantContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    (stop, message.error_message.clone(), text)
}

/// The server side of one Agent scenario; it ends the body when told to.
enum Backend {
    /// A gated in-process body.
    Gate(support::proxy::Gate),
    /// A loopback server held open until released.
    Server(loopback::Loopback, std::sync::mpsc::Sender<()>),
}
impl Backend {
    /// Serve `lines` over `route`, returning the proxy address and optional transport.
    fn start(route: Route, lines: Vec<Vec<u8>>) -> (Self, String, Option<maestro_models::Fetch>) {
        match route {
            Route::Controlled => {
                let (fetch, _, gate) = support::proxy::gated();
                for bytes in lines {
                    gate.send.send(Some(Ok(bytes))).unwrap();
                }
                (
                    Self::Gate(gate),
                    "https://proxy.example".into(),
                    Some(fetch),
                )
            }
            Route::Http => {
                let (hold, wait) = std::sync::mpsc::channel();
                let mut steps = loopback::Step::writes(lines);
                steps.push(loopback::Step::Wait(wait));
                let server = loopback::serve(loopback::EVENT_STREAM_HEAD, steps).unwrap();
                let url = server.url.clone();
                (Self::Server(server, hold), url, None)
            }
        }
    }
    /// End the body and let the server finish its answer.
    fn release(&self) {
        match self {
            Self::Gate(gate) => drop(gate.send.send(None)),
            Self::Server(_, hold) => drop(hold.send(())),
        }
    }
    /// Wait for the server thread, if any.
    fn join(self) {
        if let Self::Server(server, _) = self {
            server.finish().unwrap();
        }
    }
}

/// One Agent configuration settles success, premature end and abort over either transport.
#[test]
fn proxy_agent_stream_fn_accepts_both_transports() {
    support::run_io(proxy_agent_stream_fn_accepts_both_transports_case());
}
/// Drive the scenario of `proxy_agent_stream_fn_accepts_both_transports` on the test runtime.
async fn proxy_agent_stream_fn_accepts_both_transports_case() {
    let start = line(r#"{"type":"start"}"#);
    let text_start = line(r#"{"type":"text_start","contentIndex":0}"#);
    let aborted = (
        json!("aborted"),
        Some("Request aborted by user".to_owned()),
        String::new(),
    );
    let scenarios = [
        (canonical_lines(), (json!("stop"), None, "hé🙂!".to_owned())),
        (
            canonical_lines()[..5].to_vec(),
            (
                json!("error"),
                Some(PREMATURE.to_owned()),
                "hé🙂!".to_owned(),
            ),
        ),
        (vec![start, text_start], aborted),
    ];
    for route in [Route::Controlled, Route::Http] {
        for (lines, expected) in &scenarios {
            agent_scenario(route, lines.clone(), expected).await;
        }
    }
}
/// Notified at every assistant update the Agent publishes.
fn notify_on_update(
    agent: &std::sync::Arc<maestro_agent::Agent>,
) -> std::sync::Arc<tokio::sync::Notify> {
    let updated = std::sync::Arc::new(tokio::sync::Notify::new());
    let signal = std::sync::Arc::clone(&updated);
    agent.subscribe(std::sync::Arc::new(move |event, _| {
        let update = matches!(
            &*event.read().unwrap(),
            maestro_agent::AgentEvent::MessageUpdate { .. }
        );
        if update {
            signal.notify_one();
        }
        Box::pin(std::future::ready(Ok(())))
    }));
    updated
}
/// The variant names of every Agent event, in delivery order.
fn record_lifecycle(
    agent: &std::sync::Arc<maestro_agent::Agent>,
) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
    let kinds = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let sink = std::sync::Arc::clone(&kinds);
    agent.subscribe(std::sync::Arc::new(move |event, _| {
        use maestro_agent::AgentEvent as E;
        let name = match &*event.read().unwrap() {
            E::AgentStart => "AgentStart",
            E::AgentEnd { .. } => "AgentEnd",
            E::TurnStart => "TurnStart",
            E::TurnEnd { .. } => "TurnEnd",
            E::MessageStart { .. } => "MessageStart",
            E::MessageUpdate { .. } => "MessageUpdate",
            E::MessageEnd { .. } => "MessageEnd",
            E::ToolExecutionStart { .. } => "ToolExecutionStart",
            E::ToolExecutionUpdate { .. } => "ToolExecutionUpdate",
            E::ToolExecutionEnd { .. } => "ToolExecutionEnd",
        };
        sink.lock().unwrap().push(name.to_owned());
        Box::pin(std::future::ready(Ok(())))
    }));
    kinds
}
/// Run one prompt through the proxy over `route` and check how the Agent settles.
async fn agent_scenario(
    route: Route,
    lines: Vec<Vec<u8>>,
    expected: &(Value, Option<String>, String),
) {
    let abort = expected.0 == "aborted";
    let (backend, url, fetch) = Backend::start(route, lines);
    let agent = support::proxy::proxy_agent(support::proxy::proxy_stream_fn(url, fetch));
    let updated = notify_on_update(&agent);
    let lifecycle = record_lifecycle(&agent);
    let done = agent.prompt(support::owned::input("hello"));
    if abort {
        updated.notified().await;
        agent.abort();
    } else {
        backend.release();
    }
    done.await.unwrap();
    agent.wait_for_idle().await;
    assert_eq!(&last_assistant(&agent), expected, "{route:?}");
    let kinds = lifecycle.lock().unwrap().clone();
    let message_updates = if abort { 1 } else { 4 };
    let mut expected = vec![
        "AgentStart",
        "TurnStart",
        "MessageStart",
        "MessageEnd",
        "MessageStart",
    ];
    expected.extend(std::iter::repeat_n("MessageUpdate", message_updates));
    expected.extend(["MessageEnd", "TurnEnd", "AgentEnd"]);
    assert_eq!(kinds, expected, "{route:?}");
    assert!(!agent.state().read().unwrap().is_streaming());
    backend.release();
    backend.join();
}

/// Shared publication uses the very handle a controlled proxy start exposes.
#[test]
fn proxy_shared_owners_preserve_recorded_state() {
    support::run_io(async {
        let rows: Vec<Value> =
            serde_json::from_str(include_str!("fixtures/assistant-output.json")).unwrap();
        for row in rows.iter().filter(|row| row["kind"] != "synthetic_failure") {
            let (proxy, partial) = recorded_proxy_start(row).await;
            if row["kind"] != "proxy_initial" {
                recorded_proxy_failure(row, proxy, partial).await;
            }
        }
    });
}

/// Capture initial state and fully join its controlled producer before returning its handles.
async fn recorded_proxy_start(
    row: &Value,
) -> (
    maestro_models::AssistantMessageEventStream,
    maestro_models::SharedAssistantMessage,
) {
    use maestro_models::AssistantMessageEvent;
    use std::sync::Arc;
    let tasks = Arc::new(support::owned::Tasks::default());
    let (fetch, _, mut gate) = support::proxy::gated();
    let mut model = support::model();
    if row["kind"] == "proxy_initial" {
        model.api = row["input"]["model"]["api"].as_str().unwrap().into();
        model.provider = row["input"]["model"]["provider"].as_str().unwrap().into();
        model.id = row["input"]["model"]["id"].as_str().unwrap().into();
    }
    let before = maestro_models::timestamp_now();
    let proxy = maestro_agent::stream_proxy(
        model,
        support::proxy::context(),
        support::proxy::proxy(support::proxy::with_fetch(fetch)),
        tasks.spawner(),
    );
    let producer = tokio::spawn(tasks.take());
    gate.reads.recv().await.unwrap();
    gate.send
        .send(Some(Ok(line(r#"{"type":"start"}"#))))
        .unwrap();
    gate.reads.recv().await.unwrap();
    let Some(AssistantMessageEvent::Start { partial }) = proxy.next().await else {
        panic!("missing start")
    };
    if row["kind"] == "proxy_initial" {
        let mut actual = partial.read().unwrap().clone();
        assert!((before..=maestro_models::timestamp_now()).contains(&actual.timestamp));
        actual.timestamp = row["input"]["timestamp"].as_f64().unwrap();
        let expected: maestro_models::AssistantMessage =
            serde_json::from_value(row["expected"].clone()).unwrap();
        assert_eq!(actual, expected);
    }
    gate.send.send(Some(Ok(done("stop")))).unwrap();
    gate.send.send(None).unwrap();
    producer.await.unwrap();
    gate.dropped.await.unwrap();
    assert!(Arc::ptr_eq(&proxy.result().await, &partial));
    (proxy, partial)
}

/// Inject authored failure text at its public publication boundary, not at the aborting producer.
async fn recorded_proxy_failure(
    row: &Value,
    proxy: maestro_models::AssistantMessageEventStream,
    partial: maestro_models::SharedAssistantMessage,
) {
    use maestro_models::providers::assistant_output::publish_failure;
    use maestro_models::{AssistantMessageEventStream, Cancellation};
    use std::sync::Arc;
    let result = proxy.result().await;
    let signal = Cancellation::new();
    if row["input"]["aborted"].as_bool().unwrap() {
        signal.abort();
    }
    let expected = &row["expected"];
    partial.write().unwrap().content =
        serde_json::from_value(expected["output"]["content"].clone()).unwrap();
    let publication = if row["kind"] == "proxy_after_terminal" {
        proxy
    } else {
        AssistantMessageEventStream::new()
    };
    publish_failure(
        &publication,
        &partial,
        Some(&signal),
        row["input"]["message"].as_str().unwrap().into(),
    );
    let events = recorded_publication_events(&publication, &partial).await;
    assert!(Arc::ptr_eq(&publication.result().await, &result));
    let actual = partial.read().unwrap();
    assert_eq!(
        serde_json::to_value(&actual.content).unwrap(),
        expected["output"]["content"]
    );
    assert_eq!(
        serde_json::to_value(&actual.stop_reason).unwrap(),
        expected["output"]["stopReason"]
    );
    assert_eq!(
        json!(actual.error_message),
        expected["output"]["errorMessage"]
    );
    if row["kind"] == "proxy_after_terminal" {
        assert_eq!(json!(events), expected["eventTypes"]);
        assert_eq!(
            json!(Arc::ptr_eq(&result, &partial)),
            expected["sameMessage"]
        );
    } else {
        assert_eq!(json!(events), expected["events"]);
        assert_eq!(expected["ended"], true);
    }
}

/// Collect terminal events after producer completion, retaining identity observations.
async fn recorded_publication_events(
    publication: &maestro_models::AssistantMessageEventStream,
    partial: &maestro_models::SharedAssistantMessage,
) -> Vec<Value> {
    use maestro_models::AssistantMessageEvent;
    use std::sync::Arc;
    let mut events = Vec::new();
    while let Some(event) = publication.next().await {
        match event {
            AssistantMessageEvent::Done { message, .. } => {
                assert!(Arc::ptr_eq(&message, partial));
                events.push(json!("done"));
            }
            AssistantMessageEvent::Error { reason, error } => events.push(
                json!({"type":"error", "reason":reason,"sameMessage":Arc::ptr_eq(&error, partial)}),
            ),
            _ => panic!("unexpected publication"),
        }
    }
    events
}

/// A reconstructed call keeps the proxy's truthy fallback for each root kind.
#[test]
fn proxy_argument_roots_keep_their_source_fallback() {
    support::run_io(proxy_argument_roots_keep_their_source_fallback_case());
}
/// Drive the scenario of `proxy_argument_roots_keep_their_source_fallback` on the test runtime.
async fn proxy_argument_roots_keep_their_source_fallback_case() {
    let start = line(r#"{"type":"toolcall_start","contentIndex":0,"id":"c","toolName":"t"}"#);
    for (delta, expected) in [
        ("null", json!({})),
        ("false", json!({})),
        ("0", json!({})),
        (r#"\""#, json!({})),
        ("42", json!(42)),
        ("true", json!(true)),
        (r#"\"text\""#, json!("text")),
        ("[1,1]", json!([1, 1])),
        (r#"{\"q\":1}"#, json!({"q": 1})),
    ] {
        let lines = [
            start.clone(),
            line(&format!(
                r#"{{"type":"toolcall_delta","contentIndex":0,"delta":"{delta}"}}"#
            )),
            line(r#"{"type":"toolcall_end","contentIndex":0}"#),
            terminal(),
        ];
        let run = run_lines(&lines).await;
        assert_eq!(
            run.events[0],
            json!({"type": "toolcall_start", "contentIndex": 0})
        );
        assert_eq!(
            run.events[1],
            json!({"type": "toolcall_delta", "contentIndex": 0, "delta": delta.replace("\\\"", "\"")}),
            "{delta}"
        );
        let tool = json!({"type": "toolCall", "id": "c", "name": "t", "arguments": expected});
        assert_eq!(run.message()["content"], json!([tool]), "{delta}");
        assert_eq!(run.events[2]["toolCall"], tool, "{delta}");
    }
}
