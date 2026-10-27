# Skills

For creating, installing and invoking a skill in Maestro, start with the
[user guide](../skills.md). This page describes the native loader API.

`maestro-resources` discovers described instruction files on the native file
system. The caller supplies home, working and configuration roots;
the loader does not select application configuration.

## Supplied paths

```rust
use maestro_resources::{load_skills, format_skills_for_prompt, LoadSkillsOptions};

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
})?;
assert!(loaded.diagnostics.is_empty());
assert_eq!(loaded.skills.len(), 1);
assert!(format_skills_for_prompt(&loaded.skills).contains("<name>calendar</name>"));
std::fs::remove_dir_all(root)?;
# Ok(())
# }
```

With defaults enabled, user `agent_dir/skills` precedes project
`cwd/config_dir_name/skills`, followed by explicit paths in caller order.
Directory entries come in name order from the shared directory listing.
An unignored regular `SKILL.md` is attempted before other entries and stops descent,
even when its metadata is invalid. Otherwise the scan loads lowercase `.md` files
only at its root and descends into directories for entry files. Hidden entries,
`node_modules`, broken links and nonfiles are skipped; file and directory links
follow their targets. Resource discovery and package discovery share
[`skills::observed_type`](../../crates/maestro-resources/src/skills/discovery.rs).

Relative explicit paths normalize with `cwd` as the first operand, so a relative
`cwd` itself continues from the process working directory. On
Windows a drive-relative path on the drive of an absolute `cwd`, such as
`C:item.md`, continues from `cwd`; on another drive, such as `D:item.md`, it
continues from that drive's current directory, or from the
drive root when the drive has none and the process working directory is on another
drive. The loader reads the working directory and the drive directories at each
resolution whose operands leave a part open: a relative path or, on Windows, a
path rooted without a drive or relative to a drive. A call whose paths are all
anchored never reads them, and a change of working directory between two
resolutions shows in the second. When the working directory cannot be read, `load_skills` fails with that error if resolving the
project configuration directory or an explicit path needs it, for example beneath
a relative `cwd`; an absolute `cwd` on POSIX loads as usual.
`load_skills_from_dir` never fails for it: a scan makes each entry's path
relative to its directory with `cwd` as the first operand of both resolutions,
except that the directory itself is empty without resolving, and entries it
skips (hidden, `node_modules`, broken links) are not located. A directory
whose entries are open paths while the working directory cannot be read
ends with the skills gathered before the failure, and `load_skills` goes on with
its other roots. A
user skills root that needs the unreadable directory makes the classification of
an existing explicit path that path's warning. `~`, `~/suffix` and `~suffix`
expand against supplied `home`, including repeated slashes after the tilde, and an
expansion that is not absolute resolves against `cwd`. A bare `~` with an empty
`home` leaves the expansion empty, which names the working directory; a suffixed
form keeps the text after the tilde, so an empty `home` makes `~/` name the
working directory and `~child` name `child` below it, and a relative `home`
resolves below `cwd`. Joined
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
target, read relative to the link's directory, and scans again from the root;
other components keep their authored spelling and case, and a Windows link
target that names a drive or share loses its verbatim prefix. A link may be
expanded any number of times: `loop/loop/x`, with `loop` linked to its own
directory, resolves however often `loop` repeats. The walk fails when a link is
expanded again before the scan has read any component of what followed it at its
previous expansion, because the steps in between would repeat forever. When a
component cannot be inspected, a link target is missing, a link loops, a link
expands without the scan reaching what followed it or a relative path needs a
working directory that cannot be read, exact authored strings are the identity
keys, so distinct absolute spellings can produce name collisions.
Ordinary diagnostics precede all collisions.
With defaults disabled, explicit paths equal to a resolved user or project skills
root, or beginning with that root plus its separator, receive that scope, with user
precedence when roots overlap. The target spelling is not normalized for this comparison; a repeated
separator inside the root spelling therefore leaves the path temporary. Other
explicit paths are temporary; defaults-enabled explicit paths are always temporary.

## Validation and metadata

See [frontmatter](frontmatter.md) for the shared escape-decoding rule.

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

## Filesystem access

The loaders read the native file system directly. `read_entries` lists a directory
in name order without following links, `read_text` decodes a file as UTF-8 with
replacement characters, `metadata` follows links and `symlink_metadata` does not,
`real_path` resolves links, `current_directory` reads the process working
directory, and `drive_directories` reports the current directory of each Windows
drive from its `=X:` environment variable (none elsewhere). Browser builds have no
file system: listing fails with `Unsupported` and every read fails.

Both loaders take an explicit `cwd`. It is the first operand of each lexical
resolution against the caller: the project configuration directory, relative
explicit paths and the paths a scan makes relative to its directory for its
ignore matcher. Filesystem access is separate: the loader reads each scanned directory as authored and
each project or explicit path as the loader computed it, so `cwd` never
relocates an authored path it reads. The process directories
complete only what those operands leave open, and alone resolve native real
paths and the user and project roots that classify explicit paths. A working
directory that cannot be read is never replaced by an empty one: a resolution
that needs it fails with the working-directory error, which a scan turns into the end of the
affected directory, and one that the operands or a drive directory anchor does
not need it.
Each entered directory contributes `.gitignore`, `.ignore`, `.fdignore` in that
order to the shared [`maestro-pattern`](../libraries/package-pattern.md) rule owner.
Directory identities remain literal; rules see candidates relative to their own
directory. The deepest matching scope decides, and leaving a directory removes
its scope. Ignored directories are pruned before child rules are loaded.
Escaped leading exclamation marks stay literal. Invalid rules are inert and
ignore-file read failures are quiet.
Directory read and broken-link metadata failures are quiet; instruction reads,
metadata parsing and explicit path failures return path-bearing warnings.
