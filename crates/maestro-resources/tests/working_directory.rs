#![cfg(unix)]

#[cfg(test)]
pub mod support;
use maestro_resources::{
    LoadSkillsFromDirOptions, LoadSkillsOptions, NativeResourceOperations, canonicalize_path,
    load_skills, load_skills_from_dir,
};
use std::io::ErrorKind;
use support::text;

/// Only a path no operand anchors needs the working directory, which this test removes.
#[test]
fn maestro_paths_resolve_absolute_paths_without_a_working_directory() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    let skill = dir.file(
        "skills/calendar/SKILL.md",
        "---\ndescription: Calendar\n---",
    );
    let alias = dir.0.join("alias");
    std::os::unix::fs::symlink(dir.0.join("target"), &alias).unwrap();
    let gone = dir.0.join("gone");
    std::fs::create_dir(&gone).unwrap();
    std::env::set_current_dir(&gone).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    assert_eq!(
        canonicalize_path(text(&alias.join("skill.md")), &NativeResourceOperations),
        text(&file)
    );
    for relative in ["alias/skill.md", "é.md", "é/ü.md"] {
        assert_eq!(
            canonicalize_path(relative, &NativeResourceOperations),
            relative
        );
    }
    let options = |cwd| LoadSkillsOptions {
        cwd,
        home: "/home",
        agent_dir: "/agent",
        config_dir_name: ".maestro",
        skill_paths: &[],
        include_defaults: true,
    };
    let error = load_skills(options("work"), &NativeResourceOperations).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(
        load_skills(options(text(&dir.0)), &NativeResourceOperations)
            .unwrap()
            .skills
            .is_empty()
    );
    let scan = |cwd, dir| {
        load_skills_from_dir(
            LoadSkillsFromDirOptions {
                cwd,
                dir,
                source: "path",
            },
            &NativeResourceOperations,
        )
    };
    assert!(scan("work", "skills").skills.is_empty());
    let loaded = scan("work", text(&dir.0.join("skills")));
    assert_eq!(loaded.skills[0].file_path, text(&skill));
}
