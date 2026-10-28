# Custom gateway provider

The [`custom-provider-gitlab-duo/main.rs`](custom-provider-gitlab-duo/main.rs)
example adds the `gitlab-duo` provider. It serves Claude Opus 4.5, Sonnet 4.5
and Haiku 4.5, and GPT-5.1, GPT-5 Mini and GPT-5 Codex through GitLab's AI
gateway. Each request runs on Maestro's built-in Anthropic or OpenAI protocol.
Loading the component is opt-in.

## Prerequisites

- A GitLab account with GitLab Duo enabled.
- A GitLab sign-in: run `/login gitlab-duo`, or set `GITLAB_TOKEN` to a GitLab
  access token.
- The pinned toolchain and the `wasm32-wasip2` target, to build the component.

Each request trades your GitLab token for a direct access token at `gitlab.com`,
then sends the conversation to `cloud.gitlab.com`. A real run shares your
conversation with GitLab and can incur charges. The controlled runs below use
no network and no credentials.

## Build

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example custom_provider_gitlab_duo \
  --target wasm32-wasip2 --release
```

## Run a greeting probe

Put the built `custom_provider_gitlab_duo.wasm` in a directory, for example
`./gateway/`, then load the component file in print mode:

```sh
maestro -e ./gateway/custom_provider_gitlab_duo.wasm --model gitlab-duo/claude-sonnet-4-5-20250929 \
  --mode json -p "Say hello in exactly 3 words."
```

Use any registered model ID listed above with the `gitlab-duo/` provider prefix.
Add `--thinking low` to request low reasoning. JSON mode prints the stream's
thinking, text and final outcome. This is a live request, not a controlled test.

Maestro uses stored credentials from `/login gitlab-duo`, or `GITLAB_TOKEN`.
Without either, the stream reports `No GitLab access token. Run /login gitlab-duo
or set GITLAB_TOKEN`. Load the extension when signing in so its login is available.

## Registration

The component registers the gateway address, the `GITLAB_TOKEN` key variable, the
`gitlab-duo-api` protocol identifier, the six models with their prices, context
windows and output limits, and the GitLab Duo sign-in.

## Direct access

The first request posts your GitLab token to
`/api/v4/ai/third_party_agents/direct_access`. Maestro reuses the returned token
and headers for 25 minutes for the same GitLab credential, and drops them when
you change credentials, sign in or refresh. A 403
reports that GitLab Duo is not enabled for the account. Any other failure reports
its status and body, and the status stays even when the body cannot be read.

## Routing

Claude models use the Anthropic protocol through `/ai/v1/proxy/anthropic/`. GPT
models use the OpenAI Responses protocol through `/ai/v1/proxy/openai/v1`. The
request carries the direct access headers and bearer token, and replaces any
headers the caller supplied. Events reach the caller in order. A missing token,
an unknown model or a failed direct access ends the stream with an error message.

## Sign-in

`/login gitlab-duo` shows GitLab's authorization address with a proof key. After
you approve, the browser lands on `http://127.0.0.1:8080/callback`, where nothing
listens. Paste that full address into the prompt. Maestro checks its `state`,
exchanges the code, and stores the credentials five minutes before they expire.
Refresh uses the stored refresh token.

## Controlled runs

The first command runs sign-in, refresh and direct access with a movable clock.
The second loads the built component against the native catalog, with controlled
GitLab replies and controlled protocols. It covers registration, the route of each
model, error messages, raw options, sign-in through the host and the probe. The
probe reads the stored sign-in, asks one model for a three-word greeting with a
100-token limit, adds low reasoning with `--thinking`, and prints thinking, text,
errors and the final reason.

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_gateway_provider_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- custom_gateway_provider_
```
