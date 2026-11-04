//! Text decoding against the parse round trip it skips.

use super::xml_parser::{decode_text, parse_xml_string};

/// Decode through the XML parse the fast path bypasses.
fn round_trip(text: &str) -> String {
    let fragment = format!("<t>{}</t>", text.replace('\r', "&#13;"));
    parse_xml_string(&fragment, false)
        .map_or_else(|_| text.to_owned(), |xml| xml.root().text_content())
}

/// Runs whose decoding differs from their literal bytes.
const PIECES: [&str; 34] = [
    "",
    "plain",
    " ",
    "\n",
    "\t",
    "\r",
    "\r\n",
    "\u{85}",
    "\u{2028}",
    "&",
    "&amp;",
    "&lt;",
    "&gt;",
    "&quot;",
    "&apos;",
    "&#13;",
    "&#x41;",
    "&#65;",
    "&#38;",
    "&bogus;",
    "&#0;",
    "&amp",
    "<",
    ">",
    "a<b",
    "<b>x</b>",
    "]]>",
    "\u{0}",
    "\u{1}",
    "\u{b}",
    "\u{fffd}",
    "é",
    "日本語",
    "𝟘",
];

#[test]
fn text_decoding_matches_the_parse_round_trip() {
    let mut diverged = Vec::new();
    for left in PIECES {
        for right in PIECES {
            let run = format!("{left}{right}");
            let decoded = decode_text(&run);
            let parsed = round_trip(&run);
            if decoded != parsed {
                diverged.push(format!("{run:?}: decoded {decoded:?} parsed {parsed:?}"));
            }
        }
    }
    assert!(diverged.is_empty(), "{diverged:#?}");
}
