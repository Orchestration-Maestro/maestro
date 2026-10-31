# Application resources

`maestro_app::resources` discovers the resources of one working directory and
assembles the system prompt. `maestro_app::runtime::agent_session_services`
binds credentials, settings, the model catalog and those resources to one
session directory. Both are native-only. The SDK root exports the loader,
its options and records, `load_project_context_files` and the services.

## Resource loader

`DefaultResourceLoader::new` takes the working directory, the agent directory,
an optional shared `SettingsManager` and `ResourceLoaderOptions`. The options
require the caller's `LocalSet` and the module loader that turns a resolved
extension path into a factory. Everything else is optional: the event bus,
base package sources, additional paths per kind, inline extension factories,
disabled kinds, the system
prompt, appended texts, one override per result and the offline selection its
package manager uses. Every result is empty until the first reload.
Getters return the current snapshot; a later reload replaces snapshots and
never changes one already handed out.

A reload runs in this order:

1. Wait for queued settings writes, then reload settings from storage.
2. Resolve base package sources and run their factories first. Apply their
   load-time inputs and acquire contributed sources. Resolve configured packages
   and automatic resources, then additional extension sources as temporary sources.
3. Record the provenance of every resolved resource, enabled or not, in an
   index built afresh for this reload: the first entry per path wins. An
   enabled automatic or package skill directory that holds `SKILL.md` is
   loaded through that file. Additional extension and skill
   sources record `cli` provenance.
4. Load additional extensions before configured ones, then each inline factory
   as `<inline:N>` on the same runtime. Base factories are not run again; their
   registrations retain their ordinary package positions. Tool and flag names that a later
   extension path registers again are reported as conflicts; every extension
   stays loaded. An additional local extension path that does not exist after
   resolution is reported.
5. Load skills, prompt templates and themes from additional sources, then
   configured ones, then the additional paths. A disabled kind keeps only the
   explicit and additional paths. Prompt templates and themes keep the first
   resource per name and report each later one as a collision.
6. Discover context files unless disabled.
7. Use the explicit system prompt, else `<cwd>/.maestro/SYSTEM.md`, else
   `<agent>/SYSTEM.md`, else a contributed base system prompt. Appended texts
   start with contributed base text, followed by the explicit list or the first
   `APPEND_SYSTEM.md` found the same way. An ordinary prompt input naming an
   existing file is replaced by its text; an empty value means none.

Each override receives the freshly loaded result by value, in the order
extensions, skills, prompts, themes, context files, system prompt and appended
texts. The appended-text override replaces the ordinary append choice; base
append text still goes first.

Additional paths are trimmed, a leading `~` becomes the home directory, and the
result is resolved against the loader's working directory. Merging keeps the
first path per real path. A missing additional skill, prompt template or theme
path is reported as an error unless a diagnostic already names it.

### Base package inputs

The application supplies `ResourceLoaderOptions::base_package_sources` as an
ordered list of ordinary package sources. The default is empty. Base packages
are not identified by a package marker or a name built into the loader.
Supplied base sources still load under `--no-extensions`, just like explicit
`-e` sources. That flag skips configured and automatic extensions, not the base
bootstrap. Default source installation and source filters are separate inputs.
Base resources remain below project, user and configured package resources.
Tools and flag metadata use the first registration in the ordinary extension
list; shortcuts use the later registration, and duplicate commands get numbered
invocation names. Flag defaults retain the first defined value, even when that
owner later replaces its flag metadata.

During its factory, a base entrypoint may call `ExtensionAPI::contribute_load_time`
with a JSON object. Native and component authors use the same fields:

```json
{
  "packageSources": [{"source": "./package", "scope": "project"}],
  "baseSystemPrompt": "base system",
  "baseAppendText": ["first", "second"],
  "modelFiles": ["./models.json"],
  "baseLayers": []
}
```

Fields may be omitted; unknown fields and wrong types are load errors attributed
to the publishing extension. Package sources use the ordinary installer and
resource collector, with their supplied scope, in this startup. Duplicate source
entries do not load an extension twice. Prompt inputs are text, not file names.
The first base system prompt is the fallback below explicit inputs and local
files; base append texts keep contribution order. Model paths resolve like other
resource paths and precede the catalog's existing files.

