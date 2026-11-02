use glam::Vec2;
use std::collections::VecDeque;

use crate::core::config::{
    CEILING_HANG_OFFSET_PX, FACING_TURN_THRESHOLD_PX_PER_SEC, FEET_POINT, FLOOR_SURFACE_OFFSET_PX,
    LANDING_IMPACT_SPEED_NORMALIZER, LANDING_SKID_MAX_VY_PX_SEC, LEDGE_LANDING_TOLERANCE_PX,
    MID_POINT, POINTER_VELOCITY_WINDOW_SECS, RIDE_CURSOR_MAX_TILT_DEGREES, RIDE_CURSOR_OFFSET_PX,
    RIDE_CURSOR_TILT_DEG_PER_PX_SEC, SIT_TO_LAYDOWN_SECS, SLOW_RELEASE_DOWN_PX_SEC,
    SLOW_RELEASE_RETENTION, SPRITE_HEIGHT, SPRITE_WIDTH, THROW_GRAB_MARGIN_PX, THROW_MAX_SPEED,
    THROW_MIN_SPEED, UPRIGHT_ANGLE_MARGIN_DEGREES, WALL_IMPACT_MAX_SPEED_PX_SEC,
    WALL_IMPACT_MIN_SPEED_PX_SEC,
};
use crate::core::events::EngineEvent;
use crate::core::state::{Facing, Stance};
use crate::core::visual::VisualTransform;
use crate::physics::ballistics::FlightState;
use crate::physics::collision::{LedgeSurface, ReachBounds, SquashState};
use crate::physics::pendulum::PendulumState;
use crate::physics::reactions::{DazedState, SkidState};

#[derive(Debug, Clone, Copy)]
struct PointerSample {
    pos: Vec2,
    timestamp: f64,
}

pub struct PhysicsEngine {
    #[deprecated(
        since = "0.3.1",
        note = "Use `stance()`. Direct mutation bypasses FSM invariants and will be removed in 0.4.0."
    )]
    pub stance: Stance,
    #[deprecated(
        since = "0.3.1",
        note = "Use `center_pos()`. This field will be removed in 0.4.0."
    )]
    pub center_pos: Vec2,
    #[deprecated(
        since = "0.3.1",
        note = "Use `rotation()`. This field will be removed in 0.4.0."
    )]
    pub rotation: f32,
    #[deprecated(
        since = "0.3.1",
        note = "Use `scale()`. This field will be removed in 0.4.0."
    )]
    pub scale: f32,
    #[deprecated(
        since = "0.3.1",
        note = "Use `squash_scale()`. This field will be removed in 0.4.0."
    )]
    pub squash_scale: Vec2,
    #[deprecated(
        since = "0.3.1",
        note = "Use `facing()`. This field will be removed in 0.4.0."
    )]
    pub facing: Facing,

    #[deprecated(
        since = "0.3.1",
        note = "Use `pendulum()`. This field will be removed in 0.4.0."
    )]
    pub pendulum: Option<PendulumState>,
    #[deprecated(
        since = "0.3.1",
        note = "Use `flight()`. This field will be removed in 0.4.0."
    )]
    pub flight: Option<FlightState>,
    #[deprecated(
        since = "0.3.1",
        note = "Use `squash()`. This field will be removed in 0.4.0."
    )]
    pub squash: Option<SquashState>,
    #[deprecated(
        since = "0.3.1",
        note = "Use `skid()`. This field will be removed in 0.4.0."
    )]
    pub skid: Option<SkidState>,
    #[deprecated(
        since = "0.3.1",
        note = "Use `dazed()`. This field will be removed in 0.4.0."
    )]
    pub dazed: Option<DazedState>,
    #[deprecated(
        since = "0.3.1",
        note = "Use `seated_surface()`. This field will be removed in 0.4.0."
    )]
    pub seated_surface: Option<LedgeSurface>,

    pointer_samples: VecDeque<PointerSample>,
    last_cursor_pos: Vec2,
    landing_surface: Option<LedgeSurface>,
    sit_elapsed: f32,
}

