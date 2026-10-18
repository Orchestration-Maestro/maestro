# Write a durable agent brief

The brief is the contract for an AFK implementer. The original request and discussion
remain context. For an issue, describe the new change. For a pull or merge request,
describe work remaining on the supplied diff rather than asking for a second build.
Start every posted brief with the [triage disclosure](SKILL.md).

Name behavioural contracts, interfaces, types and config shapes. Prefer durable
interfaces over current file paths, line numbers and a recipe tied to today's layout.
The implementer will explore the current tree. Acceptance must be independently
checkable, and exclusions must stop plausible adjacent work.

## Template

```markdown
## Agent brief

Category: bug / enhancement
Summary: one-line change

### Current behaviour
What happens now, including the supplied diff's working path for a request.

### Desired behaviour
What users should observe, including realistic errors and edge cases.

### Key interfaces
Named inputs, outputs, types, config shapes and retained contracts.

### Acceptance
- [ ] Independently observable result.
- [ ] Required edge case and error behaviour.
- [ ] Existing behaviour that must remain unchanged.

### Out of scope
Adjacent features and policy changes this brief does not authorize.
```

## Examples are illustrative, not command exports

Good bug brief: 'The example CatalogReader rejects an empty optional description.
Accept present-empty and absent descriptions separately, retaining all other validation.
Observe both through the public reader; invalid entry ids must still fail.'

Bad bug brief: 'Fix the catalog thing in the main file near line 150.' It gives no
current/desired distinction, stable interface, acceptance or exclusion.

Good enhancement brief: 'The example archive reader accepts one authored source.
Allow an ordered source list; first matching record wins. Observe two distinguishable
matches and an empty list. Parallel fetching and new archive formats are excluded.'
This supplies a contract without prescribing loops or guessed implementation files.

For a request, suppose a contributor has added an illustrative `--json` flag to an
example `catalog list` command. The happy path already serializes records, but errors
are still plain text. A good brief asks to finish that diff: serialize failures under
the flag, retain exit codes, keep default output unchanged and test success and error.
It explicitly excludes adding the flag to other commands or changing the success
schema. These invented example commands do not assert that Maestro provides them.

Avoid criteria like 'triage works'. Name the action and the observable result so a
fresh implementer and reviewer can agree whether the work is complete.
