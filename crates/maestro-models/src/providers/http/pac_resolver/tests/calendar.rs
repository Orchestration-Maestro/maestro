//! Controlled native clock projections exercise registered helper timezone branches.

use boa_engine::context::{HostHooks, time::FixedClock};
use boa_engine::{Context, Source};
use std::rc::Rc;

/// A local calendar five hours behind UTC, without changing process environment.
struct Offset;

impl HostHooks for Offset {
    fn local_timezone_offset_seconds(&self, _: i64) -> i32 {
        -5 * 3600
    }
}

#[test]
fn calendar_helpers_distinguish_local_and_gmt_at_a_date_transition() {
    let clock = Rc::new(FixedClock::from_millis(1_709_253_000_000));
    let mut context = Context::builder()
        .clock(clock)
        .host_hooks(Rc::new(Offset))
        .build()
        .unwrap();
    super::super::initialize(&mut context, "function FindProxyForURL(){return 'DIRECT'}").unwrap();
    let result = context.eval(Source::from_bytes(
        "JSON.stringify([timeRange(19),timeRange(0),timeRange(0,'GMT'),timeRange(19,'GMT'),weekdayRange('THU'),weekdayRange('FRI'),weekdayRange('FRI','GMT'),weekdayRange('FRI','FRI','GMT'),weekdayRange('THU','GMT'),dateRange(29,'FEB',2024),dateRange(1,'MAR',2024),dateRange(1,'MAR',2024,'GMT'),dateRange(29,'FEB',2024,'GMT')])"
    )).unwrap();
    assert_eq!(
        result.as_string().unwrap().to_std_string_escaped(),
        "[true,false,true,false,true,false,true,true,false,true,false,true,false]"
    );
}
