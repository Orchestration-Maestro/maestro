//! The ordinary generation's filesystem below the native root.
use super::{Place, Placed, View, is_root, parent, read_link, symlink};
use wasmtime::component::Resource;
use wasmtime_wasi::{
    filesystem::Descriptor,
    p2::{
        DynInputStream, DynOutputStream, FsError, FsResult,
        bindings::filesystem::{
            preopens,
            types::{
                self, Advice, DescriptorFlags, DescriptorStat, DescriptorType, DirectoryEntry,
                DirectoryEntryStream, ErrorCode, Filesize, HostDescriptor,
                HostDirectoryEntryStream, MetadataHashValue, NewTimestamp, OpenFlags, PathFlags,
            },
        },
    },
};

impl View<'_> {
    /// The descriptor and name for `path` below `fd`, holding a native directory below the root.
    async fn place(
        &mut self,
        fd: Resource<Descriptor>,
        path: String,
        follow: bool,
    ) -> FsResult<Placed> {
        if !is_root(self.fs.table, &fd, self.root) {
            return Ok((fd, path, None));
        }
        match wasmtime_wasi::runtime::spawn_blocking(move || parent(path, follow)).await? {
            Place::Native(dir, name) => {
                let held = self.fs.table.push(Descriptor::Dir(dir))?;
                Ok((Resource::new_borrow(held.rep()), name, Some(held)))
            }
            Place::Root(path) => Ok((fd, path, None)),
        }
    }

    /// Places both paths of a two-path operation, closing the first directory if the second fails.
    async fn place_both(
        &mut self,
        (fd, path): (Resource<Descriptor>, String),
        (new_fd, new_path): (Resource<Descriptor>, String),
    ) -> FsResult<(Placed, Placed)> {
        let first = self.place(fd, path, false).await?;
        match self.place(new_fd, new_path, false).await {
            Ok(second) => Ok((first, second)),
            Err(error) => {
                self.release(first.2)?;
                Err(error)
            }
        }
    }

    /// Closes a directory that [`View::place`] held.
    fn release(&mut self, held: Option<Resource<Descriptor>>) -> FsResult<()> {
        if let Some(held) = held {
            self.fs.table.delete(held)?;
        }
        Ok(())
    }
}

impl preopens::Host for View<'_> {
    fn get_directories(&mut self) -> wasmtime::Result<Vec<(Resource<Descriptor>, String)>> {
        self.directories()
    }
}

impl types::Host for View<'_> {
    fn convert_error_code(&mut self, err: FsError) -> wasmtime::Result<ErrorCode> {
        self.fs.convert_error_code(err)
    }

    fn filesystem_error_code(
        &mut self,
        err: Resource<wasmtime::Error>,
    ) -> wasmtime::Result<Option<ErrorCode>> {
        self.fs.filesystem_error_code(err)
    }
}

impl HostDirectoryEntryStream for View<'_> {
    async fn read_directory_entry(
        &mut self,
        stream: Resource<DirectoryEntryStream>,
    ) -> FsResult<Option<DirectoryEntry>> {
        self.fs.read_directory_entry(stream).await
    }

    fn drop(&mut self, stream: Resource<DirectoryEntryStream>) -> wasmtime::Result<()> {
        HostDirectoryEntryStream::drop(&mut self.fs, stream)
    }
}

