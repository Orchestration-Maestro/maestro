# Print mode

`run_print_mode` runs one session to completion and presents its result: the
final answer alone in text mode, or every session event as a JSON line in JSON
mode. The caller supplies the `AgentSessionRuntime` it already owns, so no
executor, input source or second session is created here. To launch JSON output
and consume its records, follow [Consume JSON session events](json-events.md).

## Options

`PrintModeOptions` carries the typed `PrintMode`, the later `messages`, the
`initial_message` and the `initial_images` of that first prompt. An omitted
message list is an empty vector; a message is sent only when it holds text, so an
absent or empty first message sends nothing and its images stay unsent. Nothing
is trimmed: whitespace, a byte-order mark and a next-line character reach the
prompt unchanged. Later messages are awaited one at a time with default prompt
options, and a rejection stops the remaining ones.

## Output

Text mode reads only the final history entry. An assistant error or abort writes
its supplied message to standard error, or `Request error` and `Request aborted`
when the message is empty, and returns 1. Any other ending writes each text block
with one added newline and omits thinking and tool-call blocks; an empty history
or a final entry that is not an assistant response writes nothing. Blocks are
written as they are, without trimming, cleaning or coalescing.

JSON mode writes the session's existing header once before the extensions are
bound, then one compact record for every subscribed session event through the
shared JSON line writer. Events that arrive during the first binding are not yet
subscribed. A represented assistant error or abort still returns 0 in this mode.
An event record the run cannot encode or write is kept and reported through the
ordinary error path: the run writes that message to standard error and returns 1
rather than letting a truncated stream look successful.

## Errors and cleanup

The returned status is ordinary: 0 for a completed run and 1 for a failed
operation, whose message is written to standard error. Cleanup failures reject
instead: a failed guarded disposal or a failed flush is returned as an error, and
a disposal failure skips the flush.

Every run registers termination signals before it starts. On a signal the run
kills the tracked detached children through the application interface, starts the
guarded disposal and exits with 128 plus the signal number once that disposal
settles; a repeated signal exits without waiting for the one already pending.
Normal completion and a caught error remove those handlers, dispose the runtime
once and flush raw standard output.

## Internals

The run drives the application runtime through adapter traits: the session and
runtime operations it needs, and the output, signal and child-process effects it
performs. These adapters are crate-private, so callers present a run only through
`run_print_mode`; the module's own tests supply their own to observe the same
runner. The six command operations are not bound here: the run asks the
application runtime for them, so one owner builds the operations a command
context receives.

A frontend that presents a run keeps no output global of its own; protocol and
final bytes go through the application's output guard.

## Exports

`maestro-cli` re-exports `PrintMode`, `PrintModeOptions` and `run_print_mode` at
its root. The application interface it calls is unchanged: child cleanup, the
session-change result, the JSON line writer and the command option records are
the existing application owners.
