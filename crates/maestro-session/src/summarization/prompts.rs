//! Instructions used by the summary completion requests.
/// Structured summary instruction.
pub(super) const SUMMARIZATION_PROMPT: &str = r#"The messages above are a conversation to summarize. Write a structured context checkpoint that lets another LLM pick the work back up.

Follow this format EXACTLY:

## Goal
[What the user wants to get done. List several items when the session covers more than one job.]

## Constraints & Preferences
- [Limits, wishes or requirements the user stated]
- [Or "(none)" when the user stated none]

## Progress
### Done
- [x] [Tasks and changes already finished]

### In Progress
- [ ] [Work underway now]

### Blocked
- [Anything stopping progress, if present]

## Key Decisions
- **[Decision]**: [Short reason]

## Next Steps
1. [What should happen next, in order]

## Critical Context
- [Facts, samples or pointers needed to carry on]
- [Or "(none)" when nothing applies]

Keep every section short. Quote error messages, function names and file paths exactly as they appear."#;
/// Structured summary instruction.
pub(super) const UPDATE_SUMMARIZATION_PROMPT: &str = r#"The messages above are NEW conversation messages. Fold them into the existing summary given in <previous-summary> tags.

Revise the existing structured summary with what the new messages add. RULES:
- KEEP everything the previous summary already says
- ADD whatever the new messages contribute: progress, decisions, context
- REVISE the Progress section: move an item from "In Progress" to "Done" once it is finished
- REVISE "Next Steps" to reflect what was achieved
- KEEP error messages, function names and file paths exactly as they appear
- You may drop anything that no longer matters

Follow this format EXACTLY:

## Goal
[Keep the existing goals and add more if the task widened]

## Constraints & Preferences
- [Keep the existing entries and add any newly found]

## Progress
### Done
- [x] [List the items finished before AND those finished now]

### In Progress
- [ ] [Work underway now - revise from the latest progress]

### Blocked
- [Blockers that remain - drop any that are resolved]

## Key Decisions
- **[Decision]**: [Short reason] (keep every earlier one, add the new)

## Next Steps
1. [Revise from the current state]

## Critical Context
- [Keep the important context and add more where needed]

Keep every section short. Quote error messages, function names and file paths exactly as they appear."#;
/// Structured summary instruction.
pub(super) const TURN_PREFIX_SUMMARIZATION_PROMPT: &str = r"This is the PREFIX of a turn that grew too large to keep whole. The SUFFIX, the most recent work, stays in context.

Summarize the prefix so the retained suffix makes sense:

## Original Request
[What the user asked for in this turn]

## Early Progress
- [Main decisions and work completed in the prefix]

## Context for Suffix
- [What a reader needs to follow the retained recent work]

Be brief. Cover only what explains the retained suffix.";
