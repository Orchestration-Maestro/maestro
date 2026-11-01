# Maestro packages

This specification defines Maestro's packages: extension components built in this workspace and shipped as
ordinary packages. Each one is rebuilt from a reference package written for the reference agent's extension
interface. It extends the [one component extension system](maestro-port.md#r07--one-component-extension-system)
and the [package sources](maestro-port.md#r06--instructions-resources-and-packages) of the port specification.
Terms in [Glossary](#glossary) are also defined in [the glossary](../../CONTEXT.md).

## Purpose and authority

Packages are owner additions outside the core. They follow the reference release's extension interface, so every
host call they make exists in the foundation, and they add no core behaviour of their own. Each package is ported
like any reference:

- each reference file maps to a Rust module with the reference's exported names in Rust casing, with idiomatic Rust inside;
- the reference package's own tests are ported red first under Maestro names;
- reference defects are fixed and listed once;
- every capability is kept, and nothing is added without a reference line or an owner decision.

The packages are the subagents, MCP gateway, web-access, question, todo, usage, CLI-bridge provider, code-insight,
context-saving, lean-mode, focus-mode and plan-review packages. Three skill packs need no build, because they load as package content with
per-package skill filters: Maestro's own skill pack ([P14](#p14-maestro-skill-pack)) and two third-party
packs. The porting rules above apply to code packages; the skill pack merges source skill packs as P14
describes.

## Problem statement

The reference packages are JavaScript and TypeScript extensions, and Maestro loads only WebAssembly components.
Without a port, every capability they give a user is lost. A direct port runs into six problems:

- The packages import the reference agent's exports (sessions, settings, package roots, schema checks and
  compaction). A component cannot link native code.
- Some packages copy logic the agent does not export: package-root scans, MCP configuration parsing and a
  key-identifier check.
- Some logic is duplicated across packages: three HTML converters, two language-server clients, two script engines
  and four local web servers.
- Several packages use host features of releases after the reference.
- Two references may not be copied: one licence permits rebuilding behaviour only, and one vendor SDK is
  proprietary.
- The largest files are registration hubs that import every later capability, so a file-order port does not
  compile in order.

## Solution

- **One crate class.** Each package is a workspace crate of class `package`. Its library builds one component, and
  the executable never links it ([P1](#p1-package-crates)).
- **Host-backed facades.** A reference export a package imports becomes a facade with the same name, served by the
  native owner through the host ([facades](#host-backed-facades)).
- **One owner per mechanism.** Five shared libraries and three package-to-package edges replace every copy
  ([shared libraries](#shared-libraries)).
- **Reference-release routes.** Every later-host feature takes its reference-release route, or is dropped where it
  loses no capability ([routes](#later-host-features)).
- **Growth.** A delivery never calls a later delivery's capability; later deliveries add their own arms
  ([growth](#growth-later-capabilities-add-their-own-arms)).
- **Clean-room rebuilds.** The behaviour-only and proprietary references are rebuilt from documented and observed
  behaviour ([clean-room](#clean-room-rebuilds)).

## User stories

1. As a user, I want subagents (single, parallel, chain and background runs), with steering, a supervisor channel,
   acceptance gates, workflows, worktrees, missions and a fleet view, so that I can delegate work as I do today.
2. As a user, I want MCP servers configured once and used as tools, with sign-in, approvals, prompts, resources and
   app interfaces.
3. As a user, I want web search, readable fetch, GitHub, PDF, video, cookies-based authenticated fetch and result
   curation.
4. As a user, I want structured questions, a task list and usage reports with fast routing.
5. As a user, I want subscription models served through the Anthropic command-line client, with one provider per
   account.
6. As a user, I want language-server diagnostics, navigation, structural search, a read guard and scanners after
   edits.
7. As a user, I want bulky tool output kept out of my context, with a content index and continuity across
   compaction.
8. As a user, I want the lean and focus instruction modes, and an optional plan review in the browser.
9. As a user of another agent, I want Maestro's tools served over MCP.
10. As a new user, I want the base packages installed by default and easy to turn off.
11. As a maintainer, I want no `unsafe`, no patched fork, no copy of a host algorithm in a guest and no duplicate
    logic.

12. As a user, I want one skill pack covering design, specs, tickets, test-first building, debugging, review and
    shipping, with one router skill that names every skill and when to use it.

## Capabilities kept

| Package | Kept capabilities | Deliveries |
|---|---|---|
| Subagents | discovery from every scope; the tool in single, parallel, chain and background modes; in-process children with fresh or forked context; background runs that survive the call and wake the parent; steering, stop and detach; the supervisor channel; structured output and acceptance gates; scripted workflows; worktrees; missions and schedules; models and profiles; the fleet view and inspectors; cost and the event API; agent memory; commands, prompts, skills, doctor, guide and sharing; external command-line agents; MCP tool selection; the watchdog; pane placement and state reports | #1316, 10–24, 31, 51, 52 |
| MCP gateway | stdio, streamable HTTP, SSE and local-socket transports; lifecycle modes; metadata cache; configuration sources, imports and project-server approval; proxy, direct, namespace and search-activated tools; approvals; OAuth and stored credentials; prompts, resources, elicitation and sampling; app interfaces; panel and commands; scripting and semantic search; the API for other extensions | 25–30, 59 |
| Web access | search across every provider value with routing and fallback; readable, raw and answer fetch; source checks; stored results; SSRF protection and proxying; GitHub; PDF; video; browser cookies, authenticated fetch and Gemini Web; curation and summaries; activity widget | 32–39 |
| Questions | the question tool, its dialog, host fallbacks, configuration and locales | 3 |
| Todo | the task tool, replay, widget, command, configuration and locales | 2 |
| Usage | the usage report across providers, the status line, settings and fast routing | 6, 7 |
| CLI bridge | the subscription provider, in-session tools, session rebuilds, accounts and AskClaude | 8, 9 |
| Code insight | language servers and diagnostics, navigation, structural search and rewrite, code understanding, the read guard, formatting, autofix and scanners, commands and widget | 43–50 |
| Context saving | execution tools, routing, the content index and search, continuity and commands | 40–42 |
| Instruction modes | lean and focus levels, commands, rulesets and persisted settings | 4, 5 |
| Plan review (optional) | plan mode, browser plan review, code review and annotation, tools, inbox and event API | 53–58 |
| Skill pack | one router; interviews, ideas, option pages and prototypes; domain language and design vocabulary; specs, tickets, wayfinding and triage; test-first implementation and orchestration; debugging, verification and doubt; two-axis review and receiving review; git, pull requests, quality gates and shipping; security, performance, frontend and observability; documentation and the prose pass; writing for agents, research, retros, wizards, questionnaires and teaching | 62–64 |
| Serving tools | every tool of a headless session over MCP | 60 |

Not kept, because no capability is lost: features that need a later host release (each has a route below), other
agents' hook files, and runtime mechanics ([list](#runtime-mechanics-not-copied)).

## Design

### P1: Package crates

- **Class.** `workspace-crates.json` gains the class `package`. Conventions forbid any `core` or `dedicated` crate
  from depending on a `package` crate. They allow `package` → `package` edges, which must stay acyclic.
- **Allowed dependencies.** A package crate may depend on `maestro-extensions-wasm`, other package crates,
  `maestro-request`, `maestro-entry`, `maestro-tool`, `maestro-json`, and approved libraries that build for
  `wasm32-wasip2`.
- **Enforcement.** The conventions checker reads package classifications from the inventory, checks declared
  and host-resolved dependencies, and checks component targets and export locations in member sources.
- **One export per component.** The library is an `rlib` exposing `factory`. The component is the cdylib example
  target `component` at `src/component.rs`, which holds the one `export_extension!` line. A library never exports,
  because two export sets in one component fail to link. Conventions check both rules.
- **Build and ship.** `cargo build -p <crate> --example component --target wasm32-wasip2 --release --locked`. The
  component ships in a package whose `package.json` lists it in the `maestro` member. Consumers never compile.

### Host-backed facades

A package capability that calls a reference export uses a host-backed facade with the same name:

- `createAgentSession` and `AgentSession`;
- `SessionManager` and `buildSessionContext`;
- the `SettingsManager` readers and the `DefaultPackageManager` roots;
- `validateToolArguments`;
- compaction and branch summaries;
- message conversion, read-only tools, model listing and cost;
- the theme helpers.

Each facade is served by its native owner through `maestro-app`'s host operations. A guest never copies the
algorithm, and the facades add no crate-graph edge (delivery 1).

### Child sessions

- **Foreground children** run in process through the session facade, with steering on the returned session.
- **Background children** run in a detached `maestro` process that loads the subagents package in its runner role
  and serves the run directory.
- **Pane placement** runs Maestro's workspace agent commands through host exec. It is one protocol, with no
  protocol crate edge from a guest.

### Guest network and processes

The full-user host gives components the user's network (name lookup, connect, listen) and detached child processes.
While the native terminal's queued output runs, component stdout goes through the same ordered queue,
so it never splits a host frame. Other modes and regular-file destinations inherit stdout.
Local sockets arrive through `maestro-ipc`, the one local-socket owner (delivery 59).

### Shared libraries

| Library (class `package`) | One job | Consumers |
|---|---|---|
| `maestro-package-config` | read a package's own JSON configuration file | questions, todo |
| `maestro-html` | convert HTML documents to Markdown | web access, context saving, plan review |
| `maestro-lsp` | speak the language-server protocol | code insight, subagents |
| `maestro-script` | run sandboxed workflow scripts | subagents, MCP gateway |
| `maestro-loopback` | serve local browser pages and open them | MCP gateway, web access, plan review |

Package-to-package edges, each through the depended-on package's library:

- subagents → CLI bridge, for stream-json records;
- subagents → MCP gateway, for configuration and the metadata cache;
- web access → MCP gateway, for the streamable HTTP transport and secret reads.

### P2: Todo

`maestro-todo` keeps the task graph and replays it from tool results on the branch (delivery 2).

### P3: Questions

`maestro-questions` owns the one questionnaire dialog of the workspace (delivery 3).

### P4: Instruction modes

`maestro-minimal-mode` provides Maestro's `lean` mode through `/lean lite|full|ultra|off`.
`maestro-focus-mode` provides `focus` through `/focus lite|full|ultra|off`. Both are on by default at `full`.

Focus levels are:

- `lite`: action first and one next step at the end;
- `full`: the whole ruleset;
- `ultra`: full plus at most three items per list and the shortest working answer.

Lean injects instructions through `before_agent_start`; focus uses hidden custom messages, reinjected after
compaction or a level change. Both persist their settings (deliveries 4–5).

Lean's review and audit skills are type-only. They stay out of the model's skill list and run through their
alias commands.

### P5: Usage

`maestro-usage` reads credentials through the catalog getters and routes fast requests through the awaited request
hook (deliveries 6–7).

### P6: CLI bridge

`maestro-anthropic-cli` registers one provider per configured account and owns the stream-json records the
subagents package reuses (deliveries 8–9).

### P7: Subagents

`maestro-subagents` grows from the subagent example's port. Its registration hubs take each later capability's arm
in the delivery that brings that capability (#1316, deliveries 10–24, 31, 51, 52).

### P8: MCP

`maestro-mcp` runs rmcp inside the guest. Its first delivery qualifies that; if the qualification fails, the
library gap is classified once before any workaround (deliveries 25–30, 59).

### P9: Web access

`maestro-web-access` keeps one module per provider, as data plus one function (deliveries 32–39).

### P10: Context saver

`maestro-context-saver` is a clean-room rebuild (deliveries 40–42).

### P11: Code insight

`maestro-code-insight` runs language servers through `maestro-lsp`. Structural work runs the ast-grep program the
reference package depends on, and no in-process parser is added (deliveries 43–50).

### P12: Plan review

`maestro-plan-review` is optional and not a base package. Its two built browser apps are served unchanged as package
data. Its `checklist` module is shared with the plan-mode example target (deliveries 53–58).

### P13: MCP server

The frontend crate `maestro-mcp-server` (layer 5) serves every tool registered in a headless session over stdio MCP
as `maestro mcp`. No package carries server code (delivery 60).

### P14: Maestro skill pack

The skill pack merges three MIT-licensed skill packs and one MIT-licensed prose-pass skill into one skill per
job (deliveries 62–64). It holds no code, and the skill loader and package discovery read it unchanged.

- **Package.** `packages/maestro-skills/` is a content-only package. It holds `package.json` without a `maestro`
  member, `skills/<name>/SKILL.md` with supporting files, `references/` beside `skills/`, `LICENSE` and
  `THIRD-PARTY-NOTICES.md`. Conventional discovery loads `skills/`. `references/` is not a skill directory, and
  `skills/` holds no loose Markdown file.
- **Names.** Every skill is named `maestro-<job>`, and the router is `maestro-ask`. A user or project skill with
  the same name still overrides the pack's.
- **Invocation.** 23 skills are model-invoked. 14 are type-only (`disable-model-invocation: true`) and run as
  `/skill:<name>`, the router among them. Descriptions name triggers ("Use when…") and never summarize the
  workflow.
- **Router.** `maestro-ask` names every skill in the pack and when to use it: the main flow, the on-ramps, the
  specialist skills, and the lean and focus modes that hold standing behaviour. It reads a skill before
  describing it.
- **Main flow.** `maestro-grilling` → `maestro-to-spec` → `maestro-to-tickets` → `maestro-implement`, built
  with `maestro-tdd`, `maestro-code-review` and `maestro-verify`, then `maestro-pr` and `maestro-retro`. The user
  approves the spec and the tickets before work starts.
- **Shared references.** Type-only skills cannot call each other, so the text several skills share lives in
  seven plain files under `references/`: subagent briefs, severity, definition of done, untrusted input, report
  statuses, phase boundaries and issue trackers.
- **Issue trackers.** No setup step runs, because the pack ships built in. A skill that publishes or reads
  issues first uses the repository's own tracker file. Otherwise it uses the tracker behind `git remote`
  (GitHub through `gh`, GitLab through `glab`), and otherwise local Markdown under `.scratch/`. Five triage
  roles default to labels with the same names.
- **Host mechanics.** Skills name Maestro's tools and commands: `ask_user_question`, `todo`, `subagent`,
  `/skill:`, `/new`, `/compact`, `/fork` and `/tree`, with `AGENTS.md` as the rules file. `CLAUDE.md` stays an
  accepted context file name.
- **Option pages.** `maestro-show-options` writes one self-contained HTML page and opens it with the platform's
  opener. When no browser can reach the file, it prints the path and asks in the terminal.
- **Overlaps.** Simplification findings come from lean's type-only review and audit skills, and
  `maestro-receive-review` applies them. Full security audits belong to the third-party security-audit pack;
  `maestro-security` covers writing secure code.
- **Content rule.** Each section of a skill traces to a source line or to a host-mechanic translation. The pack
  carries none of the workspace rules of Maestro's own maintainers.
- **Notices.** The package's notice file keeps each source author's copyright line and the MIT permission text.
  Third-party text that has an unclear or share-alike licence is rewritten in our own words.
- **Qualification.** One `maestro-packages` integration test resolves the pack as a package source and loads its
  skills. It checks the diagnostics, the names, the `/skill:` pointers, router coverage and relative links.
- **Growth.** A delivery points only at skills already delivered; later deliveries add their pointers and
  router arms.

Not copied, each listed once: other hosts' hook, plugin, command and tool-map files; the always-on router
injection; Node and shell helper scripts, except one shell template; styling and diagram scripts loaded from a
CDN; external agent command lines; the option-page server's live reload, click log and reach from another
machine; the source packs' unfinished, language-specific and host-specific skills; the one-time repository
setup skill; a vendor best-practices page; and a picture menu whose licence is unclear, which is rewritten.

### Base packages

Every package except plan review is a base package. Plan review is the only optional package and is off by default. Each release publishes the base packages' components as package
sources. The application lists those sources as the lowest default `packages` source. The reference's
missing-source install installs them on first run, and a user's object-form filter entry turns one off
(delivery 61). The skill pack has no component. Its package directory is published as content.
Base packages load before every other extension but keep their ordinary place in the extension list, and a base
package may publish the base layer and decision hook of
[manifest mode](manifest-mode.md#core-hooks) (H1, H2, H4). The manifest base package is
specified there.

### Libraries

Added with first use at an exact pinned version: `boa_engine` (JavaScript), `rusqlite` with bundled SQLite and FTS5,
`htmd` and `dom_smoothie` (HTML), `lsp-types`, `aes-gcm`, `aes`, `cbc`, `pbkdf2`, `sha1`, and `chrono-tz`.
Already approved and reused: rmcp, sha2, notify and the PDF library.

## Implementation decisions

### Growth: later capabilities add their own arms

- **Rule.** A delivery ports no call into a capability that a later delivery provides.
- **Arms.** Each later delivery adds its arms to modules that already exist: registrations, schema fields, option
  branches and dispatch arms. Every issue lists the modules it extends.
- **No placeholders.** No delivery adds a stub, a placeholder result or a refusal for a later capability.

### Later-host features

| Later-host feature | Reference-release route |
|---|---|
| a settled-agent event | `agent_end` plus idle |
| dynamic tool loaders and transcript-backed tool changes | eager registration |
| unregistering or hiding tools | `setActiveTools` |
| prompt sections in the system message, declaration pinning | the `before_agent_start` prompt and `getSystemPrompt()` |
| template expansion in `sendUserMessage` | send the expanded text |
| stored-credential reads | the catalog's credential getters |
| an API-kind predicate | compare `model.api` |
| mouse regions | the expand key the component already names |
| the configuration-directory constant | the agent directory from the path helpers |
| direct tool execution from extensions | replay from tool-result details |
| the later host's built-in MCP and its sign-in import | none: the MCP gateway package is the owner |

### Names

Crates, modules, configuration file locations and documentation use Maestro names. Environment variables named
after the reference agent take Maestro's prefix. Tool, command, flag and event names stay exactly as the reference
packages spell them, because skills, agent files and workspace instructions call them by those names.

### Memory

Agent memory files and the context-saving database stay package-owned until a Maestro memory owner is specified.

### Clean-room rebuilds

- **Context-saving package.** Its licence permits rebuilding behaviour only. Its documentation and observable
  behaviour define the result, and its published build only sizes the work.
- **CLI bridge.** The command-line client's protocol is recorded from observed behaviour; the vendor SDK is never
  copied.

## Testing decisions

- **Reference tests.** Each delivery ports its reference package's tests for its files, red first, under Maestro
  names.
  - The MCP gateway and plan-review tests are mapped now.
  - The other packages' tests are mapped at step 1 from a pinned checkout of each upstream repository.
  - Until a mapping exists, every acceptance row gets at least one red-first behaviour test.
- **Real host.** Every capability is proven by a built component running through the real host, not a fake guest.
- **Coverage.** At least 80% line coverage, from tests that prove behaviour.

## Corrected reference defects

| Defect | Fix | Delivery |
|---|---|---|
| A malicious MCP server could steer sign-in to its own authorization server and collect refresh tokens | keep the installed release's issuer validation | 25, 26 |
| Two packages carry the same key-identifier check | one copy in `maestro-package-config` | 2 |
| The subagents package re-parses package roots and MCP configuration | the package-manager facade and `maestro-mcp::config` | 10, 31 |
| Three HTML converters, two language-server clients, two script engines, four local servers | one library each | 33, 43, 16, 26 |

## Runtime mechanics not copied

Results match the reference. These mechanisms are replaced, each listed once:

- module caching and restart-to-update checks;
- first-instance provider registration across module instances;
- proxying through a `curl` subprocess;
- SQLite command-line fallbacks and runtime SQLite probing;
- `vm` contexts, worker threads and parser pre-checks for scripts;
- package-root discovery, single-SDK imports and runner bootstrap;
- version probing through API shapes;
- rename retry ladders and cache-retention environment scoping;
- npx resolution caches;
- native Node modules (SQLite, keyring, ast-grep bindings), whose capabilities are kept;
- bundled client binaries;
- process, heap and event-loop instrumentation.

## Out of scope

- **Other hosts.** Other agents' hook files, plugin folders and status-line scripts in the packages and skill packs.
- **Reference examples.** The task-list, questionnaire, question and notification examples of the reference agent
  are separate example deliveries of the port specification.
- **Rebuilt browser apps.** Plan review's two built browser apps are served as data, not rebuilt.

## Delivery order

| # | Delivery | Blocked by |
|---|---|---|
| 1 | Expose sessions, settings, package roots and schema checks to components | — |
| 2 | Track tasks with the todo package | — |
| 3 | Ask the user structured questions | 2 |
| 4 | Inject lean-mode instructions | 2 |
| 5 | Inject the focus-mode ruleset | 2 |
| 6 | Report provider usage and quotas | 2 |
| 7 | Route Codex requests to the priority tier with /fast | 1, 6 |
| 8 | Serve subscription models through the Anthropic command-line client | 1, 2 |
| 9 | Delegate questions with the AskClaude tool | 8 |
| — | The subagent example as the subagents package's first slice (#1316) | 2 |
| 10 | Discover subagent definitions from every scope | 1, 2, #1316 |
| 11 | Run subagents in process with the full subagent tool | 1, 10 |
| 12 | Run subagents in the background and wake the parent | 10, 11 |
| 13–24 | Steering, supervisor channel, acceptance, workflows, worktrees, missions, models, fleet, cost and API, memory, commands, external agents | 10–12 and as listed in each issue |
| 25–30 | MCP gateway, sign-in, interaction, app interfaces, panel, scripting | 2 and as listed |
| 31 | Give subagents their selected MCP tools | 10, 25 |
| 32–39 | Web search, fetch, providers, GitHub, cookies, PDF, video, curation | 2 and as listed |
| 40–42 | Context saving | 2, 33 |
| 43 | Run language servers for the code-insight package | 2 |
| 44–50 | Diagnostics, navigation, structural search, understanding, read guard, scanners, commands | 43 and as listed |
| 51 | Review subagent boundaries with the watchdog | 1, 10, 11, 43 |
| 52 | Report subagents to the workspace and place them in panes | 10, 11, 20, 22, 24 |
| 53 | Port the plan-mode example and its done-marker tracker | 2 |
| 54–58 | Plan review (optional): plan mode, plan review, code review, annotation, tools | 53, 26, 33 and as listed |
| 59 | Connect components to local sockets | 25 |
| 60 | Serve Maestro tools to other agents over MCP | — |
| 62 | Ship the skill pack's core development flow | — |
| 63 | Add orchestration, sessions, trackers and documentation to the skill pack | 62 |
| 64 | Add the specialist skills to the skill pack | 63 |
| 61 | Install the base packages by default | every base-package delivery |

## Glossary

- **Package crate:** a workspace crate of class `package` whose library builds one extension component shipped in a
  package; the executable never links it.
- **Component target:** a package crate's `component` cdylib example, holding its one export.
- **Base package:** a Maestro package installed by default from its published source; a user filter turns it off.
- **Host-backed facade:** a guest call with a reference export's name, served by the native owner through the host.
- **Subagent:** a child agent run delegated by the subagent tool; foreground children run in process, background
  children in a detached runner process.
- **Loopback page:** a local browser page a package serves on a loopback port.
- **Clean-room rebuild:** a package rebuilt from documented and observed behaviour because its reference may not be
  copied.
- **MCP server frontend:** the frontend that serves a headless session's tools over MCP.

- **Skill pack:** a content-only package of skills and the shared reference files they read; Maestro's own is
  `maestro-skills`.
- **Type-only skill:** a skill marked `disable-model-invocation: true`; it stays out of the model's skill list and
  runs only when typed as `/skill:<name>` or through an alias command.

## Amendment to the foundation specification

- **R07.** Add: "Maestro's own packages are component guests specified in [Maestro packages](packages.md)."
- **Crates and delivery order.** Add a *Package crates* table listing each package crate and shared library with
  its one job, outside the layer numbers. Add `maestro-mcp-server` (present MCP server operations; `maestro-app`;
  layer 5) and add it to `maestro`'s allowed dependencies. Add `maestro-ipc` to `maestro-extensions-wasmtime`'s
  allowed dependencies (delivery 59).
- **Completion and explicit later scope.** Replace "added MCP/supervisor capabilities remain later owner phases"
  with "Maestro's packages follow [Maestro packages](packages.md)."
