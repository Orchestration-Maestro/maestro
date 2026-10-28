# Command-line input

For daily workflows, complete option and environment tables, and attachment
examples, read [daily use](usage.md). This page describes the library boundary.

The `maestro-cli` library prepares the input of a command-line run: it turns
authored arguments into retained input, prepares file and image attachments,
assembles the first message, renders the available-model table, selects what a
run does before its frontend starts, and runs the configuration and session
selectors through supplied presenters. It is a command-line frontend; every
selector widget stays in `maestro-chat`.

## Executables

Run the source executable with `cargo run --locked -p maestro -- --help`.
Use `--version` for version-only output, `--mode rpc` for the standard-stream
protocol, or no mode flag for interactive startup when stdin is a terminal.
Redirected stdin selects print mode. Non-interactive startup reserves stdout
for protocol or result records, including during help and version handling.

`maestro-model-account` is a separate binary for account help, provider listing
and login: `cargo run --locked -p maestro --bin maestro-model-account -- --help`.
It does not run session startup.

The executable loads authored extension components through the native component
adapter on its one local executor. Unix supplies the real terminal and selectors;
other native targets retain the terminal adapter's unsupported-operation boundary.
Package commands wait for their queued settings writes before the process returns.

## Arguments

`input::args::parse_args` consumes an authored argument list and retains it as
`Args`. Supplied options stay absent until authored, while messages, file
arguments, unknown flags and diagnostics always exist; an unknown flag keeps its
value in authored order and a repeated scalar keeps the last value. Diagnostics
name the option that carried them. `input::args::print_help` renders the complete
help document, including the registrations extensions contributed, and
`is_valid_thinking_level` accepts exactly the reasoning levels the agent takes.

## Files

`input::file_processor::process_file_arguments` walks the authored file
arguments in order and returns their wrapper text and image attachments. A
missing file and an unreadable text file are separate diagnostics; an image that
cannot be fitted leaves only its omission note and later arguments still append.
The walk runs over `FileArgumentOperations`, so a caller supplies the path,
existence, size, MIME, read and fit effects; `NativeFileArgumentOperations`
performs them on this machine.

## First message

`input::initial_message::build_initial_message` combines piped input, prepared
file text and the first authored prompt into one message and leaves the
remaining prompts queued.

## Model table

`input::list_models::list_models` prints the available models of a
`ModelRegistry` under the supplied locale comparator, with an optional fuzzy
search over provider and model. A failed `models.json` load prints the warning
before the table, an empty catalog prints the login guidance, and an unmatched
search prints its own line.

## Selectors

`input::config_selector::select_config` and
`input::session_picker::select_session` present a supplied component in a frame
writer: focus moves before the first frame, and everything a presentation
started stops with it, whether it closes, is cancelled or fails. The
configuration presentation starts a theme watch and stops it; the session
presentation has no theme and stops only the writer. The explicit-exit
notification ends the process after those effects.
`input::selector::SelectorHost::native` supplies the native terminal frame
writer. Before it presents, the session selector loads the configured
keybindings and installs that manager, so the search field and the list dispatch
the configured controls. `packages::handle_config_command` presents the
configuration selector over the configured paths it resolved from the loaded
settings. The records a presentation exchanges with its factory
live in `maestro_app::presentation_data::input_selectors`; this library
re-exports `ConfigSelectorOptions` from there and imports the rest from that
application path, so the interactive frontend and this library name one owner.
See [selectors](chat/selectors.md) for the widgets.

## Final mode dispatch

`startup::main::dispatch::dispatch_mode` hands a prepared session runtime to the
supplied interactive or RPC presentation, or to the existing print runner. It
reports the captured model scope in startup order and passes prepared prompts,
images and transport options unchanged. The caller retains the streams and
startup timing state.

