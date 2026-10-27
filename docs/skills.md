# Create and use skills

A skill gives Maestro instructions for a recurring task without putting the full
procedure into every prompt. You can ask Maestro to write one, or create a
`SKILL.md` yourself. This guide builds a small handoff checklist that needs no
scripts, dependencies or service credentials.

## Contents

1. [Create a project skill](#create-a-project-skill)
2. [Choose where to load it](#choose-where-to-load-it)
3. [Invoke a skill](#invoke-a-skill)
4. [Write the instructions and metadata](#write-the-instructions-and-metadata)
5. [Resolve warnings and review trust](#resolve-warnings-and-review-trust)

## Create a project skill

From your project directory, create a folder:

```sh
mkdir -p .maestro/skills/handoff-check
```

Save this complete file as `.maestro/skills/handoff-check/SKILL.md`:

```markdown
---
name: handoff-check
description: Prepare a concise handoff from supplied work notes. Use before handing unfinished work to another person or session.
---

# Handoff check

Use the notes supplied after this skill's command. If there are no notes, ask
for them. Do not inspect files or run commands unless the user asks you to.

Write four lines:
- Result: what the notes say works now.
- Unchecked: what has not been verified.
- Risk: one concrete remaining risk, or "none stated".
- Next: one action the next person can start immediately.

Do not invent successful tests or completed work.
```

Start Maestro in that project and enter:

```text
/skill:handoff-check Parser updated; unit tests passed; large files unchecked.
```

The command inserts the instructions and your notes into the model's input. It
does not run a shell script. The model produces the handoff, so wording can vary.
A session still needs a configured model; see [model selection](selection.md).
After editing an already loaded skill's metadata, use `/reload` or restart to
refresh its name and description. Invocation reads the current file body.

For an explicit, non-interactive run using your configured model:

```sh
maestro --no-skills --skill .maestro/skills/handoff-check --no-session -p \
  '/skill:handoff-check Parser updated; unit tests passed; large files unchecked.'
```

This makes a model request and may incur your provider's normal charges. It is
not an offline validator. `--no-session` prevents saving this run's session.

## Choose where to load it

Maestro discovers skills at startup and on resource reload. The user
configuration root defaults to `~/.maestro/agent`; setting
`MAESTRO_CODING_AGENT_DIR` changes that root, not the shared `.agents` roots.
See [configuration paths](application/configuration.md).

| Source | Location or configuration |
| --- | --- |
| User | `~/.maestro/agent/skills/` (or `<configuration-root>/skills/`) |
| Shared user | `~/.agents/skills/` |
| Project | `.maestro/skills/` in the session working directory |
| Shared project and ancestors | `.agents/skills/` in the working directory and each ancestor through the nearest directory containing `.git`, inclusive; without that marker, through the filesystem root |
| Package | A package's conventional `skills/` directory, or its `package.json` `maestro.skills` entries |
| Settings | The top-level `skills` list in user or project `settings.json` |
| Command line | Repeatable `--skill <file-or-directory>` paths |

Only shared `.agents/skills/` roots use the ancestor walk. Maestro does not walk
ancestors for `.maestro/skills/`. The home shared root is user-scoped, even when
it lies on the project walk.

### Discovery rules

Conventional `.maestro` skill roots accept loose root `.md` files as individual
skills. Shared `.agents/skills/` roots ignore loose Markdown and require
skill directories containing `SKILL.md`. A directory scan searches recursively
for `SKILL.md`; once it finds an admitted entry file it stops descending into
that directory, including when that file has invalid metadata. Put sibling
skills beside one another, not beneath another skill's entry file.

Hidden entries, `node_modules` and broken links are skipped. Symbolic links to
files and directories follow their targets. `.gitignore`, `.ignore` and
`.fdignore` can exclude entries. The [loader reference](resources/skills.md)
gives the filesystem and ignore-rule details.

### Settings and existing skill collections

Use the same skill directory from another workspace by pointing at it; copying
its contents is unnecessary. For example, in `~/.maestro/agent/settings.json`:

```json
{
  "skills": ["~/work/team-guidance/skills"]
}
```

Project settings live at `.maestro/settings.json`. Their relative paths start
at `.maestro/`, not the shell's working directory. If an existing collection is
at `<project>/guidance/skills`, use:

```json
{
  "skills": ["../guidance/skills"]
}
```

User-setting relative paths start at the user configuration root. Explicit CLI
paths instead start at the working directory. Each entry can name a Markdown
file or a directory. Review reused collections for assumptions about tool names,
paths and permissions. Loading them does not install their dependencies or
translate instructions written for another environment.

### Packages and selection

The shipped [development skills](packages/maestro-skills.md) are one collection,
not the skill format itself. Follow that page to load a local checkout. To author
a package, use [package discovery](packages.md#conventional-resource-discovery): package content goes
in `skills/`, or a `maestro.skills` list selects its paths. For example:

```json
{
  "name": "team-guidance",
  "version": "1.0.0",
  "maestro": {
    "skills": ["skills/handoff-check"]
  }
}
```

The package contains `skills/handoff-check/SKILL.md` with the file above.
Register the package through the `packages` setting; putting a directory in
`skills` loads instruction resources, not a package manifest.

For configured resources, project local paths precede project automatic roots,
then user local paths, user automatic roots and package resources. Distinct
files with the same skill name keep the first loaded skill and report a
collision. Additional CLI paths are appended after configured resources; they
are not a same-name override. For exact selection and exclusion rules, see
[configured resource resolution](packages.md#configured-resource-resolution).

### Explicit paths without discovery

Use `--no-skills` to leave out discovered and settings-selected skills while
keeping the paths you explicitly supply:

```sh
maestro --no-skills --skill .maestro/skills/handoff-check
```

Repeat `--skill` for more paths. To load exactly one file instead of scanning its
folder, give `.maestro/skills/handoff-check/SKILL.md`. A missing explicit path
produces a diagnostic. This flag disables skill discovery, not extensions,
context files or the other resource kinds.

## Invoke a skill

When the `read` tool is enabled, the system prompt lists each visible skill's
name, description and file location. The full instructions stay on disk until
the model reads them. A matching description helps the model choose the skill;
it does not guarantee the model will read it. This is progressive disclosure:
keep the entry file focused, with larger references in separate files.

`/skill:handoff-check` explicitly expands the selected file without relying on
the model to choose it. Maestro removes its frontmatter, wraps its body with
the skill name, location and reference directory, and places text after the
first space after that block. Arguments are ordinary text, not shell arguments
or `$1` substitutions. For example:

```text
/skill:handoff-check Tests passed locally; deployment has not been checked.
```

Unknown skill names remain ordinary prompt text. If the file cannot be read,
Maestro reports a skill expansion error and leaves the original text intact.

`enableSkillCommands` defaults to `true`. Change it in `settings.json`, or use
`/settings` in an interactive session:

```json
{
  "enableSkillCommands": false
}
```

This controls interactive skill command completion. It does not unload skills,
hide them from the model or prevent a manually typed `/skill:name` from
expanding. To omit a skill from the advertised model list while keeping explicit
invocation, set `disable-model-invocation: true` in its frontmatter. That is a
selection aid, not an access restriction.

## Write the instructions and metadata

Use one directory per skill. Only `SKILL.md` is required; supporting directories
are conventions you choose, not special loader inputs:

```text
handoff-check/
  SKILL.md
  references/
    checklist.md
  scripts/
    summarize.sh
  assets/
    handoff-template.txt
```

The introductory example needs none of these extra files. If you add them,
explain when to read a reference or run a helper, its dependencies and expected
inputs. Relative references such as `references/checklist.md` start at the skill
directory. Tool commands should use the resolved absolute path; the session's
working directory does not change to the skill directory automatically.

### Frontmatter fields

| Field | Guidance and current behavior |
| --- | --- |
| `name` | Use 1 to 64 lowercase ASCII letters, digits or hyphens, matching the containing directory. No leading, trailing or repeated hyphens. A missing, null or empty name falls back to the directory basename. |
| `description` | Required nonblank string, at most 1024 characters for warning-free loading. Say what the skill does and when to choose it. |
| `disable-model-invocation` | Optional; only boolean `true` omits it from the model's advertised skill list. Defaults to `false`. |
| `license` | Optional author metadata, such as `MIT` or a bundled license path; the loader does not enforce it. |
| `compatibility` | Optional author metadata describing required tools or environment. Keep it within the Agent Skills convention of 500 characters; Maestro does not validate this limit. |
| `metadata` | Optional mapping for author information; it does not configure Maestro. |
| `allowed-tools` | Optional Agent Skills metadata. Maestro does not use this field to authorize or restrict tools. |

Unknown fields do not change loading. The [Agent Skills format](https://agentskills.io/specification)
defines the portable conventions; Maestro warns about many violations rather
than enforcing all of them.

`handoff-check`, `release-notes` and `log-summary` are valid names. `Handoff`,
`-handoff` and `handoff--check` produce warnings. Prefer a task-specific
description such as the example's over `Helps with work`: the description is
what the model sees before it reads the procedure.

## Resolve warnings and review trust

Missing or whitespace-only descriptions omit a skill. Invalid YAML, or a
nonstring name or description, also prevents loading and produces a diagnostic.
Name spelling, directory-name mismatch and excessive name or description length
produce warnings but retain the skill. Fix warnings rather than depending on
that leniency. [Frontmatter parsing](resources/frontmatter.md) explains accepted
YAML and parser failures.

A repeated real file reached through another path is silently deduplicated.
Different files sharing a name produce a collision diagnostic identifying the
winner and loser. Rename one skill or remove the unwanted source; do not assume
a later CLI path wins. Review the startup resource diagnostics if a skill does
not appear. A missing default directory is quiet, unlike a missing explicit
path.

Read the instructions, referenced material and helper code before loading an
unfamiliar skill or package. Discovery reads metadata; it does not execute skill
helpers. Once invoked, however, instructions can ask the model to read private
files, run programs, change the project or send data outside it using available
tools. Neither `disable-model-invocation` nor `allowed-tools` provides a sandbox.
Do not put secrets in skill text or trust a skill because it loaded without
warnings.
