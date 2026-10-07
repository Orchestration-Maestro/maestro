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

find_env_keys reports the nonempty environment-variable names for a provider in precedence order; it does not return their values or list ambient credential signals. get_env_api_key returns the first value or the ambient marker `<authenticated>`. Both return Result with None for ordinary absence. Provider names are exact and values are not trimmed.

Anthropic checks ANTHROPIC_OAUTH_TOKEN before ANTHROPIC_API_KEY. GitHub Copilot checks COPILOT_GITHUB_TOKEN, GH_TOKEN and GITHUB_TOKEN in that order. Fireworks checks FIREWORKS_API_KEY. The built-in provider table also retains its inherited-name string coercions; an unknown provider otherwise has no key.

Google Vertex first checks GOOGLE_CLOUD_API_KEY. Without that key, it requires an existing GOOGLE_APPLICATION_CREDENTIALS path, or the conventional home/.config/gcloud/application_default_credentials.json path when no explicit path is set, plus GOOGLE_CLOUD_PROJECT or GCLOUD_PROJECT and GOOGLE_CLOUD_LOCATION. Only existence is checked; an explicit missing path does not fall back. The completed existence probe is cached for the process lifetime.

Amazon Bedrock recognizes AWS_PROFILE, paired AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, AWS_BEARER_TOKEN_BEDROCK, AWS_CONTAINER_CREDENTIALS_RELATIVE_URI, AWS_CONTAINER_CREDENTIALS_FULL_URI or AWS_WEB_IDENTITY_TOKEN_FILE. Any nonempty alternative returns `<authenticated>` without loading or refreshing credentials. These signals never appear in find_env_keys.

Native lookup uses the process environment and native filesystem. The empty-environment proc recovery branch is restricted to a Bun host and caches both successful and failed reads; ordinary native Rust execution is not that host. A temporarily unavailable native filesystem facility does not cache absence. Browser hosts have no native credential files; a missing process object produces ReferenceError with message process is not defined when the selected branch reads it. An ordinary unknown provider can still return None without reading process.

The helpers emit no console output, execute no secret command, perform no network request and impose no global authentication gate. They expose environment values only through the deliberate value lookup. Stored credentials, login, token refresh and provider invocation remain separate operations.

```rust
use maestro_models::{find_env_keys, get_env_api_key};

assert!(find_env_keys("controlled-unknown-provider").unwrap().is_none());
assert!(get_env_api_key("controlled-unknown-provider").unwrap().is_none());
```
