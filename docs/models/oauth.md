# OAuth authorization

`maestro-models` exports OAuth records, `generate_pkce`, `oauth_success_html` and
`oauth_error_html`, callback/provider contracts, Anthropic subscription login
and refresh, and response-account `login_openai_codex`,
`refresh_openai_codex_token` and `OPENAI_CODEX_OAUTH_PROVIDER`,
and GitHub Copilot device login and refresh.

`OAuthCredentials` carries token strings, an expiry number in Unix epoch
milliseconds and flattened provider extension fields. It does not validate
credentials or persist them. Prompt and authorization records omit absent
optional fields during serialization but retain explicit empty text and false.
Selection prompts keep the supplied option order and repeated identifiers.

PKCE requests 32 secure random bytes, encodes an unpadded URL-safe Base64
verifier, and hashes that verifier's UTF-8 text with SHA-256 to produce the
identically encoded challenge. System entropy failures retain their native
message in `DiagnosticErrorInfo`; no error name, code or stack is added.

Callback pages escape `&`, `<`, `>`, double quotes and apostrophes in caller
message and details text. Other characters and whitespace remain unchanged;
existing entities are escaped again. Error details are omitted only when absent
or empty. Both documents retain the centered dark callback layout and fixed
authentication wording. Embedded `assets/brand/brand.json` supplies the dark
semantic colors and ordered display, body and mono font stacks.
Primary font families and non-generic fallback names are quoted; CSS generic
fallback keywords remain unquoted, recognized case-insensitively. Stack order
and supplied case are preserved. `assets/brand/mark.svg` supplies the fitted mark.
Required pack fields and reached color aliases must resolve; failures return
diagnostic messages rather than a
fallback palette. Replacing these assets changes the identity at build time;
there is no runtime asset loader or network font acquisition.

## No-network example

```rust
use maestro_models::{generate_pkce, oauth_success_html, OAuthCredentials};

let credentials = OAuthCredentials {
    refresh: "supplied-refresh".into(),
    access: "supplied-access".into(),
    expires: 0.125,
    extra: Default::default(),
};
let key = generate_pkce()?;
assert_eq!(key.verifier.len(), 43);
let page = oauth_success_html("You may close this tab.")?;
assert!(page.contains("<p>You may close this tab.</p>"));
assert_eq!(credentials.expires, 0.125);
# Ok::<(), maestro_models::DiagnosticErrorInfo>(())
```

## Subscription accounts

`login_anthropic(callbacks: OAuthCallbacks, fetch: Option<Fetch>)` returns a
`BoxFuture<Result<OAuthCredentials, OAuthError>>`. `OAuthCallbacks` retains an
`Arc<dyn OAuthLoginCallbacks + Send + Sync>` natively, or an
`Arc<dyn OAuthLoginCallbacks>` in a browser. The caller supplies required
`on_auth` and asynchronous `on_prompt` methods. Progress defaults to a successful
no-op; manual input, selection and signal default to absent.

Anthropic uses auth, prompt, progress and optional concurrent manual input. It
ignores selection and the caller's signal. Callback waiting has no deadline.
Pasted input recognizes an absolute URL, then code/state separated by `#`, then
query text containing `code=`, then a bare code. Nonempty mismatching state is
rejected; absent state uses the verifier and explicitly empty state does not.
Rejected callback requests leave the same wait available for later success.
Manual input continues independently if a callback selects the code first.

Login binds a native listener at `127.0.0.1:53692`; a nonempty
`MAESTRO_OAUTH_CALLBACK_HOST` changes only its binding host. A binding that
conflicts with the configured address makes login return a native binding error
before interaction callbacks run. The redirect remains
`http://localhost:53692/callback`. A browser binding attempt returns
`Anthropic OAuth requires a native callback listener`; importing the library,
PKCE, pages and refresh do not require that listener. Interaction is supplied by
library callbacks: this library does not open a browser or provide a UI.

Token exchange is started before listener shutdown. Login observes accepting
listener shutdown before returning a result. Shutdown closes idle admitted
connections and disables keep-alive while allowing in-flight responses to finish;
login does not wait for every admitted peer to drain. Dropping the login future
requests shutdown.

