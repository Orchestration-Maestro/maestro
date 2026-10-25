//! Native post-update observations from isolated repository histories.
use super::types::{Heads, Native};
use crate::native_git::{Repository, git};

/// Produces the recorded checkout observations from real Git operations.
pub(super) fn run(scenario: &str) -> Native {
    let repo = Repository::new();
    prepare(&repo, scenario);
    if scenario == "scope-only" {
        repo.update(false);
    } else {
        repo.update_all(scenario == "pinned");
    }
    let head = git(&repo.target, &["rev-parse", "HEAD"]);
    Native {
        heads: Heads {
            remote: head == git(&repo.remote, &["rev-parse", "HEAD"]),
            original: head == repo.original,
        },
        content: std::fs::read_to_string(repo.target.join("content")).unwrap(),
        untracked_exists: repo.target.join("untracked").exists(),
        project_acquired: repo
            .scratch
            .path()
            .join("cwd/.maestro/git/github.com/acme/tool")
            .exists(),
    }
}
/// Builds distinguishable histories without deriving expected results from the updater.
fn prepare(repo: &Repository, scenario: &str) {
    match scenario {
        "current" => {
            std::fs::write(repo.target.join("content"), "dirty").unwrap();
            std::fs::write(repo.target.join("untracked"), "keep").unwrap();
        }
        "multiple" => {
            repo.commit("second");
            repo.commit("third");
            repo.commit("fourth");
        }
        "rewritten" | "removed" | "replacement" => replacement(repo, scenario),
        _ => {
            repo.commit("updated");
        }
    }
    if scenario == "detached" {
        git(&repo.target, &["checkout", "--detach", &repo.original]);
    }
    if scenario == "non-origin" {
        git(
            &repo.target,
            &["remote", "add", "upstream", repo.remote.to_str().unwrap()],
        );
        git(&repo.target, &["fetch", "upstream"]);
        git(&repo.target, &["branch", "--set-upstream-to=upstream/main"]);
    }
}
/// Replaces a previously installed multi-commit history, including unrelated roots.
fn replacement(repo: &Repository, scenario: &str) {
    repo.commit("second");
    repo.commit("third");
    repo.update_all(false);
    if scenario == "replacement" {
        git(&repo.seed, &["checkout", "--orphan", "replacement"]);
        git(&repo.seed, &["rm", "-rf", "."]);
        std::fs::write(repo.seed.join("other"), "unrelated").unwrap();
        git(&repo.seed, &["add", "other"]);
        git(&repo.seed, &["commit", "-m", "orphan"]);
        git(&repo.seed, &["branch", "-M", "main"]);
        repo.commit("unrelated");
    } else {
        git(&repo.seed, &["reset", "--hard", &repo.original]);
        repo.commit(if scenario == "rewritten" {
            "rewritten"
        } else {
            "removed-replaced"
        });
    }
    std::fs::write(repo.target.join("untracked"), "remove").unwrap();
}
