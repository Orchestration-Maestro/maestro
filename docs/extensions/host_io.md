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
when it can observe it. Failure to observe cwd does not prevent loading an
accessible absolute artifact. Ordinary operating-system failures stay errors.

This is not complete native filesystem parity. Relative symbolic links can
leave cwd, but the pinned filesystem implementation rejects absolute symbolic
links; support remains open in #2642. Atomic saves, file modes and Windows root
qualification remain undelivered. Final application-mode output routing remains
separate from these host capabilities.