Each `baseLayers` item has `scope` (`user` for global settings, or `project`) and
`layer`: the settings owner's `BaseSettingsLayer` record, or `null` to remove that
scope's layer. Only one base package source may publish settings layers during
one reload. Its entrypoints share that identity; another package is reported
without applying its publication. An extension whose factory fails contributes
nothing. The channel closes before ordinary extensions load and is rebuilt on
reload. Context-file loading is unchanged.

### Provenance

A resource takes the provenance an extension contributed for its path or a
containing directory, else the reload's entry for the path or its nearest
recorded ancestor. Lookups walk the resource path's own ancestors, so their cost
follows the path depth. Without a match, a resource under
`<agent>/{skills,prompts,themes,extensions}` is user-scoped and one under
`<cwd>/.maestro/...` is project-scoped; anything else is a temporary local
resource based at the directory itself or the file's parent. Each extension
gets a new provenance record that its commands and tools share.

`extend_resources` resolves extension-contributed paths the same way, records
their provenance and rebuilds each kind that gained paths, keeping the reload's
package and automatic provenance for the resources rebuilt beside them.

## Context files

`load_project_context_files` returns the agent directory's context file, then
one file per directory from the filesystem root down to the working directory,
each path once. In each directory the first existing name of `AGENTS.md`,
`AGENTS.MD`, `CLAUDE.md` and `CLAUDE.MD` is used. A file that exists but cannot
be read writes `Warning: Could not read <path>: <error>` to standard error, in
yellow when standard error is a terminal, and the next name is tried.

## System prompt

`resources::system_prompt::build_system_prompt` takes the prompt options. The
documentation paths come from the running process's package directory (see
[application configuration](configuration.md#installation-paths)); the examples
path is `<package>/examples`, which holds the index pages for the extension and
SDK examples. A non-empty custom
prompt replaces the default one. The default prompt lists the selected tools
(read, bash, edit and write when none are selected) that have a non-empty
snippet, or `(none)`, followed by the guidelines and the documentation paths.
Guidelines keep their first occurrence: the exploration guideline the tools call
for, the trimmed caller guidelines, then the two standing ones. The exploration
guideline names only the `grep`, `find` and `ls` tools that are selected; with
`bash` alone it speaks of listing and searching files without naming a tool. The
documentation section tells the model to resolve the `docs/` and `examples/`
topic paths against the installed package directory. Both prompts end
with the appended text, the context files, the skills when the `read` tool is
available, the local date and the working directory with `/` separators. Only
the default prompt resolves documentation paths.

## Default event bus

Without a supplied bus, the loader builds one for its execution family on the
caller's `LocalSet`. Listener prefixes run during emission and continuations run
on the task set; the threaded family sends them through one channel drained by a
task on the set. A listener failure writes the bus's
`Event handler error (<channel>): <error>` line to standard error. The
[resource library notes](../libraries/application-resources.md) list the
features this uses.

## Session services

`create_agent_session_services` uses the supplied agent directory or
`get_agent_dir()`, credentials from `<agent>/auth.json`, settings for the
working directory and agent directory, and the catalog from
`<agent>/models.json`, unless each is supplied. It reloads a loader sharing the
services' settings, applies contributed model files, then applies each queued
extension provider registration in
order, reporting a failure as `Extension "<path>" error: <message>` without
stopping the others, and empties the queue. Extension flag values set a
registered boolean flag to `true` and a registered string flag to its text; a
string flag without text and unregistered names are reported as errors. No
session is created; diagnostics are returned for the caller to present.

## Theme directories

Native `config::get_theme_directories` selects the shipped theme directory from
the package directory (`MAESTRO_PACKAGE_DIR`, else the running installation's)
and the custom theme directory from the environment-selected agent directory. A
run from a source checkout reads the shipped themes from
`crates/maestro-theme/assets/theme` in that package directory; a standalone
executable reads them from the `theme` folder in the package directory. See
[application configuration](configuration.md#installation-paths).

## Startup session construction

The command-line `startup::main::runtime::create_runtime` constructs a selected
session over already-created services. It collects service, settings and
extension diagnostics before applying model, thinking and tool selections. An
explicit API key needs a selected command-line or scoped model, even when the
session constructor could choose a default later. Caller patterns, including an
empty list, take precedence over saved enabled-model patterns. The operation
retains the supplied history and service handles; it does not bind directories,
replace a runtime or launch a frontend. Startup entry and executable composition
remain separate work.
