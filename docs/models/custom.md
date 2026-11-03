# Configure custom models

Edit `~/.maestro/agent/models.json` to connect a local server, add a model, or
route a built-in provider through a proxy. If you set `MAESTRO_CODING_AGENT_DIR`,
put `models.json` in that directory instead. See [configuration roots](../application/configuration.md#identity-and-roots).

## Connect a local server

Start your server separately and use its model ID and OpenAI-compatible base URL.
This complete file defines one text model:

```json
{
  "providers": {
    "workstation": {
      "baseUrl": "http://127.0.0.1:11434/v1",
      "api": "openai-completions",
      "apiKey": "local-only",
      "models": [{ "id": "qwen2.5-coder:7b" }]
    }
  }
}
```

A new provider needs `baseUrl`, `apiKey`, and an `api` on the provider or each
model. A server that ignores authentication still needs a nonempty placeholder
key in this file. Do not use a real key for a local server that does not need one.

Check discovery without sending a prompt:

```sh
maestro --list-models workstation
```

Send a prompt only after the server is ready:

```sh
maestro --provider workstation --model qwen2.5-coder:7b --thinking off --no-session -p 'Reply with READY.'
```

To declare the model's limits and request format explicitly, replace the file
with this complete configuration. Use limits your server actually supports;
these values describe a configured deployment, not hardware detection.

```json
{
  "providers": {
    "workstation": {
      "baseUrl": "http://127.0.0.1:11434/v1",
      "api": "openai-completions",
      "apiKey": "local-only",
      "compat": {
        "supportsDeveloperRole": false,
        "supportsReasoningEffort": false,
        "supportsUsageInStreaming": false,
        "maxTokensField": "max_tokens"
      },
      "models": [{
        "id": "qwen2.5-coder:7b",
        "name": "Workstation Coder",
        "reasoning": false,
        "input": ["text"],
        "contextWindow": 32768,
        "maxTokens": 4096,
        "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 }
      }]
    }
  }
}
```

For a reasoning-capable local model, `supportsDeveloperRole: false` keeps the
instruction role as `system`; `supportsReasoningEffort: false` suppresses the
ordinary OpenAI effort field. Set `reasoning: true` only when the model supports
reasoning, and choose a [thinking format](#reasoning-and-cache-compatibility) your
server accepts.

## Connect a content-generation endpoint

For Google's content-generation protocol, use `google-generative-ai`. This
minimal file uses an environment variable for the key:

```json
{
  "providers": {
    "content-lab": {
      "baseUrl": "https://generativelanguage.googleapis.com/v1beta",
      "api": "google-generative-ai",
      "apiKey": "CONTENT_LAB_KEY",
      "models": [{ "id": "gemini-2.5-flash" }]
    }
  }
}
```

This full variant declares image input, reasoning, limits and prices. Prices are
local estimates in USD per million tokens; update them for your deployment.

```json
{
  "providers": {
    "content-lab": {
      "baseUrl": "https://generativelanguage.googleapis.com/v1beta",
      "api": "google-generative-ai",
      "apiKey": "CONTENT_LAB_KEY",
      "headers": { "x-client-label": "workbench" },
      "models": [{
        "id": "gemini-2.5-flash",
        "name": "Content Lab Flash",
        "api": "google-generative-ai",
        "baseUrl": "https://generativelanguage.googleapis.com/v1beta",
        "reasoning": true,
        "input": ["text", "image"],
        "contextWindow": 1048576,
        "maxTokens": 65536,
        "cost": { "input": 0.3, "output": 2.5, "cacheRead": 0.03, "cacheWrite": 0 },
        "headers": { "x-model-label": "flash" }
      }]
    }
  }
}
```

Set `CONTENT_LAB_KEY` in the launching process without storing the secret in the
file. `maestro --list-models content-lab` checks discovery without a paid call.
Requests and thinking budgets are described in [content generation](content-generation.md).

## Reload and diagnose

Save the file, then open `/model` in an interactive session with no scoped model
list. The selector refreshes the catalog. With a scoped list, candidates come
from that list instead; use `/reload` to reload the session resources and catalog.
A new CLI process also reads the file. There is no automatic file watcher.

A malformed file produces a `models.json` diagnostic with its path. Maestro
falls back to built-in descriptors instead of retaining the previous custom
entries. Correct the file and reload. Line comments (`//`) and trailing commas
are accepted; block comments and a leading byte-order mark are not.

`--list-models` and the selector list model IDs. The configured `name` also
participates in [model matching](../selection.md) and appears in model details.
Discovery does not prove the endpoint accepts the request or that a key is valid.

## Choose a protocol

These APIs cover the custom HTTP endpoints above:

| `api` | Endpoint protocol | Request details |
| --- | --- | --- |
| `openai-completions` | Chat Completions | [Chat completions](chat-completions.md) |
| `openai-responses` | Responses | [Responses](responses.md) |
| `anthropic-messages` | Messages | [Messages](messages.md) |
| `google-generative-ai` | Content generation | [Content generation](content-generation.md) |

`api` is an open protocol identifier, not a schema enum. Other built-in adapters
and extension-registered APIs have their own requirements; see the
[protocol inventory](api.md). Accepting a string in the file does not install an
adapter. Supply a base URL for a new provider, including content-generation
providers. Use a base URL, not the final `/chat/completions` request path.

## Provider and model fields

The root object contains `providers`, an object keyed by provider ID. Provider
IDs and model IDs together identify an entry.

| Provider field | When omitted | Meaning |
| --- | --- | --- |
| `baseUrl` | First built-in model's URL for custom entries on an existing provider | Endpoint default; also overrides built-in model URLs. Required for a new provider with models. |
| `api` | First built-in model's API for custom entries | Default API for custom entries; does not switch the API of existing built-in entries. |
| `apiKey` | Credential owner's resolution | Request key source. Required for a new provider with models. |
| `headers` | No added headers | Map of header names to configured value strings. |
| `authHeader` | `false` | Add `Authorization: Bearer <resolved key>` after resolving headers; requires a nonempty selected key. |
| `compat` | Existing model compatibility or protocol detection | Compatibility defaults, overlaid by model-specific values. |
| `models` | No custom entries | Array of definitions added or replaced by ID. |
| `modelOverrides` | No built-in metadata changes | Object keyed by built-in model ID; see [overrides](#change-built-in-models). |

A provider with no models must supply at least one of `baseUrl`, `headers`,
`compat` or `modelOverrides`; an `apiKey` alone is not a valid provider entry.

| Custom model field | When omitted | Meaning |
| --- | --- | --- |
| `id` | Required | Nonempty identifier sent to the endpoint. |
| `name` | `id` | Nonempty label for matching and display. |
| `api` | Provider, then first built-in API | Nonempty protocol identifier for this model. |
| `baseUrl` | Provider, then first built-in URL | Nonempty endpoint for this model. |
| `reasoning` | `false` | Whether the model supports reasoning. |
| `thinkingLevelMap` | Protocol's level mapping | Supported levels and provider names; see below. |
| `input` | `["text"]` | Array containing `text` and/or `image`. Declare images only if supported. |
| `contextWindow` | `128000` | Positive context limit in tokens. |
| `maxTokens` | `16384` | Positive maximum output tokens, not a promise of that much output. |
| `cost` | Four zero prices | `input`, `output`, `cacheRead`, `cacheWrite`, in USD per million tokens. Supply all four when defining this object on a custom model. |
| `headers` | No model-specific headers | Configured header values overlaid on provider headers. |
| `compat` | Provider compatibility | Overlay on provider compatibility, not a whole-record replacement. |

## Keys and headers

Each configured key or header value is a string:

| Form | Example | Resolution |
| --- | --- | --- |
| Environment name | `"CONTENT_LAB_KEY"` | Use that environment value if it is nonempty. |
| Literal | `"local-only"` | If that variable is absent or empty, use the string itself. |
| Shell command | `"!credential-helper print-key"` | Run the command and use trimmed stdout. |

An absent or empty environment variable falls back to its name as a literal key. Check
your launching environment. Do not put real secrets in shared files. Commands
are executable configuration: read an imported file before using it.

Commands resolve at request time, not during discovery. Maestro runs a configured
command on each resolution without a TTL, stale-value fallback or recovery cache.
Put any required caching in your helper. See [configured values](../credentials.md#configured-values)
for command failure and stored-credential precedence.

Provider headers resolve first, then model headers replace matching values.
`authHeader: true` adds Authorization last at the catalog boundary. The protocol
adapter also has its own authentication headers; this flag does not disable them.
For a proxy, keep the upstream key in `apiKey` and add proxy authentication in
`headers`, for example `"x-proxy-key": "WORKBENCH_PROXY_KEY"`.

## Declare thinking levels

A model's `thinkingLevelMap` accepts `off`, `minimal`, `low`, `medium`, `high` and
`xhigh`. An absent ordinary key retains the protocol's default mapping; `xhigh`
requires an explicit string mapping. A string supplies a provider value; `null`
marks a level unsupported so selection skips or clamps it. `off: null` describes
a model whose reasoning cannot be disabled.

For a reasoning model that accepts only off, high and a provider-specific maximum:

```json
{
  "id": "reasoner-local",
  "reasoning": true,
  "thinkingLevelMap": {
    "minimal": null,
    "low": null,
    "medium": null,
    "high": "high",
    "xhigh": "max"
  }
}
```

This is a model entry to insert in `models`, not a standalone file. Mapping a
name does not make a protocol support a new control; content-generation uses its
own thinking budgets. See [thinking options](../model-options.md).

## Change built-in models

To route existing Messages models through a proxy, keep their descriptors:

```json
{
  "providers": {
    "anthropic": { "baseUrl": "https://messages-proxy.example" }
  }
}
```

Stored credentials and environment-based authentication still apply. Use
`headers` and `authHeader` if the proxy needs additional authentication.

`modelOverrides` changes selected built-in metadata without reconstructing a
whole descriptor:

```json
{
  "providers": {
    "openrouter": {
      "modelOverrides": {
        "anthropic/claude-sonnet-4": {
          "name": "Sonnet on Bedrock",
          "cost": { "input": 3 },
          "compat": { "openRouterRouting": { "only": ["amazon-bedrock"] } }
        }
      }
    }
  }
}
```

Allowed override fields are `name`, `reasoning`, `thinkingLevelMap`, `input`,
`cost`, `contextWindow`, `maxTokens`, `headers` and `compat`. Partial `cost` and
thinking maps retain unsupplied entries. Unknown IDs are ignored. Use provider
`baseUrl` for built-in endpoint changes; `api`, `id` and `baseUrl` are not
per-model override fields.

Built-ins remain unless a custom `models` entry has the same provider and ID.
Custom entries are applied after built-in overrides and replace that descriptor
with custom defaults, rather than inheriting its metadata. New IDs append;
repeated IDs replace the first slot. Compatibility overlays retain unsupplied
fields. `openRouterRouting` and `vercelGatewayRouting` merge one level: supplied
lists and nested objects replace their previous values. See
[catalog composition](../catalog.md#loading-and-composition) for exact semantics.

## Messages compatibility

For an `anthropic-messages` proxy that rejects per-tool eager streaming, put this
`compat` object on the provider or model:

```json
{
  "supportsEagerToolInputStreaming": false,
  "supportsLongCacheRetention": false,
  "supportsCacheControlOnTools": false
}
```

These three fields default to `true`. With eager streaming disabled, Maestro omits
`tools[].eager_input_streaming` and uses the
`fine-grained-tool-streaming-2025-05-14` beta header when tools are present.
With long retention disabled it omits the `cache_control.ttl: "1h"` extension,
including when the caller requests long retention. This does not disable short
cache markers. With cache markers on tools disabled, the last tool carries no
marker, while the system prompt and last message keep theirs.

Two more fields default to `false`. `sendSessionAffinityHeaders` sends the
session ID as `x-session-affinity` when caching is on and the request uses an API
key; the header is skipped for an empty session ID, an account token and a
subscription token. `allowEmptySignature` replays unsigned thinking with
`signature: ""` instead of converting it to text. See [Messages requests](messages.md).

## Chat Completions compatibility

Put `compat` on the provider for defaults, or on a model for exceptions. For an
unrecognized local endpoint the defaults below apply; built-in descriptors and
recognized provider IDs/URLs can select different values.

| Field | Generic default | Effect |
| --- | --- | --- |
| `supportsStore` | `true` | Accept the request's `store` field. |
| `supportsDeveloperRole` | `true` | Use `developer` for instructions on reasoning models; otherwise use `system`. |
| `supportsReasoningEffort` | `true` | Accept ordinary `reasoning_effort`. |
| `supportsUsageInStreaming` | `true` | Include `stream_options.include_usage`. |
| `maxTokensField` | `"max_completion_tokens"` | Alternative: `"max_tokens"`. |
| `requiresToolResultName` | `false` | Add the tool name to result messages. |
| `requiresAssistantAfterToolResult` | `false` | Add an assistant bridge before a user message following tool results. |
| `requiresThinkingAsText` | `false` | Replay thinking as assistant text. |
| `requiresReasoningContentOnAssistantMessages` | `false` | Include empty `reasoning_content` on replayed assistant messages when reasoning is enabled. |
| `thinkingFormat` | `"openai"` | Select the reasoning payload convention below. |
| `cacheControlFormat` | Absent | `"anthropic"` adds prompt-cache markers. |
| `supportsStrictMode` | `true` | Include `strict` in tool definitions. |
| `supportsLongCacheRetention` | `true` | Allow extended cache lifetimes. |
| `zaiToolStream` | `false` | Enable `tool_stream` for tool-enabled requests. |
| `sendSessionAffinityHeaders` | `false` | Send session affinity headers when a session ID is available and caching is enabled. |
| `openRouterRouting` | Absent | Forward routing preferences as `provider` for OpenRouter URLs. |
| `vercelGatewayRouting` | Absent | Forward `only`/`order` as `providerOptions.gateway`, whatever the base URL. |

Detection disables store/developer support for nonstandard endpoints such as
Z.ai, Moonshot, Cloudflare, xAI, Cerebras, Chutes, DeepSeek and OpenCode. Reasoning
effort defaults off for xAI, Z.ai, Moonshot and Cloudflare Gateway. Strict mode
defaults off for Moonshot and Cloudflare Gateway; long caching defaults off for
Cloudflare Workers and Gateway. Moonshot, Chutes and Cloudflare Gateway use
`max_tokens`. DeepSeek defaults to reasoning-content replay and `deepseek`
thinking; Z.ai selects `zai`; OpenRouter selects `openrouter`. On OpenRouter,
only model IDs beginning `anthropic/` or `openai/` get the developer role. The
OpenCode Go gateway streams reasoning as `reasoning`; Maestro replays it as
`reasoning_content`. Override fields explicitly when a proxy's URL no
longer identifies the upstream behavior.

### Reasoning and cache compatibility

| `thinkingFormat` | Enabled reasoning | Disabled reasoning |
| --- | --- | --- |
| `openai` | `reasoning_effort` when supported | Mapped `off` value, if supplied and supported |
| `openrouter` | `reasoning.effort` | Mapped `off`, otherwise `none` |
| `deepseek` | `thinking.type: "enabled"` and `reasoning_effort` | `thinking.type: "disabled"` |
| `zai` | `thinking: {"type": "enabled"}` | `thinking: {"type": "disabled"}` |
| `qwen` | Top-level `enable_thinking: true` | `enable_thinking: false` |
| `qwen-chat-template` | `chat_template_kwargs` with `enable_thinking: true`, `preserve_thinking: true` | `chat_template_kwargs.enable_thinking: false` |

Named effort formats use `thinkingLevelMap`. Models with `reasoning: false` do
not send these reasoning controls. For local Qwen endpoints, choose `qwen` for
top-level controls or `qwen-chat-template` for chat-template controls.

`cacheControlFormat: "anthropic"` marks the system prompt, last tool definition
and last user/assistant text. It defaults on for provider `openrouter` with model
IDs beginning `anthropic/` or `~anthropic/`. Long retention adds `ttl: "1h"` when supported;
OpenAI prompt caching instead uses `prompt_cache_retention: "24h"`. Retention
`none` suppresses cache and affinity fields. File compatibility describes what
an endpoint accepts; request [cache retention](../model-options.md#cache-retention)
selects what to request.

### Routing

For OpenRouter, `openRouterRouting` carries provider preferences. Common fields
are `only`, `order`, `ignore`, `quantizations` (string arrays), `allow_fallbacks`,
`require_parameters`, `zdr`, `enforce_distillable_text` (booleans), and
`data_collection` (`"allow"` or `"deny"`). `sort` accepts a string or
`{"by":"price","partition":"model"}`. `max_price` accepts numeric or string
prices for `prompt`, `completion`, `image`, `audio`, `request`.
`preferred_min_throughput` and `preferred_max_latency` accept numbers or
percentile objects with `p50`, `p75`, `p90`, `p99`. The object is forwarded as
written when the base URL contains `openrouter.ai`; Maestro does not choose an
upstream itself. Check [OpenRouter's routing contract](https://openrouter.ai/docs/guides/routing/provider-selection)
for service-side meanings and supported values.

For a Vercel gateway, use:

```json
{
  "vercelGatewayRouting": {
    "only": ["fireworks", "novita"],
    "order": ["novita", "fireworks"]
  }
}
```

This is a `compat` fragment. Maestro sends it for any base URL, so a proxy in
front of the gateway keeps it. The gateway, not Maestro, selects the provider.

## Responses compatibility

For `openai-responses`, `supportsDeveloperRole`, `sendSessionIdHeader` and
`supportsLongCacheRetention` default to `true`. The first sends instructions as
`developer` on reasoning models, or as `system` when `false`; the second controls
the session ID header; the third allows `prompt_cache_retention: "24h"` when long
retention is requested. Use `false` for any capability a proxy rejects. Chat Completions reasoning-format and
routing switches do not configure the Responses adapter. See [Responses](responses.md).
