//! The asynchronous generation's filesystem below the native root.
use super::{Place, Placed, Root, View, is_root, parent, read_link, symlink};
use crate::lifecycle::State;
use wasmtime::{
    AsContextMut,
    component::{Access, Accessor, FutureReader, Resource, StreamReader},
};
use wasmtime_wasi::{
    filesystem::{Descriptor, WasiFilesystem, WasiFilesystemView},
    p3::{
        bindings::filesystem::{
            preopens,
            types::{
                self, Advice, DescriptorFlags, DescriptorStat, DescriptorType, DirectoryEntry,
                ErrorCode, Filesize, HostDescriptorWithStore, MetadataHashValue, NewTimestamp,
                OpenFlags, PathFlags,
            },
        },
        filesystem::{FilesystemError, FilesystemResult},
    },
};

/// The standard implementation's accessor for the same store.
fn fs(store: &Accessor<State, Root>) -> Accessor<State, WasiFilesystem> {
    store.with_getter::<WasiFilesystem>(<State as WasiFilesystemView>::filesystem)
}

/// The standard implementation's access to the same store.
fn access<'a>(store: &'a mut Access<'_, State, Root>) -> Access<'a, State, WasiFilesystem> {
    Access::<State, WasiFilesystem>::new(
        store.as_context_mut(),
        <State as WasiFilesystemView>::filesystem,
    )
}

/// Whether `fd` is the root itself.
fn rooted(store: &Accessor<State, Root>, fd: &Resource<Descriptor>) -> bool {
    store.with(|mut access| {
        let view = access.get();
        is_root(view.fs.table, fd, view.root)
    })
}

/// The descriptor and name for `path` below `fd`, holding a native directory below the root.
async fn place(
    store: &Accessor<State, Root>,
    fd: Resource<Descriptor>,
    path: String,
    follow: bool,
) -> FilesystemResult<Placed> {
    if !rooted(store, &fd) {
        return Ok((fd, path, None));
    }
    match wasmtime_wasi::runtime::spawn_blocking(move || parent(path, follow)).await? {
        Place::Native(dir, name) => {
            let held = store.with(|mut access| access.get().fs.table.push(Descriptor::Dir(dir)))?;
            Ok((Resource::new_borrow(held.rep()), name, Some(held)))
        }
        Place::Root(path) => Ok((fd, path, None)),
    }
}

/// Places both paths of a two-path operation, closing the first directory if the second fails.
async fn place_both(
    store: &Accessor<State, Root>,
    (fd, path): (Resource<Descriptor>, String),
    (new_fd, new_path): (Resource<Descriptor>, String),
) -> FilesystemResult<(Placed, Placed)> {
    let first = place(store, fd, path, false).await?;
    match place(store, new_fd, new_path, false).await {
        Ok(second) => Ok((first, second)),
        Err(error) => {
            release(store, first.2)?;
            Err(error)
        }
    }
}

/// Closes a directory that [`place`] held.
fn release(
    store: &Accessor<State, Root>,
    held: Option<Resource<Descriptor>>,
) -> FilesystemResult<()> {
    if let Some(held) = held {
        store.with(|mut access| access.get().fs.table.delete(held))?;
    }
    Ok(())
}

impl preopens::Host for View<'_> {
    fn get_directories(&mut self) -> wasmtime::Result<Vec<(Resource<Descriptor>, String)>> {
        self.directories()
    }
}

impl types::Host for View<'_> {
    fn convert_error_code(&mut self, error: FilesystemError) -> wasmtime::Result<ErrorCode> {
        self.fs.convert_error_code(error)
    }
}

impl types::HostDescriptor for View<'_> {
    fn drop(&mut self, fd: Resource<Descriptor>) -> wasmtime::Result<()> {
        types::HostDescriptor::drop(&mut self.fs, fd)
    }
}

impl HostDescriptorWithStore<State> for Root {
    fn read_via_stream(
        mut store: Access<State, Self>,
        fd: Resource<Descriptor>,
        offset: Filesize,
    ) -> wasmtime::Result<(StreamReader<u8>, FutureReader<Result<(), ErrorCode>>)> {
        WasiFilesystem::read_via_stream(access(&mut store), fd, offset)
    }

    fn write_via_stream(
        mut store: Access<'_, State, Self>,
        fd: Resource<Descriptor>,
        data: StreamReader<u8>,
        offset: Filesize,
    ) -> wasmtime::Result<FutureReader<Result<(), ErrorCode>>> {
        WasiFilesystem::write_via_stream(access(&mut store), fd, data, offset)
    }

    fn append_via_stream(
        mut store: Access<'_, State, Self>,
        fd: Resource<Descriptor>,
        data: StreamReader<u8>,
    ) -> wasmtime::Result<FutureReader<Result<(), ErrorCode>>> {
        WasiFilesystem::append_via_stream(access(&mut store), fd, data)
    }

    async fn advise(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        offset: Filesize,
        length: Filesize,
        advice: Advice,
    ) -> FilesystemResult<()> {
        WasiFilesystem::advise(&fs(store), fd, offset, length, advice).await
    }

    async fn sync_data(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
    ) -> FilesystemResult<()> {
        WasiFilesystem::sync_data(&fs(store), fd).await
    }

    async fn get_flags(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
    ) -> FilesystemResult<DescriptorFlags> {
        WasiFilesystem::get_flags(&fs(store), fd).await
    }

