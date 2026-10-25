# Learning workspace formats

These describe files in the invocation workspace, not inside the installed skill package. Templates use
placeholders; they are not reports of goals, verified resources or demonstrated learning.

## Mission

Keep `MISSION.md` short, with one concrete mission:

```markdown
# Mission: <topic>

## Why
<The real-world outcome motivating learning.>

## Success looks like
- <An observable thing the learner will be able to do.>

## Constraints
- <Time, budget, prior commitments and preferences.>

## Out of scope
- <Excluded topics.>
```

Interview vague goals before writing. Prefer a specific outcome over "understand the topic". Unrelated
missions use separate workspaces. Revise when goals change, after the learner confirms, and capture the
change in a learning record rather than leaving an obsolete mission to steer lessons.

## Resources

Use `RESOURCES.md` with Knowledge, Wisdom (Communities) and, when needed, Gaps headings. Annotate every
link with what it covers and when to use it. Prefer primary sources, recognised experts, peer-reviewed work
and well-moderated communities; leave out marketing disguised as education.

Write down gaps that block the mission so future search has a purpose. Prune incorrect, shallow or off-mission
resources. Record community preferences, including an opt-out, so recommendations are not repeated unwantedly.
Use placeholder resource entries until the actual source is checked; an illustrative list is not a set of
verified recommendations.

## Learning records

Create `learning-records/` when the first record is needed. Find the highest existing number and add one;
name the next file `0001-slug.md`, with the actual next number. The whole record can be a short title and
one to three sentences stating what was learned or established and why it changes subsequent teaching.

Record non-obvious demonstrated understanding, the stated depth of prior knowledge, a corrected misconception
or a confirmed mission shift. Add evidence and implications only when useful. Exposure, activity logs and
repeated glossary definitions do not qualify. A learner's claim of prior knowledge is attributed as such,
not silently promoted into a checked performance result.

When contradicted, mark the earlier record superseded by the new record's number rather than deleting it.
Preserve the history of changing understanding. A mission-change record points to `MISSION.md`.

## Glossary

Use `GLOSSARY.md` as the canonical learning vocabulary. Add a term only after the learner demonstrates
understanding, not when it is merely introduced. Write a concise one- or two-sentence definition of what it
is, using established terms. Choose one spelling and record aliases to avoid.

Group terms naturally if needed; a flat list is fine. Resolve ambiguous usage explicitly for this workspace.
As understanding improves, revise the definition rather than retaining a stale one. Apply these terms in
lessons, exercises and records without duplicating the definitions in each file.