impl HostDescriptor for View<'_> {
    async fn advise(
        &mut self,
        fd: Resource<Descriptor>,
        offset: Filesize,
        len: Filesize,
        advice: Advice,
    ) -> FsResult<()> {
        self.fs.advise(fd, offset, len, advice).await
    }

    async fn sync_data(&mut self, fd: Resource<Descriptor>) -> FsResult<()> {
        self.fs.sync_data(fd).await
    }

    async fn get_flags(&mut self, fd: Resource<Descriptor>) -> FsResult<DescriptorFlags> {
        self.fs.get_flags(fd).await
    }

    async fn get_type(&mut self, fd: Resource<Descriptor>) -> FsResult<DescriptorType> {
        self.fs.get_type(fd).await
    }

    async fn set_size(&mut self, fd: Resource<Descriptor>, size: Filesize) -> FsResult<()> {
        self.fs.set_size(fd, size).await
    }

    async fn set_times(
        &mut self,
        fd: Resource<Descriptor>,
        atim: NewTimestamp,
        mtim: NewTimestamp,
    ) -> FsResult<()> {
        self.fs.set_times(fd, atim, mtim).await
    }

    async fn read(
        &mut self,
        fd: Resource<Descriptor>,
        len: Filesize,
        offset: Filesize,
    ) -> FsResult<(Vec<u8>, bool)> {
        self.fs.read(fd, len, offset).await
    }

    async fn write(
        &mut self,
        fd: Resource<Descriptor>,
        buf: Vec<u8>,
        offset: Filesize,
    ) -> FsResult<Filesize> {
        self.fs.write(fd, buf, offset).await
    }

    async fn read_directory(
        &mut self,
        fd: Resource<Descriptor>,
    ) -> FsResult<Resource<DirectoryEntryStream>> {
        self.fs.read_directory(fd).await
    }

    async fn sync(&mut self, fd: Resource<Descriptor>) -> FsResult<()> {
        self.fs.sync(fd).await
    }

    async fn create_directory_at(
        &mut self,
        fd: Resource<Descriptor>,
        path: String,
    ) -> FsResult<()> {
        let (fd, path, held) = self.place(fd, path, false).await?;
        let result = self.fs.create_directory_at(fd, path).await;
        self.release(held)?;
        result
    }

    async fn stat(&mut self, fd: Resource<Descriptor>) -> FsResult<DescriptorStat> {
        self.fs.stat(fd).await
    }

    async fn stat_at(
        &mut self,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
    ) -> FsResult<DescriptorStat> {
        let (fd, path, held) = self
            .place(fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW))
            .await?;
        let result = self.fs.stat_at(fd, flags, path).await;
        self.release(held)?;
        result
    }

    async fn set_times_at(
        &mut self,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
        atim: NewTimestamp,
        mtim: NewTimestamp,
    ) -> FsResult<()> {
        let (fd, path, held) = self
            .place(fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW))
            .await?;
        let result = self.fs.set_times_at(fd, flags, path, atim, mtim).await;
        self.release(held)?;
        result
    }

    async fn link_at(
        &mut self,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        old_path: String,
        new_fd: Resource<Descriptor>,
        new_path: String,
    ) -> FsResult<()> {
        let ((fd, old_path, old), (new_fd, new_path, new)) =
            self.place_both((fd, old_path), (new_fd, new_path)).await?;
        let result = self.fs.link_at(fd, flags, old_path, new_fd, new_path).await;
        self.release(old)?;
        self.release(new)?;
        result
    }

    async fn open_at(
        &mut self,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
        open: OpenFlags,
        descriptor: DescriptorFlags,
    ) -> FsResult<Resource<Descriptor>> {
        let (fd, path, held) = self
            .place(fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW))
            .await?;
        let result = self.fs.open_at(fd, flags, path, open, descriptor).await;
        self.release(held)?;
        result
    }

    fn drop(&mut self, fd: Resource<Descriptor>) -> wasmtime::Result<()> {
        HostDescriptor::drop(&mut self.fs, fd)
    }

    async fn readlink_at(&mut self, fd: Resource<Descriptor>, path: String) -> FsResult<String> {
        if !is_root(self.fs.table, &fd, self.root) {
            return self.fs.readlink_at(fd, path).await;
        }
        wasmtime_wasi::runtime::spawn_blocking(move || read_link(&path))
            .await?
            .ok_or_else(|| ErrorCode::IllegalByteSequence.into())
    }

    async fn remove_directory_at(
        &mut self,
        fd: Resource<Descriptor>,
        path: String,
    ) -> FsResult<()> {
        let (fd, path, held) = self.place(fd, path, false).await?;
        let result = self.fs.remove_directory_at(fd, path).await;
        self.release(held)?;
        result
    }

    async fn rename_at(
        &mut self,
        fd: Resource<Descriptor>,
        old_path: String,
        new_fd: Resource<Descriptor>,
        new_path: String,
    ) -> FsResult<()> {
        let ((fd, old_path, old), (new_fd, new_path, new)) =
            self.place_both((fd, old_path), (new_fd, new_path)).await?;
        let result = self.fs.rename_at(fd, old_path, new_fd, new_path).await;
        self.release(old)?;
        self.release(new)?;
        result
    }

    async fn symlink_at(
        &mut self,
        fd: Resource<Descriptor>,
        old_path: String,
        new_path: String,
    ) -> FsResult<()> {
        if !is_root(self.fs.table, &fd, self.root) {
            return self.fs.symlink_at(fd, old_path, new_path).await;
        }
        Ok(wasmtime_wasi::runtime::spawn_blocking(move || symlink(&old_path, &new_path)).await?)
    }

    async fn unlink_file_at(&mut self, fd: Resource<Descriptor>, path: String) -> FsResult<()> {
        let (fd, path, held) = self.place(fd, path, false).await?;
        let result = self.fs.unlink_file_at(fd, path).await;
        self.release(held)?;
        result
    }

    fn read_via_stream(
        &mut self,
        fd: Resource<Descriptor>,
        offset: Filesize,
    ) -> FsResult<Resource<DynInputStream>> {
        self.fs.read_via_stream(fd, offset)
    }

    fn write_via_stream(
        &mut self,
        fd: Resource<Descriptor>,
        offset: Filesize,
    ) -> FsResult<Resource<DynOutputStream>> {
        self.fs.write_via_stream(fd, offset)
    }

    fn append_via_stream(
        &mut self,
        fd: Resource<Descriptor>,
    ) -> FsResult<Resource<DynOutputStream>> {
        self.fs.append_via_stream(fd)
    }

    async fn is_same_object(
        &mut self,
        a: Resource<Descriptor>,
        b: Resource<Descriptor>,
    ) -> wasmtime::Result<bool> {
        self.fs.is_same_object(a, b).await
    }

    async fn metadata_hash(&mut self, fd: Resource<Descriptor>) -> FsResult<MetadataHashValue> {
        self.fs.metadata_hash(fd).await
    }

    async fn metadata_hash_at(
        &mut self,
        fd: Resource<Descriptor>,
        flags: PathFlags,
        path: String,
    ) -> FsResult<MetadataHashValue> {
        let (fd, path, held) = self
            .place(fd, path, flags.contains(PathFlags::SYMLINK_FOLLOW))
            .await?;
        let result = self.fs.metadata_hash_at(fd, flags, path).await;
        self.release(held)?;
        result
    }
}

#[cfg(test)]
mod tests;
