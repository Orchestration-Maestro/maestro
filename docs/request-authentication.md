# Request authentication

Model invocation forwards a supplied `StreamOptions.api_key` unchanged. It does
not select credentials, check empty secrets, authorize secret-free endpoints,
overlay headers or call an auth resolver. Resolve the credential owner explicitly
before supplying an adapter's api key. See [provider credentials](credentials.md).

`AuthResolver`, `RequestAuth`, `SecretString`, `AuthStatus`, `TokenExchange` and
`TokenExchangeResult` remain the credential owner's records and interfaces.
`SecretString::expose()` is deliberate access; its Debug output is a fixed marker,
not memory zeroization. Source labels and credential names must be non-secret.
Configured status is metadata, not validation or a prediction of request success.

The credential owner supplies precedence, persistence, helper execution and
cancellation behavior. Token exchange is an explicit credential primitive, not
an invocation side effect. Adapters are responsible for keeping supplied secrets
out of generated content and diagnostics. Supplied option/header maps are
sensitive surfaces and must not be logged.

## Environment credentials

`find_env_keys(provider)` returns nonempty API-key variable names in precedence
order, or `None`. `get_env_api_key(provider)` returns the first value, an ambient
`<authenticated>` marker, or `None`. Values are not trimmed or cached. Provider
names are exact. Neither function performs network requests or loads credentials.

Anthropic checks `ANTHROPIC_OAUTH_TOKEN` before `ANTHROPIC_API_KEY`. GitHub Copilot
checks `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, then `GITHUB_TOKEN`. Fireworks checks
`FIREWORKS_API_KEY`. Ambient signals never appear in `find_env_keys`.

Google Vertex first checks `GOOGLE_CLOUD_API_KEY`. Otherwise it requires an
existing `GOOGLE_APPLICATION_CREDENTIALS` path, or the home directory's
`.config/gcloud/application_default_credentials.json` when that variable is
absent or empty, plus `GOOGLE_CLOUD_PROJECT` or `GCLOUD_PROJECT` and
`GOOGLE_CLOUD_LOCATION`. An explicit missing path does not fall back. Only
existence is checked, and that result is cached for the process lifetime.
Credential paths retain native operating-system characters, including non-UTF-8
paths on Unix.

Amazon Bedrock recognizes `AWS_PROFILE`, paired `AWS_ACCESS_KEY_ID` and
`AWS_SECRET_ACCESS_KEY`, `AWS_BEARER_TOKEN_BEDROCK`,
`AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`, `AWS_CONTAINER_CREDENTIALS_FULL_URI`,
or `AWS_WEB_IDENTITY_TOKEN_FILE`. Any nonempty alternative returns the marker
without checking credential contents.

### Native behavior differences

- Unknown providers, including `constructor` and other object-property names,
  return `None`; no inherited-property lookup is performed.
- Browser builds use the empty standard environment and return `None` rather
  than reporting a missing-global error. Page-defined environment shims are not
  read.
- Standard Rust environment, home-directory and filesystem facilities are used
  directly: non-UTF-8 secret values are not strings, home lookup uses the native
  account fallback, and path joining does not lexically normalize components.
  There is no runtime-specific environment recovery or import-readiness race.

```rust
use maestro_models::{find_env_keys, get_env_api_key};

assert!(find_env_keys("controlled-unknown-provider").is_none());
assert!(get_env_api_key("controlled-unknown-provider").is_none());
```
