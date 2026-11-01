//! Conditions of `includeIf` sections and the paths of include entries.
use super::{
    super::discovery::GitWorktreeInfo,
    wildmatch::{Rules, wildmatch},
};
use std::path::{Path, PathBuf};

/// The checkout and values a condition is judged against.
pub(super) struct Subject<'a> {
    /// The checkout whose configuration is read.
    pub info: &'a GitWorktreeInfo,
    /// The branch `onbranch` conditions compare with.
    pub branch: &'a str,
    /// The home directory that a leading `~/` names.
    pub home: Option<&'a Path>,
}

/// Prefix of the condition that tests remote URLs.
const HAS_CONFIG: &str = "hasconfig:remote.*.url:";

impl Subject<'_> {
    /// Whether `condition`, written in the file at `config_path`, holds.
    ///
    /// `urls` are the remote URLs known so far; without them a `hasconfig` condition fails.
    pub(super) fn holds(
        &self,
        condition: &str,
        config_path: &Path,
        urls: Option<&[String]>,
    ) -> bool {
        if let Some(pattern) = condition.strip_prefix("gitdir/i:") {
            self.git_dir_matches(pattern, config_path, true)
        } else if let Some(pattern) = condition.strip_prefix("gitdir:") {
            self.git_dir_matches(pattern, config_path, false)
        } else if let Some(pattern) = condition.strip_prefix("onbranch:") {
            wildmatch(&directory_prefix(pattern), self.branch, PATHNAME)
        } else if let Some(pattern) = condition.strip_prefix(HAS_CONFIG) {
            urls.is_some_and(|urls| urls.iter().any(|url| wildmatch(pattern, url, ANYWHERE)))
        } else {
            false
        }
    }

    /// Whether the checkout's metadata directory matches a `gitdir` pattern.
    fn git_dir_matches(&self, pattern: &str, config_path: &Path, ignore_case: bool) -> bool {
        let Some(pattern) = self.gitdir_pattern(pattern, config_path) else {
            return false;
        };
        let rules = Rules {
            ignore_case,
            ..PATHNAME
        };
        [
            self.info.git_dir.clone(),
            self.info.git_common_dir.clone(),
            self.info.repo_root.join(".git"),
        ]
        .iter()
        .any(|candidate| wildmatch(&pattern, &candidate.display().to_string(), rules))
    }

    /// Anchor a `gitdir` pattern: `~/` below the home, `./` beside the file, else anywhere.
    fn gitdir_pattern(&self, pattern: &str, config_path: &Path) -> Option<String> {
        let anchored = if pattern.starts_with("~/") {
            super::expand_home(pattern, self.home)?
                .display()
                .to_string()
        } else if let Some(rest) = pattern.strip_prefix("./") {
            config_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(rest)
                .display()
                .to_string()
        } else if Path::new(pattern).is_absolute() {
            pattern.to_owned()
        } else {
            format!("**/{pattern}")
        };
        Some(directory_prefix(&anchored))
    }

    /// Where the include entry `value`, written in `config_path`, points.
    pub(super) fn include_path(&self, value: &str, config_path: &Path) -> Option<PathBuf> {
        let path = super::expand_home(value, self.home)?;
        Some(if path.is_absolute() {
            path
        } else {
            config_path.parent().unwrap_or(Path::new(".")).join(path)
        })
    }
}

/// Branch and directory names stop at `/`.
const PATHNAME: Rules = Rules {
    pathname: true,
    ignore_case: false,
};

/// URLs compare as plain text.
const ANYWHERE: Rules = Rules {
    pathname: false,
    ignore_case: false,
};

/// A pattern ending in `/` matches everything below that directory.
fn directory_prefix(pattern: &str) -> String {
    if pattern.ends_with('/') {
        format!("{pattern}**")
    } else {
        pattern.to_owned()
    }
}
