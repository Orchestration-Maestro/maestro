# Repository contribution policy

The private `repository_policy` developer binary runs four trusted metadata
adapters. It is not an application CLI or a public library API.

```sh
just check
just test
```

Use a credential-cleared child with fresh HOME/config/TMPDIR/XDG directories
for every local test/doc/conventions/hook command. Leave actual authentication
untouched. Shared `just ci` is the merge authority. Fixtures never mutate live
issues or pull requests or call a provider.

## Invocation and outputs

Build `maestro-test-conventions`' `repository_policy` binary with locked dependencies
from the trusted default branch, without a write token during compilation.
Invoke it with exactly one argument: `approve-contributor`, `issue-gate`, `pr-gate`
or `contribution-policy`. The adapters provide `GITHUB_EVENT_NAME` and
`GITHUB_EVENT_PATH`; approval additionally supplies `MAESTRO_APPROVAL_APP_SLUG`.
The installed `gh` CLI receives credentials only in its environment, structured
JSON stdin and explicit argv. No submitted text is evaluated as shell code.

Created-comment approval emits `status=skipped|already|added|updated`, followed
by `capability=issue|pr` when present, to `GITHUB_OUTPUT`. Other routes emit no
step outputs. The approval step is named `update`. Errors produce a nonzero exit
and credential-free branch diagnostics. The capability-setting diagnostic reports
the computed list change before publication, not effective approval; the approval
reply still waits for the protected merge.

## Capabilities and event order

`.github/APPROVED_CONTRIBUTORS` uses `username capability`; recognized capabilities
are `issue` and `pr`. Comparisons ignore Unicode case. ECMAScript whitespace,
literal LF records, preserved comments/malformed lines and last-valid-duplicate
lookup are intentional. A changed write normalizes recognized entries only,
changes the last matching duplicate and retains other bytes/order. A no-op
never normalizes the list. New entries preserve the author's spelling.

- Approval admits only created non-PR issue comments. ASCII whole-word `lgtmi`
  wins over `lgtm`, even if `lgtm` appears first. Permission is checked before
  the trusted local list: only admin/maintain/write may request a change.
- Already-effective approval replies immediately without a write or downgrade.
  Added/updated approval creates a numeric-ID `chore/` branch from the trusted
  default head, a GitHub-signed `createCommitOnBranch` commit changing only the
  approval file, then a PR and ordinary squash auto-merge bound to its exact head
  SHA. Before branch creation, `git rev-parse HEAD` must match the fetched
  default-branch SHA; otherwise the operation fails with
  `Trusted checkout HEAD does not match default branch SHA`. There is no retry.
  The signed commit retains its expected-head concurrency check. There is no
  direct default-branch push or bypass.
- Only a qualifying merged, same-repository, default-base PR from the configured
  App may continue approval. Trusted metadata must have the expected branch,
  marker and exact changed-file set. The original issue/comment, commenter
  permission, requested capability and effective default-branch list are fetched
  again before a reply. Pending or rejected approvals never receive success text.
- Issue and PR gates admit opened events only (`pull_request_target` for PRs).
  Bots ending in case-sensitive `[bot]` return before reads. Write+ collaborators
  are checked before the default-branch list. Either capability permits issues;
  only `pr` permits PRs. Unauthorized guidance is posted first, then issue labels
  if any, then closure. PR closure uses the pulls endpoint, with no labels.
- Activity checks bots, then either recognized approval, then collaborator access,
  then configured repositories in order. A positive match adds one informational
  label only; it never comments, closes or blocks the item.

Approval-file errors fail issue/PR gates instead of closing on unavailable data.
Activity logs read/permission/search errors and continues; search failure or no
match succeeds without a label. Label, comment, close, branch, commit, PR and
enqueue failures stop at that operation. There is no retry, rollback or polling.

## Data and defaults

`.github/repository-policy.json` has one current shape:

| Field | Default and meaning |
| --- | --- |
| `issue_gate.message_mode` | `normal`; only exact `refactor` selects maintenance guidance |
| `issue_gate.weekend_days` | `[]`; UTC Sunday=0 through Saturday=6, empty disables the route |
| `issue_gate.weekend_message` | Empty; optional configured paragraph immediately after ordinary opening text |
| `issue_gate.weekend_labels` | `[]`; added before maintenance labels when the selected day matches |
| `issue_gate.refactor_until` | Empty; configured maintenance date is message data, not an expiry clock |
| `issue_gate.refactor_branch` | Empty; optional repository branch link |
| `issue_gate.refactor_reason` | Empty; optional complete reason paragraph |
| `issue_gate.refactor_labels` | `[]`; labels for explicit maintenance mode |
| `help_url` | Empty; optional maintenance emergency destination, never invented |
| `activity_gate.repositories` | `[]`; ordered repository selectors for `repo:<repository> author:<login>` search, `per_page=1` |
| `activity_gate.label` | Empty; an empty label or repository list disables activity effects |

Dates use issue `created_at`, not wall time. UTC offset boundaries, date-only
input, normalized day overflow and midnight `24:00:00` follow metadata timestamp
semantics. Invalid dates do not match a schedule. Runner timezone is explicitly
UTC. Base64/base64url content accepts whitespace and missing padding, retains
BOM and uses replacement characters for invalid UTF-8.

The committed policy has no historical users, active schedule, maintenance event,
help destination or external activity repository. Forms disable blank issues;
contact links are empty. Bug reports require description/repro, proposals require
what/why; expected/version/how are optional. Keep templates and help documentation
consistent when policy data changes. Existing triage-role labels are unchanged.

## Activation and limits of evidence

Workflows checkout only the explicit default branch with credentials not persisted;
all action references are full SHAs. They never build submitted code, use head
artifacts, or put a write-token environment on the build step. Approval mints
its App token after build with only contents/issues/pull-requests write scope;
other gates use the ordinary scoped workflow token.

Activation requires `RELEASE_APP_CLIENT_ID`, `RELEASE_APP_PRIVATE_KEY`, a visible
App installation with those permissions, GitHub-signed commits, and protected
CI/auto-merge/merge queue operation. Missing credentials or permissions cause
failure, never approval. Controlled process/YAML/Git fixtures prove deterministic
routing and operation contracts, not hosted App availability or queue activation.
Real activation needs separately authorized metadata fixtures; local tests never
comment on or close live items.

## Dependency closure

Runtime addition is base64 0.23.1 (`std` only, defaults off).
ISO dates use integer Gregorian day and millisecond arithmetic with the full
±8.64e15 ms time-value range; no library date range constrains routing.
Numeric fields require ASCII digits. ISO date-times use a `T`/`t` separator;
offsets require two-digit hours within 0–23 and two-digit minutes, with or
without a colon. Date-only forms and expanded years are accepted except the
negative-zero expanded year. Space-separated and other non-ISO date text is
invalid and receives no schedule guidance or labels. A 213-input differential
routing test covers field grammar, fractions, offsets, partial dates and time
limits.
Test-only yaml-rust2 0.13.0 (defaults off) resolves arraydeque 0.5.1 and hashlink
0.12.2; hashlink resolves hashbrown 0.17.1, which resolves foldhash 0.2.0.
The direct additions and transitive dependencies are MIT OR Apache-2.0, except
foldhash 0.2.0, which is Zlib licensed; the lockfile records the exact versions.
No remote retrieval, timezone, SIMD default or alternate-encoding feature is used.
The existing serde_json closure is reused. Library types stay private.
