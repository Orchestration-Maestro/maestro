//! Branch tracking read from user, repository, included and worktree configuration.
#![cfg(test)]

use maestro_test_directory::Scratch;
use maestro_workspace::workspace::git::{
    config::{BranchConfig, ConfigEnvironment, read_branch_config},
    discovery::git_worktree_info,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const TRACKING: &str = "[branch \"main\"]\n\tremote = origin\n\tmerge = refs/heads/main\n";
const INCLUDED_TRACKING: &str = "[remote \"included\"]\n\tfetch = +refs/heads/*:refs/remotes/included/*\n[branch \"main\"]\n\tremote = included\n\tmerge = refs/heads/main\n";
const FORK_REFSPEC: &str = "[remote \"fork\"]\n\tfetch = +refs/heads/*:refs/remotes/fork/*\n";
const FORK_URL: &str = "[remote \"fork\"]\n\turl = https://example.test/fork.git\n";
const FORK_CONDITION: &str = "[includeIf \"hasconfig:remote.*.url:*fork.git\"]\n\tpath = ";

/// A checkout on `main` whose repository configuration the test chooses.
struct Checkout {
    scratch: Scratch,
}

impl Checkout {
    fn new(config: &str) -> Self {
        let scratch = Scratch::new().unwrap();
        let git = scratch.path().join("repo/.git");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(git.join("config"), config).unwrap();
        Self { scratch }
    }

    fn root(&self) -> PathBuf {
        self.scratch.path().join("repo")
    }

    fn git(&self) -> PathBuf {
        self.root().join(".git")
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.scratch.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    fn observe_with(&self, environment: &ConfigEnvironment) -> Option<BranchConfig> {
        let info = git_worktree_info(&self.root()).unwrap();
        read_branch_config(&info, "main", environment).branch
    }

    fn observe(&self) -> BranchConfig {
        self.observe_with(&ConfigEnvironment::default()).unwrap()
    }

    fn observe_with_user(&self, user: &[&Path]) -> Option<BranchConfig> {
        self.observe_with(&ConfigEnvironment {
            user_files: user.iter().map(|path| path.to_path_buf()).collect(),
            home: None,
        })
    }
}

fn upstream(config: &BranchConfig) -> (&str, Option<String>) {
    (&config.remote, config.upstream_full_ref())
}

#[test]
fn fetch_refspec_maps_the_upstream_reference() {
    let checkout = Checkout::new(&format!(
        "[remote \"origin\"]\n\tfetch = +refs/heads/*:refs/remotes/upstream/*\n{TRACKING}"
    ));
    let config = checkout.observe();
    assert_eq!(
        upstream(&config),
        ("origin", Some("refs/remotes/upstream/main".into()))
    );
}

#[test]
fn tracking_defaults_to_the_remote_namespace_without_a_refspec() {
    let config = Checkout::new(TRACKING).observe();
    assert_eq!(config.merge_ref, "refs/heads/main");
    assert_eq!(
        upstream(&config),
        ("origin", Some("refs/remotes/origin/main".into()))
    );
}

#[test]
fn same_repository_remote_tracks_its_merge_reference() {
    let config =
        Checkout::new("[branch \"main\"]\n\tremote = .\n\tmerge = refs/heads/trunk\n").observe();
    assert_eq!(upstream(&config), (".", Some("refs/heads/trunk".into())));
}

#[test]
fn included_file_supplies_remote_and_branch() {
    let checkout = Checkout::new("[include]\n\tpath = included.cfg\n");
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    let config = checkout.observe();
    assert_eq!(
        upstream(&config),
        ("included", Some("refs/remotes/included/main".into()))
    );
}

#[test]
fn user_configuration_applies_before_repository_configuration() {
    let checkout = Checkout::new("");
    let user = checkout.write(
        "user.gitconfig",
        "[remote \"global\"]\n\tfetch = +refs/heads/*:refs/remotes/global/*\n[branch \"main\"]\n\tremote = global\n\tmerge = refs/heads/main\n",
    );
    let config = checkout.observe_with_user(&[&user]).unwrap();
    assert_eq!(
        upstream(&config),
        ("global", Some("refs/remotes/global/main".into()))
    );
}

#[test]
fn repository_configuration_overrides_user_configuration() {
    let checkout = Checkout::new(TRACKING);
    let user = checkout.write(
        "user.gitconfig",
        "[remote \"global\"]\n\tfetch = +refs/heads/*:refs/remotes/global/*\n[branch \"main\"]\n\tremote = global\n\tmerge = refs/heads/main\n",
    );
    let config = checkout.observe_with_user(&[&user]).unwrap();
    assert_eq!(
        upstream(&config),
        ("origin", Some("refs/remotes/origin/main".into()))
    );
}

#[test]
fn repeated_includes_apply_where_they_appear() {
    let checkout = Checkout::new(
        "[include]\n\tpath = included.cfg\n[branch \"main\"]\n\tremote = middle\n[include]\n\tpath = included.cfg\n",
    );
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn include_cycles_stop() {
    let checkout = Checkout::new("[include]\n\tpath = a.cfg\n");
    checkout.write("repo/.git/a.cfg", "[include]\n\tpath = b.cfg\n");
    checkout.write(
        "repo/.git/b.cfg",
        &format!("[include]\n\tpath = a.cfg\n{INCLUDED_TRACKING}"),
    );
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn gitdir_condition_matches_the_git_directory() {
    let checkout = Checkout::new("");
    let condition = format!(
        "[includeIf \"gitdir:{}\"]\n\tpath = included.cfg\n",
        checkout.git().display()
    );
    checkout.write("repo/.git/config", &condition);
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn gitdir_condition_with_a_trailing_slash_matches_nested_checkouts() {
    let checkout = Checkout::new("");
    let parent = checkout.scratch.path();
    let condition = format!(
        "[includeIf \"gitdir:{}/\"]\n\tpath = included.cfg\n",
        parent.display()
    );
    checkout.write("repo/.git/config", &condition);
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn gitdir_condition_is_relative_to_the_including_file() {
    let checkout = Checkout::new("");
    let user = checkout.write(
        "user.gitconfig",
        "[includeIf \"gitdir:./repo/.git\"]\n\tpath = user-included.cfg\n",
    );
    checkout.write("user-included.cfg", INCLUDED_TRACKING);
    assert_eq!(
        checkout.observe_with_user(&[&user]).unwrap().remote,
        "included"
    );
}

#[test]
fn gitdir_condition_expands_the_supplied_home() {
    let checkout = Checkout::new("");
    checkout.write(
        "repo/.git/config",
        "[includeIf \"gitdir:~/repo/\"]\n\tpath = included.cfg\n",
    );
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    let with_home = ConfigEnvironment {
        user_files: Vec::new(),
        home: Some(checkout.scratch.path().to_path_buf()),
    };
    assert_eq!(
        checkout.observe_with(&with_home).unwrap().remote,
        "included"
    );
    assert!(
        checkout
            .observe_with(&ConfigEnvironment::default())
            .is_none()
    );
}

/// An include path written `~//name` stays below the home instead of naming the root.
#[test]
fn include_path_with_repeated_separators_stays_below_the_home() {
    let checkout = Checkout::new("");
    checkout.write("repo/.git/config", "[include]\n\tpath = ~//included.cfg\n");
    checkout.write("included.cfg", INCLUDED_TRACKING);
    let with_home = ConfigEnvironment {
        user_files: Vec::new(),
        home: Some(checkout.scratch.path().to_path_buf()),
    };
    assert_eq!(
        checkout.observe_with(&with_home).unwrap().remote,
        "included"
    );
}

#[test]
fn gitdir_condition_ignores_case_only_when_asked() {
    let checkout = Checkout::new("");
    let upper = checkout.git().display().to_string().to_uppercase();
    checkout.write(
        "repo/.git/config",
        &format!("[includeIf \"gitdir/i:{upper}\"]\n\tpath = insensitive.cfg\n[includeIf \"gitdir:{upper}\"]\n\tpath = sensitive.cfg\n"),
    );
    checkout.write("repo/.git/sensitive.cfg", TRACKING);
    checkout.write("repo/.git/insensitive.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn gitdir_star_stays_within_one_directory() {
    let checkout = Checkout::new("");
    let parent = checkout.scratch.path().display();
    checkout.write(
        "repo/.git/config",
        &format!("[includeIf \"gitdir:{parent}/*/.git\"]\n\tpath = shallow.cfg\n[includeIf \"gitdir:{parent}/*.git\"]\n\tpath = crossing.cfg\n"),
    );
    checkout.write("repo/.git/shallow.cfg", INCLUDED_TRACKING);
    checkout.write("repo/.git/crossing.cfg", TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn gitdir_double_star_crosses_directories() {
    let checkout = Checkout::new("");
    let parent = checkout.scratch.path().display();
    checkout.write(
        "repo/.git/config",
        &format!("[includeIf \"gitdir:{parent}/**/.git\"]\n\tpath = included.cfg\n"),
    );
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn wildcard_matching_does_not_backtrack_exponentially() {
    let checkout = Checkout::new("");
    let stars = "*a".repeat(24);
    checkout.write(
        "repo/.git/config",
        &format!("[includeIf \"onbranch:{stars}b\"]\n\tpath = included.cfg\n"),
    );
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    let info = git_worktree_info(&checkout.root()).unwrap();
    let branch = "a".repeat(60);
    let read = read_branch_config(&info, &branch, &ConfigEnvironment::default());
    assert!(read.branch.is_none());
}

#[test]
fn onbranch_condition_matches_the_current_branch() {
    let checkout = Checkout::new("[includeIf \"onbranch:main\"]\n\tpath = included.cfg\n");
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn onbranch_condition_that_does_not_match_is_skipped() {
    let checkout = Checkout::new(&format!(
        "{TRACKING}[includeIf \"onbranch:release/*\"]\n\tpath = included.cfg\n"
    ));
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "origin");
}

#[test]
fn section_and_key_names_ignore_case() {
    let config = Checkout::new(
        "[Remote \"origin\"] # remote section\n\tFetch = +refs/heads/*:refs/remotes/origin/*\n[Branch \"main\"] ; branch section\n\tRemote = origin\n\tMerge = refs/heads/main\n",
    )
    .observe();
    assert_eq!(
        upstream(&config),
        ("origin", Some("refs/remotes/origin/main".into()))
    );
}

#[test]
fn later_remote_override_keeps_earlier_refspecs() {
    let checkout = Checkout::new(&format!(
        "[extensions]\n\tworktreeConfig = true\n{FORK_REFSPEC}{TRACKING}"
    ));
    checkout.write(
        "repo/.git/config.worktree",
        "[branch \"main\"]\n\tremote = fork\n",
    );
    let config = checkout.observe();
    assert_eq!(
        upstream(&config),
        ("fork", Some("refs/remotes/fork/main".into()))
    );
}

#[test]
fn worktree_configuration_is_ignored_without_the_extension() {
    let checkout = Checkout::new(&format!("{FORK_REFSPEC}{TRACKING}"));
    checkout.write(
        "repo/.git/config.worktree",
        "[branch \"main\"]\n\tremote = fork\n",
    );
    assert_eq!(checkout.observe().remote, "origin");
}

#[test]
fn worktree_extension_accepts_git_booleans() {
    for enabled in ["true", "TRUE", "1", "yes", "on", ""] {
        let value = if enabled.is_empty() {
            String::new()
        } else {
            format!(" = {enabled}")
        };
        let checkout = Checkout::new(&format!(
            "[extensions]\n\tworktreeConfig{value}\n{FORK_REFSPEC}{TRACKING}"
        ));
        checkout.write(
            "repo/.git/config.worktree",
            "[branch \"main\"]\n\tremote = fork\n",
        );
        assert_eq!(checkout.observe().remote, "fork", "worktreeConfig{value}");
    }
    for disabled in ["false", "0", "no", "off"] {
        let checkout = Checkout::new(&format!(
            "[extensions]\n\tworktreeConfig = {disabled}\n{FORK_REFSPEC}{TRACKING}"
        ));
        checkout.write(
            "repo/.git/config.worktree",
            "[branch \"main\"]\n\tremote = fork\n",
        );
        assert_eq!(
            checkout.observe().remote,
            "origin",
            "worktreeConfig = {disabled}"
        );
    }
}

#[test]
fn last_worktree_extension_value_wins() {
    let checkout = Checkout::new(&format!(
        "[extensions]\n\tworktreeConfig = false\n\tworktreeConfig = true\n{FORK_REFSPEC}{TRACKING}"
    ));
    checkout.write(
        "repo/.git/config.worktree",
        "[branch \"main\"]\n\tremote = fork\n",
    );
    assert_eq!(checkout.observe().remote, "fork");
}

#[test]
fn extension_inside_an_included_file_is_ignored() {
    let checkout = Checkout::new(&format!(
        "[include]\n\tpath = extension.cfg\n{FORK_REFSPEC}{TRACKING}"
    ));
    checkout.write(
        "repo/.git/extension.cfg",
        "[extensions]\n\tworktreeConfig = true\n",
    );
    checkout.write(
        "repo/.git/config.worktree",
        "[branch \"main\"]\n\tremote = fork\n",
    );
    assert_eq!(checkout.observe().remote, "origin");
}

#[test]
fn linked_worktree_reads_its_own_worktree_configuration() {
    let scratch = Scratch::new().unwrap();
    let common = scratch.path().join("repo/.git");
    let git_dir = common.join("worktrees/linked");
    let worktree = scratch.path().join("linked");
    fs::create_dir_all(&git_dir).unwrap();
    fs::create_dir_all(&worktree).unwrap();
    fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", git_dir.display()),
    )
    .unwrap();
    fs::write(git_dir.join("commondir"), "../..\n").unwrap();
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        common.join("config"),
        format!("[extensions]\n\tworktreeConfig = TRUE\n{FORK_REFSPEC}{TRACKING}"),
    )
    .unwrap();
    fs::write(
        git_dir.join("config.worktree"),
        "[branch \"main\"]\n\tremote = fork\n",
    )
    .unwrap();
    let info = git_worktree_info(&worktree).unwrap();
    let config = read_branch_config(&info, "main", &ConfigEnvironment::default())
        .branch
        .unwrap();
    assert_eq!(
        upstream(&config),
        ("fork", Some("refs/remotes/fork/main".into()))
    );
}

#[test]
fn hasconfig_condition_matches_a_remote_url() {
    let checkout = Checkout::new(&format!("{FORK_URL}{FORK_CONDITION}included.cfg\n"));
    checkout.write("repo/.git/included.cfg", INCLUDED_TRACKING);
    assert_eq!(checkout.observe().remote, "included");
}

#[test]
fn user_hasconfig_condition_matches_a_repository_remote_url() {
    let checkout = Checkout::new(FORK_URL);
    let user = checkout.write(
        "user.gitconfig",
        &format!("{FORK_CONDITION}user-included.cfg\n"),
    );
    checkout.write("user-included.cfg", INCLUDED_TRACKING);
    let config = checkout.observe_with_user(&[&user]).unwrap();
    assert_eq!(
        (config.remote.as_str(), config.merge_ref.as_str()),
        ("included", "refs/heads/main")
    );
}

#[test]
fn hasconfig_include_that_defines_a_remote_url_is_skipped() {
    let checkout = Checkout::new(FORK_URL);
    let user = checkout.write(
        "user.gitconfig",
        &format!("{TRACKING}{FORK_CONDITION}user-included.cfg\n"),
    );
    checkout.write(
        "user-included.cfg",
        "[remote \"included\"]\n\turl = https://example.test/included.git\n[branch \"main\"]\n\tremote = included\n\tmerge = refs/heads/main\n",
    );
    assert_eq!(
        checkout.observe_with_user(&[&user]).unwrap().remote,
        "origin"
    );
}

#[test]
fn hasconfig_include_chain_that_defines_a_remote_url_is_skipped() {
    let checkout = Checkout::new(FORK_URL);
    let user = checkout.write(
        "user.gitconfig",
        &format!("{TRACKING}{FORK_CONDITION}user-included.cfg\n"),
    );
    checkout.write(
        "user-included.cfg",
        "[include]\n\tpath = nested-remote.cfg\n[branch \"main\"]\n\tremote = included\n\tmerge = refs/heads/main\n",
    );
    checkout.write(
        "nested-remote.cfg",
        "[remote \"included\"]\n\turl = https://example.test/included.git\n",
    );
    assert_eq!(
        checkout.observe_with_user(&[&user]).unwrap().remote,
        "origin"
    );
}

#[test]
fn hasconfig_include_nesting_a_remote_defining_hasconfig_include_is_skipped() {
    let checkout = Checkout::new(FORK_URL);
    let user = checkout.write(
        "user.gitconfig",
        &format!("{TRACKING}{FORK_CONDITION}user-included.cfg\n"),
    );
    checkout.write(
        "user-included.cfg",
        &format!(
            "{FORK_CONDITION}nested-remote.cfg\n[branch \"main\"]\n\tremote = included\n\tmerge = refs/heads/main\n"
        ),
    );
    checkout.write(
        "nested-remote.cfg",
        "[remote \"included\"]\n\turl = https://example.test/included.git\n",
    );
    assert_eq!(
        checkout.observe_with_user(&[&user]).unwrap().remote,
        "origin"
    );
}

#[test]
fn worktree_remote_urls_do_not_satisfy_hasconfig() {
    let checkout = Checkout::new(&format!(
        "[extensions]\n\tworktreeConfig = true\n{TRACKING}"
    ));
    checkout.write(
        "repo/.git/config.worktree",
        &format!("{FORK_URL}{FORK_CONDITION}included.cfg\n"),
    );
    checkout.write(
        "repo/.git/included.cfg",
        "[branch \"main\"]\n\tremote = included\n",
    );
    assert_eq!(checkout.observe().remote, "origin");
}

#[test]
fn inline_comment_after_a_fetch_refspec_is_ignored() {
    let config = Checkout::new(&format!(
        "[remote \"origin\"]\n\tfetch = +refs/heads/*:refs/remotes/upstream/* # custom map\n{TRACKING}"
    ))
    .observe();
    assert_eq!(
        config.upstream_full_ref().as_deref(),
        Some("refs/remotes/upstream/main")
    );
}

#[test]
fn unmapped_refspec_clears_the_upstream() {
    let config = Checkout::new(&format!(
        "[remote \"origin\"]\n\tfetch = +refs/pull/*:refs/remotes/origin/pr/*\n{TRACKING}"
    ))
    .observe();
    assert_eq!(config.upstream_full_ref(), None);
}

#[test]
fn negative_refspec_does_not_remove_the_mapping() {
    let config = Checkout::new(&format!(
        "[remote \"origin\"]\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n\tfetch = ^refs/heads/main\n{TRACKING}"
    ))
    .observe();
    assert_eq!(
        config.upstream_full_ref().as_deref(),
        Some("refs/remotes/origin/main")
    );
}

#[test]
fn exact_refspec_maps_only_its_own_reference() {
    let config = Checkout::new(&format!(
        "[remote \"origin\"]\n\tfetch = refs/heads/other:refs/remotes/origin/other\n\tfetch = refs/heads/main:refs/remotes/origin/trunk\n{TRACKING}"
    ))
    .observe();
    assert_eq!(
        config.upstream_full_ref().as_deref(),
        Some("refs/remotes/origin/trunk")
    );
}

#[test]
fn branch_without_a_merge_reference_has_no_tracking() {
    let checkout = Checkout::new("[branch \"main\"]\n\tremote = origin\n");
    assert!(
        checkout
            .observe_with(&ConfigEnvironment::default())
            .is_none()
    );
}

#[test]
fn other_branches_are_not_read() {
    let checkout =
        Checkout::new("[branch \"other\"]\n\tremote = origin\n\tmerge = refs/heads/other\n");
    assert!(
        checkout
            .observe_with(&ConfigEnvironment::default())
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn retargeted_symlink_makes_the_read_stale() {
    let checkout = Checkout::new("");
    let first = checkout.write("first.cfg", "");
    let second = checkout.write("second.cfg", "");
    let alias = checkout.scratch.path().join("branch.cfg");
    std::os::unix::fs::symlink(&first, &alias).unwrap();
    let info = git_worktree_info(&checkout.root()).unwrap();
    let environment = ConfigEnvironment {
        user_files: vec![alias.clone()],
        home: None,
    };
    let read = read_branch_config(&info, "main", &environment);
    assert!(read.is_current());
    fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&second, &alias).unwrap();
    assert!(!read.is_current());
}

#[test]
fn edited_file_makes_the_read_stale() {
    let checkout = Checkout::new(TRACKING);
    let info = git_worktree_info(&checkout.root()).unwrap();
    let read = read_branch_config(&info, "main", &ConfigEnvironment::default());
    assert!(read.is_current());
    fs::write(
        checkout.git().join("config"),
        format!("{TRACKING}# edited\n"),
    )
    .unwrap();
    assert!(!read.is_current());
}

#[test]
fn created_file_makes_a_missing_read_stale() {
    let checkout = Checkout::new("[include]\n\tpath = later.cfg\n");
    let info = git_worktree_info(&checkout.root()).unwrap();
    let read = read_branch_config(&info, "main", &ConfigEnvironment::default());
    assert!(read.is_current());
    checkout.write("repo/.git/later.cfg", INCLUDED_TRACKING);
    assert!(!read.is_current());
}

#[test]
fn unreadable_file_is_read_again_next_time() {
    let checkout = Checkout::new("");
    fs::remove_file(checkout.git().join("config")).unwrap();
    fs::create_dir(checkout.git().join("config")).unwrap();
    let info = git_worktree_info(&checkout.root()).unwrap();
    let read = read_branch_config(&info, "main", &ConfigEnvironment::default());
    assert!(!read.is_current());
}

#[test]
fn user_files_follow_git_precedence() {
    let home = Path::new("/home/member");
    let xdg = Path::new("/xdg");
    let files = |xdg, home| ConfigEnvironment::from_variables(xdg, home).user_files;
    assert_eq!(
        files(Some(xdg), Some(home)),
        [
            PathBuf::from("/xdg/git/config"),
            PathBuf::from("/home/member/.gitconfig")
        ]
    );
    assert_eq!(
        files(None, Some(home)),
        [
            PathBuf::from("/home/member/.config/git/config"),
            PathBuf::from("/home/member/.gitconfig")
        ]
    );
    assert_eq!(files(Some(xdg), None), [PathBuf::from("/xdg/git/config")]);
    assert!(files(None, None).is_empty());
    assert_eq!(
        ConfigEnvironment::from_variables(None, Some(home))
            .home
            .as_deref(),
        Some(home)
    );
}
