//! Path resolution against the live process directories.

#[cfg(unix)]
use super::files::{metadata, symlink_metadata};
use super::real_path::real_path;
#[cfg(unix)]
use super::{LoadSkillsFromDirOptions, ResourceFileType, load_skills_from_dir};
use super::{LoadSkillsOptions, LoadSkillsResult, load_skills};
use crate::paths::resolve;
#[cfg(not(windows))]
use crate::{DiagnosticType, SourceOrigin, SourceScope};
use maestro_test_directory::Scratch;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// A skill file with a description.
#[cfg(not(windows))]
const SKILL: &str = "---\ndescription: Calendar\n---";

/// How the child running a case can read its working directory.
#[cfg(not(windows))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cwd {
    /// An ordinary directory.
    Readable,
    /// A directory deeper than the kernel's path limit below an ancestor without read
    /// permission: reading the working directory fails with `PermissionDenied`, while
    /// paths relative to it still work.
    #[cfg(unix)]
    Unreadable,
}

/// A scratch tree whose `process` directory is the working directory of the child.
#[cfg(not(windows))]
struct Fixture {
    /// Restores read permission before the tree is removed.
    #[cfg(unix)]
    _restore: Option<maestro_test_directory::Restore>,
    /// Owner of the tree.
    _scratch: Scratch,
    /// Root for anchored paths.
    root: PathBuf,
    /// The working directory, as reading it reports it.
    process: PathBuf,
}

#[cfg(not(windows))]
impl Fixture {
    /// Create the tree and enter its working directory; `None` when an unreadable one
    /// stays readable, as in a privileged run.
    fn new(cwd: Cwd) -> Option<Self> {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path().to_path_buf();
        #[cfg(unix)]
        if cwd == Cwd::Unreadable {
            let (process, restore) = scratch.enter_unreadable("process").unwrap()?;
            return Some(Self {
                _restore: Some(restore),
                _scratch: scratch,
                root,
                process,
            });
        }
        let process = root.join("process");
        fs::create_dir(&process).unwrap();
        std::env::set_current_dir(&process).unwrap();
        #[cfg(not(unix))]
        let _ = cwd;
        Some(Self {
            #[cfg(unix)]
            _restore: None,
            _scratch: scratch,
            root,
            process,
        })
    }

    /// The anchored spelling of `relative` below the root.
    fn path(&self, relative: &str) -> String {
        self.root.join(relative).to_str().unwrap().to_owned()
    }

    /// The spelling of `relative` below the working directory.
    fn below_process(&self, relative: &str) -> String {
        self.process.join(relative).to_str().unwrap().to_owned()
    }
}

/// Write `text` at `path`, anchored or relative to the working directory.
#[cfg(not(windows))]
fn write(path: impl AsRef<Path>, text: &str) {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

/// Run `body` for `case` of the test `name` in a child whose working directory is `cwd`.
///
/// The calling process starts the child and returns once it passed. A child where the
/// working directory stays readable although `Unreadable` was asked for (a privileged
/// run) skips the case.
#[cfg(not(windows))]
fn in_child(name: &str, case: &str, cwd: Cwd, body: impl FnOnce(&Fixture)) {
    let selection = maestro_test_process::selection();
    if !selection.is_child() {
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .case(case)
            .output()
            .unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
        return;
    }
    if !selection.runs_case(case) {
        return;
    }
    let Some(fixture) = Fixture::new(cwd) else {
        return;
    };
    body(&fixture);
}

/// Load `skill_paths` for caller directory `cwd`, with `agent_dir` as the user configuration.
fn load(
    (cwd, agent_dir): (&str, &str),
    skill_paths: &[&str],
    include_defaults: bool,
) -> io::Result<LoadSkillsResult> {
    load_skills(LoadSkillsOptions {
        cwd,
        home: "/home",
        agent_dir,
        config_dir_name: ".maestro",
        skill_paths: &skill_paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
        include_defaults,
    })
}

/// Scan `dir` for caller directory `cwd` through the public directory loader.
#[cfg(unix)]
fn scan(cwd: &str, dir: &str) -> LoadSkillsResult {
    load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd,
        dir,
        source: "path",
    })
}