#[allow(deprecated)]
impl PhysicsEngine {
    pub fn new(initial_pos: Vec2) -> Self {
        Self {
            stance: Stance::Stand,
            center_pos: initial_pos,
            rotation: 0.0,
            scale: 1.0,
            squash_scale: Vec2::ONE,
            facing: Facing::Right,
            pendulum: None,
            flight: None,
            squash: None,
            skid: None,
            dazed: None,
            seated_surface: None,
            pointer_samples: VecDeque::with_capacity(16),
            last_cursor_pos: initial_pos,
            landing_surface: None,
            sit_elapsed: 0.0,
        }
    }

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
            .is_some_and(|p| p.grab_point.y > MID_POINT.y);
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

    pub fn step(
        &mut self,
        dt: f32,
        screen_left: f32,
        screen_top: f32,
        screen_right: f32,
        screen_bottom: f32,
        ledges_y: &[f32],
    ) -> Vec<EngineEvent> {
        let surfaces = ledges_y
            .iter()
            .enumerate()
            .map(|(index, y)| LedgeSurface {
                hwnd: index as isize,
                left: screen_left,
                right: screen_right,
                y: *y,
                window_title: None,
            })
            .collect::<Vec<_>>();
        self.step_with_surfaces(
            dt,
            screen_left,
            screen_top,
            screen_right,
            screen_bottom,
            &surfaces,
        )
    }

    pub fn step_with_surfaces(
        &mut self,
        dt: f32,
        screen_left: f32,
        screen_top: f32,
        screen_right: f32,
        screen_bottom: f32,
        ledges: &[LedgeSurface],
    ) -> Vec<EngineEvent> {
        let mut events = Vec::new();

        self.squash_scale = if let Some(squash) = &mut self.squash {
            if let Some(scale) = squash.update_dt(dt) {
                scale
            } else {
                self.squash = None;
                Vec2::ONE
            }
        } else {
            Vec2::ONE
        };

        if self.stance == Stance::Sit || self.stance == Stance::LayDown {
            let Some(surface) = self.seated_surface.as_ref() else {
                self.begin_fall();
                return events;
            };
            let current = if surface.is_floor() {
                Some(surface)
            } else {
                ledges.iter().find(|ledge| ledge.hwnd == surface.hwnd)
            };
            let Some(current) = current else {
                self.seated_surface = None;
                self.begin_fall();
                return events;
            };
            let Some((min_x, max_x)) = current.standing_limits(SPRITE_WIDTH) else {
                self.seated_surface = None;
                self.begin_fall();
                return events;
            };
            self.center_pos.x = self.center_pos.x.clamp(min_x, max_x);
            self.center_pos.y = current.y;
            self.rotation = 0.0;
            if self.stance == Stance::Sit {
                self.sit_elapsed += dt;
                if self.sit_elapsed >= SIT_TO_LAYDOWN_SECS {
                    self.stance = Stance::LayDown;
                    self.sit_elapsed = 0.0;
                }
            }
            return events;
        }

        match self.stance {
            Stance::Fly => {
                let mut should_land = None;
                let mut impact_squash = None;

                if let Some(flight) = &mut self.flight {
                    flight.update(dt);
                    self.center_pos = flight.center;
                    self.rotation = flight.angle;

                    let reach = ReachBounds::compute(flight.angle);

                    if flight.center.x + reach.min_x < screen_left && flight.velocity.x < 0.0 {
                        flight.center.x = screen_left - reach.min_x;
                        let impact_speed = flight.velocity.length();
                        let hard = (flight.velocity.x.abs() - WALL_IMPACT_MIN_SPEED_PX_SEC)
                            .clamp(0.0, WALL_IMPACT_MAX_SPEED_PX_SEC)
                            / WALL_IMPACT_MAX_SPEED_PX_SEC;
                        flight.bounce(Vec2::new(1.0, 0.0), hard);
                        impact_squash = Some((Vec2::new(1.0, 0.0), hard));
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(1.0, 0.0),
                            speed: impact_speed,
                            point: Vec2::new(screen_left, flight.center.y),
                            window_title: None,
                        });
                    } else if flight.center.x + reach.max_x > screen_right
                        && flight.velocity.x > 0.0
                    {
                        flight.center.x = screen_right - reach.max_x;
                        let impact_speed = flight.velocity.length();
                        let hard = (flight.velocity.x.abs() - WALL_IMPACT_MIN_SPEED_PX_SEC)
                            .clamp(0.0, WALL_IMPACT_MAX_SPEED_PX_SEC)
                            / WALL_IMPACT_MAX_SPEED_PX_SEC;
                        flight.bounce(Vec2::new(-1.0, 0.0), hard);
                        impact_squash = Some((Vec2::new(-1.0, 0.0), hard));
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(-1.0, 0.0),
                            speed: impact_speed,
                            point: Vec2::new(screen_right, flight.center.y),
                            window_title: None,
                        });
                    }

