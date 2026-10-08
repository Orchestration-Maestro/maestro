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
let paths = [directory.to_string_lossy().into_owned()];
let root_text = root.to_str().ok_or("non-UTF-8 fixture path")?;
let loaded = load_skills(LoadSkillsOptions {
    cwd: root_text,
    home: root_text,
    agent_dir: root_text,
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

Relative explicit paths normalize with `cwd` as the first operand, so a relative
`cwd` itself continues from the process working directory the adapter reports. On
Windows a drive-relative path on the drive of an absolute `cwd`, such as
`C:item.md`, continues from `cwd`; on another drive, such as `D:item.md`, it
continues from the directory the adapter reports for that drive, or from the
drive root when none is reported and the process working directory is on another
drive. `~`, `~/suffix` and `~suffix` expand against supplied `home`, including
repeated slashes after the tilde. Joined
user configuration and scanned child paths concatenate their parts before lexical
normalization; later rooted parts do not replace the earlier root. Project
configuration paths instead resolve against `cwd`; an absolute configuration
directory replaces the working-directory prefix.
Authored path fields and loader inputs are strings; only filesystem calls use
native paths. Already-absolute explicit file spelling is preserved, including
repeated separators and dot components. Aliases of
retained files are silently deduplicated by real path; duplicate names keep the
first discovery. Only winning paths are remembered, so repeated losing paths
produce repeated collisions. A native real path first folds `.` and `..`
lexically, resolving a relative path against the process working directory, so
`link/..` names the directory holding `link`. It then replaces each link by its
target, read relative to the link's directory; other components keep their
authored spelling and case, and a Windows link target that names a drive or share
loses its verbatim prefix. A link may be expanded any number of times:
`loop/loop/x`, with `loop` linked to its own directory, resolves however often
`loop` repeats. The exception is a link met again while everything that followed
it at its previous expansion still ends the path after it; that repeat cannot
progress. When a component cannot be inspected, a link target is missing, a link
loops, a link's expansion cannot progress or a relative path needs a working
directory that cannot be read, exact authored strings are the identity keys, so
distinct absolute spellings can produce name collisions.
Ordinary diagnostics precede all collisions.
With defaults disabled, explicit paths equal to a resolved user or project skills
root, or beginning with that root plus its separator, receive that scope, with user
precedence when roots overlap. The target spelling is not normalized for this comparison; a repeated
separator inside the root spelling therefore leaves the path temporary. Other
explicit paths are temporary; defaults-enabled explicit paths are always temporary.

## Validation and metadata

Description validation precedes name validation. Missing or blank descriptions
omit the skill after collecting name warnings. Other validation warnings retain
it: overlong descriptions, parent-name mismatch, overlong names, invalid name
characters, edge hyphens and consecutive hyphens. Lengths count complete Unicode
characters. Missing, null or empty names use the containing directory basename,
including the literal names `.` and `..` when they end the authored parent
directory. The base directory retains that authored spelling.
Non-null nonstring names/descriptions produce one path-bearing typed failure.
Only boolean `disable-model-invocation: true` hides a skill from the prompt.
Descriptions retain authored newlines and whitespace. Prompt XML escapes each
field, preserves visible caller order and uses the instruction file location.

Metadata parsing normalizes line endings, retains typed scalars, aliases and
ordered collections, rejects duplicate equal YAML keys and multiple documents,
and reports native parser causes with coordinates. Hexadecimal and octal integers
require unsigned digits after `0x` or `0o`; an inner sign remains text, even with
an explicit integer tag. Integers of any width round to the numeric scalar's
floating-point representation, overflowing to infinity rather than becoming text.
Unknown skill fields are ignored by discovery. Extra YAML tags `binary`, `set`, `timestamp`, `omap` and
`pairs` resolve as underlying core-schema data, not foreign runtime objects.
Collection-valued mapping-key pairs are discarded while ordinary fields survive.
Library/process warnings are not printed or returned as skill diagnostics.

## Filesystem adapters

`ResourceOperations` supplies ordered `read_dir`, lossy UTF-8 `read_file`,
link-following `metadata`, independent `exists` observations, fallible
`canonicalize`, the process working directory (`current_directory`) and the
current directory of each Windows drive (`drive_directories`; the native adapter
reads each drive's `=X:` environment variable and reports none elsewhere). The
same public loaders accept replacement adapters without caller algorithm changes.
Native operations are available outside `wasm32`; browser callers supply their
own adapter.

Both loaders take an explicit `cwd`. It is the first operand of each lexical
resolution against the caller: the project configuration directory, relative
explicit paths and the coordinates a scan's ignore matcher compares. Filesystem
access is separate: the adapter receives each scanned directory as authored and
each project or explicit path as the loader computed it, so `cwd` never
relocates an authored path it reads. The process directories the adapter reports
complete only what those operands leave open, and alone resolve native real
paths and the user and project roots that classify explicit paths. A working
directory the adapter cannot read anchors nothing: a path that no operand or
drive directory anchors stays relative, or on Windows rooted without a drive.
Each scan shares one case-insensitive ignore matcher. Its root and candidates are
resolved from `cwd`; rule prefixes use their lexical relative paths.
Rules are appended in
`.gitignore`, `.ignore`, `.fdignore` order with directory prefixes and compiled
when rules change. Later rules may reopen files, but child negation cannot reopen
an excluded parent. Escaped leading exclamation marks stay literal. Invalid
patterns and ignore read/compile failures follow the quiet partial-result route.
Directory read and broken-link metadata failures are quiet; instruction reads,
metadata parsing and explicit path failures return path-bearing warnings.
