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
`active_filename()` returns an owned observation of that shared selection slot.
`show_artifact(filename)` selects a literal name and updates its view without
calling the host-open notification. Empty and unknown names remain observable;
`open_artifact(filename)` additionally notifies only for an existing map key.

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
`browser::tools::artifacts::LocalArtifactsPanel::new_local` instead returns a panel
retaining the existing local execution pointer, permitting native `Rc` captures; its
`tool()` then builds the local executable tool over that panel's command state. Both
panels use the same command state, declarations and policy.
The module also exposes `LocalArtifactPresentation` and the typed `FileType`.
These local types do not change the default SDK tool's thread-safety.

## History reconstruction

`reconstruct_from_messages` rebuilds the panel from a conversation. Assistant
artifact calls are collected by identifier, where a later duplicate wins; every
message is then folded in order. A recorded artifact entry applies its action,
and one whose content is empty is skipped; a successful artifact tool result
applies only when the live command would accept the same arguments against the
file replayed so far, so a rejected command leaves no trace. Replayed arguments
pass through the shared argument checker, so a numeric `content` arrives as its
string form. `update` replaces the first literal occurrence, keeps dollar
sequences and empty replacements literal, and keeps a file whose content the
replacement emptied.

Reconstruction empties the previous map handle, removes every old view and
materializes the files silently, without the HTML log wait or per-file
notifications; every HTML view reloads after each file, so all views end on the
final state. It then selects the first file when nothing is selected and notifies
the host once.

## Sandbox artifact access

`ArtifactsPanelState` retains the map slot, the selection and the optional
conversation independently of the presentation operations, so a view operation
can read them without the panel. The state never owns the conversation: the
caller supplies it and frees it by dropping it, and a view operation reads it
only while the caller's handle is alive.
`get_html_artifact_runtime_providers` returns the conversation's attachments
provider when it has attachments, followed by read-only access to the panel's
artifacts. The read-only provider reads the live store and never the conversation;
the attachments provider holds the attachments taken when the providers are
built, so callers build providers per view operation to observe a replaced
conversation.

`ArtifactsRuntimeProvider` gives sandboxed code access to these artifacts. It
injects a snapshot of every artifact at `get_data`, embeds its helper source at
`get_runtime`, answers `artifact-operation` messages for `list`, `get`,
`createOrUpdate` and `delete`, and returns one of the two guidance constants for
its access mode. Reads answer from the live map. Writes run the panel's own
commands, so a command the panel rejects replies with its failure text and records
nothing, while an accepted write appends its history entry to the conversation
collection when one was supplied. Read-only access refuses the write actions
before any command runs, keeps no history and has no view effects, and reads
keep working. The embedded helper finds an existing empty artifact by presence
rather than by content and serializes non-string content once.

## Browser integration

This interface does not mount browser views or format console records. HTML view
mounting is owned by [#1761](https://github.com/Orchestration-Maestro/maestro/issues/1761),
and tool cards by [#820](https://github.com/Orchestration-Maestro/maestro/issues/820).
Sandbox corrections remain in [#104](https://github.com/Orchestration-Maestro/maestro/issues/104)
and [#105](https://github.com/Orchestration-Maestro/maestro/issues/105).

The [native browser panel](../browser/artifacts.md#native-panel) owns a local
presentation adapter over these commands and exposes it through `application`.
Its native element lookup returns the live format owner for independent headers.
Standalone artifact bodies and headers mount independently through `DomContent`.
The [retained interface](../browser/artifacts.md#retained-panel) still accepts a
caller-supplied presentation for connection and drawing; tab activation uses the
same application selection operation.
