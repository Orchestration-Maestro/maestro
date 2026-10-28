//! Package descriptors observed independently of discovered resources.
use crate::sandbox::{Fixture, block_on};
use maestro_packages::{PackageManager, SourceScope};
use maestro_settings::PackageSource;
use serde_json::json;

#[test]
fn resolved_packages_keep_project_pin_and_empty_filtered_roots() {
    let fixture =
        Fixture::new(&json!({"packages":["https://example.test/team/repo@v1", "./empty"]}));
    fixture
        .settings
        .borrow_mut()
        .set_project_packages(vec![PackageSource::Unknown(
            json!({"source":"https://example.test/team/repo@v2", "extensions":[]}),
        )]);
    let project = fixture
        .scratch
        .path()
        .join("project/.maestro/git/example.test/team/repo");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(fixture.agent("empty")).unwrap();
    std::fs::write(project.join("package.json"), r#"{"maestro":{}}"#).unwrap();
    std::fs::write(
        std::path::Path::new(&fixture.agent("empty")).join("package.json"),
        r#"{"maestro":{}}"#,
    )
    .unwrap();
    let result = block_on(fixture.manager.resolve(None)).unwrap();
    assert!(result.extensions.is_empty());
    assert_eq!(result.packages.len(), 2);
    let selected = &result.packages[0];
    assert_eq!(selected.source, "https://example.test/team/repo@v2");
    assert_eq!(selected.identity, "git:example.test/team/repo");
    assert_eq!(selected.scope, SourceScope::Project);
    assert_eq!(selected.installed_path, project.to_str().unwrap());
    assert_eq!(result.packages[1].scope, SourceScope::User);
    assert_eq!(result.packages[1].source, "./empty");
    assert_eq!(
        result.packages[1].identity,
        format!("local:{}", fixture.agent("empty"))
    );
    assert_eq!(result.packages[1].installed_path, fixture.agent("empty"));
}

#[test]
fn explicit_descriptors_keep_npm_pins_and_temporary_local_scope() {
    let fixture = Fixture::new(&json!({}));
    let npm = fixture
        .scratch
        .path()
        .join("project/.maestro/npm/node_modules/example");
    std::fs::create_dir_all(&npm).unwrap();
    std::fs::write(
        npm.join("package.json"),
        r#"{"version":"1.2.3","maestro":{}}"#,
    )
    .unwrap();
    let result = block_on(
        fixture
            .manager
            .resolve_extension_sources(&["npm:example@1.2.3".into()], Some(SourceScope::Project)),
    )
    .unwrap();
    assert!(result.extensions.is_empty());
    assert_eq!(result.packages.len(), 1);
    assert_eq!(result.packages[0].identity, "npm:example");
    assert_eq!(result.packages[0].source, "npm:example@1.2.3");
    assert_eq!(result.packages[0].scope, SourceScope::Project);
    assert_eq!(result.packages[0].installed_path, npm.to_str().unwrap());
    let local = npm.to_str().unwrap().to_owned();
    let result = block_on(
        fixture
            .manager
            .resolve_extension_sources(std::slice::from_ref(&local), Some(SourceScope::Temporary)),
    )
    .unwrap();
    assert_eq!(result.packages[0].scope, SourceScope::Temporary);
    assert_eq!(result.packages[0].installed_path, local);
}