`refresh_anthropic_token(refresh_token: String, fetch: Option<Fetch>)` returns
`BoxFuture<Result<OAuthCredentials, OAuthError>>`. Each token operation invokes
one selected Fetch (the shared default when absent) with one 30-second deadline
covering headers and body consumption. Full response text is consumed before
checking HTTP status. Required token fields are decoded after last-member
selection; unread fields do not become credential extras. Invalid token types
or nonfinite expiry fail validation. Expiry uses the completion clock plus
`expires_in * 1000 - 300000`; empty tokens and negative durations are retained.
Neither login nor refresh persists credentials.

`ANTHROPIC_OAUTH_PROVIDER` implements `OAuthProviderInterface`: `id`, `name`,
`uses_callback_server`, `login`, `refresh_token`, `get_api_key` and
`modify_models`. It reports `anthropic`, `Anthropic (Claude Pro/Max)` and
`Some(true)`, delegates login/refresh, and borrows `credentials.access` as its
API key. The default model modifier returns the original moved vector without
changing its contents, order or allocation. Other providers may override it
with a fallible transformation.

`OAuthError` retains boxed `DiagnosticErrorInfo`, optional supplied/native
`errno` and an optional wrapped `cause`. Display prints its message;
`Error::source` exposes the wrapped cause. Token error context uses
`OAuthError`'s supplied-field formatting for Fetch diagnostics, which carry name,
message, code and stack but not errno or nested sources. Native binding errors
retain native messages and available errno; no engine stack is manufactured.


## Response accounts

`login_openai_codex(callbacks: OAuthCallbacks, originator: Option<String>,
fetch: Option<Fetch>)` returns `BoxFuture<Result<OAuthCredentials, OAuthError>>`.
It generates PKCE followed by an independent 16-byte hexadecimal state. The
originator defaults to `maestro`; explicit empty text is retained.

The native listener binds port 1455, with the binding-host setting described
above and redirect `http://localhost:1455/auth/callback`. While waiting for a callback, native
binding or listener failure permits manual/prompt fallback; browser serving
returns `Response-account OAuth is only available in native environments`.
Pasted input uses the shared recognition described under Subscription accounts.
Nonempty mismatching pasted state fails with `State mismatch`; absent or empty
pasted state is accepted. Callback requests instead require exact matching
state before accepting a nonempty code.

Authorization notification precedes concurrent manual input. Settled manual
errors are checked before selecting an available callback code; losing manual
work continues independently. Progress, selector and callback signal are not
consulted. The listener remains owned through token processing and account
extraction; explicit completion observes accepting-listener shutdown, while
abandoning the future requests it. Admitted-peer shutdown uses the shared
listener policy described above.

