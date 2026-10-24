//! Local or GMT time ranges with the documented argument profiles.

use boa_engine::{Context, JsResult, JsValue};

/// Interpret numeric arguments with decimal-prefix conversion.
pub(super) fn time_range(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let (args, gmt) = super::util::timezone(args);
    let now = super::util::calendar(gmt, context)?;
    let values: Option<Vec<_>> = args
        .iter()
        .map(|value| {
            value
                .to_string(context)
                .map(|text| super::util::decimal_prefix(&text.to_std_string_escaped()))
        })
        .collect::<JsResult<_>>()?;
    Ok(values
        .is_some_and(|values| matches(&values, [now[4], now[5], now[6]]))
        .into())
}

/// Compare one-hour, half-open hour, inclusive minute and inclusive second profiles.
fn matches(values: &[i32], now: [i32; 3]) -> bool {
    let seconds = |h: i32, m: i32, s: i32| i64::from(h) * 3600 + i64::from(m) * 60 + i64::from(s);
    match *values {
        [hour] => now[0] == hour,
        [start, end] => start <= now[0] && now[0] < end,
        [h1, m1, h2, m2] => {
            (seconds(h1, m1, 0)..=seconds(h2, m2, 59)).contains(&seconds(now[0], now[1], 0))
        }
        [h1, m1, s1, h2, m2, s2] => {
            (seconds(h1, m1, s1)..=seconds(h2, m2, s2)).contains(&seconds(now[0], now[1], now[2]))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn endpoints_keep_each_profile() {
        assert!(super::matches(&[12], [12, 59, 59]));
        assert!(!super::matches(&[12, 13], [13, 0, 0]));
        assert!(super::matches(&[12, 30, 13, 0], [13, 0, 59]));
        assert!(!super::matches(&[12, 30, 13, 0], [13, 1, 0]));
        assert!(super::matches(&[0, 0, 0, 0, 0, 30], [0, 0, 30]));
        assert!(!super::matches(&[0, 0, 0, 0, 0, 30], [0, 0, 31]));
        assert!(!super::matches(&[23, 1], [0, 0, 0]));
    }
}
