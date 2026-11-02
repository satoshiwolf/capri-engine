use glam::Vec2;

use crate::core::config::{
    CEILING_HANG_OFFSET_PX, FACING_TURN_THRESHOLD_PX_PER_SEC, FEET_POINT, MID_POINT,
    POINTER_VELOCITY_WINDOW_SECS, RIDE_CURSOR_MAX_TILT_DEGREES, RIDE_CURSOR_OFFSET_PX,
    RIDE_CURSOR_TILT_DEG_PER_PX_SEC, SLOW_RELEASE_DOWN_PX_SEC, SLOW_RELEASE_RETENTION,
    SPRITE_HEIGHT, SPRITE_WIDTH, THROW_GRAB_MARGIN_PX, THROW_MAX_SPEED, THROW_MIN_SPEED,
};
use crate::core::events::EngineEvent;
use crate::core::state::{Facing, Stance};
use crate::physics::ballistics::FlightState;
use crate::physics::collision::{LedgeSurface, SquashState};
use crate::physics::pendulum::PendulumState;
use crate::physics::reactions::{DazedState, SkidState};

use super::{PhysicsEngine, PointerSample};

#[allow(deprecated)]
impl PhysicsEngine {
    pub fn stance(&self) -> Stance {
        self.stance
    }

    pub fn center_pos(&self) -> Vec2 {
        self.center_pos
    }

    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn squash_scale(&self) -> Vec2 {
        self.squash_scale
    }

    pub fn facing(&self) -> Facing {
        self.facing
    }

    pub fn pendulum(&self) -> Option<&PendulumState> {
        self.pendulum.as_ref()
    }

    pub fn flight(&self) -> Option<&FlightState> {
        self.flight.as_ref()
    }

    pub fn squash(&self) -> Option<&SquashState> {
        self.squash.as_ref()
    }

    pub fn skid(&self) -> Option<&SkidState> {
        self.skid.as_ref()
    }

    pub fn dazed(&self) -> Option<&DazedState> {
        self.dazed.as_ref()
    }

    pub fn seated_surface(&self) -> Option<&LedgeSurface> {
        self.seated_surface.as_ref()
    }

    pub fn pick_up(&mut self, cursor_pos: Vec2) -> EngineEvent {
        self.clear_motion_state();
        self.stance = Stance::Held;

        let local_grab = self.visual_transform().screen_to_local(cursor_pos) + MID_POINT;

        let grab_point = Vec2::new(
            local_grab
                .x
                .clamp(THROW_GRAB_MARGIN_PX, SPRITE_WIDTH - THROW_GRAB_MARGIN_PX),
            local_grab
                .y
                .clamp(THROW_GRAB_MARGIN_PX, SPRITE_HEIGHT - THROW_GRAB_MARGIN_PX),
        );

        self.pendulum = Some(PendulumState::new(grab_point));
        self.pointer_samples.clear();
        self.last_cursor_pos = cursor_pos;

        EngineEvent::Grabbed {
            screen_pos: self.center_pos,
            grab_offset: grab_point,
        }
    }

    pub fn update_held(&mut self, cursor_pos: Vec2, timestamp: f64, dt: f32) {
        if self.stance != Stance::Held {
            return;
        }

        self.pointer_samples.push_back(PointerSample {
            pos: cursor_pos,
            timestamp,
        });
        while self
            .pointer_samples
            .front()
            .is_some_and(|sample| timestamp - sample.timestamp > POINTER_VELOCITY_WINDOW_SECS)
        {
            self.pointer_samples.pop_front();
        }
        while self.pointer_samples.len() > 16 {
            self.pointer_samples.pop_front();
        }

        let cursor_velocity = self.calculate_pointer_velocity();

        let grab_point = if let Some(pendulum) = &mut self.pendulum {
            pendulum.update(cursor_velocity, dt);
            self.rotation = pendulum.angle;
            Some(pendulum.grab_point)
        } else {
            None
        };
        if let Some(grab_point) = grab_point {
            let grab_offset = self
                .visual_transform()
                .local_delta_to_screen(grab_point - MID_POINT);
            self.center_pos = cursor_pos - grab_offset;
            if cursor_velocity.x.abs() > FACING_TURN_THRESHOLD_PX_PER_SEC {
                self.facing = if cursor_velocity.x < 0.0 {
                    Facing::Left
                } else {
                    Facing::Right
                };
            }
        }

        self.last_cursor_pos = cursor_pos;
    }

