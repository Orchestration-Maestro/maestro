//! Mounted numeric CSS-property transitions, sampled by the canvas frame clock.

/// Timing operands resolved by the caller's stylesheet.
#[derive(Clone, Copy)]
pub(crate) struct TransitionStyle {
    /// Specified duration in seconds, before reversal shortening.
    pub(crate) duration: f64,
    /// Cubic-bezier control points in x/y order.
    pub(crate) curve: [f32; 4],
}
/// One sampled property and its repaint requirement.
pub(crate) struct Sample<const N: usize> {
    /// Current unquantized components.
    pub(crate) value: [f32; N],
    /// Whether the property needs another frame.
    pub(crate) active: bool,
}
/// One property's target and optional running transition.
pub(crate) struct Transition<const N: usize> {
    /// Resolved destination components.
    target: [f32; N],
    /// Interpolation and reversing metadata, absent at rest.
    run: Option<Run<N>>,
}
/// Captured operands of a running transition.
struct Run<const N: usize> {
    /// Current-value starting components.
    start: [f32; N],
    /// Frame-clock origin in seconds.
    time: f64,
    /// Effective duration after reversal shortening.
    duration: f64,
    /// Destination whose return qualifies as a reversal.
    reversing_start: [f32; N],
    /// Accumulated reversing shortening factor.
    shortening: f32,
    /// Timing curve captured when this run began.
    curve: [f32; 4],
}
impl<const N: usize> Transition<N> {
    /// Mount a property at its resolved style without an initial transition.
    pub(crate) fn new(initial: [f32; N]) -> Self {
        Self {
            target: initial,
            run: None,
        }
    }
    /// Sample the old run before retargeting from its current value.
    pub(crate) fn sample(
        &mut self,
        now: f64,
        target: [f32; N],
        style: TransitionStyle,
    ) -> Sample<N> {
        let mut value = self.target;
        let mut eased = 1.;
        if let Some(run) = &self.run {
            let progress = ((now - run.time) / run.duration).clamp(0., 1.);
            eased =
                super::transition_easing(super::attachment_content::points(progress), run.curve);
            value = std::array::from_fn(|i| egui::lerp(run.start[i]..=self.target[i], eased));
            if progress >= 1. {
                value = self.target;
                self.run = None;
            }
        }
        if !same_value(&target, &self.target) {
            let (reversing_start, shortening) = self
                .run
                .as_ref()
                .filter(|run| same_value(&target, &run.reversing_start))
                .map_or((value, 1.), |run| {
                    (
                        self.target,
                        (eased * run.shortening + 1. - run.shortening).clamp(0., 1.),
                    )
                });
            self.target = target;
            let duration = style.duration * f64::from(shortening);
            self.run = if same_value(&target, &value) || duration <= 0. {
                value = target;
                None
            } else {
                Some(Run {
                    start: value,
                    time: now,
                    duration,
                    reversing_start,
                    shortening,
                    curve: style.curve,
                })
            };
        }
        Sample {
            value,
            active: self.run.is_some(),
        }
    }
}

/// Compare resolved numeric endpoints exactly, rather than using a progress tolerance.
fn same_value<const N: usize>(left: &[f32; N], right: &[f32; N]) -> bool {
    left.iter()
        .zip(right)
        .all(|(a, b)| a.partial_cmp(b) == Some(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests;
