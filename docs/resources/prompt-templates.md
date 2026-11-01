# Prompt templates

Save a repeated instruction as a Markdown file, then send it with `/name`.
The filename supplies the command: `handoff.md` becomes `/handoff`.

## Write and invoke a template

1. From your project directory, create the prompt folder:

   ```sh
   mkdir -p .maestro/prompts
   cat > .maestro/prompts/handoff.md <<'EOF'
   ---
   description: Prepare a focused handoff note
   argument-hint: "<area> [details...]"
   ---
   Prepare a handoff for $1.
   Include: ${@:2}
   EOF
   ```

2. Start `maestro`. Type `/hand` to find `/handoff` in command completion.
   The menu shows `<area> [details...]` beside the description. The hint is
   display text, not argument validation.
3. Send this in the editor:

   ```text
   /handoff "billing service" 'retry policy' metrics
   ```

   Maestro sends this expanded prompt to the model:

   ```text
   Prepare a handoff for billing service.
   Include: retry policy metrics
   ```

4. After adding or editing a template while Maestro is running, send `/reload`
   when the session is idle. Completion and subsequent invocations use the
   reloaded templates. Reload also refreshes other resources; see
   [application resources](../application/resources.md#resource-loader).

Without a description, the menu uses the first nonblank body line, shortened
at 60 characters. Keep descriptions short enough to distinguish commands.
Quote frontmatter values containing punctuation such as a colon.

## Choose where it loads

| Source | Location or configuration |
| --- | --- |
| Your templates | `~/.maestro/agent/prompts/*.md` by default; with `MAESTRO_CODING_AGENT_DIR`, use `<agent-dir>/prompts/*.md` |
| Project templates | `<working-directory>/.maestro/prompts/*.md` |
| Packages | A configured package's `prompts/` directory, or its `package.json` field `maestro.prompts` |
| Settings | `prompts` array in `~/.maestro/agent/settings.json` (or the overridden agent directory) and `<working-directory>/.maestro/settings.json` |
| One launch | Repeat `--prompt-template <path>` for each file or directory |

Automatic user and project prompt discovery reads only immediate `.md` files.
A nested filename does not create a command namespace: a loaded
`team/deployment.md` is still `/deployment`.

To try a nested template, copy the example under a different command name:

```sh
mkdir -p .maestro/prompts/team
cp .maestro/prompts/handoff.md .maestro/prompts/team/deployment.md
```

Add this entry to project settings, keeping any other settings you already have.
Paths are relative to the settings scope's configuration directory, not the
shell's working directory:

```json
{
  "prompts": ["prompts/team"]
}
```

In project settings this selects `.maestro/prompts/team`. Configured document
folders recurse, so templates below it load too. An explicit file such as
`"prompts/team/deployment.md"` selects only that file. The resource loader reads
both scopes' lists separately; a project list does not remove the user list.
See [settings](../settings.md) for file storage and other preference merging.

To share the same files in a local package, use this package-root `package.json`
and add the package to your [package configuration](../packages.md):

```json
{
  "name": "team-prompts",
  "maestro": { "prompts": ["prompts/team"] }
}
```

Package prompt discovery also recurses. Paths in `maestro.prompts` are relative
to the package root. Without a `maestro` resource manifest, the conventional
`prompts/` directory supplies package templates. See
[package-root manifests](../packages.md#package-root-manifests).

For a one-off file, run from the project directory:

```sh
maestro --prompt-template .maestro/prompts/team/deployment.md
```

CLI paths are relative to the working directory. A CLI directory loads its
immediate `.md` files, not nested folders. To skip automatic, settings and package
prompt discovery while keeping explicitly named CLI templates:

```sh
maestro --no-prompt-templates --prompt-template .maestro/prompts/handoff.md
```

You can repeat `--prompt-template`; `--no-prompt-templates` alone loads no
prompt templates. Avoid duplicate basenames across sources: the application
keeps the first loaded name and reports later collisions. The
[resource loader](../application/resources.md#resource-loader) describes ordering.

## Select arguments

Arguments are text, not shell expressions. Single or double quotes group words;
the quote delimiters disappear. Backslashes stay literal, empty quoted arguments
disappear, and no shell command runs during substitution.

For `/handoff "billing service" 'retry policy' metrics`, the selections are:

| Marker in the body | Inserted text |
| --- | --- |
| `$1` | `billing service` |
| `$2` | `retry policy` |
| `$@` or `$ARGUMENTS` | `billing service retry policy metrics` |
| `${@:2}` | `retry policy metrics` |
| `${@:2:1}` | `retry policy` |

Positions start at one. `${@:N}` takes everything from position N;
`${@:N:L}` takes at most L arguments starting there. Missing arguments insert
empty text. All-argument and slice markers join selected arguments with spaces;
they do not preserve the original quoting. Marker-like text inside an argument
is inserted literally, without another substitution pass.

## Parser and loader reference

`maestro_resources::prompt_templates` loads supplied Markdown templates and expands
leading slash commands. It does not execute a shell or select application settings.

```rust
use maestro_resources::{PromptTemplate, SyntheticSourceOptions,
    create_synthetic_source_info, expand_prompt_template};

let templates = vec![PromptTemplate {
    name: "review".into(),
    description: "Review selected paths".into(),
    argument_hint: Some("<paths>".into()),
    content: "Review $1; remaining: ${@:2}; all: $@".into(),
    file_path: "/prompts/review.md".into(),
    source_info: create_synthetic_source_info("/prompts/review.md".into(),
        SyntheticSourceOptions {
            source: "local".into(), scope: None, origin: None,
            base_dir: Some("/prompts".into()),
        }),
}];
assert_eq!(expand_prompt_template("/review src tests", &templates),
    "Review src; remaining: tests; all: src tests");
```

Expansion selects the first exact name after a leading `/`, ending the name at
the first whitespace scalar (space, tab, newline or any other). The name must
contain at least one non-whitespace character; `/` and `/ x` stay unchanged.
Argument parsing
removes single/double quote delimiters; only unquoted whitespace scalars separate
arguments. Empty quoted fragments disappear and
backslashes remain literal. Original `$1`, `$2`, `$ARGUMENTS`, `$@`, `${@:N}` and
`${@:N:L}` markers select arguments; inserted text is never rescanned. Missing
positional arguments insert empty text. Slice start zero selects from the first
argument. Selected arguments join with one space, preserving their own whitespace.
Unknown commands remain unchanged.

`load_prompt_templates` accepts caller-selected roots, home spelling and ordered
explicit paths through `LoadPromptTemplatesOptions` and reads the native file system.
With defaults enabled, it scans user then project directories before explicit
paths. Disabling defaults leaves explicit paths active. Directory scans are
shallow, keep the listing's name order and accept exact `.md` suffixes, including hidden
files and file symlinks. Repeated paths and names remain repeated. Paths use
[the lexical path owner](../../crates/maestro-path/src/lib.rs); provenance uses authored spelling, not link targets.

Metadata uses the shared [frontmatter parser](frontmatter.md). Only `description`
and `argument-hint` are selected: absent/null fields are absent, strings retain
text, and other selected types silently omit the file. Empty descriptions fall
back to the first nonblank body line, retaining its whitespace and at most 60
Unicode scalars, followed by `...` only when longer. Nonempty authored descriptions
are not truncated. Empty hints are omitted.

File read/metadata parsing/selected-field failures omit only that file. A failure
of the initial link-kind metadata check skips that link; directory-read or
source-classification failure (including a link's second metadata check) ends
only that scan, retaining earlier records. Explicit-path metadata/source
failures skip that path. Project-root and explicit-path resolution errors outside
those catches propagate when the process directory is needed and unavailable.
