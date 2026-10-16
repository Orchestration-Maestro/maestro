//! Source-derived document preparation observations.
use super::document::insert_runtime;
/// Read parser-restored program text from the carrier envelopes.
fn programs(html: &str) -> String {
    html.split("<svg")
        .skip(1)
        .map(|part| {
            part.split_once("<script")
                .unwrap()
                .1
                .split_once('>')
                .unwrap()
                .1
                .split_once("</script>")
                .unwrap()
                .0
                .replace("&#13;", "\r")
                .replace("&lt;", "<")
                .replace("&amp;", "&")
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    let html = super::document::wrap(source, "RUNTIME");
    assert!(html.starts_with("<!DOCTYPE html>\n<html>\n<head>\n\tRUNTIME"));
    let program = programs(&html);
    assert!(program.contains(source));
    assert!(html.contains("<svg display=\"none\"><script type=\"module\">"));
    assert!(program.contains("await window.complete(null, returnValue)"));
    assert!(program.contains("await window.complete({"));
    assert!(programs(&super::document::wrap("", "RUNTIME")).contains("const userCodeFunc"));
    assert!(!super::PrepareHtmlOptions::default().is_html_artifact);
    assert!(!super::PrepareHtmlOptions::default().is_standalone);
}

#[test]
fn inline_carriers_preserve_text_and_exact_boot_scripts() {
    let source = "&<\r>\"\0雪";
    let encoded = "&amp;&lt;&#13;>\"\0雪";
    let runtime = super::runtime::assemble("id", source, &[], true).unwrap();
    assert!(runtime.contains(&format!("<template><svg><script>window.sandboxId = \"id\";\n{encoded}\n\n</script></svg></template><script>{{const t=document.currentScript.previousElementSibling,s=t.content.firstChild.firstChild;t.replaceWith(s);s.remove()}}</script>")));
    let html = super::document::wrap(source, "RUNTIME");
    let program = include_str!("repl.js")
        .replacen("__USER_CODE__", source, 1)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('\r', "&#13;");
    assert_eq!(
        html,
        format!(
            "<!DOCTYPE html>\n<html>\n<head>\n\tRUNTIME\n</head>\n<body>\n\t<svg display=\"none\"><script type=\"module\">{program}</script></svg><script>document.currentScript.previousElementSibling.remove()</script>\n</body>\n</html>"
        )
    );
    assert!(programs(&html).contains(source));
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
            let program = programs(&html);
            assert_eq!(program.contains("window.sendRuntimeMessage"), !standalone);
            assert_eq!(
                program.contains("document.addEventListener('click'"),
                !standalone
            );
            assert!(html.contains("font-size: initial;"));
            assert_eq!(
                html.contains("<svg display=\"none\"><script type=\"module\">"),
                !artifact
            );
            assert!(!program.contains("window.console.log ="));
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
    let program = programs(&html);
    let positions: Vec<_> = [
        "window.sandboxId =",
        "window[\"2\"]",
        "window[\"a\"]",
        "window.sendRuntimeMessage",
        "window.first",
        "window.second",
        "document.addEventListener('click'",
    ]
    .map(|needle| program.find(needle).unwrap())
    .into();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(program.contains("})(\"quoted\\\"雪\");"));
}
