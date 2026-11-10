//! The two prompt templates the analysis children receive.

/// The instruction that opens the analysis prompt.
const ANALYSIS: &str = "You are reviewing session transcripts for recurring user instructions that could be automated.\n\n";
/// The instruction that opens the aggregation prompt.
const AGGREGATION: &str =
    "You are combining the pattern analyses written to several summary files.\n\n";
/// The analysis instructions that follow any existing instruction files.
const READING: &str = "HOW TO READ THE TRANSCRIPT:
The transcript file is large, so read it in 1000-line chunks with the offset and limit parameters:
1. Start with limit=1000 (lines 1-1000)
2. Next read with offset=1001, limit=1000 (lines 1001-2000)
3. Keep adding 1000 to offset until the end of the file is reached
4. Analyse and write the summary only once the ENTIRE file has been read

ANALYSIS TASK:
Find patterns where the user keeps giving similar instructions. Each could turn into:
- AGENTS.md entries: style rules for code, behavioral guidance and project conventions
- Skills: workflows with several steps that rely on outside tools (browser, search, APIs)
- Prompt templates: prompts that can be reused for frequent tasks

Check every pattern against the current AGENTS.md content to decide whether it is NEW or EXISTING.

REQUIRED OUTPUT FORMAT (strict):
Produce a file laid out exactly like this, with --- between patterns.

PATTERN: <brief name>
STATUS: NEW | EXISTING
TYPE: agents-md | skill | prompt-template
FREQUENCY: <how many times it occurred>
EVIDENCE:
- \"<verbatim quote 1>\"
- \"<verbatim quote 2>\"
- \"<verbatim quote 3>\"
DRAFT:
<proposed text for the AGENTS.md entry, SKILL.md or prompt template>
---

Rules:
- Report only patterns seen at least 2 times
- STATUS is NEW when AGENTS.md lacks the pattern, EXISTING when AGENTS.md already covers it
- EVIDENCE must quote the transcripts exactly
- DRAFT must be content that can be used as it stands
- When no pattern is found, write \"NO PATTERNS FOUND\"
- Add no text outside this format";
/// The merging instructions that follow the listed summary files.
const MERGING: &str = "STEP 3: Produce one consolidated final summary that:
1. Merges duplicates (the same pattern appearing in several files)
2. Ranks patterns by their combined frequency over all files
3. Orders by status (NEW before EXISTING), then by type
4. Keeps the best and most complete DRAFT for each distinct pattern
5. Re-checks STATUS against AGENTS.md content (a summary may say NEW for something AGENTS.md already holds)

REQUIRED OUTPUT FORMAT (strict):
Lay the final summary out like this:

# NEW PATTERNS (not yet in AGENTS.md)

## AGENTS.MD: <pattern name>
Total Frequency: <sum across all files>
Evidence:
- \"<best quotes>\"
Draft:
<merged draft>

## SKILL: <pattern name>
...

## PROMPT-TEMPLATE: <pattern name>
...

---

# EXISTING PATTERNS (already covered by AGENTS.md, listed for reference)

## <pattern name>
Total Frequency: <N>
Already covered by: <quote the relevant AGENTS.md section>

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
        "{ANALYSIS}STEP 1: Open the existing AGENTS.md file(s) to learn which patterns they already hold:\n{}\n\nSTEP 2: {READING}",
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
        "{AGGREGATION}STEP 1: Open the existing AGENTS.md file(s) to learn which patterns they already hold:\n{known}\n\nSTEP 2: Read EVERY summary file listed here:\n{listed}\n\n{MERGING}\n\nWrite the final summary to {final_path}"
    )
}
