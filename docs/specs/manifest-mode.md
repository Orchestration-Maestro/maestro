# Manifest mode

This specification defines manifest mode. The foundation changes are recorded in
[Amendment to the foundation specification](#amendment-to-the-foundation-specification),
and the terms in [Glossary](#glossary) are also defined in [the glossary](../../CONTEXT.md).

## Purpose and authority

Maestro runs in one of two modes, chosen by settings.

- **Internal mode** is the [foundation specification](maestro-port.md), unchanged.
  It applies whenever settings name no manifest. Nothing in this document adds a
  read, a file, a network call, a printed line, a setting effect or a wire field to
  internal mode, except the one corrected package defect listed under
  [Corrected reference defect](#corrected-reference-defect).
- **Manifest mode** applies when settings name a manifest: an organization's
  innersource configuration, published as a pinned package release. The manifest
  supplies a layer for every agent-facing item, and it may lock, bound or require
  values. Users and projects keep personal changes within those constraints.

Everything in manifest mode is an owner-approved difference with no reference
source, listed in [Differences with no reference source](#differences-with-no-reference-source).
Delivery, installation, update, settings layering, resource loading, model
selection and session construction reuse the foundation owners; manifest mode
composes them differently and adds one policy owner. It adds no crate, no crate
edge, no loader, no settings scope and no new trait.

Manifest mode is native-only. The browser application has no package installer and
stays in internal mode.

## Problem statement

An organization wants every developer's agent to start from the same reviewed
configuration: system prompt, persona, instructions, skills, prompt templates,
extensions, models, thinking levels, tools and agent-behaviour settings. Teams own
parts of that configuration and change it by pull request. Some values must not be
changed locally (an approved model list, a privacy setting, mandatory standards),
while developers still personalise everything else and try local extensions without
a pull request. Today each developer configures Maestro by hand, nothing is
enforced, and nothing tells a developer which configuration is active.

## Solution

Settings name one manifest release, for example
`"manifest": "git:github.com/org/maestro-manifests@v1.4.0"`. Maestro installs that
release the way it installs packages, reads its areas, and builds the session from
five layers: engine defaults, the manifest, the user, the project and the run. The
manifest's policy decides which local changes are allowed. A broken manifest stops
agent sessions with the exact fix. `maestro update` proposes newer manifest
releases and never moves a pin silently. The interactive startup shows which
manifest is active, and settings views mark locked values.

## User stories

1. As an organization administrator, I want to publish one manifest release, so that every developer who names it gets the same agent configuration.
2. As a team lead, I want the project's `.maestro/settings.json` to name the manifest release, so that the whole team uses it and changes it by pull request.
3. As a developer, I want to name a manifest in my own settings, so that every project I open uses it.
4. As a developer, I want Maestro to behave exactly as before when no manifest is named, so that nothing changes for me unless I opt in.
5. As a developer, I want a missing manifest release installed on first start, so that I do not run a separate install step.
6. As a developer working offline, I want a clear refusal naming the missing manifest and the fix, so that I never silently run without my organization's locks.
7. As a developer, I want a broken manifest to stop agent sessions with the settings file, the problem and the fix, so that I can repair it quickly.
8. As a developer with a broken manifest, I want `--help`, `--version`, `maestro install`, `maestro update`, `maestro list`, `maestro config` and settings editing to keep working, so that I can repair the setup from Maestro itself.
9. As an administrator, I want to lock the manifest selection itself, so that a project cannot point developers at another configuration.
10. As a team lead, I want to bump the project's manifest pin by pull request and have every teammate's next start use the new release, so that adoption needs no manual reinstall.
11. As a developer, I want `maestro update` to show a newer manifest release and what changed, so that I decide when to adopt it.
12. As a developer, I want to opt into applying manifest releases without a prompt, so that I stay current without extra steps.
13. As a security reviewer, I want any release that grows permissions to ask before it applies, even with automatic application on, so that new tools, wider bounds or removed locks are never adopted silently.
14. As a developer, I want ordinary package updates to keep working unchanged in manifest mode, so that my own packages update as before.
15. As an administrator, I want the manifest to supply the base system prompt, so that every agent starts from the organization's prompt.
16. As an administrator, I want the main agent's persona appended to the system prompt, so that its behaviour is consistent.
17. As a developer, I want my own `SYSTEM.md`, `APPEND_SYSTEM.md` and prompt flags to keep working when the manifest does not lock them.
18. As an administrator, I want to lock the system prompt or the appended prompt, so that local files and flags cannot replace them.
19. As a standards owner, I want standards instructions always loaded, so that `--no-context-files` cannot drop them.
20. As a developer, I want my project's `AGENTS.md` and ancestor context files loaded after the manifest's instructions, so that repository guidance still applies.
21. As a team, I want to ship skills, prompt templates, extensions and themes in the manifest, so that developers get them without installing packages.
22. As a developer, I want to add my own skills, templates and extensions and to shadow optional manifest ones, so that I can personalise and experiment.
23. As an administrator, I want required resources that cannot be disabled, shadowed or dropped by discovery flags, so that mandatory capabilities stay active.
24. As an administrator, I want to forbid local extensions, skills, prompt templates or packages, so that only reviewed code and instructions run.
25. As a developer, I want `maestro config` to toggle optional manifest resources with the same filters I use for packages, so that I control my working set.
26. As an administrator, I want the manifest to declare packages that every developer gets, so that shared packages need no per-developer install.
27. As an administrator, I want to supply custom providers and models in the manifest, so that developers reach the organization's endpoints.
28. As a security reviewer, I want manifest provider keys to be environment or command references, never literal secrets, so that no secret is published in the manifest.
29. As a developer, I want my own `models.json` to add providers and partially override manifest models without losing their other fields, unless providers are locked.
30. As an administrator, I want an allowed set of models, so that every selection path (default, flags, `/model`, cycling, RPC, extensions, SDK and resumed sessions) stays inside it.
31. As an administrator, I want a default thinking level with a minimum and a maximum, so that developers may raise thinking up to the maximum but not beyond it.
32. As a developer, I want a stored default outside the policy resolved to an allowed value with a diagnostic, so that an old setting never blocks my start.
33. As a developer, I want an explicit request outside the policy refused with the lock or bound that forbids it, so that I know why it failed.
34. As a developer resuming a session, I want a recorded model outside the policy replaced by the policy's choice with the usual fallback message, so that edited session files cannot bypass the policy.
35. As an administrator, I want to lock or bound agent-behaviour settings (transport, steering, follow-up, compaction, branch summary, retry, skill commands, image resizing, thinking budgets), so that agent behaviour and spend are governed.
36. As a privacy officer, I want to force image blocking and disable install telemetry, so that no environment variable or setting turns them back on.
37. As a developer, I want presentation, keybindings, machine paths, credentials and my theme choice to stay mine, so that the manifest never governs my terminal or secrets.
38. As an administrator, I want the main agent's tools limited by the manifest's role definition, so that developers can drop tools but never add tools beyond it.
39. As an administrator, I want local extensions unable to replace a locked prompt, activate forbidden tools, register providers under a provider lock or change model and thinking outside the policy.
40. As a developer, I want `/reload` to pick up manifest and settings changes, and to keep my current configuration with an error when the new manifest is broken.
41. As a developer switching to a project that names another manifest, I want the new session built from that project's manifest, so that each project gets its own configuration.
42. As a developer, I want the interactive startup to show the active manifest source, its release and the settings file that names it, so that I know which configuration is in force.
43. As a developer, I want `/settings` and `maestro config` to mark manifest-set and locked values, and never offer an edit the policy would refuse.
44. As an RPC or JSON client author, I want the protocol unchanged, so that clients keep working in both modes.
45. As an SDK author, I want internal-mode construction and every injectable component unchanged, so that embedding programs behave as before.
46. As a team, I want to select the language and team areas a project uses, so that developers load only the instructions and resources relevant to them.
47. As an administrator, I want team areas to add or narrow but never loosen the organization's constraints, so that a team package cannot unlock a value.
48. As a role author, I want manifest roles with model, thinking and tool limits for authored agents, so that delegated agents follow the same policy (delivered with the authored-agent consumer).
49. As a developer, I want to raise a role's thinking or choose its model within the manifest's bounds through my settings, so that I tune roles without editing the manifest (delivered with the authored-agent consumer).
50. As an administrator, I want the manifest to supply MCP servers that local gateway options cannot drop, with credentials kept local (delivered with the MCP base package).

## Behaviour contract

### M1 — Selection

1. The top-level `manifest` setting names the manifest. Its value takes the package
   entry form of `packages`: a source string, or an object with `source` and the
   optional `extensions`, `skills`, `prompts` and `themes` filters. The source uses
   the package source grammar and its parser.
2. An absent key, an empty string or an object with an empty `source` means internal
   mode.
3. The source must be pinned in the package sense: an npm source with a version, or a
   Git source with a ref. A local path, an unpinned source or a wrong-typed value is
   an invalid selection (M9).
4. The selection is read from the raw user and project settings files, before any
   manifest loads. With one value, that value is used. With two values naming the
   same identity, the project value is used (the ordinary project-wins order).
   With two values naming different identities, the user's manifest is loaded
   first: if it locks `manifest`, the project value is ignored with a diagnostic;
   otherwise the project value is used. If the user's manifest cannot be loaded in
   that case, the selection is broken (M9).
5. Identity follows package identity: npm package name, or Git repository URL
   without its ref.
6. A lock on `manifest` fixes the identity. Changing the identity or removing the
   value through Maestro is refused. Changing the version of the same identity is
   allowed only through the update apply route (M11).
7. The manifest installs in the scope of the settings file that names it: user
   scope for the user file, project scope for the project file. The selected
   `manifest` value is never written into `packages`.
8. `MAESTRO_CODING_AGENT_DIR` keeps relocating the user configuration, including a
   user selection. No flag selects a manifest.

### M2 — Delivery and acquisition

1. Acquisition is ordinary package acquisition for the selected source and scope,
   with the package owner's progress, offline handling, configured npm and Git
   commands, and installation paths. There is no second downloader, cache or
   updater.
2. A missing release installs at start. With `MAESTRO_OFFLINE`, or when installation
   fails, the release stays missing and agent sessions refuse (M9).
3. A pinned npm release whose installed version differs from the pin is reinstalled
   (existing behaviour). A pinned Git release whose checkout is not at its ref is
   checked out at the ref (corrected defect). A pin change made by a pull request
   therefore takes effect at the next start.
4. A release that is installed but contains no resources is valid; absence of
   files is not a missing release.

### M3 — Layout and reading

The manifest is read as areas, found by location. The engine reads no area
descriptor; ownership records in a manifest repository are repository data.

| Area | Location | Loaded |
|---|---|---|
| Common | the release root | always, first |
| Standards | `standards/<domain>/` | always, every domain, in directory-name order |
| Core | `core/` | always |
| Languages | `languages/<name>/` | when listed in `manifestAreas`, in list order |
| Team packages | `packages/<name>/` | when listed in `manifestAreas`, in list order, after languages |

Every area may contain:

| Content | Meaning |
|---|---|
| `skills/`, `prompts/`, `extensions/`, `themes/`, or a package-manifest `maestro` key | Package resources, read by the package collector (M6) |
| `instructions/*.md` | Context instructions, in file-name order (M5) |
| `agents/*.md` | Role definitions in the authored-agent frontmatter format: `name`, `description`, `tools`, `model`, plus `thinking`; the body is the role prompt |
| `settings/settings.json` | A settings layer using the ordinary settings keys (M4) |
| `settings/policy.json` | Policy (M4) |

Only the core area may contain `SYSTEM.md` (base system prompt), the role named
`maestro` (the main agent's persona and tools), `llm/models.json` (model file in the
ordinary models format) and `backends/mcp/mcp.json` (MCP servers, M13).

`manifestAreas` is a list of `languages/<name>` and `packages/<name>` entries. Its
value resolves like any setting: the manifest's common, standards and core
settings give the default, then user, then project. A listed area that does not
exist, an entry with another prefix, or a `manifestAreas` or `manifest` value or
constraint inside a language or team-package area makes the manifest invalid.
Duplicate entries load once.

Settings layers combine in area order with the ordinary one-level merge. The
manifest's settings must not set the resource path keys `extensions`, `skills`,
`prompts` or `themes`; area folders supply those resources. Its `packages` entries
are the manifest's required packages (M6).

### M4 — Layers and policy

Resolution order, lowest to highest: engine defaults; manifest (areas in M3 order);
user; project; run (flags and SDK options); runtime changes. Raw user and project
settings stay raw: the manifest layer is never written into them, and their
snapshots, unknown members, pending edits and reload behaviour are unchanged.

`settings/policy.json` (all members optional; an unknown member makes the manifest
invalid):

```json
{
  "locked": ["defaultProvider", "defaultModel", "compaction.enabled", "extensions", "systemPrompt"],
  "bounds": {
    "defaultThinkingLevel": { "min": "medium", "max": "xhigh" },
    "retry.maxRetries": { "max": 5 }
  },
  "models": ["anthropic/claude-*", "openai/gpt-6.1-sol"],
  "required": ["core/skills/review", "standards/security/instructions/secrets.md"],
  "roles": {
    "coder": {
      "locked": ["model"],
      "bounds": { "thinking": { "min": "high", "max": "xhigh" } },
      "models": ["anthropic/*"]
    }
  }
}
```

| Member | Class | Meaning |
|---|---|---|
| `locked` | L | Each entry is a settings key path (dotted for nested members) or one of the lock names `systemPrompt`, `appendSystemPrompt`, `providers`. The locked value is the manifest's settings value, or the engine default when the manifest sets none; a `manifest` lock fixes the manifest's own identity (M1). |
| `bounds` | B | `min` and/or `max` for numeric keys and for thinking levels (order: off < minimal < low < medium < high < xhigh). |
| `models` | B (allowed set) | Model patterns in the `enabledModels` syntax. Every model selection must match. |
| `required` | A | Paths relative to the release root. A resource at or under a required path is required. Standards-area instructions are always required. |
| `roles` | L/B per role | `locked` (`model`, `thinking`, `tools`), `bounds.thinking`, `models` for one role (M12). |
| `credentialHeaders` | Credential classification | Header names added to the default credential list (M7); additions from every selected area combine. |

Governed keys and what the manifest may do with them:

| Keys | Manifest may |
|---|---|
| `defaultProvider`, `defaultModel` | lock; constrain through `models`; default |
| `defaultThinkingLevel` | lock; bound; default |
| `enabledModels` | lock; default (the resolved scope is always intersected with `models`) |
| `thinkingBudgets.{minimal,low,medium,high}` | lock; bound (maximum); default |
| `transport`, `steeringMode`, `followUpMode`, `enableSkillCommands`, `images.autoResize` | lock; default |
| `compaction.{enabled,reserveTokens,keepRecentTokens}`, `branchSummary.{reserveTokens,skipPrompt}`, `retry.{enabled,maxRetries,baseDelayMs}`, `retry.provider.{timeoutMs,maxRetries,maxRetryDelayMs}` | lock; bound (numeric members only); default |
| `images.blockImages`, `enableInstallTelemetry` | lock (normally to `true` and `false`); default |
| `packages`, `extensions`, `skills`, `prompts` | lock (no local additions); require entries |
| `manifest`, `manifestAreas`, `manifestAutoUpdate` | lock; default (`manifestAreas`, `manifestAutoUpdate`) |
| `multiplexer.tabBarRight` | lock; default |
| `multiplexer.allowNested` | lock; default |
| `daemon.resumeAgentsOnRestore` | lock; default |
| `daemon.detectionCatalogCheck` | lock; default |
| `multiplexer.remote.manageSshConfig` | lock; default |
| `theme`, `themes`, `shellCommandPrefix` | default only |
| every other key (presentation, keybindings, machine paths, session directory, changelog state, credentials) | nothing: user-only |

Core invariants are compiled in: a lock or bound on a default-only or user-only key,
on an unknown key, or a bound on a non-numeric, non-thinking key makes the manifest
invalid.

Combining areas, in area order: locks join, and a later area that sets a different
value for a key an earlier area locked, or locks it with a different value, makes
the manifest invalid. Minimums take the largest, maximums the smallest, allowed
model sets intersect, required paths join. A combined bound with minimum above
maximum, or a manifest settings value outside its own combined constraints, makes
the manifest invalid. Team areas can therefore only add or narrow.

Stored values and explicit requests differ:

- **Stored values** (user and project files, restored sessions): a locked key uses
  the manifest value; a bounded value is clamped to the bound; a model outside
  `models` is replaced by the manifest default when it is allowed, otherwise by the
  first available model, in catalog order, matching the earliest pattern that
  matches any available model. Each produces a
  diagnostic through the channel that reports settings load errors. The stored
  file is never rewritten by a read.
- **Explicit requests** (setters, settings edits, flags, SDK options, RPC commands,
  `/model`, cycling, extension calls, environment overrides of governed values) that
  a lock, bound, allowed set or required entry forbids are refused with a message
  naming the constraint and the manifest. A refused request changes no memory, no
  pending edit, no file, no session history and emits no change event.

The policy check has one owner, the settings owner. Every other owner asks it before
its own effect.

### M5 — Prompts and instructions

1. **Base system prompt.** Candidates, highest first: explicit (flag or SDK option or
   SDK override callback), project `SYSTEM.md`, user `SYSTEM.md`, manifest
   `core/SYSTEM.md`, engine default. A custom prompt still replaces the default
   documentation block. Under a `systemPrompt` lock, local files are ignored with a
   diagnostic and an explicit replacement is refused.
2. **Appended prompt.** The manifest append (the body of the core `maestro` role) is
   always first, followed by the ordinary choice: every explicit append in order,
   otherwise the discovered `APPEND_SYSTEM.md`. Under an `appendSystemPrompt` lock,
   local append files are ignored with a diagnostic and explicit appends are refused.
3. **Instructions.** Manifest instructions come first, in area order and file-name
   order, followed by the ordinary global and root-to-cwd context files with their
   filename aliases and first-readable rule. `--no-context-files` drops the ordinary
   files and optional manifest instructions, never required ones. All selected
   instruction files load; `applyTo` metadata is not interpreted.
4. Date, cwd, skills block, tool snippets and guidelines keep their existing
   assembly.

### M6 — Resources and packages

1. Each selected area root enters ordinary package collection as a package-origin
   source with the selection's source string and scope, so manifest resources rank
   below project and user resources. Their base directory is the release root, so
   filters and `maestro config` patterns are relative to the release root.
2. The selection's object filters apply with the ordinary filter semantics. A filter
   that disables a required resource is ignored with a diagnostic; `maestro config`
   refuses to write one.
3. A required resource wins its name collision; the local resource is reported as
   the collision loser. Discovery-disabling flags (`--no-skills`,
   `--no-prompt-templates`, `--no-extensions`, `--no-themes`) and SDK override
   callbacks never remove a required resource; a required resource missing after
   overrides refuses construction.
4. A lock on `extensions`, `skills`, `prompts` or `packages` stops local additions of
   that kind: settings entries and automatic user and project folders are ignored
   with a diagnostic, and explicit flags (`-e`, `--skill`, `--prompt-template`) refuse
   the run. Forbidden local components are excluded before they execute.
5. The manifest's `packages` entries are required packages. They install in the
   selection's scope and are collected after the area roots at package precedence.
   A missing required package refuses sessions. Removing a package whose identity
   matches a required package in the same scope is refused before any file is
   deleted.
6. Optional resources that fail to load are skipped with the ordinary warning. A
   required resource that fails to load refuses sessions.
7. Themes from the manifest are offered; the manifest's `theme` value is a default
   the user may change.

### M7 — Models and thinking

1. **Model files.** The manifest `core/llm/models.json` is read before the user's
   `models.json` with the same parser. A user entry that partially overrides a
   manifest provider or model keeps every manifest field it does not set; a user
   model with the same provider and ID replaces the manifest model. Refresh and
   dynamic provider removal restore the correct underlying layers. Under a
   `providers` lock the user file is ignored with a diagnostic and extension
   provider registration is refused.
2. A manifest `apiKey` or credential-header value must be a command (`!…`) or an
   environment variable name (upper-case letters, digits, underscores); any other
   value in those positions makes the manifest invalid. Credential header names
   match case-insensitively; the default list is `Authorization` and
   `Proxy-Authorization`, extended by `credentialHeaders` in the manifest's policy.
   Other headers accept literal values, including `Content-Type: application/json`,
   and keep ordinary environment and command resolution. Local credentials and key
   flags are unchanged.
3. **Selection.** Every path that selects a model passes `models`: the initial
   default, `--provider`/`--model`, `--models`, SDK options, `/model`, model
   cycling (scoped and unscoped candidates are filtered), RPC model commands,
   extension setters and session restoration. Every path that selects thinking
   passes the thinking bound and lock: `--thinking`, a model-pattern thinking
   suffix, settings, thinking cycling, RPC, extension setters and restoration.
4. The policy governs the requested thinking level. The model's capability clamp is
   applied afterwards, unchanged: a non-reasoning model still runs with thinking
   off under any minimum.
5. A refused runtime change happens before the active model or thinking level, the
   session history, the persisted default or the selection event change. The
   extension setter returns its existing unsuccessful result.
6. **Restoration.** A recorded model outside `models` is treated as not restorable
   and the ordinary fallback and fallback message follow. A recorded thinking level
   outside its bound is clamped with a diagnostic. Session files are not rewritten.

### M8 — Tools and the extension host

1. When the core `maestro` role declares `tools`, that list is the ceiling of the
   main agent's active tools. `--tools`, `--no-tools`, `--no-builtin-tools`, SDK tool
   options and extension activation may remove tools from it, never add beyond it.
   Registered tools outside the ceiling stay registered and inactive; refresh and
   reload never activate them. Removing every tool remains allowed.
2. An extension's per-turn system prompt replacement under a `systemPrompt` lock is
   not applied and is reported through the existing extension error path.
3. Extension model, thinking, tool and provider changes go through the same checks
   as the corresponding application operations.
4. `MAESTRO_TELEMETRY` can disable install telemetry but cannot enable it when
   `enableInstallTelemetry` is locked to `false`.
5. Extensions keep full user permissions. Governing configuration does not sandbox
   an extension's file or process access.

### M9 — Failure and repair

| Case | Behaviour |
|---|---|
| No manifest named | Internal mode |
| Invalid selection (not pinned, local, wrong type) | Agent sessions refuse |
| Release missing and not installable (offline, network, authentication) | Agent sessions refuse |
| Invalid manifest: malformed settings or policy, unknown or conflicting lock, impossible bound, invalid role file, invalid model file, literal secret, invalid `manifestAreas` | Agent sessions refuse |
| Missing required package, or a required resource that fails to load | Agent sessions refuse |
| Optional resource that fails to load | Skipped with the ordinary warning |
| A kind absent from the manifest | No manifest layer for that kind; engine defaults apply |
| Stored value conflicting with policy | Resolved as in M4 with a diagnostic |
| Explicit request conflicting with policy | Refused as in M4 |

A refusal names the settings file that selects the manifest, the source, the
problem and the fix, and no model request is made. It is never a silent fallback to
internal mode or to an earlier copy. `--help`, `--version`, `install`, `remove`,
`update`, `list`, `config` and settings editing work without a valid manifest. JSON
and RPC report a refusal through their existing error forms.

Every construction of session services (start, new session, resume, fork, import,
switching to another working directory) recomposes the manifest and refuses on
failure through the ordinary construction failure path. `/reload` recomposes too;
when recomposition fails, the session keeps its current composition and reports the
manifest error with its fix, the same way a failed settings scope keeps its
accepted values on reload.

### M10 — Visibility

1. Internal mode shows nothing new anywhere.
2. In manifest mode the interactive startup resource listing starts with a
   `[Manifest]` section: the source, the pinned release and the settings file that
   names it.
3. Manifest resources carry ordinary package-origin source information.
4. `/settings` and `maestro config` mark manifest-set and locked values and required
   resources, and offer no edit the policy would refuse.
5. Refusal and diagnostic messages name the lock, bound, allowed set or required
   entry and the manifest.
6. RPC and JSON output gain no fields.

### M11 — Updates

1. `maestro update`, `--extensions` and `<source>` keep updating unpinned packages and
   skipping pinned ones; the engine self-update is unchanged. Anyone may run them.
2. In manifest mode, interactive startup also checks the selected manifest for a
   newer release and only notifies, alongside the ordinary package update notice.
3. `maestro update` (without `--self` alone) also looks for a newer release of the
   selected manifest. It shows the current and newer release, the changed manifest
   files and every permission growth.
4. A permission growth is any change that adds a tool to a role, widens `models` or a
   bound, removes a lock or a required entry, or adds an extension or an MCP server.
   It is computed by composing the current and the candidate release and comparing
   them.
5. Inspection only fetches and reads the candidate's files, manifest and requested
   permissions in the temporary package scope. It reuses the package owner's fetch
   and checkout steps without their install step; it runs no lifecycle script,
   build step or component. Normal installation, including lifecycle steps, runs
   through ordinary acquisition only after approval. Rejection or cancellation
   leaves the active files, pin and settings byte-identical.
6. By default the newer release is applied only after the user confirms. With
   `manifestAutoUpdate: true` it applies without asking unless it grows permissions,
   which always asks. Without an interactive terminal a release is proposed, never
   applied.
7. Applying writes the new pin, same identity, into the settings file that holds the
   selection, then installs it through M2. For a project file, the team commits the
   change by pull request. A failed installation reports the real state; there is
   no rollback copy.
8. A pinned manifest never moves on its own: startup never installs a newer release.

### M12 — Roles (delivered with the authored-agent consumer)

1. Manifest role files are discovered below the user and project agent folders;
   user and project roles shadow optional manifest roles by name; required roles
   cannot be shadowed.
2. A role's `model`, `thinking` and `tools` come from its frontmatter, constrained by
   `roles.<role>` in the policy. `tools` is always narrow-only.
3. The user setting `roles.<role>.model` and `roles.<role>.thinking` override a
   role's values within its policy; these are preferences, never constraints.
4. The main agent's model and thinking come from settings (M4), not from the
   `maestro` role; the `maestro` role supplies the persona (M5) and the tool ceiling
   (M8).

### M13 — MCP servers (delivered with the MCP base package)

The core `backends/mcp/mcp.json`, in the MCP package's own configuration format, is
the gateway's lowest configuration source. Local servers and their credentials stay
local. Exclusive or explicit gateway configuration options cannot drop a required
manifest server.

## Implementation decisions

- **Composition, not a second system.** The application's session-services
  constructor reads the selection once. Without a selection it runs the existing
  construction. With one, it supplies the manifest through the existing seams: the
  resource loader's prompt, append and context inputs and override hooks; the
  package collector's sources; the settings manager's read-only base layer and
  policy; the model registry's ordered model files; and the session's selection and
  tool operations.
- **Manifest reader.** A private, native-only module of `maestro-app` owns
  selection, acquisition requests, area reading and the records it produces. Its
  records are concrete types; no trait.
- **Policy owner.** `maestro-settings` owns policy decoding, combination, stored-value
  resolution and edit refusal, as one concrete record built once per services
  construction and held by those services. It owns no resource scanning and no
  model catalog. Model patterns are matched by the catalog's existing matcher at
  the application boundary; settings gains no catalog dependency.
- **Settings base layer.** The settings manager holds an optional read-only base
  layer and policy. Every rebuild (construction, reload, overrides, edits) applies
  base, then the ordinary one-level user and project merge, then policy resolution.
  There is no new settings scope and no change to raw scopes or the write queue.
- **Packages.** The package owner gains a source operation that validates a pinned
  source, an installed-root acquisition operation reused by ordinary resolution and
  manifest selection, collection of supplied roots in the same accumulator, the
  corrected Git pin check, and newest-release discovery for one pinned source.
  Source parsing stays single.
- **Catalog.** The model registry accepts an ordered list of model files; the
  existing single-file construction is the one-element case.
- **Application operations.** Model, thinking, tool, behaviour-setting and extension
  host operations ask the policy before their own effects. Frontends render; they
  check nothing themselves.
- **No crate, edge or dependency is added.** The reader uses the application's
  existing JSON, path and frontmatter owners.

## Testing decisions

- Test external behaviour at the owning public interface: settings policy through
  the settings manager; manifest reading and composition through the application's
  services and SDK construction; package acquisition through controlled package
  operations; catalog layers through the model registry; startup and selectors
  through the chat frontend's existing harnesses.
- Use synthetic current-format manifest fixtures checked into the tests, with
  every policy class, area kind and failure case. No private or captured
  organization configuration is a fixture.
- Every refused request asserts that memory, pending edits, files, session
  history and emitted events are unchanged, not only the error text.
- Inspect a candidate whose lifecycle script writes a sentinel file, then reject
  or cancel it: the sentinel never appears and the active files, pin and settings
  stay byte-identical. An approved candidate installs through the ordinary flow.
- Manifest model-file tests accept a literal non-credential header, reject literal
  API keys and credential headers (including mixed-case names and policy-added
  names), and accept environment or command references in credential positions.
- Internal-mode equality is tested: with no selection, resources, prompt, model,
  tools, settings writes and JSON/RPC output equal the foundation behaviour, and no
  manifest path is read.
- Prior art: settings behaviour and reload tests, configured-source and pinned
  package checks, resource-loader precedence cases, catalog composition and
  provider lifecycle tests, startup listing and selector tests.

## Differences with no reference source

Each item is an owner-approved manifest-mode difference.

1. The `manifest` setting, its self-lock and the user-first reading of a locked
   selection.
2. The manifest layer between engine defaults and user settings, and the areas and
   their order.
3. The `manifestAreas` setting.
4. `settings/policy.json`: locks, bounds, the model allowed set, required entries
   and role constraints, and the lock names `systemPrompt`, `appendSystemPrompt` and
   `providers`.
5. Stored-value resolution and explicit-request refusal.
6. Refusing agent sessions on a broken manifest.
7. The `[Manifest]` startup section and the lock markers in settings views.
8. Newer-release proposals for the pinned manifest, permission-growth confirmation
   and the `manifestAutoUpdate` setting.
9. The `thinking` role field and the `roles.<role>.{model,thinking}` user setting.
10. Literal-secret rejection in manifest model files.

## Corrected reference defect

A pinned Git source whose checkout is not at its ref is never checked out at the
ref: installation returns early because the checkout folder exists, and the Git
installation path excludes the ref. A pin change in settings therefore keeps the
old files, while a pinned npm source is reinstalled on a version mismatch. The
package owner checks a pinned Git checkout against its ref and checks out the ref
when they differ. This also corrects internal mode.

## Runtime mechanics not copied

No manifest file watcher, polling interval, timer, daemon, cache, transaction or
rollback is added. Network timeout and worker counts stay the package owner's.
Lock-file leases, promise scheduling and engine error formats are not reproduced;
existing Rust owners keep their ordering and accepted-write semantics.

## Out of scope

- Administrator-only switches, system-wide configuration stores, signed releases
  and a last-valid copy.
- Detection of edits to installed manifest files; configuration authority is not a
  sandbox. Relocating the configuration root, editing settings files, older or
  modified programs, SDK programs supplying their own components, and local
  administrators remain outside it.
- An `applyTo` path filter, presets as an engine concept, area descriptors read by
  the engine, workflows, handoffs, policy hooks, quality profiles, bootstrap and
  `maestro init`.
- New RPC or JSON fields.
- Manifest mode in the browser application.

## Glossary

Add to `CONTEXT.md`:

- **Manifest:** An organization's pinned configuration release named by the
  `manifest` setting; distinct from a package manifest and a detection manifest.
- **Package manifest:** The `maestro` key of a package's `package.json` listing its
  resources.
- **Internal mode:** Running without a manifest; foundation behaviour.
- **Manifest mode:** Running with a named manifest, which adds a layer and a policy.
- **Manifest area:** One folder of a manifest (common, standard, core, language or
  team package) whose content enters the manifest layer in a fixed order.
- **Policy:** The combined locks, bounds, allowed models, required entries and role
  constraints of a manifest; the settings owner decides every governed change.
- **Lock:** A policy entry fixing a value that local layers cannot change.
- **Bound:** A policy minimum or maximum for a numeric value or thinking level.
- **Required resource:** A manifest resource that local layers cannot disable,
  shadow or drop.
- **Permission growth:** A manifest release change that adds role tools, widens
  allowed models or a bound, removes a lock or required entry, or adds an extension
  or MCP server.

Replace the sentence "Manifest governance and settings value locks are later
scope, not active foundation concepts." with "Manifest mode is specified in
[manifest mode](manifest-mode.md)."

## Amendment to the foundation specification

In `docs/specs/maestro-port.md`:

1. R03: replace "No mandatory manifest, origins or locked values are active in this
   phase." with "Internal mode has no manifest, origins or locked values; [manifest
   mode](manifest-mode.md) adds them only when settings name a manifest."
2. Completion and explicit later scope: replace "governed manifest/init/settings
   locks" with "`maestro init`", and add "Manifest mode and its settings locks are
   specified in [manifest mode](manifest-mode.md)."
