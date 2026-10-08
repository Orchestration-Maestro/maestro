# Shared OAuth primitives

`maestro-models` exports OAuth records, `generate_pkce`, `oauth_success_html` and
`oauth_error_html`. Account login and refresh are not delivered here.

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
