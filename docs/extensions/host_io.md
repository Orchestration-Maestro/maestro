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

Stdin and stderr are inherited separately. Stdout follows the native terminal's
output lifecycle, described below. Guest buffers still need to be flushed when
immediate visibility matters. Network name lookup, TCP and UDP use the user's
network without an added address filter.

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
qualification remain undelivered.

## Supplied stdout

`artifact_factory_with_stdout(path, widgets, installation, writer, terminal)`
retains a standard `Write + Send` sink across initialization and later
callbacks. Both standard output generations share that sink and report the
supplied terminal observation. Closing one guest output view does not close the
sink. `Stdout::new(writer, terminal)` supplies the same sink as the optional fifth
argument of `artifact_factory_with_io`, alongside the caller's native operations.
The ordinary `artifact_factory` continues to inherit stdout.

`ProcessTerminal::output_writer()` returns a cloneable, `Send + Sync` writer
while queued terminal output runs. Each write copies its bytes once into the
same ordered ingress as complete host frames. Flush checks admission; it does
not wait for physical delivery. Stop closes admission, drains accepted bytes
through the existing partial-write owner, then restores descriptor flags.
Retained handles return `BrokenPipe` after closure. Stopped terminals and
synchronous regular-file destinations return no handle.

The executable creates one unstarted native terminal before loading extensions.
Its factories retain `ProcessTerminal::standard_output_writer()`; the interactive
frontend starts that same terminal. This durable writer selects the current
admission sender on every call, including after reload and suspend/resume. Stop
holds inherited writes behind the final drain and descriptor restoration. A
failed queued writer reports its error until stop; it does not bypass the queue.

Before start, after stop and for regular files, the durable writer uses inherited
stdout. Writes report partial progress; explicit flush calls deliver buffered bytes
and report flush errors. Both guest output generations forward flush to that writer.
Print, JSON and RPC therefore keep stdout, not a suppressed stream or a stderr substitute. Extensions that write
stdout in protocol modes can still interfere with their protocol records. The
ordered-output guarantee applies only while queued terminal output runs; it does
not add a second queue or writer task. Windows interactive startup remains tracked
in [#2844](https://github.com/Orchestration-Maestro/maestro/issues/2844).
