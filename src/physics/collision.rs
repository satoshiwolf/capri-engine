use crate::core::config::{
    ANIME_FRAME_SECS, LEDGE_SUPPORT_MARGIN_RATIO, SPRITE_HEIGHT, SPRITE_WIDTH,
    SQUASH_COMPRESSION_RANGE, SQUASH_FIRST_RECOVERY_SCALE, SQUASH_FIRST_SETTLE_FRACTION,
    SQUASH_HOLD_FRAME_BASE, SQUASH_HOLD_FRAME_SCALE, SQUASH_MIN_COMPRESSION,
    SQUASH_OVERSHOOT_SCALE, SQUASH_RECOVERY_SECS, SQUASH_SECOND_SETTLE_FRACTION,
    SQUASH_TRANSVERSE_RATIO,
};
use glam::Vec2;

pub const FLOOR_SURFACE_HWND: isize = 0;

#[derive(Debug, Clone, PartialEq)]
pub struct LedgeSurface {
    pub hwnd: isize,
    pub left: f32,
    pub right: f32,
    pub y: f32,
    pub window_title: Option<String>,
}

impl LedgeSurface {
    pub fn contains_x(&self, x: f32, margin: f32) -> bool {
        x >= self.left - margin && x <= self.right + margin
    }

    /// Returns the range in which the sprite center can rest without crossing either edge.
    pub fn standing_limits(&self, sprite_width: f32) -> Option<(f32, f32)> {
        let margin = sprite_width * LEDGE_SUPPORT_MARGIN_RATIO;
        let min_x = self.left + margin;
        let max_x = self.right - margin;
        (min_x <= max_x).then_some((min_x, max_x))
    }

    pub fn floor(left: f32, right: f32, y: f32) -> Self {
        Self {
            hwnd: FLOOR_SURFACE_HWND,
            left,
            right,
            y,
            window_title: None,
        }
    }

    pub fn is_floor(&self) -> bool {
        self.hwnd == FLOOR_SURFACE_HWND
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReachBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl ReachBounds {
    pub fn compute(angle_degrees: f32) -> Self {
        let rad = angle_degrees.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();

        let half_w = SPRITE_WIDTH / 2.0;
        let half_h = SPRITE_HEIGHT / 2.0;

        let corners = [
            Vec2::new(-half_w, -half_h),
            Vec2::new(half_w, -half_h),
            Vec2::new(-half_w, half_h),
            Vec2::new(half_w, half_h),
        ];

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for corner in corners {
            let rx = corner.x * cos - corner.y * sin;
            let ry = corner.x * sin + corner.y * cos;
            min_x = min_x.min(rx);
            max_x = max_x.max(rx);
            min_y = min_y.min(ry);
            max_y = max_y.max(ry);
        }

        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SquashState {
    /// Unit normal of the impacted surface.
    pub normal: Vec2,
    /// Compression factor; `0.6` represents 40 percent compression.
    pub compression: f32,
    /// Transverse stretch factor, perpendicular to the impact normal.
    pub stretch: f32,
    /// Remaining deformation frames.
    #[deprecated(
        since = "0.3.1",
        note = "Use `update_dt()` and `hold_duration`. This field will be removed in 0.4.0."
    )]
    pub frames_remaining: u8,
    /// Time elapsed in seconds.
    pub elapsed: f32,
    /// Time spent frozen against the impact surface.
    pub hold_duration: f32,
}

impl SquashState {
    #[allow(deprecated)]
    pub fn new(normal: Vec2, hard: f32) -> Self {
        let hard = hard.clamp(0.0, 1.0);
        // Compression ranges from 0.84 at light impact to 0.54 at full impact.
        let compression = SQUASH_MIN_COMPRESSION - hard * SQUASH_COMPRESSION_RANGE;
        let stretch = 1.0 + (1.0 - compression) * SQUASH_TRANSVERSE_RATIO;
        let frames = (SQUASH_HOLD_FRAME_BASE + hard * SQUASH_HOLD_FRAME_SCALE).round() as u8;
        let hold_duration = frames as f32 * ANIME_FRAME_SECS;

        Self {
            normal,
            compression,
            stretch,
            frames_remaining: frames,
            elapsed: 0.0,
            hold_duration,
        }
    }

    #[deprecated(
        since = "0.3.1",
        note = "Use `update_dt()` for time-based animation. This method will be removed in 0.4.0."
    )]
    #[allow(deprecated)]
    pub fn update(&mut self) -> Option<Vec2> {
        if self.frames_remaining == 0 {
            return None;
        }
        self.frames_remaining -= 1;

        let scale = if self.normal.x.abs() > self.normal.y.abs() {
            Vec2::new(self.compression, self.stretch)
        } else {
            Vec2::new(self.stretch, self.compression)
        };

        Some(scale)
    }

    pub fn update_dt(&mut self, dt: f32) -> Option<Vec2> {
        self.elapsed += dt.max(0.0);
        if self.elapsed > self.hold_duration + SQUASH_RECOVERY_SECS {
            return None;
        }

        let factor = if self.elapsed <= self.hold_duration {
            self.compression
        } else {
            let settle = (self.elapsed - self.hold_duration) / SQUASH_RECOVERY_SECS;
            if settle < SQUASH_FIRST_SETTLE_FRACTION {
                1.0 + (1.0 - self.compression) * SQUASH_FIRST_RECOVERY_SCALE
            } else if settle < SQUASH_SECOND_SETTLE_FRACTION {
                SQUASH_OVERSHOOT_SCALE
            } else {
                1.0
            }
        };

        let scale = if self.normal.x.abs() > self.normal.y.abs() {
            Vec2::new(factor, 1.0 + (1.0 - factor) * SQUASH_TRANSVERSE_RATIO)
        } else {
            Vec2::new(1.0 + (1.0 - factor) * SQUASH_TRANSVERSE_RATIO, factor)
        };
        Some(scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squash_uses_authored_time_independent_of_tick_rate() {
        let mut squash = SquashState::new(Vec2::X, 1.0);
        let at_60 = squash.update_dt(1.0 / 60.0);
        let mut squash = SquashState::new(Vec2::X, 1.0);
        let at_120 = squash.update_dt(1.0 / 120.0);
        let at_120 = at_120.and_then(|_| squash.update_dt(1.0 / 120.0));

        assert_eq!(at_60, at_120);
    }

    #[test]
    fn ledge_surface_respects_horizontal_span() {
        let ledge = LedgeSurface {
            hwnd: 1,
            left: 100.0,
            right: 300.0,
            y: 200.0,
            window_title: Some("Editor".to_string()),
        };

        assert!(ledge.contains_x(200.0, 20.0));
        assert!(!ledge.contains_x(500.0, 20.0));
    }

    #[test]
    fn standing_limits_reject_spans_narrower_than_the_support_margin() {
        let narrow = LedgeSurface {
            hwnd: 1,
            left: 100.0,
            right: 140.0,
            y: 200.0,
            window_title: None,
        };
        let exact = LedgeSurface {
            left: 100.0,
            right: 196.0,
            ..narrow.clone()
        };

        assert_eq!(narrow.standing_limits(SPRITE_WIDTH), None);
        assert_eq!(exact.standing_limits(SPRITE_WIDTH), Some((148.0, 148.0)));
    }
}
