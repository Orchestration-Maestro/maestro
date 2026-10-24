use super::*;
use std::path::PathBuf;

/// One source with `text` as its contents and no parsed syntax.
fn text(path: &str, text: &str) -> Source {
    Source {
        path: PathBuf::from(path),
        contents: text.into(),
        syntax: None,
    }
}

/// One member whose crate directory is `root` and whose only source is `source`.
fn single(root: &str, name: &str, source: Source) -> Member {
    Member {
        name: name.into(),
        directory: PathBuf::from(root),
        sources: vec![source],
    }
}

#[test]
fn macro_arguments_report_installation() {
    let contents =
        "fn build() { assert!(js_sys::Reflect::define_property(node, &name, &descriptor)); }";
    assert_eq!(installed(&text("src/components/x.rs", contents)), [1]);
}

#[test]
fn closure_and_impl_bodies_report_installation() {
    let contents = "impl A {\n    fn set(&self) {\n        let write = move || {\n            let _ = Reflect::define_property(self.element(), &name(), &descriptor());\n        };\n    }\n}";
    assert_eq!(installed(&text("src/components/x.rs", contents)), [4]);
}

#[test]
fn literals_and_documentation_do_not_install() {
    let contents = "/// `js_sys::Reflect::define_property` belongs to the owner.\nfn label() -> &'static str { \"Reflect::define_property\" }";
    assert!(installed(&text("src/components/x.rs", contents)).is_empty());
}

#[test]
fn the_owner_module_installs_descriptors() {
    let contents =
        "fn install() { let _ = js_sys::Reflect::define_property(element, &name, &descriptor); }";
    let owner = single(
        "/work/crates/maestro-web",
        "maestro-web",
        text("/work/crates/maestro-web/src/dom.rs", contents),
    );
    assert_eq!(check(&owner, &owner.sources[0]), Ok(()));
}

#[test]
fn another_module_reports_installation() {
    let contents =
        "fn install() { let _ = js_sys::Reflect::define_property(element, &name, &descriptor); }";
    let other = single(
        "/work/crates/maestro-web",
        "maestro-web",
        text("/work/crates/maestro-web/src/components/x/dom.rs", contents),
    );
    let message = check(&other, &other.sources[0]).unwrap_err();
    assert!(
        message.contains("src/components/x/dom.rs:1: only maestro-web::dom::install"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn test_sources_install_descriptors() {
    let contents =
        "fn install() { let _ = js_sys::Reflect::define_property(element, &name, &descriptor); }";
    let tests = single(
        "/work/crates/maestro-web",
        "maestro-web",
        text("/work/crates/maestro-web/src/dom/tests.rs", contents),
    );
    assert_eq!(check(&tests, &tests.sources[0]), Ok(()));
}
