# Settings guide

Settings are JSON files in two scopes, and the project file overrides the global
one. This guide says what to set and where to look; the
[settings reference](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/settings.md)
is the one authority for storage, merging, conversions and each setting's type and
default.

| Scope | File |
| --- | --- |
| Global | `settings.json` in the agent directory |
| Project | `settings.json` in the project's configuration directory |

Edit either file directly. Unknown keys are kept as written.

## Scopes and merging

The two files merge at the top level, and an object that appears in both merges
one level deep. Arrays, scalars and `null` in the project file replace the global
value, and the merge does not descend into grandchildren: a project
`retry.provider` replaces the whole global `retry.provider` object.

```json
{ "theme": "dark", "compaction": { "enabled": true, "reserveTokens": 16384 } }
```

```json
{ "compaction": { "reserveTokens": 8192 } }
```

Together these give `theme: dark` with `compaction` holding `enabled: true` and
`reserveTokens: 8192`.

## Model and thinking

`defaultProvider`, `defaultModel` and `defaultThinkingLevel` (`off`, `minimal`,
`low`, `medium`, `high` or `xhigh`) choose the starting model. `hideThinkingBlock`
hides reasoning output. `thinkingBudgets` sets token budgets for `minimal`,
`low`, `medium` and `high`.

```json
{ "thinkingBudgets": { "minimal": 1024, "low": 4096, "medium": 10240, "high": 32768 } }
```

## Display

`theme`, `quietStartup`, `collapseChangelog`, `doubleEscapeAction`,
`treeFilterMode`, `editorPaddingX`, `autocompleteMaxVisible`,
`showHardwareCursor` and `markdown.codeBlockIndent` shape the interface. An unset
`theme` reads as unset; the interface that renders it chooses its own dark
fallback, so the settings getter does not report a theme name you never wrote.
`terminal.showTerminalProgress` (stored, off by default) turns terminal progress
indicators on.

## Warnings

`warnings.anthropicExtraUsage` controls the paid-extra-usage warning for
subscription sign-in. The settings getter returns what is stored; the consumer
applies the default. The interactive mode shows the
[billing notice](../chat/commands.md#subscription-billing-notice) unless the value
is `false`.

```json
{ "warnings": { "anthropicExtraUsage": false } }
```

## Compaction and branch summaries

`compaction.enabled`, `compaction.reserveTokens` and `compaction.keepRecentTokens`
control automatic compaction; `branchSummary.reserveTokens` and
`branchSummary.skipPrompt` control branch summaries. The shipped reserve for both
is 16384 tokens. A larger reserve in your own file is your override, not a
default.

```json
{ "compaction": { "enabled": true, "reserveTokens": 16384, "keepRecentTokens": 20000 } }
```

## Retries

Agent-level retries are `retry.enabled`, `retry.maxRetries` and
`retry.baseDelayMs` (exponential: base, double, quadruple). Provider-level
requests use `retry.provider.timeoutMs`, `retry.provider.maxRetries` and
`retry.provider.maxRetryDelayMs`. When a provider asks to wait longer than the
last cap, the request fails with an explanation instead of waiting silently; `0`
disables the cap.

```json
{ "retry": { "enabled": true, "maxRetries": 3, "provider": { "timeoutMs": 3600000, "maxRetries": 0 } } }
```

## Delivery and transport

`steeringMode` and `followUpMode` are `all` or `one-at-a-time`. `transport`
prefers `sse`, `websocket`, `websocket-cached` or `auto`; the default is `auto`.

## Terminal and images

`terminal.showImages`, `terminal.imageWidthCells`, `terminal.clearOnShrink`,
`terminal.showTerminalProgress`, `images.autoResize` (to 2000 by 2000 pixels at
most) and `images.blockImages` (never send images to a model).

## Shell and package manager

`shellPath` selects the shell and `shellCommandPrefix` prefixes every shell
command. For Bash initialization and a dedicated alias file, see
[Use aliases in shell commands](../shell-aliases.md).

`npmCommand` is the argument vector used for every npm operation, including
installs, uninstalls and dependency installs inside git packages;
write it exactly as the process should be launched.

```json
{ "npmCommand": ["mise", "exec", "node@20", "--", "npm"] }
```

With a custom `npmCommand`, git package dependency installs use plain `install`.
The global modules location is read with `root -g`; when the first element is
`bun`, it is read with `pm bin -g` instead.

## Session directory

`sessionDir` is where session files are stored; it accepts absolute or relative
paths and `~`. When several sources set it, precedence is `--session-dir`, then
`MAESTRO_CODING_AGENT_SESSION_DIR`, then `sessionDir` in settings, then the
project's default directory described in the
[session format](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_format.md#file-location).

## Model cycling

`enabledModels` lists the patterns used when cycling models, in the same format
as the model-selection option. The interactive
[scope panel](../chat/commands.md#editing-the-model-scope) edits and saves it.

```json
{ "enabledModels": ["claude-*", "gpt-4o", "gemini-2*"] }
```

## Resources and packages

`extensions`, `skills`, `prompts` and `themes` list local files or directories;
`enableSkillCommands` registers skills as commands. Paths in the global file
resolve relative to the agent directory and paths in the project file relative to
the project's configuration directory; absolute paths and `~` work. Arrays accept
glob patterns, `!pattern` exclusions, `+path` to force-include and `-path` to
force-exclude an exact path. `packages` holds package sources as strings, or as
objects whose `source` is filtered by `extensions`, `skills`, `prompts` and
`themes` lists:

```json
{ "packages": ["example-skills", { "source": "example-skills", "skills": ["search"], "extensions": [] }] }
```

Resolution and installation belong to the
[package sources](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-sources.md)
and [package](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/packages.md)
guides.

## Telemetry, version checks and offline mode

Two separate network features have separate opt-outs, and the Maestro origin
that serves them is configurable and initially empty. Until an origin is
configured, no install report is sent, no latest-version check is made and no
share link is produced; this guide names no hostname.

| Feature | Endpoint path | Opt out |
| --- | --- | --- |
| Install and update report | `/api/report-install` | `enableInstallTelemetry: false` |
| Latest-version check | `/api/latest-version` | `MAESTRO_SKIP_VERSION_CHECK=1` |
| Session share link | `/session/` | do not share |

Turning off the install report does not disable the version check, and the
reverse. `--offline` or `MAESTRO_OFFLINE=1` disables every startup network
operation listed here, including package update checks. This crate stores the
`enableInstallTelemetry` preference only; sending and checking belong to the
[application hosted services](../application/services.md). Reporting captures
the preference and environment override when invoked; changing the preference
after preparation does not change that request.

## Example

```json
{
  "defaultProvider": "local",
  "defaultModel": "model-1",
  "defaultThinkingLevel": "medium",
  "compaction": { "enabled": true, "reserveTokens": 16384, "keepRecentTokens": 20000 },
  "retry": { "enabled": true, "maxRetries": 3 },
  "enabledModels": ["local/*"],
  "packages": ["example-skills"]
}
```
