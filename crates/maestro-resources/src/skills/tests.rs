//! Process-directory context shared by every path resolution.

use super::operations::ProcessContext;
#[cfg(not(target_arch = "wasm32"))]
use super::real_path::real_path;
#[cfg(not(windows))]
use super::{LoadSkillsFromDirOptions, load_skills_from_dir};
use super::{
    LoadSkillsOptions, LoadSkillsResult, ResourceEntry, ResourceFileType, ResourceOperations,
    load_skills,
};
use maestro_path::{Cwd, win32};
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
};

/// Adapter with fixed process directories over a small virtual filesystem.
struct Process {
    /// The process working directory, or none when it cannot be read.
    current: Option<String>,
    /// The current directory of each drive.
    drives: Vec<(char, String)>,
    /// Every file that exists, with its text.
    files: Vec<(String, String)>,
    /// Every directory that exists, with its entries in listing order.
    directories: Vec<(String, Vec<(String, ResourceFileType)>)>,
    /// Every path `exists` was asked about, in order.
    asked: RefCell<Vec<PathBuf>>,
    /// Each process-directory observation requested, in order.
    observed: RefCell<Vec<&'static str>>,
}

impl Process {
    /// Add the existing directory `path` listing `entries`.
    #[cfg(not(windows))]
    fn with_directory(mut self, path: &str, entries: &[(&str, ResourceFileType)]) -> Self {
        let entries = entries
            .iter()
            .map(|(name, kind)| ((*name).to_owned(), *kind))
            .collect();
        self.directories.push((path.to_owned(), entries));
        self
    }
    /// Add the existing file `path` holding `text`.
    #[cfg(not(windows))]
    fn with_file(mut self, path: &str, text: &str) -> Self {
        self.files.push((path.to_owned(), text.to_owned()));
        self
    }
    /// The text of the file `path`.
    fn text(&self, path: &Path) -> Option<&str> {
        self.files
            .iter()
            .find(|(file, _)| Path::new(file) == path)
            .map(|(_, text)| text.as_str())
    }
}

impl ResourceOperations for Process {
    fn exists(&self, path: &Path) -> bool {
        self.asked.borrow_mut().push(path.to_owned());
        self.text(path).is_some()
            || self
                .directories
                .iter()
                .any(|(directory, _)| Path::new(directory) == path)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        let (_, entries) = self
            .directories
            .iter()
            .find(|(directory, _)| Path::new(directory) == path)
            .ok_or(io::ErrorKind::NotFound)?;
        Ok(entries
            .iter()
            .map(|(name, file_type)| ResourceEntry {
                name: name.into(),
                file_type: *file_type,
            })
            .collect())
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        self.text(path)
            .map(str::to_owned)
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        if self.text(path).is_some() {
            Ok(ResourceFileType::File)
        } else {
            Err(io::ErrorKind::NotFound.into())
        }
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        Err(io::ErrorKind::NotFound.into())
    }
    fn current_directory(&self) -> io::Result<String> {
        self.observed.borrow_mut().push("current_directory");
        self.current
            .clone()
            .ok_or_else(|| io::ErrorKind::PermissionDenied.into())
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        self.observed.borrow_mut().push("drive_directories");
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
        files: Vec::new(),
        directories: Vec::new(),
        asked: RefCell::default(),
        observed: RefCell::default(),
    }
}

/// Load `skill_paths` for caller directory `cwd`, with `agent_dir` as the user configuration.
fn load(
    adapter: &Process,
    (cwd, agent_dir): (&str, &str),
    skill_paths: &[&str],
    include_defaults: bool,
) -> io::Result<LoadSkillsResult> {
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
    )
    .unwrap();
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some("/process/work/notes.md")
    );
}

/// An unreadable process directory fails the loader for a relative caller directory and not for an anchored one.
#[cfg(not(windows))]
#[test]
fn explicit_paths_need_the_process_directory_only_beneath_a_relative_cwd() {
    let mut adapter = process("/process", &[]);
    adapter.current = None;
    for (paths, include_defaults) in [
        (&["notes.md"][..], false),
        (&["/abs/notes.md"], false),
        (&[], true),
    ] {
        let error = load(&adapter, ("work", "/agent"), paths, include_defaults).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{paths:?}");
    }
    let result = load(&adapter, ("/work", "/agent"), &["notes.md"], false).unwrap();
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some("/work/notes.md")
    );
}

