# Cloud conversations

`maestro-models` streams Amazon Bedrock conversations directly through the AWS SDK for Rust.
`stream_bedrock(model, context, options)` takes `BedrockOptions`: the common `StreamOptions`,
`region`, `profile`, `request_metadata` (cost-allocation tags, sent as written) and
`bearer_token`. It returns the call's `AssistantMessageEventStream` at once, and every failure
the call detects ends that stream with an error update. The SDK owns credential lookup, request
signing, retries and the binary event framing; this module owns configuration, the request
input, hook ordering and the reduction of events into the shared message. This delivery sends
user text and the system prompt and reduces text answers. Assistant history, tool results,
images, tool declarations, cache markers and thinking settings are not converted yet, and
thinking and tool-call events are not reduced yet; the call is not registered with the
built-in dispatch, so only this function reaches it. Only `max_tokens`, `temperature`, `signal`,
`fetch` and the two hooks of the common options are read.

On native targets the call must run inside a Tokio runtime with its time and I/O drivers, and
the request work is owned by the runtime, so dropping the returned stream does not cancel it.
Called outside a runtime, the stream ends at once with `Streaming requires a running Tokio
runtime.`

## Region, profile and endpoint

The region is the nonempty `region` option, otherwise nonempty `AWS_REGION`, otherwise nonempty
`AWS_DEFAULT_REGION`. The profile is the nonempty `profile` option, otherwise nonempty
`AWS_PROFILE`. A model whose `base_url` is a standard runtime host
(`bedrock-runtime[-fips].<region>.amazonaws.com[.cn]`, matched without regard to case, ignoring
port and path) is only a source of region: with a configured region or profile the SDK derives
the endpoint and, for a profile, the region from its own configuration; with neither, the
endpoint is used and its region signs the request. Any other `base_url` is a custom endpoint
and is always used. A configured profile also stops a custom endpoint from falling back to
`us-east-1`: without a region the SDK reads it from the profile, and only without a profile does
`us-east-1` apply. An endpoint that is not a URL ends the stream with the URL library's text
after the payload hook ran.

## Authentication

The SDK's credential chain signs the request unless a bearer token is selected. The token is the
nonempty `bearer_token` option, otherwise nonempty `AWS_BEARER_TOKEN_BEDROCK`, and replaces
signing with `Authorization: Bearer <token>`. `AWS_BEDROCK_SKIP_AUTH` equal to `1` instead signs
with placeholder credentials and ignores both bearer sources, which is for proxies that do not
authenticate; any other value of that variable has no effect. No `api_key` is read.

Proxying is on when any of `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY` or their lowercase spellings
is nonempty. Then the proxy of each destination is the first nonempty of
`npm_config_<scheme>_proxy`, `<scheme>_proxy`, `npm_config_proxy` and `all_proxy`, each read as
its lowercase spelling and then its uppercase one. A proxy written without a scheme takes the
destination's scheme. The first nonempty of `npm_config_no_proxy` and `no_proxy`, read the same
way, excludes destinations from proxying: `*` excludes all, `host` that host only, `.suffix` and
`*suffix` hosts ending so, and a `:port` limits an entry; entries are separated by commas or
JavaScript whitespace. With proxying off, no proxy is used whatever other variables say. A proxy
or `AWS_BEDROCK_FORCE_HTTP1=1` makes the connection speak HTTP/1 only. HTTP and HTTPS proxies
are supported; SOCKS and proxy auto-configuration are not yet. A selected proxy that is not a
URL, or whose scheme is not `http` or `https`, ends the stream with an error before the
destination is contacted; a destination the exclusion list leaves out never reads its proxy. The
default sender returns a redirect response as it is, without following it, so it ends the call as
a response that is not accepted.

## Hooks and results