/// A relative caller directory is an operand: it continues from the process directory.
#[cfg(not(windows))]
#[test]
fn explicit_path_beneath_a_relative_cwd_continues_from_the_process_directory() {
    let name = "explicit_path_beneath_a_relative_cwd_continues_from_the_process_directory";
    in_child(name, "readable", Cwd::Readable, |f| {
        let result = load(("work", &f.path("agent")), &["notes.md"], false).unwrap();
        let expected = f.below_process("work/notes.md");
        assert_eq!(
            result.diagnostics[0].path.as_deref(),
            Some(expected.as_str())
        );
    });
}

/// An unreadable process directory fails the loader for a relative caller directory and not for an anchored one.
#[cfg(unix)]
#[test]
fn explicit_paths_need_the_process_directory_only_beneath_a_relative_cwd() {
    let name = "explicit_paths_need_the_process_directory_only_beneath_a_relative_cwd";
    in_child(name, "unreadable", Cwd::Unreadable, |f| {
        let agent = f.path("agent");
        let anchored = f.path("abs/notes.md");
        for (paths, include_defaults) in [
            (&["notes.md"][..], false),
            (&[anchored.as_str()][..], false),
            (&[][..], true),
        ] {
            let error = load(("work", &agent), paths, include_defaults).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{paths:?}");
        }
        let work = f.path("work");
        let result = load((&work, &agent), &["notes.md"], false).unwrap();
        let expected = f.path("work/notes.md");
        assert_eq!(
            result.diagnostics[0].path.as_deref(),
            Some(expected.as_str())
        );
    });
}

/// A user root that needs the unreadable process directory turns the classification of an existing path into its warning.
#[cfg(unix)]
#[test]
fn scope_classification_needing_the_unreadable_process_directory_warns_for_that_path() {
    let name = "scope_classification_needing_the_unreadable_process_directory_warns_for_that_path";
    in_child(name, "unreadable", Cwd::Unreadable, |f| {
        write(f.path("work/notes.md"), "---\ndescription: Notes\n---");
        let result = load((&f.path("work"), "agent"), &["notes.md", "gone.md"], false).unwrap();
        assert!(result.skills.is_empty());
        let found: Vec<_> = result
            .diagnostics
            .iter()
            .map(|d| (d.path.clone(), d.message.clone()))
            .collect();
        let denied = std::env::current_dir().unwrap_err().to_string();
        assert_eq!(
            found,
            [
                (Some(f.path("work/notes.md")), denied),
                (
                    Some(f.path("work/gone.md")),
                    "skill path does not exist".to_owned()
                )
            ]
        );
    });
}

/// A scan root that is its own operand never needs the process directory.
#[cfg(unix)]
#[test]
fn directory_loading_does_not_read_the_process_directory_for_the_scan_root_alone() {
    let name = "directory_loading_does_not_read_the_process_directory_for_the_scan_root_alone";
    in_child(name, "unreadable", Cwd::Unreadable, |_| {
        let result = scan(".", ".");
        assert!(result.skills.is_empty() && result.diagnostics.is_empty());
    });
}

/// An entry that needs an unreadable process directory ends its directory quietly; a readable one loads.
#[cfg(unix)]
#[test]
fn directory_loading_ends_a_directory_quietly_when_an_entry_needs_the_unreadable_process_directory()
{
    let name = "directory_loading_ends_a_directory_quietly_when_an_entry_needs_the_unreadable_process_directory";
    let layouts = || {
        write("SKILL.md", SKILL);
        write("skills/calendar/SKILL.md", SKILL);
        [(".", "SKILL.md"), ("skills", "skills/calendar/SKILL.md")]
    };
    in_child(name, "readable", Cwd::Readable, |_| {
        for (dir, file) in layouts() {
            assert_eq!(scan(".", dir).skills[0].file_path, file, "{dir}");
        }
    });
    in_child(name, "unreadable", Cwd::Unreadable, |_| {
        for (dir, _) in layouts() {
            let quiet = scan(".", dir);
            assert!(
                quiet.skills.is_empty() && quiet.diagnostics.is_empty(),
                "{dir}"
            );
        }
    });
}

