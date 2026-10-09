# OAuth authorization

`maestro-models` exports OAuth records, `generate_pkce`, `oauth_success_html` and
`oauth_error_html`, subscription callback/provider contracts and Anthropic
subscription login and refresh.

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
`MAESTRO_OAUTH_CALLBACK_HOST` changes only its binding host. Another process
holding port 53692 makes login return a native binding error before interaction
callbacks run. The redirect remains
`http://localhost:53692/callback`. A browser binding attempt returns
`Anthropic OAuth requires a native callback listener`; importing the library,
PKCE, pages and refresh do not require that listener. Interaction is supplied by
library callbacks: this library does not open a browser or provide a UI.

Token exchange is started before listener shutdown. Login observes accepting
listener shutdown before returning a result, but does not wait for every
admitted peer to drain. Dropping the login future requests shutdown.

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
`Error::source` exposes the wrapped cause. Token error context uses supplied
error names, codes, errno, nested causes and nonempty supplied stack text.
Native errors retain native messages; no engine stack is manufactured.
