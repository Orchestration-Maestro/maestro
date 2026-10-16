# Artifact commands

[`ArtifactsPanel`](../../crates/maestro-app/src/browser/tools/artifacts.rs)
retains an ordered collection of mutable, dated files. Its `tool()` returns an
executable `artifacts` declaration that remains usable after the caller releases
the panel. The application SDK exports `Artifact`, `ArtifactsParams` and
`ArtifactsPanel`.

## Commands

- `create` requires a nonempty filename and content; duplicate filenames fail.
- `update` requires an existing file, nonempty `old_str` and present `new_str`.
  Only the first literal match changes. Empty replacement is allowed; dollar
  sequences have no special meaning.
- `rewrite` requires an existing file and nonempty content. Its non-HTML success
  text is empty.
- `get` returns content unchanged; `delete` removes the record and view.
- `logs` reads the current HTML view's console output. Missing and non-HTML views
  return different messages, independently of record presence.

Command failures are returned text, not tool execution errors. The model schema
requires `command` and `filename`; the shared argument checker prepares accepted
scalar strings before typed execution. The executable itself requires an object
with a string command tag. Unknown properties are allowed. Filenames and content
are not trimmed or interpreted as filesystem paths.

## Retained state and presentation

`artifacts()` returns the current mutable ordered-map handle. Create and delete
mutate that map before replacing its container with a shallow copy. Surviving map
aliases keep their observations, and retained record handles survive deletion.
Update and rewrite edit the existing record in place. Create observes the injected
clock twice, once for each date; an edit observes it once for the update date.

`ArtifactPresentation` supplies clock, view, notification and wait operations.
Callbacks run without application state or record locks held. Mutation effects
publish state before updating views and notifying the host; dependent HTML views
refresh afterward. Deleting the selected file selects the first remaining file,
or clears the selection if no file remains. Opening an unknown file does nothing.

`browser::tools::artifacts::get_file_type` is the single filename discriminator.
It selects HTML, SVG, Markdown, image, PDF, spreadsheet, Word, text/code or generic
from the last literal dot-separated segment with Unicode lowercase. It does not
trim whitespace or normalize paths. HTML selection (including the filename
`html`) triggers the HTML path. Normal HTML mutations
capture the selected view's log reader before requesting a 1,500 ms wait, then
append a newline and that reader's current output. The host owns wait execution;
the application adds no timer runtime. A missing HTML view appends only the
newline without waiting.

The model guidance uses `command` and `logs`, matching the executable schema.
The SDK also exports the read-only artifact storage and attachment runtime
descriptions; their actual runtime operations are separate from this command
owner.

`ArtifactsPanel` defaults to the existing shared presentation pointer.
`browser::tools::artifacts::LocalArtifactsPanel::new_local` instead retains the
existing local execution pointer and returns a local executable tool, permitting
native `Rc` captures. Both use the same command state, declarations and policy.
The module also exposes `LocalArtifactPresentation` and the typed `FileType`.
These local types do not change the default SDK tool's thread-safety.

## Browser integration

This interface does not mount browser views or format console records. Artifact
reconstruction and runtime access remain in [#753](https://github.com/Orchestration-Maestro/maestro/issues/753),
the tabbed/overlay panel in [#1108](https://github.com/Orchestration-Maestro/maestro/issues/1108),
HTML views in [#819](https://github.com/Orchestration-Maestro/maestro/issues/819),
and tool cards in [#820](https://github.com/Orchestration-Maestro/maestro/issues/820).
Sandbox corrections remain in [#104](https://github.com/Orchestration-Maestro/maestro/issues/104)
and [#105](https://github.com/Orchestration-Maestro/maestro/issues/105).

The [generic-file view](../browser/artifacts.md) is embeddable independently of
the panel shell and accepts the existing replaceable native host.
