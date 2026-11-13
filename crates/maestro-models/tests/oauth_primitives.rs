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
