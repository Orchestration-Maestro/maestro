# Embedding examples

Run an example from the repository root:

```sh
cargo run -p maestro-app --example minimal
```

These samples use the public session SDK and its scripted model provider. Model
requests stay offline and use nonsecret sample keys. Each run creates disposable
configuration and project files, then removes them on exit. Persistent-session
and settings examples write only inside that disposable directory.

## Samples

The sources are in [`crates/maestro-app/examples/sdk`](../../crates/maestro-app/examples/sdk).
Each target has executable behavior tests selected by the default application
test command:

```sh
cargo nextest run -p maestro-app
```

| Target | Demonstrates |
| --- | --- |
| `minimal` | Prompt a session, receive text deltas and inspect stored messages. |
| `custom_model` | Find catalog models, select one explicitly and request a thinking level. |
| `custom_prompt` | Replace a system prompt or append instructions to the discovered prompt. |
| `skills` | Filter discovered skills and add an SDK-attributed skill. |
| `tools` | Select read-only or custom built-in tools for a supplied working directory. |
| `extensions` | Register an inline extension with a command, events and an executable tool. |
| `context_files` | Discover project instructions and add caller-supplied context files. |
| `prompt_templates` | Merge discovered templates with a deployment template and expand it. |
| `api_keys_and_oauth` | Supply credential owners, custom paths and non-persisted runtime keys. |
| `settings` | Apply runtime preferences, change a persistent preference and flush it. |
| `sessions` | Use memory history, create a file, continue it, list sessions and reopen one. |
| `full_control` | Select model, thinking, settings, tools and resources without discovery. |
| `session_runtime` | Create directory-bound services, replace the session and rebind subscriptions. |

## Resource and session ownership

Keep the caller-owned local executor alive while a session runs. Retain the
session's subscription removal operation, and remove old subscriptions before
binding a replacement session. Dispose sessions when their work ends.

Load resources before applying runtime settings overrides, then supply that
loaded resource owner to session creation. A second reload currently discards
those overrides; [override retention](https://github.com/Orchestration-Maestro/maestro/issues/3591)
tracks that behavior. The settings sample shows the required order.

The inline extension sample needs no component runtime. Native component
loading belongs to the embedding executable: supply the external runtime
adapter as the resource loader's module loader. The executable's SDK tests
exercise installed component packages, tool reload, asynchronous standard
filesystem operations and retained standard-library file operations.

## Using a live provider

Live model requests require a configured provider, an available model and
credentials. OAuth also requires the provider's interactive login
flow and any callback listener it uses. Those prerequisites are not part of the
default samples or their tests. Explicit live model requests can incur charges.

Replace the scripted model registration with your configured catalog and
credential owner when integrating a live provider. See [the SDK](sdk.md)
and [credentials](../credentials.md) for the public interfaces. For subprocess
embedding rather than an in-process session, use [the RPC client](../client.md).
