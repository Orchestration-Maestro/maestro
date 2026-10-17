//! Property-transition behavioural witnesses.
use super::{Transition, TransitionStyle};

/// The default utility-theme timing operands.
const STYLE: TransitionStyle = TransitionStyle {
    duration: 0.15,
    curve: [0.4, 0., 0.2, 1.],
};

#[test]
fn recorded_reversals_retain_current_values_and_shortening() {
    for (time, value, duration, next) in [
        (0.0375, 0.236_587, 0.035_488_084, 0.062_851_8),
        (0.075, 0.775_561, 0.116_334_197, 0.729_698),
        (0.1125, 0.959_368, 0.143_905_161, 0.924_780),
    ] {
        let mut property = Transition::new([0.]);
        property.sample(0., [1.], STYLE);
        let entering = property.sample(time, [1.], STYLE);
        assert!((entering.value[0] - value).abs() < 0.000_002);
        let reversed = property.sample(time, [0.], STYLE);
        assert_eq!(
            reversed.value.map(f32::to_bits),
            entering.value.map(f32::to_bits)
        );
        assert_eq!(
            property.sample(time, [0.], STYLE).value.map(f32::to_bits),
            reversed.value.map(f32::to_bits)
        );
        let frame = property.sample(time + 1. / 60., [0.], STYLE);
        assert!((frame.value[0] - next).abs() < 0.000_002);
        let midpoint = property.sample(time + duration / 2., [0.], STYLE);
        assert!(midpoint.active);
        let end = property.sample(time + duration + 0.000_001, [0.], STYLE);
        assert_eq!(end.value.map(f32::to_bits), [0_f32.to_bits()]);
        assert!(!end.active);
    }
}

#[test]
fn repeated_reversal_uses_reversing_adjustment_not_distance() {
    let mut property = Transition::new([0.]);
    property.sample(0., [1.], STYLE);
    property.sample(0.075, [0.], STYLE);
    let time = 0.075 + 0.116_334_196_668_009 / 2.;
    let third = property.sample(time, [1.], STYLE);
    assert!((third.value[0] - 0.174_066).abs() < 0.000_002);
    let middle = property.sample(time + 0.123_890_105_427_931 / 2., [1.], STYLE);
    assert!((middle.value[0] - 0.814_63).abs() < 0.000_01);
    let end = property.sample(time + 0.123_891, [1.], STYLE);
    assert_eq!(end.value.map(f32::to_bits), [1_f32.to_bits()]);
    assert!(!end.active);
}

#[test]
fn completed_exit_and_third_target_restart_the_full_curve() {
    let mut property = Transition::new([0., 10.]);
    property.sample(0., [1., 20.], STYLE);
    assert_eq!(
        property
            .sample(1., [1., 20.], STYLE)
            .value
            .map(f32::to_bits),
        [1_f32, 20.].map(f32::to_bits)
    );
    property.sample(1., [0., 10.], STYLE);
    let exit = property.sample(1.075, [0., 10.], STYLE);
    assert!((exit.value[0] - 0.224_439).abs() < 0.000_002);
    let changed = property.sample(1.075, [2., 30.], STYLE);
    assert_eq!(
        exit.value.map(f32::to_bits),
        changed.value.map(f32::to_bits)
    );
    let halfway = property.sample(1.15, [2., 30.], STYLE);
    assert!((halfway.value[0] - 1.601_495).abs() < 0.000_003);
    assert_eq!(
        property
            .sample(2., [2., 30.], STYLE)
            .value
            .map(f32::to_bits),
        [2_f32, 30.].map(f32::to_bits)
    );
}

#[test]
fn independent_properties_do_not_restart_each_other() {
    let mut opacity = Transition::new([0.7]);
    let mut color = Transition::new([0., 0., 0.]);
    opacity.sample(0., [1.], STYLE);
    color.sample(0., [255., 0., 0.], STYLE);
    color.sample(0.0375, [0., 255., 0.], STYLE);
    assert!((opacity.sample(0.075, [1.], STYLE).value[0] - 0.932_668).abs() < 0.000_002);
    let instant = TransitionStyle {
        duration: 0.,
        ..STYLE
    };
    let settled = color.sample(0.075, [1., 2., 3.], instant);
    assert_eq!(
        settled.value.map(f32::to_bits),
        [1_f32, 2., 3.].map(f32::to_bits)
    );
    assert!(!settled.active);
}
