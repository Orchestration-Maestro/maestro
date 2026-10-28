# Add a model provider

Use a provider component when a session needs a different endpoint, account flow,
or streaming protocol. If only model metadata or an endpoint changes, consider a
[local model file](../catalog.md#loading-and-composition) instead. Both routes
use the same catalog and request-authentication owner.

## Contents

1. [Register a controlled endpoint](#register-a-controlled-endpoint)
2. [Change or remove a registration](#change-or-remove-a-registration)
3. [Choose metadata and authentication](#choose-metadata-and-authentication)
4. [Supply an account flow](#supply-an-account-flow)
5. [Produce an assistant stream](#produce-an-assistant-stream)
6. [Check the provider without live credentials](#check-the-provider-without-live-credentials)

## Register a controlled endpoint

Start with the [extension project instructions](../extensions.md#create-the-project).
Keep that project's manifest and toolchain, but replace `src/lib.rs` with this
complete factory. The endpoint is deliberately loopback-only; start a controlled
Chat Completions server on port 18765 before sending a prompt. Nothing here
implements a server or contacts a public provider.

```rust
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture, ProviderConfig};
use serde_json::json;

maestro_extensions_wasm::export_extension!(factory);

fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let config: ProviderConfig = serde_json::from_value(json!({
            "name": "Workshop endpoint",
            "baseUrl": "http://127.0.0.1:18765/v1",
            "apiKey": "workshop-test-key",
            "api": "openai-completions",
            "authHeader": true,
            "headers": {"X-Workshop": "controlled"},
            "models": [{
                "id": "workshop-text",
                "name": "Workshop text",
                "reasoning": false,
                "input": ["text"],
                "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
                "contextWindow": 8192,
                "maxTokens": 256
            }]
        })).map_err(|error| error.to_string())?;
        api.register_provider("workshop", config)
    })
}
```

Build and package it using [the component build instructions](../extensions.md#build-the-component).
If you kept the project name `hello-extension`, the output remains
`target/wasm32-wasip2/release/hello_extension.wasm`; the provider name does not
rename the artifact. From that extension directory, with `maestro` on `PATH`,
run in a disposable home and project:

```sh
scratch=$(mktemp -d)
component="$PWD/target/wasm32-wasip2/release/hello_extension.wasm"
mkdir -p "$scratch/home" "$scratch/agent" "$scratch/project"
cd "$scratch/project"
HOME="$scratch/home" MAESTRO_CODING_AGENT_DIR="$scratch/agent" \
  maestro --no-extensions -e "$component" --list-models workshop
HOME="$scratch/home" MAESTRO_CODING_AGENT_DIR="$scratch/agent" \
  maestro --no-extensions -e "$component" --provider workshop \
  --model workshop-text --thinking off --no-session --no-tools -p 'Reply with WORKSHOP-OK'
```

`--no-extensions` disables discovery, not the explicit `-e` component.
The listing should show provider `workshop` and model `workshop-text`; the print
invocation should return the text supplied by your controlled server. Keep the temporary directory
until you have inspected the results. These assignments redirect home-based
configuration, credentials and project discovery, but do not remove inherited
provider environment variables. Run from a shell without real provider secrets.
Do not use this fixture's literal test key against a real service.

Register in the factory, not a `session_start` handler, when model discovery must
finish before startup selection and `--list-models`. The factory returns a future:
await any discovery there, validate the response, then register. The host waits
for factory completion. Facade registrations queue until runtime binding; after
binding they apply before the call returns. Queued validation failures are
reported as extension errors, so inspect startup diagnostics as well as exit status.

For working code behind this route, see the
[provider catalog component](../../crates/maestro-extensions-wasm/examples/context_bridge/providers.rs)
and its [controlled host checks](../../crates/maestro-extensions-wasmtime/tests/provider_catalog.rs).
Standalone provider example deliveries are tracked separately in
[#2223](https://github.com/Orchestration-Maestro/maestro/issues/2223) and
[#2224](https://github.com/Orchestration-Maestro/maestro/issues/2224); those are not
installed example packages today.

## Change or remove a registration

For an existing provider, deserialize a `ProviderConfig` containing only
`baseUrl` and/or `headers`, then call `api.register_provider` with that provider's
ID. Existing models remain; matching descriptors receive the new endpoint or
request headers. Do not supply a model list merely to change routing.

A **nonempty** `models` list replaces that provider's models. Missing and empty
lists do not immediately clear them. Dynamic models require complete metadata;
they do not inherit the local-file loader's defaults. A nonempty list requires
a provider endpoint, an API on each model or the provider, and a configured key
or OAuth implementation. A stream callback also requires a nonempty provider API.

Repeat registration applies the incoming change immediately, then merges its
present fields into the retained configuration for refresh. Absent fields keep
previous values; empty strings, empty collections and `false` are present values.
Request-authentication sources have their own replacement rule: a nonempty key,
present headers or a true auth-header flag replaces that source as a whole.
See [dynamic catalog rules](../catalog.md#dynamic-providers) before splitting one
configuration over several calls.

Call `api.unregister_provider("workshop")?` to remove the registration. Once
bound, no reload is needed. Removal drops its dynamic models, configured-key
fallback, OAuth membership and custom stream registrations, restores the
underlying built-in/file catalog, and replays surviving dynamic providers.
Unknown names are a no-op. Already retained descriptors, account handles and
started calls remain valid independently of registry membership.

For direct catalog work, `context.model_registry()?` exposes
`register_provider`, `unregister_provider` and `refresh`. These mutate that
catalog, not the facade's pre-bind queue. Refresh reloads the model file and
replays retained providers in registration order. Invalid incoming metadata
changes nothing; a later OAuth transformation or replay failure can leave
already-applied effects visible. Do not assume registration is a transaction.

## Choose metadata and authentication

Rust uses snake_case fields; the recipe uses the shared JSON form's camelCase.
Callbacks are Rust values in `config.callbacks`, never serialized JSON.

| Provider field | What to supply |
| --- | --- |
| `name` | Optional display label; otherwise account metadata, a built-in display label, or the provider ID supplies it, in that order. An empty label is retained. |
| `baseUrl`, `api` | Endpoint and protocol fallback for models. A URL alone does not choose a wire format. |
| `apiKey` | Literal, environment-variable name, or `!`-prefixed secret command. Prefer an environment name or helper outside fixtures. |
| `headers` | Ordered request-header expressions, using the same configured-value forms as keys. |
| `authHeader` | When true, synthesize `Authorization: Bearer <resolved key>`; requires a nonempty resolved key. It does not implement a custom protocol. |
| `models` | Complete model definitions; nonempty lists replace the provider's models. |
| `callbacks.oauth`, `callbacks.stream_simple` | Local callable account and streaming implementations. Both are optional. |

The [bundled API table](../models/api.md#bundled-protocols-and-direct-operations)
lists the nine protocol identifiers and their owners. Use `openai-completions`
for a Chat Completions peer, not for every service with an HTTP endpoint.
If your wire format differs, register a custom stream callback under your own API.

Each [model definition](../../crates/maestro-request/src/provider_config.rs)
requires `id`, `name`, `reasoning`, `input`, `cost`, `contextWindow` and `maxTokens`.
Use `text` and, only if supported, `image` in `input`. Prices are USD per million
tokens in four non-overlapping categories: `input`, `output`, `cacheRead` and
`cacheWrite`. Optional model `api` and `baseUrl` override provider defaults;
model `headers` are request sources, not descriptor metadata.

For reasoning models, `thinkingLevelMap` maps `off`, `minimal`, `low`, `medium`,
`high` and `xhigh` to protocol-specific strings; null marks an unsupported level.
`compat` carries protocol-specific options. For Chat Completions, check
[compatibility controls](../models/chat-completions.md) for developer/system role,
reasoning effort, token-limit field, tool-result requirements, thinking format
and cache-control behavior. Do not copy one server's quirks onto another.

At request time the catalog selects stored/runtime credentials before its
configured provider-key fallback. A present empty credential key suppresses that
fallback. It resolves provider headers, then model headers, merging them over
literal descriptor headers; an enabled auth header is added last. Environment
values and configured commands are read at request time, not registration time.
Availability metadata does not execute a helper, refresh a token, or prove that
the server will accept the credential. See
[request authentication](../catalog.md#request-authentication) and
[stored credentials](../credentials.md#stored-credentials).

## Supply an account flow

Implement the guest's
[`OAuthProviderInterface`](../../crates/maestro-extensions-wasm/src/oauth.rs)
and set `config.callbacks.oauth = Some(Rc::new(your_provider))` before registration.
The installed account ID is the registration name, not the implementation's
`id()` return value. `name()` supplies the account label;
`uses_callback_server()` reports optional metadata, not a running listener.

The complete [authored account implementation](../../crates/maestro-extensions-wasm/examples/context_bridge/oauth/registration.rs)
shows all methods, registration through both the facade and an independent catalog,
and controlled login and refresh. Its token strings and endpoints are test data,
not an authorization service. Replace those operations, not just their labels,
when connecting a real identity system.

1. In `login`, call `on_auth` with the authorization URL. Await `on_prompt` for
   pasted input if your flow needs it. Both return fallible results; propagate
   callback failures rather than continuing with an empty code.
2. Use optional `on_progress`, `on_manual_code_input`, `on_select` and `signal`
   only when supplied. A device flow can publish its URL and user code through
   authorization/progress text; there is no separate `on_device_code` callback.
   Your provider owns token exchange, state/PKCE checks, callback-listener
   cleanup and cancellation policy. See [native operations](guest_operations.md).
3. Return `OAuthCredentials` with `refresh`, `access`, `expires` (Unix epoch
   milliseconds), and any provider extension fields. Use finite expiry numbers
   in component data. Never log token values or put them in a package.
4. Implement `refresh_token` to return updated credentials, retaining the previous
   refresh token when the service does not rotate it. `get_api_key` converts
   credentials to the request key; it may compute a key rather than return `access`.
5. Return `None` from `modify_models` if unchanged, or a replacement model list
   for account-specific endpoints/capabilities. Preserve unrelated providers when
   transforming the supplied list. Loading invokes a supplied modifier when stored
   OAuth credentials exist. Immediate registration also requires a nonempty incoming
   model list; OAuth-only and endpoint-only registrations do not invoke it.
   A transform error can leave prior effects.

After registration, enter `/login`, choose **Use a subscription**, then choose
the provider. `/login` takes no provider argument. The application's credential owner saves
accepted credentials in `auth.json` under the selected agent directory; the
isolated recipe uses `$scratch/agent/auth.json`. Normally that directory is
`~/.maestro/agent`, overridden by `MAESTRO_CODING_AGENT_DIR`.
The credential owner handles locked refresh and persistence on key resolution;
calling the provider's `refresh_token` directly neither tests expiry nor saves
its result. See [OAuth primitives](../models/oauth.md#provider-registry) and
[credential storage](../credentials.md) for the distinction.

## Produce an assistant stream

Set `config.callbacks.stream_simple` to a guest `StreamFunction<StreamOptionsInput>`.
Despite the field name, this callback receives either the caller's raw or simple
option variant unchanged. Keep the supplied model, context, hooks and cancellation
signal for that invocation; do not replace options with defaults when forwarding
to a bundled protocol.

Use the [model callback example](../../crates/maestro-extensions-wasm/examples/context_bridge/model_registered.rs)
for registration and dispatch, the
[assistant event example](../../crates/maestro-extensions-wasm/examples/context_bridge/assistant_streams.rs)
for every event variant, and the
[stream lifetime example](../../crates/maestro-extensions-wasm/examples/context_bridge/stream_lifetimes.rs)
for independently owned producers. These are executable component fixtures, not
standalone provider packages.

Write the producer in this order:

1. Create an `AssistantMessageEventStream`. Initialize one shared assistant
   message with the selected model's API/provider/ID, empty content, timestamp,
   zero usage/cost and an initial stop reason. Publish `Start` with that handle.
2. For each block, append content before publishing its start event. Update the
   message before publishing each delta; use the block's stable `content_index`.
   Text uses `TextStart`, `TextDelta`, `TextEnd`; reasoning uses the corresponding
   `Thinking*` events. Preserve signatures and redaction data when supplied.
3. For tools, retain the call ID/name and accumulate argument JSON across chunks.
   Publish `ToolcallStart`, `ToolcallDelta` and `ToolcallEnd` with the final
   `ToolCall`. An incomplete JSON fragment is not final arguments; malformed
   completed JSON must fail clearly. Model argument values can have any JSON root
   kind; schema validation and tool execution belong to the normal tool caller.
4. Populate usage and costs, then publish exactly one terminal event:
   `Done` with `Stop`, `Length` or `ToolUse`, or `Error` with `Error` or `Aborted`.
   Set the message's stop reason and retain the failure detail on error.
   Close the stream; terminal admission settles the final result independently
   of event iteration. Ending without a terminal result leaves `result()` pending.
5. On cancellation, stop your transport/work and publish the aborted final message.
   Dropping a reader or removing registration does not cancel an independent
   producer. Handle transport, hook and decoding failures so observers receive
   a terminal outcome instead of waiting forever.

Do not reconstruct the final message from emitted deltas: events point to the
message you maintain. Authored partial handles remain shared; observations
crossing the component boundary are decoded snapshots. See
[stream ownership and forwarding](guest_streams.md#providers-called-by-native-code).

Count usage categories without overlap. For example, 10 uncached input tokens,
3 output tokens, 2 cache-read tokens and 1 cache-write token total 16 tokens;
with rates 1, 2, 0.5 and 1.5 USD/million, costs are 0.000010, 0.000006,
0.000001 and 0.0000015 USD, totaling 0.0000185 USD.
Native authors can use `maestro_models::calculate_cost`; a guest-authored stream
must supply its usage/cost fields rather than expect the event buffer to price it.
See [accounting](../models/accounting.md) for reported totals and pricing helpers.

## Check the provider without live credentials

Use a controlled peer or injected transport first. Check the actual endpoint,
headers and payload at that boundary, not just the registration's return value.
A successful list operation proves catalog publication, not a successful request.

From a Maestro checkout, run the existing offline component checks:

```sh
~/.local/bin/capped cargo nextest run --locked -p maestro-extensions-wasmtime -E 'test(provider_catalog::)'
~/.local/bin/capped cargo nextest run --locked -p maestro-extensions-wasmtime -E 'test(oauth::)'
~/.local/bin/capped cargo nextest run --locked -p maestro-extensions-wasmtime -E 'test(model_streams::)'
~/.local/bin/capped cargo nextest run --locked -p maestro-extensions-wasmtime -E 'test(authored_streams::)'
```

Nextest runs the repository's component setup script before these tests; direct
`cargo test` does not supply those fixture paths. The checks compile the real
guest and exercise the native host. They do not
validate your server implementation. Before distributing a provider, adapt the
[provider conformance checks](../models/conformance.md) to each model family:

| Boundary | Include |
| --- | --- |
| Catalog/account | Factory registration, post-bind replacement/removal/restoration, header/key precedence, login failure, expiry refresh and account model transforms |
| Stream | Ordered start/deltas/final result/EOF, empty response, early cancellation, transport and hook errors, independent result observers |
| Content/tools | Unicode splits, malformed argument JSON, missing tool result, image input/result, cross-provider history handoff |
| Limits/accounting | Context overflow, image limits, output limit, uncached/cache categories, reported total tokens and calculated costs |

Use disposable auth/model/settings files, fixed responses and test keys. Never
turn a passing offline check into an unannounced live API call. Validate packaging
and the explicit component load separately, following the registration recipe
above. Keep commands and observed outcomes with your provider's review evidence.
