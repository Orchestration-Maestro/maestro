# Cloud conversations

`maestro-models` streams Amazon Bedrock conversations directly through the AWS SDK for Rust.
`stream_bedrock(model, context, options)` takes `BedrockOptions`: the common `StreamOptions`,
`region`, `profile`, `request_metadata` (cost-allocation tags, sent as written),
`bearer_token`, `tool_choice`, `reasoning`, `thinking_budgets`, `interleaved_thinking` and
`thinking_display`. The option types `BedrockOptions` and `BedrockThinkingDisplay` are also
exported at the crate root. It returns the call's `AssistantMessageEventStream` at once, and
failures the call detects end that stream with an error update. The SDK owns credential lookup,
request signing, retries and binary event framing; this module owns configuration, request
input, hook ordering and event reduction. The consumed common options are `max_tokens`,
`temperature`, `cache_retention`, `signal`, `fetch` and the two hooks.

Assistant output uses `bedrock-converse-stream` even when the supplied model names another
API; request processing and payload hooks still receive the original descriptor.

## Conversation input

Message roles and content kinds are closed typed records. JSON model invocations
reject an unknown role or content block during decoding, before provider conversion.

History first passes through the [shared projection](../conversation-projection.md). User content keeps
its text and block order. Blank user and tool-result text is dropped, and a turn or result left
without content holds the placeholder `<empty>`. Blank assistant text and thinking are skipped;
retained text is not trimmed. Claude thinking without a nonblank signature becomes plain text; other models send
reasoning text without a signature. Consecutive tool results become one user turn, preserving
result order and error or success status.

Images remaining after [shared projection](../conversation-projection.md) accept `image/jpeg`,
`image/jpg`, `image/png`, `image/gif` and `image/webp`. The payload hook sees decoded byte arrays;
the sent body carries padded standard base64. An unsupported MIME type or invalid base64 in a
remaining image ends the stream before the payload hook. Tool declarations keep their
descriptions and schema values. With nonempty tools, `tool_choice` selects automatic, required
or named tool use; `ToolChoice::None`, absent tools or empty tools omit the tool configuration.

