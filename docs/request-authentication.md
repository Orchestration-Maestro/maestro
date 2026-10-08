# Request authentication

Model invocation forwards supplied authentication options unchanged; it does not
implicitly discover environment credentials. Call the helpers explicitly before
supplying an adapter's API key. Configured credentials are not a successful login
or a prediction of request success. Neither helper makes network requests, reads
credential contents or validates a login.

## Environment credentials

`find_env_keys(provider)` returns all populated, nonempty API-key variable names
in precedence order, or `None`. `get_env_api_key(provider)` returns the first
value, an ambient `<authenticated>` marker, or `None`. API-key values are not
trimmed or cached: empty values are absent, but `"0"`, spaces, tabs and Unicode
whitespace are retained unchanged. Provider names match exactly and are
case-sensitive. Ambient configuration never appears in `find_env_keys`.

Anthropic checks `ANTHROPIC_OAUTH_TOKEN` before `ANTHROPIC_API_KEY`. GitHub Copilot
checks `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, then `GITHUB_TOKEN`. Fireworks checks
`FIREWORKS_API_KEY`. OAuth token variables can supply keys.

## Ambient configuration

Google Vertex first checks `GOOGLE_CLOUD_API_KEY`. Otherwise it requires an
existing `GOOGLE_APPLICATION_CREDENTIALS` path, or the home directory's
`.config/gcloud/application_default_credentials.json` when that variable is
absent or empty, plus nonempty `GOOGLE_CLOUD_PROJECT` or `GCLOUD_PROJECT` and
`GOOGLE_CLOUD_LOCATION`. An explicit missing path does not fall back. Only
existence is checked, including directories and native symlink semantics; that
result, true or false, is cached for the process lifetime. Relative paths use the
current working directory. Credential paths retain native operating-system
characters, including non-UTF-8 paths on Unix.

Amazon Bedrock recognizes nonempty `AWS_PROFILE`, paired `AWS_ACCESS_KEY_ID` and
`AWS_SECRET_ACCESS_KEY`, `AWS_BEARER_TOKEN_BEDROCK`,
`AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`, `AWS_CONTAINER_CREDENTIALS_FULL_URI`,
or `AWS_WEB_IDENTITY_TOKEN_FILE`. Any alternative returns `<authenticated>`
without checking files or credential contents. An access key, secret key or
session token alone is insufficient; a session token is not required with the
paired keys.

## Native and browser behavior

Unknown providers, including `constructor` and other object-property names,
return `None`. Browser builds have no process environment and return `None`;
page-defined environment shims are not read. The browser smoke check compiles the
real browser entry and its assertions; it does not execute them in a browser.

Standard Rust environment, home-directory and filesystem facilities are used
directly. Non-Unicode API-key values are absent, home lookup uses native account
fallback, and path joining does not lexically normalize components. There is no
runtime-specific environment recovery or import-readiness race.

## Controlled example

With only `FIREWORKS_API_KEY=test-fireworks-key` in a controlled process:

```rust,ignore
use maestro_models::{find_env_keys, get_env_api_key};

assert_eq!(find_env_keys("fireworks"), Some(vec!["FIREWORKS_API_KEY".into()]));
assert_eq!(get_env_api_key("fireworks"), Some("test-fireworks-key".into()));
```

Run the integration case, which creates that environment in a fresh child
without copying user credentials or changing the parent environment:

```sh
cargo test --locked -p maestro-models --test environment_model_keys maestro_fireworks_catalog_and_keys_remain_complete -- --exact
```
