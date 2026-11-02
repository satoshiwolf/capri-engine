use glam::Vec2;

use crate::core::state::Facing;

/// Shared screen-space transform used by rendering and CPU hit testing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualTransform {
    pub center: Vec2,
    pub rotation_degrees: f32,
    pub scale: Vec2,
    pub facing: Facing,
}

impl VisualTransform {
    pub fn new(center: Vec2, rotation_degrees: f32, scale: Vec2, facing: Facing) -> Self {
        Self {
            center,
            rotation_degrees,
            scale,
            facing,
        }
    }

    pub fn local_delta_to_screen(self, delta: Vec2) -> Vec2 {
        let scaled = Vec2::new(
            delta.x * self.scale.x * self.facing.as_f32(),
            delta.y * self.scale.y,
        );
        rotate(scaled, self.rotation_degrees)
    }

    pub fn screen_to_local(self, screen_point: Vec2) -> Vec2 {
        let unrotated = rotate(screen_point - self.center, -self.rotation_degrees);
        let facing = self.facing.as_f32();
        Vec2::new(
            unrotated.x / non_zero(self.scale.x * facing),
            unrotated.y / non_zero(self.scale.y),
        )
    }

    pub fn apply(self, local_point: Vec2) -> Vec2 {
        self.center + self.local_delta_to_screen(local_point)
    }
}

fn rotate(point: Vec2, degrees: f32) -> Vec2 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Vec2::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

fn non_zero(value: f32) -> f32 {
    if value.abs() < f32::EPSILON {
        f32::EPSILON.copysign(value)
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_transform_round_trips_local_points() {
        let transform = VisualTransform::new(
            Vec2::new(140.0, 220.0),
            31.0,
            Vec2::new(1.2, 0.8),
            Facing::Left,
        );
        let local = Vec2::new(42.0, -17.0);

        let screen = transform.apply(local);
        let restored = transform.screen_to_local(screen);

        assert!((restored - local).length() < 0.001);
    }

    #[test]
    fn zero_scale_does_not_divide_by_zero() {
        let transform = VisualTransform::new(Vec2::ZERO, 0.0, Vec2::new(0.0, 0.0), Facing::Right);

        assert!(transform.screen_to_local(Vec2::ONE).is_finite());
    }
}
