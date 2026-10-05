# maestro-agent

Owns streamed conversation and executable-tool turns through `Agent`.
Embedding callers inject `maestro-models`, the only internal production dependency,
and ordered `Tool` callbacks through `AgentOptions`. Private modules own admission,
event reduction, FIFO policy, complete batch ordering and awaited progress settlement;
there is no alternate model invocation interface or public scheduler.

Tools prepare current-format input before shared validation. Ordered before hooks
may replace arguments without revalidation; ordered after hooks replace supplied
whole output fields. Parallel is default; any sequential override serializes the
whole batch. Completion-order execution ends and source-order transcript results
are distinct. Cooperative cancellation awaits launched tools and accepted progress,
without a cap, deadline or rollback guarantee. Details remain in raw history but
are stripped from model requests; termination hints remain runtime-only.

See [the agent guide](../../docs/agent.md) for configuration, errors and a compiling
credential-free example, [the glossary](../../CONTEXT.md) for execution terms,
and [the execution contract](../../docs/specs/2026-10-05-engine-core.md).

Focused checks:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo test -p maestro-agent
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just check
```

A credential-free executable callback uses only shared model output types:

```rust
use maestro_agent::{Tool, ToolResult};
use maestro_models::{InputContent, TextContent, ToolDeclaration};
use std::sync::Arc;

let tool = Tool {
    declaration: ToolDeclaration {
        name: "echo".into(), description: "Echo text".into(),
        parameters: serde_json::json!({
            "type": "object", "properties": {"text": {"type": "string"}},
            "required": ["text"],
        }),
    },
    label: "Echo".into(), prepare: None, execution_mode: None,
    execute: Arc::new(|invocation| Box::pin(async move {
        Ok(ToolResult {
            content: vec![InputContent::Text(TextContent {
                text: invocation.args["text"].as_str().unwrap().into(),
                replay_metadata: None,
            })],
            details: serde_json::json!({}), terminate: None,
        })
    })),
};
```

Supply it as `AgentOptions { tools: vec![tool], ..Default::default() }`.
The guide's complete example is compiled with the crate's documentation tests.
