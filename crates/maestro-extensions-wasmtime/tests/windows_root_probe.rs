//! Throwaway qualification of unconverted native spellings with a plain cwd.
#![cfg(windows)]
use super::wasi_support::{COMPONENT, Harness, run};
use serde_json::json;
use std::path::Path;

#[test]
fn probe_unconverted_spellings() {
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let plain = scratch.path().to_str().unwrap().trim_start_matches(r"\\?\").to_owned();
    std::fs::create_dir(format!(r"{plain}\cwd")).unwrap();
    std::fs::create_dir(format!(r"{plain}\sibling Ω space")).unwrap();
    std::fs::create_dir_all(format!(r"{plain}\cwd\{}\sibling Ω space", &plain[3..])).unwrap();
    std::fs::write(format!(r"{plain}\cwd\{}\sibling Ω space\file Ω name", &plain[3..]), b"decoy").unwrap();
    std::env::set_current_dir(format!(r"{plain}\cwd")).unwrap();
    let native = format!(r"{plain}\sibling Ω space\file Ω name");
    std::fs::write(&native, b"target").unwrap();
    println!("PROBE plain-cwd={:?}", std::env::current_dir().unwrap());
    run(async {
        let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
        println!("PROBE guest-cwd={}", host.request(json!({"op":"cwd"})).await);
        for generation in ["p2", "p3"] {
            for path in [native.clone(), native.replace('\\', "/"), format!(r"\\?\{native}")] {
                let observed = host
                    .request(json!({"op":"file","generation":generation,"path":path,"raw":true,"bytes":[]}))
                    .await;
                let text = observed["bytes"].as_array().map(|bytes| String::from_utf8(bytes.iter().map(|b| b.as_u64().unwrap() as u8).collect()).unwrap());
                println!("PROBE raw {generation} {path:?} => {observed} text={text:?}");
            }
        }
    });
    std::env::set_current_dir(original).unwrap();
}
