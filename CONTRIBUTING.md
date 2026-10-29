# Contributing to Maestro

This guide exists to save both sides time.

## The One Rule

**You must understand your code.** If you cannot explain what your changes do and how they interact with the rest of the system, your PR will be closed.

Using AI to write code is fine. Submitting AI-generated slop without understanding it is not.

If you use an agent, run it from the repository root directory so it picks up `AGENTS.md` automatically. Your agent must follow the rules and guidelines in that file.

Use the [identity guide](docs/identity.md) for voice, code naming and brand assets.

## Quality Bar For Issues

If you open an issue, you must use one of the two GitHub issue templates.

If you open an issue, keep it short, concrete, and worth reading.

- Keep it concise. If it does not fit on one screen, it is too long.
- Write in your own voice.
- State the bug or request clearly.
- Explain why it matters.
- If you want to implement the change yourself, say so.

## Blocking

If you ignore this document twice, or if you spam the tracker with agent-generated issues, your GitHub account will be permanently blocked.

If you send a large volume of issues through automation, your GitHub account will be permanently blocked. No taksies backsies.

## Before Submitting a PR

Before submitting a PR:

```bash
just check
just test
```

Both must pass. `just test` runs cargo-nextest followed by Cargo doctests.
Nextest flags a test as slow at 30 seconds and kills it at 120 seconds, with a
ten-second shutdown grace period on Unix. Fix a slow test at its cause. A test
that needs more than 120 seconds needs a per-test or shared-fixture exception
with its measured cause and an open removal issue. Fixing the cause also removes
the exception; see [test time budget](docs/development.md#test-time-budget).
Required CI runs tests on Linux and checks compilation for Windows and macOS.
Native Windows and macOS test runs must be restored before the first release.
A test that binds the fixed OAuth callback port goes in a module named
`oauth_callback`, which nextest runs one at a time.
Shared Linux CI reserves ports 53692 and 1455 from automatic allocation so OAuth callback tests can bind their required ports.
Development and test builds retain file:line debug information, not debugger
variable views.

After changing a dependency, its features or `Cargo.lock`, run `cargo hakari generate`
(and `cargo hakari manage-deps` after adding a crate) and commit the result;
`just check` runs `cargo hakari verify`. See
[feature unification](docs/development.md#feature-unification).

Do not edit `CHANGELOG.md`. Changelog entries are added by maintainers.

If you are adding a new provider, see `AGENTS.md` for required tests.

## Code quality limits

`just check` enforces at most 5 parameters (at most 1 boolean), 60 lines per
function, cognitive complexity 15, nesting depth 4 and 500 production lines per
Rust file. Test directories, `tests.rs` and trailing test modules do not count
against file length. Pedantic Clippy lints are errors; production code may not
use `unwrap`, `expect` or `panic!`. Unsafe code is forbidden. Thresholds live in `clippy.toml`; levels live in
`[workspace.lints]` and crates inherit them with `[lints] workspace = true`, except
for the named `maestro-extensions-wasm` binding
exception documented in [quality checks](docs/architecture.md#quality-checks). The workspace forbids
protected quality lints, so the compiler rejects their `allow` and `expect`
attributes, including attributes emitted by macros.

## Philosophy

Maestro's core is minimal. If your feature does not belong in the core, it should be an extension. PRs that bloat the core will likely be rejected.

## Questions?

## FAQ

### Why do some issues get no reply?

A reply is maintenance work too. Low-signal issues, unclear reports, duplicates, and issues that do not follow this guide may be closed without discussion. This keeps time available for reproducible bugs, thoughtful requests, and contributors who have done the work to make their report actionable.

### Why not let AI triage everything?

AI can help group duplicates, summarize reports, and spot missing information. It is not trusted to make final maintainer decisions. Polished AI-generated issues can still be wrong, misleading, or expensive to investigate. Human review remains the final gate.

### Is this hostile to contributors?

No. It is a guardrail against burnout and tracker spam. Short, concrete, reproducible issues are welcome. Thoughtful contributions are welcome. Automated slop, entitlement, and large volumes of low-effort reports are not.

Every crate builds its integration tests as one program from
`tests/integration.rs`. Add new `tests/*.rs` files as unconditional out-of-line
modules in that root; support modules stay in their existing directories. The
conventions check covers every workspace member, new crates included: a member
with top-level integration-test sources needs `autotests = false` and exactly
one test target, the runnable `integration` target at `tests/integration.rs`.
