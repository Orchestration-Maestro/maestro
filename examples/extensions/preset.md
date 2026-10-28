# Preset

Build with `--example preset` using the shared build command. The component reads
`presets.json` in `MAESTRO_CODING_AGENT_DIR` (default `~/.maestro/agent`) and
`<cwd>/.maestro/presets.json` at session start. A project entry replaces the whole
global entry with the same name. Missing files are ignored; unreadable files,
invalid JSON or invalid field types report a diagnostic and leave the other
layer available.

Each file maps names to optional `provider`, `model`, `thinkingLevel`, `tools`
and `instructions` fields. For example:

```json
{
  "plan": {
    "thinkingLevel": "high",
    "tools": ["read", "grep", "find", "ls"],
    "instructions": "Understand the problem before proposing changes."
  }
}
```

`--preset plan` applies the preset before session startup completes, including
model selection. `/preset plan` selects it directly; `/preset` opens **Select
Preset**. Ctrl+Shift+U cycles names in sorted order, including `(none)` to clear.
Clearing restores the model, thinking level and tools captured before the first
application. Without a snapshot it restores `read`, `bash`, `edit` and `write`.
Unknown models, missing authentication and unknown tools produce warnings;
nonempty valid tool selections still apply. Empty or wholly invalid tool lists
leave the host's tools unchanged.

Active instructions append to the supplied system prompt. The footer shows
`preset:<name>`. Turn-start entries retain the active name, including a cleared
choice, on the current branch. Session start and tree navigation restore only
the newest name on that branch, without reapplying model or tool settings.
The flag takes precedence over the saved name.

Loading is opt-in and needs a component-enabled native host with filesystem and
environment access; browsing the selector also needs custom UI and retained
widgets. Adding `provider` and `model` selects that exact configured registry
entry and needs its authentication for real requests. This example changes
session settings and appends session entries, but writes no configuration file.
Real prompts can incur provider charges. The `preset_` controlled runs use
read-only fixture files, supplied registry replies and terminal input; they
need no credentials, installation or paid services.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- preset_
```
