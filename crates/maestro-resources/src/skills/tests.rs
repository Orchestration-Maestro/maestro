//! Process-directory context shared by every path resolution.

use super::operations::with_process_context;
#[cfg(not(target_arch = "wasm32"))]
use super::real_path::real_path;
use super::{LoadSkillsOptions, ResourceEntry, ResourceFileType, ResourceOperations, load_skills};
use maestro_path::win32;
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
};

/// Adapter with fixed process directories over a filesystem holding at most one file.
struct Process {
    /// The process working directory, or none when it cannot be read.
    current: Option<String>,
    /// The current directory of each drive.
    drives: Vec<(char, String)>,
    /// The only file that exists, with its text.
    file: Option<(String, String)>,
    /// Every path `exists` was asked about, in order.
    asked: RefCell<Vec<PathBuf>>,
}

impl Process {
    /// Whether `path` is the one file.
    fn holds(&self, path: &Path) -> bool {
        self.file
            .as_ref()
            .is_some_and(|(file, _)| Path::new(file) == path)
    }
}

impl ResourceOperations for Process {
    fn exists(&self, path: &Path) -> bool {
        self.asked.borrow_mut().push(path.to_owned());
        self.holds(path)
    }
    fn read_dir(&self, _: &Path) -> io::Result<Vec<ResourceEntry>> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn read_file(&self, _: &Path) -> io::Result<String> {
        self.file
            .as_ref()
            .map(|(_, text)| text.clone())
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        if self.holds(path) {
            Ok(ResourceFileType::File)
        } else {
            Err(io::ErrorKind::NotFound.into())
        }
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn current_directory(&self) -> io::Result<String> {
        self.current
            .clone()
            .ok_or_else(|| io::ErrorKind::PermissionDenied.into())
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        self.drives.clone()
    }
}

/// Report `current` and `entries` as the process directories of an empty filesystem.
fn process(current: &str, entries: &[(char, &str)]) -> Process {
    Process {
        current: Some(current.to_owned()),
        drives: entries
            .iter()
            .map(|(letter, directory)| (*letter, (*directory).to_owned()))
            .collect(),
        file: None,
        asked: RefCell::default(),
    }
}

/// Load `skill_paths` for caller directory `cwd`, with `agent_dir` as the user configuration.
fn load(
    adapter: &Process,
    (cwd, agent_dir): (&str, &str),
    skill_paths: &[&str],
    include_defaults: bool,
) -> super::LoadSkillsResult {
    load_skills(
        LoadSkillsOptions {
            cwd,
            home: "/home",
            agent_dir,
            config_dir_name: ".maestro",
            skill_paths: &skill_paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>(),
            include_defaults,
        },
        adapter,
    )
}

/// A relative caller directory is an operand: it continues from the process directory.
#[cfg(not(windows))]
#[test]
fn explicit_path_beneath_a_relative_cwd_continues_from_the_process_directory() {
    let result = load(
        &process("/process", &[]),
        ("work", "/agent"),
        &["notes.md"],
        false,
    );
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some("/process/work/notes.md")
    );
}

/// An unreadable process directory matters only to a path that no operand anchors.
#[cfg(not(windows))]
#[test]
fn explicit_paths_need_the_process_directory_only_beneath_a_relative_cwd() {
    let mut adapter = process("/process", &[]);
    adapter.current = None;
    for (cwd, located) in [("work", "work/notes.md"), ("/work", "/work/notes.md")] {
        let result = load(&adapter, (cwd, "/agent"), &["notes.md"], false);
        assert_eq!(result.diagnostics[0].path.as_deref(), Some(located));
    }
}

/// A relative path fails with the working-directory error kind; an absolute path does not.
#[cfg(unix)]
#[test]
fn real_path_needs_the_working_directory_only_for_a_relative_path() {
    let mut adapter = process("/process", &[]);
    adapter.current = None;
    let error = real_path(Path::new("é.md"), &adapter).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(real_path(Path::new("/"), &adapter).unwrap(), Path::new("/"));
}

