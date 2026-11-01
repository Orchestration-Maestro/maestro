//! Branch tracking read from user, repository, included and worktree configuration.
//!
//! One walk applies files in Git's order: the user files, the repository file, then the
//! worktree file when the repository enables it. Includes apply where they appear; a file
//! already being read is not read again inside itself, which stops include cycles.
mod condition;
mod files;
mod refspec;
/// Line syntax shared with the metadata readers of this module's parent.
pub(super) mod syntax;
mod wildmatch;

use super::discovery::GitWorktreeInfo;
use condition::Subject;
use files::{Dependency, Reader};
use std::path::{Path, PathBuf};
use syntax::{Item, is_true, plain_entries};

/// Values the caller supplies because configuration depends on them besides the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigEnvironment {
    /// User-level files, lowest precedence first.
    pub user_files: Vec<PathBuf>,
    /// Home directory that a leading `~/` in an include path or `gitdir` pattern names.
    pub home: Option<PathBuf>,
}

impl ConfigEnvironment {
    /// Git's user-level files for the supplied `XDG_CONFIG_HOME` and home directory.
    ///
    /// The XDG file comes from nonempty `XDG_CONFIG_HOME`, else from `~/.config`;
    /// `~/.gitconfig` follows it and takes precedence.
    #[must_use]
    pub fn from_variables(xdg_config_home: Option<&Path>, home: Option<&Path>) -> Self {
        let xdg = xdg_config_home
            .filter(|path| !path.as_os_str().is_empty())
            .map(|directory| directory.join("git/config"))
            .or_else(|| expand_home("~/.config/git/config", home));
        Self {
            user_files: xdg
                .into_iter()
                .chain(expand_home("~/.gitconfig", home))
                .collect(),
            home: home.map(Path::to_path_buf),
        }
    }
}

/// Expand a leading tilde without an account fallback, preserving native home bytes.
fn expand_home(value: &str, home: Option<&Path>) -> Option<PathBuf> {
    maestro_path::expand_tilde_native(value, || home.map(Path::to_path_buf).ok_or(())).ok()
}

/// The remote and merge reference a branch tracks, with the fetch refspecs that map them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchConfig {
    /// Remote named by `branch.<name>.remote`.
    pub remote: String,
    /// Reference named by `branch.<name>.merge`.
    pub merge_ref: String,
    /// Every `remote.<name>.fetch` value met, paired with its remote.
    fetch_refspecs: Vec<(String, String)>,
}

impl BranchConfig {
    /// The local reference the remote's fetch refspecs map the merge reference to.
    #[must_use]
    pub fn upstream_full_ref(&self) -> Option<String> {
        refspec::tracking_ref(
            &self.remote,
            &self.merge_ref,
            self.fetch_refspecs
                .iter()
                .filter(|(remote, _)| *remote == self.remote)
                .map(|(_, refspec)| refspec.as_str()),
        )
    }
}

/// One configuration read and the files it depended on.
#[derive(Debug, Clone)]
pub struct ConfigRead {
    /// Tracking for the branch, when both remote and merge reference are configured.
    pub branch: Option<BranchConfig>,
    /// Every file the walk touched, with what it showed.
    dependencies: Vec<Dependency>,
}

impl ConfigRead {
    /// Whether every file this read depended on is unchanged and was read without error.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.dependencies.iter().all(Dependency::is_current)
    }
}

/// Read the tracking configuration of `branch` for the checkout described by `info`.
#[must_use]
pub fn read_branch_config(
    info: &GitWorktreeInfo,
    branch: &str,
    environment: &ConfigEnvironment,
) -> ConfigRead {
    let mut walker = Walker {
        subject: Subject {
            info,
            branch,
            home: environment.home.as_deref(),
        },
        reader: Reader::default(),
        reading: Vec::new(),
        inspecting: false,
    };
    let repository = info.git_common_dir.join("config");
    let files: Vec<&Path> = environment
        .user_files
        .iter()
        .map(PathBuf::as_path)
        .chain([repository.as_path()])
        .collect();
    let worktree_enabled = walker.worktree_extension_enabled(&repository);
    let mut urls = Vec::new();
    for file in &files {
        walker.walk(file, None, &mut |variable| {
            urls.extend(variable.remote_url().map(str::to_owned));
        });
    }
    let mut tracking = Tracking::new(branch);
    for file in &files {
        walker.walk(file, Some(&urls), &mut |variable| tracking.apply(variable));
    }
    if worktree_enabled {
        let worktree = info.git_dir.join("config.worktree");
        walker.walk(&worktree, Some(&urls), &mut |variable| {
            tracking.apply(variable);
        });
    }
    ConfigRead {
        branch: tracking.finish(),
        dependencies: walker.reader.dependencies,
    }
}

/// A variable met while walking files, with the section it belongs to.
struct Variable<'a> {
    /// Section name as written.
    section: &'a str,
    /// Subsection, when the section has one.
    subsection: Option<&'a str>,
    /// Variable name as written.
    key: &'a str,
    /// Value, absent for a bare key.
    value: Option<&'a str>,
}

impl Variable<'_> {
    /// Whether this is `section.key` under the subsection, ignoring case in section and key.
    fn is(&self, section: &str, key: &str) -> bool {
        self.section.eq_ignore_ascii_case(section) && self.key.eq_ignore_ascii_case(key)
    }

    /// The URL, when this is `remote.<name>.url`.
    fn remote_url(&self) -> Option<&str> {
        self.subsection
            .and(self.value)
            .filter(|_| self.is("remote", "url"))
    }
}