/// A user root that needs the unreadable process directory turns the classification of an existing path into its warning.
#[cfg(not(windows))]
#[test]
fn scope_classification_needing_the_unreadable_process_directory_warns_for_that_path() {
    let mut adapter = process("/process", &[]);
    adapter.current = None;
    let adapter = adapter.with_file("/work/notes.md", "---\ndescription: Notes\n---");
    let result = load(
        &adapter,
        ("/work", "agent"),
        &["notes.md", "gone.md"],
        false,
    )
    .unwrap();
    assert!(result.skills.is_empty());
    let found: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.path.as_deref(), d.message.as_str()))
        .collect();
    let denied = io::Error::from(io::ErrorKind::PermissionDenied).to_string();
    assert_eq!(
        found,
        [
            (Some("/work/notes.md"), denied.as_str()),
            (Some("/work/gone.md"), "skill path does not exist")
        ]
    );
}

/// A skill file with a description.
#[cfg(not(windows))]
const SKILL: &str = "---\ndescription: Calendar\n---";

/// Scan `dir` for caller directory `cwd` through the public directory loader.
#[cfg(not(windows))]
fn scan(adapter: &Process, cwd: &str, dir: &str) -> LoadSkillsResult {
    load_skills_from_dir(
        LoadSkillsFromDirOptions {
            cwd,
            dir,
            source: "path",
        },
        adapter,
    )
}

/// A scan root that is its own operand never needs the process directory.
#[cfg(not(windows))]
#[test]
fn directory_loading_does_not_read_the_process_directory_for_the_scan_root_alone() {
    let mut adapter = process("/process", &[]).with_directory(".", &[]);
    adapter.current = None;
    let result = scan(&adapter, ".", ".");
    assert!(result.skills.is_empty() && result.diagnostics.is_empty());
    assert!(adapter.observed.borrow().is_empty());
}

/// An entry that needs an unreadable process directory ends its directory quietly; a readable one loads, reading once.
#[cfg(not(windows))]
#[test]
fn directory_loading_ends_a_directory_quietly_when_an_entry_needs_the_unreadable_process_directory()
{
    use ResourceFileType::{Directory, File};
    let layouts = [
        (
            ".",
            process("/process", &[])
                .with_directory(".", &[("SKILL.md", File)])
                .with_file("SKILL.md", SKILL),
            "SKILL.md",
        ),
        (
            "skills",
            process("/process", &[])
                .with_directory("skills", &[("calendar", Directory)])
                .with_directory("skills/calendar", &[("SKILL.md", File)])
                .with_file("skills/calendar/SKILL.md", SKILL),
            "skills/calendar/SKILL.md",
        ),
    ];
    for (dir, mut adapter, file) in layouts {
        let loaded = scan(&adapter, ".", dir);
        assert_eq!(loaded.skills[0].file_path, file);
        assert_eq!(
            *adapter.observed.borrow(),
            ["current_directory", "drive_directories"]
        );
        adapter.current = None;
        let quiet = scan(&adapter, ".", dir);
        assert!(
            quiet.skills.is_empty() && quiet.diagnostics.is_empty(),
            "{dir}"
        );
    }
}

/// An operand that anchors the scan root keeps the process directory unread.
#[cfg(not(windows))]
#[test]
fn directory_loading_does_not_read_the_process_directory_when_an_operand_anchors_the_scan() {
    use ResourceFileType::{Directory, File};
    for (cwd, dir) in [("/work", "skills"), ("work", "/work/skills")] {
        let mut adapter = process("/process", &[])
            .with_directory(dir, &[("calendar", Directory)])
            .with_directory(&format!("{dir}/calendar"), &[("SKILL.md", File)])
            .with_file(&format!("{dir}/calendar/SKILL.md"), SKILL);
        adapter.current = None;
        let result = scan(&adapter, cwd, dir);
        assert_eq!(result.skills.len(), 1, "{cwd} {dir}");
        assert!(adapter.observed.borrow().is_empty(), "{cwd} {dir}");
    }
}

/// Entries a scan skips before locating them never need the process directory.
#[cfg(not(windows))]
#[test]
fn directory_loading_does_not_locate_hidden_vendored_or_broken_entries() {
    use ResourceFileType::{Directory, Symlink};
    let mut adapter = process("/process", &[]).with_directory(
        "skills",
        &[
            (".hidden", Directory),
            ("node_modules", Directory),
            ("broken", Symlink),
        ],
    );
    adapter.current = None;
    let result = scan(&adapter, "work", "skills");
    assert!(result.skills.is_empty() && result.diagnostics.is_empty());
    assert!(adapter.observed.borrow().is_empty());
}

