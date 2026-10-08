mod links;
mod names;
mod pointers;
mod temporal;

pub(super) fn check(format: &str, value: &str) -> bool {
    match format {
        "date" => temporal::date(value),
        "time" => temporal::time(value),
        "date-time" => temporal::date_time(value),
        "duration" => temporal::duration(value),
        "email" => names::email(value),
        "idn-email" => names::idn_email(value),
        "hostname" => names::hostname(value),
        "idn-hostname" => names::idn_hostname(value),
        "ipv4" => value.parse::<std::net::Ipv4Addr>().is_ok(),
        "ipv6" => value.parse::<std::net::Ipv6Addr>().is_ok(),
        "uri" => links::uri(value),
        "uri-reference" => links::uri_reference(value),
        "iri" => url::Url::parse(value).is_ok(),
        "iri-reference" => links::iri_reference(value),
        "uri-template" => links::uri_template(value),
        "url" => links::url(value),
        "uuid" => links::uuid(value),
        "regex" => !value.is_empty() && regress::Regex::new(value).is_ok(),
        "json-pointer" => pointers::json_pointer(value),
        "json-pointer-uri-fragment" => pointers::json_pointer_uri_fragment(value),
        "relative-json-pointer" => pointers::relative_json_pointer(value),
        _ => true,
    }
}

fn matches(pattern: &str, value: &str, flags: &str) -> bool {
    regress::Regex::with_flags(pattern, flags).is_ok_and(|regex| regex.find(value).is_some())
}
