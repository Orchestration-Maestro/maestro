//! Shared OAuth data and callback behavior.
use maestro_models::OAuthCredentials;
use serde_json::json;

#[test]
fn oauth_credentials_retain_extension_fields() {
    for (refresh, access, expires) in [
        ("r", "a", 0.0),
        ("", "", -2.5),
        ("刷新", "令牌", 0.125),
        ("r2", "a2", 1_700_000_000_123.0),
    ] {
        let value = json!({"refresh":refresh,"access":access,"expires":expires,"provider":{"items":[null,"雪",3]},"account":null});
        let credentials: OAuthCredentials = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            (
                &*credentials.refresh,
                &*credentials.access,
                credentials.expires
            ),
            (refresh, access, expires)
        );
        assert_eq!(serde_json::to_value(credentials).unwrap(), value);
    }
}

#[test]
fn oauth_prompt_records_keep_optional_presence() {
    use maestro_models::{OAuthAuthInfo, OAuthPrompt};
    for placeholder in [None, Some(""), Some(" 雪 ")] {
        for allow_empty in [None, Some(false), Some(true)] {
            let prompt = OAuthPrompt {
                message: " enter\n".into(),
                placeholder: placeholder.map(str::to_owned),
                allow_empty,
            };
            let mut expected = json!({"message":" enter\n"});
            if let Some(text) = placeholder {
                expected["placeholder"] = json!(text);
            }
            if let Some(flag) = allow_empty {
                expected["allowEmpty"] = json!(flag);
            }
            assert_eq!(serde_json::to_value(&prompt).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<OAuthPrompt>(expected).unwrap(),
                prompt
            );
        }
        let info = OAuthAuthInfo {
            url: " custom:雪 ".into(),
            instructions: placeholder.map(str::to_owned),
        };
        let mut expected = json!({"url":" custom:雪 "});
        if let Some(text) = placeholder {
            expected["instructions"] = json!(text);
        }
        assert_eq!(serde_json::to_value(&info).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<OAuthAuthInfo>(expected).unwrap(),
            info
        );
    }
}

#[test]
fn oauth_selection_records_preserve_order() {
    use maestro_models::{OAuthProvider, OAuthProviderId, OAuthSelectOption, OAuthSelectPrompt};
    let open_id: OAuthProviderId = "自由:custom".into();
    let empty_id: OAuthProvider = String::new();
    let options = vec![
        OAuthSelectOption {
            id: open_id.clone(),
            label: "第二".into(),
        },
        OAuthSelectOption {
            id: empty_id,
            label: "first".into(),
        },
        OAuthSelectOption {
            id: open_id,
            label: "重复".into(),
        },
    ];
    for choices in [vec![], options] {
        let prompt = OAuthSelectPrompt {
            message: " choose ".into(),
            options: choices,
        };
        let encoded = serde_json::to_value(&prompt).unwrap();
        let expected = if prompt.options.is_empty() {
            json!({"message":" choose ","options":[]})
        } else {
            json!({"message":" choose ","options":[{"id":"自由:custom","label":"第二"},{"id":"","label":"first"},{"id":"自由:custom","label":"重复"}]})
        };
        assert_eq!(encoded, expected);
        assert_eq!(
            serde_json::from_value::<OAuthSelectPrompt>(encoded).unwrap(),
            prompt
        );
    }
}

