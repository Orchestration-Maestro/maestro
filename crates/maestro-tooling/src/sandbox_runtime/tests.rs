//! Audit decisions over generated script text and linker maps.
use super::audit::{libraries, script};
use super::{rustflags, tool};
use std::path::Path;

/// Linker map lines of the audited runtime build.
const MAP: &str = "\
Addr Off Size Out In Symbol
  - 3a995 CODE
  4d 1f2 11 CODE /t/release/deps/maestro_sandbox_runtime.maestro_sandbox_runtime.14d67be3bc71c250-cgu.0.rcgu.o:(f)
  4d 1f2 11 CODE /s/lib/wasm32-emscripten/libc.a(strdup.o):(strdup)
  4d 1f2 11 CODE /s/lib/wasm32-emscripten/libdlmalloc.a(dlmalloc.o):(malloc)
  4d 1f2 11 CODE /r/lib/rustlib/wasm32-unknown-emscripten/lib/libcompiler_builtins-dcafdf8ff8e30174.rlib(compiler_builtins.o):(memcpy)
  4d 1f2 11 CODE /s/lib/wasm32-emscripten/libclang_rt.builtins-legacysjlj.a(stack_ops.o):(_emscripten_stack_alloc)
";

#[test]
fn audited_libraries_are_accepted_and_listed() {
    let linked = libraries(MAP).unwrap();
    assert_eq!(
        linked,
        [
            "libc.a",
            "libclang_rt.builtins-legacysjlj.a",
            "libcompiler_builtins-dcafdf8ff8e30174.rlib",
            "libdlmalloc.a",
            "maestro_sandbox_runtime.maestro_sandbox_runtime.14d67be3bc71c250-cgu.0.rcgu.o",
        ]
    );
}

#[test]
fn unlisted_archives_and_objects_are_refused() {
    for line in [
        "/s/lib/wasm32-emscripten/libc++abi.a(abi.o):(f)",
        "/s/lib/wasm32-emscripten/libemmalloc.a(emmalloc.o):(malloc)",
        "/s/lib/wasm32-emscripten/libGL.a(gl.o):(glClear)",
        "/t/release/deps/other_crate.0.rcgu.o:(f)",
    ] {
        let refusal = libraries(&format!("{MAP}  4d 1f2 11 CODE {line}\n")).unwrap_err();
        assert!(
            refusal.contains("not on the audited list"),
            "{line}: {refusal}"
        );
    }
}

#[test]
fn script_text_must_survive_an_inline_script_and_forbid_dynamic_code() {
    script(b"var Module=typeof Module!=\"undefined\"?Module:{};").unwrap();
    for (bytes, expected) in [
        (&b"a\0b"[..], "NUL"),
        (&b"a\rb"[..], "\\r"),
        (&b"x</SCRIPT>"[..], "</script"),
        (&b"x<!--"[..], "<!--"),
        (&b"eval(1)"[..], "eval("),
        (&b"new Function(1)"[..], "new Function"),
        (&b"\xff"[..], "UTF-8"),
    ] {
        let refusal = script(bytes).unwrap_err();
        assert!(refusal.contains(expected), "{refusal}");
    }
}

#[test]
fn rustflags_keep_paths_with_spaces_in_one_argument() {
    let flags = rustflags(
        Path::new("/work/maestro sandbox"),
        Path::new("/work/maestro sandbox/target/sandbox-runtime/runtime.map"),
        &[],
    );
    let arguments: Vec<&str> = flags.split('\x1f').collect();
    assert!(arguments.contains(&"--remap-path-prefix=/work/maestro sandbox=/checkout"));
    assert!(
        arguments.contains(
            &"-Clink-arg=-Wl,-Map=/work/maestro sandbox/target/sandbox-runtime/runtime.map"
        )
    );
}

/// Dependency sources are recorded under their package name and version, so the generated
/// script is the same whichever Cargo home built it.
#[test]
fn dependency_sources_are_recorded_without_the_cargo_home() {
    let root = "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/js-sys-0.3.106";
    let flags = rustflags(
        Path::new("/work/maestro"),
        Path::new("/work/maestro/target/sandbox-runtime/runtime.map"),
        &[(root.to_owned(), "js-sys-0.3.106".to_owned())],
    );
    let arguments: Vec<&str> = flags.split('\x1f').collect();
    assert!(
        arguments.contains(&format!("--remap-path-prefix={root}=js-sys-0.3.106").as_str()),
        "{flags}"
    );
}

#[test]
fn a_missing_tool_is_named_with_its_release() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let absent = scratch.path().join("emcc");
    let name = absent.to_str().unwrap();
    let refusal = tool(name, "6.0.10").unwrap_err().to_string();
    assert!(refusal.contains(name), "{refusal}");
    assert!(refusal.contains("6.0.10"), "{refusal}");
}
