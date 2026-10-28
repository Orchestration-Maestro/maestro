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
use maestro_extensions::helpers::{guest_path, native_path};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use wasmtime::component::{HasData, Linker, Resource, ResourceTable, ResourceTableError};
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
    /// Descriptors awaiting reclamation after their operation ends.
    pub(crate) retired: Retired,
}

impl View<'_> {
    /// Lists the root under its component name, `/`.
    fn directories(&mut self) -> wasmtime::Result<Vec<(Resource<Descriptor>, String)>> {
        self.retired.drain(self.fs.table);
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
type Placed = (Resource<Descriptor>, String, Option<Held>);

/// Per-instance reclamation of private temporary directory entries.
#[derive(Clone, Default)]
pub(crate) struct Retired {
    /// Only the store-access boundary mutates the resource table.
    pending: Arc<Mutex<Vec<u32>>>,
}

impl Retired {
    /// Reclaims retired entries before the table is exposed to another operation.
    pub(crate) fn drain(&self, table: &mut ResourceTable) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for rep in pending.drain(..) {
            // These private entries have no children and are retired exactly once.
            let _ = table.delete::<Descriptor>(Resource::new_own(rep));
        }
    }
}

/// Retires a temporary descriptor without requiring store access from Drop.
struct Held {
    /// The table entry owned by this one operation.
    rep: u32,
    /// The instance's shared reclamation queue.
    retired: Retired,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.retired
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.rep);
    }
}

impl View<'_> {
    /// Places a native directory in the table under cancellation-safe ownership.
    fn hold(&mut self, dir: Dir, name: String) -> Result<Placed, ResourceTableError> {
        let rep = self.fs.table.push(Descriptor::Dir(dir))?.rep();
        Ok((
            Resource::new_borrow(rep),
            name,
            Some(Held {
                rep,
                retired: self.retired.clone(),
            }),
        ))
    }
}

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
    /// A native final name that the string-based interface cannot represent.
    InvalidName,
    /// The final-component chain exceeds the lookup bound.
    Loop,
}

/// Finds the native directory that holds `path` below the root, and the name to use in it.
///
/// A following lookup first follows links in the final name, as the system does.
/// A Unix directory the user may search but not list cannot be opened, so a path
/// below it stays with the root, where the standard implementation resolves each
/// component and an absolute link is refused.
fn parent(path: String, follow: bool) -> io::Result<Place> {
    let native = PathBuf::from(native_path(&format!("/{path}")));
    let native = if follow {
        // The observer cannot fail; an error here means the link bound was exceeded.
        let Ok(target) =
            maestro_path::resolve_final_links(&native, |path| Ok(std::fs::read_link(path).ok()))
        else {
            return Ok(Place::Loop);
        };
        target
    } else {
        native
    };
    // Path components discard a trailing dot, but the kernel must still see it.
    let bytes = native.as_os_str().as_encoded_bytes();
    let dot = bytes
        .rsplit(|byte| std::path::is_separator(char::from(*byte)))
        .find(|component| !component.is_empty())
        == Some(b".".as_slice());
    let slash = if bytes
        .last()
        .is_some_and(|byte| std::path::is_separator(char::from(*byte)))
    {
        "/"
    } else {
        ""
    };
    let (directory_path, name) = if dot {
        (native.as_path(), ".".into())
    } else if let (Some(parent), Some(name)) = (native.parent(), native.file_name()) {
        let Some(name) = name.to_str() else {
            return Ok(Place::InvalidName);
        };
        (parent, format!("{name}{slash}"))
    } else {
        (native.as_path(), ".".into())
    };
    match directory(directory_path) {
        Ok(dir) => Ok(Place::Native(dir, name)),
        Err(error) if cfg!(unix) && error.kind() == io::ErrorKind::PermissionDenied => {
            Ok(Place::Root(path))
        }
        Err(error) => Err(error),
    }
}

/// Reads the target of a link below the root as the system does, in the component's spelling.
///
/// `None` stands for a target that is not Unicode.
fn read_link(path: &str) -> io::Result<Option<String>> {
    let target = std::fs::read_link(native_path(&format!("/{path}")))?;
    Ok(target.to_str().map(guest_path))
}

/// Creates a link below the root to a target in the component's spelling.
///
/// The target is kept as given, absolute or relative, and the system resolves
/// it when the link is used. A Windows link needs its target's kind: a target
/// that is not a directory, including a missing one, gets a file link.
fn symlink(target: &str, path: &str) -> io::Result<()> {
    let link = PathBuf::from(native_path(&format!("/{path}")));
    let target = PathBuf::from(native_path(target));
    #[cfg(windows)]
    let created = if link.parent().unwrap_or(&link).join(&target).is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    };
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(target, link);
    created
}