                    if flight.center.y + reach.min_y < screen_top && flight.velocity.y < 0.0 {
                        flight.center.y = screen_top - reach.min_y;
                        let impact_speed = flight.velocity.length();
                        let hard = (flight.velocity.y.abs() - WALL_IMPACT_MIN_SPEED_PX_SEC)
                            .clamp(0.0, WALL_IMPACT_MAX_SPEED_PX_SEC)
                            / WALL_IMPACT_MAX_SPEED_PX_SEC;
                        flight.bounce(Vec2::new(0.0, 1.0), hard);
                        impact_squash = Some((Vec2::new(0.0, 1.0), hard));
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(0.0, 1.0),
                            speed: impact_speed,
                            point: Vec2::new(flight.center.x, screen_top),
                            window_title: None,
                        });
                    }

                    let ground_y = screen_bottom - FLOOR_SURFACE_OFFSET_PX;
                    let feet_y = flight.center.y + reach.max_y;

                    let mut hit_surface = None;
                    if flight.velocity.y > 0.0 {
                        for ledge in ledges {
                            if ledge.contains_x(flight.center.x, SPRITE_WIDTH * 0.25)
                                && feet_y >= ledge.y
                                && feet_y - flight.velocity.y * dt
                                    <= ledge.y + LEDGE_LANDING_TOLERANCE_PX
                            {
                                hit_surface = Some(ledge.clone());
                                break;
                            }
                        }
                    }
                    let target_y = hit_surface
                        .as_ref()
                        .map(|ledge| ledge.y)
                        .or_else(|| (feet_y >= ground_y).then_some(ground_y));

                    if let Some(target_y) = target_y {
                        let upright = flight.angle.rem_euclid(360.0);
                        let is_upright = upright < UPRIGHT_ANGLE_MARGIN_DEGREES
                            || upright > 360.0 - UPRIGHT_ANGLE_MARGIN_DEGREES;
                        let vy = flight.velocity.y;
                        let vx = flight.velocity.x;
                        let speed = flight.velocity.length();
                        should_land = Some((
                            target_y,
                            reach.max_y,
                            is_upright,
                            vy,
                            vx,
                            speed,
                            hit_surface,
                        ));
                    }
                }
                if let Some((normal, hard)) = impact_squash {
                    self.apply_squash(normal, hard);
                }

                if let Some((target_y, reach_max_y, is_upright, vy, vx, speed, hit_surface)) =
                    should_land
                {
                    self.center_pos.y = target_y - reach_max_y;
                    self.flight = None;
                    self.apply_squash(
                        Vec2::new(0.0, 1.0),
                        (vy / LANDING_IMPACT_SPEED_NORMALIZER).clamp(0.0, 1.0),
                    );

                    if is_upright && vy < LANDING_SKID_MAX_VY_PX_SEC {
                        self.center_pos.y = target_y - (FEET_POINT.y - MID_POINT.y) * self.scale;
                        self.stance = Stance::Skid;
                        self.rotation = 0.0;
                        self.skid =
                            Some(SkidState::new(Vec2::new(self.center_pos.x, target_y), vx));
                        self.landing_surface = hit_surface.clone();
                        let window_title = hit_surface
                            .as_ref()
                            .and_then(|surface| surface.window_title.clone());
                        events.push(EngineEvent::Landed {
                            surface_y: target_y,
                            window_title,
                            skid: true,
                        });
                    } else {
                        self.stance = Stance::Dazed;
                        self.center_pos.y = target_y;
                        self.rotation = 0.0;
                        self.dazed = Some(DazedState::new());
                        self.landing_surface = hit_surface.clone();
                        let window_title = hit_surface
                            .as_ref()
                            .and_then(|surface| surface.window_title.clone());
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(0.0, -1.0),
                            speed,
                            point: Vec2::new(self.center_pos.x, target_y),
                            window_title,
                        });
                    }
                }
            }
            Stance::Skid => {
                let floor_standing_offset = (FEET_POINT.y - MID_POINT.y) * self.scale;
                if let Some(skid) = &mut self.skid {
                    let finished = skid.update(dt);
                    self.center_pos.x = skid.feet_pos.x;
                    if finished {
                        if let Some(surface) = self.landing_surface.take() {
                            self.center_pos = Vec2::new(self.center_pos.x, surface.y);
                            self.stance = Stance::Sit;
                            self.seated_surface = Some(surface);
                            self.sit_elapsed = 0.0;
                        } else {
                            self.stance = Stance::Stand;
                            self.center_pos.y = skid.feet_pos.y - floor_standing_offset;
                        }
                        self.skid = None;
                    }
                }
            }
            Stance::Dazed => {
                if let Some(dazed) = &mut self.dazed {
                    let recovered = dazed.update(dt);
                    if recovered {
                        self.rotation = 0.0;
                        self.dazed = None;
                        if let Some(surface) = self.landing_surface.take() {
                            self.center_pos.y = surface.y;
                            self.stance = Stance::Sit;
                            self.seated_surface = Some(surface);
                            self.sit_elapsed = 0.0;
                        } else {
                            self.stance = Stance::Stand;
                            self.seated_surface = None;
                            self.center_pos.y = self.floor_standing_center(self.center_pos.y);
                        }
                    }
                }
            }
            _ => {}
        }

        events
    }

    pub fn visual_transform(&self) -> VisualTransform {
        VisualTransform::new(
            self.center_pos,
            self.rotation,
            Vec2::new(
                self.scale * self.squash_scale.x,
                self.scale * self.squash_scale.y,
            ),
            self.facing,
        )
    }

    pub fn has_landing_surface(&self) -> bool {
        self.landing_surface.is_some()
    }

    fn begin_fall(&mut self) {
        self.clear_motion_state();
        self.stance = Stance::Fly;
        self.rotation = 0.0;
        self.flight = Some(FlightState::new_throw(
            self.center_pos,
            Vec2::new(0.0, 1.0),
            false,
        ));
    }

    fn floor_standing_center(&self, surface_y: f32) -> f32 {
        surface_y - (FEET_POINT.y - MID_POINT.y) * self.scale
    }

    fn apply_squash(&mut self, normal: Vec2, hard: f32) {
        let mut squash = SquashState::new(normal, hard);
        self.squash_scale = squash.update_dt(0.0).unwrap_or(Vec2::ONE);
        self.squash = Some(squash);
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

    fn clear_motion_state(&mut self) {
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

#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_release_enters_fall_instead_of_hovering() {
        let mut physics = PhysicsEngine::new(Vec2::new(100.0, 100.0));
        physics.pick_up(Vec2::new(100.0, 100.0));
        physics.release();

        assert_eq!(physics.stance, Stance::Fly);
        assert!(physics.flight.is_some());
    }

    #[test]
    fn grab_point_remains_under_cursor_while_rotating() {
        let mut physics = PhysicsEngine::new(Vec2::new(200.0, 200.0));
        let cursor = Vec2::new(230.0, 180.0);
        physics.pick_up(cursor);
        physics.rotation = 25.0;
        physics.update_held(cursor, 0.016, 0.016);
        let grab = physics
            .pendulum
            .as_ref()
            .map(|pendulum| pendulum.grab_point)
            .expect("pendulum");
        let rendered_grab = physics.visual_transform().apply(grab - MID_POINT);

        assert!((rendered_grab - cursor).length() < 0.001);
    }

    #[test]
    fn landing_on_visible_ledge_enters_sit_after_skid() {
        let mut physics = PhysicsEngine::new(Vec2::new(200.0, 0.0));
        physics.stance = Stance::Fly;
        physics.flight = Some(FlightState::new_throw(
            Vec2::new(200.0, 0.0),
            Vec2::new(0.0, 500.0),
            false,
        ));
        let ledge = LedgeSurface {
            hwnd: 7,
            left: 100.0,
            right: 400.0,
            y: 220.0,
            window_title: Some("Editor".to_string()),
        };

        for _ in 0..180 {
            physics.step_with_surfaces(1.0 / 60.0, 0.0, -500.0, 800.0, 600.0, &[ledge.clone()]);
            if physics.stance == Stance::Sit {
                break;
            }
        }

        assert_eq!(physics.stance, Stance::Sit);
        assert_eq!(
            physics.seated_surface.as_ref().map(|surface| surface.hwnd),
            Some(7)
        );
    }

    #[test]
    fn narrow_support_forces_fall_without_panicking() {
        let mut physics = PhysicsEngine::new(Vec2::new(120.0, 200.0));
        physics.stance = Stance::Sit;
        physics.seated_surface = Some(LedgeSurface {
            hwnd: 7,
            left: 100.0,
            right: 140.0,
            y: 200.0,
            window_title: None,
        });

        let events = physics.step_with_surfaces(1.0 / 60.0, 0.0, 0.0, 800.0, 600.0, &[]);

        assert!(events.is_empty());
        assert_eq!(physics.stance, Stance::Fly);
        assert!(physics.flight.is_some());
    }

    #[test]
    fn command_transitions_clear_stale_motion_and_remain_stable() {
        let mut physics = PhysicsEngine::new(Vec2::new(100.0, 100.0));
        physics.pick_up(Vec2::new(100.0, 100.0));
        physics.release();

        physics.hang_on_ceiling(-500.0);
        assert_eq!(physics.stance, Stance::Upside);
        assert!(physics.flight.is_none());
        physics.step_with_surfaces(1.0 / 60.0, 0.0, -500.0, 800.0, 600.0, &[]);
        assert_eq!(physics.stance, Stance::Upside);

        physics.ride_cursor();
        assert_eq!(physics.stance, Stance::Ride);
        physics.step_with_surfaces(1.0 / 60.0, 0.0, -500.0, 800.0, 600.0, &[]);
        assert_eq!(physics.stance, Stance::Ride);
    }

    #[test]
    fn sleep_uses_floor_and_wake_restores_standing_pose() {
        let mut physics = PhysicsEngine::new(Vec2::new(200.0, 100.0));
        physics.sleep(0.0, 800.0, 600.0);

        physics.step_with_surfaces(1.0 / 60.0, 0.0, -500.0, 800.0, 600.0, &[]);
        assert_eq!(physics.stance, Stance::LayDown);
        assert_eq!(physics.center_pos.y, 600.0);

        physics.wake();
        assert_eq!(physics.stance, Stance::Stand);
        assert_eq!(
            physics.center_pos.y,
            600.0 - (FEET_POINT.y - MID_POINT.y) * physics.scale
        );
        assert!(physics.seated_surface.is_none());
    }

    #[test]
    fn wall_impact_reports_pre_bounce_speed() {
        let mut physics = PhysicsEngine::new(Vec2::new(0.0, 100.0));
        physics.stance = Stance::Fly;
        physics.flight = Some(FlightState::new_throw(
            Vec2::new(0.0, 100.0),
            Vec2::new(-1000.0, 0.0),
            false,
        ));

        let events = physics.step_with_surfaces(1.0 / 60.0, 0.0, -500.0, 800.0, 600.0, &[]);
        let reported_speed = events
            .iter()
            .find_map(|event| match event {
                EngineEvent::Impact { speed, .. } => Some(*speed),
                _ => None,
            })
            .expect("wall impact event");
        let rebound_speed = physics
            .flight
            .as_ref()
            .map(|flight| flight.velocity.length())
            .expect("flight state");

        assert!(reported_speed > rebound_speed);
    }
}
