//! Throwaway qualification of native Windows path spellings through the real author.
#![cfg(windows)]
use super::wasi_support::{COMPONENT, Harness, run};
use serde_json::json;
use std::path::Path;

/// Records each spelling's outcome in both generations without asserting.
fn observe(layout: &str) {
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::create_dir(scratch.path().join("cwd")).unwrap();
    std::fs::create_dir(scratch.path().join("sibling Ω space")).unwrap();
    if layout == "root" {
        std::fs::write(scratch.path().join("cwd/probe-root-layout"), "").unwrap();
    }
    std::env::set_current_dir(scratch.path().join("cwd")).unwrap();
    let target = scratch.path().join("sibling Ω space/file Ω name");
    let verbatim = target.to_str().unwrap().replace('/', "\\");
    let native = verbatim.trim_start_matches(r"\\?\").to_owned();
    let forward = native.replace('\\', "/");
    let unc = format!(r"\\localhost\{}${}", &native[..1], &native[2..]);
    let paths = [
        verbatim.clone(),
        native.clone(),
        maestro_extensions::helpers::guest_path(&native),
        forward.clone(),
        format!("/{forward}"),
        format!("/{native}"),
        "../sibling Ω space/file Ω name".into(),
        r"..\sibling Ω space\file Ω name".into(),
        unc.clone(),
        format!("//localhost/{}${}", &forward[..1], &forward[2..]),
    ];
    println!("PROBE layout={layout} native-cwd={:?}", std::env::current_dir().unwrap());
    println!("PROBE layout={layout} native-unc-control={:?}", std::fs::read(&unc).map(|_| ()));
    run(async {
        let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
        let cwd = host.request(json!({"op":"cwd"})).await;
        println!("PROBE layout={layout} guest-cwd={cwd}");
        for generation in ["p2", "p3"] {
            for path in &paths {
                let observed = host
                    .request(json!({"op":"file","generation":generation,"path":path,"write":true,"bytes":b"x"}))
                    .await;
                println!("PROBE layout={layout} {generation} {path:?} => {observed}");
            }
        }
    });
    std::env::set_current_dir(original).unwrap();
}

#[test]
fn probe_drive_layout() {
    observe("drives");
}

#[test]
fn probe_root_layout() {
    observe("root");
}