    pub fn release(&mut self) -> EngineEvent {
        if self.stance != Stance::Held {
            return EngineEvent::Dropped {
                screen_pos: self.center_pos,
            };
        }

        let velocity = self.calculate_pointer_velocity();
        let speed = velocity.length();

        let feet_first = self
            .pendulum
            .as_ref()
            .is_some_and(|pendulum| pendulum.grab_point.y > MID_POINT.y);
        self.pendulum = None;

        if speed >= THROW_MIN_SPEED {
            self.stance = Stance::Fly;
            let flight = FlightState::new_throw(self.center_pos, velocity, feet_first);
            let speed_val = flight.velocity.length();
            self.rotation = flight.angle;
            if velocity.x.abs() > FACING_TURN_THRESHOLD_PX_PER_SEC {
                self.facing = if velocity.x < 0.0 {
                    Facing::Left
                } else {
                    Facing::Right
                };
            }
            self.flight = Some(flight);

            EngineEvent::Thrown {
                velocity,
                speed: speed_val,
            }
        } else {
            self.stance = Stance::Fly;
            self.rotation = 0.0;
            self.flight = Some(FlightState::new_throw(
                self.center_pos,
                velocity * SLOW_RELEASE_RETENTION + Vec2::new(0.0, SLOW_RELEASE_DOWN_PX_SEC),
                feet_first,
            ));
            EngineEvent::Dropped {
                screen_pos: self.center_pos,
            }
        }
    }

    pub fn update_ride(&mut self, cursor_pos: Vec2, cursor_velocity: Vec2, dt: f32) {
        if self.stance != Stance::Ride {
            return;
        }
        self.center_pos = cursor_pos + Vec2::new(0.0, RIDE_CURSOR_OFFSET_PX);
        self.rotation = (cursor_velocity.x * RIDE_CURSOR_TILT_DEG_PER_PX_SEC)
            .clamp(-RIDE_CURSOR_MAX_TILT_DEGREES, RIDE_CURSOR_MAX_TILT_DEGREES);
        self.squash_scale = Vec2::ONE;
        if cursor_velocity.x.abs() > FACING_TURN_THRESHOLD_PX_PER_SEC {
            self.facing = if cursor_velocity.x < 0.0 {
                Facing::Left
            } else {
                Facing::Right
            };
        }
        let _ = dt;
    }

    pub fn hang_on_ceiling(&mut self, screen_top: f32) {
        self.clear_motion_state();
        self.stance = Stance::Upside;
        self.rotation = 180.0;
        self.center_pos.y = screen_top + CEILING_HANG_OFFSET_PX;
    }

    pub fn ride_cursor(&mut self) {
        self.clear_motion_state();
        self.stance = Stance::Ride;
    }

    pub fn sleep(&mut self, screen_left: f32, screen_right: f32, screen_bottom: f32) {
        let surface = self
            .seated_surface
            .clone()
            .or_else(|| self.landing_surface.clone())
            .unwrap_or_else(|| LedgeSurface::floor(screen_left, screen_right, screen_bottom));

        self.clear_motion_state();
        self.stance = Stance::LayDown;
        self.center_pos = Vec2::new(
            self.center_pos.x.clamp(surface.left, surface.right),
            surface.y,
        );
        self.rotation = 0.0;
        self.seated_surface = Some(surface);
        self.sit_elapsed = 0.0;
    }

    pub fn perch_on(&mut self, surface: LedgeSurface) -> bool {
        let Some((min_x, max_x)) = surface.standing_limits(SPRITE_WIDTH) else {
            return false;
        };

        let center_x = self.center_pos.x.clamp(min_x, max_x);
        self.clear_motion_state();
        self.stance = Stance::Sit;
        self.center_pos = Vec2::new(center_x, surface.y);
        self.rotation = 0.0;
        self.seated_surface = Some(surface);
        self.sit_elapsed = 0.0;
        true
    }

    pub fn wake(&mut self) {
        let support = self
            .seated_surface
            .take()
            .or_else(|| self.landing_surface.take());
        self.clear_motion_state();
        self.stance = Stance::Stand;
        self.rotation = 0.0;
        if let Some(surface) = support {
            self.center_pos.y = self.floor_standing_center(surface.y);
        }
    }

    pub fn reset_stand(&mut self) {
        self.wake();
    }

    pub fn face_toward(&mut self, x: f32) {
        self.facing = if x < self.center_pos.x {
            Facing::Left
        } else {
            Facing::Right
        };
    }

    pub(super) fn floor_standing_center(&self, surface_y: f32) -> f32 {
        surface_y - (FEET_POINT.y - MID_POINT.y) * self.scale
    }

    fn calculate_pointer_velocity(&self) -> Vec2 {
        if self.pointer_samples.len() < 2 {
            return Vec2::ZERO;
        }

        let (Some(oldest), Some(newest)) =
            (self.pointer_samples.front(), self.pointer_samples.back())
        else {
            return Vec2::ZERO;
        };

        let dt = (newest.timestamp - oldest.timestamp) as f32;
        if dt <= 0.001 {
            return Vec2::ZERO;
        }

        let delta = newest.pos - oldest.pos;
        let vel = delta / dt;

        let speed = vel.length();
        if speed > THROW_MAX_SPEED {
            vel.normalize() * THROW_MAX_SPEED
        } else {
            vel
        }
    }

    pub(super) fn clear_motion_state(&mut self) {
        self.pendulum = None;
        self.flight = None;
        self.squash = None;
        self.squash_scale = Vec2::ONE;
        self.skid = None;
        self.dazed = None;
        self.seated_surface = None;
        self.landing_surface = None;
        self.sit_elapsed = 0.0;
    }
}
