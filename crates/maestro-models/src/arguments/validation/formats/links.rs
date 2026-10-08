//! Link and identifier grammars distinct from URL normalization.

use super::matches;

/// Check the ASCII reference grammar, including relative references.
pub(super) fn uri_reference(value: &str) -> bool {
    matches(
        r"^(?!.*[^\x00-\x7F])(?!.*\\)(?:(?:[a-z][a-z0-9+\-.]*:)?(?:\/\/[^\s[\]{}<>^`|]*)?|[^\s[\]{}<>^`|]*)(?:\?[^\s[\]{}<>^`|]*)?(?:#[^\s[\]{}<>^`|]*)?$",
        value,
        "i",
    )
}

/// Check literal text and template-expression syntax without expansion.
pub(super) fn uri_template(value: &str) -> bool {
    matches(
        r#"^(?:(?:[^\x00-\x20"'<>%\\^`{|}]|%[0-9a-f]{2})|\{[+#./;?&=,!@|]?(?:[a-z0-9_]|%[0-9a-f]{2})+(?::[1-9][0-9]{0,3}|\*)?(?:,(?:[a-z0-9_]|%[0-9a-f]{2})+(?::[1-9][0-9]{0,3}|\*)?)*\})*$"#,
        value,
        "i",
    )
}

/// Check supported network schemes, public addresses and domain/port grammar.
pub(super) fn url(value: &str) -> bool {
    matches(
        r"^(?:https?|ftp):\/\/(?:\S+(?::\S*)?@)?(?:(?!(?:10|127)(?:\.\d{1,3}){3})(?!(?:169\.254|192\.168)(?:\.\d{1,3}){2})(?!172\.(?:1[6-9]|2\d|3[0-1])(?:\.\d{1,3}){2})(?:[1-9]\d?|1\d\d|2[01]\d|22[0-3])(?:\.(?:1?\d{1,2}|2[0-4]\d|25[0-5])){2}(?:\.(?:[1-9]\d?|1\d\d|2[0-4]\d|25[0-4]))|(?:(?:[a-z0-9\u{00a1}-\u{ffff}]+-)*[a-z0-9\u{00a1}-\u{ffff}]+)(?:\.(?:[a-z0-9\u{00a1}-\u{ffff}]+-)*[a-z0-9\u{00a1}-\u{ffff}]+)*(?:\.(?:[a-z\u{00a1}-\u{ffff}]{2,})))(?::\d{2,5})?(?:\/[^\s]*)?$",
        value,
        "iu",
    )
}

/// Accept hyphenated identifiers with an optional case-insensitive URN prefix.
pub(super) fn uuid(value: &str) -> bool {
    matches(
        r"^(?:urn:uuid:)?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$",
        value,
        "i",
    )
}

/// Validate internationalized relative references without accepting malformed escapes.
pub(super) fn iri_reference(value: &str) -> bool {
    if value
        .chars()
        .any(|character| character == ' ' || character == '\\' || character.is_ascii_control())
        || matches(r"%(?![0-9a-f]{2})", value, "i")
    {
        return false;
    }
    if !value.contains(':') && matches(r"^[a-z][a-z0-9+\-.]*//", value, "i") {
        return false;
    }
    url::Url::parse("http://example.com").is_ok_and(|base| base.join(value).is_ok())
}

/// Validate absolute syntax without imposing URL normalization or network port limits.
pub(super) fn uri(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    if !matches(r"^[a-z][a-z0-9+.-]*$", scheme, "i") {
        return false;
    }
    let path = if let Some(authority_path) = rest.strip_prefix("//") {
        let endpoint = authority_path
            .find(['/', '?', '#'])
            .unwrap_or(authority_path.len());
        let Some(authority) = authority_path.get(..endpoint) else {
            return false;
        };
        if !authority_valid(authority) {
            return false;
        }
        let Some(path) = authority_path.get(endpoint..) else {
            return false;
        };
        path
    } else {
        rest
    };
    matches(
        r"^(?:[a-z0-9\-._~!$&'()*+,;=:@/?#]|%[0-9a-f]{2})*$",
        path,
        "i",
    )
}

/// Validate user information, host literals and optional numeric ports.
fn authority_valid(authority: &str) -> bool {
    let host_port = if let Some((userinfo, host)) = authority.split_once('@') {
        if !matches(
            r"^(?:[a-z0-9\-._~!$&'()*+,;=:]|%[0-9a-f]{2})*$",
            userinfo,
            "i",
        ) {
            return false;
        }
        host
    } else {
        authority
    };
    if let Some(bracketed) = host_port.strip_prefix('[') {
        let Some((literal, port)) = bracketed.split_once(']') else {
            return false;
        };
        let ip = literal.parse::<std::net::Ipv6Addr>().is_ok()
            || matches(r"^v[0-9a-f]+\.[a-z0-9\-._~!$&'()*+,;=:]+$", literal, "i");
        return ip
            && (port.is_empty()
                || port
                    .strip_prefix(':')
                    .is_some_and(|port| port.bytes().all(|byte| byte.is_ascii_digit())));
    }
    let (host, port) = host_port
        .split_once(':')
        .map_or((host_port, None), |(host, port)| (host, Some(port)));
    host.chars().all(|character| {
        !character.is_ascii()
            || character.is_ascii_alphanumeric()
            || "-._~!$&'()*+,;=".contains(character)
    }) && port.is_none_or(|port| port.bytes().all(|byte| byte.is_ascii_digit()))
}
