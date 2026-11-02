use crate::core::config::{MID_POINT, SPRITE_HEIGHT, SPRITE_WIDTH};
use crate::core::visual::VisualTransform;
use glam::Vec2;

/// One-byte-per-pixel opacity mask for CPU-only hit testing.
#[derive(Debug, Clone)]
pub struct AlphaBitmask {
    pub width: u32,
    pub height: u32,
    pub alpha_data: Vec<u8>,
}

impl AlphaBitmask {
    pub fn new(width: u32, height: u32, mut alpha_data: Vec<u8>) -> Self {
        alpha_data.resize(width as usize * height as usize, 0);
        Self {
            width,
            height,
            alpha_data,
        }
    }

    /// Alpha values above 10 count as solid.
    pub fn is_solid(&self, local_x: i32, local_y: i32) -> bool {
        if local_x < 0
            || local_x >= self.width as i32
            || local_y < 0
            || local_y >= self.height as i32
        {
            return false;
        }
        let idx = (local_y as usize) * (self.width as usize) + (local_x as usize);
        self.alpha_data.get(idx).is_some_and(|alpha| *alpha > 10)
    }

    /// Tests a screen-space point against the sprite transform without a GPU readback.
    pub fn hit_test_screen(
        &self,
        screen_point: Vec2,
        center_pos: Vec2,
        rotation_degrees: f32,
        scale: f32,
        facing_x: f32,
    ) -> bool {
        let facing = if facing_x < 0.0 {
            crate::core::state::Facing::Left
        } else {
            crate::core::state::Facing::Right
        };
        self.hit_test_transformed(
            screen_point,
            VisualTransform::new(center_pos, rotation_degrees, Vec2::splat(scale), facing),
            MID_POINT,
        )
    }

    pub fn hit_test_transformed(
        &self,
        screen_point: Vec2,
        transform: VisualTransform,
        pivot: Vec2,
    ) -> bool {
        let local = transform.screen_to_local(screen_point) + pivot;
        self.is_solid(local.x.round() as i32, local.y.round() as i32)
    }
}

pub fn create_default_mask() -> AlphaBitmask {
    let w = SPRITE_WIDTH as u32;
    let h = SPRITE_HEIGHT as u32;
    let mut data = vec![0u8; (w * h) as usize];

    for y in 40..h - 20 {
        for x in 40..w - 40 {
            data[(y * w + x) as usize] = 255;
        }
    }

    AlphaBitmask::new(w, h, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_mask_data_is_padded_without_panicking() {
        let mask = AlphaBitmask::new(2, 2, vec![255]);
        assert!(mask.is_solid(0, 0));
        assert!(!mask.is_solid(1, 1));
        assert!(!mask.is_solid(10, 10));
    }

    #[test]
    fn transformed_hit_test_matches_render_transform() {
        let mut alpha = vec![0; 20 * 20];
        alpha[10 * 20 + 10] = 255;
        let mask = AlphaBitmask::new(20, 20, alpha);
        let transform = VisualTransform::new(
            Vec2::new(100.0, 100.0),
            30.0,
            Vec2::new(1.5, 0.75),
            crate::core::state::Facing::Right,
        );
        let pivot = Vec2::new(10.0, 10.0);
        let screen = transform.apply(Vec2::ZERO);

        assert!(mask.hit_test_transformed(screen, transform, pivot));
    }
}
