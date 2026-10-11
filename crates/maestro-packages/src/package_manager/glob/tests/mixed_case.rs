//! Public manager observations under injected filesystem case policies.
use crate::package_manager::{
    CommandCaptureOptions, CommandOutput, PackageFuture, PackageOperations,
};
use crate::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use maestro_pattern::Platform;
use maestro_resources::{
    NativeResourceOperations,
    skills::{ResourceEntry, ResourceFileType, ResourceOperations},
};
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
    rc::Rc,
};
/// Native adapter mapping only the authored B component to its disk spelling.
struct CaseFilesystem;
/// Maps a path without substituting listing or ordering decisions.
fn native(path: &Path) -> PathBuf {
    path.components()
        .map(|part| {
            if part.as_os_str() == "B" {
                std::ffi::OsStr::new("b")
            } else {
                part.as_os_str()
            }
        })
        .collect()
}
impl ResourceOperations for CaseFilesystem {
    fn current_directory(&self) -> io::Result<String> {
        panic!("unexpected cwd")
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        vec![]
    }
    fn exists(&self, path: &Path) -> bool {
        NativeResourceOperations.exists(&native(path))
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        NativeResourceOperations.read_dir(&native(path))
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        NativeResourceOperations.read_file(&native(path))
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        NativeResourceOperations.metadata(&native(path))
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        NativeResourceOperations.symlink_metadata(&native(path))
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        NativeResourceOperations.canonicalize(&native(path))
    }
}
/// Package effects restricted to local manifest resolution.
struct Local;
impl PackageOperations for Local {
    fn resource_operations(&self) -> &dyn ResourceOperations {
        &CaseFilesystem
    }
    fn read_file(&self, path: &str) -> io::Result<String> {
        std::fs::read_to_string(native(Path::new(path)))
    }
    fn offline_value(&self) -> Option<String> {
        None
    }
    fn exists(&self, path: &str) -> bool {
        CaseFilesystem.exists(Path::new(path))
    }
    fn home_dir(&self) -> io::Result<String> {
        panic!("unexpected home")
    }
    fn temp_dir(&self) -> String {
        panic!("unexpected temp directory")
    }
    fn current_dir(&self) -> io::Result<String> {
        panic!("unexpected cwd")
    }
    fn drive_directory(&self, _: char) -> Option<String> {
        None
    }
    fn run_command_capture<'a>(
        &'a self,
        _: &'a str,
        _: &'a [String],
        _: CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String> {
        panic!("unexpected command")
    }
    fn spawn(
        &self,
        _: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> io::Result<()> {
        panic!("unexpected spawn")
    }
    fn run_command_sync(&self, _: &str, _: &[String]) -> io::Result<CommandOutput> {
        panic!("unexpected command")
    }
    fn create_dir_all(&self, _: &str) -> io::Result<()> {
        panic!("unexpected create")
    }
    fn write_file(&self, _: &str, _: &str, _: bool) -> io::Result<()> {
        panic!("unexpected write")
    }
    fn directory_is_empty(&self, _: &str) -> io::Result<bool> {
        panic!("unexpected listing")
    }
    fn remove_path(&self, _: &str) -> io::Result<()> {
        panic!("unexpected removal")
    }
    fn run_command<'a>(
        &'a self,
        _: &'a str,
        _: &'a [String],
        _: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        panic!("unexpected command")
    }
}
/// Restores the private host selection even when an assertion fails.
struct Policy;
impl Drop for Policy {
    fn drop(&mut self) {
        super::super::TEST_PLATFORM.with(|selected| selected.set(None));
    }
}
/// Promotion matches comparison keys but retains the authored path and metadata.
#[test]
fn manifest_mixed_case_promotion_keeps_authored_alias() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for (policy, name) in [(Platform::Darwin, "darwin"), (Platform::Windows, "windows")] {
        super::super::TEST_PLATFORM.with(|selected| selected.set(Some(policy)));
        let _policy = Policy;
        for alias in [false, true] {
            let scene = super::Scene(std::env::temp_dir().join(format!(
                "maestro-mixed-{name}-{alias}-{}",
                std::process::id()
            )));
            std::fs::create_dir(&scene.0).unwrap();
            super::setup(
                &scene.0,
                super::Setup {
                    files: [
                        ("a/u.ts".into(), "fixture".into()),
                        ("b/preload.ts".into(), "fixture".into()),
                    ]
                    .into(),
                    dirs: vec![],
                    links: std::collections::BTreeMap::default(),
                },
            );
            if alias {
                std::os::unix::fs::symlink("../a/u.ts", scene.0.join("b/u.ts")).unwrap();
            } else {
                std::fs::write(scene.0.join("b/u.ts"), "fixture").unwrap();
            }
            std::fs::write(
                scene.0.join("package.json"),
                r#"{"maestro":{"extensions":["{B/preload.ts,*/*.ts}"]}}"#,
            )
            .unwrap();
            let root = scene.0.to_str().unwrap();
            let manager = DefaultPackageManager::new(
                PackageManagerOptions {
                    cwd: root.into(),
                    agent_dir: root.into(),
                    settings_manager: Rc::new(RefCell::new(
                        maestro_settings::SettingsManager::in_memory(
                            maestro_settings::Settings::default(),
                        ),
                    )),
                },
                Local,
            );
            let result = runtime
                .block_on(manager.resolve_extension_sources(&[root.into()], None))
                .unwrap();
            let expected = if alias {
                vec!["B/preload.ts", "a/u.ts"]
            } else {
                vec!["B/preload.ts", "a/u.ts", "B/u.ts"]
            };
            assert_eq!(
                result
                    .extensions
                    .iter()
                    .map(|r| r.path.strip_prefix(&format!("{root}/")).unwrap())
                    .collect::<Vec<_>>(),
                expected,
                "{name} alias={alias}"
            );
            for resource in result.extensions {
                assert!(resource.enabled);
                assert_eq!(resource.metadata.source, root);
                assert_eq!(
                    resource.metadata.scope,
                    maestro_resources::SourceScope::User
                );
                assert_eq!(resource.metadata.origin, crate::SourceOrigin::Package);
                assert_eq!(resource.metadata.base_dir.as_deref(), Some(root));
            }
            assert!(
                result.skills.is_empty() && result.prompts.is_empty() && result.themes.is_empty()
            );
        }
    }
}
