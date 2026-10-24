//! File and launch effects in the chat callers, delegated or performed in place.

use super::check;
use crate::source::{Member, Source, visit_token_identifiers};
use std::path::PathBuf;

/// The chat member with one source at `relative`.
fn chat(relative: &str, contents: &str) -> Member {
    Member {
        name: "maestro-chat".into(),
        directory: PathBuf::from("/chat"),
        sources: vec![Source {
            path: PathBuf::from("/chat").join(relative),
            contents: contents.into(),
            syntax: syn::parse_file(contents).ok(),
        }],
    }
}

/// The extension editor's own creation, write and launch before it delegated them.
const IN_PLACE: &[&str] = &[
    "use std::fs::{self, OpenOptions};",
    "fn open() { let mut options = OpenOptions::new(); options.write(true); }",
    "fn fill(file: &mut F) { file.write_all(b\"text\").unwrap(); }",
    "fn launch(c: C) { c.spawn_sync(maestro_app::SpawnSyncOptions::Inherit); }",
    "fn shell(c: &str) { maestro_app::windows_shell_launch(c, None); }",
    "use std::fs::File as Handle;",
    "fn bypass() { let _ = edit_external_file(a, b, c, d); std::fs::write(p, b); }",
    "fn bypass() { drop(std::fs::write(p, b)); }",
    "fn bypass() { inner!(std::fs::write(p, b)); }",
    "fn bypass() { let write = || std::fs::write(p, b); }",
    "impl Mode { fn bypass(&self) { let _: () = drop(std::fs::write(p, b)); } }",
];

#[test]
fn editor_effect_guard_rejects_native_bypasses() {
    for caller in [
        "src/components/extension_editor.rs",
        "src/interactive_mode/actions.rs",
    ] {
        for body in IN_PLACE {
            let error = check(&[chat(caller, body)]).unwrap_err();
            assert!(error.starts_with(&format!("/chat/{caller}:1: `")), "{body}");
        }
        for body in [
            "fn edit(f: &ExternalEditorFile) { edit_external_file(ui, editor, f) }",
            "fn paste(p: &Path) { write_temporary_file(p, b\"png\").is_ok(); }",
            "#[cfg(test)] mod tests { fn f() { std::fs::write(p, b).unwrap(); } }",
        ] {
            assert_eq!(check(&[chat(caller, body)]), Ok(()), "{body}");
        }
    }
    for owner in ["src/external_editor.rs", "src/temporary_file.rs"] {
        assert_eq!(check(&[chat(owner, IN_PLACE[1])]), Ok(()));
    }
}

#[test]
fn editor_effect_guard_walk_is_linear() {
    for repeats in [32, 64, 128] {
        let tokens: proc_macro2::TokenStream = "m!({ std::fs::write([p], (b)); })"
            .repeat(repeats)
            .parse()
            .unwrap();
        let mut visits = 0;
        visit_token_identifiers(&tokens, &mut |_| visits += 1);
        assert_eq!(visits, 6 * repeats);
    }
}
