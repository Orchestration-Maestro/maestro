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

## Absolute-link qualification

The real-component probe uses ordinary Rust `std::fs` through WASI 0.2 and
`wasi:filesystem` 0.3 `open-at`/`read-via-stream` through actual extension
loading and callback dispatch. On an unprivileged Linux host:

| Observation | Rust / WASI 0.2 | Standard 0.3 | Native control |
| --- | --- | --- | --- |
| Direct path, relative file/directory links outside cwd | Exact bytes | Exact bytes | Not separately exercised |
| Absolute file/directory links outside cwd | Permission denied | `NotPermitted` | Exact bytes |
| Later target update through the same links | Still denied for absolute links | Still `NotPermitted` for absolute links | Updated bytes |
| Dangling absolute link | Permission denied | `NotPermitted` | Not found |
| Genuinely unreadable direct target | Permission denied | `Access` | Permission denied |
| Listing through an absolute directory link | Not exercised | Not exercised | Actual entry names |

Direct and relative-link reads also observe the updated bytes. The fixture
retains the original 0.3 error code beside its semantic error label, so a
runtime refusal cannot be mistaken for an operating-system denial.
Another preopen cannot change the pinned resolver. Its public context and
permission settings do not provide a replacement path resolver.

The selected native route is `maestro-app::extension_native_io::files`, the
single application file-effect owner planned in #2880. That interface is not
yet delivered: the guest `maestro_extensions_wasm::fs` facade currently exposes
watches, not byte reads or directory listings. No facade runtime success is
claimed here, and no replacement filesystem is added.

Before #2642 can use that route, #2880 must expose native byte reads and
directory listings through the existing native-I/O interface, canonical WIT,
guest facade and local pump. Those operations must follow absolute file and
directory links, observe a changed target through the same path, preserve
direct/relative controls, and retain native dangling-link and permission errors.
No path rewriting, canonicalization cache or second file-effect owner is needed.
The native control above records the expected behavior, not a guest guarantee.

#2642 remains open and depends on #2880; #207 remains open too. A later facade
success would not close the ordinary 0.2/standard 0.3 parity requirement.
Windows and macOS runtime behavior is not qualified by this Linux probe.

## Supplied stdout

`artifact_factory_with_stdout(path, widgets, installation, writer, terminal)`
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