/// A relative scan root is not the matcher's strip prefix for a nested directory of the same name.
#[cfg(not(windows))]
#[test]
fn ignore_rules_match_below_a_relative_root_named_like_its_subdirectory() {
    use ResourceFileType::{Directory, File};
    let ignoring = |rule: &str| {
        process("/process", &[])
            .with_directory("skills", &[(".gitignore", File), ("skills", Directory)])
            .with_directory("skills/skills", &[("calendar", Directory)])
            .with_directory("skills/skills/calendar", &[("SKILL.md", File)])
            .with_file("skills/.gitignore", rule)
            .with_file("skills/skills/calendar/SKILL.md", SKILL)
    };
    let kept = scan(&ignoring("other\n"), "/work", "skills");
    assert_eq!(kept.skills.len(), 1);
    let ignored = scan(&ignoring("skills/calendar\n"), "/work", "skills");
    assert!(ignored.skills.is_empty() && ignored.diagnostics.is_empty());
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
    let _ = load(&adapter, ("work", "/agent"), &[], true).unwrap();
    assert_eq!(
        *adapter.asked.borrow(),
        [
            PathBuf::from("/agent/skills"),
            PathBuf::from("/process/work/.maestro/skills")
        ]
    );
}

/// Open project and explicit paths ask the adapter for its process directories once between them.
#[cfg(not(windows))]
#[test]
fn loading_asks_the_adapter_once_for_all_open_paths() {
    let adapter = process("/process", &[]);
    let result = load(&adapter, ("work", "/agent"), &["a.md", "b.md"], true).unwrap();
    let paths: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| d.path.as_deref())
        .collect();
    assert_eq!(
        paths,
        [Some("/process/work/a.md"), Some("/process/work/b.md")]
    );
    assert_eq!(
        *adapter.observed.borrow(),
        ["current_directory", "drive_directories"]
    );
}

/// A relative user root classifies explicit paths beneath the process directory, not the cwd.
#[cfg(not(windows))]
#[test]
fn relative_user_root_continues_from_the_process_directory_not_the_cwd() {
    let adapter = process("/process", &[]).with_file(
        "/process/agent/skills/calendar.md",
        "---\ndescription: Calendar\n---",
    );
    let result = load(
        &adapter,
        ("/work", "agent"),
        &["/process/agent/skills/calendar.md"],
        false,
    )
    .unwrap();
    assert_eq!(result.skills[0].source_info.scope, crate::SourceScope::User);
}

/// The caller directory wins on its own drive; another drive continues from its own entry.
#[test]
fn resolution_context_continues_other_drives_from_their_reported_directories() {
    let adapter = process(
        r"C:\Users\me",
        &[('C', r"C:\Users\me"), ('D', r"D:\skills")],
    );
    ProcessContext::new(&adapter).observe(|current, drive_directories| {
        let cwd = Cwd {
            current: current.unwrap(),
            drive_directories,
        };
        assert_eq!(cwd.current, r"C:\Users\me");
        assert_eq!(
            win32::resolve(&[r"C:\work", r"C:notes.md"], &cwd),
            r"C:\work\notes.md"
        );
        assert_eq!(
            win32::resolve(&[r"C:\work", r"D:calendar\SKILL.md"], &cwd),
            r"D:\skills\calendar\SKILL.md"
        );
    });
}

/// A drive without a reported directory continues from that drive's root.
#[test]
fn resolution_context_falls_back_to_the_drive_root_without_an_entry() {
    let adapter = process(r"C:\Users\me", &[('C', r"C:\Users\me")]);
    ProcessContext::new(&adapter).observe(|current, drive_directories| {
        let cwd = Cwd {
            current: current.unwrap(),
            drive_directories,
        };
        assert_eq!(
            win32::resolve(&[r"C:\work", r"D:calendar\SKILL.md"], &cwd),
            r"D:\calendar\SKILL.md"
        );
    });
}

/// The adapter is asked nothing while paths are anchored, and once for all later open paths, a failure included.
#[cfg(not(windows))]
#[test]
fn resolution_context_asks_the_adapter_once_and_only_for_an_open_path() {
    let readable = process("/process", &[]);
    let context = ProcessContext::new(&readable);
    assert_eq!(context.resolve(&["/work", "a"]).unwrap(), "/work/a");
    assert!(readable.observed.borrow().is_empty());
    assert_eq!(context.resolve(&["a"]).unwrap(), "/process/a");
    assert_eq!(context.resolve(&["b"]).unwrap(), "/process/b");
    assert_eq!(
        *readable.observed.borrow(),
        ["current_directory", "drive_directories"]
    );

    let mut unreadable = process("/process", &[]);
    unreadable.current = None;
    let context = ProcessContext::new(&unreadable);
    for path in ["a", "b"] {
        let error = context.resolve(&[path]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
    assert_eq!(
        *unreadable.observed.borrow(),
        ["current_directory", "drive_directories"]
    );
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
    let result = load(&adapter, (r"C:\work", r"C:\agent"), &["C:notes.md"], false).unwrap();
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
    )
    .unwrap();
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
    )
    .unwrap();
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
