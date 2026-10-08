use super::matches;

pub(super) fn json_pointer(value: &str) -> bool {
    matches(r"^(?:\/(?:[^~/]|~0|~1)*)*$", value, "")
}

pub(super) fn json_pointer_uri_fragment(value: &str) -> bool {
    matches(
        r"^#(?:\/(?:[a-z0-9_\-.!$&'()*+,;:=@]|%[0-9a-f]{2}|~0|~1)*)*$",
        value,
        "i",
    )
}

pub(super) fn relative_json_pointer(value: &str) -> bool {
    matches(
        r"^(?:0|[1-9][0-9]*)(?:#|(?:\/(?:[^~/]|~0|~1)*)*)$",
        value,
        "",
    )
}
