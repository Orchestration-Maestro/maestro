//! Source-derived document preparation observations.
use super::document::insert_runtime;

#[test]
fn html_runtime_uses_first_matching_insertion_point() {
    for (input, expected) in [
        ("雪<HeAd >x</HeAd>", "雪<HeAd >RUNTIME x</HeAd>"),
        ("<HTML lang='en'>x", "<HTML lang='en'>RUNTIME x"),
        ("fragment", "RUNTIME fragment"),
        ("", "RUNTIME "),
        ("<header>x", "<header>RUNTIME x"),
        ("<head>a<head>b", "<head>RUNTIME a<head>b"),
        ("<head", "RUNTIME <head"),
    ] {
        assert_eq!(insert_runtime(input, "RUNTIME "), expected);
    }
}

#[test]
fn repl_wrapper_treats_html_literals_as_code() {
    let source = "return '<html><head>literal</head></html>';";
    let html = super::document::wrap(source, "RUNTIME");
    assert!(html.starts_with("<!DOCTYPE html>\n<html>\n<head>\n\tRUNTIME"));
    assert!(html.contains(source));
    assert!(html.contains("<script type=\"module\">"));
    assert!(html.contains("await window.complete(null, returnValue)"));
    assert!(html.contains("await window.complete({"));
    assert!(super::document::wrap("", "RUNTIME").contains("const userCodeFunc"));
    assert!(!super::PrepareHtmlOptions::default().is_html_artifact);
    assert!(!super::PrepareHtmlOptions::default().is_standalone);
}

#[test]
fn script_escaping_preserves_literal_terminators() {
    assert_eq!(
        super::document::escape_script("雪'</ScRiPt>\\x </scripted>"),
        "雪'<\\/ScRiPt>\\x <\\/scripted>"
    );
    let html = super::document::wrap("return '</SCRIPT>';", "RUNTIME");
    assert!(html.contains("return '<\\/SCRIPT>';"));
    assert_eq!(html.matches("</script>").count(), 1);
}

#[test]
fn standalone_documents_omit_online_helpers() {
    for artifact in [false, true] {
        for standalone in [false, true] {
            let runtime = super::runtime::assemble("id", "", &[], standalone).unwrap();
            let html = super::document::prepare(
                "CODE",
                &runtime,
                super::PrepareHtmlOptions {
                    is_html_artifact: artifact,
                    is_standalone: standalone,
                },
            );
            assert_eq!(html.contains("window.sendRuntimeMessage"), !standalone);
            assert_eq!(
                html.contains("document.addEventListener('click'"),
                !standalone
            );
            assert!(html.contains("font-size: initial;"));
            assert_eq!(html.contains("<script type=\"module\">"), !artifact);
            assert!(!html.contains("window.console.log ="));
        }
    }
}

#[test]
fn generated_scripts_preserve_provider_order() {
    let functions = vec![
        "function(id){window.first=id}".into(),
        "function(id){window.second=id}".into(),
    ];
    let html = super::runtime::assemble(
        "quoted\"雪",
        "window[\"2\"] = 2;\nwindow[\"a\"] = 3;",
        &functions,
        false,
    )
    .unwrap();
    let positions: Vec<_> = [
        "window.sandboxId =",
        "window[\"2\"]",
        "window[\"a\"]",
        "window.sendRuntimeMessage",
        "window.first",
        "window.second",
        "document.addEventListener('click'",
    ]
    .map(|needle| html.find(needle).unwrap())
    .into();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(html.contains("})(\"quoted\\\"雪\");"));
}
