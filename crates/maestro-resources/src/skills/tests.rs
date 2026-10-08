//! Drive-directory context shared by every path resolution.

use super::operations::with_cwd;
#[cfg(windows)]
use super::{LoadSkillsOptions, load_skills, real_path::real_path};
use super::{ResourceEntry, ResourceFileType, ResourceOperations};
use maestro_path::win32;
use std::{
    io,
    path::{Path, PathBuf},
};

/// Adapter with fixed drive directories over an empty filesystem.
struct Drives(Vec<(char, String)>);

impl ResourceOperations for Drives {
    fn exists(&self, _: &Path) -> bool {
        false
    }
    fn read_dir(&self, _: &Path) -> io::Result<Vec<ResourceEntry>> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn read_file(&self, _: &Path) -> io::Result<String> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        self.0.clone()
    }
}

/// Report `entries` as the drive directories.
fn drives(entries: &[(char, &str)]) -> Drives {
    Drives(
        entries
            .iter()
            .map(|(letter, directory)| (*letter, (*directory).to_owned()))
            .collect(),
    )
}

/// A drive-relative path continues from the directory the adapter reports for its drive.
#[test]
fn resolution_context_carries_the_reported_drive_directories() {
    let adapter = drives(&[('C', r"C:\Users\me"), ('D', r"D:\skills")]);
    with_cwd(r"C:\work", &adapter, |cwd| {
        assert_eq!(cwd.current, r"C:\work");
        assert_eq!(
            win32::resolve(&[r"D:calendar\SKILL.md"], cwd),
            r"D:\skills\calendar\SKILL.md"
        );
        assert_eq!(
            win32::resolve(&[r"C:notes.md"], cwd),
            r"C:\Users\me\notes.md"
        );
    });
}

/// A drive without a reported directory continues from that drive's root.
#[test]
fn resolution_context_falls_back_to_the_drive_root_without_an_entry() {
    let adapter = drives(&[('C', r"C:\Users\me")]);
    with_cwd(r"C:\work", &adapter, |cwd| {
        assert_eq!(
            win32::resolve(&[r"D:calendar\SKILL.md"], cwd),
            r"D:\calendar\SKILL.md"
        );
    });
}

/// The native adapter reads the `=X:` variable of each drive, in letter order.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_drive_directories_come_from_equals_prefixed_variables() {
    let variables = [
        ("=D:", r"D:\skills"),
        ("=C:", r"C:\work"),
        ("=1:", "ignored"),
        ("D", "ignored"),
    ];
    let found = super::operations::env_drive_directories(|name| {
        variables
            .iter()
            .find(|(variable, _)| *variable == name)
            .map(|(_, directory)| (*directory).into())
    });
    assert_eq!(
        found,
        [('C', r"C:\work".to_owned()), ('D', r"D:\skills".to_owned())]
    );
}

/// Load one explicit skill path and return the path its warning names.
#[cfg(windows)]
fn explicit_path_warning(adapter: &Drives, skill_path: &str) -> Option<String> {
    let result = load_skills(
        LoadSkillsOptions {
            cwd: r"C:\work",
            home: r"C:\home",
            agent_dir: r"C:\agent",
            config_dir_name: ".maestro",
            skill_paths: &[skill_path.to_owned()],
            include_defaults: false,
        },
        adapter,
    );
    result.diagnostics.into_iter().next()?.path
}

/// An explicit drive-relative skill path continues from its drive's directory.
#[cfg(windows)]
#[test]
fn explicit_skill_path_continues_from_the_drive_directory() {
    let adapter = drives(&[('D', r"D:\skills")]);
    assert_eq!(
        explicit_path_warning(&adapter, r"D:calendar\SKILL.md").as_deref(),
        Some(r"D:\skills\calendar\SKILL.md")
    );
}

/// An explicit drive-relative skill path without a drive entry continues from the drive root.
#[cfg(windows)]
#[test]
fn explicit_skill_path_without_a_drive_entry_continues_from_the_drive_root() {
    let adapter = drives(&[]);
    assert_eq!(
        explicit_path_warning(&adapter, r"D:calendar\SKILL.md").as_deref(),
        Some(r"D:\calendar\SKILL.md")
    );
}

/// A real path continues a drive-relative path from the reported drive directory.
#[cfg(windows)]
#[test]
fn real_path_continues_from_the_drive_directory() {
    let directory = std::env::temp_dir().join(format!("maestro-real-path-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("item.md"), "item").unwrap();
    let text = directory.to_str().unwrap();
    let letter = text.chars().next().unwrap().to_ascii_uppercase();
    let adapter = drives(&[(letter, text)]);
    let resolved = real_path(Path::new(&format!("{letter}:item.md")), &adapter);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(resolved.unwrap(), directory.join("item.md"));
}
