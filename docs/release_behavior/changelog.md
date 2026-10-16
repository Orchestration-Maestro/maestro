# Current model behavior

Start with the [model invocation library](../models/api.md) for installation and
executable examples. This page points to current contracts, not past releases.
Model invocation, account primitives and agent execution have separate owners;
choose inputs for the operation you actually call.

## Models and authentication

A model descriptor supplies identity, protocol, endpoint, capabilities and rates.
Catalog membership does not register an invocation API. Use exact provider/model
IDs from the [catalog](../models/catalog.md), then check the
[bundled protocol table](../models/api.md#bundled-protocols-and-direct-operations).
The [custom descriptor example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_invokes_custom_descriptors`, sends an authored descriptor through
its actual protocol to a controlled endpoint.

[Request authentication](../request-authentication.md) separates explicit keys,
environment discovery and ambient-configuration markers. A marker is not a usable
secret or proof that an API can be invoked. Browser builds do not read a process
environment; see [native and browser authentication](../models/api.md#native-and-browser-authentication)
for the explicit-input boundary and compile qualification.

[Account flows](../models/oauth.md) expose login and refresh primitives, not an
implicit credential store. The caller supplies interaction callbacks and keeps
returned credentials. The [account examples](../../crates/maestro-models/tests/model_guide_examples/accounts.rs)
exercise supplied credentials and retained registry handles. Check the guide's
[Not available yet](../models/api.md#not-available-yet) section before relying on
an unfinished invocation, credential flow or command.

## Thinking and replay

Choose current thinking levels through the model's
[thinking-level map](../models/catalog.md), not an old capability
helper or compatibility effort map. Missing and explicit-null entries differ.
Raw provider options and simple thinking requests are not interchangeable; the
[thinking/options example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_selects_thinking_and_transport_options`, checks both routes.

[Conversation projection](../conversation-projection.md) creates a request view
without editing stored history. Replay compatibility considers provider, API and
model identity; signatures, foreign thinking, failed attempts, missing tool
results and image capability follow that owner's rules. Normalized tool IDs must
retain the association with each original call occurrence. Use the
[history example](../../crates/maestro-models/tests/model_guide_examples/providers.rs),
`maestro_guide_projects_and_serializes_history`, for typed persistence and an
actual cross-provider continuation rather than reconstructing displayed text.

## Streams and tool arguments

The [streaming contract](../models/api.md#streaming-and-completion) distinguishes
queued events from the shared final result. Dropping an observer is not
cancellation, and ending iteration without a result does not invent one. The
[stream examples](../../crates/maestro-models/tests/model_guide_examples/streams.rs)
cover interleaved content, terminal failures and cancellation after progress.

[Tool argument repair](../models/arguments.md) supports incremental display; a
completed tool-call event is not permission to execute. Use the returned
checked/coerced arguments only after explicit validation, as in
`maestro_guide_validates_before_tool_execution` in the
[tool examples](../../crates/maestro-models/tests/model_guide_examples/tools.rs).
The caller still owns authorization and the tool effect.

For an execution loop, continuation, steering and tool progress, use the
[agent library](../agent.md) and its
[controlled guide tests](../../crates/maestro-agent/tests/maestro_agent_guide.rs).
These operations are not a second agent API inside the model library.

## Transport, cache and usage

Timeouts, retries, header layering, response hooks and cache preferences belong
to the selected adapter. Empty common options do not select a universal policy.
Read [Messages](../models/messages.md), [compatible chat](../models/chat-completions.md),
[Responses](../models/responses.md), [response sessions](../models/response-sessions.md),
[content generation](../models/content-generation.md),
[cloud conversations](../models/cloud-conversations.md) or
[native reasoning conversations](../models/reasoning-conversations.md) for the
protocol being invoked. Typed retained hooks and clients are not JSON fields;
see [record contracts](../records.md) and [raw extras](../models/module-linkage.md#raw-extras).

Session affinity does not itself imply a retained socket. Response sessions own
automatic transport selection, pre-start fallback, cached input and connection
cleanup. Other registered session resources use the explicit
[cleanup contract](../records.md#cleanup-and-diagnostics). Reuse those owners
rather than treating observer drop as request or session teardown.

[Accounting](../models/accounting.md) documents reported token categories,
protocol-specific totals and rate-based cost estimates, not guaranteed billing.
[Model option helpers](../model-options.md) describe overflow classification;
a recognized overflow is not a policy to retry every provider failure.

## Current inputs and examples

Use [typed conversation and option records](../models/api.md#conversation-and-json-records),
current catalog IDs, caller-owned credentials and the selected protocol's options.
Old endpoints, removed providers, earlier defaults and credential-storage helpers
are not alternative current contracts. This page adds no compatibility reader or
migration format.

Run the complete offline
[quick-start example](../../crates/maestro-models/tests/model_guide_examples/streams.rs),
`maestro_guide_quick_start_completes_without_network`, through the guide's
[installation instructions](../models/api.md#installation-and-offline-quick-start).
For an opt-in deterministic two-turn model, use the existing
[scripted model guide](../models/faux.md) and its
[executing example](../../crates/maestro-models/tests/scripted_model_invocation.rs).
Ordinary builds stay offline; explicit
[catalog generation](../models/generation.md) commands acquire model feeds.
