//! Ordered span mapping and candidate construction invariants.
use super::{Kind, Node, Run, Text, links};

#[test]
fn mapped_boundaries_and_candidates_remain_ordered() {
    for (decoded, authored) in [
        ("*foo@example.com tail", r"\*foo@example.com tail"),
        ("(https://example.com", r"\(https://example.com"),
        ("https://example.com/*a.", r"https://example.com/\*a."),
        ("\\foo@example.com", r"\\foo@example.com"),
        ("≂̸", "&NotEqualTilde;"),
        ("&", "&amp;"),
    ] {
        let run = Run::new(vec![Node {
            kind: Kind::Text(Text {
                decoded: decoded.to_owned(),
                authored: authored.to_owned(),
            }),
            range: 0..authored.len(),
        }]);
        let mut ends = (0, 0);
        for unit in &run.units {
            assert_eq!((unit.decoded.start, unit.authored.start), ends);
            ends = (unit.decoded.end, unit.authored.end);
        }
        assert_eq!(ends, (decoded.len(), authored.len()));
        for (text, display) in [(&run.decoded, true), (&run.authored, false)] {
            let map = if display {
                Run::authored_offset
            } else {
                Run::decoded_offset
            };
            let offsets: Vec<_> = text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain([text.len()])
                .map(|offset| map(&run, offset))
                .collect();
            assert!(offsets.windows(2).all(|pair| pair[0] <= pair[1]));
        }
        for (range, _) in links(&run) {
            assert!(range.start <= range.end, "{authored}: {range:?}");
        }
    }
}
