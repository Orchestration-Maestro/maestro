# Skills

`maestro-resources` discovers described instruction files through caller-supplied
filesystem operations. The caller supplies home, working and configuration roots;
the loader does not select application configuration.

## Supplied paths

```rust
use maestro_resources::{load_skills, format_skills_for_prompt, LoadSkillsOptions,
    NativeResourceOperations};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let root = std::env::temp_dir().join(format!("maestro-skills-guide-{}", std::process::id()));
let directory = root.join("calendar");
std::fs::create_dir_all(&directory)?;
std::fs::write(directory.join("SKILL.md"),
    "---\nname: calendar\ndescription: Schedule meetings\n---\nRead the calendar.")?;
let paths = [directory];
let loaded = load_skills(LoadSkillsOptions {
    cwd: &root,
    home: &root,
    agent_dir: &root,
    config_dir_name: ".maestro",
    skill_paths: &paths,
    include_defaults: false,
}, &NativeResourceOperations);
assert!(loaded.diagnostics.is_empty());
assert_eq!(loaded.skills.len(), 1);
assert!(format_skills_for_prompt(&loaded.skills).contains("<name>calendar</name>"));
std::fs::remove_dir_all(root)?;
# Ok(())
# }
```

With defaults enabled, user `agent_dir/skills` precedes project
`cwd/config_dir_name/skills`, followed by explicit paths in caller order.
Directory-entry order belongs to the filesystem adapter; the loader does not sort.
An unignored regular `SKILL.md` is attempted before other entries and stops descent,
even when its metadata is invalid. Otherwise the scan loads lowercase `.md` files
only at its root and descends into directories for entry files. Hidden entries,
`node_modules`, broken links and nonfiles are skipped; file and directory links
follow their targets.

Relative paths normalize against `cwd`. `~`, `~/suffix` and `~suffix` expand
against supplied `home`. Already-absolute input spelling is preserved. Canonical
aliases of retained files are silently deduplicated; duplicate names keep the
first discovery. Only winning paths are remembered, so repeated losing paths
produce repeated collisions. Ordinary diagnostics precede all collisions.
With defaults disabled, explicit paths under the user or project skills roots
receive that scope, with user precedence when roots overlap. Other explicit paths
are temporary; defaults-enabled explicit paths are always temporary.

## Validation and metadata

Description validation precedes name validation. Missing or blank descriptions
omit the skill after collecting name warnings. Other validation warnings retain
it: overlong descriptions, parent-name mismatch, overlong names, invalid name
characters, edge hyphens and consecutive hyphens. Lengths count complete Unicode
characters. Missing, null or empty names use the containing directory basename.
Non-null nonstring names/descriptions produce one path-bearing typed failure.
Only boolean `disable-model-invocation: true` hides a skill from the prompt.
Descriptions retain authored newlines and whitespace. Prompt XML escapes each
field, preserves visible caller order and uses the instruction file location.

Metadata parsing normalizes line endings, retains typed scalars, aliases and
ordered collections, rejects duplicate equal YAML keys and multiple documents,
and reports native parser causes with coordinates. Unknown skill fields are
ignored by discovery. Extra YAML tags `binary`, `set`, `timestamp`, `omap` and
`pairs` resolve as underlying core-schema data, not foreign runtime objects.
Collection-valued mapping-key pairs are discarded while ordinary fields survive.
Library/process warnings are not printed or returned as skill diagnostics.

## Filesystem adapters

`ResourceOperations` supplies ordered `read_dir`, lossy UTF-8 `read_file`,
link-following `metadata`, independent `exists` observations and fallible
`canonicalize`. The same public loaders accept replacement adapters without
caller algorithm changes. Native operations are available outside `wasm32`;
browser callers supply their own adapter.

Each scan shares one case-insensitive ignore matcher. Rules are appended in
`.gitignore`, `.ignore`, `.fdignore` order with directory prefixes and compiled
when rules change. Later rules may reopen files, but child negation cannot reopen
an excluded parent. Escaped leading exclamation marks stay literal. Invalid
patterns and ignore read/compile failures follow the quiet partial-result route.
Directory read and broken-link metadata failures are quiet; instruction reads,
metadata parsing and explicit path failures return path-bearing warnings.
