# Development prompts

Four supplied Markdown instruction files cover repository development:

| Prompt | Purpose | Argument hint |
|---|---|---|
| [cl](agents/prompts/cl.md) | Audit release metadata | none |
| [is](agents/prompts/is.md) | Analyze issues without implementing by default | `<issue>` |
| [pr](agents/prompts/pr.md) | Review pull requests | `<PR-URL>` |
| [wr](agents/prompts/wr.md) | Finish the current task through protected delivery | `[instructions]` |

The issue and completion files retain literal `$ARGUMENTS`; the review file
retains `$@`. These are authored template inputs, not shell commands.

Load these files by supplying their explicit paths to
[`load_prompt_templates`](../crates/maestro-resources/src/prompt_templates.rs)
with `LoadPromptTemplatesOptions` and `NativeResourceOperations`. Set
`include_defaults: false` to load only the supplied paths. The `docs/` location
is not an automatic discovery directory. Loading and expansion behavior belongs
to the [prompt-template resource guide](resources/prompt-templates.md).

The instructions use the repository's existing triage, approval and signed-commit
rules. Release auditing checks conventional commits, PR descriptions and available
release metadata; implementation PRs do not edit CHANGELOG.md. The prompts are
instructions, not a guarantee that an assistant executes their steps.

`.maestro/git/.gitignore` and `.maestro/npm/.gitignore` ignore local cache
contents while keeping each immediate `.gitignore` visible. A nested `.gitignore`
inside an ignored directory stays ignored. These rules do not hide prompt sources.

## Qualification

The resource integration test loads the four shipped files and checks their
metadata and literal argument lines. The conventions integration test runs native
Git against 19 paths: ten ignored cache paths and nine visible paths. Instruction
paragraphs require manual review; they are not executable workflow tests.

```sh
cargo test -p maestro-resources --test development_guidance --locked
cargo test -p maestro-test-conventions --test development_guidance --locked
mise exec -- just check
```

Use the pinned tools and repository checks described in
[AGENTS.md](../AGENTS.md). Shared CI runs the complete test suite.
