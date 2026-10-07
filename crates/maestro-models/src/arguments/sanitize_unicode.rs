//! Filtering of unpaired UTF-16 units.

/// Remove unpaired surrogate units, retaining valid pairs and all other units.
///
/// ```
/// use maestro_models::sanitize_surrogates;
/// assert_eq!(sanitize_surrogates(&[0xd800, 0xd83d, 0xde48]), "🙈");
/// assert_eq!(sanitize_surrogates(&"Hello 🌍".encode_utf16().collect::<Vec<_>>()), "Hello 🌍");
/// ```
pub fn sanitize_surrogates(text: &[u16]) -> String {
    let retained: Vec<u16> = text
        .iter()
        .enumerate()
        .filter_map(|(i, &unit)| {
            let valid = match unit {
                0xd800..=0xdbff => text
                    .get(i + 1)
                    .is_some_and(|u| (0xdc00..=0xdfff).contains(u)),
                0xdc00..=0xdfff => i > 0 && (0xd800..=0xdbff).contains(&text[i - 1]),
                _ => true,
            };
            valid.then_some(unit)
        })
        .collect();
    String::from_utf16(&retained).unwrap()
}
