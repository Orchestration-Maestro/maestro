//! Recorded local date-time presentation through the public formatter.
#![cfg(test)]
#![cfg(feature = "date-time-format")]
use maestro_text::date_time_format::LocaleDateTimeFormat;

#[test]
fn locale_date_times_match_recorded_wall_fields() {
    let cases: Vec<[String; 4]> =
        serde_json::from_str(include_str!("fixtures/date_time_formatting.json")).unwrap();
    assert_eq!(cases.len(), 2301 * 4);
    for [locale, _, instant, expected] in cases {
        let fields: Vec<i32> = instant
            .split(['-', 'T', ':', 'Z'])
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        let formatter =
            LocaleDateTimeFormat::new(&|key| (key == "LANG").then(|| locale.clone())).unwrap();
        assert_eq!(
            formatter
                .format(
                    [fields[0], fields[1], fields[2]],
                    [
                        u8::try_from(fields[3]).unwrap(),
                        u8::try_from(fields[4]).unwrap(),
                        u8::try_from(fields[5]).unwrap()
                    ]
                )
                .unwrap(),
            expected,
            "{locale} {instant}"
        );
    }
}
