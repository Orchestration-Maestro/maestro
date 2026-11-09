//! Refusals for generated script text and for libraries outside the audited list.
use std::path::Path;

/// Text an inline classic script cannot carry verbatim.
const UNSAFE_TEXT: [&str; 4] = ["</script", "<script", "<!--", "\r"];
/// Dynamic code compilation the frame's policy forbids.
const DYNAMIC_CODE: [&str; 2] = ["eval(", "new Function"];

/// Why generated output is not acceptable.
pub(super) type Refusal = String;

/// Accept script bytes an HTML document carries verbatim and no policy forbids.
pub(super) fn script(bytes: &[u8]) -> Result<(), Refusal> {
    if bytes.contains(&0) {
        return Err("the script contains a NUL byte".into());
    }
    let text =
        std::str::from_utf8(bytes).map_err(|error| format!("the script is not UTF-8: {error}"))?;
    let lower = text.to_ascii_lowercase();
    for fragment in UNSAFE_TEXT {
        if lower.contains(fragment) {
            return Err(format!(
                "the script contains {fragment:?}, which an inline script cannot carry"
            ));
        }
    }
    for fragment in DYNAMIC_CODE {
        if text.contains(fragment) {
            return Err(format!("the script contains {fragment:?}"));
        }
    }
    Ok(())
}

/// The linked archives and objects of a linker map, each audited.
pub(super) fn libraries(map: &str) -> Result<Vec<String>, Refusal> {
    let mut found = std::collections::BTreeSet::new();
    for token in map.split_whitespace() {
        let file = token
            .split('(')
            .next()
            .unwrap_or(token)
            .trim_end_matches(':');
        let name = file.rsplit('/').next().unwrap_or(file);
        let path = file.contains('/');
        let extension = extension_of(name);
        let archive = path && matches!(extension, Some("a" | "rlib"));
        let object =
            path && extension == Some("o") && !token.contains(".a(") && !token.contains(".rlib(");
        if archive || object {
            if !audited(name) {
                return Err(format!("{name} is linked but is not on the audited list"));
            }
            found.insert(name.to_owned());
        }
    }
    Ok(found.into_iter().collect())
}

/// The extension of a linked file's name.
fn extension_of(name: &str) -> Option<&str> {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
}

/// Whether a linked file is one of the audited libraries.
///
/// The runtime's own object carries the Rust standard library and the locked
/// crates, whose notices the workspace already ships. The archives are the
/// Emscripten C library (MIT), the allocator (public domain), the compiler runtime
/// builtins (Apache-2.0 with the LLVM exception) and the Rust compiler builtins.
fn audited(name: &str) -> bool {
    name == "libc.a"
        || name == "libdlmalloc.a"
        || (name.starts_with("libclang_rt.builtins") && extension_of(name) == Some("a"))
        || (name.starts_with("libcompiler_builtins-") && extension_of(name) == Some("rlib"))
        || (name.starts_with("maestro_sandbox_runtime.") && extension_of(name) == Some("o"))
}
