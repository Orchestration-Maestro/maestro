//! One component root that resolves each native path when a component uses it.
//!
//! The standard filesystem interfaces reach only paths below the directories a
//! component lists when it starts. Windows has no directory above every drive
//! and network share, and a link with an absolute target leaves a listed
//! directory on every system. The component therefore lists one root, `/`.
//! For a path below it, the host opens the native directory that holds the path
//! with the user's own access, so the operating system resolves that directory,
//! and the standard implementation handles the final name in it.
use crate::lifecycle::State;
use maestro_extensions::helpers::native_path;
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use wasmtime::component::{HasData, Linker, Resource, ResourceTable};
use wasmtime_wasi::{
    FsPerms, OpenMode,
    filesystem::{Descriptor, Dir, WasiFilesystemCtxView},
};

mod p2;
mod p3;

/// The filesystem interfaces of both generations with one native root.
pub(crate) struct Root;
impl HasData for Root {
    type Data<'a> = View<'a>;
}

/// The instance's standard filesystem beside the directory behind its root.
pub(crate) struct View<'a> {
    /// The standard implementation every operation finishes in.
    pub(crate) fs: WasiFilesystemCtxView<'a>,
    /// The directory whose handle identifies the root.
    pub(crate) root: &'a Dir,
}

impl View<'_> {
    /// Lists the root under its component name, `/`.
    fn directories(&mut self) -> wasmtime::Result<Vec<(Resource<Descriptor>, String)>> {
        let root = self.fs.table.push(Descriptor::Dir(self.root.clone()))?;
        Ok(vec![(root, "/".into())])
    }
}

/// Replaces both generations' filesystem interfaces with the rooted ones.
pub(crate) fn link(linker: &mut Linker<State>) -> wasmtime::Result<()> {
    use wasmtime_wasi::{p2::bindings::filesystem as ordinary, p3::bindings::filesystem as next};
    linker.allow_shadowing(true);
    ordinary::types::add_to_linker::<State, Root>(linker, State::root_view)?;
    ordinary::preopens::add_to_linker::<State, Root>(linker, State::root_view)?;
    next::types::add_to_linker::<State, Root>(linker, State::root_view)?;
    next::preopens::add_to_linker::<State, Root>(linker, State::root_view)?;
    linker.allow_shadowing(false);
    Ok(())
}

/// Opens the directory behind the root itself: `/`, or on Windows the cwd's drive or share root.
pub(crate) fn open() -> io::Result<Dir> {
    match &std::env::current_dir() {
        Ok(cwd) if cfg!(windows) => directory(cwd.ancestors().last().unwrap_or(cwd)),
        _ => directory(Path::new("/")),
    }
}

/// Opens a native directory with the user's own access.
fn directory(path: &Path) -> io::Result<Dir> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    // FILE_FLAG_BACKUP_SEMANTICS lets Windows open a directory.
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, 0x0200_0000);
    Ok(Dir::new(
        options.open(path)?,
        FsPerms::ReadWrite,
        OpenMode::READ | OpenMode::WRITE,
        false,
    ))
}

/// A descriptor and the path to use with it, beside a native directory held for the operation.
type Placed = (Resource<Descriptor>, String, Option<Resource<Descriptor>>);

/// Whether `fd` is the root itself rather than a directory opened below it.
fn is_root(table: &ResourceTable, fd: &Resource<Descriptor>, root: &Dir) -> bool {
    matches!(table.get(fd), Ok(Descriptor::Dir(dir)) if Arc::ptr_eq(&dir.dir, &root.dir))
}

/// Where one path operation below the root runs.
enum Place {
    /// In the native directory that holds the path, under its final name.
    Native(Dir, String),
    /// In the root itself, under the whole path.
    Root(String),
}

/// Finds the native directory that holds `path` below the root, and the name to use in it.
///
/// A following lookup first follows links in the final name, as the system does.
/// A Unix directory the user may search but not list cannot be opened, so a path
/// below it stays with the root, where the standard implementation resolves each
/// component and an absolute link is refused.
fn parent(path: String, follow: bool) -> io::Result<Place> {
    let mut native = PathBuf::from(native_path(&format!("/{path}")));
    if follow {
        for _ in 0..40 {
            let Ok(target) = std::fs::read_link(&native) else {
                break;
            };
            native = native
                .parent()
                .map_or_else(|| target.clone(), |parent| parent.join(&target));
        }
    }
    let slash = if path.ends_with('/') { "/" } else { "" };
    let (directory_path, name) = match (native.parent(), native.file_name()) {
        (Some(parent), Some(name)) => (parent, format!("{}{slash}", name.to_string_lossy())),
        _ => (native.as_path(), ".".into()),
    };
    match directory(directory_path) {
        Ok(dir) => Ok(Place::Native(dir, name)),
        Err(error) if cfg!(unix) && error.kind() == io::ErrorKind::PermissionDenied => {
            Ok(Place::Root(path))
        }
        Err(error) => Err(error),
    }
}
