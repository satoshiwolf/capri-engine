use crate::core::config::{
    DAZED_DURATION_SECS, DAZED_STAR_CENTER_OFFSET_Y_PX, DAZED_STAR_PHASE_RATE_RAD_PER_SEC,
    DAZED_STAR_RADIUS_X_PX, DAZED_STAR_RADIUS_Y_PX, SKID_DECELERATION, SKID_START_SPEED_PX_SEC,
};
use glam::Vec2;

#[derive(Debug, Clone)]
pub struct SkidState {
    /// Horizontal skid velocity in pixels per second.
    pub velocity_x: f32,
    /// Feet position in screen pixels.
    pub feet_pos: Vec2,
    pub finished: bool,
}

impl SkidState {
    pub fn new(feet_pos: Vec2, velocity_x: f32) -> Self {
        Self {
            velocity_x,
            feet_pos,
            finished: velocity_x.abs() < SKID_START_SPEED_PX_SEC,
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        if self.finished {
            return true;
        }

        let brake = SKID_DECELERATION * dt;
        if self.velocity_x.abs() <= brake {
            self.velocity_x = 0.0;
            self.finished = true;
        } else {
            self.velocity_x -= self.velocity_x.signum() * brake;
        }

        self.feet_pos.x += self.velocity_x * dt;
        self.finished
    }
}

#[derive(Debug, Clone)]
pub struct DazedState {
    /// Elapsed dazed time in seconds.
    pub timer: f32,
    /// Total dazed duration in seconds.
    pub duration: f32,
    /// Angular phase for the star orbit in radians.
    pub star_phase: f32,
}

impl DazedState {
    pub fn new() -> Self {
        Self {
            timer: 0.0,
            duration: DAZED_DURATION_SECS,
            star_phase: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.timer += dt;
        self.star_phase += DAZED_STAR_PHASE_RATE_RAD_PER_SEC * dt;
        self.timer >= self.duration
    }

    pub fn star_positions(&self) -> [Vec2; 4] {
        let mut stars = [Vec2::ZERO; 4];
        // The smaller vertical radius gives the orbit a tilted perspective.
        for (i, star) in stars.iter_mut().enumerate() {
            let offset = (i as f32) * (std::f32::consts::PI / 2.0);
            let angle = self.star_phase + offset;
            *star = Vec2::new(
                angle.cos() * DAZED_STAR_RADIUS_X_PX,
                angle.sin() * DAZED_STAR_RADIUS_Y_PX + DAZED_STAR_CENTER_OFFSET_Y_PX,
            );
        }
        stars
    }
}

impl Default for DazedState {
    fn default() -> Self {
        Self::new()
    }
}
