//! Application-selected settings persistence qualification.
#![cfg(test)]
#![cfg(unix)]
/// Shared recorded queries and assertions.
#[path = "support/default_configuration.rs"]
pub mod fixture;
#[test]
fn externally_removed_packages_survive_unrelated_write() {
    fixture::settings(
        "externally_removed_packages_survive_unrelated_write",
        fixture::Group::ExternallyRemovedPackagesSurviveUnrelatedWrite,
    );
}

#[test]
fn external_extensions_survive_unrelated_write() {
    fixture::settings(
        "external_extensions_survive_unrelated_write",
        fixture::Group::ExternalExtensionsSurviveUnrelatedWrite,
    );
}

#[test]
fn external_project_prompts_survive_project_extension_write() {
    fixture::settings(
        "external_project_prompts_survive_project_extension_write",
        fixture::Group::ExternalProjectPromptsSurviveProjectExtensionWrite,
    );
}

#[test]
fn project_edits_win_over_external_same_field() {
    fixture::settings(
        "project_edits_win_over_external_same_field",
        fixture::Group::ProjectEditsWinOverExternalSameField,
    );
}
