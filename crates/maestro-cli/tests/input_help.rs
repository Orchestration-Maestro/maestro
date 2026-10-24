//! Complete help rendering observed through the public writer.

use crate::support;
use maestro_app::{ExtensionFlag, FlagType};
use maestro_cli::input::args::print_help;
use std::sync::{Arc, RwLock};

/// Build one retained registration from a recorded flag description.
fn flag(value: &serde_json::Value) -> Arc<RwLock<ExtensionFlag>> {
    let kind = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap();
    Arc::new(RwLock::new(ExtensionFlag {
        name: value["name"].as_str().unwrap().to_owned(),
        description: value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        r#type: if kind == "string" {
            FlagType::String
        } else {
            FlagType::Boolean
        },
        default: None,
        extension_path: value["extensionPath"].as_str().unwrap().to_owned(),
    }))
}

/// Render one case's help document with its recorded registrations and style.
fn render(case: &support::Recorded) -> String {
    let flags: Vec<Arc<RwLock<ExtensionFlag>>> = case.input["flags"]
        .as_array()
        .expect("declared flags")
        .iter()
        .map(flag)
        .collect();
    let styles_enabled = case.input["styles_enabled"]
        .as_bool()
        .expect("recorded style choice");
    let mut output = Vec::new();
    print_help(&flags, &mut output, styles_enabled).expect("writeable help output");
    String::from_utf8(output).expect("valid utf-8 help")
}

#[test]
fn help_retains_every_line_and_extension_label() {
    for case in support::operation("help", "help") {
        assert_eq!(
            render(&case),
            case.expected["stdout"]
                .as_str()
                .expect("recorded help text"),
            "{}",
            case.case
        );
        assert_eq!(case.expected["stderr"].as_str(), Some(""), "{}", case.case);
    }
}

#[test]
fn help_describes_configured_or_available_provider() {
    let case = support::case_by_title("help", "help-140");
    let document = render(&case);
    assert!(
        document.contains("Provider name (default: configured or available provider)"),
        "{document}"
    );
    assert!(!document.contains("(default: google)"));
    for provider in [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "DEEPSEEK_API_KEY",
        "GROQ_API_KEY",
    ] {
        assert!(document.contains(provider), "{provider}");
    }
}
