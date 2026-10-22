//! Listing-format selection and entry lookup.

use super::super::listing::{Format, entry, format};

#[test]
fn ftp_listing_format_follows_the_last_line() {
    for (line, expected) in [
        (
            "-rw-r--r-- 1 user group 4 Nov 5 13:46 script.pac",
            Format::Posix,
        ),
        (
            "11-05-20  01:46PM                  4 script.pac",
            Format::Dos,
        ),
        (
            "type=file;size=4;modify=20240101120000; script.pac",
            Format::Mlsd,
        ),
        (" script.pac", Format::Mlsd),
    ] {
        assert_eq!(format(line), Some(expected), "{line}");
    }
    assert_eq!(format(""), None);
    assert_eq!(format("total 4"), None);
    assert_eq!(format("readable nonsense"), None);
}

#[test]
fn ftp_listing_entries_are_matched_by_name() {
    let lines: Vec<String> = [
        "type=dir;modify=20240101120000; /",
        "type=file;size=4;modify=20240101120000; other.pac",
        "type=file;size=4;modify=20240101120000; script.pac",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(
        entry(&lines, Format::Mlsd, "script.pac"),
        Some("type=file;size=4;modify=20240101120000; script.pac")
    );
    assert_eq!(entry(&lines, Format::Mlsd, "missing.pac"), None);
    assert_eq!(entry(&[], Format::Mlsd, "script.pac"), None);
    let text: Vec<String> = [
        "-rw-r--r-- 1 user group 4 Nov 5 13:46 script.pac",
        "garbage",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(
        entry(&text, Format::Posix, "script.pac"),
        Some("-rw-r--r-- 1 user group 4 Nov 5 13:46 script.pac")
    );
}
