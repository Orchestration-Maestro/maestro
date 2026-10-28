# SSH

Build with `--example ssh`. Load the component into a native component-enabled
host and pass `--ssh user@host:/remote/path`, or `--ssh user@host` to discover the
remote directory with `pwd`. Without the flag, the registered read, write, edit
and bash tools use the host's original local operations. With it, those tools
and user `!` commands run through SSH. Session startup reports the selected host
and directory; the supplied system prompt names that remote directory.
If directory discovery fails, these tools and user `!` commands return that error
instead of running locally. Clearing the flag restores local operations.

Loading and remote execution are opt-in. The local host needs `ssh` on `PATH`,
key-based authentication and a trusted host key already configured, with no
password or host-key prompts. The remote needs a POSIX shell, bash, `cat`, `test`,
`mkdir` and `file` for image classification. The example transfers original file
bytes and writes UTF-8 content through a child input pipe. Shell quoting keeps
paths literal, including apostrophes and shell substitutions. Paths beneath the
local working directory map to the remote directory; other absolute paths stay
absolute. Windows host separators become POSIX separators; backslashes in POSIX
file names stay literal.

SSH uses the user's own connection configuration and permissions: remote reads
can expose private data, and writes, edits and shell commands can change or delete
remote files. Cancellation and bash deadlines terminate the local SSH process;
they cannot guarantee that work already accepted remotely stops. No connection
pool or remote rollback is provided. The controlled `ssh_` runs use disposable
files and a checked child process instead of a live server, with no provider,
credentials or paid service.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- ssh_maps_windows
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- ssh_
```
