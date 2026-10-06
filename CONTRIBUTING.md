# Contributing to Maestro

## The one rule

You must **understand your code**: explain what it does and how it interacts
with the system. Generated code is welcome only when you understand it.
Run your agent from the repository root so it discovers `AGENTS.md`.

## Contribution gate

New contributors' issues and PRs are auto-closed. Maintainers review
closed issues daily and reopen worthwhile reports. Reports below the quality
bar may receive neither reopening nor a reply.

Authorized maintainer comments request approval:

- `lgtmi` requests future issue rights only.
- `lgtm` requests future issue and PR rights.

A changed approval becomes effective only after its protected approval PR has
merged. An already-effective approval receives an immediate reply.
`lgtmi` never grants PR rights. Write-level collaborators and bots are exempt
from the issue/PR gates. Approval commands themselves have no bot exemption.
See [repository policy](docs/repository_policy.md) for exact matching and timing.

The current policy has no weekend restriction, maintenance freeze or external
help destination. Those are configurable data, not an assumed active schedule.

## Report quality

Use the bug or contribution proposal form. Keep the report on one screen,
concrete and in your own voice. Include a reproducible bug or clear request,
why it matters, and whether you intend to implement it. A maintainer may
reopen, request approval, or decline; human judgment is final.

Ignoring this guide twice, spamming agent-generated reports or sending large
volumes of automated issues can result in permanent account blocking by
maintainers. The automation does not block accounts.

## Before submitting a PR

Obtain effective `lgtm` approval first. Read `AGENTS.md` and the linked spec.
Run both commands with a credential-cleared, disposable HOME/config/temp/XDG
child environment; do not move or delete your actual authentication files:

```sh
just check
just test
```

Both must pass; shared `just ci` is the merge authority. Follow signed commits,
protected PRs and the merge queue. Do not edit `CHANGELOG.md`: release automation
creates it from conventional commits. See the provider checklist in `AGENTS.md`
when contributing model protocols or providers.

## Philosophy and questions

Keep the core minimal. Prefer an extension for behavior outside the core's job.
This repository currently publishes no external support destination; do not
invent one or use the bug tracker for unrelated questions.

## FAQ

### Why auto-close new contributions?

The gate creates a review buffer rather than promising immediate review.
Reproducible, thoughtfully written reports can be reopened on a maintainer's
schedule. Submitting generated text without checking it transfers work to
maintainers instead of helping them.

### Why support a weekend route?

Maintainers need uninterrupted time away from triage. The mechanism can select
configured UTC days and guidance, but the committed schedule is off. No weekend
report is currently excluded from review by a special schedule.

### Why do some issues get no reply?

Replies are maintenance work. Unclear reports, duplicates and low-signal text
can be closed without discussion, leaving time for actionable bugs and requests.

### Why not let automation make final decisions?

Automation can summarize and group reports; polished output can still be wrong.
Maintainers make the final human triage and approval decisions.

### Are thoughtful contributions welcome?

Yes. Short, concrete reports and understood changes are welcome. The gate limits
burnout and spam; it does not replace maintainer review.
