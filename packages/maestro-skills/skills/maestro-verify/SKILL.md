---
name: maestro-verify
description: "Use when about to claim completion, a fix, a passing check, or a non-trivial design guarantee."
---

# Establish evidence before claiming success

## Run the exact check

Identify the full command that proves each claim. Run it fresh, without truncating the work to a convenient
subset. Read its complete output, exit code and counts. State only what that evidence proves.
An old run, a successful agent report, a clean lint run or a plausible diff does not prove the tests pass.

Keep a claim/evidence table: claim, exact command or artifact, observed result, and remaining limit.
A requirement checklist and test output answer different questions; inspect both.
Use the [definition of done](../../references/definition-of-done.md) before marking work complete.

## Doubt non-trivial claims

For non-trivial code, uncertain designs, irreversible changes or visual work, name the claim and why it matters.
Extract the smallest reviewable artifact and its contract, stripping your reasoning and desired conclusion.
Give only that artifact and contract to a fresh independent `subagent` using the
[review brief rules](../../references/subagents.md). Ask it to find violations, hidden assumptions and failure modes,
not to validate your confidence. Visual claims need the rendered artifact, not only source.

Re-read each finding against the artifact and classify it in order:

1. **Contract misread:** repair the unclear contract before judging the artifact.
2. **Actionable:** change the artifact and verify it.
3. **Trade-off:** document the real cost and let the user see what is accepted.
4. **Noise:** explain why it is correct; add missing context to the contract when that would prevent confusion.

Stop when only trivial or already considered findings remain, after at most three cycles, or at an explicit user
override. Substantive findings still open at that point go to the user, not into an unbounded review loop.
Inside a subagent, do not start another doubt pass or label self-review independent; report the missing pass
to the parent so it can arrange it.

## Written designs

Ask a fresh builder to read the design as the person who must implement it. Classify questions as already answered,
minor clarifications or user decisions. Resolve what the document already determines, then put remaining decisions
to the user together. Recheck the revised artifact and stop for approval rather than starting downstream work.
