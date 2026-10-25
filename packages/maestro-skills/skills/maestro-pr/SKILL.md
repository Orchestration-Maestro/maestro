---
name: maestro-pr
description: "Use when a verified change needs a pull request description."
---

# Explain the change for review

Use the repository's domain vocabulary. Keep the body in three sections and omit conversational preambles.

## Summary

Choose the smallest picture that makes ownership and the change clear:

- Pseudocode for a decision or algorithm.
- A call tree for runtime flow.
- A component tree for visible structure and state ownership.
- A shallow file tree for responsibilities or layout.
- A diagram for relationships, sequence or data movement.
- A diff sketch when an existing shape gains or loses a part.

Keep the picture next to the claim it supports. Show the complete new block when omissions would hide the
contract or prevent the reader using it. State what changed and which adjacent scope stayed untouched.

## Evidence

Pair before and after observations. Rendered screenshots are strongest for visual changes; executed tests,
console output and reproducible demonstrations are stronger than source-only reasoning for behaviour.
Name the exact check and its actual result. Use `/skill:maestro-verify`; do not replace evidence with “tests added”.

## Merge danger

State whether the change is a one-way door (destructive or hard to reverse) or a two-way door (cheap to revert).
Describe the blast radius in concrete terms: affected consumers, persisted data, configuration or visible layout.
Name any remaining uncertainty and the condition that would expose it.
