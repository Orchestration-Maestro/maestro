//! Conditions of `includeIf` sections and the paths of include entries.
use super::{
    super::discovery::GitWorktreeInfo,
    wildmatch::{Rules, wildmatch},
};
use maestro_path::{posix, slashed, win32};
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
            self.git_dir_matches(
                pattern,
                config_path,
                Rules {
                    ignore_case: true,
                    ..PATHNAME
                },
                WINDOWS,
            )
        } else if let Some(pattern) = condition.strip_prefix("gitdir:") {
            self.git_dir_matches(pattern, config_path, PATHNAME, WINDOWS)
        } else if let Some(pattern) = condition.strip_prefix("onbranch:") {
            wildmatch(&directory_prefix(pattern), self.branch, PATHNAME)
        } else if let Some(pattern) = condition.strip_prefix(HAS_CONFIG) {
            urls.is_some_and(|urls| urls.iter().any(|url| wildmatch(pattern, url, ANYWHERE)))
        } else {
            false
        }
    }

    /// Whether the checkout's metadata directory matches a `gitdir` pattern.
    ///
    /// `windows` is the host's spelling: its patterns and checkout paths reach one
    /// comparison in slash-separated form, which is the only separator the rules know.
    fn git_dir_matches(
        &self,
        pattern: &str,
        config_path: &Path,
        rules: Rules,
        windows: bool,
    ) -> bool {
        let Some(pattern) = self.gitdir_pattern(pattern, config_path, windows) else {
            return false;
        };
        let repo_git = self.info.repo_root.join(".git");
        [&self.info.git_dir, &self.info.git_common_dir, &repo_git]
            .into_iter()
            .any(|candidate| {
                let text = candidate.display().to_string();
                wildmatch(&pattern, &slashed(&text, windows), rules)
            })
    }

    /// Anchor a `gitdir` pattern: `~/` below the home, `./` beside the file, else anywhere.
    ///
    /// With `windows`, every separator of the result is `/`, the only one the rules know.
    fn gitdir_pattern(&self, pattern: &str, config_path: &Path, windows: bool) -> Option<String> {
        let pattern = directory_prefix(&slashed(pattern, windows));
        let anchored = if pattern.starts_with("~/") {
            let home = super::expand_home(&pattern, self.home)?
                .display()
                .to_string();
            slashed(&home, windows).into_owned()
        } else if let Some(rest) = pattern.strip_prefix("./") {
            let beside = config_path.parent().unwrap_or(Path::new(".")).join(rest);
            slashed(&beside.display().to_string(), windows).into_owned()
        } else if is_absolute(&pattern, windows) {
            pattern
        } else {
            format!("**/{pattern}")
        };
        Some(anchored)
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

/// Whether the host spells paths with `\` as well as `/`.
const WINDOWS: bool = cfg!(windows);

/// Whether an authored path names a root, in a supplied path flavor.
fn is_absolute(pattern: &str, windows: bool) -> bool {
    if windows {
        win32::is_absolute(pattern)
    } else {
        posix::is_absolute(pattern)
    }
}

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

#[cfg(test)]
mod tests;
