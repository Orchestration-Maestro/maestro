//! Listing-format selection and entry lookup.

use super::super::listing::{Format, entry, format, modified};

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

#[test]
fn ftp_listing_parsers_keep_accepted_names() {
    let dos = [
        "11-05-20  01:46PM                  4 my script.pac",
        "11-05-20  01:46PM       <DIR>          .",
        "11-05-20  01:46PM       <DIR>          ..",
    ];
    assert_eq!(
        entry(&dos.map(str::to_owned), Format::Dos, "my script.pac"),
        Some(dos[0])
    );
    assert_eq!(entry(&dos.map(str::to_owned), Format::Dos, "."), None);
    assert_eq!(entry(&dos.map(str::to_owned), Format::Dos, ".."), None);

    let unix = [
        (
            "lrwxrwxrwx 1 user group 4 Nov 5 13:46 link.pac -> target.pac",
            "link.pac",
        ),
        (
            "-rw-r--r-- 1 user group 4 2024-01-01 12:00 numeric.pac",
            "numeric.pac",
        ),
        (
            "-rw-r--r-- 1 user group 4 5 Nov 13:46 reversed.pac",
            "reversed.pac",
        ),
        (
            "-rw-r--r-- 1 user group 4 1月 5日 12:00 japanese.pac",
            "japanese.pac",
        ),
    ];
    for (line, name) in unix {
        let lines = [line.to_owned()];
        assert_eq!(format(line), Some(Format::Posix), "{line}");
        assert_eq!(entry(&lines, Format::Posix, name), Some(line), "{name}");
    }
    let linked = [unix[0].0.to_owned()];
    assert_eq!(entry(&linked, Format::Posix, "target.pac"), None);

    for line in [
        "type=cdir;modify=20240101120000; /",
        "type=pdir;type=file;modify=20240101120000; inside.pac",
        "type=file;modify=20240101120000; ../outside.pac",
    ] {
        assert_eq!(modified(line), None, "{line}");
    }
    let duplicate = "type=file;modify=20240101120000;modify=20240101120001; script.pac";
    assert_eq!(modified(duplicate), Some("20240101120001"));
    let empty = "type=file;modify=20240101120000;modify=; script.pac";
    assert_eq!(modified(empty), Some("20240101120000"));
    let spaced = "type=file;modify=20240101120000; my script.pac";
    assert_eq!(
        entry(&[spaced.to_owned()], Format::Mlsd, "my script.pac"),
        Some(spaced)
    );
}
