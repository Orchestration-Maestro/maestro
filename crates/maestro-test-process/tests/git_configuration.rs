//! Native Git fixtures ignore the runner's configuration and signing policy.

use maestro_test_process::{git_command, with_environment};
use std::path::Path;

/// Run a fixture command in `root` and retain its diagnostics on failure.
fn git(root: &Path, args: &[&str]) -> String {
    let output = git_command::command()
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().into()
}

/// Inherited settings cannot change fixture authors or turn a lightweight tag into a signed tag.
#[test]
fn git_commands_ignore_user_configuration_and_signing() {
    let path = std::env::var("PATH").unwrap();
    with_environment(
        "git_commands_ignore_user_configuration_and_signing",
        &[
            ("PATH", &path),
            ("GIT_CONFIG_GLOBAL", "$BASE/global"),
            ("GIT_CONFIG_SYSTEM", "$BASE/system"),
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "fixture.injected"),
            ("GIT_CONFIG_VALUE_0", "inherited"),
            ("GIT_CONFIG_PARAMETERS", "'fixture.parameters=inherited'"),
            ("GIT_AUTHOR_NAME", "Inherited Author"),
            ("GIT_AUTHOR_EMAIL", "inherited@example.invalid"),
        ],
        |base| {
            let root = Path::new(base);
            for file in ["global", "system"] {
                std::fs::write(root.join(file), "[fixture]\n inherited = true\n[tag]\n gpgsign = true\n[commit]\n gpgsign = true\n").unwrap();
            }
            std::fs::create_dir(root.join("home")).unwrap();
            std::fs::write(root.join("home/.gitconfig"), "[fixture]\n home = true\n").unwrap();

            let config = git(root, &["config", "--list", "--show-origin"]);
            assert!(!config.contains("fixture."), "{config}");
            git(root, &["init", "-q"]);
            git(root, &["commit", "--quiet", "--allow-empty", "-m", "fixture"]);
            git(root, &["tag", "v1"]);

            assert_eq!(git(root, &["log", "-1", "--format=%an <%ae>"]), "Fixture <fixture@example.invalid>");
            assert_eq!(git(root, &["cat-file", "-t", "v1"]), "commit");
        },
    ).unwrap();
}