`on_payload` receives the request as the service input records spell it: `modelId`, `messages`,
`system` when the prompt is nonempty, `inferenceConfig` (present, possibly empty) and
`requestMetadata` when given. The value it returns replaces the request as a whole. Before
anything is sent, the model identifier is read for the route (one that is missing or not text
fails at the route, whatever else is malformed), then the rest is selected into the service's
input records: members the records do not declare are dropped, `null` members and list or map
entries are left out, `additionalModelRequestFields` keeps any JSON, and numbers are written
as doubles. The hook runs before the SDK looks up an endpoint, region or credentials, so its
failure ends the stream with its message as it is, before any of those can fail.
`on_response` receives the status and, under `x-amzn-requestid`, the first present of the
response headers `x-amzn-requestid`, `x-amzn-request-id` and `x-amz-request-id` when that value
is nonempty. It runs after the first event of the response has been decoded (or the response has
ended) and before any update is sent, so a response whose first frame fails to decode never
reaches it. A response that is not accepted, or whose first frame fails, ends the
stream with `<prefix>: <message>`; the prefixes are `Internal server error`, `Model stream
error`, `Validation error`, `Throttling error` and `Service unavailable` for those service
errors, and any other service error uses its own class name; an exception frame later in the
stream uses the same prefixes. Other SDK failures keep the SDK's own text with its causes.

Events reduce into the one shared message: `messageStart` starts it, a text delta opens a text
block for its block number when none is open and appends, `contentBlockStop` closes the block
of that number, `messageStop` sets the stop reason (`end_turn` and `stop_sequence` stop,
`max_tokens` and `model_context_window_exceeded` length, `tool_use` tool use, anything else
error, which ends the call with `An unknown error occurred`), and `metadata` sets the usage counts, with
the reported total unless it is zero. A message that starts with a user turn ends with
`Unexpected assistant message start but got user message start instead`. An aborted signal ends
the call with `Request was aborted` while the call waits for the response or an event, keeping
the text received so far; a callback failure that happened first keeps its own message and only
takes the aborted stop reason.

```rust
use std::sync::{Arc, Mutex};

use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::{Context, DiagnosticErrorInfo, Model, StreamOptions};

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "meta.llama-controlled", "name": "Controlled model", "api": "bedrock-converse-stream",
    "provider": "amazon-bedrock", "baseUrl": "https://bedrock-runtime.eu-central-1.amazonaws.com",
    "reasoning": false, "input": ["text"], "contextWindow": 8192, "maxTokens": 1024,
    "cost": {"input": 1, "output": 2, "cacheRead": 0, "cacheWrite": 0}
}))?;
let context: Context = serde_json::from_value(serde_json::json!({
    "systemPrompt": "Answer briefly.",
    "messages": [{"role": "user", "content": "question", "timestamp": 1}]
}))?;
let seen = Arc::new(Mutex::new(None));
let recorder = Arc::clone(&seen);
let options = BedrockOptions {
    common: StreamOptions {
        // Refuse the request after seeing it, so nothing is sent.
        on_payload: Some(Arc::new(move |payload, _| {
            *recorder.lock().expect("lock") = Some(payload);
            Box::pin(async {
                Err(DiagnosticErrorInfo {
                    name: None,
                    message: "not sent".into(),
                    stack: None,
                    code: None,
                })
            })
        })),
        ..StreamOptions::default()
    },
    bearer_token: Some("controlled-token".into()),
    ..BedrockOptions::default()
};

let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
runtime.block_on(async {
    let stream = stream_bedrock(model, context, Some(options));
    let message = stream.result().await;
    assert_eq!(message.read().expect("read").error_message.as_deref(), Some("not sent"));
});

let payload = seen.lock().expect("lock").take().expect("the hook ran");
assert_eq!(payload["modelId"], "meta.llama-controlled");
assert_eq!(payload["system"][0]["text"], "Answer briefly.");
assert_eq!(payload["messages"][0]["content"][0]["text"], "question");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Browsers

Bedrock is not available in browsers. On browser targets the call touches no process global
and no sender, runs neither hook, and returns a stream that has already ended with the error
`Bedrock is not supported in browsers.`; the SDK is not part of the browser build.
