//! Source-derived document preparation observations.
use super::document::{insert_runtime, prepare};
use super::{PrepareHtmlOptions, runtime::COMPLETION_PROPERTY};
use crate::components::sandbox::runtime_bundle::BUNDLE;
use maestro_app::{BuiltinRuntime, Runtime};

/// Options for a loaded HTML artifact.
const ARTIFACT: PrepareHtmlOptions = PrepareHtmlOptions {
    is_html_artifact: true,
    is_standalone: false,
};

/// Prepare a document with the quoted identifier `"id"` and no globals.
fn document(code: &str, runtimes: &[Runtime], options: PrepareHtmlOptions) -> String {
    prepare(code, "id", &[], runtimes, options).unwrap()
}

/// The configuration block's text, restored as the parser sees it.
fn configuration(html: &str) -> serde_json::Value {
    let after = html.split("data-maestro-sandbox>").nth(1).unwrap();
    let text = after.split("</script>").next().unwrap();
    serde_json::from_str(text).unwrap()
}

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
    let html = document(source, &[], PrepareHtmlOptions::default());
    assert!(html.starts_with("<!DOCTYPE html>\n<html>\n<head>\n\t<style>"));
    assert_eq!(
        html.matches("<svg display=\"none\" data-maestro-sandbox-program>")
            .count(),
        1
    );
    assert!(html.contains("&lt;html>&lt;head>literal&lt;/head>&lt;/html>"));
    assert_eq!(
        configuration(&html)["program"]
            .as_str()
            .map(|key| key.starts_with("__maestro")),
        Some(true)
    );
    assert!(!PrepareHtmlOptions::default().is_html_artifact);
    assert!(!PrepareHtmlOptions::default().is_standalone);
}

#[test]
fn inline_carriers_preserve_text_and_encode_parser_hazards() {
    let provider = "function(id){return \"&<\\r>雪\\0\"} /* &<\r> */";
    let html = document("", &[Runtime::Content(provider.into())], ARTIFACT);
    assert!(
        html.contains("<template data-maestro-sandbox-provider data-maestro-capture=\"__maestro")
    );
    assert!(html.contains("&amp;&lt;\\r>雪\\0\"} /* &amp;&lt;&#13;> */"));
    let program = document("return 1; /* &<\r> */", &[], PrepareHtmlOptions::default());
    assert!(program.contains("/* &amp;&lt;&#13;> */"));
}

#[test]
fn standalone_documents_omit_online_helpers() {
    for artifact in [false, true] {
        for standalone in [false, true] {
            let html = document(
                "CODE",
                &[],
                PrepareHtmlOptions {
                    is_html_artifact: artifact,
                    is_standalone: standalone,
                },
            );
            let config = configuration(&html);
            assert_eq!(config["standalone"], standalone);
            assert!(html.contains("font-size: initial;"));
            assert_eq!(html.contains("data-maestro-sandbox-program"), !artifact);
            assert_eq!(
                config.get("program").is_some() || config.get("programError").is_some(),
                !artifact
            );
        }
    }
}

#[test]
fn runtime_insertion_orders_carriers_configuration_and_bundle() {
    let runtimes = [
        Runtime::Builtin(BuiltinRuntime::Console),
        Runtime::Content("function(id){window.first=id}".into()),
        Runtime::Empty,
        Runtime::Builtin(BuiltinRuntime::Artifacts),
        Runtime::Content("function(id){window.second=id}".into()),
        Runtime::Builtin(BuiltinRuntime::Attachments),
        Runtime::Builtin(BuiltinRuntime::FileDownload),
        Runtime::Builtin(BuiltinRuntime::Console),
    ];
    let globals = [
        ("\"2\"".to_owned(), "2".to_owned()),
        ("\"a\"".to_owned(), "{\"b\":null}".to_owned()),
    ];
    let html = prepare("", "quoted\"雪", &globals, &runtimes, ARTIFACT).unwrap();
    let positions: Vec<_> = [
        "window.first=id",
        "window.second=id",
        "<script type=\"application/json\" data-maestro-sandbox>",
        &format!("<script>{BUNDLE}</script>"),
    ]
    .map(|needle| html.find(needle).unwrap())
    .into();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        html.matches("<template data-maestro-sandbox-provider")
            .count(),
        2
    );
    let config = configuration(&html);
    assert_eq!(config["sandboxId"], "quoted\"雪");
    assert_eq!(
        config["globals"],
        serde_json::json!([[ "2", 2], ["a", {"b": null}]])
    );
    assert_eq!(
        config["runtime"],
        serde_json::json!([
            "console",
            "provider",
            "artifacts",
            "provider",
            "attachments",
            "fileDownload",
            "console"
        ])
    );
    assert_eq!(config["completion"], COMPLETION_PROPERTY);
}

#[test]
fn marker_looking_input_is_inert_text() {
    let markers = "__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__ </script><!--";
    let globals = [(format!("\"{markers}\""), format!("\"{markers}\""))];
    let html = prepare(
        &format!("return '{markers}';"),
        markers,
        &globals,
        &[Runtime::Content(format!(
            "function(){{return '{markers}'}}"
        ))],
        PrepareHtmlOptions::default(),
    )
    .unwrap();
    assert_eq!(
        html.matches("<template data-maestro-sandbox-provider")
            .count(),
        1
    );
    assert_eq!(html.matches("data-maestro-sandbox-program>").count(), 1);
    let config = configuration(&html);
    assert_eq!(config["sandboxId"], markers);
    assert_eq!(config["globals"][0][1], markers);
    let block = html
        .split_once("data-maestro-sandbox>")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    assert!(!block.contains('<'));
}

#[test]
fn refused_supplied_content_is_reported_not_emitted() {
    let html = document("return )(", &[], PrepareHtmlOptions::default());
    assert!(!html.contains("data-maestro-sandbox-program>"));
    assert!(
        configuration(&html)["programError"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
    assert!(configuration(&html).get("program").is_none());
    let refusal = prepare("", "id", &[], &[Runtime::Content("1; 2".into())], ARTIFACT);
    assert!(refusal.is_err());
}

mod execution;