/// The project defaults directory is the caller directory joined with the configuration name.
#[cfg(not(windows))]
#[test]
fn project_defaults_beneath_a_relative_cwd_continue_from_the_process_directory() {
    let adapter = process("/process", &[]);
    let _ = load(&adapter, ("work", "/agent"), &[], true);
    assert_eq!(
        *adapter.asked.borrow(),
        [
            PathBuf::from("/agent/skills"),
            PathBuf::from("/process/work/.maestro/skills")
        ]
    );
}

/// A relative user root classifies explicit paths beneath the process directory, not the cwd.
#[cfg(not(windows))]
#[test]
fn relative_user_root_continues_from_the_process_directory_not_the_cwd() {
    let mut adapter = process("/process", &[]);
    adapter.file = Some((
        "/process/agent/skills/calendar.md".into(),
        "---\ndescription: Calendar\n---".into(),
    ));
    let result = load(
        &adapter,
        ("/work", "agent"),
        &["/process/agent/skills/calendar.md"],
        false,
    );
    assert_eq!(result.skills[0].source_info.scope, crate::SourceScope::User);
}

/// The caller directory wins on its own drive; another drive continues from its own entry.
#[test]
fn resolution_context_continues_other_drives_from_their_reported_directories() {
    let adapter = process(
        r"C:\Users\me",
        &[('C', r"C:\Users\me"), ('D', r"D:\skills")],
    );
    with_process_context(&adapter, |context, _| {
        assert_eq!(context.current, r"C:\Users\me");
        assert_eq!(
            win32::resolve(&[r"C:\work", r"C:notes.md"], context),
            r"C:\work\notes.md"
        );
        assert_eq!(
            win32::resolve(&[r"C:\work", r"D:calendar\SKILL.md"], context),
            r"D:\skills\calendar\SKILL.md"
        );
    });
}

/// A drive without a reported directory continues from that drive's root.
#[test]
fn resolution_context_falls_back_to_the_drive_root_without_an_entry() {
    let adapter = process(r"C:\Users\me", &[('C', r"C:\Users\me")]);
    with_process_context(&adapter, |context, _| {
        assert_eq!(
            win32::resolve(&[r"C:\work", r"D:calendar\SKILL.md"], context),
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

/// An explicit drive-relative skill path on the cwd's drive continues from the cwd.
#[cfg(windows)]
#[test]
fn explicit_skill_path_on_the_cwd_drive_continues_from_the_cwd() {
    let adapter = process(r"C:\Users\me", &[('C', r"C:\Users\me")]);
    let result = load(&adapter, (r"C:\work", r"C:\agent"), &["C:notes.md"], false);
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(r"C:\work\notes.md")
    );
}

/// An explicit drive-relative skill path without a drive entry continues from the drive root.
#[cfg(windows)]
#[test]
fn explicit_skill_path_without_a_drive_entry_continues_from_the_drive_root() {
    let adapter = process(r"C:\Users\me", &[]);
    let result = load(
        &adapter,
        (r"C:\work", r"C:\agent"),
        &[r"D:calendar\SKILL.md"],
        false,
    );
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(r"D:\calendar\SKILL.md")
    );
}

/// An explicit drive-relative skill path on another drive continues from that drive's directory.
#[cfg(windows)]
#[test]
fn explicit_skill_path_on_another_drive_continues_from_the_drive_directory() {
    let adapter = process(r"C:\Users\me", &[('D', r"D:\skills")]);
    let result = load(
        &adapter,
        (r"C:\work", r"C:\agent"),
        &[r"D:calendar\SKILL.md"],
        false,
    );
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(r"D:\skills\calendar\SKILL.md")
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
    let adapter = process(r"C:\Users\me", &[(letter, text)]);
    let resolved = real_path(Path::new(&format!("{letter}:item.md")), &adapter);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(resolved.unwrap(), directory.join("item.md"));
}
