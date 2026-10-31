# Standalone account commands

`maestro_cli::model_login::cli` presents the account command a user runs without
starting an agent session: `help`, `list` and `login` over the OAuth providers in
the snapshot the caller supplies, which exists before any account file does. It is
a library, not a process.
[Command-line input](cli.md) covers the session frontend that shares this crate;
the two argument grammars stay separate.

## What the caller supplies

`cli::main` takes the arguments after the command name, the ordered provider
snapshot, a live resolver and a console. The snapshot decides which providers a
table lists and which identifiers a login accepts. The resolver decides which
implementation serves a login, and it is asked only after the snapshot accepted
that identifier, so replacing or withdrawing a provider implementation between
the question and the login is visible. Implementation:
`crates/maestro-cli/src/model_login/cli.rs`.

An Anthropic login listens for the browser's redirect on local port 53692. When
another program holds that port, the login uses a free local port and names it in the
redirect address, so sign-in still finishes; see the [account flows](models/oauth.md).

The help examples name `maestro-model-account`. The standalone executable and
the terminal console it runs on belong to the command-line startup frontend.

## The two host traits

`AccountConsole` owns the host effects: `write` publishes complete text and adds
no newline of its own, sending it to the error stream when asked; and
`create_interface` opens a fresh `AccountInput` for one interaction.

`AccountInput` owns one question interface. `prompt` receives the complete
question the command would display, including its trailing space, and returns
the answer untrimmed; `close` releases the interface. The command closes the
selection interface before login opens its own, and always closes the login
interface when that login finishes, including when the provider kept its
callback handle. Neither trait implements line editing, scheduling or a
terminal: an adapter supplies them.

## Account file

A successful login merges into `auth.json` in the caller's working directory
through the application's caller-path operations
(`maestro_app::model_account::cli`). A missing, unreadable or unparsable file
loads as an empty object; an existing object keeps every unrelated provider and
member. The entry is the account tag followed by the supplied credential
members, with two-space JSON and no final newline, and a supplied `type` member
takes the tag's slot. A valid array or scalar root is refused with the owner's
file-shape error and left untouched, so a login cannot silently drop stored
credentials. The file is stored through the credentials crate's `write_auth_file`
([credentials](credentials.md)): it is written in place under the shared recoverable
publication, so a write cut short is completed by the next coordinated writer, and it is
readable only by its owner on Unix. Errors keep the `Error:` line and exit status one.

This is not the session credential store: the command only reads and writes the
standalone account file at that caller-relative path, and offers no path option.
