//! Inclusive weekday intervals that may cross the end of the week.

use boa_engine::{Context, JsResult, JsValue};

/// Select GMT from either optional position and preserve exact weekday strings.
pub(super) fn weekday_range(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let second_gmt = args
        .get(1)
        .is_some_and(|value| super::util::literal(value, "GMT"));
    let third_gmt = args
        .get(2)
        .is_some_and(|value| super::util::literal(value, "GMT"));
    let now = super::util::calendar(second_gmt || third_gmt, context)?;
    let start = args.first().and_then(weekday).unwrap_or(-1);
    let end = if second_gmt && !third_gmt {
        None
    } else {
        args.get(1).and_then(weekday)
    };
    Ok(matches(start, end, now[3]).into())
}

/// Read a weekday only from an exact authored string.
fn weekday(value: &JsValue) -> Option<i32> {
    let text = value.as_string()?;
    ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"]
        .iter()
        .zip(0..)
        .find_map(|(name, index)| (text == *name).then_some(index))
}

/// An omitted or invalid end selects only the first weekday.
fn matches(start: i32, end: Option<i32>, today: i32) -> bool {
    match end {
        None => today == start,
        Some(end) if start <= end => (start..=end).contains(&today),
        Some(end) => today >= start || today <= end,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_ordered_pairs_keep_inclusive_wrap() {
        for (start, end) in (0..7).flat_map(|start| (0..7).map(move |end| (start, end))) {
            let mut members = vec![start];
            let mut next = start;
            while next != end {
                next = (next + 1) % 7;
                members.push(next);
            }
            for today in 0..7 {
                assert_eq!(
                    super::matches(start, Some(end), today),
                    members.contains(&today)
                );
            }
            assert!(super::matches(start, None, start));
            assert!(!super::matches(start, None, (start + 1) % 7));
        }
    }
}