`refresh_openai_codex_token(refresh_token: String, fetch: Option<Fetch>)` returns
`BoxFuture<Result<OAuthCredentials, OAuthError>>` without serving a callback.
Each reached token operation sends one ordered form POST through the selected Fetch, without
adding a deadline, retry or cancellation signal. See
[`default_fetch`](https://docs.rs/maestro-models/latest/maestro_models/fn.default_fetch.html)
for the default transport's platform policy. Neither operation persists credentials.

Token validation requires nonempty access/refresh strings and finite numeric
duration and expiry. Expiry is the completion clock plus `expires_in * 1000`,
without a buffer. Successful credentials carry only the selected tokens,
expiry and `extra["accountId"]`: a nonempty string extracted from the access
payload, not signature or issuer verification. Missing account metadata fails
with `Failed to extract accountId from token`. Refresh transport/decoding
failures receive one refresh-error context; authored token failures and account
extraction failures retain their own messages. Refresh writes no token-failure
messages to stderr.

`OPENAI_CODEX_OAUTH_PROVIDER` delegates these operations with the default
originator, borrows the access key and inherits the unchanged model modifier.

### Controlled refresh example

This example supplies its response without contacting an account service:

```rust
use maestro_models::{Fetch, HttpResponse, refresh_openai_codex_token};
use std::{collections::BTreeMap, sync::Arc};

let fetch: Fetch = Arc::new(|_| Box::pin(async {
    use base64::Engine as _;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acct_123"}}"#);
    let body = format!(
        r#"{{"access_token":"h.{payload}.s","refresh_token":"rotated","expires_in":3600}}"#
    );
    Ok(HttpResponse {
        status: 200,
        status_text: String::new(),
        headers: BTreeMap::new(),
        body: Box::pin(futures_util::stream::iter([Ok(body.into_bytes())])),
    })
}));
let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
let credentials = runtime.block_on(refresh_openai_codex_token("old".into(), Some(fetch)))?;
assert_eq!(credentials.extra["accountId"], "acct_123");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Device accounts

`login_github_copilot(callbacks, fetch)` prompts for an optional enterprise
URL/domain, then publishes the device response's verification URI and code.
`normalize_domain` trims authorization whitespace and returns a parsed hostname;
valid hostless URLs return an empty hostname. Login rejects a nonblank input
without a nonempty hostname and otherwise uses `github.com` for blank input.

Polling waits before every request. Its initial interval is the larger of one
second and the floored server interval in milliseconds, with a 1.2 safety
multiplier. Pending and unknown responses continue polling; slow-down selects a
positive supplied interval or adds five seconds, then uses a 1.4 multiplier.
The multiplied interval is rounded upward; each wait is bounded by the
remaining server lifetime. A poll
admitted before expiry still runs after its wait: success or a server error can
win at expiry. Timeout after a slow-down response includes clock-sync guidance.
Nonfinite consumed timing operands, nonfinite deadlines or remaining lifetimes,
and selected waits that cannot fit a duration return the device-fields error;
a negative remaining wait becomes zero. Overflowing interval arithmetic is
accepted when the clamp or remaining lifetime selects a finite wait.
Native waits remain subject to the runtime's timer limits; an unrepresentable
native deadline falls back to about 30 years.

The login signal is checked after prompting, at entered poll iterations and
while waiting, not during Fetch calls. Completed waits release their cancellation
observers. A callback failure stops its phase. After token exchange, login emits
`Enabling models...` and waits for concurrent policy attempts for every current
catalog model of this provider; individual HTTP and transport policy failures
are ignored and policy bodies are not read.

`refresh_github_copilot_token(refresh_token, enterprise_domain, fetch)` returns
service credentials with `expires_at * 1000 - 300000` as the expiry. Empty tokens
and finite zero, negative or fractional expiry are accepted; nonfinite expiry
is rejected. A nonempty supplied enterprise string is used verbatim and
retained; an empty string selects the standard endpoint but remains in returned
metadata. Login and refresh do not store credentials or start
a callback listener. Requests use one selected Fetch without retries or an
additional deadline; see the [transport guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/chat-completions.md#transport) for the shared adapter.

`get_github_copilot_base_url(token, enterprise_domain)` uses the first nonempty
`proxy-ep=` substring in a token, replacing only its leading `proxy.` with
`api.`. Without it, a nonempty enterprise operand selects
`https://copilot-api.{domain}`; otherwise the standard account endpoint is used.
The helper does not parse or normalize the supplied endpoint text.

`GITHUB_COPILOT_OAUTH_PROVIDER` reports `github-copilot` and `GitHub Copilot`,
borrows the access key and delegates login and refresh. Its model modifier
normalizes truthy enterprise metadata and changes only matching-provider base URLs,
preserving descriptor order and other fields. Absent, null, false, zero or empty
enterprise metadata skips modifier normalization. Refresh retains supplied string
or null metadata; absent metadata remains absent. Other non-string values fail
before refresh effects; truthy non-string values fail before model modification
with a native decoding error. These exports are also available through
`oauth::device::github_copilot` and `oauth`.

### Controlled refresh example

```rust
use maestro_models::{Fetch, HttpResponse, refresh_github_copilot_token};
use std::sync::Arc;

let fetch: Fetch = Arc::new(|request| {
    assert_eq!(request.method, "GET");
    assert_eq!(request.headers["authorization"], "Bearer supplied-refresh");
    Box::pin(async {
        Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: Default::default(),
            body: Box::pin(futures_util::stream::iter([Ok(
                br#"{"token":"controlled-access","expires_at":1234}"#.to_vec(),
            )])),
        })
    })
});
# async fn example(fetch: Fetch) -> Result<(), maestro_models::OAuthError> {
let credentials = refresh_github_copilot_token(
    "supplied-refresh".into(), None, Some(fetch),
).await?;
assert_eq!(credentials.access, "controlled-access");
assert_eq!(credentials.expires, 934000.0);
# Ok(())
# }
# tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(example(fetch))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
