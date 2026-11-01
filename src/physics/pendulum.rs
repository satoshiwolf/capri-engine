use crate::core::config::{PENDULUM_DAMPING, PENDULUM_MAX_ANGLE, PENDULUM_STIFFNESS};
use glam::Vec2;

#[derive(Debug, Clone)]
pub struct PendulumState {
    /// Grab point in local sprite pixels.
    pub grab_point: Vec2,
    /// Oscillation angle in degrees.
    pub angle: f32,
    /// Angular velocity in degrees per second.
    pub angular_velocity: f32,
    /// Speed-dependent hair smear offset in pixels.
    pub smear_offset: Vec2,
    /// Hair lean factor in the range `-7.0..=7.0`.
    pub hair_lean: f32,
}

impl PendulumState {
    pub fn new(grab_point: Vec2) -> Self {
        Self {
            grab_point,
            angle: 0.0,
            angular_velocity: 0.0,
            smear_offset: Vec2::ZERO,
            hair_lean: 0.0,
        }
    }

    pub fn update(&mut self, cursor_velocity: Vec2, dt: f32) {
        let dt = dt.clamp(0.001, 0.05);

        let target_angle =
            (cursor_velocity.x * 0.014).clamp(-PENDULUM_MAX_ANGLE, PENDULUM_MAX_ANGLE);

        // Damped second-order spring: w' = k * (target - angle) - c * w.
        let spring_accel = (target_angle - self.angle) * PENDULUM_STIFFNESS
            - self.angular_velocity * PENDULUM_DAMPING;
        self.angular_velocity += spring_accel * dt;
        self.angle += self.angular_velocity * dt;

        let speed = cursor_velocity.length();
        if speed > 80.0 {
            let dir = cursor_velocity.normalize();
            let k = (speed / 2200.0).clamp(0.0, 1.0);
            self.hair_lean = (-dir.x * 7.0 * k).clamp(-7.0, 7.0);
            if k > 0.35 {
                self.smear_offset = -dir * 20.0 * k;
            } else {
                self.smear_offset = Vec2::ZERO;
            }
        } else {
            self.hair_lean = 0.0;
            self.smear_offset = Vec2::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pendulum_spring() {
        let mut pendulum = PendulumState::new(Vec2::new(120.0, 200.0));
        let velocity = Vec2::new(500.0, 0.0);

        for _ in 0..10 {
            pendulum.update(velocity, 0.016);
        }

        assert!(pendulum.angle > 0.0);
        assert!(pendulum.angle <= PENDULUM_MAX_ANGLE);
    }
}
