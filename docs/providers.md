# Set up a model provider

Run `maestro --list-models` to see models available with your current credentials.
This reads the shipped catalog and your configuration; it does not send a prompt
or validate a key. For a source checkout, follow [development setup](development.md#setup-and-build)
first. Commands below assume the resulting `maestro` executable is on `PATH`.
Shell examples use Bash syntax. Replace example values before making a request.

Choose one route:

- [Save a key or sign in](#save-a-key-or-sign-in) in the terminal UI.
- [Set an environment key](#environment-keys) for a process or shell.
- [Edit the credential file](#credential-file-and-key-selection) for named variables or secret helpers.
- [Configure a cloud service](#cloud-services) for Azure, AWS, Cloudflare or Vertex.

## Save a key or sign in

These terminal steps use the native Unix frontend. Native Windows interactive
launch is not wired yet; [Windows runtime completion](https://github.com/Orchestration-Maestro/maestro/issues/486)
owns that limitation. A working terminal adapter alone does not enable login UI.

1. Start `maestro` in a terminal and enter `/login`.
2. Choose `Use an API key` or `Use a subscription`, then the provider.
3. For a key, paste the key. For a subscription, follow the authorization link
   and the dialog's instructions. A callback-based login also accepts the pasted
   redirect URL. Escape cancels the dialog.
4. Enter `/model` to select a model. Login selects the provider default only
   when the session has no resolved model; it does not replace an existing selection.

A saved key is trimmed at entry. Saving it does not test the account's permissions,
credits or model access. Bedrock's key choice instead opens AWS setup guidance;
there is no Bedrock API key saved by that dialog.

### Subscription accounts and charges

| Sign-in choice | Provider ID | Account and usage |
| --- | --- | --- |
| Anthropic (Claude Pro/Max) | `anthropic` | Claude Pro or Max subscription authentication. Requests draw from [extra usage](https://claude.ai/settings/usage), billed per token rather than against plan limits. |
| ChatGPT Plus/Pro (Codex Subscription) | `openai-codex` | ChatGPT Plus or Pro account. This is separate from the `openai` API-key provider; check the account's [Codex usage limits](https://developers.openai.com/codex/pricing/) before use. |
| GitHub Copilot | `github-copilot` | An account with Copilot access to the chosen model. Check [Copilot plans and billing](https://docs.github.com/en/copilot/concepts/billing/individual-plans) for quotas and charges. |

For Copilot, leave the enterprise-domain prompt blank for `github.com`, or enter
your enterprise server's domain. A successful login still depends on account and
organization model policy. If a model is refused, check that it is enabled in
your Copilot model selector and permitted by your organization.

Tokens are saved in the same credential file as API keys and refreshed when
expired. Do not edit token fields by hand. Anthropic subscription use shows a
billing warning once per interactive mode; `warnings.anthropicExtraUsage: false`
in [settings](sessions/settings.md) hides the warning, not the charges.

To remove a saved credential, enter `/logout` and select it. This removes the
local record; it does not revoke the remote authorization, cancel a subscription,
or clear environment variables, `--api-key` or `models.json`. Remove those
sources separately if you want the provider to become unavailable. Use the
provider's account controls to revoke access remotely.

The standalone `maestro-model-account` executable has a different storage scope:
it writes `auth.json` in its current working directory, not the session's agent
directory. Its `list` and `--help` commands do not authorize an account. Prefer
`/login` for session setup. See [command-line executables](cli.md#executables)
and the [login/logout behavior](chat/commands.md#signing-in).

## Environment keys

For example, set a key in the current shell, then inspect the available models:

```sh
export MISTRAL_API_KEY='replace-with-your-key'
maestro --list-models mistral
```

The search is fuzzy, not an exact provider filter. Listing models makes no paid
request. To use one, start `maestro --provider mistral --model '<model-id>'`
with an ID from the table, or choose it through `/model`. Sending a prompt can
incur provider charges. See [model selection](selection.md).

The provider ID is also the top-level key in `auth.json`. Variable names in a
cell are checked from left to right. Empty variables are absent; other values
are not trimmed. Use the variable spelling shown here. Provider IDs are case-sensitive.

| Provider | Provider ID / auth-file key | Environment key candidates |
| --- | --- | --- |
| Anthropic | `anthropic` | `ANTHROPIC_OAUTH_TOKEN`, `ANTHROPIC_API_KEY` |
| OpenAI | `openai` | `OPENAI_API_KEY` |
| OpenAI Codex | `openai-codex` | None; subscription login |
| GitHub Copilot | `github-copilot` | `COPILOT_GITHUB_TOKEN` |
| Azure OpenAI Responses | `azure-openai-responses` | `AZURE_OPENAI_API_KEY` |
| Amazon Bedrock | `amazon-bedrock` | AWS credential chain, not an API-key field; see below |
| Google Gemini | `google` | `GEMINI_API_KEY` |
| Google Vertex AI | `google-vertex` | `GOOGLE_CLOUD_API_KEY`, otherwise application-default credentials |
| Cerebras | `cerebras` | `CEREBRAS_API_KEY` |
| Cloudflare AI Gateway | `cloudflare-ai-gateway` | `CLOUDFLARE_API_KEY` |
| Cloudflare Workers AI | `cloudflare-workers-ai` | `CLOUDFLARE_API_KEY` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` |
| Fireworks | `fireworks` | `FIREWORKS_API_KEY` |
| Groq | `groq` | `GROQ_API_KEY` |
| Hugging Face | `huggingface` | `HF_TOKEN` |
| Kimi For Coding | `kimi-coding` | `KIMI_API_KEY` |
| Mistral | `mistral` | `MISTRAL_API_KEY` |
| MiniMax | `minimax` | `MINIMAX_API_KEY` |
| MiniMax (China) | `minimax-cn` | `MINIMAX_CN_API_KEY` |
| Moonshot AI | `moonshotai` | `MOONSHOT_API_KEY` |
| Moonshot AI (China) | `moonshotai-cn` | `MOONSHOT_API_KEY` |
| OpenCode Zen | `opencode` | `OPENCODE_API_KEY` |
| OpenCode Go | `opencode-go` | `OPENCODE_API_KEY` |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` |
| Vercel AI Gateway | `vercel-ai-gateway` | `AI_GATEWAY_API_KEY` |
| xAI | `xai` | `XAI_API_KEY` |
| ZAI | `zai` | `ZAI_API_KEY` |
| Xiaomi MiMo | `xiaomi` | `XIAOMI_API_KEY` |
| Xiaomi MiMo Token Plan (China) | `xiaomi-token-plan-cn` | `XIAOMI_TOKEN_PLAN_CN_API_KEY` |
| Xiaomi MiMo Token Plan (Amsterdam) | `xiaomi-token-plan-ams` | `XIAOMI_TOKEN_PLAN_AMS_API_KEY` |
| Xiaomi MiMo Token Plan (Singapore) | `xiaomi-token-plan-sgp` | `XIAOMI_TOKEN_PLAN_SGP_API_KEY` |

These mappings come from the [environment-key owner](../crates/maestro-models/src/builtins/env_api_keys.rs)
and the [built-in display names](../crates/maestro-catalog/src/provider_display_names.rs).
Configured authentication is not proof that a token works. Ambient discovery
checks are described in [request authentication](request-authentication.md).

## Request attribution

Attribution and session headers match the provider name or the base URL's parsed host, not
a substring of the address, so a proxy path that names a gateway gets none of them. The
headers and their conditions are listed in the [SDK guide](application/sdk.md).

## Routing preferences

A model's `openRouterRouting` compatibility value is sent as the request's `provider`
object whenever it is set, through a proxy or custom base URL as well as the routing
gateway itself.

## Credential file and key selection

Session credentials live at `~/.maestro/agent/auth.json` by default. A nonempty
`MAESTRO_CODING_AGENT_DIR` changes the agent directory, including this file and
`models.json`; see [configuration roots](application/configuration.md#identity-and-roots).
Never commit this directory, include it in an export, or paste its contents into
a bug report. Keys and tokens are stored as plain text, not encrypted.

On Unix, the session credential backend creates missing parent directories with
mode `0700`, creates the file with `0600`, and sets `0600` on writes. Existing
parent directories are not tightened. Check their access yourself; Unix mode
bits are not a Windows ACL guarantee. The standalone account command is not
this backend and should not be assumed to set those permissions.

An API-key entry has this shape (the example values are not credentials):

```json
{
  "mistral": { "type": "api_key", "key": "MY_MISTRAL_SECRET" },
  "openai": { "type": "api_key", "key": "example-literal-key" },
  "deepseek": { "type": "api_key", "key": "!printf '%s' example-helper-key" }
}
```

A key beginning with `!` runs the rest as a shell command. Successful stdout is
decoded as UTF-8 and trimmed; native commands have a 10-second timeout. Command
results, including no result, are cached process-wide for stored keys. Restart
Maestro after rotating a helper's output. Only configure commands you trust.
The example helper is for offline checks, not secret storage.

Otherwise, Maestro reads a nonempty environment variable named by the entire
`key` string. If it is absent or empty, that string itself is the literal key.
A misspelled variable name can therefore be sent as a credential. Literal and
environment values are not trimmed by the file resolver.

For ordinary session request keys, precedence is:

1. A nonempty `--api-key` runtime override.
2. The provider's saved API key or OAuth credential.
3. The provider environment candidates in the table.
4. A custom provider key from `models.json`.

A saved API-key helper that returns no value or an empty value stops the stored
lookup before environment discovery. A malformed stored API-key/OAuth record can
also prevent environment fallback. Expired OAuth refresh has its own failure
rules; see [request keys](credentials.md#request-keys). Fix or remove the stale
record rather than assuming an environment key wins. Bedrock uses AWS credentials
instead of this API-key chain.

### Inspect a disposable configuration without a request

Run this in a shell where `maestro` is on `PATH`. It creates a new directory and
does not overwrite your normal credentials:

```sh
scratch=$(mktemp -d)
mkdir -m 700 "$scratch/agent"
(umask 077; printf '%s\n' '{"mistral":{"type":"api_key","key":"example-offline-key"}}' > "$scratch/agent/auth.json")
MAESTRO_CODING_AGENT_DIR="$scratch/agent" maestro --list-models mistral
```

The model table confirms discovery, not request-time helper execution or remote
key validity. Delete the disposable directory when finished. Do not send a
prompt with the dummy key.

## Cloud services

The examples below configure discovery and list models only. Obtain credentials
through your cloud administrator; replace the sample scope and secret values
before making requests. No cloud login or paid call is required to inspect the
model table. Catalog prices are estimates, not your provider's bill.

### Azure OpenAI

Create a model deployment in your Azure resource and obtain its endpoint and
key. Configure either the endpoint or resource-name shorthand:

```sh
export AZURE_OPENAI_API_KEY='example-azure-key'
export AZURE_OPENAI_RESOURCE_NAME='example-resource'
maestro --list-models azure-openai-responses
```

Instead of the resource name, `AZURE_OPENAI_BASE_URL` accepts an endpoint such as
`https://example-resource.openai.azure.com` or a Cognitive Services root.
It wins over the resource name. Standard roots normalize to `/openai/v1`.
`AZURE_OPENAI_API_VERSION` defaults to `v1`. To map a catalog model ID to a
separately named deployment, set `AZURE_OPENAI_DEPLOYMENT_NAME_MAP` to
`model-id=deployment-name,other-model=other-deployment`. A match changes the sent
model name, not its catalog identity or prices. See [Azure request configuration](models/responses.md#cloud-invocation)
for SDK overrides and endpoint rules, and [Azure Responses setup](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/responses).

### Amazon Bedrock

Choose one credential source: an AWS profile, paired access/secret keys (plus
`AWS_SESSION_TOKEN` for temporary credentials), a Bedrock bearer token, or your
workload's role credentials. For a profile already configured by your administrator:

```sh
export AWS_PROFILE='example-profile'
export AWS_REGION='us-west-2'
maestro --list-models amazon-bedrock
```

Alternatives are `AWS_ACCESS_KEY_ID` plus `AWS_SECRET_ACCESS_KEY`, or
`AWS_BEARER_TOKEN_BEDROCK`. ECS container credentials
(`AWS_CONTAINER_CREDENTIALS_RELATIVE_URI` or `AWS_CONTAINER_CREDENTIALS_FULL_URI`)
and web identity (`AWS_WEB_IDENTITY_TOKEN_FILE` with `AWS_ROLE_ARN` at request
time) are supported.
Profile discovery alone does not verify that the profile exists. A selected
profile takes precedence over standalone environment access keys.

Region selection uses an explicit SDK option, then `AWS_REGION`, then
`AWS_DEFAULT_REGION`. Profiles may provide their own region; without a profile
or configured region, a standard catalog endpoint supplies it. Custom endpoints
without a profile or region fall back to `us-east-1`. See the full
[region and credential chain](models/cloud-conversations.md#region-profile-and-endpoint)
before using an inference-profile ARN or proxy.

For application inference profiles whose ID and name do not identify Claude,
`AWS_BEDROCK_FORCE_CACHE=1` enables cache points. `AWS_BEDROCK_FORCE_HTTP1=1`
forces HTTP/1. For a proxy, configure the model's `baseUrl` in `models.json`.
`AWS_BEDROCK_SKIP_AUTH=1` uses placeholder signing credentials and ignores bearer
tokens: use it only for a proxy designed not to authenticate with AWS.
Check [AWS model access](https://docs.aws.amazon.com/bedrock/latest/userguide/model-access.html)
and your IAM permissions before sending a prompt.

### Cloudflare

Create a token with access to the intended account/service. Both providers use
`CLOUDFLARE_API_KEY`; scope identifiers are environment variables even when you
save the key through `/login`:

```sh
export CLOUDFLARE_API_KEY='example-cloudflare-token'
export CLOUDFLARE_ACCOUNT_ID='example-account'
export CLOUDFLARE_GATEWAY_ID='example-gateway'
maestro --list-models cloudflare
```

Workers AI (`cloudflare-workers-ai`) needs the account ID only and uses direct
model IDs starting with `@cf/`. AI Gateway (`cloudflare-ai-gateway`) needs both
account ID and gateway slug. Its Workers AI route uses `/compat` and model IDs
starting with `workers-ai/@cf/`; OpenAI and Anthropic routes use `/openai` and
`/anthropic` with their native model IDs.

Gateway requests send the Cloudflare token in `cf-aig-authorization`. Configure
[unified billing](https://developers.cloudflare.com/ai-gateway/features/unified-billing/)
or [stored provider keys](https://developers.cloudflare.com/ai-gateway/configuration/bring-your-own-keys/)
in the gateway if you want Cloudflare to supply upstream authentication. Workers
AI needs no separate upstream key. Inline provider keys require an additional
upstream `Authorization` header through a model/provider override, not a second
value in `CLOUDFLARE_API_KEY`. See [catalog request headers](catalog.md#request-authentication).
Workers AI chat requests with a nonempty session ID send `x-session-affinity`
for [prefix caching](https://developers.cloudflare.com/workers-ai/features/prompt-caching/).
An unset placeholder variable fails at request preparation; model listing does
not check account IDs or gateway existence.

### Google Vertex AI

For native application-default credentials, authorize through your organization's
Google Cloud tooling first. The usual workstation route is
`gcloud auth application-default login`; this contacts Google and is not an
offline verification command. Alternatively, set `GOOGLE_APPLICATION_CREDENTIALS`
to your administrator-supplied credential file. Never commit that file.

After credentials exist, configure project and location:

```sh
export GOOGLE_CLOUD_PROJECT='example-project'
export GOOGLE_CLOUD_LOCATION='us-central1'
maestro --list-models google-vertex
```

`GCLOUD_PROJECT` is the fallback project variable. Without an explicit credential
path, native discovery checks `~/.config/gcloud/application_default_credentials.json`.
The path's existence is cached for the process lifetime: restart Maestro if you
create or replace that path after discovery. An explicit missing path does not
fall back to the default. A project, location and existing path enable discovery;
valid contents and permissions are checked later when preparing a request.

`GOOGLE_CLOUD_API_KEY` is the API-key alternative and takes precedence over
application-default credentials. It is different from Gemini's `GEMINI_API_KEY`.
Browser Vertex calls require a real API key; they cannot read your native ADC
file. See [Vertex invocation and credentials](models/content-generation.md#cloud-invocation)
and [Google ADC setup](https://cloud.google.com/docs/authentication/provide-credentials-adc).

## Custom endpoints and providers

For a service speaking a supported protocol, configure `providers` in your agent
directory's `models.json`: [catalog composition](catalog.md#loading-and-composition)
explains custom models, built-in overrides and request-time key/header resolution.
For a new protocol or OAuth flow, use an extension's provider registration:
[guest provider interfaces](extensions/guest.md) and [OAuth contracts](models/oauth.md).
These routes do not change the built-in credential precedence above.

## Troubleshoot without exposing a key

- No models listed: check the exact provider ID, empty variables and the selected
  agent directory. Discovery is local; cloud scopes and account access may still fail later.
- New environment key ignored: inspect the provider's saved record and any
  `--api-key` override. `/logout` does not clear your shell or model file.
- Helper output stale: restart the process. Keep secret output out of logs.
- Login saved only for this session: check credential-store diagnostics and file
  permissions. Persistence errors can leave the credential in memory without saving it.
- Request refused after successful setup: check model/deployment access, account
  billing and cloud scopes at the provider. Do not paste tokens into reports.
