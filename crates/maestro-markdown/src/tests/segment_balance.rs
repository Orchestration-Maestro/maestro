//! Selection and snapshot lifetimes at the retained text boundary.
use crate::MappedText;

/// Conservative AVL height bound in logical bytes, including a leaf.
pub(super) fn depth_bound(length: usize) -> usize {
    2 * (length + 1).next_power_of_two().ilog2() as usize + 3
}

#[test]
fn selected_prefix_append_keeps_logarithmic_ownership() {
    let piece = MappedText::from("a!".to_owned());
    let mut text = MappedText::default();
    for n in 1..=100_000 {
        text.append(&piece);
        text = text.select(0..text.len() - 1);
        if n <= 64 || n % 1_000 == 0 {
            assert!(
                text.storage_depth() <= depth_bound(n),
                "n={n} depth={}",
                text.storage_depth()
            );
        }
    }
    assert_eq!(text.to_owned_string(), "a".repeat(100_000));
    drop(text);
}

/// Reproducible operation choices without another test dependency.
fn random(seed: &mut u64) -> usize {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    usize::try_from(*seed & u64::from(u32::MAX)).unwrap()
}

#[test]
fn random_text_windows_and_snapshots_keep_values_and_depth() {
    for seed in [1, 42, 0x1841, 0x1907] {
        random_text_sequence(seed);
    }
}

/// Compare each operation and retained snapshot with independently owned strings.
fn random_text_sequence(mut seed: u64) {
    let mut text = MappedText::default();
    let mut expected = String::new();
    let mut snapshots: Vec<(MappedText, String)> = Vec::new();
    for step in 0..100_000 {
        match random(&mut seed) % 5 {
            0 => {
                let piece = "abcd".get(..1 + random(&mut seed) % 4).unwrap();
                text.append(&MappedText::from(piece.to_owned()));
                expected.push_str(piece);
            }
            1 | 2 => {
                let start = random(&mut seed) % (expected.len() + 1);
                let end = start + random(&mut seed) % (expected.len() - start + 1);
                text = text.select(start..end);
                expected = expected[start..end].to_owned();
            }
            3 => {
                snapshots.push((text.clone(), expected.clone()));
                if snapshots.len() > 64 {
                    snapshots.remove(random(&mut seed) % snapshots.len());
                }
            }
            _ => {
                if !snapshots.is_empty() {
                    let (saved, value) = snapshots.remove(random(&mut seed) % snapshots.len());
                    assert_eq!(saved.to_owned_string(), value);
                    assert!(saved.storage_depth() <= depth_bound(value.len()));
                    drop(saved);
                }
            }
        }
        assert_eq!(text.to_owned_string(), expected, "step={step}");
        assert!(text.storage_depth() <= depth_bound(expected.len()));
    }
    for (saved, value) in snapshots {
        assert_eq!(saved.to_owned_string(), value);
        assert!(saved.storage_depth() <= depth_bound(value.len()));
    }
}

#[test]
fn repeated_quote_resumption_preserves_authored_and_body_values() {
    let n = 64;
    let continuation = "> - a\n    cont\n> \n> para\n> \n";
    let source = "> intro\nlazy\n".to_owned() + &continuation.repeat(n);
    let document = crate::parse(source.clone());
    assert_eq!(document.nodes[0].authored.to_owned_string(), source);
    let crate::Kind::Quote(_, body) = &document.nodes[0].kind else {
        panic!("quote");
    };
    let expected =
        "intro\nlazy\n- a\n    cont\n> \n> para\n> \n".to_owned() + &continuation.repeat(n - 1);
    let expected = expected.strip_suffix("> \n").unwrap().to_owned() + ">\n ";
    assert_eq!(body.to_owned_string(), expected);
    assert!(body.storage_depth() <= depth_bound(body.len()));
    drop(document);
}
