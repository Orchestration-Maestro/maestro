//! Artifact runtime guidance available to browser SDK consumers.
#![cfg(all(test, not(target_arch = "wasm32")))]

#[test]
fn read_write_artifact_prompt_is_exported_for_embedding() {
    let prompt: &str = maestro_web::ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW;
    assert_eq!(
        prompt,
        maestro_app::browser::tools::prompts::ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW
    );
    assert!(prompt.contains("createOrUpdateArtifact(filename, content, mimeType?)"));
    assert!(prompt.contains("deleteArtifact(filename)"));
}
