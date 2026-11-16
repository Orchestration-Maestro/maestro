#![cfg(unix)]

use crate::support;
use maestro_resources::{
    LoadSkillsFromDirOptions, LoadSkillsOptions, canonicalize_path, load_skills,
    load_skills_from_dir,
};
use std::io::ErrorKind;
use support::text;

/// Only a path no operand anchors needs the working directory, which this test removes.
#[test]
fn maestro_paths_resolve_absolute_paths_without_a_working_directory() {
    // Removing the process working directory would leave every other test in this
    // executable without one, so this test runs alone in a child process.
    if !maestro_test_process::selection().is_child() {
        {
            let output = maestro_test_process::TestProcess::current(
                "maestro_paths_resolve_absolute_paths_without_a_working_directory",
            )
            .unwrap()
            .nocapture()
            .env_clear()
            .case("removed-working-directory")
            .output()
            .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
        return;
    }
    check_removed_working_directory();
}

/// Absolute anchors survive a removed working directory; relative operands remain unchanged.
fn check_removed_working_directory() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    let skill = dir.file(
        "skills/calendar/SKILL.md",
        "---\ndescription: Calendar\n---",
    );
    let alias = dir.0.path().join("alias");
    std::os::unix::fs::symlink(dir.0.path().join("target"), &alias).unwrap();
    dir.0.enter_removed("gone").unwrap();
    assert_eq!(
        canonicalize_path(text(&alias.join("skill.md"))),
        text(&file)
    );
    for relative in ["alias/skill.md", "é.md", "é/ü.md"] {
        assert_eq!(canonicalize_path(relative), relative);
    }
    let options = |cwd| LoadSkillsOptions {
        cwd,
        home: "/home",
        agent_dir: "/agent",
        config_dir_name: ".maestro",
        skill_paths: &[],
        include_defaults: true,
    };
    let error = load_skills(options("work")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(
        load_skills(options(text(dir.0.path())))
            .unwrap()
            .skills
            .is_empty()
    );
    let scan = |cwd, dir| {
        load_skills_from_dir(LoadSkillsFromDirOptions {
            cwd,
            dir,
            source: "path",
        })
    };
    assert!(scan("work", "skills").skills.is_empty());
    let loaded = scan("work", text(&dir.0.path().join("skills")));
    assert_eq!(loaded.skills[0].file_path, text(&skill));
}
