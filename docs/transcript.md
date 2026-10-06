# Transcript byte access

`Storage` provides raw byte access at explicit caller-supplied paths. The session owner selects paths and owns history, formats and persistence timing. Storage does not parse JSON, decode UTF-8, frame lines, deduplicate bytes or normalize paths.

The eight synchronous operations are `exists`, `mkdir`, `read_file`, `read_prefix`, `read_dir`, `modified`, `append_file` and `write_file`. Existence follows targets and failed observations return false. Only mkdir recursively creates directories. Reads create nothing; append and write create files only when their parents exist. Empty appends still create files. An empty path returns `NotFound` for every I/O operation, synchronous or asynchronous, and false for existence; it never denotes the current directory.

Prefix reads open even for a zero-byte request. A nonzero request performs one bounded positional read at offset zero and returns only the bytes actually read, including incomplete UTF-8. A failed positional read intentionally retains the opened descriptor; successful reads and failed opens do not leak descriptors.

Append writes the supplied bytes in call order without framing. Write truncates the existing file in place, removing its old suffix. Both follow symbolic links, retain ordinary permissions and preserve the target identity rather than replacing it by rename. I/O errors retain native error data and may follow partial changes; they do not poison later operations. No transaction, flush guarantee, retry policy, byte cap or timeout is added.

Name-only directory listing returns every name without fetching member metadata. Modification time is a separate selected-target observation. The asynchronous kind listing returns owned `Dirent` values with File, Directory, SymbolicLink or Other classifications of the entries themselves, including dangling links. Native Unix listings use unsigned filename-byte order; controlled adapters retain their configured order. Windows enumeration equivalence still requires runtime qualification before release.

The four asynchronous operations are `read_file_async`, `modified_async`, `read_dir_async` and `read_dir_with_file_types_async`. Each admits owned work immediately, independent of polling, and publishes its own raw result on completion. Dropping the future abandons observation, not admitted work. No runtime-library type, aggregate wait, progress callback or application sorting policy is exposed.

`FileStorage` is a native unit adapter with one private process-shared pool of four filesystem workers. The fixed four-worker pool is the selected Rust library equivalent; it does not read another runtime's pool-size environment knob or promise identical scheduling. Queueing is uncapped, filesystem work runs outside the caller thread and future polling performs no I/O. Thread-creation failure returns an I/O error; a later admission can try initialization again.

The object-safe `Storage` interface, directory records and reusable `conformance` assertions are available to browser builds without requiring Send or Sync. `FileStorage` is native-only. A browser build verifies interface and controlled-adapter portability, not native filesystem support. The conformance assertions use supplied fresh roots and one unchanged caller against independent adapters.

This isolated native example writes and appends opaque bytes without selecting a session or configuration directory:

```rust
#[cfg(not(target_arch = "wasm32"))]
{
    use maestro_storage::{FileStorage, Storage};
    let root = std::env::temp_dir().join(format!("maestro-byte-example-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let storage = FileStorage;
    let directory = root.join("nested");
    storage.mkdir(&directory)?;
    let file = directory.join("bytes");
    storage.write_file(&file, b"\0\xff")?;
    storage.append_file(&file, b"\r\n")?;
    assert_eq!(storage.read_file(&file)?, b"\0\xff\r\n");
    std::fs::remove_dir_all(root)?;
}
# Ok::<(), std::io::Error>(())
```
