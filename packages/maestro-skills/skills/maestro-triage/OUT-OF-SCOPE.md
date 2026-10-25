# Retain rejected enhancement decisions

Keep one `.out-of-scope/<concept>.md` per rejected enhancement concept. Use a short
recognizable kebab-case name. Several requests for the same idea belong in one file;
match by meaning, so 'night theme' can match 'dark mode' without sharing keywords.

## Record the reason

Write a readable design note with the concept, decision, durable reason and linked
prior requests. A reason names a project scope choice, technical constraint or strategic
trade-off. Temporary lack of time is a deferral, not a durable rejection. Use examples
or code where they clarify the reason; keep invented examples clearly illustrative.

```markdown
# Example: hosted synchronization

Decision: this example project stores data locally.

## Why this is out of scope
Hosted synchronization requires account and conflict-resolution policy that belongs
to the embedding application, not the local reader.

## Prior requests
Add named links to the actual requests that established or revisited this decision.
```

## Check before creating

During triage, read existing concept records and show likely matches to the maintainer.
Ask whether the reason still applies; do not automatically reject a superficially
similar request. Keep their responses distinct:

- Confirm: append the new named request link to the existing record, explain and close.
- Reconsider: update or remove the record and continue normal triage. Old issues remain
  historical; reconsideration does not automatically reopen them.
- Distinct: continue triage without merging different concepts into one rejection.

Only rejected enhancements enter this knowledge base, including rejected enhancement
requests with code attached. Already-implemented requests are not rejections: close
with a pointer to the implementation. Rejected bugs get an explanation and closure,
not an enhancement record.

When creating a new record, confirm the decision, check for a matching concept, write
or append the durable reason and real request links, post the explanation with the
[triage disclosure](SKILL.md), then close using the repository's mapped wontfix role.
