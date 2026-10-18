//! Packed-field comparisons distinguish styling from text.

use super::Mask;

#[test]
fn cell_masks_classify_runs() {
    type Style = Mask<{ 0xffff << 26 }>;
    let plain = [u64::from('A') << 2, u64::from('B') << 2, 0, 0];
    assert!(Style::matches(&plain));
    let styled = [plain[0] | (7 << 26), plain[1] | (7 << 26), 8 << 26, 7 << 26];
    assert!(!Style::matches(&styled));
    assert_eq!(Style::pattern(styled[0]), 7 << 26);
    assert_eq!(Style::strip(styled[0]), plain[0]);
    assert!(Style::eql(&styled[..2], 7 << 26));
    assert!(!Style::eql(&styled, 7 << 26));
    assert_eq!(Style::eql_prefix(&styled, 7 << 26), 2);
    assert_eq!(Style::eql_prefix(&[7 << 26; 4], 7 << 26), 4);
    assert!(!Style::eql_exact(&styled[..2], styled[0]));
    assert!(Style::eql_exact(&[styled[0]; 4], styled[0]));
    assert_ne!(Style::strip(styled[0] | (1 << 45)), plain[0]);
}
