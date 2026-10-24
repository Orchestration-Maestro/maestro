# Command-line input

The `maestro-cli` library prepares the input of a command-line run: it turns
authored arguments into retained input, prepares file and image attachments,
assembles the first message, renders the available-model table and runs the
configuration and session selectors through supplied presenters. It is a
command-line frontend; every selector widget stays in `maestro-chat`.

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

## Account commands

`model_login::cli` presents the standalone `help`, `list` and `login` commands
over an ordered provider snapshot, a live resolver and a console the caller
supplies. It keeps its own argument grammar and writes the caller-relative
account file, not the session credential store. See
[standalone account commands](model_login.md).

## Print mode

`print::print_mode::run_print_mode` runs one session to completion over a
caller-owned runtime and presents the result as its final text or as session
event records. See [print mode](print.md) for the options, the exact output and
the cleanup and error boundary.
