# Component operating environment

The native host supplies one owned WASI context and resource table to both the
standard-library 0.2 interfaces and the native asynchronous 0.3 interfaces.
The existing Store thread runs their I/O and timer drivers; application
capabilities keep their original local owners.

Environment keys and values are copied from the native environment, including
empty values. Native strings use replacement for invalid Unicode. The host
removes any inherited `MAESTRO_EXTENSION_PATH` entry before adding the selected
artifact path for this instance. Neither another instance nor the inherited
value can replace it. Other entries are unchanged.

Stdin, stdout and stderr are inherited separately. Guest buffers still need to
be flushed when immediate visibility matters. Network name lookup, TCP and UDP
use the user's network without an added address filter.

On Unix the host preopens `/` for reading and writing, rather than restricting
access to the extension directory. It supplies the native current directory
when it can observe it. Guest export startup applies that observation once to
Rust’s standard-library cwd before the author factory runs; its libc would
otherwise start at `/`. Failure to observe cwd does not prevent loading an
accessible absolute artifact: no initial directory is supplied, and the guest
keeps its libc default. Filesystem operations still report ordinary
operating-system failures.

This is not complete native filesystem parity. Relative symbolic links can
leave cwd, but the pinned filesystem implementation rejects absolute symbolic
links; support remains open in #2642. Atomic saves, file modes and Windows root
qualification remain undelivered. Final application-mode output routing remains
separate from these host capabilities.

## Supplied stdout

`artifact_factory_with_stdout(path, widgets, installation_version, writer, terminal)`
retains a standard `Write + Send` sink across initialization and later
callbacks. Both standard output generations share that sink and report the
supplied terminal observation. Closing one guest output view does not close the
sink. The ordinary `artifact_factory` continues to inherit stdout.

`ProcessTerminal::output_writer()` returns a cloneable, `Send + Sync` writer
while queued terminal output runs. Each write copies its bytes once into the
same ordered ingress as complete host frames. Flush checks admission; it does
not wait for physical delivery. Stop closes admission, drains accepted bytes
through the existing partial-write owner, then restores descriptor flags.
Retained handles return `BrokenPipe` after closure. Stopped terminals and
synchronous regular-file destinations return no handle.

The executable's final selection of this supplied route is not wired here.
The ordered-output guarantee applies to active queued destinations, not to
inherited stdout redirected to a regular file.
