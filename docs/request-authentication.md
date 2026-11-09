# Request authentication

For the provider/key table and terminal setup, see [provider setup](providers.md).

Model invocation forwards the authentication options you supply, unchanged. It
never discovers environment credentials on its own, so call the helpers yourself
before you pass an adapter its API key. A configured credential is not a
completed login and does not predict that a request will succeed. Neither helper
makes a network request, reads credential contents or validates a login.

Stored request-key selection and the configured-auth status reread the credential
file. A key another process stores or removes therefore reaches the next request
or status read. See [stored credentials](credentials.md#stored-credentials) for
the acquisition waits and for the accepted-record fallback when a reread fails.

## Environment credentials

`find_env_keys(provider)` returns the names of all populated, nonempty API-key
variables in precedence order, or `None`. `get_env_api_key(provider)` returns the
first value, the ambient `<authenticated>` marker, or `None`. API-key values are
neither trimmed nor cached: an empty value counts as absent, while `"0"`, spaces,
tabs and Unicode whitespace stay exactly as set. Provider names match exactly and
are case-sensitive. `find_env_keys` never lists ambient configuration.

Anthropic checks `ANTHROPIC_OAUTH_TOKEN` before `ANTHROPIC_API_KEY`. An API-key
request sends `x-api-key` alone; `ANTHROPIC_AUTH_TOKEN` is never read, on the
default endpoint or a compatible one. GitHub Copilot checks only
`COPILOT_GITHUB_TOKEN`. The generic `GH_TOKEN` and `GITHUB_TOKEN` belong to other
tools and never count as Copilot credentials. Fireworks checks
`FIREWORKS_API_KEY`. OAuth token variables can supply keys.

## Ambient configuration

Google Vertex checks `GOOGLE_CLOUD_API_KEY` first. Without it, Vertex needs two
things. The first is an existing credentials path: `GOOGLE_APPLICATION_CREDENTIALS`,
or the home directory's `.config/gcloud/application_default_credentials.json`
when that variable is absent or empty. The second is a nonempty
`GOOGLE_CLOUD_PROJECT` or `GCLOUD_PROJECT`, plus a nonempty
`GOOGLE_CLOUD_LOCATION`. An explicit path that does not exist gets no fallback. The
check tests existence only, directories and native symlink semantics included, and
caches the result, true or false, for the life of the process. A relative path
resolves against the current working directory. Credential paths keep native
operating-system characters, including non-UTF-8 paths on Unix.

Amazon Bedrock recognizes any one of these: a nonempty `AWS_PROFILE`, the pair
`AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`, `AWS_BEARER_TOKEN_BEDROCK`,
`AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`, `AWS_CONTAINER_CREDENTIALS_FULL_URI` or
`AWS_WEB_IDENTITY_TOKEN_FILE`. Each returns `<authenticated>` without checking
files or credential contents. An access key, secret key or session token alone is
not enough, and the paired keys do not need a session token.

## Native and browser behavior

An unknown provider returns `None`, and so do `constructor` and other
object-property names. Browser builds have no process environment and return
`None`; environment shims that a page defines are not read. The browser smoke
check compiles the real browser entry and its assertions but does not run them in
a browser.

The code calls the standard Rust environment, home-directory and filesystem
facilities directly. A non-Unicode API-key value counts as absent, the home lookup
falls back to the native account entry, and path joining does not normalize
components lexically. No runtime-specific environment recovery or import-readiness
race exists.

## Controlled example

With only `FIREWORKS_API_KEY=test-fireworks-key` in a controlled process:

```rust,ignore
use maestro_models::{find_env_keys, get_env_api_key};

assert_eq!(find_env_keys("fireworks"), Some(vec!["FIREWORKS_API_KEY".into()]));
assert_eq!(get_env_api_key("fireworks"), Some("test-fireworks-key".into()));
```

Run the integration case. It builds that environment in a fresh child process,
copies no user credentials and leaves the parent environment unchanged:

```sh
cargo test --locked -p maestro-models --test integration environment_model_keys::maestro_fireworks_catalog_and_keys_remain_complete -- --exact
```
