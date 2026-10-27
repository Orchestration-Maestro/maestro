# Install and author resource packages

Use a package to share extensions, skills, prompt templates, themes or browser
contributions. Maestro accepts npm packages, Git repositories and local paths.
Consumers load built extension components; installing a package does not compile
Rust. For the acquisition API and detailed discovery rules, see
[package resources](packages.md).

Review packages before installing them. Git preparation and npm installation can
run dependency lifecycle scripts. Loaded extensions inherit the user's access,
and skills can direct the model to run tools. Neither a package filter nor a
component is a security boundary. See [extension access](extensions.md) before
loading code from someone else.

## Contents

1. [Choose a source and scope](#choose-a-source-and-scope)
2. [Inspect and maintain packages](#inspect-and-maintain-packages)
3. [Assemble a distribution](#assemble-a-distribution)
4. [Select resources](#select-resources)
5. [Resolve overlapping sources](#resolve-overlapping-sources)
6. [Troubleshoot](#troubleshoot)

## Choose a source and scope

Use the executable built in [your first session](first-session.md#1-build-from-source).
The commands below use `maestro` as that executable's name. The remote package
names and repository addresses illustrate source syntax; replace them with a
reviewed package you can access. Pick one source form for a package: the npm,
Git and local commands in each block are alternatives.

| Source | Unpinned spelling | Pinned spelling |
| --- | --- | --- |
| npm | `npm:@workshop/review-kit` | `npm:@workshop/review-kit@1.0.0` |
| Git shorthand | `git:github.com/workshop/review-kit` | `git:github.com/workshop/review-kit@v1.0.0` |
| Git URL | `https://github.com/workshop/review-kit` | `https://github.com/workshop/review-kit#v1.0.0` |
| Local | `./review-kit` or an absolute path | Use your own version control; local paths have no package pin |

An explicit npm suffix, including a tag or range, prevents package updates.
For a reproducible version, use an exact version rather than a moving tag or
range. Git refs also prevent package updates; choose a commit hash if the ref
must not move. See [source parsing](package-sources.md) for SSH forms and identity.

### Register for your user or project

1. Install a source for all projects using your user configuration:

   ```sh
   maestro install npm:@workshop/review-kit@1.0.0
   maestro install git:github.com/workshop/review-kit@v1.0.0
   maestro install ./review-kit
   ```

2. Use `--local` (or `-l`) when the registration belongs to this project:

   ```sh
   maestro install npm:@workshop/review-kit@1.0.0 --local
   maestro install git:github.com/workshop/review-kit@v1.0.0 -l
   maestro install ./review-kit -l
   ```

The default registration file is `~/.maestro/agent/settings.json`; project
registrations go to `<cwd>/.maestro/settings.json`. A nonempty
`MAESTRO_CODING_AGENT_DIR` changes the user directory. Installation prints progress
and a completion line, then saves the source under `packages`. Commit project
settings if the team should load those packages. Online startup acquires missing
remote contents; `--offline` skips remote sources that need acquisition.

User npm installs use the configured npm command's global root; project npm
installs use `<cwd>/.maestro/npm/node_modules`. Git checkouts use
`<agent>/git/<host>/<path>` or `<cwd>/.maestro/git/<host>/<path>`. Local installation
checks the path but does not copy it. Input paths resolve from your working
directory; stored relative paths resolve from the directory containing the
settings file. Let `install` calculate that stored path rather than copying a
relative input into settings unchanged.

To use a package-manager wrapper, set `npmCommand` in settings. This example
selects npm through mise for lookup, installation and Git dependency preparation:

```json
{
  "npmCommand": ["mise", "exec", "--", "npm"]
}
```

The default is `npm`. This array is a command and literal leading arguments,
not a shell expression. See [npm command selection](packages.md#sources).

### Try without registration

Pass repeatable `-e` sources for one run:

```sh
maestro -e npm:@workshop/review-kit@1.0.0
maestro -e git:github.com/workshop/review-kit@v1.0.0
maestro -e ./review-kit
```

Despite the flag's long name, `--extension`, a package directory can supply all
its resource kinds. Explicit sources do not add a `packages` registration.
Remote contents use the OS temporary directory's `maestro-extensions` cache,
which can survive the process; this is not a promise to delete files at exit.
Local sources resolve from the current working directory. Existing unpinned
Git sources refresh online; a failed refresh can leave the cached contents in
use. [Temporary acquisition](packages.md#conventional-resource-discovery)
describes the cache paths and failure rules. These commands otherwise start a
normal session, so select your model and credentials as usual.

## Inspect and maintain packages

1. Run `maestro list`. It shows configured user and project sources, installed
   paths when present, and `(filtered)` for object-form registrations. It is a
   settings listing, not a check of live resource validity or available updates.
2. Run `maestro config` in a terminal to enable or disable discovered extensions,
   skills, prompts and themes. Search for an entry, select it with the navigation
   keys, toggle with Space, and close with Escape. The selector saves changes in
   the owning scope; see [selectors](chat/selectors.md#resource-configuration).
   Known issue: keys are currently ignored ([#3006](https://github.com/Orchestration-Maestro/maestro/issues/3006)); edit `packages`/filter settings manually meanwhile.
3. Update packages without updating the executable:

   ```sh
   maestro update --extensions
   maestro update --extension npm:@workshop/review-kit
   ```

   The named command selects the configured package by identity. Pinned sources
   and local paths are skipped. To change a pin, install the new pinned source;
   Maestro replaces the first matching registration without dropping its filters.
   Plain `maestro update` selects both packages and the executable. Source builds
   may need a manual application update; see
   [self-update selection](application/configuration.md#self-update-selection).

4. Remove from the same scope you installed into:

   ```sh
   maestro remove npm:@workshop/review-kit
   maestro remove git:github.com/workshop/review-kit -l
   maestro remove ./review-kit -l
   ```

   `uninstall` is an alias for `remove`. Removal deletes managed npm/Git contents
   before removing the registration. Local removal leaves your files intact.
   Managed Git updates can discard edits and untracked files; use a local source
   when developing a package. Read [Git recovery limits](package-sources.md#git-preparation-and-recovery)
   before changing an installed checkout.

## Assemble a distribution

1. Create the resources using their existing formats:
   [extensions](extensions.md), [prompt templates](resources/prompt-templates.md),
   [skills](resources/skills.md), [themes](theme.md), and optionally
   [browser contributions](browser/frontend.md#contributions).
2. Build extensions before distribution. Follow the
   [component packaging instructions](extensions.md#package-the-component) and
   the [dependency example](../examples/extensions/with-deps/). Keep the SDK and
   host revisions matched. Rust dependencies are linked into the component;
   consumers do not need Cargo or a source-language interpreter.
3. Put the built files beside a `package.json`. This complete manifest includes
   each supported resource kind; the contribution is for a browser host:

   ```json
   {
     "name": "@workshop/review-kit",
     "version": "1.0.0",
     "description": "Review instructions and tools for a project",
     "files": ["contributions", "dist", "prompts", "skills", "themes"],
     "maestro": {
       "extensions": ["dist/review.wasm"],
       "skills": ["skills/review"],
       "prompts": ["prompts/review.md"],
       "themes": ["themes/review.json"],
       "contributions": ["contributions/review.json"]
     }
   }
   ```

   Paths are relative to the package root. Create every listed file; a manifest
   selects resources, not their contents. Include any data the component reads
   alongside its artifact. The host supplies `MAESTRO_EXTENSION_PATH` so it can
   locate that data. A terminal session does not render browser contributions.
4. Inspect the distribution with `npm pack --dry-run`. Include built artifacts
   in the npm tarball or Git commit, not only in an ignored build directory.
   Try the local directory with `maestro -e ./review-kit` from its parent before
   publishing it through your npm or Git release process.

Maestro does not use npm `keywords`, `video` or `image` metadata for package
selection or resource discovery. Use `name`, `version` and `description` for the package's
ordinary npm metadata; this guide does not assume a Maestro package gallery.

### Dependencies and entry points

Git acquisition runs npm installation when `package.json` exists. With the
default npm command it omits development dependencies; a configured nonempty
`npmCommand` uses plain `install`. Declare runtime files in `dependencies`, not
only `devDependencies`. Avoid install-time compilation of the extension.

If another package supplies resources, include it in the distributed contents
(for npm, use `dependencies` and `bundledDependencies`), then name the desired
paths explicitly, such as `node_modules/@workshop/shared-skills/skills` in
`maestro.skills`. Discovery does not automatically scan `node_modules`.
Do not copy host SDK libraries into a package as JavaScript peer dependencies;
the extension interface is the built component's imports and exports.

Without a `maestro` manifest, conventional directories discover resources:
`extensions/`, `skills/`, `prompts/`, `themes/` and `contributions/`. Extensions
admit top-level component files and child entry points such as `index.wasm`;
helper files inside a child are not extra entries. Skills find `SKILL.md`
directories and loose root Markdown; prompts recurse for `.md`, themes and
contributions for `.json`. Ignore files can prune directory discovery. A
manifest's explicit lists replace conventional package-root selection, so list
every kind you want. See [manifest selection](packages.md#package-root-manifests)
for globs and exclusions. Discovery also recognizes `.js` and `.ts` names, but
the executable loads components, not those source languages; use `.wasm` entries.

## Select resources

Edit `packages` in the appropriate settings file. Keep a string to accept the
package's resource selection, or use an object to filter each kind:

```json
{
  "packages": [
    {
      "source": "npm:@workshop/review-kit@1.0.0",
      "extensions": [],
      "skills": ["skills/**", "!skills/experimental/**", "+skills/experimental/check/SKILL.md"],
      "prompts": ["prompts/review.md", "-prompts/legacy.md"],
      "themes": [],
      "contributions": []
    }
  ]
}
```

This leaves extensions, themes and contributions disabled and selects the review
prompt. It excludes experimental skills, restoring the exact `check` skill only
if the package admitted it.
Omit a filter key to use the package's selection for that kind; `[]` disables it.
Positive patterns select candidates; `!pattern` excludes matches, `+path`
restores an exact candidate, and `-path` removes an exact candidate last.
A restoration cannot discover a file outside the candidate set. Nonempty
manifest lists supply those candidates; empty or absent lists fall back to the
conventional directory for a present settings filter. Manifest exclusions have
already removed their candidates. See [configured resolution](packages.md#configured-resource-resolution)
and [file patterns](package-patterns.md) for the exact selection rules.

For conventional discovery, `<package>/prompts/.gitignore` can contain
`drafts/` to prune the drafts directory. Manifest positive globs instead expand
without consulting ignore files. To exclude a matched manifest entry, put
`!prompts/drafts/**` in `maestro.prompts` beside its positive entries.

## Resolve overlapping sources

Project package registrations win over user registrations with the same
identity, even when their pins differ. npm identity is the package name; Git
identity is the repository host/path without transport or ref; local identity is
the resolved absolute path. `list` can show both registrations even though
resolution uses only the project one. Separate npm and Git installations have
different identities and can load two copies of the same extension. If both
register the same tool, startup reports a conflict; keep one source or disable
one copy with a filter.

Within each resource kind, configured resolution ranks project local paths,
project automatic discovery, user local paths, user automatic discovery, then
package resources. It retains the first canonical file, even when that record
is disabled. Installing one package at two paths does not make all its resources
unique; symlinks can resolve to the same file. Explicit `-e` resources also
participate in the application's loading rules; see
[resource loading](application/resources.md#resource-loader).

## Troubleshoot

| Result | Next action |
| --- | --- |
| `Path does not exist: <path>` | Check the install input from the current directory; check stored paths from their settings directory. |
| `No matching package found for <source>` | Copy the source from `maestro list`, including `npm:` or `git:`. |
| Package listed but resource missing | Check the manifest, ignored directories, filters and disabled duplicates before reinstalling. |
| `Tool "<name>" conflicts with <path>` | Remove a duplicate extension source or disable one copy with a package filter. Different checkouts are different files. |
| Discovered `.js` or `.ts` fails to load | Distribute a built Maestro extension component instead; see [component authoring](extensions.md). |
| Missing package disappears offline | Acquire it online first; offline startup skips sources needing acquisition. Explicit `install` itself is not disabled by offline mode. |
