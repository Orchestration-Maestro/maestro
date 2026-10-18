# Choose context at a phase boundary

Make this decision between coherent phases, not halfway through a line of reasoning. Mid-phase, continue or
delegate an independent bounded task. Context size is a heuristic about whether the next phase fits, not a fixed budget.

Work these branches in order:

1. **Continue** when the next phase needs this conversation as a primary source, or there is enough room for it.
   Keeping the original reasoning avoids loss from summarization.
2. **Use `/new`** when the current context is irrelevant to the next task. Do not discard reasoning the next phase needs.
3. **Use a fresh `subagent`** when the task is bounded enough to complete without steering. Leave the parent context intact
   and pass a [self-contained brief](subagents.md).
4. **Use `/compact`** when relevant context must carry forward but the next phase will not fit. Tell compaction what the
   next phase needs, keeping decisions, sources, current state and unresolved questions.

Use `/fork` for an exploratory branch when the current session should remain available, and `/tree` to navigate
session branches. If work travels to another directory, collaborator or harness, write a portable summary with
source pointers and decisions; do not assume the other context can read this conversation.

Every summary trades detail for room. Preserve primary sources and the reasons behind decisions, not only the
resulting action list. At the next phase, load the relevant spec and files instead of flooding context with unrelated work.