/// An operand that anchors the scan root keeps the process directory unread.
#[cfg(unix)]
#[test]
fn directory_loading_does_not_read_the_process_directory_when_an_operand_anchors_the_scan() {
    let name =
        "directory_loading_does_not_read_the_process_directory_when_an_operand_anchors_the_scan";
    in_child(name, "unreadable", Cwd::Unreadable, |f| {
        write("skills/calendar/SKILL.md", SKILL);
        write(f.path("work/skills/calendar/SKILL.md"), SKILL);
        let (work, skills) = (f.path("work"), f.path("work/skills"));
        for (cwd, dir) in [(work.as_str(), "skills"), ("work", skills.as_str())] {
            assert_eq!(scan(cwd, dir).skills.len(), 1, "{cwd} {dir}");
        }
    });
}

/// Entries a scan skips before locating them never need the process directory.
#[cfg(unix)]
#[test]
fn directory_loading_does_not_locate_hidden_vendored_or_broken_entries() {
    let name = "directory_loading_does_not_locate_hidden_vendored_or_broken_entries";
    in_child(name, "unreadable", Cwd::Unreadable, |_| {
        write("skills/.hidden/SKILL.md", SKILL);
        write("skills/node_modules/SKILL.md", SKILL);
        std::os::unix::fs::symlink("missing", "skills/broken").unwrap();
        let result = scan("work", "skills");
        assert!(result.skills.is_empty() && result.diagnostics.is_empty());
    });
}

/// A relative scan root is not the matcher's strip prefix for a nested directory of the same name.
#[cfg(unix)]
#[test]
fn ignore_rules_match_below_a_relative_root_named_like_its_subdirectory() {
    let name = "ignore_rules_match_below_a_relative_root_named_like_its_subdirectory";
    in_child(name, "readable", Cwd::Readable, |f| {
        write("skills/skills/calendar/SKILL.md", SKILL);
        let work = f.path("work");
        write("skills/.gitignore", "other\n");
        assert_eq!(scan(&work, "skills").skills.len(), 1);
        write("skills/.gitignore", "skills/calendar\n");
        let ignored = scan(&work, "skills");
        assert!(ignored.skills.is_empty() && ignored.diagnostics.is_empty());
    });
}

