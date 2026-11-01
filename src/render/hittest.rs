use crate::core::config::{MID_POINT, SPRITE_HEIGHT, SPRITE_WIDTH};
use glam::Vec2;

/// One-byte-per-pixel opacity mask for CPU-only hit testing.
#[derive(Debug, Clone)]
pub struct AlphaBitmask {
    pub width: u32,
    pub height: u32,
    pub alpha_data: Vec<u8>,
}

impl AlphaBitmask {
    pub fn new(width: u32, height: u32, alpha_data: Vec<u8>) -> Self {
        assert_eq!((width * height) as usize, alpha_data.len());
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
        self.alpha_data[idx] > 10
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
        let delta = screen_point - center_pos;

        let rad = -rotation_degrees.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let unrotated = Vec2::new(delta.x * cos - delta.y * sin, delta.x * sin + delta.y * cos);

        let local_x = (unrotated.x / (scale * facing_x)) + MID_POINT.x;
        let local_y = (unrotated.y / scale) + MID_POINT.y;

        self.is_solid(local_x.round() as i32, local_y.round() as i32)
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
