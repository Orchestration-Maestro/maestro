//! SOCKS URL constructor behavior.

use super::SocksProxyAgent;

/// One independently recorded constructor case.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Non-secret case label.
    id: String,
    /// Authored URL.
    input: Input,
    /// Successful fields or rejection.
    expected: Expected,
}

/// Constructor input.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Selected URL.
    url: String,
}

/// Constructor outcome.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Expected {
    /// Accepted proxy fields.
    Accepted(Fields),
    /// Invalid input.
    Rejected(Rejection),
}

/// Accepted constructor fields.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    /// Local lookup policy.
    lookup: bool,
    /// Proxy host.
    host: String,
    /// Effective port.
    port: u16,
    /// Decoded username.
    user: String,
    /// Decoded password.
    password: String,
}

/// Rejection marker.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Rejection {
    /// Whether construction fails.
    error: bool,
}

#[test]
fn socks_urls_keep_scheme_dns_ports_and_credentials() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../../tests/fixtures/socks_urls.json"))
            .expect("constructor fixture");
    assert_eq!(cases.len(), 30);
    for case in cases {
        let parsed = url::Url::parse(&case.input.url)
            .map_err(std::io::Error::other)
            .and_then(|url| SocksProxyAgent::new(&url));
        match case.expected {
            Expected::Rejected(expected) => {
                assert!(expected.error);
                assert!(parsed.is_err(), "case {}", case.id);
            }
            Expected::Accepted(expected) => {
                let proxy = parsed.expect("accepted constructor");
                assert_eq!(proxy.lookup, expected.lookup, "case {}", case.id);
                assert_eq!(proxy.host, expected.host, "case {}", case.id);
                assert_eq!(proxy.port, expected.port, "case {}", case.id);
                assert!(proxy.user == expected.user, "username case {}", case.id);
                assert!(
                    proxy.password == expected.password,
                    "password case {}",
                    case.id
                );
            }
        }
    }
}
