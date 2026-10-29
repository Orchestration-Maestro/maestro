# Authored subagents

`maestro-subagents` provides the `subagent` extension tool. Its
[README](../../crates/maestro-subagents/README.md) contains build/load commands,
agent definitions, all three invocation modes and controlled tests. The package
ships four sample agents and three workflow prompt templates.

The reusable library exposes `factory`; its one component export lives in
`src/component.rs`, built with `--example component --target wasm32-wasip2`.
Unlike the shared extension examples, this component belongs to its package
crate. It adds no entry to the shared example target or preparation lists.

## Execution boundary

Each task launches the configured Maestro executable in JSON print mode with
`--no-session`. Optional model/tools, a private appended prompt file and the task
are literal arguments; each child gets its own context. `cwd` is per task.
The native process owner supplies streaming pipes, exit observation and signals;
JSONL framing is shared with other Maestro connections. A final unterminated
line is accepted and malformed lines are ignored.

Single and chain failures retain child messages, stderr, usage and error details.
A chain stops on its first failure; parallel execution keeps task order while
at most four children run across at most eight tasks. Cancellation reaches every
owned child; no new deadline, supervisor runtime or background scheduler is added.
The renderer derives failure status from the retained details.

## Discovery boundary

Discovery reads the selected agent directory and the nearest project's
`.maestro/agents` afresh for every invocation. User scope is the default; project
scope and project shadowing require explicit selection. Requested project agents
need confirmation only in interactive mode with `confirmProjectAgents` enabled.
Definition parsing uses the host's frontmatter owner, not a package YAML parser.
Authored native paths and source labels remain in the discovered records.

These definitions are not manifest roles. This package adds no manifest policy,
role-identity flag, background runner or scheduling setting.

## Qualification

The composition-root `subagent_example` tests execute the real component with
controlled filesystem fixtures, original native children and the real UI binding.
They cover scope/shadowing, links, trust, the three modes, streaming/usage,
validation, failures, cleanup, the concurrency limit and cancellation. Native
Cargo tests also cover the public agent-list formatter without effects.

Native coverage instrumentation cannot measure code executed inside a component.
Numerical component coverage is tracked in
[the workflow coverage qualification](https://github.com/Orchestration-Maestro/maestro-rust-workflows/issues/207).
