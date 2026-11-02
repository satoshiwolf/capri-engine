use glam::Vec2;
use std::collections::VecDeque;

use crate::core::config::{
    FEET_POINT, FLOOR_SURFACE_OFFSET_PX, LANDING_IMPACT_SPEED_NORMALIZER,
    LANDING_SKID_MAX_VY_PX_SEC, LEDGE_LANDING_TOLERANCE_PX, MID_POINT, SIT_TO_LAYDOWN_SECS,
    SPRITE_WIDTH, UPRIGHT_ANGLE_MARGIN_DEGREES, WALL_IMPACT_MAX_SPEED_PX_SEC,
    WALL_IMPACT_MIN_SPEED_PX_SEC,
};
use crate::core::events::EngineEvent;
use crate::core::state::{Facing, Stance};
use crate::core::visual::VisualTransform;
use crate::physics::ballistics::FlightState;
use crate::physics::collision::{LedgeSurface, ReachBounds, SquashState};
use crate::physics::pendulum::PendulumState;
use crate::physics::reactions::{DazedState, SkidState};

mod transitions;

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

    fn apply_squash(&mut self, normal: Vec2, hard: f32) {
        let mut squash = SquashState::new(normal, hard);
        self.squash_scale = squash.update_dt(0.0).unwrap_or(Vec2::ONE);
        self.squash = Some(squash);
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
    fn perch_command_rejects_unsupported_spans() {
        let mut physics = PhysicsEngine::new(Vec2::new(200.0, 100.0));
        let wide = LedgeSurface {
            hwnd: 7,
            left: 100.0,
            right: 400.0,
            y: 220.0,
            window_title: Some("Editor".to_string()),
        };
        let narrow = LedgeSurface {
            right: 140.0,
            ..wide.clone()
        };

        assert!(!physics.perch_on(narrow));
        assert_eq!(physics.stance, Stance::Stand);
        assert!(physics.perch_on(wide));
        assert_eq!(physics.stance, Stance::Sit);
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
