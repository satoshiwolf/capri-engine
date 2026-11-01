use crate::core::config::{SPRITE_HEIGHT, SPRITE_WIDTH};
use glam::Vec2;

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
    pub frames_remaining: u8,
}

impl SquashState {
    pub fn new(normal: Vec2, hard: f32) -> Self {
        let hard = hard.clamp(0.0, 1.0);
        // Compression ranges from 0.84 at light impact to 0.54 at full impact.
        let compression = 0.84 - hard * 0.30;
        let stretch = 1.0 + (1.0 - compression) * 0.60;
        let frames = (1.0 + hard * 4.0).round() as u8;

        Self {
            normal,
            compression,
            stretch,
            frames_remaining: frames,
        }
    }

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
}