/// Branch, remote and refspec values gathered in file order, later values replacing earlier.
struct Tracking<'a> {
    /// The branch whose section is read.
    branch: &'a str,
    /// Latest `branch.<name>.remote`.
    remote: String,
    /// Latest `branch.<name>.merge`.
    merge_ref: String,
    /// Every `remote.<name>.fetch`.
    fetch_refspecs: Vec<(String, String)>,
}

impl<'a> Tracking<'a> {
    /// An empty gathering for `branch`.
    fn new(branch: &'a str) -> Self {
        Self {
            branch,
            remote: String::new(),
            merge_ref: String::new(),
            fetch_refspecs: Vec::new(),
        }
    }

    /// Take the variable when it concerns the branch or a fetch refspec.
    fn apply(&mut self, variable: &Variable<'_>) {
        let (Some(subsection), Some(value)) = (variable.subsection, variable.value) else {
            return;
        };
        if variable.section.eq_ignore_ascii_case("branch") && subsection == self.branch {
            if variable.key.eq_ignore_ascii_case("remote") {
                value.clone_into(&mut self.remote);
            } else if variable.key.eq_ignore_ascii_case("merge") {
                value.clone_into(&mut self.merge_ref);
            }
        } else if variable.is("remote", "fetch") {
            self.fetch_refspecs
                .push((subsection.to_owned(), value.to_owned()));
        }
    }

    /// The tracking, when both a remote and a merge reference were set.
    fn finish(self) -> Option<BranchConfig> {
        (!self.remote.is_empty() && !self.merge_ref.is_empty()).then_some(BranchConfig {
            remote: self.remote,
            merge_ref: self.merge_ref,
            fetch_refspecs: self.fetch_refspecs,
        })
    }
}

/// Reads files in order and follows their includes.
struct Walker<'a> {
    /// What conditions are judged against.
    subject: Subject<'a>,
    /// Files read so far.
    reader: Reader,
    /// Files being read, outermost first.
    reading: Vec<PathBuf>,
    /// Whether a remote-URL inspection is running; it follows every matching include.
    inspecting: bool,
}

impl Walker<'_> {
    /// Whether the repository file enables `extensions.worktreeConfig`; its last value counts
    /// and includes do not.
    fn worktree_extension_enabled(&mut self, repository: &Path) -> bool {
        let (_, items) = self.reader.read(repository);
        items.is_some_and(|items| {
            plain_entries(&items, "extensions")
                .filter(|(key, _)| key.eq_ignore_ascii_case("worktreeConfig"))
                .last()
                .is_some_and(|(_, value)| is_true(value))
        })
    }

    /// Visit the variables of `path` in order, entering includes where they appear.
    ///
    /// `urls` are the remote URLs that `hasconfig` conditions test; without them those
    /// conditions fail.
    fn walk(&mut self, path: &Path, urls: Option<&[String]>, visit: &mut dyn FnMut(&Variable<'_>)) {
        let (path, items) = self.reader.read(path);
        let Some(items) = items else { return };
        if self.reading.contains(&path) {
            return;
        }
        self.reading.push(path.clone());
        let mut section = ("", None);
        for item in items.iter() {
            match item {
                Item::Header { name, subsection } => {
                    section = (name.as_str(), subsection.as_deref());
                }
                Item::Entry { key, value } => {
                    let variable = Variable {
                        section: section.0,
                        subsection: section.1,
                        key,
                        value: value.as_deref(),
                    };
                    self.entry(&variable, &path, urls, visit);
                }
            }
        }
        self.reading.pop();
    }

    /// Enter the file an include entry names, or hand any other variable to `visit`.
    fn entry(
        &mut self,
        variable: &Variable<'_>,
        path: &Path,
        urls: Option<&[String]>,
        visit: &mut dyn FnMut(&Variable<'_>),
    ) {
        match self.include_target(variable, path, urls) {
            Some(Include::Enter(target)) => self.walk(&target, urls, visit),
            Some(Include::EnterUnlessItDefinesRemotes(target)) => {
                if !self.defines_remote_url(&target, urls) {
                    self.walk(&target, urls, visit);
                }
            }
            Some(Include::Skip) => {}
            None => visit(variable),
        }
    }

    /// What an include entry asks for, or `None` when the variable is not an include.
    fn include_target(
        &self,
        variable: &Variable<'_>,
        config_path: &Path,
        urls: Option<&[String]>,
    ) -> Option<Include> {
        if !variable.key.eq_ignore_ascii_case("path") {
            return None;
        }
        let guarded = match (variable.section, variable.subsection) {
            (section, None) if section.eq_ignore_ascii_case("include") => false,
            (section, Some(condition)) if section.eq_ignore_ascii_case("includeIf") => {
                if !self.subject.holds(condition, config_path, urls) {
                    return Some(Include::Skip);
                }
                condition.starts_with("hasconfig:") && !self.inspecting
            }
            _ => return None,
        };
        let target = variable
            .value
            .and_then(|value| self.subject.include_path(value, config_path));
        Some(match target {
            Some(target) if guarded => Include::EnterUnlessItDefinesRemotes(target),
            Some(target) => Include::Enter(target),
            None => Include::Skip,
        })
    }

    /// Whether `path`, with the files it includes, sets any `remote.<name>.url`.
    ///
    /// Matching `hasconfig` includes inside `path` are read as well, so a URL they set counts.
    fn defines_remote_url(&mut self, path: &Path, urls: Option<&[String]>) -> bool {
        let mut defines = false;
        self.inspecting = true;
        self.walk(path, urls, &mut |variable| {
            defines |= variable.remote_url().is_some();
        });
        self.inspecting = false;
        defines
    }
}

/// What an include entry does.
enum Include {
    /// Read the file here.
    Enter(PathBuf),
    /// Read the file here unless it, or a file it includes, sets a remote URL.
    EnterUnlessItDefinesRemotes(PathBuf),
    /// Read nothing.
    Skip,
}