Cache retention follows the shared [options policy](../model-options.md#cache-retention). None disables cache points. Supported
Claude candidates receive a cache point after a nonempty system prompt and on the final
converted turn only when it is a user turn. Long retention adds `ttl: "1h"`.
`AWS_BEDROCK_FORCE_CACHE=1` enables cache points only when neither the identifier nor the name,
including their separator-normalized forms, contains `claude`.

A Claude request without an explicit `max_tokens` sends the model's advertised cap as
`inferenceConfig.maxTokens`; other Bedrock models send none unless asked.

Thinking settings apply only to reasoning Claude models. Opus 4.6 to 4.8, Sonnet 4.6 and Sonnet 5
candidates use adaptive thinking with mapped effort; Opus 4.7 preserves native extra-high effort
before consulting the descriptor map. Other Claude candidates use fixed budgets from
[`ThinkingBudgets`](../model-options.md), with interleaved thinking enabled unless explicitly
false. Display defaults to summarized; a government region selected from the option or
region environment variables, or a government model identifier, omits the display. The
endpoint and profile region do not determine that display decision.

`stream_simple_bedrock`, in `maestro_models::providers::cloud::amazon_bedrock`, accepts
`SimpleStreamOptions`, resolves [base options and token budgets](../model-options.md), adjusts
fixed-budget Claude token limits and the selected budget slot, then calls `stream_bedrock`.
Adaptive Claude and other models do not receive that adjustment.

The catalog generator also marks Messages models whose case-sensitive IDs contain
`opus-4-6`, `opus-4.6`, `opus-4-7`, `opus-4.7`, `opus-4-8`, `opus-4.8`, `sonnet-4-6`,
`sonnet-4.6`, `sonnet-5` or `sonnet.5` with `forceAdaptiveThinking: true`, retaining their
other compatibility settings. The [Messages adapter](messages.md#thinking) sends adaptive
thinking and mapped effort for these models without a thinking budget or interleaved-thinking beta.

Text, thinking and tool-call updates reduce into the shared assistant message. Generic
`stream`, `stream_simple`, `complete` and `complete_simple` dispatch through the bundled
registration; [module linkage](module-linkage.md) describes callback replacement and raw extras.

`stream_bedrock_for` and `stream_simple_bedrock_for` in the same module take an
explicit execution family and producer submitter. `BedrockOptions<E>` retains
that family's payload and response hooks; the SDK client and first-event
response admission stay unchanged. These functions are not root exports.

On native targets the default call must run inside a Tokio runtime with its time and I/O drivers, and
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

The credential chain, composed from the native providers, signs the request unless a bearer token is
selected. Without a nonempty explicit `profile` option, an empty `AWS_PROFILE` selects nothing, so
the environment's keys come first and the `default` profile is the fallback: the `default` profile then
runs the same sources as a selected profile, from its own source to container or instance credentials.
A profile named by the option or by `AWS_PROFILE` replaces the environment's keys.

When a profile is selected the standalone environment-keys step is skipped (a profile may still
name the environment as the credential source of a role) and these sources run in order, the next
one only after a source that did not apply:

1. The selected profile. A profile that assumes a role is resolved from its `source_profile` or
   named `credential_source` (`Environment`, `EcsContainer` or `Ec2InstanceMetadata`) and each role
   is assumed from the source toward the selected profile; a source profile with complete static
   keys ends that walk. A profile that assumes no role uses, in this order, complete static keys,
   web identity (`web_identity_token_file` with `role_arn`), `credential_process`, SSO, then login
   (`login_session`). A chained role therefore wins over the static keys of the profile that
   names it, and a login profile can be the source of a role.
2. The selected profile's own `credential_process`, once more.
3. Web identity from `AWS_WEB_IDENTITY_TOKEN_FILE` and `AWS_ROLE_ARN`.
4. Container credentials when `AWS_CONTAINER_CREDENTIALS_RELATIVE_URI` or
   `AWS_CONTAINER_CREDENTIALS_FULL_URI` is nonempty, which excludes instance metadata even when
   the container lookup fails; otherwise instance metadata, unless `AWS_EC2_METADATA_DISABLED`
   is nonempty and not `false`.

These let the chain go on when no other source of the profile decides first: a missing profile,
incomplete static keys, absent environment keys for a role source, a missing or unsupported
source profile or credential source, a source cycle, and a credential process that fails, prints
unusable output or returns expired credentials. A web identity, role-assumption or SSO source that
was selected and fails ends it, as does a role that
needs a multi-factor code (`Profile <name> requires multi-factor authentication, but no MFA code
callback was provided.`), an SSO profile with a missing field (`Missing required SSO field:
<field>.`) or session (`Missing SSO session: <session>.`), and a container or instance source
whose own error is not a missing-credentials one.

A login profile signs with the token the AWS CLI cached for its session, in
`~/.aws/login/cache` or the directory named by `AWS_LOGIN_CACHE_DIRECTORY`. The native provider
reads the cache and refreshes a token that is about to expire, following the retry settings of the
shared configuration. A refresh the sign-in service refuses as access denied, whatever its reason,
ends the chain, and so does a cache that is missing or incomplete. Any other failure of the token
request (validation, server, throttling or transport) lets the chain go on to the next source,
decided from the error type and never from its text. A refreshed token is used even when saving
it back to the cache fails.

A named SSO session supplies only the start URL and region (a session without them leaves the
field missing, whatever the profile says); the account and role come from the profile. A profile
with only a start URL uses the legacy token cache, and Maestro checks no token expiry itself; token
renewal and its failures belong to the native SSO provider. Every role of
a walk is assumed in the region of the first role profile when it sets one, so a later role
profile's own `region` has no effect. Retries and token caches belong to the SDK, as do the
session names of assumed roles, except that a web identity source without `role_session_name`
gets a name generated here.

The bearer token is the nonempty `bearer_token` option, otherwise nonempty `AWS_BEARER_TOKEN_BEDROCK`, and replaces
signing with `Authorization: Bearer <token>`. `AWS_BEDROCK_SKIP_AUTH` equal to `1` instead signs
with placeholder credentials and ignores both bearer sources, which is for proxies that do not
authenticate; any other value of that variable has no effect. No `api_key` is read.

Login refresh, role assumption, web identity, SSO credentials and named-session OIDC refresh use
conversation routing, including a supplied sender and [destination-specific proxy selection](#proxy-routing).
Container and instance metadata stay direct, including named role sources, and do not use the
supplied sender. Default connectors retain connection pools by SDK timeout settings and apply the SDK's
connection and response-header deadlines, not a body-read deadline;
a supplied sender retains its own timeout policy.

## Proxy routing

Proxying is on when any of `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY` or their lowercase spellings
is nonempty. Then the proxy of each destination is the first nonempty of
`npm_config_<scheme>_proxy`, `<scheme>_proxy`, `npm_config_proxy` and `all_proxy`, each read as
its lowercase spelling and then its uppercase one. A proxy written without a scheme takes the
destination's scheme. The first nonempty of `npm_config_no_proxy` and `no_proxy`, read the same
way, excludes destinations from proxying: `*` excludes all, `host` that host only, `.suffix` and
`*suffix` hosts ending so, and a `:port` limits an entry; entries are separated by commas or
JavaScript whitespace. With proxying off, no proxy is used whatever other variables say. A proxy
or `AWS_BEDROCK_FORCE_HTTP1=1` makes the connection speak HTTP/1 only. HTTP and HTTPS proxies
are supported, as are `socks`, `socks4`, `socks4a`, `socks5` and `socks5h`. `socks4` and
`socks5` resolve destination names locally; the other SOCKS schemes send names to the proxy.
SOCKS4 sends IPv6 destinations as names; SOCKS5 retains their address family.
SOCKS URL credentials are percent-decoded once as UTF-8; `+` stays a plus. Invalid escapes
fail when selected. SOCKS4 sends the complete user ID followed by NUL, including an empty ID;
passwords are decoded but not transmitted. For SOCKS5, configured credentials offer both
no-login and username/password methods, including when one field is empty. A SOCKS5 field
exceeding 255 UTF-8 bytes fails only when the proxy
selects password authentication. Empty credentials use no-login alone.

The SOCKS handshake has a 30-second deadline covering proxy connection and negotiation,
after local destination lookup and before destination TLS. Each SOCKS request owns its
connection through response-body consumption; cancellation or a header deadline closes
pending connection work. Request preparation and response conversion use the
[shared HTTP transport](chat-completions.md#transport).
A selected proxy that is not a URL or has an unsupported scheme ends the stream with an error
before the destination is contacted; a destination the exclusion list leaves out never reads
its proxy. The default sender returns redirects without following them.

### Proxy scripts

A selected `pac+data:`, `pac+file:`, `pac+ftp:`, `pac+http:` or `pac+https:` URL loads a
proxy script instead of opening a direct destination connection. Both opaque
`pac+data:,script` and hierarchical `pac+data://,script` spellings are accepted.
The script receives the complete destination URL and its canonical host, including
a nondefault port in the URL, before choosing ordered DIRECT, HTTP, HTTPS or SOCKS
candidates. Unknown directives advance; malformed recognized proxy URLs fail.
Fallback happens only before application bytes are sent. A connected proxy's
CONNECT refusal is returned with its status, headers and body, without replay.

Scripts have the standard host, DNS, network, local-address, time, weekday and
calendar helpers. `myIpAddress()` discovers the local IPv4 through a UDP route
lookup for `8.8.8.8:53`. It sends no packet and needs no answering endpoint or
time limit. The socket closes before returning the address. On any socket error,
it falls back to native interfaces, selecting the first non-loopback IPv4 on the
first eligible interface and preserving interface and per-interface address
order. An authored `alert` emits its exact message through tracing rather than
printing.

Each bridge retains separate script globals for each destination protocol and
source URL, independently of its connection timeout settings. Compilation resolves
and retains `FindProxyForURL`, including lexical declarations; later reassignment
does not replace that callable. Missing or non-callable exports fail compilation.
Unchanged source keeps that context; successfully compiled replacement code
creates a new one. Failed compilation cannot publish source metadata or
revive an old script through a later unchanged response. File sources compare the
opened file's millisecond modification time; HTTP and HTTPS sources use cache
headers, validators and up to five redirects; FTP sources ask the server for the
file's modification time and compare it at millisecond precision.
Source requests do not recursively apply the script they are loading. Destination
exclusions and a supplied sender skip script loading entirely.

An FTP source logs in with the URL's decoded credentials, or anonymously when it has
none, at the URL's port, or 21 when it names none. After the greeting it enables UTF-8,
logs in, reads the server's features, selects binary mode and file structure, and asks a
machine listing to carry the facts it reads. A password never appears in a diagnostic: a
rejected command reports the control characters it held, without the command. It asks the
modification-time command first: a refused file is not found, while an unsupported command
falls back to listing the containing directory. Only a machine listing carries a
comparable instant, so an entry from a Unix or DOS listing is not found; a listing in no
accepted format reports the listing format as its own error. Machine-listing entries keep
the name the server spelled and the stamp it reported, including a fractional stamp, a
symbolic-link type, and a name that holds a semicolon or starts with a space; the last
nonempty modification fact wins, and a current- or parent-directory entry is not a source
entry. A listing asks for a machine listing first when the server advertises one, then for
`LIST -a` and `LIST`, and keeps the first command the server accepts; only a refused
command advances to the next one. A transfer the server refuses is not found rather than an
empty script, and a transfer that breaks off is an error rather than a shortened one.

A passive data connection opens in extended passive mode first, taking only the port from a
reply that may delimit it with `|` or, as the IBM i server does, with `!`. When that preparation
fails the loader falls back to the classic mode, which keeps the data host the server advertises
unless that address is private and the control connection's is not. Every completed read or
write restarts its own connection's inactivity window, so a greeting, reply or transfer that
keeps arriving byte by byte outlives thirty seconds while a silent connection fails at its own
boundary. The data connection's window starts when the server admits the transfer, and control
replies during the transfer neither end nor restart it; once the data ends, the control
connection's window runs again until the completion arrives. A transfer
settles only when both the data connection ended and the control connection reported
completion, in either order. A directory listing larger than 40 MiB is refused, and the control
and data connections close on success, failure and cancellation, with the goodbye sent without
waiting for its reply.

## Hooks and results

`on_payload` receives the request in `PayloadHookInput::payload` as the service input
records spell it: `modelId`, `messages`,
`system` when the prompt is nonempty, `inferenceConfig` (present, possibly empty) and
`requestMetadata` when given, plus tool configuration and model-specific thinking fields when
selected. The returned record's JSON replaces the request as a whole; see
[hook records](../records.md). Before
anything is sent, the model identifier is read for the route (one that is missing or not text
fails at the route, whatever else is malformed), then the rest is selected into the service's
input records: undeclared ordinary record members are dropped, `null` known record members and list or map
entries are left out, free-form tool input, tool schema, structured tool-result JSON and `additionalModelRequestFields` keep nested JSON nulls,
and numbers are written
as doubles. The hook runs before the SDK looks up an endpoint, region or credentials, so its
failure ends the stream with its message as it is, before any of those can fail.
The payload hook can supply known document, audio/video, search/citation and guard-content
shapes. Guard content is selected in both message and system blocks. Tools also select
structured result JSON, document/video/search results, server tool/result types, strict
selection, system tools and cache points. Blob fields take byte arrays and are sent as padded
standard base64, including empty arrays.

Declared request unions accept `$unknown: [name, value]` when no known member survives.
A non-null known member suppresses fallback, even when its value is empty or false; an invalid
known member fails rather than activating fallback. Only a selected non-null fallback is
decoded as a string/document pair. Missing or null fallback leaves the empty known record.
The supplied name is literal, including `__proto__`, and its document retains nested nulls
and arrays, even when the name matches a known blob field. Ordinary records do not accept
this fallback. Open values use the [JSON owner's canonical formatting](../json.md).

`on_response` receives the status and, under `x-amzn-requestid`, the first present of the
response headers `x-amzn-requestid`, `x-amzn-request-id` and `x-amz-request-id` when that value
is nonempty. It runs after the first event of the response has been decoded (or the response has
ended) and before any update is sent, so a response whose first frame fails to decode never
reaches it. A response that is not accepted, or whose first frame fails, ends the
stream with `<prefix>: <message>`; the prefixes are `Internal server error`, `Model stream
error`, `Validation error`, `Throttling error` and `Service unavailable` for those service
errors, and any other service error uses its own class name; an exception frame later in the
stream uses the same prefixes. Other SDK failures keep the SDK's own text with its causes.

Events of a kind the SDK does not know are skipped before anything else sees them; the
response hook waits for the first known event or the end of the stream. Events reduce into the one shared message: `messageStart` starts it, a text delta opens a text
block for its block number when none is open and appends. Reasoning updates similarly open
thinking blocks; text updates emit thinking deltas, while signatures append silently. A tool
start appends a tool-call block, and each input fragment reparses its accumulated arguments
through the [streamed JSON parser](../model-options.md). Deltas select the first still-open
block with that service number and update only a matching content kind. Closing removes
that open match, so reusing the number can create a fresh block. Public content indices
follow arrival order rather than service numbering. Block numbers and unfinished JSON
buffers stay private, including on successful EOF without block stops. `contentBlockStop`
emits the selected block's text, thinking or finalized tool call; `messageStop` sets the stop reason (`end_turn` and `stop_sequence` stop,
`max_tokens` and `model_context_window_exceeded` length, `tool_use` tool use, anything else
error, which ends the call with `An unknown error occurred`), and `metadata` sets the usage counts, with
the reported total unless it is zero. A message that starts with a user turn ends with
`Unexpected assistant message start but got user message start instead`. An aborted signal ends
the call with `Request was aborted` while the call waits for the response or an event, keeping
the content received so far; a callback failure that happened first keeps its own message and only
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
    common: StreamOptions::<maestro_models::Threaded> {
        // Refuse the request after seeing it, so nothing is sent.
        on_payload: Some(Arc::new(move |payload, _| {
            *recorder.lock().expect("lock") = Some(payload.payload);
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