#[test]
fn pkce_uses_system_entropy() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest as _, Sha256};
    let key = maestro_models::generate_pkce().unwrap();
    for text in [&key.verifier, &key.challenge] {
        assert_eq!(text.len(), 43);
        assert!(
            text.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        );
        assert_eq!(URL_SAFE_NO_PAD.decode(text).unwrap().len(), 32);
    }
    assert_eq!(
        URL_SAFE_NO_PAD.decode(key.challenge).unwrap(),
        Sha256::digest(key.verifier.as_bytes()).as_slice()
    );
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Controlled oracle data.
struct SuccessCase {
    /// Supplied oracle message.
    message: String,
    /// Supplied oracle escaped message.
    escaped_message: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Controlled oracle data.
struct ErrorCase {
    /// Supplied oracle message.
    message: String,
    /// Supplied oracle details.
    details: Option<String>,
    /// Supplied oracle escaped message.
    escaped_message: String,
    /// Supplied oracle escaped details.
    escaped_details: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
/// Controlled oracle data.
struct Pages {
    /// Supplied oracle success.
    success: Vec<SuccessCase>,
    /// Supplied oracle error.
    error: Vec<ErrorCase>,
}

#[test]
fn oauth_pages_escape_caller_text() {
    use maestro_models::oauth_success_html;
    let pages: Pages = serde_json::from_str(include_str!("fixtures/oauth_pages.json")).unwrap();
    assert_eq!((pages.success.len(), pages.error.len()), (9, 63));
    for SuccessCase {
        message,
        escaped_message,
    } in pages.success
    {
        let html = oauth_success_html(&message).unwrap();
        assert!(html.contains(&format!("<p>{escaped_message}</p>")));
        assert!(html.contains("<title>Authentication successful</title>"));
        assert!(html.contains("<h1>Authentication successful</h1>"));
        assert!(!html.contains("<div class=\"details\">"));
        assert!(!html.contains("<script>"));
    }
    for case in pages.error {
        check_error_page(case).unwrap();
    }
}

#[test]
fn oauth_pages_keep_callback_layout() {
    for html in [
        maestro_models::oauth_success_html("Layout witness").unwrap(),
        maestro_models::oauth_error_html("Layout witness", Some("Details witness")).unwrap(),
    ] {
        let markers = [
            "<!doctype html>",
            "<html lang=\"en\">",
            "<meta charset=\"utf-8\" />",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />",
            "<style>",
            "<body>",
            "<main>",
            "<div class=\"logo\">",
            "<h1>",
            "<p>Layout witness</p>",
            "</main>",
            "</body>",
            "</html>",
        ];
        let offsets = markers.map(|marker| html.find(marker).unwrap());
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(html.matches("<p>").count(), 1);
        for declaration in [
            "box-sizing: border-box",
            "color-scheme: dark",
            "margin: 0;",
            "min-height: 100vh",
            "display: flex",
            "align-items: center",
            "justify-content: center",
            "padding: 24px",
            "text-align: center",
            "width: 100%",
            "max-width: 560px",
            "flex-direction: column",
            "width: 72px",
            "height: 72px",
            "display: block",
            "margin-bottom: 24px",
            "margin: 0 0 10px",
            "font-size: 28px",
            "line-height: 1.15",
            "font-weight: 650",
            "line-height: 1.7",
            "font-size: 15px",
            "margin-top: 16px",
            "font-size: 13px",
            "white-space: pre-wrap",
            "word-break: break-word",
            ".logo svg { width: 100%; height: 100%; }",
        ] {
            assert!(html.contains(declaration), "{declaration}");
        }
        assert!(!html.contains("<script"));
        assert!(!html.contains("@import"));
        assert!(!html.contains("<link"));
    }
}

/// Check each emitted text position for one failed callback query.
fn check_error_page(case: ErrorCase) -> Result<(), maestro_models::DiagnosticErrorInfo> {
    use maestro_models::oauth_error_html;
    let ErrorCase {
        message,
        details,
        escaped_message,
        escaped_details,
    } = case;
    let html = oauth_error_html(&message, details.as_deref())?;
    assert!(html.contains(&format!("<p>{escaped_message}</p>")));
    assert!(html.contains("<title>Authentication failed</title>"));
    assert!(html.contains("<h1>Authentication failed</h1>"));
    if let Some(text) = escaped_details {
        assert!(html.contains(&format!("<div class=\"details\">{text}</div>")));
    } else {
        assert!(!html.contains("<div class=\"details\">"));
    }
    assert!(!html.contains("<script>"));
    Ok(())
}
