# Manifest mode

This specification defines manifest mode and the authored artifacts it shares with
ordinary local use. Changes to other specifications are recorded in
[Amendment to other specifications](#amendment-to-other-specifications), and the
terms in [Glossary](#glossary) are also defined in [the glossary](../../CONTEXT.md).
The placement is recorded in the
[manifest placement decision](../adr/manifest-extension-placement.md).

## Purpose and authority

Maestro runs in one of two modes, chosen by settings.

- **Internal mode** is the [foundation specification](maestro-port.md), unchanged.
  It applies whenever settings name no manifest. Nothing in this document adds a
  read, a file, a network call, a printed line, a setting effect or a wire field to
  internal mode, except the items listed under
  [Internal-mode changes](#internal-mode-changes).
- **Manifest mode** applies when settings name a manifest: an organization's
  innersource configuration, published as a pinned package release. The manifest
  supplies a layer for every agent-facing item, and it may lock, bound or require
  values. Users and projects keep personal changes within those constraints.

Manifest mode is a capability of the default-on `maestro-manifest` base package,
a package crate in this workspace released with Maestro. The core holds no
manifest code. It gains five generic hooks (H1 to H5, see
[Core hooks](#core-hooks)) and one generic setting that any package may use, and
the manifest package is their first user. Every manifest-mode behaviour is a
difference with no reference source, listed in
[Differences with no reference source](#differences-with-no-reference-source).

The same **authored artifacts** work without a manifest. A user or a project
defines settings, system prompts, context files, instructions, roles, skills,
prompt templates, workflows, extensions, themes and models in local files, and the
manifest is one optional source of the same artifact kinds. Each kind has one format that is valid in both
places ([M14](#m14--authored-artifacts)).

Manifest mode is native-only. The browser application has no package installer and
stays in internal mode.

## Problem statement

An organization wants every developer's agent to start from the same reviewed
configuration: system prompt, persona, instructions, skills, prompt templates,
workflows, extensions, models, thinking levels, tools and agent-behaviour settings.
Teams own parts of that configuration and change it by pull request. Some values
must not be changed locally (an approved model list, a privacy setting, mandatory
standards), while developers still personalise everything else and try new
artifacts locally before proposing them. Urgent fixes must reach running laptops
quickly, and a bad release must be reversible. Today each developer configures
Maestro by hand, nothing is enforced, nothing stops an agent from running a
destructive command or passing personal data to a provider, nothing tells a
developer which configuration is active, and Maestro cannot explain the
organization's configuration when asked.

## Solution

Settings name one manifest release, for example
`"manifest": "git:github.com/org/maestro-manifests@v1.4.0"`. The manifest package
installs that release with the ordinary package installer, reads its areas from the
release's manifest root and publishes a base settings layer and its policy through
the core hooks. The session is built from five layers: engine defaults, the
manifest, the user, the project and the run. The policy decides which local changes
are allowed, and every governed change is checked before it has any effect. Teams
ship capabilities: one package per team area, bundling roles, skills,
instructions, prompt templates and workflows. Guard rules in the manifest's core
area confirm destructive commands,
refuse writes to protected paths, replace personal data with markers and withhold
prompt injection from the model. A broken manifest makes the manifest package fail
its load, which stops agent sessions with the exact fix. `/manifest update` moves
the pin forward or back, a release can declare a minimum that every laptop takes at
its next session start, and running sessions check for new releases at an interval
the organization sets.
The interactive startup and `/manifest status` show which manifest is active, and
Maestro's own documentation describes every manifest artifact, so the agent can
answer questions about it.

## User stories

### Selection and delivery

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
11. As an administrator, I want any edit to the installed release, or to an installed instruction copy, repaired at every start with a notice, so that nobody can silently turn the organization's prompt, policy or instructions into something else.

### Updates

12. As a developer, I want `/manifest status` to show the active release, the newest release and what changed, so that I decide when to adopt it.
13. As a developer, I want to opt into applying manifest releases without a prompt, so that I stay current without extra steps.
14. As a security reviewer, I want any release change that grows permissions to ask before it applies, even with automatic application on, so that new tools, wider bounds or removed locks are never adopted silently.
15. As a developer, I want ordinary package updates and `maestro update` to keep working unchanged in manifest mode, so that my own packages update as before.
16. As an administrator, I want a release to declare a minimum release, so that an urgent fix reaches every laptop at its next session start.
17. As an administrator, I want running sessions to check for new releases at an interval I set, so that long-running sessions hear about a fix without a restart.
18. As a developer, I want `/manifest update <version>` to move to any release of the same manifest, older or newer, so that I can roll back a bad release.
19. As an administrator, I want a rollback never to go below the minimum release, so that a known-bad release cannot come back.

### Prompts and instructions

20. As an administrator, I want the manifest to supply the base system prompt, so that every agent starts from the organization's prompt.
21. As an administrator, I want the main agent's persona appended to the system prompt, so that its behaviour is consistent.
22. As a developer, I want my own `SYSTEM.md`, `APPEND_SYSTEM.md` and prompt flags to keep working when the manifest does not lock them.
23. As an administrator, I want to lock the system prompt or the appended prompt, so that local files and flags cannot replace them.
24. As a standards owner, I want standards instructions always applied, so that no local file, flag or setting can drop them (settled with the rules system).
25. As a developer, I want my project's `AGENTS.md` and ancestor context files to keep loading as before, so that repository guidance still applies.
26. As a developer, I want to pick instruction files from the manifest and have them copied into my `.maestro/instructions/`, so that I install only the guidance I need.

### Resources, packages and capabilities

27. As a team, I want to ship skills, prompt templates, extensions, themes and workflows in the manifest, so that developers get them without installing packages.
28. As a developer, I want to add my own skills, templates, workflows and extensions and to shadow optional manifest ones, so that I can personalise and experiment.
29. As an administrator, I want required resources that cannot be disabled, shadowed or dropped by discovery flags, so that mandatory capabilities stay active.
30. As an administrator, I want to forbid local additions of any resource kind
    (extensions, skills, prompt templates, themes, packages, workspace actions,
    browser contributions, workflows, roles, instructions and MCP servers), so that
    only reviewed code and instructions run.
31. As a developer, I want `maestro config` to toggle optional manifest resources with the same filters I use for packages, so that I control my working set.
32. As an administrator, I want the manifest to declare packages that every developer gets, so that shared packages need no per-developer install.
33. As a team, I want my area to be one capability that bundles roles, skills, instructions, prompt templates and workflows, so that a developer selects it by name.
34. As an organization, I want capability folders grouped the way our source-hosting organization is grouped, so that folder ownership and access follow our existing teams.
35. As a developer, I want to pin any package to any version and take single items from it with the ordinary filters, so that I can hold one skill or workflow at a known version.

### Models, thinking and tools

36. As an administrator, I want to supply custom providers and models in the manifest, so that developers reach the organization's endpoints.
37. As a security reviewer, I want manifest provider keys to be environment or command references, never literal secrets, so that no secret is published in the manifest.
38. As a developer, I want my own `models.json` to add providers and partially override manifest models without losing their other fields, unless providers are locked.
39. As an administrator, I want an allowed set of models, so that every selection path (default, flags, `/model`, cycling, RPC, extensions, SDK and resumed sessions) stays inside it.
40. As an administrator, I want a default thinking level with a minimum and a maximum, so that developers may raise thinking up to the maximum but not beyond it.
41. As a developer, I want a stored default outside the policy resolved to an allowed value with a diagnostic, so that an old setting never blocks my start.
42. As a developer, I want an explicit request outside the policy refused before anything changes, with the lock or bound that forbids it, so that I know why it failed.
43. As a developer resuming a session, I want a recorded model outside the policy replaced by the policy's choice with the usual fallback message, so that edited session files cannot bypass the policy.
44. As an administrator, I want to lock or bound agent-behaviour settings (transport, steering, follow-up, compaction, branch summary, retry, skill commands, image resizing, thinking budgets), so that agent behaviour and spend are governed.
45. As a privacy officer, I want to force image blocking and disable install telemetry, so that no environment variable or setting turns them back on.
46. As an administrator, I want to lock any presentation key, one keybinding
    action, a machine path or the session directory, whether the changelog shows
    at start, and the credential sources allowed per provider, so that every
    governed surface can be fixed while the manifest still never holds a secret. A
    developer keeps each surface until it is locked, and a lock on one leaves the
    others free.
47. As an administrator, I want the main agent's tools limited by the manifest's role definition, so that developers can drop tools but never add tools beyond it.
48. As an administrator, I want local extensions unable to replace a locked prompt, activate forbidden tools, register providers under a provider lock or change model and thinking outside the policy.

### Workspace daemon

49. As an administrator, I want workspace settings that run commands or reach the network to obey the manifest's locks in the workspace daemon too, so that a lock holds in every Maestro process.
50. As a developer, I want project settings never to change the shared workspace daemon, so that one project cannot change every workspace.

### Workflows

51. As a developer, I want to write a workflow as one YAML file in my configuration root or project, so that I can run a multi-step agent process without a manifest.
52. As a team, I want the same workflow file to work unchanged in a capability, so that a local workflow becomes a team workflow by pull request.
53. As a developer, I want the manifest to provide the reference chains (implement and review, implement, scout and plan) as workflows, so that I start with working examples.
54. As a developer, I want to ask Maestro to write or change a workflow and have it check the file, so that I never need to learn the format first.
55. As an administrator, I want to require a workflow and everything it references, so that a project cannot replace its roles or prompts.
56. As a workflow author, I want literal text to stay fixed and positional arguments to be supplied per run, so that I decide what a run may change.
57. As a developer, I want a failed step to stop the workflow and say which step failed, so that later steps never run on bad input.

### Visibility, catalog and documentation

58. As a developer, I want the interactive startup to show the active manifest source, its release and the settings file that names it, so that I know which configuration is in force.
59. As a developer, I want `/settings` and `maestro config` to mark manifest-set and locked values, and never offer an edit the policy would refuse.
60. As an RPC or JSON client author, I want the protocol unchanged, so that clients keep working in both modes.
61. As an SDK author, I want internal-mode construction and every injectable component unchanged, so that embedding programs behave as before.
62. As a developer, I want to browse the manifest's capabilities, roles, workflows, instructions and policy in the browser catalog, with each entry's source, lock state and owning team, so that I can find what my organization offers.
63. As a developer, I want to ask Maestro about any part of the manifest and get an answer from its documentation, so that I never have to read the repository to understand it.
64. As a manifest author, I want documentation for every artifact kind, at the same level as Maestro's own documentation, so that contributors write correct files.
65. As a manifest maintainer, I want pull-request checks that load the release the same way a session does, so that a broken release never gets tagged.
66. As a manifest author, I want a published schema for every artifact kind, checked before I can commit and again in required CI, so that a malformed file never reaches the main branch.
67. As a developer, I want the catalog to show every manifest entry as a card with its relations, its dependency graph and its owning team, so that I can see who owns what and what depends on it.

### Reload, switching and roles

68. As a developer, I want `/reload` to pick up manifest and settings changes, and to keep my current configuration with an error when the new manifest is broken.
69. As a developer switching to a project that names another manifest, I want the new session built from that project's manifest, so that each project gets its own configuration.
70. As a team, I want to select the language areas and capabilities a project uses, so that developers load only what is relevant to them.
71. As an administrator, I want language areas and capabilities to add or narrow but never loosen the organization's constraints, so that a team cannot unlock a value.
72. As a role author, I want manifest roles with model, thinking and tool limits for authored agents, so that delegated agents follow the same policy (delivered with the authored-agent consumer).
73. As a developer, I want to raise a role's thinking or choose its model within the manifest's bounds through my settings, so that I tune roles without editing the manifest (delivered with the authored-agent consumer).
74. As an administrator, I want the manifest to supply MCP servers that local gateway options cannot drop, with credentials kept local (delivered with the MCP base package).

### Guards

75. As a security reviewer, I want every guard rule to be a regular expression in a manifest file, so that I review and approve each rule by pull request.
76. As a security reviewer, I want each rule to carry text it must match and text it must not match, checked at every load and in required CI, so that I see proof that the expression does what it says.
77. As an administrator, I want commands that match a destructive-command rule to ask before they run and to be refused when no one can answer, so that an agent cannot delete or overwrite by accident.
78. As an administrator, I want writes and edits to protected paths refused, so that agents never change secret files or repository internals.
79. As a privacy officer, I want personal data replaced by a marker in tool output and prompts before the model sees them, and in agent output before it is saved or sent again, so that it never reaches a provider or a session file through Maestro.
80. As a security reviewer, I want tool output that matches a prompt-injection rule withheld from the model with the reason, so that fetched content cannot instruct the agent.
81. As a developer, I want guards only when a manifest is selected, so that Maestro without a manifest behaves as before.
82. As a privacy officer, I want guarded replies to appear only once they are complete and checked, so that personal data never shows on screen or in event streams.
83. As a privacy officer, I want every saved session cleaned when guard rules first apply and whenever they change, so that no personal data stays at rest.

## Behaviour contract

### M1 — Selection

1. The top-level `manifest` setting names the manifest. Its value takes the package
   entry form of `packages`: a source string, or an object with `source` and the
   optional `extensions`, `skills`, `prompts` and `themes` filters. The source uses
   the package source grammar and its parser.
2. An absent key, an empty string or an object with an empty `source` means internal
   mode.
3. The source must be a Git source pinned to a ref (a tag or a commit), so that every
   start can verify the release (M2.5). An npm source, a local path, an unpinned
   source or a wrong-typed value is an invalid selection (M9). The source names the
   whole repository and no folder in it: the manifest root is fixed (M3).
4. The manifest package reads the selection from the raw user and project settings
   documents through the settings readers (H5), before any manifest loads. With one
   value, that value is used. With two values naming the same identity, the project
   value is used (the ordinary project-wins order). With two values naming different
   identities, the user's manifest is loaded first: if it locks `manifest`, the
   project value is ignored with a diagnostic; otherwise the project value is used.
   If the user's manifest cannot be loaded in that case, the selection is broken (M9).
5. Identity follows package identity: the Git repository URL without its ref.
6. A lock on `manifest` fixes the identity. Changing the identity or removing the
   value through Maestro is refused. Changing the version of the same identity is
   allowed only through `/manifest update` (M11).
7. The manifest installs in the scope of the settings file that names it: user
   scope for the user file, project scope for the project file. The selected
   `manifest` value is never written into `packages`.
8. `MAESTRO_CODING_AGENT_DIR` keeps relocating the user configuration, including a
   user selection. No flag selects a manifest.

### M2 — Delivery and acquisition

1. Acquisition is ordinary package acquisition for the selected source and scope,
   requested by the manifest package through the package operations (H5), with the
   package owner's progress, offline handling, configured npm and Git commands, and
   installation paths. There is no second downloader, cache or updater.
2. A missing release installs while the manifest package loads. With
   `MAESTRO_OFFLINE`, or when installation fails, the release stays missing and the
   manifest package fails its load (M9).
3. A pinned Git release whose checkout is not at its ref is checked out at the ref
   (corrected defect). A pin change made by a pull request therefore takes effect at
   the next start.
4. A release whose manifest root contains no resources is valid; absence of
   files is not a missing release. A release with no manifest root is invalid
   (M3, M9).
5. **Integrity.** At every session start the manifest package checks that the
   release checkout is clean and at its pinned commit: the working tree matches the
   commit, with no modified, added, deleted or untracked files, and the checked-out
   commit is the one the ref names. When the checkout has drifted, the package
   restores it to the pinned commit with the package owner's Git commands through
   the package operations (H5), then shows `Manifest release restored: <release>`
   with the files it reset or removed. Drift never stops a session: when the
   pinned commit cannot be restored from the checkout, the release is installed
   again (M2.2); when that is impossible too, for example offline, the session runs
   on the manifest package's last accepted composition (M9) with a warning naming
   the drift, and the next start repairs it. An edit to the organization's files is
   therefore never kept and never silent.

### M3 — Layout and reading

A release keeps its manifest content in one folder, `manifests/`: the **manifest
root**. Maestro reads nothing outside the manifest root, except the catalog's
ownership lookup in the release's `CODEOWNERS` (M16.3, M17.3), which is repository
data and never manifest content. Documentation, schemas, catalog blueprints, CI
files and the README are repository tooling outside the root and are never read as
manifest content. Every other path in this specification that names manifest
content is relative to the manifest root. One repository holds one manifest, with
one manifest root, and the `manifest` setting stays a plain pinned Git source (M1).

The manifest is read as areas, found by location under the manifest root. Maestro
reads no area descriptor; ownership records in a manifest repository are
repository data.

| Area | Location under the manifest root | Loaded |
|---|---|---|
| Common | the manifest root itself | always, first |
| Standards | `standards/<domain>/` | always, every domain, in directory-name order |
| Core | `core/` | always |
| Languages | `languages/<name>/` | when listed in `manifestAreas`, in list order |
| Capabilities | `capabilities/<group>/<name>/` | when listed in `manifestAreas`, in list order, after languages |

Every area may contain:

| Content | Meaning |
|---|---|
| `skills/`, `prompts/`, `extensions/`, `themes/`, or a package-manifest `maestro` key | Package resources, read by the package collector (M6) |
| `instructions/*.instructions.md` | Instruction files with `applyTo` frontmatter, offered for installation (M5.3) |
| `agents/*.md` | Role definitions in the authored-agent frontmatter format: `name`, `description`, `tools`, `model`, plus `thinking`; the body is the role prompt |
| `workflows/*.yaml` | Workflows ([M15](#m15--workflows)) |
| `settings/settings.json` | A settings layer using the ordinary settings keys (M4) |
| `settings/policy.json` | Policy (M4) |

Only the common area may contain `CHANGELOG.md`, the release's changelog (M10.6,
M11.3).

Only the core area may contain `SYSTEM.md` (base system prompt), the role named
`maestro` (the main agent's persona and tools), `llm/models.json` (model file in the
ordinary models format), `backends/mcp/mcp.json` (MCP servers, M13) and
`guards/*.yaml` (guard files, [M20](#m20--guards)).

A capability is one package: its folder holds a `package.json` and its own
resources ([M16](#m16--capabilities-and-ownership)). `<group>` and `<name>` are
lowercase, hyphenated and at most 64 characters each.

`manifestAreas` is a list of `languages/<name>` and `capabilities/<group>/<name>`
entries, relative to the manifest root. Its value resolves like any setting: the
manifest's common, standards and core settings give the default, then user, then
project. A listed area that does not exist, an entry with another prefix, or a
`manifestAreas`, `manifest`, `minimumRelease` or `updateCheckMinutes` value or
constraint inside a language area or capability makes the manifest invalid.
Duplicate entries load once.

Settings layers combine in area order with the ordinary one-level merge. The
manifest's settings must not set the resource path keys `extensions`, `skills`,
`prompts` or `themes`; area folders supply those resources. Its `packages` entries
are the manifest's required packages (M6).

### M4 — Layers and policy

Resolution order, lowest to highest: engine defaults; manifest (areas in M3 order);
user; project; run (flags and SDK options); runtime changes. Raw user and project
settings stay raw: the manifest layer is never written into them, and their
snapshots, unknown members, pending edits and reload behaviour are unchanged. The
manifest layer and its locked keys reach the settings owner as the base layer (H1).

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
  "required": ["core/skills/review", "core/workflows/secure-release.yaml"],
  "roles": {
    "coder": {
      "locked": ["model"],
      "bounds": { "thinking": { "min": "high", "max": "xhigh" } },
      "models": ["anthropic/*"]
    }
  },
  "credentialHeaders": ["X-Org-Token"],
  "minimumRelease": "1.5.1",
  "updateCheckMinutes": 30
}
```

| Member | Class | Meaning |
|---|---|---|
| `locked` | L | Each entry is a settings key path (dotted for nested members), one of the lock names `systemPrompt`, `appendSystemPrompt`, `providers`, `workflows`, `agents`, `instructions`, `mcpServers`, or one of three paths that exist only in the base layer because their local form is not a setting: `keybindings.<action>`, `showChangelog` and `credentials.<provider>`. The locked value is the manifest's settings value, or the engine default when the manifest sets none; a `manifest` lock fixes the manifest's own identity (M1). |
| `bounds` | B | `min` and/or `max` for numeric keys and for thinking levels (order: off < minimal < low < medium < high < xhigh). |
| `models` | B (allowed set) | Model patterns in the `enabledModels` syntax. Every model selection must match. |
| `required` | A | Paths relative to the manifest root. A resource at or under a required path is required. A required workflow also requires its references (M15). How required instructions stay present is settled with the rules system. |
| `roles` | L/B per role | `locked` (`model`, `thinking`, `tools`), `bounds.thinking`, `models` for one role (M12). |
| `credentialHeaders` | Credential classification | Header names added to the default credential list (M7); additions from every selected area combine. |
| `minimumRelease` | Release floor | The lowest release a laptop may run (M11). A release version; Git tags compare as semantic versions after an optional leading `v`. Common and core areas only. |
| `updateCheckMinutes` | Check interval | Minutes between release checks in a running session (M11). A non-negative integer; default 30; `0` checks at session start only. Common and core areas only. |

Every configuration surface a manifest can govern is lockable. A lock never
loosens anything: it fixes a value or refuses local additions, and every other rule
(bounds, allowed models, required entries, and the stored-value and
explicit-request rules below) still applies. Governed keys and what the manifest
may do with them:

| Keys and lock names | Manifest may |
|---|---|
| `defaultProvider`, `defaultModel` | lock; constrain through `models`; default |
| `defaultThinkingLevel` | lock; bound; default |
| `enabledModels` | lock; default (the resolved scope is always intersected with `models`) |
| `thinkingBudgets.{minimal,low,medium,high}` | lock; bound (maximum); default |
| `transport`, `steeringMode`, `followUpMode`, `enableSkillCommands`, `images.autoResize` | lock; default |
| `compaction.{enabled,reserveTokens,keepRecentTokens}`, `branchSummary.{reserveTokens,skipPrompt}`, `retry.{enabled,maxRetries,baseDelayMs}`, `retry.provider.{timeoutMs,maxRetries,maxRetryDelayMs}` | lock; bound (numeric members only); default |
| `images.blockImages`, `enableInstallTelemetry` | lock (normally to `true` and `false`); default |
| `holdUncheckedText` | lock; default (locked to `true` whenever a guard rule targets `agent-output` or `tool-output`, M20.7) |
| `packages`, `extensions`, `skills`, `prompts`, `themes`, `actions`, `contributions`, `workflows`, `agents`, `instructions`, `mcpServers` | lock (no local additions); require entries |
| `manifest`, `manifestAreas`, `manifestAutoUpdate` | lock; default (`manifestAreas`, `manifestAutoUpdate`) |
| `multiplexer.tabBarRight` | lock; default |
| `multiplexer.allowNested` | lock; default |
| `daemon.resumeAgentsOnRestore` | lock; default |
| `daemon.detectionCatalogCheck` | lock; default |
| `multiplexer.remote.manageSshConfig` | lock; default |
| `update.versionCheck` | lock; default |
| `update.channel` | lock; default |
| `shellPath`, `sessionDir`, `daemon.paneShell`, `daemon.worktreesDirectory` | lock (a location, written in the key's own path syntax; the manifest holds a value only as the lock's value) |
| `npmCommand` | lock (the package-manager argument vector, program and arguments; the manifest holds a value only as the lock's value) |
| `keybindings.<action>` | lock (one named action; other actions stay free) |
| `showChangelog` | lock (whether the changelog shows at start) |
| `credentials.<provider>` | lock (the credential sources allowed for one provider) |
| `theme`, `themes`, `shellCommandPrefix` and every other key of the settings schema that no row names (the presentation keys) | lock; default |

`actions` and `contributions` are governed once their resource kinds are collected.
The kinds `workflows`, `agents`, `instructions` and `mcpServers` are lock names, not
settings keys: their local form is a folder or a configuration file. `themes` is a
resource-path key: a manifest sets no value for it (M3), and its lock stops local
themes like the other kinds in its row (M6.4). Two keys take no `locked` entry: `lastChangelogVersion`, which is state
Maestro writes, and `roles.<role>.{model,thinking}`, which the policy's `roles`
member governs (M12). Guard rules take none either (M20).

Invariants: a lock on an unknown key or on a key without a lock meaning, and a bound
on a non-numeric, non-thinking key, make the manifest invalid; so does a manifest
value for a location or command key (`shellPath`, `npmCommand`, `sessionDir`,
`daemon.paneShell`, `daemon.worktreesDirectory`) that `locked` does not name; a
lock name outside the `locked` row is an unknown key. The manifest package holds
the governed-key table; the core holds none.

#### Lock meanings

Each lock below applies the stored-value and explicit-request rules that follow
this section; the columns say what they mean for that lock. A diagnostic and a
refusal name the lock and the manifest. The first four rows extend the
local-addition locks of M6.4 to kinds that have their own owners.

| Lock | Meaning | Stored values | Explicit requests | Owner |
|---|---|---|---|---|
| `workflows` | Only the manifest's workflows exist. A required workflow is the by-value lock: it cannot be shadowed (M15.6). | Workflow files in the configuration root's `workflows/` and the project's `workflows/` are ignored with a diagnostic. | An option that adds a workflow file refuses the run. Running a manifest workflow is unaffected. | Workflow runner of the subagent capability |
| `agents` | Only the manifest's roles exist. A required role is the by-value lock: it cannot be shadowed (M12.1). | Role files in the configuration root's `agents/` and the nearest project `agents/` are ignored with a diagnostic, so a workflow's `agentScope` resolves manifest roles only. | An option that adds a role file refuses the run. | Subagents package |
| `instructions` | Only the manifest's installed copies are instruction files. A required instruction is the by-value lock: its copy stays present and refreshed (M5.3). | Instruction files in the configuration root's `instructions/` and the project's `instructions/` that the manifest package did not install are ignored with a diagnostic. Context files (`AGENTS.md`, `CLAUDE.md`) are not instruction files and still load. | An option that adds an instruction file refuses the run. `/manifest install` of the manifest's own file stays allowed. | Rules system loads; the manifest package installs and refreshes |
| `mcpServers` | Only the manifest's servers exist. A required server is the by-value lock: it cannot be dropped (M13). | Servers defined in the user's and the project's gateway configuration are ignored with a diagnostic. | A gateway option that adds a server refuses the run. | MCP package |
| presentation keys (`theme`, `themes`, `shellCommandPrefix` and the rest) | The key has the manifest's value, or the engine default when the manifest sets none. | A user or project value is replaced by the locked value with a diagnostic. | `/settings`, a setter or a flag that sets a different value is refused. | Settings owner (H1); frontends render |
| `keybindings.<action>` | The binding of one named action is the manifest's (a string or a list of strings, the forms `keybindings.json` accepts), or the action's default when the manifest sets none. Other actions stay free. | The user's `keybindings.json` entry for that action is ignored with a diagnostic; every other entry applies. The file is never rewritten. | An SDK or extension call that sets that action's binding is refused. | Application's keybinding loader; frontends render |
| `shellPath`, `sessionDir`, `daemon.paneShell`, `daemon.worktreesDirectory` | The program or folder is the manifest's value, written in the syntax the key already reads: an absolute path, and `~` or `~/` where the key accepts home-relative paths. For `sessionDir`, new session files are stored only there. | A user or project value is replaced with a diagnostic. Session files stored elsewhere stay where they are. | A flag or environment override that names a different value is refused (`--session-dir` and `MAESTRO_CODING_AGENT_SESSION_DIR` for `sessionDir`), and so is `--session <path>` naming a file outside the locked folder. A location equal to the locked one after the key's own expansion is accepted; the comparison is lexical. | Settings owner (H1); the session manager and the process launchers read the effective value; the daemon reads it through H3 |
| `npmCommand` | The package-manager command is the manifest's argument vector: the program and every argument after it, for example `["mise", "exec", "node@20", "--", "npm"]`. The program is resolved as the key resolves it today, so it need not be an absolute path. | A user or project vector is replaced as a whole with a diagnostic; arguments are never merged or dropped. | A setter, settings edit or SDK call that sets a different vector is refused. A vector equal to the locked one, entry by entry and in order, is accepted. | Settings owner (H1); the package manager reads the effective vector |
| `showChangelog` | The changelog entries since the last run show at interactive start (`true`, as today) or never show (`false`). `lastChangelogVersion` is state Maestro writes, not a preference, so a lock on it is invalid; this is the nearest useful lock, and the state stays Maestro's and is written as before. | The key exists only in the base layer; the user's files cannot set it. | A settings edit of the key is refused. `/changelog` still shows the full changelog on request under either value. | Settings owner (H1); the interactive start reads the effective value |
| `credentials.<provider>` | The sources Maestro may use for one provider: any of `environment` (the provider's environment variable), `command` (a key produced by a `!` command), `signIn` (sign-in tokens) and `storedFile` (a literal key, whether stored by login, written in the model file or passed with `--api-key`). Secrets are never manifest values, so this is the nearest useful lock. A value naming a secret or another source makes the manifest invalid. | A stored record or environment variable of an excluded source is ignored with a diagnostic. The credentials file is never rewritten by a read. | Storing a key or tokens of an excluded source (a pasted key when only `signIn` is allowed), `--api-key` when `storedFile` is excluded, and an SDK or extension write of one are refused before any file changes. | Credentials owner (`AuthStorage`), asked through the decision hook (H2) |

Guard rules are locked both ways without an entry: every rule comes from the
manifest (by value), and no local guard file or setting exists to add one (M20).

Combining areas, in area order: locks join, and a later area that sets a different
value for a key an earlier area locked, or locks it with a different value, makes
the manifest invalid. Minimums take the largest, maximums the smallest, allowed
model sets intersect, required paths join. A combined bound with minimum above
maximum, or a manifest settings value outside its own combined constraints, makes
the manifest invalid. Language areas and capabilities can therefore only add or
narrow.

Stored values and explicit requests differ:

- **Stored values** (user and project files, restored sessions): a locked key uses
  the manifest value; a bounded value is clamped to the bound; a model outside
  `models` is replaced by the manifest default when it is allowed, otherwise by the
  first available model, in catalog order, matching the earliest pattern that
  matches any available model. Each produces a diagnostic through the channel that
  reports settings load errors. The stored file is never rewritten by a read.
- **Explicit requests** (setters, settings edits, flags, SDK options, RPC commands,
  `/model`, cycling, extension calls, environment overrides of governed values,
  package install and remove, resource toggles) that a lock, bound, allowed set or
  required entry forbids are refused before their first effect, with a message
  naming the constraint and the manifest. A refused request changes no memory, no
  pending edit, no file, no session history and emits no change event. An allowed
  request runs the foundation's steps in the foundation's order.

The manifest package decides governed changes through the decision hook (H2).
Every owner asks the hook before its own effect, except package commands, which
run before packages load. Before any package effect, they use
the persisted base layer for the operation's scope (H1): `install` refuses when
`packages` is locked, and `remove` refuses an identity listed in that layer's
`packages` entries, before deleting any file.
The package commands reuse the core's base-layer enforcement (H1); they hold no
manifest policy logic.

A lock behaves the same in every Maestro process. The settings owner enforces the
locked keys of one base layer in agent sessions and in the workspace daemon alike;
the daemon only reads the persisted global layer, because it hosts no extensions
(H1, H3).

### M5 — Prompts and instructions

1. **Base system prompt.** Candidates, highest first: explicit (flag or SDK option or
   SDK override callback), project `SYSTEM.md`, user `SYSTEM.md`, manifest
   `core/SYSTEM.md`, engine default. The manifest package contributes its candidate
   as a base prompt input (H4). A custom prompt still replaces the default
   documentation block. Under a `systemPrompt` lock, local files are ignored with a
   diagnostic and an explicit replacement is refused.
2. **Appended prompt.** The manifest append (the body of the core `maestro` role) is
   always first, followed by the ordinary choice: every explicit append in order,
   otherwise the discovered `APPEND_SYSTEM.md`. Under an `appendSystemPrompt` lock,
   local append files are ignored with a diagnostic and explicit appends are refused.
3. **Instructions.** An area's `instructions/*.instructions.md` files are offered,
   not loaded. `/manifest install <name>` copies the chosen file into
   `~/.maestro/agent/instructions/`, or with `-l` into the project's
   `.maestro/instructions/`, the same scopes as package installation. The manifest
   package records, in its own package state, the release commit and the file
   digest each copy came from. At every session start a copy whose content no
   longer matches its release file is refreshed from the release and listed in the
   same restore notice (M2.5); when a newer release changes an installed file, the
   copy is updated with it.

   Instructions are locked both ways. **By value:** an instruction at or under a
   `required` path has its copy installed, in the selection's scope, whenever it is
   missing at session start, listed in the same notice, and refreshed like any
   installed copy; no local file, flag or setting removes it. **No local
   additions:** an `instructions` lock stops local instruction files other than the
   installed copies (M4). The rules system honours both: it loads every required
   copy, even when a flag or setting turns instruction loading off, and under the
   lock it loads only installed copies. The manifest package installs and
   refreshes; it does not load.

   Loading instruction files, `applyTo` and always-apply rules belong to the rules
   system ([Out of scope](#out-of-scope)); the context-file loader and
   `--no-context-files` keep their foundation meaning here.
4. Date, cwd, skills block, tool snippets and guidelines keep their existing
   assembly.

### M6 — Resources and packages

1. Each selected area root is contributed as a package-origin source with the
   selection's source string and scope (H4), so manifest resources rank below
   project and user resources. Their base directory is the manifest root, so
   filters and `maestro config` patterns are relative to the manifest root.
2. The selection's object filters apply with the ordinary filter semantics. A filter
   that disables a required resource is ignored with a diagnostic; `maestro config`
   refuses to write one.
3. A required resource wins its name collision; the local resource is reported as
   the collision loser. Discovery-disabling flags (`--no-skills`,
   `--no-prompt-templates`, `--no-extensions`, `--no-themes`) and SDK override
   callbacks never remove a required resource; a required resource missing after
   overrides refuses construction.
4. A lock on `extensions`, `skills`, `prompts`, `themes`, `packages`, `actions`,
   `contributions`, `workflows`, `agents`, `instructions` or `mcpServers` stops
   local additions of that kind: settings entries and automatic user and project
   folders and files are ignored with a diagnostic, and explicit flags and options
   that add one (`-e`, `--skill`, `--prompt-template`, `--theme`) refuse the run.
   Forbidden local components are excluded before they execute, because base
   packages load first (H4). The last four kinds have owners beyond the resource
   loader; [Lock meanings](#lock-meanings) names each.
5. The manifest's `packages` entries are required packages. They install in the
   selection's scope and are collected after the area roots at package precedence,
   in the same startup (H4). A missing required package refuses sessions. Removing
   a package whose identity matches a required package in the same scope is refused
   before any file is deleted. With no persisted layer, package commands retain
   foundation behaviour; the next session start installs a removed required
   package again. A forbidden local addition is excluded before execution (M6.4).
   Startup installs include the transitive required-package closure through the
   same package-manager install operation as explicit selections (M16.5), in the
   selection's scope, with one operation and one settings write. Satisfying
   installed packages in that scope are reused. The whole closure passes source,
   version and cycle checks and the applicable M4/M6.4 policy before any install;
   a missing requirement or any refusal installs nothing and refuses the session
   with its reason. Required manifest packages remain protected from removal,
   including combined removal (M16.5).
6. Optional resources that fail to load are skipped with the ordinary warning. A
   required resource that fails to load refuses sessions.
7. Themes from the manifest are offered; the manifest's `theme` value is a default
   the user may change unless `theme` is locked.

### M7 — Models and thinking

1. **Model files.** The manifest `core/llm/models.json` is contributed as a model
   file read before the user's `models.json` with the same parser (H4). A user entry
   that partially overrides a manifest provider or model keeps every manifest field
   it does not set; a user model with the same provider and ID replaces the manifest
   model. Refresh and dynamic provider removal restore the correct underlying
   layers. Under a `providers` lock the user file is ignored with a diagnostic and
   extension provider registration is refused. There is no project models file.
2. A manifest `apiKey` or credential-header value must be a command (`!…`) or an
   environment variable name (upper-case letters, digits, underscores); any other
   value in those positions makes the manifest invalid. Credential header names
   match case-insensitively; the default list is `Authorization` and
   `Proxy-Authorization`, extended by `credentialHeaders` in the manifest's policy.
   Other headers accept literal values, including `Content-Type: application/json`,
   and keep ordinary environment and command resolution. Local credentials and key
   flags are unchanged, except that a `credentials.<provider>` lock excludes the
   sources it does not list (M4).
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
| Invalid selection (not pinned, local, wrong type) | The manifest package fails its load; agent sessions refuse |
| Release missing and not installable (offline, network, authentication) | The manifest package fails its load; agent sessions refuse |
| Invalid manifest: no manifest root, malformed settings or policy, unknown or conflicting lock, impossible bound, invalid role file, invalid model file, invalid workflow, invalid guard file, literal secret, invalid `manifestAreas` | The manifest package fails its load; agent sessions refuse |
| Missing required package, or a required resource that fails to load | Agent sessions refuse |
| Optional resource that fails to load | Skipped with the ordinary warning |
| A kind absent from the manifest | No manifest layer for that kind; engine defaults apply |
| Stored value conflicting with policy | Resolved as in M4 with a diagnostic |
| Explicit request conflicting with policy | Refused as in M4 |

A failed load uses the foundation's extension-load failure: the error is reported as
`Failed to load extension "<path>": <message>` and the program exits with status 1
in every mode. The message names the settings file that selects the manifest, the
source, the problem and the fix, and no model request is made. For a release with
no manifest root the problem reads `release <ref> has no manifests/ folder`. There
is no silent fallback to internal mode or to an earlier copy; unrepairable drift
uses the last accepted composition with a warning (M2.5).

Before failing its load at session start for a broken pinned release, the manifest
package checks for a newer release of the same identity. If that release loads
cleanly, it applies under the required-release rules (M11.4), including permission
growth confirmation. Growth is compared with the last accepted composition of that
identity; without one, the candidate's whole permission set counts as growth and
is listed for confirmation. Without an interactive terminal, the package refuses
a candidate with growth and reports the fix. If no candidate can be applied, the
load fails with the fix. Checks still obey `MAESTRO_OFFLINE` and
`update.versionCheck` (M11.2).

Package commands and `config` run before packages load. They use the last
persisted layer for their scope (H1), even when the manifest is broken; with none,
they retain foundation behaviour. A changed selection keeps the old layer until
the next session start refreshes it. `--version`, `list`, viewing config and
editing unlocked settings remain available. `--help` runs after runtime creation
but before load errors are reported, so a broken manifest does not prevent help.

Every construction of session services (start, new session, resume, fork, import,
switching to another working directory) recomposes the manifest and refuses on
failure. `/reload` recomposes too; when recomposition fails during a reload, the
manifest package keeps publishing the composition it accepted before, reports the
manifest error with its fix through the extension error path, and the session keeps
running under that composition. The manifest package keeps its last accepted
composition in its own package state for reload, drift recovery and repair
permission comparisons; a start runs on it only for unrepairable drift (M2.5).

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
6. `/manifest status` shows the active release, the settings file that selects it,
   the newest known release, the minimum release, and the changelog entries between
   the active and newest releases read from the `CHANGELOG.md` in the candidate's
   manifest root.
7. RPC and JSON output gain no fields.

### M11 — Updates

1. `maestro update`, `--extensions` and `<source>` keep updating unpinned packages and
   skipping pinned ones; the engine self-update is unchanged. Package commands run
   before packages load and enforce the persisted base layer (M4, H1), without
   inspecting the manifest.
2. **Checks.** The manifest package checks the selected manifest for newer releases
   at every session start and then every `updateCheckMinutes` minutes while the
   session runs (`0`: session start only). Checks follow `update.versionCheck` and
   `MAESTRO_OFFLINE`; offline keeps the current pin with a warning. A newer release
   shows a notice in that session; a required one says that it applies at the next
   start and that `/manifest update` applies it now.
3. **Inspection.** A check lists the source's releases and reads, from the newest
   candidate's manifest root, the common `settings/policy.json`, the core
   `core/settings/policy.json` and `CHANGELOG.md` through the package operations
   (H5) in the temporary package scope, without the install step. The two policy
   files are optional; their `minimumRelease` values combine in area order under M4
   (the higher floor wins) before the check decides whether the release is
   required. Inspection runs no lifecycle script, build step or component.
4. **Required releases.** When the newest release's `minimumRelease` is above the
   active pin, the release is required. At the next session start it applies
   without asking unless it grows permissions; then the user is asked, and declining
   makes the manifest package fail its load with the fix. Without an interactive
   terminal a required release without permission growth applies; one with growth
   refuses with the fix.
5. **Permission growth** is any change that adds a tool to a role, widens `models`
   or a bound, removes a lock or a required entry, or adds an extension, a package
   or an MCP server. It is computed by composing the current and the candidate
   release and comparing them. A rollback can grow permissions, and is checked the
   same way.
6. **`/manifest update [version]`.** Without a version it takes the newest release;
   with one it moves to that release of the same identity, older or newer, never
   below the newest known `minimumRelease`. It shows the current and target release,
   the changed manifest files, the changelog entries and every permission growth,
   and applies only after the user confirms. With `manifestAutoUpdate: true` an
   optional newer release applies without asking unless it grows permissions. In
   print mode, `maestro -p "/manifest update <version>"` applies a release without
   permission growth and refuses one with growth. The command works under a
   `manifest` lock, because it keeps the identity.
7. **Applying** writes the new pin, same identity, into the settings file that holds
   the selection through the settings owner (H5), installs it through M2 and
   reloads the session. For a project file, the team commits the change by pull
   request. A failed installation reports the real state; there is no rollback
   copy. Rejection or cancellation leaves the active files, pin and settings
   byte-identical.
8. A pinned manifest never moves on its own outside M11.4, except to repair a
   broken pinned release at session start (M9). Repair uses a clean newer release
   of the same identity and the same permission-growth confirmation, compared
   against the last accepted composition of that identity. With no accepted
   composition, the candidate's whole permission set counts as growth. Otherwise
   startup never installs a newer optional release.

### M12 — Roles (delivered with the authored-agent consumer)

1. Roles are discovered by the subagents package from the user's `agents/` folder,
   the nearest project `agents/` folder and the selected manifest areas. User and
   project roles shadow optional manifest roles by name; required roles cannot be
   shadowed. Under an `agents` lock the user's and the project's role files are
   ignored (M4). Project roles keep the reference's confirmation before they run.
2. A role's `model`, `thinking` and `tools` come from its frontmatter, constrained by
   `roles.<role>` in the policy. `tools` is always narrow-only.
3. The user setting `roles.<role>.model` and `roles.<role>.thinking` override a
   role's values within its policy; these are preferences, never constraints.
4. The main agent's model and thinking come from settings (M4), not from the
   `maestro` role; the `maestro` role supplies the persona (M5) and the tool ceiling
   (M8).

### M13 — MCP servers (delivered with the MCP base package)

The core `backends/mcp/mcp.json`, in the MCP package's own configuration format, is
the gateway's lowest configuration source. The MCP package honours two locks.

- **By value.** A required manifest server cannot be dropped: no exclusive or
  explicit gateway configuration option removes it, and it wins a name collision
  with a local server as any required resource does (M6.3).
- **No local additions.** An `mcpServers` lock stops local server definitions (M4):
  the servers the user and the project configure for the gateway are ignored with a
  diagnostic, and an explicit gateway option that adds one refuses the run.

Credentials for the manifest's servers stay local; a manifest never holds a secret
(M7.2).

### M14 — Authored artifacts

Each artifact kind has one format. `U` is the configuration root
(`~/.maestro/agent` by default), `Pj` is the project's `.maestro` folder and `A` is a
selected manifest area. Every path in the Manifest column is relative to the
manifest root (M3).

| Artifact kind | Format | Local | Manifest | Read by |
|---|---|---|---|---|
| Settings | ordinary settings JSON | `U/settings.json`, `Pj/settings.json` | `A/settings/settings.json` | settings owner |
| Policy | `policy.json` (M4) | none | `A/settings/policy.json` | manifest package |
| Guards | `maestro.guards/v1` YAML (M20) | none | `core/guards/*.yaml` | manifest package |
| System prompt | Markdown | `U/SYSTEM.md`, `Pj/SYSTEM.md` | `core/SYSTEM.md` | resource loader |
| Appended prompt | Markdown | `U/APPEND_SYSTEM.md`, `Pj/APPEND_SYSTEM.md` | body of `core/agents/maestro.md` | resource loader |
| Instructions | Markdown with `applyTo` frontmatter | `U/instructions/`, `Pj/instructions/` (installed copies) | `A/instructions/*.instructions.md` | rules system; installed by the manifest package |
| Context file | Markdown | `U/AGENTS.md` or `CLAUDE.md`, root-to-cwd `AGENTS.md`/`CLAUDE.md` | none | resource loader |
| Role | Markdown with `name`, `description`, `tools`, `model`, `thinking` frontmatter | `U/agents/*.md`, nearest project `agents/` | `A/agents/*.md` | subagents package |
| Skill | `SKILL.md` with frontmatter | `U/skills/`, `Pj/skills/` and the existing aliases | `A/skills/` | resource loader |
| Prompt template | Markdown with frontmatter and argument expansion | `U/prompts/`, `Pj/prompts/` | `A/prompts/` | resource loader |
| Workflow | `maestro.workflow/v1` YAML (M15) | `U/workflows/*.yaml`, `Pj/workflows/*.yaml` | `A/workflows/*.yaml` | workflow runner |
| Extension | built component plus package manifest | `U/extensions/`, `Pj/extensions/`, packages | `A/extensions/` | resource loader |
| Theme | theme JSON | `U/themes/`, `Pj/themes/` | `A/themes/` | resource loader |
| Models | `models.json` | `U/models.json` | `core/llm/models.json` | model registry |

Sessions, caches, sockets and other runtime state stay local and have no manifest
form. Keybindings, machine paths and the session directory have a manifest form
only as the value of a lock (M4). Credentials have none: a credential lock names
sources, never secrets. A file moves between a local folder and a manifest area
without edits. Shadowing replaces a whole definition by name; there is no
field-wise merge of roles or workflows.

### M15 — Workflows

A workflow is a named, ordered or parallel run of steps, written as one YAML file.
It runs on the workflow runner that ships by default with Maestro's subagent
capability; this specification defines the file the runner accepts and where its
files come from. The file has the same shape locally and in a manifest area.

```yaml
apiVersion: maestro.workflow/v1
kind: Workflow
name: implement-and-review
description: Worker implements, reviewer reviews, worker applies feedback
chain:
  - agent: worker
    task: "Implement: $@"
  - agent: reviewer
    task: "Review the implementation from the previous step: {previous}"
  - agent: worker
    task: "Apply the feedback from this review: {previous}"
```

1. **Members.** `apiVersion` (`maestro.workflow/v1`), `kind` (`Workflow`), `name`
   (lowercase, hyphenated, at most 64 characters, equal to the file stem),
   `description`, optional `agentScope` (`user`, `project` or `both`; default
   `user`) and exactly one of `chain` or `tasks`. Unknown members, a wrong
   `apiVersion` or `kind`, a name that differs from the stem, both or neither of
   `chain` and `tasks`, or an empty step list make the file invalid.
2. **Agent steps** take the reference subagent tool's item shape: `agent` (a role
   name), `task` and an optional `cwd`. `chain` runs its steps in order; `tasks`
   runs them in parallel with the reference's limits of eight tasks and four at a
   time, and returns every result.
3. **Text.** In a chain, `{previous}` in a step's text is replaced by the previous
   step's final output, as the reference chain does. The workflow's run arguments
   expand in step text with the prompt-template rules (`$1`, `$@`, `$ARGUMENTS`,
   `${@:N}`, `${@:N:L}`). There is no other expression language. Literal text is
   fixed by the file; arguments are supplied per run.
4. **Added step kinds.** A chain step may instead be `command: [program, arg, …]`, a
   literal argument vector run without a shell, whose output is its standard output
   and which fails on a non-zero exit; or `ask: <message>`, whose output is the
   user's typed answer and which fails when the user cancels or no interactive
   terminal exists. Both expand `{previous}` and arguments in each string.
5. **Failure.** A failed, cancelled or refused step stops the run with
   `Workflow stopped at step <n> (<step>): <message>`; later steps never run. The
   run's output is the last step's output.
6. **Discovery and precedence.** Workflows come from the selected manifest areas
   in area order, then the user's `workflows/`, then the project's `workflows/`. A
   later definition with the same name replaces an earlier one, except that a
   required manifest workflow cannot be shadowed. Under a `workflows` lock only the
   manifest areas contribute and the local files are ignored with a diagnostic
   (M4). Project workflows that name project roles keep the reference's
   confirmation before project roles run.
7. **Required workflows.** A required workflow's roles, prompt files and other
   referenced files resolve only from its own release's manifest root; local
   definitions with the same names do not apply to it.
8. **Running.** The runner lets a user run, list and show workflows with their
   sources, and lets the agent check a workflow file, so a user can ask Maestro to
   write, check and run one. Chains and parallel tasks use the subagent tool's own
   loop: `{previous}` passing, the parallel limits and stop-on-failure. Every step
   runs through the ordinary model, tool and role policy, so a workflow gains no
   power its runner lacks.
9. **Defaults.** The manifest provides the reference's three chains as workflow
   files in its core area, with the reference's roles: `implement-and-review`
   (worker, reviewer, worker), `implement` (scout, planner, worker) and
   `scout-and-plan` (scout, planner). No package bundles workflow files.
10. YAML is parsed by Maestro's existing YAML parser; the runner adds no YAML
    library.

### M16 — Capabilities and ownership

1. A capability is one package inside a manifest release, at
   `capabilities/<group>/<name>/` in the manifest root, with its own `package.json`
   and resources. It is selected by `capabilities/<group>/<name>` in
   `manifestAreas`.
2. The group and name follow the organization's own structure. An organization
   whose source-hosting teams are grouped by area may use that grouping as
   `<group>` and the team as `<name>`.
3. Ownership is repository data that already lives in `CODEOWNERS`. A copy under
   the manifest root would be a second copy of the same data, and GitHub honours
   `CODEOWNERS` only in `.github/`, the repository root or `docs/`, so the file
   stays outside the manifest root. The release's `.github/CODEOWNERS` gives each
   group folder, each capability folder and every other path its owning team.
   Sessions never read it; only the catalog's ownership lookup does (M17.3), the
   one read outside the manifest root (M3).
4. A capability that needs its own release cycle is published as its own package
   and listed in the manifest's `packages` with a pin. Capabilities inside the
   release share the release's version.
5. Any package can be pinned to any version with the ordinary source grammar, and
   single items are taken from a pinned package with the ordinary package filters.
   Filters for package-owned kinds (`workflows`, `agents`) are read
   by the owning package and applied with the package owner's filter semantics
   through a host-backed call (H5).

   A package requirement uses that same source grammar, including pins. The
   existing package-manager install operation resolves the transitive closure for
   the chosen entry in its selected scope, installs each missing package once and
   persists the selection with one settings write. Already installed packages in
   that scope satisfying a requirement are reused; another scope does not satisfy
   it. CLI install, SDK calls, catalog selection (M17.4) and startup required
   installs (M6.5) use this one operation, not a second resolver.

   Before installing anything or writing settings, the operation checks the whole
   closure. A missing requirement refuses with the missing source and its
   requirer. A cycle refuses with the cycle's packages in order, ending at its
   starting package. Requirements on one package that no single version satisfies
   refuse with both requirers and their requirements. An incompatible installed
   version refuses with the installed package and version and the conflicting
   requirer and requirement; it is never silently replaced. Every addition must
   pass M4/M6.4 policy, using H1's persisted layer for early commands; a refusal
   names the lock or other constraint, including a `packages` lock or a source
   outside the allowed set. Each such refusal states its reason and installs
   nothing, leaving settings unchanged.

   The existing remove operation refuses to remove a package required by another
   installed package in the same scope. It names every dependent, transitively,
   and offers one explicit step to remove the package and all those dependents
   together. Only that explicit request performs the combined removal through the
   same operation and one settings write; it first checks policy for every package
   to be removed. Required manifest packages cannot be removed this way. A refusal
   deletes nothing and leaves settings unchanged. CLI and SDK removal use the
   same operation. CLI, SDK and catalog listings use the existing scoped package
   listing and report actual installed state without installing packages. The
   daemon retains H3's global-only policy view and gains no package resolver;
   any package operation exposed through it uses these same operations and the
   applicable persisted global layer.

### M17 — Catalog data (delivered with the catalog)

1. **Blueprints.** The manifest package declares one catalog blueprint per artifact
   kind, as data: manifest release, capability group, capability, policy, role,
   workflow, instruction, alongside the existing package, skill and prompt kinds.
   A blueprint lists the entry's properties (name, description, source area as its
   path under the manifest root, release, required, locked, modified locally) and
   its relations.
2. **Relations.** A release requires packages and contains areas; a capability
   group contains capabilities; a capability provides skills, prompts, roles,
   instructions and workflows; a workflow uses roles and prompts; a policy locks,
   bounds or requires entries; every entry is owned by a team. A package requires
   packages through the same `requires` relation as a release, not a new schema.
   A skill or workflow inherits its package's requirements.
3. **Ownership.** Every entry's owning team comes from the release's `CODEOWNERS`,
   applied the way the source host applies it to the entry's path in the
   repository (the manifest root's folder, then the entry's path under it): the
   last matching pattern wins. An entry with no matching pattern shows as unowned.
   This is the catalog's only read outside the manifest root (M3).
4. **Views.** Each entry gets a card with its properties, owner and relations; the
   catalog draws the dependency graph from the relations, so a team sees what its
   capability provides and what depends on it. Choosing an entry adds its
   transitive required packages through M16.5 in the selection's scope. Before
   installation the catalog lists every package to be added and which entry
   requires it, including transitive requirers, and asks for one confirmation for
   the whole selection. Packages already satisfying requirements are shown as
   reused, not additions. Cancellation changes nothing. After installation it
   reports what was installed; an installation failure reports the real partial
   state rather than claiming success or rolling back. A preflight refusal shows
   its reason and installs nothing.
5. Locked entries are read-only in the catalog, and edits go through their owners.
   When a page opens, the catalog reads the installed release's manifest root and
   the ownership file (item 3); it keeps no copy.

### M18 — Documentation

1. Maestro's documentation gains `docs/manifest.md` (selection, areas, policy,
   updates, failure and repair, the governed-key table, instructions) and
   `docs/workflows.md` (the workflow format, the defaults and how to ask Maestro to
   write one).
2. The agent finds them through skills, the reference's own route for
   on-demand knowledge: the manifest package ships a `manifest` skill and the
   workflow runner a `workflows` skill. Each names its document; in manifest mode
   the `manifest` skill also names the active release's `docs/` folder. The
   system prompt's documentation list is unchanged.
3. Each manifest release documents itself: a `README.md` and one page per artifact
   kind under `docs/`. These sit outside the manifest root and are never read as
   manifest content; the `manifest` skill only points the agent at them.

### M19 — Schemas and checks

1. **One schema per artifact kind.** The manifest package publishes a JSON Schema
   for `settings/policy.json`, the workflow file, the guard file, role and
   instruction frontmatter and a capability's `package.json`. The schemas are
   generated from the same Rust types the package uses to read those files, so the
   published rules and the loading rules cannot drift. Generation uses `schemars`
   and validation uses `jsonschema`, both already pinned in the workspace. YAML and
   frontmatter are converted to their JSON value before validation.
2. **In the manifest repository.** The generated schemas live under `schemas/`,
   outside the manifest root. A pre-commit hook validates every artifact in the
   manifest root against them, so a malformed file cannot be committed. A required
   pull-request check runs the same validation, fails when the committed schemas
   differ from freshly generated ones, and loads the release with Maestro
   (`maestro -p "/manifest status"` on the pull request's commit), so a malformed or
   broken release cannot merge.
3. **Tests.** Every schema has fixtures that must pass and fixtures that must fail,
   covering each member, each wrong JSON kind and each unknown member.

### M20 — Guards

Guards check commands, paths and text against rules kept in the manifest's core
area, and block, confirm, redact or warn. Security reviewers approve every rule by
pull request, and each rule carries the examples that prove what it matches. Guards
run only in manifest mode, in every session that loads the manifest package. There
is no local guard file and no setting that adds, removes or disables a rule; rules
change only with the manifest. Guards are therefore locked both ways without a lock
entry: by value, because every rule comes from the manifest, and against local
additions, because nothing local can add one. `guards` is not a lock name.

1. **Files.** Each `core/guards/<family>.yaml` file in the manifest root has two
   members: `apiVersion` (`maestro.guards/v1`) and `rules`, a non-empty list.
   `<family>` is lowercase and hyphenated, at most 64 characters, and names the
   rule family, for example
   `destructive-commands`, `protected-paths`, `pii` or `prompt-injection`. The
   manifest package reads files in file-name order and rules in list order.

   ```yaml
   apiVersion: maestro.guards/v1
   rules:
     - id: canada-sin
       description: Canadian social insurance number
       regex: '\b\d{3}[- ]?\d{3}[- ]?\d{3}\b'
       action: redact
       appliesTo: [tool-output, agent-output, prompt]
       examples:
         match: ['SIN 046-454-286']
         noMatch: ['order 12345']
   ```

2. **Rules.** Every member is required.

   | Member | Meaning |
   |---|---|
   | `id` | Lowercase and hyphenated, at most 64 characters, unique across all guard files |
   | `description` | One line, shown in every refusal, confirmation and warning the rule causes |
   | `regex` | A regular expression in the syntax of the workspace's `regex` crate. Matching takes linear time in the text's length. Inline flags such as `(?i)` and `(?m)` set case and line behaviour; look-around and back-references do not exist in this syntax |
   | `action` | `block`, `confirm`, `redact` or `warn` (item 4) |
   | `appliesTo` | A non-empty list of targets (item 3) |
   | `examples` | `match`, a non-empty list of strings the expression must match, and `noMatch`, a non-empty list of strings it must not match |

   An unknown or missing member, a duplicate `id`, an expression that does not
   compile, an action that does not allow one of the rule's targets, or an example
   that does not behave as listed makes the manifest invalid (M9). The manifest
   package checks the examples every time it loads the manifest, so the
   pull-request check that loads the release (M19.2) checks them too.

3. **Targets.** Guards never check images.

   | Target | Text checked | Checked |
   |---|---|---|
   | `command` | The `command` argument of the `bash` tool, and the user's `!` and `!!` shell commands | Before the command runs |
   | `path` | The `path` argument of the `write` and `edit` tools, as given | Before the tool runs |
   | `tool-output` | Each text block of a tool result, every string in its details, such as an edit's diff, and the output of the user's `!` and `!!` commands | Before the completed result or output reaches the model, the screen or the session file; partial results and output are withheld (item 7) |
   | `agent-output` | Each text and thinking block of an assistant message | When the message ends, before it is shown, saved or sent again (item 7) |
   | `prompt` | Each text block of a user message from every route: typed prompts, steering and follow-up messages, SDK and RPC prompts, after prompt-template and skill expansion | When the message ends, before it is saved or sent |

   The guard checks the output of the user's shell commands through the existing
   user-shell hook: it supplies the command's execution through the host's local
   shell operation and returns the checked output. An earlier extension that
   handles a user shell command itself takes it outside the guard (item 6).

   Every agent request to the provider is checked again with the same
   `tool-output`, `agent-output` and `prompt` rules through the existing context
   hook. The check covers every message the request carries, including summaries,
   the output of the user's shell commands and sessions saved before the manifest
   was selected.

   Compaction and branch summaries go through the existing session-before-compact
   and session-before-tree hooks, through which an extension supplies the summary.
   When any rule targets `tool-output`, `agent-output` or `prompt`, the guard
   supplies it: it checks the messages to summarize with their targets' rules and
   any previous summary with `agent-output` rules, and checks custom instructions
   from `/compact` or branch summarization with `prompt` rules. It generates the
   summary from the checked text with the compaction-generation host operation,
   checks the result with `agent-output` rules and returns it. This covers manual
   and automatic compaction, split-turn summaries and branch summaries. Because
   guards run last (item 6), the guard's summary replaces a summary an earlier
   extension supplied.
   If generation or checking fails, the guard reports the guard name and cause
   through the existing extension error path, using the same warning channel as
   other extension failures, and returns `cancel: true` instead of throwing. The
   cancellation result gains no reason field; manual compaction keeps its existing
   `Compaction cancelled` text. No unchecked fallback summary is generated, saved
   or shown.

4. **Actions.** Each rule names its own action. The manifest's files set the usual
   choices: `confirm` for destructive commands, `block` for protected paths,
   `redact` for personal data and `block` for prompt injection.
   - **`block`.** A `command` or `path` match refuses the tool call with
     `Blocked by guard <id>: <description>`, and the interactive mode shows the same
     text as a warning. A user shell command that matches does not run; its output
     is that text. A `tool-output` match replaces the result's text with
     `Tool output withheld by guard <id>: <description>`, replaces its details with
     null and marks the result as an error, so the model learns why. A user shell
     command's output that matches is replaced by
     `Output withheld by guard <id>: <description>`. An `agent-output` match
     replaces the message's text and thinking with
     `Response withheld by guard <id>: <description>`. A `prompt` match replaces the
     message's text with `Prompt withheld by guard <id>: <description>`, and the
     interactive mode shows the same text as a warning.
   - **`confirm`** (`command` and `path` only). The interactive mode shows the
     description with the command or path, and offers `Yes` and `No`. `No` refuses
     as `block` does, with the reason `Blocked by user`. Print mode, JSON mode and
     RPC without a user interface refuse as `block` does.
   - **`redact`** (`tool-output`, `agent-output` and `prompt` only). Each match
     becomes `[<FAMILY>:<id>]`, where `<FAMILY>` is the file's family in capitals,
     for example `[PII:canada-sin]`, in text and thinking blocks, in user shell
     output and in every string in a tool result's details. A thinking block that
     changes drops its provider signature and is replayed as plain unsigned text
     through the existing conversation projection.
   - **`warn`.** The operation goes ahead unchanged, and a warning names the rule.
5. **Several matching rules.** Every rule for a target matches against the
   original text. A matching `block` rule wins, then a matching `confirm` rule; the
   first such rule in file and list order names the refusal or the question. When
   neither matches, collect all `redact` match ranges from the original text and
   merge overlapping ranges, including transitively overlapping ones. Replace
   each merged range once with the marker of the first contributing rule in file
   and list order; disjoint ranges keep their own markers. Replacement text is
   never matched again. The warnings of matching `warn` rules are shown unless
   the operation is refused.
6. **Other extensions.** The manifest package is the lowest package source in the
   extension list (H4), so its guard hooks run after every other extension's hooks
   for the same event. Guards therefore check the final command, path, result and
   message that Maestro acts on, including changes other extensions made. Other
   extensions see each value before the guards do, and an extension that handles
   an operation itself, for example by supplying a user shell command's result,
   takes that operation outside the guards. Extensions keep full user permissions
   (M8.5); guards check what the agent and the user send through Maestro.
7. **Saved sessions and streams.** Session files keep what the guards return:
   redacted text, withheld results and withheld responses. When any rule targets
   `agent-output` or `tool-output`, the manifest package sets `holdUncheckedText`
   to `true` in its base layer and locks it (H1). While the setting is `true`, the
   session sends no unchecked assistant text in any listener payload: text deltas,
   message-start payloads (including a completed reply when no start event
   arrived), message snapshots and nested model-event snapshots are all covered.
   Each reply's text reaches the interactive view, JSON and RPC event streams and
   SDK listeners only when its message ends, after the guards have checked it.
   Thinking text is held the same way and arrives with its message. Tool-call and
   tool status events still stream, with assistant text and thinking omitted from
   their accompanying snapshots. Partial tool-result content and details are
   withheld from progress events, and so is the output of a running user shell
   command; listeners receive the completed result or output only after the guards
   have checked it. Emitted snapshots do not share unchecked
   mutable text or details with listeners. `/settings` shows the setting as locked,
   like any locked value.
   With the setting `false`, the default, streaming is unchanged.
8. **Saved sessions.** At each session start under rules that target
   `tool-output`, `agent-output` or `prompt`, the manifest package checks saved
   session files in the background with the same rules as live text. It checks
   every file at the first guarded start and whenever those rules change;
   otherwise it checks only new files or files whose bytes changed since their
   last check. It rewrites each file that changes through the session owner's
   single conditional rewrite, which replaces the file atomically.
   Every session append and rewrite waits for the same stable sidecar file's
   exclusive lock through the lock component's waiting acquisition, using
   `std::fs::File::lock` (dependency authorization: [#2906](https://github.com/Orchestration-Maestro/maestro/issues/2906)).
   Settings keep their bounded acquisition. The operating system releases the
   lock when its holder exits; no separate stale-lock recovery is needed. Each
   holder keeps the lock for one write only. Ordinary writers hold it only while
   opening, writing and closing the file. If a
   writer runs on an async executor thread, it waits where the existing
   synchronous session write already runs, without a new blocking pool. The
   conditional rewrite holds the lock across one file's final re-read, hash
   comparison and atomic replacement. The package reads and checks a file before requesting
   the rewrite; if the hash of those bytes differs from the locked re-read, it
   reads and checks again. This hash check covers the window from the first read
   to taking the lock. An append attempted after the final comparison waits until
   replacement finishes, then appends to the published file. The sweep record in
   the configuration root's state, next to the persisted base layers, keeps the
   digest of the rules last applied and, for each session file, the hash of its
   bytes after it was last checked. An unchanged rule digest skips only files
   whose bytes still match their recorded hash; a changed digest checks all files.
   The original text is not kept.

## Core hooks

The core gains five generic hooks and one generic setting, `holdUncheckedText`
(M20.7). None reads a manifest, knows the manifest's governed-key table, or
combines areas; the manifest package does that.

| Hook | Owner | Behaviour | Without a publisher |
|---|---|---|---|
| H1 Base layer | `maestro-settings` | One read-only settings document plus a set of locked key paths, below the user layer. A locked path takes the base value, and an explicit edit of it is refused. The publisher marks the layer global or project-scoped. The settings owner persists layers for processes without packages (see below), and offers the current layer to its publisher. | Persisted layers still apply; with none, foundation behaviour |
| H2 Decision hook | `maestro-settings`, `maestro-app` operations and resource collection | One registered decider is asked before the first effect of governed operations (M4), when stored values resolve, and during resource collection for local additions, required collisions and required entries. Early package commands use H1's persisted layer instead. A refusal carries the message to show. | No decider; foundation order |
| H3 Daemon view | `maestro-daemon` configuration read | The workspace daemon reads the persisted global base layer plus the user settings with locks applied, never project settings. | Today's raw user read |
| H4 Load-time contributions | `maestro-app` resource loader | Base packages load before every other extension and keep their ordinary place in the extension list. During their load a base package may contribute package sources, base prompt inputs (base system prompt, base append text) and model files ranked before the user's. The context-file loader is unchanged. Contributed package sources install and load in the same startup. Only one package may publish H1 and H2. | Foundation single pass |
| H5 Host operations | `maestro-app` host operations | Extends the planned settings-reader and package-root facades: read raw user and project settings, list resolved packages with identity, scope and installed path, acquire a pinned source, list a source's releases, verify a release checkout is clean and at its pinned commit and restore it when it is not, read files from a candidate in the temporary scope, write a value through the settings owner, apply package filter semantics, parse a YAML document with the existing parser, list saved sessions, and rewrite a saved session's entries through the session owner. | Unused |

### Persisted base layers

H1 persists the global layer in the configuration root and project-scoped layers
in that root's state, keyed by project root, never inside the project. The
publisher refreshes each applicable layer from the accepted composition at every
session start and removes it when there is no selection. Each record names its
source release and when it was recorded. Package commands read the persisted layer for their operation's scope
before any effect; the daemon reads only the global layer (H3).

With no persisted layer, commands use foundation behaviour without reading the
`manifest` setting. After a selection change, the old layer governs until the next
session start replaces or removes it. Refusals name the recorded release and time
and say that starting Maestro refreshes the layer. Session-time collection is the
backstop: forbidden local additions never execute (M6.4), and missing required
packages install again (M6.5, H4).

### Internal-mode changes

Base packages load before other extensions (H4). Base packages are Maestro
additions. They keep their ordinary place in the extension list as the lowest
package source, and the list keeps the reference's order: command-line extensions
first, then project, user and package resources by precedence. The order of
every other extension is therefore unchanged, and conflicts resolve from that list
as in the reference: the first registration wins for tools and flags, the later
one for shortcuts, and duplicate commands get numbered names in list order. A base
package resolves conflicts exactly as any other package does.

Without a manifest, `holdUncheckedText` may still be set explicitly: its default
changes nothing, and enabling it only withholds text until checked.

Package requirement resolution and dependent-aware removal (M16.5) apply in both
modes. Without a manifest, the same package-manager operations resolve transitive
requirements, reuse satisfying installed packages, refuse missing requirements,
cycles and version conflicts before effects, and require explicit combined
removal of installed dependents. Package commands still enforce any persisted
policy layer (H1), without reading a manifest or requiring a selection.

## Implementation decisions

- **Placement.** The core holds only H1 to H5 and the `holdUncheckedText` setting.
  This workspace gains one default-on base package, a package crate released with
  Maestro like the other base packages ([packages](packages.md#base-packages)):
  `maestro-manifest` (selection, acquisition, area reading, prompt composition,
  instruction installation, policy decoding and combination, decisions, guards,
  `/manifest`, release checks, catalog kinds). No core crate depends on it.
  Workflow files run on the runner of Maestro's subagent capability; roles stay
  with `maestro-subagents`; MCP servers stay with `maestro-mcp`. Authored content,
  including instructions and the default workflows, lives in manifests, not in
  packages.
- **Packages talk through existing routes.** The manifest package shares its
  selected area roots and required paths with the subagent capability (roles and
  workflow runner) over the extension event bus. No package copies another's
  parsing, matching or paths.
- **Validation is loading.** The manifest package validates while it loads; a broken
  manifest fails the load. There is no separate checker command or binary.
- **Guards.** Matching is one pure module in the manifest package: rules and text
  in, a decision out, with no I/O and no hook types. A thin layer registers the
  existing tool-call, user-shell, tool-result, message-end and context hooks and
  applies the decision. Summaries go through the existing session-before-compact
  and session-before-tree hooks: the guard supplies each summary, generated from
  checked text with the compaction-generation host operation, so the context and
  message-end hooks keep their existing coverage. User shell output is checked
  through the user-shell hook's supplied local shell operation. Saved sessions are
  cleaned with two host operations over the session owner's existing listing and
  its single conditional rewrite. Every append and rewrite caller uses the same
  session-owned file exclusion (M20.8). Expressions compile with the
  workspace's existing `regex` crate. `holdUncheckedText` is read in exactly one
  place, the agent session's listener emission boundary for every event that
  carries assistant text or partial tool-result content or details, not only
  message updates. That boundary withholds partial results rather than checking
  progress snapshots with guard rules. The interactive view, JSON
  and RPC modes and SDK listeners all read that emission, so none withholds text
  itself. No other package matches guard rules; a package that reacts to guarded
  work builds on the manifest package's behaviour instead of matching again.
- **Settings base layer.** Every rebuild (construction, reload, overrides, edits)
  applies the base layer, then the ordinary one-level user and project merge, then
  locked paths, then stored-value decisions, then locked paths again. One settings-owned
  composition function owns this order. There is no new settings scope or change to raw scopes or the write queue. Decisions run
  outside the queue and storage locks.
- **Locks beyond settings values.** The three base-layer-only paths
  (`keybindings.<action>`, `showChangelog`, `credentials.<provider>`) use the same
  base layer. The manifest package shares the lock names that concern another
  package (`workflows`, `agents`, `instructions`, `mcpServers`) over the same event
  bus that shares the area roots, and each owner ignores and refuses locally. The
  application operations that set a binding, store a credential or choose a session
  folder ask the decision hook (H2) like every other governed operation; frontends
  render and check nothing.
- **Packages.** The package owner gains release listing for one source and the
  corrected Git pin check. Source parsing stays single.
- **Catalog.** The model registry accepts an ordered list of model files; the
  existing single-file construction is the one-element case.
- **Application operations.** Model, thinking, tool, behaviour-setting and resource
  operations and the extension host ask the decision hook before their own
  effects. Early package commands enforce the persisted base layer (M4, H1).
  Frontends render; they check nothing themselves.
- **No new crate in the core, and no new library.** The manifest package is a
  package crate under the package crate rules.

## Testing decisions

- Test external behaviour at the highest seam. First: session construction through
  the SDK with temporary configuration and project folders and a local release
  fixture, observing effective settings, loaded resources, prompts, models and
  refusal text. Then: the settings manager for H1, H2 and persistence; application
  operations for refusal before effect; the extension host contract for H4 and H5;
  each package through the real host.
- Use synthetic current-format manifest fixtures checked into the tests, with
  every policy class, area kind, workflow form and failure case. No private or
  captured organization configuration is a fixture.
- Manifest root: a fixture release that also holds files beside the root
  (documentation, schemas, a README and a decoy `core/` folder with a `SYSTEM.md`,
  a policy and a guard file) loads only the root's content: the decoys change no
  resource, prompt, policy or guard. A release with no manifest root fails the
  load, so agent sessions refuse, with the message M9 gives. Release inspection
  and `/manifest status` read `CHANGELOG.md` from the root, and the ownership file
  is the catalog's only read outside it.
- Every refused request asserts that memory, pending edits, files, session
  history and emitted events are unchanged, not only the error text.
- Package commands: a persisted `packages` lock refuses install, and required
  identities refuse removal in the matching scope before files or settings
  change. Commands load no extensions. Broken manifests retain those checks;
  refusal text names the saved release and time and how to refresh it. With no
  layer, commands behave as the foundation; the next start excludes forbidden
  additions and reinstalls missing required packages. A changed selection retains
  the old layer until start replaces it; clearing the selection removes it.
  Project layers stay in configuration-root state, keyed by project root; global
  and project checks never use each other's layer.
- Package requirements: exercise the existing install operation through CLI,
  SDK, catalog selection and startup required installs (M6.5), in user and project
  scopes. Exercise CLI and SDK installation, reuse, preflight refusals and
  dependent-aware removal both with and without a manifest; package-command
  policy refusals use the persisted layer. A transitive chain installs all required
  packages; a diamond installs its shared requirement once. Each selection is one
  operation and one settings write. A satisfying installed version in the selected scope is reused without
  reinstalling; an installation in another scope does not satisfy it. Catalog
  preview names every addition and its requirer, confirms once and reports the
  installed result; cancellation changes nothing. Listings report actual scoped
  state without effects; daemon-routed operations keep H3's global policy view.
  A missing requirement names its source and requirer; a cycle names its ordered
  loop; incompatible requirements name both requirers; an incompatible installed
  version names that version and the conflicting requirer and is not replaced.
  A `packages` lock and a forbidden source each name the refusing constraint.
  Every preflight refusal installs nothing and leaves settings and files
  unchanged, including at startup, where it refuses session construction.
  Removal of a required package names every direct and transitive installed
  dependent and changes nothing. Its offered explicit combined-removal step
  removes exactly that package and its dependents in that scope with one settings
  write, leaving unrelated packages alone. A required manifest package or lock
  blocking any member refuses the entire combined removal before deletion.
- Workflow lock: with `workflows` locked, a workflow file in the configuration
  root and one in the project are ignored with a diagnostic and do not list, show
  or run, an option that adds one refuses the run with nothing changed, and a
  required workflow still wins a name collision; unlocked, both load.
- Role lock: with `agents` locked, role files in the configuration root and the
  project are ignored with a diagnostic, a workflow's `agentScope: both` resolves
  manifest roles only, and an option that adds a role file refuses the run;
  unlocked, they load.
- Instruction lock: with `instructions` locked, a local instruction file that is
  not an installed copy is ignored with a diagnostic, an option that adds one
  refuses the run, installed copies and a context file still load, and
  `/manifest install` of the manifest's own file succeeds. A required instruction
  whose copy was deleted or edited is installed or refreshed at the next start and
  listed in the restore notice, even with instruction loading turned off.
- MCP lock: with `mcpServers` locked, servers in the user's and the project's
  gateway configuration are ignored with a diagnostic and a gateway option that
  adds one refuses the run; with it unlocked they load, and an exclusive gateway
  option never drops a required manifest server in either case.
- Guards come only from the manifest: a `guards` entry in `locked` makes the
  manifest invalid, and no settings key, environment variable or local file adds,
  removes or disables a rule.
- Presentation locks: with `theme`, `shellCommandPrefix` and one other
  presentation key locked, a stored user value is replaced by the manifest's with
  a diagnostic and the file is unchanged, `/settings` and a setter refuse a
  different value before any change. Unlocked keys of the same kind stay free.
- Theme resource lock: with `themes` locked, local theme folders are ignored with
  a diagnostic and `--theme` refuses the run. No manifest `themes` value is set;
  unlocked, local themes load.
- Keybinding lock: with one action locked, the user's `keybindings.json` entry for
  it is ignored with a diagnostic while the entry for another action applies, an
  SDK or extension rebind of the locked action is refused with no change, and the
  file is not rewritten.
- Machine-path lock: with `shellPath` locked, a stored user value is replaced with
  a diagnostic, a flag naming a different program is refused, and one naming the
  locked value is accepted. With `npmCommand` locked to
  `["mise", "exec", "node@20", "--", "npm"]`, a stored vector is replaced as
  a whole with a diagnostic, a setter that changes or drops one argument is
  refused, and the identical vector is accepted. A manifest value for either key
  that `locked` does not name makes the manifest invalid.
- Session-directory lock: with `sessionDir` locked to `~/org-sessions`, a stored
  value is replaced with a diagnostic, new sessions are created only there,
  `--session-dir`, `MAESTRO_CODING_AGENT_SESSION_DIR` and `--session <path>`
  naming another location are refused, and the same location spelled with
  the expanded home folder is accepted; existing session files are not moved.
- Changelog lock: with `showChangelog` locked to `false`, an unseen release shows
  no changelog at start while `lastChangelogVersion` is written as before and
  `/changelog` still shows the full text; locked to `true`, the unseen entries
  show; a settings edit of the key is refused, and a lock on
  `lastChangelogVersion` makes the manifest invalid.
- Credential lock: with `signIn` the only source allowed for a provider, a stored
  key and the provider's environment variable are ignored with a diagnostic, a
  pasted key at login and `--api-key` are refused before any file changes, sign-in
  still works, and the credentials file stays byte-identical. A value naming a
  secret or an unknown source makes the manifest invalid.
- Broken-release repair: a clean newer release of the same identity applies at
  start; permission growth asks, and refusal or a headless run with growth stops
  with the fix. Compare against the last accepted composition; without one, list
  the whole candidate permission set as growth. Offline or no clean candidate
  reports the fix. An otherwise valid pin never takes an optional release at start.
- Required-update inspection: a floor in either common or core policy is honoured;
  when both set a floor, the higher one wins. Absent files add no floor.
- Inspect a candidate whose lifecycle script writes a sentinel file, then reject
  or cancel it: the sentinel never appears and the active files, pin and settings
  stay byte-identical. An approved candidate installs through the ordinary flow.
- Manifest model-file tests accept a literal non-credential header, reject literal
  API keys and credential headers (including mixed-case names and policy-added
  names), and accept environment or command references in credential positions.
- Workflow runs use the reference's faux provider for agent steps; command steps run
  real programs in a temporary folder.
- Guards: the matching module is tested with rules and text only, for every action,
  target and precedence case. Overlapping redactions use a name rule matching
  `Alice Jones` followed by an address rule matching `Alice Jones, [^\n]+` on
  `Alice Jones, 10 Main St`: the result is `[PII:name]`, with no address left.
  Reverse the rule order and expect `[PII:address]`; disjoint matches keep their
  own markers. Each target is then tested through the real host
  with a fixture manifest: a confirmed and a refused destructive command, a
  headless refusal, a refused user shell command, a refused protected-path write,
  a redacted tool result and assistant message as saved in the session file, an
  edit result whose diff carries personal data, a withheld injected tool result,
  and a typed prompt, a steering message, a follow-up message and an expanded
  prompt template, each redacted in the session file and in the faux provider's
  request. An earlier extension that edits a tool call's arguments has the edited
  arguments checked. A failing example, an expression that does not compile, a
  duplicate id, an empty `noMatch` list and `confirm` on a text target each fail the
  load. With `holdUncheckedText` set to `true`, no unchecked assistant text,
  thinking, partial tool output (content or details) or running user shell output
  reaches the interactive, JSON, RPC or SDK listeners in any payload: test text and
  thinking deltas, a terminal-only response whose message start carries the
  completed reply, and tool-call updates whose snapshots also contain text.
  Tool-call and tool status events still arrive; message end delivers only the
  checked text and thinking, and the completed tool result contains only checked
  content and details. Include a partial tool result with `SIN 046-454-286` in both
  its text and details.
  With `false`, the events are byte-identical to today's.
- Summary guards: resume a session saved before manifest selection with personal
  data in user, assistant and tool messages and in a previous summary. Manual and
  automatic compaction, including split-turn summaries, and branch summaries send
  only checked text to the faux provider. A summary response that matches a rule
  is checked before the guard returns it, and the guard's summary replaces one an
  earlier extension supplied. `/compact SIN 046-454-286` and branch-summary
  instructions containing the same text are checked with prompt rules before
  reaching the provider. A guard generation failure (including a provider timeout)
  or check failure reports the guard name and cause through the existing extension
  error warning channel and returns `cancel: true`, without throwing or adding a
  reason field; no unchecked fallback is generated, saved or emitted. Test warning
  delivery on both summary routes without a UI, and verify that manual compaction
  retains its existing `Compaction cancelled` text.
  The summary path itself leaves existing entries unchanged; item 8 cleans them.
- Thinking, shell output and saved sessions: a thinking block with personal data
  is held until its message ends, saved redacted and replayed as unsigned text to
  the faux provider. A user shell command's output is held while it runs, saved
  redacted and withheld on a `block` match; an earlier extension that handles the
  command itself is not checked. A sweep rewrites every saved session that holds
  matching text and leaves clean files byte-identical. With the same rule digest,
  it skips files whose bytes match their recorded hashes but checks new or changed
  files; a rule change checks all files again. Run with guards, save a session
  without a manifest, then select the same rules again: that file is cleaned.
  The sweep reads and checks again when another process appends during the check.
  Attempt an append from another
  process after the final hash comparison but before replacement: it waits for
  publication, then its entry survives in the rewritten file. Hold a writer's
  lock through a slow synchronization beyond the bounded acquisition's entire
  retry window: an append waits for at most one concurrent write, finishes
  afterwards and leaves its entry intact. Exercise every append and rewrite caller
  under the same exclusion.
  A terminated lock holder does not prevent a later writer from acquiring the
  lock. A plain single-process append shows no visible delay: measure once with
  and without the coordination and record the numbers in the implementation pull
  request. A crash between writing and replacing leaves the previous file whole.
- Integrity: a modified, deleted or untracked file in the release checkout, and a
  checkout moved off its pinned commit, are each restored at start and listed in
  the notice; an edited instruction copy is refreshed; a clean checkout loads with
  no notice; a pinned commit that cannot be restored is installed again, and offline
  the session runs on the last accepted composition with a warning.
- Schemas: generated schemas equal the committed ones, and every fixture passes or
  fails as marked.
- Internal-mode equality is tested: with no selection, resources, prompt, model,
  tools, settings writes and JSON/RPC output equal the foundation behaviour except
  for the three [internal-mode changes](#internal-mode-changes): base-package
  loading, explicitly enabled `holdUncheckedText`, and package requirement
  resolution with dependent-aware removal (M16.5). No manifest path is read.
- Prior art: settings behaviour and reload tests, configured-source and pinned
  package checks, resource-loader precedence cases, catalog composition and
  provider lifecycle tests, startup listing and selector tests, extension host
  controlled tests.

## Differences with no reference source

Each item is a deliberate difference from the reference.

1. The `manifest` setting, its self-lock and the user-first reading of a locked
   selection.
2. The manifest layer between engine defaults and user settings, and the areas and
   their order, including capabilities, all under one manifest root.
3. The `manifestAreas` setting.
4. `settings/policy.json`: locks, bounds, the model allowed set, required entries,
   role constraints, credential headers, the minimum release and the check
   interval, and the lock names `systemPrompt`, `appendSystemPrompt`, `providers`,
   `workflows`, `agents`, `instructions` and `mcpServers`.
5. Stored-value resolution and refusal of explicit requests before their effects.
6. Refusing agent sessions on a broken manifest.
7. The `[Manifest]` startup section, `/manifest status` and the lock markers in
   settings views.
8. Release checks at session start and on an interval, required releases,
   `/manifest update [version]`, permission-growth confirmation and the
   `manifestAutoUpdate` setting.
9. The `thinking` role field and the `roles.<role>.{model,thinking}` user setting.
10. Literal-secret rejection in manifest model files.
11. Installing manifest instruction files into the local instruction folders.
12. The `maestro.workflow/v1` file and its `command` and `ask` steps.
13. The core hooks H1 to H5 and loading base packages first.
14. The manifest's catalog blueprints, relations and CODEOWNERS ownership.
15. Git-only manifest sources and the release integrity repair at every start.
16. Generated JSON Schemas for artifact kinds and their pre-commit and pull-request
    checks.
17. Guards (M20): rule files in the manifest, the `maestro.guards/v1` format,
    redaction, and the personal-data and prompt-injection families. Confirming
    destructive commands and refusing protected paths follow the reference's
    example extensions, with their patterns taken from the manifest instead of the
    code. Checking thinking and user shell output, and cleaning saved sessions
    when guard rules apply or change. The `holdUncheckedText` setting, which
    withholds assistant text, thinking, partial tool-result content and details,
    and running user shell output until they have been checked, while status
    events still flow.
18. Package-to-package `requires` resolution through the existing install
    operation, transitive selection, cycle and version refusals, and explicit
    combined removal of dependents (M6.5, M16.5, M17.2, M17.4).
19. A lock on every governed configuration surface: each presentation key, one
    keybinding action, each machine path and the session directory, whether the
    changelog shows at start, and the credential sources allowed per provider
    (`keybindings.<action>`, `showChangelog` and `credentials.<provider>` exist
    only in the base layer), and the local-addition locks for workflows, roles,
    instructions and MCP servers. A lock fixes a value or refuses local additions
    and never loosens another rule.

## Corrected reference defects

A pinned Git source whose checkout is not at its ref is never checked out at the
ref: installation returns early because the checkout folder exists, and the Git
installation path excludes the ref. A pin change in settings therefore keeps the
old files, while a pinned npm source is reinstalled on a version mismatch. The
package owner checks a pinned Git checkout against its ref and checks out the ref
when they differ. This also corrects internal mode.

A session file rewrite wrote the file in place, so a crash during the write left
a partial session file. Session writers also ran without file-write exclusion,
so a rewrite could lose a concurrent append. The session owner's single rewrite
now replaces the file atomically, and every append and rewrite coordinates
through the same file-write exclusion. This also corrects internal mode.

## Runtime mechanics not copied

No manifest file watcher, daemon poll, cache, transaction or rollback copy is added.
Release checks run inside sessions with the host's sleep, at the policy's interval.
Network timeout and worker counts stay the package owner's. Lock-file leases,
promise scheduling and engine error formats are not reproduced; existing Rust
owners keep their ordering and accepted-write semantics.

## Out of scope

- Administrator-only switches, system-wide configuration stores, signed releases,
  and a last-valid copy used at start for anything other than unrepairable drift
  (M2.5).
- A sandbox. The integrity check (M2.5) repairs edits to the installed release;
  relocating the configuration root, editing settings files, older or modified
  programs, SDK programs supplying their own components, and local administrators
  remain outside configuration authority.
- Per-capability refs inside one release and per-file versions.
- Live push of releases to running sessions, and reloading a session on its own.
- Presets as an engine concept, area descriptors read by Maestro, handoffs, policy hooks, quality profiles, evaluations, knowledge
  sources, bootstrap and `maestro init`, until a package reads them.
- Bounded `loop` workflow steps and nested workflows, until a later delivery
  registers them.
- The rules system that loads instruction files (always-apply rules, a listed
  rulebook read on demand, `applyTo` and stream rules). It is specified separately;
  this specification supplies, installs and refreshes the files and states the
  locks the rules system honours (M5.3).
- New RPC or JSON fields.
- Guards without a manifest, local guard files and settings that change guards;
  and detection other than regular expressions.
- Manifest mode in the browser application.

## Glossary

Add to `CONTEXT.md`:

- **Manifest:** An organization's pinned configuration release named by the
  `manifest` setting; distinct from a package manifest and a detection manifest.
- **Package manifest:** The `maestro` key of a package's `package.json` listing its
  resources.
- **Internal mode:** Running without a manifest; foundation behaviour.
- **Manifest mode:** Running with a named manifest, which adds a layer and a policy.
- **Manifest package:** The default-on base package that selects, installs, reads
  and enforces a manifest through the core hooks.
- **Manifest root:** The one folder of a release, `manifests/`, that holds all of
  its manifest content. Maestro reads manifest content only from it; documentation,
  schemas and CI stay outside.
- **Manifest area:** The manifest root itself (the common area) or one folder under
  it (standard, core, language or capability) whose content enters the manifest
  layer in a fixed order.
- **Capability:** A manifest area that is one package bundling a team's roles,
  skills, instructions, prompt templates and workflows, selected by name.
- **Capability group:** The folder that groups capabilities, usually after the
  organization's team structure.
- **Authored artifact:** A file a user, project or manifest writes to configure or
  drive Maestro, such as a role, skill or workflow; distinct from a browser
  artifact.
- **Artifact kind:** The type of an authored artifact, with one format valid
  locally and in a manifest.
- **Workflow:** A named chain or parallel set of steps in a `maestro.workflow/v1`
  file.
- **Guard:** A manifest rule that blocks, confirms, redacts or warns on a command, a
  path or text, from a `maestro.guards/v1` file in the manifest's core area.
- **Guard family:** The set of guard rules in one guard file, named by the file,
  such as personal data or destructive commands.
- **Base layer:** A read-only settings document with locked keys, published by a
  package below the user layer.
- **Blueprint:** The catalog's description of one artifact kind: its properties
  and the relations its entries have.
- **Requires:** The catalog relation from a release or package to a package it
  needs. Skills and workflows inherit their package's requirements; selecting an
  entry installs the transitive required-package closure in its scope.
- **Policy:** The combined locks, bounds, allowed models, required entries, role
  constraints and release rules of a manifest; the manifest package decides every
  governed change.
- **Lock:** A policy entry that fixes a value or refuses local additions of a
  resource kind. A lock never loosens another rule.
- **Lock name:** A `locked` entry that names something other than a settings key:
  the prompts, the providers or a resource kind that has no settings key.
- **Bound:** A policy minimum or maximum for a numeric value or thinking level.
- **Required resource:** A manifest resource that local layers cannot disable,
  shadow or drop.
- **Minimum release:** The lowest manifest release a session may run; a release
  below it is updated at the next session start.
- **Permission growth:** A manifest release change that adds role tools, widens
  allowed models or a bound, removes a lock or required entry, or adds an
  extension, package or MCP server.

## Amendment to other specifications

The foundation specification, `docs/specs/maestro-port.md`, is unchanged. It
already states that internal mode has no manifest, origins or locked values, and
points here.

`docs/specs/packages.md`, under Base packages, gains that base packages load before
other extensions and may publish the base layer and decision hook (H1, H2, H4).
