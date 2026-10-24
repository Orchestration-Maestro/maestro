//! The two prompt templates the analysis children receive.

/// The instruction that opens the analysis prompt.
const ANALYSIS: &str = "You are analyzing session transcripts to identify recurring user instructions that could be automated.\n\n";
/// The instruction that opens the aggregation prompt.
const AGGREGATION: &str =
    "You are aggregating pattern analysis results from multiple summary files.\n\n";
/// The analysis instructions that follow any existing instruction files.
const READING: &str = "READING THE TRANSCRIPT:
The transcript file is large. Read it in chunks of 1000 lines using offset/limit parameters:
1. First: read with limit=1000 (lines 1-1000)
2. Then: read with offset=1001, limit=1000 (lines 1001-2000)
3. Continue incrementing offset by 1000 until you reach the end
4. Only after reading the ENTIRE file, perform the analysis and write the summary

ANALYSIS TASK:
Look for patterns where the user repeatedly gives similar instructions. These could become:
- AGENTS.md entries: coding style rules, behavior guidelines, project conventions
- Skills: multi-step workflows with external tools (search, browser, APIs)
- Prompt templates: reusable prompts for common tasks

Compare each pattern against the existing AGENTS.md content to determine if it's NEW or EXISTING.

OUTPUT FORMAT (strict):
Write a file with exactly this structure. Use --- as separator between patterns.

PATTERN: <short descriptive name>
STATUS: NEW | EXISTING
TYPE: agents-md | skill | prompt-template
FREQUENCY: <number of times observed>
EVIDENCE:
- \"<exact quote 1>\"
- \"<exact quote 2>\"
- \"<exact quote 3>\"
DRAFT:
<proposed content for AGENTS.md entry, SKILL.md, or prompt template>
---

Rules:
- Only include patterns that appear 2+ times
- STATUS is NEW if not in AGENTS.md, EXISTING if already covered
- EVIDENCE must contain exact quotes from the transcripts
- DRAFT must be ready-to-use content
- If no patterns found, write \"NO PATTERNS FOUND\"
- Do not include any other text outside this format";
/// The merging instructions that follow the listed summary files.
const MERGING: &str = "STEP 3: Create a consolidated final summary that:
1. Merges duplicate patterns (same pattern found in multiple files)
2. Ranks patterns by total frequency across all files
3. Groups by status (NEW first, then EXISTING) and type
4. Provides the best/most complete DRAFT for each unique pattern
5. Verify STATUS against AGENTS.md content (pattern may be marked NEW in summaries but actually exists)

OUTPUT FORMAT (strict):
Write the final summary with this structure:

# NEW PATTERNS (not yet in AGENTS.md)

## AGENTS.MD: <pattern name>
Total Frequency: <sum across all files>
Evidence:
- \"<best quotes>\"
Draft:
<consolidated draft>

## SKILL: <pattern name>
...

## PROMPT-TEMPLATE: <pattern name>
...

---

# EXISTING PATTERNS (already in AGENTS.md, for reference)

## <pattern name>
Total Frequency: <N>
Already covered by: <quote relevant section from AGENTS.md>

---

# SUMMARY
- New patterns to add: <N>
- Already covered: <N>
- Top 3 new patterns by frequency: <list>";

/// The analysis prompt, with the instruction files the child should read first.
pub(super) fn analysis(agents: &[String]) -> String {
    if agents.is_empty() {
        return format!("{ANALYSIS}{READING}");
    }
    format!(
        "{ANALYSIS}STEP 1: Read the existing AGENTS.md file(s) to see what's already encoded:\n{}\n\nSTEP 2: {READING}",
        agents.join("\n")
    )
}

/// The aggregation prompt, with the instruction files and the summary paths.
pub(super) fn aggregation(agents: &[String], summaries: &[String], final_path: &str) -> String {
    let known = if agents.is_empty() {
        "(no AGENTS.md files found)".to_owned()
    } else {
        agents.join("\n")
    };
    let listed = summaries.join("\n");
    format!(
        "{AGGREGATION}STEP 1: Read the existing AGENTS.md file(s) to understand what patterns are already encoded:\n{known}\n\nSTEP 2: Read ALL of the following summary files:\n{listed}\n\n{MERGING}\n\nWrite the final summary to {final_path}"
    )
}
