//! Count escape-search and parser-delivery visits, independently of output.

use super::super::{Action, Handler, Stream};

/// Consumer that cannot affect the traversal measurement.
struct Discard;

impl Handler for Discard {
    fn vt(&mut self, _: Action<'_>) {}
}

/// Count actual adapter visits without counting produced actions.
fn visits(bytes: &[u8]) -> usize {
    let mut stream = Stream::new(false);
    stream.next_slice(bytes, &mut Discard);
    stream.input_visits
}

/// Accept bounded per-byte work whose doubling remains linear.
fn linear(n: usize, small: usize, large: usize) -> bool {
    small <= 2 * n && large <= 4 * n && large <= 2 * small
}

/// Deliberately inspect every suffix, recording each real input access.
fn rescanning(bytes: &[u8]) -> usize {
    let mut visits = 0;
    for start in 0..bytes.len() {
        for &byte in &bytes[start..] {
            std::hint::black_box(byte);
            visits += 1;
        }
    }
    visits
}

/// Inspect each supplied input byte once.
fn monotone(bytes: &[u8]) -> usize {
    let mut visits = 0;
    for &byte in bytes {
        std::hint::black_box(byte);
        visits += 1;
    }
    visits
}

#[test]
fn byte_dispatch_visits_input_monotonically() {
    let n = 1024;
    let controls = vec![b'x'; n];
    let doubled = vec![b'x'; 2 * n];
    assert!(linear(n, monotone(&controls), monotone(&doubled)));
    assert!(!linear(n, rescanning(&controls), rescanning(&doubled)));
    for pattern in [b"x".as_slice(), b"\xff", b"x\x1bD"] {
        let small = pattern.repeat(n);
        let large = pattern.repeat(2 * n);
        let counts = (visits(&small), visits(&large));
        assert!(
            linear(small.len(), counts.0, counts.1),
            "{pattern:?}: {counts:?}"
        );
        eprintln!(
            "pattern={pattern:?} bytes={} visits={counts:?}",
            small.len()
        );
    }
    for (prefix, fill, suffix) in [
        (b"\x1b[".as_slice(), b'1', b"C".as_slice()),
        (b"\x1b_", b'a', b"\x1b\\"),
    ] {
        let small = [prefix, &vec![fill; n], suffix].concat();
        let large = [prefix, &vec![fill; 2 * n], suffix].concat();
        let counts = (visits(&small), visits(&large));
        assert!(
            linear(small.len(), counts.0, counts.1),
            "{prefix:?}: {counts:?}"
        );
        eprintln!("prefix={prefix:?} bytes={} visits={counts:?}", small.len());
    }
}
