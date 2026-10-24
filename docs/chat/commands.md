# Interactive commands

The interactive mode reads a few commands from the editor before ordinary
input. Submitted text is trimmed with the shared whitespace rule first: BOM and
no-break space count as whitespace, while U+0085 and zero-width space do not. A
command clears the editor and runs; it never answers the waiting input and never
enters the editor history. Any other text, including `/Model`, `/models`,
`/model<Tab>term` and `/scoped-models extra`, is ordinary input.

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
[scope warnings](../selection.md#scopes) to standard error, yellow when standard
error is a terminal: `Warning: No models match pattern "<pattern>"` for a pattern
that selects nothing, and the invalid thinking level warning for an unknown suffix.

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
