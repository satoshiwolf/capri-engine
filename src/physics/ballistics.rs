use crate::core::config::{
    AIR_RESISTANCE, ANIME_FRAME_SECS, BOUNCE_ANGULAR_DAMP, BOUNCE_RESTITUTION_Y,
    FLIGHT_FLOOR_ALIGN_DEG_PER_SEC, FLIGHT_HITSTOP_FRAME_BASE, FLIGHT_HITSTOP_FRAME_SCALE,
    FLIGHT_HITSTOP_MIN_HARDNESS, FLIGHT_MAX_ANGULAR_SPEED_DEG_PER_SEC,
    FLIGHT_OPPOSITE_HORIZONTAL_ANGLE_DEGREES, FLIGHT_REVERSAL_ANGLE_DEGREES,
    FLIGHT_SURFACE_AXIS_THRESHOLD, FLIGHT_VERTICAL_ANGULAR_DAMP,
    FLIGHT_ZERO_HORIZONTAL_ANGLE_DEGREES, LEAP_GRAVITY, RELEASE_STREAK_SECS, SKID_DECELERATION,
    THROW_GRAVITY, THROW_MAX_SPEED, THROW_SPIN_FROM_VX, THROW_VERTICAL_BOUNCE_X_DAMP,
};
use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightPhase {
    Air,
    FloorSlide,
}

#[derive(Debug, Clone)]
pub struct FlightState {
    /// Center-of-mass position in screen pixels.
    pub center: Vec2,
    /// Linear velocity in pixels per second.
    pub velocity: Vec2,
    /// Rotation angle in degrees.
    pub angle: f32,
    /// Angular velocity in degrees per second.
    pub angular_velocity: f32,
    /// Remaining initial release-streak frames.
    pub release_streak: u8,
    /// Remaining initial release-streak time in seconds.
    pub release_streak_time: f32,
    pub phase: FlightPhase,
    pub is_leap: bool,
    /// Remaining hit-stop duration in seconds.
    pub freeze_timer: f32,
    pub hit_count: u32,
    /// Largest recorded impact hardness in the range `0.0..=1.0`.
    pub max_impact_hard: f32,
    /// Target resting angle in degrees when lying on the floor.
    pub lie_target_angle: f32,
}

impl FlightState {
    pub fn new_throw(center: Vec2, mut velocity: Vec2, feet_first: bool) -> Self {
        let speed = velocity.length();
        if speed > THROW_MAX_SPEED {
            velocity = velocity.normalize() * THROW_MAX_SPEED;
        }

        // Align the body with the initial throw direction for the release streak.
        let mut along = velocity.x.atan2(-velocity.y).to_degrees();
        if feet_first {
            along += FLIGHT_REVERSAL_ANGLE_DEGREES;
        }

        // Tumbling speed follows the horizontal throw impulse.
        let angular_velocity = (velocity.x * THROW_SPIN_FROM_VX).clamp(
            -FLIGHT_MAX_ANGULAR_SPEED_DEG_PER_SEC,
            FLIGHT_MAX_ANGULAR_SPEED_DEG_PER_SEC,
        );

        Self {
            center,
            velocity,
            angle: along,
            angular_velocity,
            release_streak: 2,
            release_streak_time: RELEASE_STREAK_SECS,
            phase: FlightPhase::Air,
            is_leap: false,
            freeze_timer: 0.0,
            hit_count: 0,
            max_impact_hard: 0.0,
            lie_target_angle: if along.rem_euclid(360.0) > 180.0 {
                FLIGHT_OPPOSITE_HORIZONTAL_ANGLE_DEGREES
            } else {
                FLIGHT_ZERO_HORIZONTAL_ANGLE_DEGREES
            },
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.freeze_timer > 0.0 {
            self.freeze_timer -= dt;
            return;
        }

        if self.release_streak_time > 0.0 {
            self.release_streak_time = (self.release_streak_time - dt).max(0.0);
            self.release_streak = (self.release_streak_time / ANIME_FRAME_SECS).ceil() as u8;
        }

        let dt = dt.clamp(0.001, 0.05);

        match self.phase {
            FlightPhase::Air => {
                let gravity = if self.is_leap {
                    LEAP_GRAVITY
                } else {
                    THROW_GRAVITY
                };
                self.velocity.y += gravity * dt;
                self.velocity.x *= AIR_RESISTANCE.powf(dt);

                self.angle += self.angular_velocity * dt;
                self.center += self.velocity * dt;
            }
            FlightPhase::FloorSlide => {
                let angle_diff = self.lie_target_angle - self.angle;
                self.angle += angle_diff.clamp(
                    -FLIGHT_FLOOR_ALIGN_DEG_PER_SEC * dt,
                    FLIGHT_FLOOR_ALIGN_DEG_PER_SEC * dt,
                );

                let brake = SKID_DECELERATION * dt;
                if self.velocity.x.abs() <= brake {
                    self.velocity.x = 0.0;
                } else {
                    self.velocity.x -= self.velocity.x.signum() * brake;
                }
                self.center.x += self.velocity.x * dt;
            }
        }
    }

    pub fn bounce(&mut self, normal: Vec2, hard: f32) {
        self.hit_count += 1;
        if hard > self.max_impact_hard {
            self.max_impact_hard = hard;
        }

        // Hold for one to four frames depending on impact hardness.
        let frames_hold = FLIGHT_HITSTOP_FRAME_BASE + hard * FLIGHT_HITSTOP_FRAME_SCALE;
        if hard > FLIGHT_HITSTOP_MIN_HARDNESS {
            self.freeze_timer = frames_hold * ANIME_FRAME_SECS;
        }

        if normal.x.abs() > FLIGHT_SURFACE_AXIS_THRESHOLD {
            self.velocity.x = -self.velocity.x * crate::core::config::BOUNCE_RESTITUTION_X;
            self.velocity.y *= THROW_VERTICAL_BOUNCE_X_DAMP;
            self.angular_velocity = -self.angular_velocity * BOUNCE_ANGULAR_DAMP;
        } else if normal.y.abs() > FLIGHT_SURFACE_AXIS_THRESHOLD {
            self.velocity.y = -self.velocity.y * BOUNCE_RESTITUTION_Y;
            self.angular_velocity *= FLIGHT_VERTICAL_ANGULAR_DAMP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flight_gravity() {
        let mut flight =
            FlightState::new_throw(Vec2::new(100.0, 100.0), Vec2::new(200.0, -500.0), false);
        let initial_vy = flight.velocity.y;

        flight.update(0.016);

        assert!(flight.velocity.y > initial_vy);
    }

    #[test]
    fn test_flight_bounce() {
        let mut flight =
            FlightState::new_throw(Vec2::new(100.0, 100.0), Vec2::new(1000.0, 0.0), false);
        flight.bounce(Vec2::new(-1.0, 0.0), 0.5);

        assert!(flight.velocity.x < 0.0);
        assert!(flight.freeze_timer > 0.0);
    }

    #[test]
    fn release_streak_uses_time_instead_of_render_ticks() {
        let velocity = Vec2::new(1000.0, -500.0);
        let mut at_60 = FlightState::new_throw(Vec2::new(100.0, 100.0), velocity, false);
        let mut at_120 = FlightState::new_throw(Vec2::new(100.0, 100.0), velocity, false);

        at_60.update(1.0 / 60.0);
        at_120.update(1.0 / 120.0);
        at_120.update(1.0 / 120.0);

        assert_eq!(at_60.release_streak, at_120.release_streak);
    }
}
