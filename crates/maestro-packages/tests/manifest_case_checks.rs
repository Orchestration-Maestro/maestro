//! Manifest expansion on volumes that fold case, as macOS and Windows volumes usually do.
#![cfg(test)]
#![cfg(any(target_os = "macos", windows))]
use crate::manifest_resource_checks::write;
use crate::resolution_support;
use crate::scratch;
use maestro_packages::PackageManager;
use serde_json::json;

/// Promotion matches comparison keys but retains the authored path and metadata.
#[test]
fn manifest_mixed_case_promotion_keeps_authored_alias() {
    let variants: &[bool] = if cfg!(unix) { &[false, true] } else { &[false] };
    for &linked in variants {
        resolution_support::run(async {
            let scene = scratch::Scratch::new()?;
            write(&scene.path().join("b/preload.ts"), "fixture");
            // A volume that keeps case apart has nothing to fold.
            if !scene.path().join("B/preload.ts").exists() {
                return Ok(());
            }
            write(&scene.path().join("a/u.ts"), "fixture");
            if linked {
                #[cfg(unix)]
                std::os::unix::fs::symlink("../a/u.ts", scene.path().join("b/u.ts"))?;
            } else {
                write(&scene.path().join("b/u.ts"), "fixture");
            }
            write(
                &scene.path().join("package.json"),
                r#"{"maestro":{"extensions":["{B/preload.ts,*/*.ts}"]}}"#,
            );
            let base = scene.path().to_str().unwrap();
            let (manager, _) = resolution_support::manager(base);
            let actual = manager
                .resolve_extension_sources(&[base.into()], None)
                .await?;
            let resource = |name: &str| {
                let sep = std::path::MAIN_SEPARATOR_STR;
                let path = format!("$BASE{sep}{}", name.replace('/', sep));
                json!({"path":path,"enabled":true,"metadata":{"source":"$BASE","scope":"user","origin":"package","baseDir":"$BASE"}})
            };
            let mut expected = vec![resource("B/preload.ts"), resource("a/u.ts")];
            if !linked {
                expected.push(resource("B/u.ts"));
            }
            assert_eq!(
                resolution_support::output(actual, base),
                json!({"extensions":expected,"skills":[],"prompts":[],"themes":[]}),
                "linked={linked}"
            );
            Ok(())
        })
        .unwrap();
    }
}
