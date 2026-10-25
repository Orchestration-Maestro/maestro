//! Skill name collisions between user, project and package skills.
#![cfg(unix)]
use crate::resource_fixtures::check_loader_cases;

#[test]
fn user_skill_overrides_package_skill() {
    check_loader_cases(
        "user_skill_overrides_package_skill",
        &["user_skill_beats_package_skill"],
    );
}

#[test]
fn project_skill_overrides_package_skill() {
    check_loader_cases(
        "project_skill_overrides_package_skill",
        &["project_skill_beats_package_skill"],
    );
}

#[test]
fn project_skill_overrides_user_and_package_skills() {
    check_loader_cases(
        "project_skill_overrides_user_and_package_skills",
        &["project_beats_user_beats_package"],
    );
}

#[test]
fn package_skill_is_the_collision_loser() {
    check_loader_cases(
        "package_skill_is_the_collision_loser",
        &["package_skill_reported_as_collision_loser"],
    );
}
