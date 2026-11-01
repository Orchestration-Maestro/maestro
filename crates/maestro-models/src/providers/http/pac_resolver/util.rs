//! Interpreter boundary conversions shared by host helpers.

use boa_engine::{Context, JsResult, JsValue};

/// Apply the interpreter's string conversion, including an omitted argument.
pub(super) fn string(args: &[JsValue], index: usize, context: &mut Context) -> JsResult<String> {
    args.get(index)
        .unwrap_or(&JsValue::undefined())
        .to_string(context)
        .map(|value| value.to_std_string_escaped())
}

/// Native runtime supplied only to the owned blocking interpreter worker.
pub(super) struct Host(pub(super) tokio::runtime::Handle);

/// All DNS-dependent helpers use the same asynchronous IPv4 lookup operation.
pub(super) fn dns_lookup(name: &str, context: &Context) -> Option<std::net::Ipv4Addr> {
    let host = context.get_data::<Host>()?;
    host.0.block_on(async {
        tokio::net::lookup_host((name, 0))
            .await
            .ok()?
            .find_map(|address| match address.ip() {
                std::net::IpAddr::V4(ip) => Some(ip),
                std::net::IpAddr::V6(_) => None,
            })
    })
}

pub(super) use super::super::decimal_prefix;

/// Remove only an exact trailing GMT marker without coercing other argument kinds.
pub(super) fn timezone(args: &[JsValue]) -> (&[JsValue], bool) {
    if args.last().is_some_and(|value| literal(value, "GMT")) {
        (&args[..args.len() - 1], true)
    } else {
        (args, false)
    }
}

/// Compare a script string without converting another value kind.
pub(super) fn literal(value: &JsValue, text: &str) -> bool {
    value.as_string().is_some_and(|value| value == text)
}

/// Observe one native instant as year, month, date, weekday, hour, minute and second.
pub(super) fn calendar(gmt: bool, context: &mut Context) -> JsResult<[i32; 7]> {
    let date = boa_engine::object::builtins::JsDate::new(context);
    let values = if gmt {
        [
            date.get_utc_full_year(context)?,
            date.get_utc_month(context)?,
            date.get_utc_date(context)?,
            date.get_utc_day(context)?,
            date.get_utc_hours(context)?,
            date.get_utc_minutes(context)?,
            date.get_utc_seconds(context)?,
        ]
    } else {
        [
            date.get_full_year(context)?,
            date.get_month(context)?,
            date.get_date(context)?,
            date.get_day(context)?,
            date.get_hours(context)?,
            date.get_minutes(context)?,
            date.get_seconds(context)?,
        ]
    };
    let mut result = [0; 7];
    for (slot, value) in result.iter_mut().zip(values) {
        *slot = value.to_i32(context)?;
    }
    result[1] += 1;
    Ok(result)
}