`MAESTRO_STARTUP_BENCHMARK` selects interactive initialization only for `1`,
`true` or `yes` (ASCII case-insensitive for the words, without trimming). Other
modes reject that request. A benchmark initializes and stops the retained
presentation, closes the supplied theme watcher, then flushes stdout before
stderr. Ordinary interactive and RPC runs add no outer cleanup. After a print
run returns successfully, dispatch closes the watcher, restores managed stdout
and returns the print status, including nonzero statuses. Errors stop at their
original phase.

The library startup entry composes this dispatch with parsing, session selection
and runtime creation. The native executable supplies the concrete chat, RPC and selector adapters.

## Print mode

`run_print_mode` runs one session to completion over a caller-owned runtime and
presents the result as its final text or as session event records. See [print
mode](print.md) for the options, the exact output and the cleanup and error
boundary.

## Account commands

`model_login::cli` presents the standalone `help`, `list` and `login` commands
over an ordered provider snapshot, a live resolver and a console the caller
supplies. It keeps its own argument grammar and writes the caller-relative
account file, not the session credential store. See
[standalone account commands](model_login.md).

## Startup

`maestro_cli::main(args, options, host)` runs startup through the application
services and returns its requested exit status. `MainOptions` supplies ordered
extra extension factories. `startup::main::MainHost` supplies package operations,
selector presentations, mode launchers, streams, file preparation, the extension
module loader and timing observations. Borrowed input and effects stay with the
caller; only owned handles enter the retained runtime factory.

Package commands run before configuration commands, parsing and stdout takeover.
The caller's startup callback receives the raw offline selection before either
command. Non-interactive ordinary output uses the application's managed stdout
route; print and RPC protocol output keep their raw route. Help and model listing
run after extension registration but before fatal runtime diagnostics.

Text, JSON and RPC startup without a resolved model prints login guidance in red
on stderr and returns status 1 before benchmark validation or mode launch.
Runtime errors take precedence. Interactive startup still opens without a model;
a selected saved/default model does not need credentials to pass this check.

The retained factory rebuilds directory-bound settings, resources and providers
when a session changes directory. Replacements share the original auth store,
startup-relative resource paths, extra factories and stream handles. Each write
or flush borrows its stream only for that call.

`startup::main` selects what a run does before any frontend starts, in the
order the authored arguments decide it. `bootstrap` picks the presentation mode
from the parsed mode, the print request and the stdin kind; any run that is not
the interactive frontend reserves standard output first. Parser diagnostics,
settings failures and conflicting fork flags are written with their exact
prefix and style, and each reporting step returns the status that ends the run
when the problem is fatal. The session directory comes from the authored
argument, from the environment expanded through the supplied home lookup, or
from the stored setting, in that order.

`sessions` resolves a session argument to a file, searching the current project
before every project, and selects the operation the arguments name: an unsaved,
forked, opened, resumed, continued or new session. An explicit fork copies the
matched session into the current project without asking, wherever it was found;
a session argument of another project is returned for confirmation instead of
being opened in place. A missing session keeps the argument its message quotes.

`options` selects the model from the authored pattern before the scoped
default, the effort from the explicit level before the model shorthand and the
scope, and the tool policy from the argument. A local resource path is resolved
once against the original working directory; a package or URL source keeps its
spelling.

`inputs::read_piped_stdin` reads non-terminal input through EOF with replacement
for malformed UTF-8, then removes authored whitespace at the edges.
`prepare_initial_message` forwards file processing and image resizing to the
existing file owner before consuming the first queued prompt. `prompt_confirm`
flushes its question before reading an untrimmed answer; an unterminated answer
at EOF supplies no decision. These operations accept caller-owned streams and
file effects. The library entry composes them; native executable wiring remains
separate.

`presentation::prompt_for_missing_session_cwd` offers Continue and Cancel using
an application-supplied selection component. It loads the theme and keybindings
before constructing its frame writer, forwards saved cursor and clear-on-shrink
policies, and returns the fallback directory only for Continue. Cancellation,
repeated notifications and dropped operations share the existing selector's
one-shot cleanup. It does not start a theme watch or bind session services.
