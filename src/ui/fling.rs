//! Wheel fling modelled on Android `OverScroller`, which KISS' list rides.
//!
//! egui's `ScrollArea` applies each wheel line as a fixed delta (`line_scroll_speed`,
//! default 40) and, for touch, *replaces* kinetic velocity on pointer-up before
//! damping it at 1000 px/s². A second tick does not add to the first, so the
//! list never accelerates and feels capped (`#35`).
//!
//! These helpers are pure so the behaviour can be tested without a window.

/// Android `ViewConfiguration` scaled maximum fling is on the order of this.
pub const FLING_MAX: f32 = 9_000.0;
/// Gentler than egui's 1000 px/s², so a flick covers more than a couple of rows.
pub const FLING_FRICTION: f32 = 380.0;
pub const FLING_STOP: f32 = 40.0;
/// One point of wheel delta becomes this many px/s of fling velocity.
pub const WHEEL_VELOCITY_SCALE: f32 = 4.0;

/// Same direction adds. Opposite direction replaces. The sum is capped.
pub fn absorb_fling(residual: f32, gesture: f32) -> f32 {
    let combined = if residual.abs() < FLING_STOP || residual * gesture > 0.0 {
        residual + gesture
    } else {
        gesture
    };
    combined.clamp(-FLING_MAX, FLING_MAX)
}

/// `(new_velocity, offset_delta)`. Positive velocity increases the scroll offset.
pub fn step_fling(velocity: f32, dt: f32) -> (f32, f32) {
    let dt = dt.clamp(0.0, 0.05);
    if velocity.abs() < FLING_STOP || dt == 0.0 {
        return (0.0, 0.0);
    }
    let friction = FLING_FRICTION * dt;
    let next = if friction >= velocity.abs() {
        0.0
    } else {
        velocity - friction * velocity.signum()
    };
    (next, velocity * dt)
}

/// Positive egui wheel delta scrolls toward the top, so the offset velocity is negative.
pub fn wheel_gesture(scroll_delta_y: f32) -> f32 {
    -scroll_delta_y * WHEEL_VELOCITY_SCALE
}

/// Which rows a recycling list would bind. KISS `ListView.getView` only
/// builds `getChildCount()` rows; this is the same window, used so a test
/// can lock "a thousand apps do not mean a thousand widgets".
pub fn visible_row_range(
    offset: f32,
    view_height: f32,
    pitch: f32,
    total: usize,
) -> std::ops::Range<usize> {
    if total == 0 || pitch <= 0.0 {
        return 0..0;
    }
    let first = (offset.max(0.0) / pitch).floor() as usize;
    let window = (view_height / pitch).ceil() as usize + 2;
    let start = first.min(total);
    let end = (start + window).min(total);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_wheel_ticks_accelerate_until_the_cap() {
        let once = absorb_fling(0.0, wheel_gesture(-80.0));
        let twice = absorb_fling(once, wheel_gesture(-80.0));
        assert!(twice > once);
        assert!(twice > 0.0);
        assert_eq!(absorb_fling(8_000.0, 5_000.0), FLING_MAX);
        assert_eq!(absorb_fling(1_200.0, -300.0), -300.0);
    }

    #[test]
    fn fling_moves_with_velocity_then_stops() {
        let (next, delta) = step_fling(400.0, 0.016);
        assert!(delta > 0.0);
        assert!(next < 400.0);
        assert_eq!(step_fling(10.0, 0.016), (0.0, 0.0));
    }

    #[test]
    fn a_long_list_only_binds_the_visible_window() {
        let range = visible_row_range(2_000.0, 600.0, 72.0, 1_000);
        assert!(range.len() < 20, "bound {} rows", range.len());
        assert!(range.start > 10);
        assert!(range.end <= 1_000);
        assert_eq!(visible_row_range(0.0, 600.0, 72.0, 0), 0..0);
    }
}
