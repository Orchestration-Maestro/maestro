//! Short deterministic hashes over supplied UTF-16 code units.
/// Hash UTF-16 units with wrapping multiplications and unsigned base36 output.
pub fn short_hash(value: &[u16]) -> String {
    let mut h1 = 0xdeadbeef_u32;
    let mut h2 = 0x41c6ce57_u32;
    for ch in value {
        h1 = (h1 ^ u32::from(*ch)).wrapping_mul(2654435761);
        h2 = (h2 ^ u32::from(*ch)).wrapping_mul(1597334677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2246822507) ^ (h2 ^ (h2 >> 13)).wrapping_mul(3266489909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2246822507) ^ (h1 ^ (h1 >> 13)).wrapping_mul(3266489909);
    format!("{}{}", base36(h2), base36(h1))
}
fn base36(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut chars = vec![];
    while n > 0 {
        chars.push(char::from_digit(n % 36, 36).unwrap());
        n /= 36;
    }
    chars.into_iter().rev().collect()
}