/// A relative path fails with the working-directory error kind; an absolute path does not.
#[cfg(unix)]
#[test]
fn real_path_needs_the_working_directory_only_for_a_relative_path() {
    let name = "real_path_needs_the_working_directory_only_for_a_relative_path";
    in_child(name, "unreadable", Cwd::Unreadable, |_| {
        let error = real_path(Path::new("é.md")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(real_path(Path::new("/")).unwrap(), Path::new("/"));
    });
}

/// The project defaults directory is the caller directory joined with the configuration name.
#[cfg(not(windows))]
#[test]
fn project_defaults_beneath_a_relative_cwd_continue_from_the_process_directory() {
    let name = "project_defaults_beneath_a_relative_cwd_continue_from_the_process_directory";
    in_child(name, "readable", Cwd::Readable, |f| {
        write("work/.maestro/skills/calendar/SKILL.md", SKILL);
        let result = load(("work", &f.path("agent")), &[], true).unwrap();
        let expected = f.below_process("work/.maestro/skills/calendar/SKILL.md");
        assert_eq!(result.skills[0].file_path, expected);
    });
}

/// Every open project and explicit path continues from the process directory.
#[cfg(not(windows))]
#[test]
fn loading_resolves_each_open_path_from_the_process_directory() {
    let name = "loading_resolves_each_open_path_from_the_process_directory";
    in_child(name, "readable", Cwd::Readable, |f| {
        let result = load(("work", &f.path("agent")), &["a.md", "b.md"], true).unwrap();
        let paths: Vec<_> = result.diagnostics.iter().map(|d| d.path.clone()).collect();
        assert_eq!(
            paths,
            [
                Some(f.below_process("work/a.md")),
                Some(f.below_process("work/b.md"))
            ]
        );
    });
}

/// Scanning beneath an anchored caller directory needs no process directory; a relative skill file whose real path cannot be resolved keeps its spelling.
#[cfg(unix)]
#[test]
fn skill_real_paths_keep_relative_spelling_when_the_process_directory_is_unreadable() {
    let name = "skill_real_paths_keep_relative_spelling_when_the_process_directory_is_unreadable";
    for (case, cwd) in [("readable", Cwd::Readable), ("unreadable", Cwd::Unreadable)] {
        in_child(name, case, cwd, |f| {
            for skill in ["a", "b"] {
                write(format!("agent/skills/{skill}/SKILL.md"), SKILL);
            }
            let loaded = load((&f.path("work"), "agent"), &[], true).unwrap();
            assert_eq!(loaded.skills.len(), 2, "{case}");
        });
    }
}

/// A relative user root classifies explicit paths beneath the process directory, not the cwd.
#[cfg(not(windows))]
#[test]
fn relative_user_root_continues_from_the_process_directory_not_the_cwd() {
    let name = "relative_user_root_continues_from_the_process_directory_not_the_cwd";
    in_child(name, "readable", Cwd::Readable, |f| {
        write("agent/skills/calendar.md", SKILL);
        let file = f.below_process("agent/skills/calendar.md");
        let result = load((&f.path("work"), "agent"), &[file.as_str()], false).unwrap();
        assert_eq!(result.skills[0].source_info.scope, crate::SourceScope::User);
    });
}

/// Two files sharing a skill name collide with their resolved spellings, while a repeated path
/// canonicalizes to one skill.
#[cfg(unix)]
#[test]
fn maestro_skills_collide_on_a_shared_name() {
    let name = "maestro_skills_collide_on_a_shared_name";
    in_child(name, "readable", Cwd::Readable, |f| {
        let work = f.below_process("work");
        for calendar in ["calendar", "other/calendar"] {
            let file = format!("{work}/home/{calendar}/SKILL.md");
            write(&file, SKILL);
        }
        let paths = [
            "~/calendar/SKILL.md",
            "~/calendar/SKILL.md",
            "~/other/calendar/SKILL.md",
        ];
        let result = load_skills(LoadSkillsOptions {
            cwd: &work,
            home: "home",
            agent_dir: &f.below_process("agent"),
            config_dir_name: ".maestro",
            skill_paths: &paths.map(str::to_owned),
            include_defaults: false,
        })
        .unwrap();
        assert_eq!(result.skills.len(), 1);
        assert_eq!(result.diagnostics.len(), 1);
        let collision = result.diagnostics[0].collision.as_ref().unwrap();
        assert_eq!(
            collision.winner_path,
            f.below_process("work/home/calendar/SKILL.md")
        );
        assert_eq!(
            collision.loser_path,
            f.below_process("work/home/other/calendar/SKILL.md")
        );
        assert_eq!(result.diagnostics[0].message, "name \"calendar\" collision");
    });
}

/// A path a drive directory completes never needs the working directory.
#[cfg(windows)]
#[test]
fn cwd_failure_does_not_block_supplied_drive() {
    let name = "cwd_failure_does_not_block_supplied_drive";
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .case("*")
            .env("=D:", r"D:\skills")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    assert_eq!(resolve(&[r"D:leaf"]).unwrap(), r"D:\skills\leaf");
    assert_eq!(resolve(&[r"C:\a", "b"]).unwrap(), r"C:\a\b");
}

/// Each resolution reads the working directory anew, so a change between two resolutions shows.
#[cfg(not(windows))]
#[test]
fn resolution_reads_the_working_directory_at_each_open_path() {
    let name = "resolution_reads_the_working_directory_at_each_open_path";
    in_child(name, "readable", Cwd::Readable, |f| {
        assert_eq!(resolve(&[&f.path("work"), "a"]).unwrap(), f.path("work/a"));
        assert_eq!(resolve(&["a"]).unwrap(), f.below_process("a"));
        let other = f.root.join("other");
        fs::create_dir(&other).unwrap();
        std::env::set_current_dir(&other).unwrap();
        assert_eq!(resolve(&["a"]).unwrap(), f.path("other/a"));
    });
    #[cfg(unix)]
    in_child(name, "unreadable", Cwd::Unreadable, |_| {
        for path in ["a", "b"] {
            let error = resolve(&[path]).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
    });
}

/// Run `body` in a child where drive `D:` has the current directory `directory`.
#[cfg(windows)]
fn with_drive_directory(name: &str, directory: &str, body: impl FnOnce()) {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .case("*")
            .env("=D:", directory)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    body();
}

/// An explicit drive-relative skill path on the cwd's drive continues from the cwd.
#[cfg(windows)]
#[test]
fn explicit_skill_path_on_the_cwd_drive_continues_from_the_cwd() {
    let result = load((r"C:\work", r"C:\agent"), &["C:notes.md"], false).unwrap();
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(r"C:\work\notes.md")
    );
}

/// An explicit drive-relative skill path without a drive entry continues from the drive root.
#[cfg(windows)]
#[test]
fn explicit_skill_path_without_a_drive_entry_continues_from_the_drive_root() {
    let name = "explicit_skill_path_without_a_drive_entry_continues_from_the_drive_root";
    if !maestro_test_process::selection().is_child() {
        // The working directory must not be on D:, or the path continues from it instead.
        let system = std::env::var("SystemDrive").unwrap();
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .case("*")
            .env_remove("=D:")
            .current_dir(format!(r"{system}\"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let result = load((r"C:\work", r"C:\agent"), &[r"D:calendar\SKILL.md"], false).unwrap();
    assert_eq!(
        result.diagnostics[0].path.as_deref(),
        Some(r"D:\calendar\SKILL.md")
    );
}

/// An explicit drive-relative skill path on another drive continues from that drive's directory.
#[cfg(windows)]
#[test]
fn explicit_skill_path_on_another_drive_continues_from_the_drive_directory() {
    let name = "explicit_skill_path_on_another_drive_continues_from_the_drive_directory";
    with_drive_directory(name, r"D:\skills", || {
        let result = load((r"C:\work", r"C:\agent"), &[r"D:calendar\SKILL.md"], false).unwrap();
        assert_eq!(
            result.diagnostics[0].path.as_deref(),
            Some(r"D:\skills\calendar\SKILL.md")
        );
    });
}

/// The drive letter of `path` and its spelling without a verbatim prefix (`\\?\C:\x`
/// gives `C` and `C:\x`).
#[cfg(windows)]
fn drive_and_plain_spelling(path: &Path) -> (char, PathBuf) {
    use std::path::{Component, Prefix};
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        panic!("{} has no drive", path.display());
    };
    let (Prefix::Disk(letter) | Prefix::VerbatimDisk(letter)) = prefix.kind() else {
        panic!("{} has no drive letter", path.display());
    };
    let text = path.to_str().unwrap();
    let plain = text.strip_prefix(r"\\?\").unwrap_or(text);
    (
        char::from(letter).to_ascii_uppercase(),
        PathBuf::from(plain),
    )
}

/// A real path continues a drive-relative path from the reported drive directory.
#[cfg(windows)]
#[test]
fn real_path_continues_from_the_drive_directory() {
    let name = "real_path_continues_from_the_drive_directory";
    let scratch = Scratch::new().unwrap();
    let directory = scratch.path().to_path_buf();
    if !maestro_test_process::selection().is_child() {
        let (letter, plain) = drive_and_plain_spelling(&directory);
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .case("*")
            .env(format!("={letter}:"), &plain)
            .env("MAESTRO_DRIVE_TEST_DIRECTORY", &plain)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let directory = PathBuf::from(std::env::var("MAESTRO_DRIVE_TEST_DIRECTORY").unwrap());
    fs::write(directory.join("item.md"), "item").unwrap();
    let (letter, _) = drive_and_plain_spelling(&directory);
    let resolved = real_path(Path::new(&format!("{letter}:item.md"))).unwrap();
    assert_eq!(resolved, directory.join("item.md"));
}

/// Native link observations do not substitute the target's kind.
#[cfg(unix)]
#[test]
fn native_nofollow_metadata_distinguishes_links() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("file"))
        .unwrap();
    fs::create_dir(root.join("dir")).unwrap();
    for (name, target) in [
        ("file-link", "file"),
        ("dir-link", "dir"),
        ("dangling", "missing"),
    ] {
        symlink(target, root.join(name)).unwrap();
    }
    for (name, expected, following) in [
        ("file", ResourceFileType::File, Some(ResourceFileType::File)),
        (
            "dir",
            ResourceFileType::Directory,
            Some(ResourceFileType::Directory),
        ),
        (
            "file-link",
            ResourceFileType::Symlink,
            Some(ResourceFileType::File),
        ),
        (
            "dir-link",
            ResourceFileType::Symlink,
            Some(ResourceFileType::Directory),
        ),
        ("dangling", ResourceFileType::Symlink, None),
    ] {
        assert_eq!(
            symlink_metadata(&root.join(name)).unwrap(),
            expected,
            "{name}: no-follow"
        );
        assert_eq!(
            metadata(&root.join(name)).ok(),
            following,
            "{name}: following"
        );
    }
    assert_eq!(
        symlink_metadata(&root.join("missing")).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
}

/// Each explicit skill form returns the reference's result for the five supplied homes, spelled
/// in the host's path flavor; a bare `~` with an empty Unix home leaves the expansion empty,
/// which names the working directory, and a relative Unix home resolves below it.
#[test]
fn explicit_skill_paths_follow_the_reference_table() {
    let rows: &[(&str, &str, &str)] = if cfg!(windows) {
        &[
            ("~", r"C:\h", r"C:\h"),
            ("~/", r"C:\h", r"C:\h"),
            ("~/x", r"C:\h", r"C:\h\x"),
            ("~x", r"C:\h", r"C:\w\~x"),
            ("~", r"C:\h\.", r"C:\h\."),
        ]
    } else {
        &[
            ("~", "/h", "/h"),
            ("~/", "/h", "/h/"),
            ("~/x", "/h", "/h/x"),
            ("~x", "/h", "/w/~x"),
            ("~", "/h/.", "/h/."),
            ("~", "", "/w"),
            ("~", "home", "/w/home"),
        ]
    };
    for &(written, home, expected) in rows {
        assert_eq!(
            super::normalize_skill_path(written, &options(home)).unwrap(),
            expected,
            "{written:?} with home {home:?}"
        );
    }
}

/// Described skill files of the explicit-path tree below the fixture root, with their descriptions.
#[cfg(not(windows))]
const EXPLICIT_TREE: [(&str, &str); 8] = [
    ("work/SKILL.md", "Working directory"),
    ("work/calendar/SKILL.md", "Meetings"),
    ("work/home/SKILL.md", "Relative home"),
    ("work/home/calendar/SKILL.md", "Meetings"),
    ("home-absolute/SKILL.md", "Absolute home"),
    ("home-absolute/calendar/SKILL.md", "Meetings"),
    ("work/.maestro/skills/SKILL.md", "Project root"),
    ("work/upper.MD", "Upper case"),
];

/// What one explicit path case selects.
#[cfg(not(windows))]
#[derive(Clone, Copy)]
enum ExplicitOutcome {
    /// The described skill file at this tree-relative path.
    Skill(&'static str),
    /// A missing-path warning at this tree-relative spelling.
    Missing(&'static str),
    /// A not-markdown warning at this tree-relative path.
    NotMarkdown(&'static str),
}

/// An expanded tilde resolves against the caller directory, so an empty home names the working
/// directory for a bare `~` while a suffixed form keeps its text, and a relative home resolves
/// below it, while every other explicit form and diagnostic stays the reference's.
#[cfg(not(windows))]
#[test]
fn explicit_tilde_paths_use_the_supplied_cwd_after_expansion() {
    let name = "explicit_tilde_paths_use_the_supplied_cwd_after_expansion";
    in_child(name, "readable", Cwd::Readable, |f| {
        write_explicit_tree(f);
        let work = f.below_process("work");
        for (home, input, expected) in explicit_cases(f) {
            let result = explicit_load(&work, &home, &input);
            assert_explicit_outcome(f, &result, expected);
        }
        let project = explicit_load(&work, "", "~/.maestro/skills");
        assert_explicit_skill(
            f,
            &project,
            "work/.maestro/skills/SKILL.md",
            SourceScope::Project,
        );
        let relative = explicit_load("work", "", "~");
        assert_explicit_skill(f, &relative, "work/SKILL.md", SourceScope::Temporary);
    });
}

/// The explicit path cases: the supplied home, the written path and the result it selects.
#[cfg(not(windows))]
fn explicit_cases(f: &Fixture) -> Vec<(String, String, ExplicitOutcome)> {
    let absolute = f.below_process("home-absolute");
    let dotted = format!("{absolute}/.");
    let expanded = format!("~/{absolute}/calendar");
    let rows = [
        ("", "~"),
        ("", "~/"),
        ("", "~/calendar"),
        ("", "~calendar"),
        ("", expanded.as_str()),
        ("", "\u{feff} ~/calendar \u{feff}"),
        ("", "~/calendar/../calendar"),
        ("", "~/missing"),
        ("", "~/upper.MD"),
        ("", "\u{85}~/calendar"),
        ("", "calendar"),
        ("", ""),
        ("home", "~"),
        ("home", "~/calendar"),
        ("home", "~calendar"),
        ("home/.", "~"),
        ("home/.", "~/calendar"),
        ("home/.", "~calendar"),
        (absolute.as_str(), "~"),
        (absolute.as_str(), "~/calendar"),
        (absolute.as_str(), "~calendar"),
        (dotted.as_str(), "~"),
        (dotted.as_str(), "~/calendar"),
        (dotted.as_str(), "~calendar"),
    ];
    let expected = explicit_outcomes();
    assert_eq!(expected.len(), rows.len());
    rows.into_iter()
        .zip(expected)
        .map(|((home, input), expected)| (home.to_owned(), input.to_owned(), expected))
        .collect()
}

/// The result each explicit path case selects, in the order [`explicit_cases`] writes the paths.
#[cfg(not(windows))]
fn explicit_outcomes() -> [ExplicitOutcome; 24] {
    use ExplicitOutcome::{Missing, NotMarkdown, Skill};
    let calendar = Skill("work/calendar/SKILL.md");
    let root = Skill("work/SKILL.md");
    let relative_home = Skill("work/home/SKILL.md");
    let relative_home_calendar = Skill("work/home/calendar/SKILL.md");
    let absolute_root = Skill("home-absolute/SKILL.md");
    let absolute_calendar = Skill("home-absolute/calendar/SKILL.md");
    [
        root,
        root,
        calendar,
        Missing("work/~calendar"),
        absolute_calendar,
        calendar,
        calendar,
        Missing("work/missing"),
        NotMarkdown("work/upper.MD"),
        Missing("work/\u{85}~/calendar"),
        calendar,
        root,
        relative_home,
        relative_home_calendar,
        Missing("work/~calendar"),
        relative_home,
        relative_home_calendar,
        Missing("work/~calendar"),
        absolute_root,
        absolute_calendar,
        Missing("work/~calendar"),
        absolute_root,
        absolute_calendar,
        Missing("work/~calendar"),
    ]
}

/// Write the described skill files of the explicit-path tree below the fixture root.
#[cfg(not(windows))]
fn write_explicit_tree(f: &Fixture) {
    for (relative, description) in EXPLICIT_TREE {
        let text = format!("---\ndescription: {description}\n---");
        write(f.below_process(relative), &text);
    }
}

/// Load one explicit path for caller directory `cwd` and the supplied home spelling.
#[cfg(not(windows))]
fn explicit_load(cwd: &str, home: &str, input: &str) -> LoadSkillsResult {
    let paths = [input.to_owned()];
    load_skills(LoadSkillsOptions {
        cwd,
        home,
        agent_dir: "/agent",
        config_dir_name: ".maestro",
        skill_paths: &paths,
        include_defaults: false,
    })
    .unwrap()
}

/// Assert the outcome one case selects.
#[cfg(not(windows))]
fn assert_explicit_outcome(f: &Fixture, result: &LoadSkillsResult, expected: ExplicitOutcome) {
    match expected {
        ExplicitOutcome::Skill(relative) => {
            assert_explicit_skill(f, result, relative, SourceScope::Temporary);
        }
        ExplicitOutcome::Missing(relative) => {
            assert_explicit_warning(f, result, relative, "skill path does not exist");
        }
        ExplicitOutcome::NotMarkdown(relative) => {
            assert_explicit_warning(f, result, relative, "skill path is not a markdown file");
        }
    }
}

/// Assert the single loaded skill at `relative`, its complete provenance and no diagnostics.
#[cfg(not(windows))]
fn assert_explicit_skill(
    f: &Fixture,
    result: &LoadSkillsResult,
    relative: &str,
    scope: SourceScope,
) {
    assert!(
        result.diagnostics.is_empty(),
        "{relative}: {:?}",
        result.diagnostics
    );
    let [skill] = result.skills.as_slice() else {
        panic!(
            "{relative}: expected one skill, found {}",
            result.skills.len()
        );
    };
    let file = f.below_process(relative);
    let (parent, _) = relative.rsplit_once('/').unwrap();
    let base = f.below_process(parent);
    let description = EXPLICIT_TREE
        .iter()
        .find_map(|(path, description)| (*path == relative).then_some(description))
        .unwrap();
    assert_eq!(skill.file_path, file);
    assert_eq!(skill.base_dir, base);
    assert_eq!(skill.name, parent.rsplit('/').next().unwrap());
    assert_eq!(skill.description, *description);
    assert!(!skill.disable_model_invocation);
    assert_eq!(skill.source_info.path, file);
    assert_eq!(skill.source_info.source, "local");
    assert_eq!(skill.source_info.scope, scope);
    assert_eq!(skill.source_info.origin, SourceOrigin::TopLevel);
    assert_eq!(skill.source_info.base_dir.as_deref(), Some(base.as_str()));
}

/// Assert the single warning at `relative` with `message` and no loaded skill.
#[cfg(not(windows))]
fn assert_explicit_warning(f: &Fixture, result: &LoadSkillsResult, relative: &str, message: &str) {
    assert!(result.skills.is_empty(), "{relative}: {:?}", result.skills);
    let [diagnostic] = result.diagnostics.as_slice() else {
        panic!(
            "{relative}: expected one warning, found {}",
            result.diagnostics.len()
        );
    };
    assert_eq!(diagnostic.r#type, DiagnosticType::Warning, "{relative}");
    let expected = f.below_process(relative);
    assert_eq!(diagnostic.path.as_deref(), Some(expected.as_str()));
    assert_eq!(diagnostic.message, message);
    assert!(diagnostic.collision.is_none());
}

/// Options with the supplied home spelling and the working directory `/w`.
fn options(home: &str) -> LoadSkillsOptions<'_> {
    LoadSkillsOptions {
        cwd: "/w",
        home,
        agent_dir: "/a",
        config_dir_name: ".maestro",
        skill_paths: &[],
        include_defaults: false,
    }
}