    async fn get_type(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
    ) -> FilesystemResult<DescriptorType> {
        WasiFilesystem::get_type(&fs(store), fd).await
    }

    async fn set_size(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        size: Filesize,
    ) -> FilesystemResult<()> {
        WasiFilesystem::set_size(&fs(store), fd, size).await
    }

    async fn set_times(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        atim: NewTimestamp,
        mtim: NewTimestamp,
    ) -> FilesystemResult<()> {
        WasiFilesystem::set_times(&fs(store), fd, atim, mtim).await
    }

    fn read_directory(
        mut store: Access<'_, State, Self>,
        fd: Resource<Descriptor>,
    ) -> wasmtime::Result<(
        StreamReader<DirectoryEntry>,
        FutureReader<Result<(), ErrorCode>>,
    )> {
        WasiFilesystem::read_directory(access(&mut store), fd)
    }

    async fn sync(store: &Accessor<State, Self>, fd: Resource<Descriptor>) -> FilesystemResult<()> {
        WasiFilesystem::sync(&fs(store), fd).await
    }

    async fn create_directory_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        path: String,
    ) -> FilesystemResult<()> {
        let (fd, path, held) = place(store, fd, path, false).await?;
        let result = WasiFilesystem::create_directory_at(&fs(store), fd, path).await;
        release(store, held)?;
        result
    }

    async fn stat(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
    ) -> FilesystemResult<DescriptorStat> {
        WasiFilesystem::stat(&fs(store), fd).await
    }

    async fn stat_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
    ) -> FilesystemResult<DescriptorStat> {
        let (fd, path, held) =
            place(store, fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW)).await?;
        let result = WasiFilesystem::stat_at(&fs(store), fd, flags, path).await;
        release(store, held)?;
        result
    }

    async fn set_times_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
        atim: NewTimestamp,
        mtim: NewTimestamp,
    ) -> FilesystemResult<()> {
        let (fd, path, held) =
            place(store, fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW)).await?;
        let result = WasiFilesystem::set_times_at(&fs(store), fd, flags, path, atim, mtim).await;
        release(store, held)?;
        result
    }

    async fn link_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        old_path: String,
        new_fd: Resource<Descriptor>,
        new_path: String,
    ) -> FilesystemResult<()> {
        let ((fd, old_path, old), (new_fd, new_path, new)) =
            place_both(store, (fd, old_path), (new_fd, new_path)).await?;
        let result =
            WasiFilesystem::link_at(&fs(store), fd, flags, old_path, new_fd, new_path).await;
        release(store, old)?;
        release(store, new)?;
        result
    }

    async fn open_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
        open: OpenFlags,
        descriptor: DescriptorFlags,
    ) -> FilesystemResult<Resource<Descriptor>> {
        let (fd, path, held) =
            place(store, fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW)).await?;
        let result = WasiFilesystem::open_at(&fs(store), fd, flags, path, open, descriptor).await;
        release(store, held)?;
        result
    }

    async fn readlink_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        path: String,
    ) -> FilesystemResult<String> {
        if !rooted(store, &fd) {
            return WasiFilesystem::readlink_at(&fs(store), fd, path).await;
        }
        wasmtime_wasi::runtime::spawn_blocking(move || read_link(&path))
            .await?
            .ok_or_else(|| ErrorCode::IllegalByteSequence.into())
    }

    async fn remove_directory_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        path: String,
    ) -> FilesystemResult<()> {
        let (fd, path, held) = place(store, fd, path, false).await?;
        let result = WasiFilesystem::remove_directory_at(&fs(store), fd, path).await;
        release(store, held)?;
        result
    }

    async fn rename_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        old_path: String,
        new_fd: Resource<Descriptor>,
        new_path: String,
    ) -> FilesystemResult<()> {
        let ((fd, old_path, old), (new_fd, new_path, new)) =
            place_both(store, (fd, old_path), (new_fd, new_path)).await?;
        let result = WasiFilesystem::rename_at(&fs(store), fd, old_path, new_fd, new_path).await;
        release(store, old)?;
        release(store, new)?;
        result
    }

    async fn symlink_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        old_path: String,
        new_path: String,
    ) -> FilesystemResult<()> {
        if !rooted(store, &fd) {
            return WasiFilesystem::symlink_at(&fs(store), fd, old_path, new_path).await;
        }
        Ok(wasmtime_wasi::runtime::spawn_blocking(move || symlink(&old_path, &new_path)).await?)
    }

    async fn unlink_file_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        path: String,
    ) -> FilesystemResult<()> {
        let (fd, path, held) = place(store, fd, path, false).await?;
        let result = WasiFilesystem::unlink_file_at(&fs(store), fd, path).await;
        release(store, held)?;
        result
    }

    async fn is_same_object(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        other: Resource<Descriptor>,
    ) -> wasmtime::Result<bool> {
        WasiFilesystem::is_same_object(&fs(store), fd, other).await
    }

    async fn metadata_hash(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
    ) -> FilesystemResult<MetadataHashValue> {
        WasiFilesystem::metadata_hash(&fs(store), fd).await
    }

    async fn metadata_hash_at(
        store: &Accessor<State, Self>,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
    ) -> FilesystemResult<MetadataHashValue> {
        let (fd, path, held) =
            place(store, fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW)).await?;
        let result = WasiFilesystem::metadata_hash_at(&fs(store), fd, flags, path).await;
        release(store, held)?;
        result
    }
}

#[cfg(test)]
mod tests;
