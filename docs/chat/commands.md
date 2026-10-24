# Interactive commands

The interactive mode reads submitted editor text before it becomes prompt input.
Text it recognizes as a command runs in the mode and answers on the transcript;
every other submission reaches the [typed input](interactive-mode.md#typed-input)
and the editor history as before.

Submitted text is trimmed with the shared whitespace rule first: BOM and
no-break space count as whitespace, while U+0085 and zero-width space do not. The
editor empties itself before calling back, so a command needs no clearing step; a
command never answers the waiting input and never enters the editor history. Any
other text, including `/Model`, `/models`, `/model<Tab>term` and
`/scoped-models extra`, is ordinary input.

The mode handles `/export`, `/model`, `/scoped-models` and `/settings`. Import,
the offline viewer and packaged asset injection are not delivered yet (see
[what is missing](#what-is-missing)).

## `/export`

`/export` with no path writes the whole session as an HTML document. A path
ending in exactly `.jsonl` writes the current branch as JSONL lines instead.

```text
/export
/export transcript.html
/export "notes from friday.jsonl"
```

Both formats reuse the [session export](../export.md) owners: the HTML document
comes from the same serializer and native bundle that
`AgentSession::export_to_html` uses, and the JSONL lines come from
`AgentSession::export_to_jsonl`. The mode adds no serializer, no asset fallback
and no second file writer.

A success shows `Session exported to: <path>` in the transcript's dim status
colour, naming the path the export owner returned. JSONL returns the
authored path resolved against the process directory; HTML returns the authored
path, or `maestro-session-<session file name>.html` when no path was given.

A recoverable failure shows `Failed to export session: <diagnostic>` in the
transcript's error colour, with the diagnostic the export owner produced. A
failure to render that notice goes to the terminal runtime's error route, which
is the route the mode's shortcuts already use.

## How the path is read

The reader recognizes the command token, then reads one path. It is token
reading, not shell parsing:

- The text must be exactly `/export`, or `/export` followed by one space. A
  prefix such as `/exporter` or `/export` followed by a tab is not the command,
  and that text is delivered as prompt input.
- After that single space, leading whitespace is removed under the shared
  [whitespace policy](../terminal/text.md): Unicode white space other than
  U+0085, plus U+FEFF.
- A quoted path returns the text between the first quote and the next matching
  quote, including empty text. An unterminated quote leaves no path, so the
  command exports HTML under the default name.
- A bare path ends at the first whitespace character. Later tokens are ignored.
- Nothing is unescaped, expanded or normalized: `~/notes.jsonl` and `"notes
  from friday.jsonl"` reach the export owner as authored.

The first matching quote wins, so `"/export/one"` is the path `/export/one` and a
second quote character inside the path ends it. Apostrophes are ordinary
characters unless the path starts with one.

## Which format is chosen

Only an exact lower-case `.jsonl` suffix selects JSONL. `.JSONL`,
`archive.jsonl.bak`, `notes.jsonl-summary` and an absent or empty path all select
HTML. The comparison stays case-sensitive on purpose: the reference command
selects JSONL for that suffix alone.

## Selecting a model

`/model` opens the model panel in place of the editor. `/model <term>` trims the
term and looks for a unique exact reference among the candidates: the session's
scoped models in their order, repeats included, or, without a scope, the
refreshed catalog's available models. A reference is `provider/id`, compared
case-insensitively with each side trimmed, or a bare id. When exactly one model
matches, it is applied at once; otherwise the panel opens with the term as its
search. Fuzzy matching is left to the panel's search.

The model-select key (`app.model.select`, Ctrl+L by default, configurable in
`keybindings.json`) also opens the panel. The panel saves the chosen model as the
default before the mode applies it. Escape puts the editor back and redraws.

Applying a model selects it on the session, refreshes the footer and the editor
border, puts the editor back when the panel was open and shows
`Model: <id>`. The id is read from the chosen model after the session's model
handlers ran, so a handler that renames it is reflected. When the session
refuses the model, for example because it has no credentials, the editor comes
back and the refusal is shown as `Error: <message>`; a change the session already
made is kept.

The footer shows the provider before the model name when the candidates span
more than one provider. The mode counts them at startup and after every scope
change.

## Editing the model scope

`/scoped-models` refreshes the catalog and opens the scope panel over every
available model, or shows `No models available` when there are none. A refresh
or availability failure is reported as an error instead. The panel starts from
the session's scope, in its order, when there is one; otherwise from the
`enabledModels` setting, resolved against the catalog. A missing or empty setting
enables every model. A setting that is not a list is treated as unset, and a list
with a member that is not text is rejected before any pattern is resolved.
Resolving the setting, and later each edit, writes the
[scope warnings](../selection.md#scopes) to standard error, yellow when
[`supports_color`](../theme.md) allows it:
`Warning: No models match pattern "<pattern>"` for a pattern that selects nothing,
and the invalid thinking level warning for an unknown suffix.

Every edit applies to the session at once but is not saved. A selection with
fewer models than the panel opened with becomes the session scope, in its order
and with any thinking level named in its patterns. Enabling every model, none,
or as many as the panel opened with clears the session scope. The provider count
is refreshed after every edit.

Saving (`app.models.save`) writes the selection to `enabledModels` in its order
and shows `Model selection saved to settings`; the panel stays open. Saving with
every model enabled, or as many as the panel opened with, removes the setting.
An empty or other explicit selection is saved as it is. Escape closes the panel,
keeping the session's edits and leaving the setting as it was.

## Changing settings

`/settings` opens the settings panel in place of the editor over the current
session and preferences: the panel reads the live session's compaction and queue
policies, the model's own thinking levels, the registered and discovered theme
inventory, and the typed preference values. A stored value of the wrong JSON type
counts as unset and the getter's default applies; the stored settings are never
rewritten by a read.

Choosing a value applies it at once and saves it through the settings owner:
session policies go to the session, which saves them, and the rest are written to
the settings file. Image visibility and width, padding, completion height,
thinking, the hardware cursor and the shrink policy update the retained rows,
editor and terminal straight away, and skill command admission rebuilds the
completion provider. Thinking visibility also rebuilds the transcript from the
session's messages. Themes publish immediately; a preview never saves, a commit
saves the requested name, and a theme that fails to load falls back to `dark`
with:

```text
Failed to load theme "<name>": <error>
Fell back to dark theme.
```

Every theme choice keeps the supplied watch effects, so a selected custom theme
is watched again after a preview or a commit. Escape closes the panel, restores
the editor with its focus and requests a frame. Forwarding to an installed
custom editor belongs to that editor's own work.

## Subscription billing notice

When the selected model belongs to the `anthropic` provider and a stored sign-in
or a key starting with `sk-ant-oat` pays for it, the mode shows once:

```text
Warning: Anthropic subscription auth is active. Third-party harness usage draws from extra usage and is billed per token, not your Claude plan limits. Manage extra usage at https://claude.ai/settings/usage.
```

The check runs after a model is applied or cycled. A stored sign-in is enough;
its key is not looked up or refreshed. Other providers, ordinary or missing keys
and failed key lookups show nothing, and a later check may still show the notice.
Setting `warnings.anthropicExtraUsage` to `false` turns the notice off; any other
value leaves it on. The notice is shown at most once for the life of the
interactive mode, even when its session is replaced, and checks that overlap
show it once. Checking at startup comes with the prompt loop.

## What is missing

- Import (`/import <path.jsonl>`) is not delivered. It waits for the shared
  dialog host and the session rebuild helpers, and it stays part of the same work.
- The complete offline viewer is separate work; the export tests supply a
  controlled bundle, so they prove the serializer wiring and not a shipped viewer.
- Packaging and asset injection are separate work. The mode reads the bundle the
  configuration's package directory selects, exactly as the other export callers
  do.
