use glam::Vec2;

use crate::core::config::{
    FEET_POINT, FLOOR_SURFACE_OFFSET_PX, LANDING_IMPACT_SPEED_NORMALIZER,
    LANDING_SKID_MAX_VY_PX_SEC, LEDGE_LANDING_TOLERANCE_PX, MID_POINT, SIT_TO_LAYDOWN_SECS,
    SPRITE_WIDTH, UPRIGHT_ANGLE_MARGIN_DEGREES, WALL_IMPACT_MAX_SPEED_PX_SEC,
    WALL_IMPACT_MIN_SPEED_PX_SEC,
};
use crate::core::events::EngineEvent;
use crate::core::state::Stance;
use crate::core::visual::VisualTransform;
use crate::physics::ballistics::FlightState;
use crate::physics::collision::{LedgeSurface, ReachBounds, SquashState};
use crate::physics::reactions::{DazedState, SkidState};

use super::PhysicsEngine;

#[derive(Clone, Copy)]
struct ScreenBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[allow(deprecated)]
impl PhysicsEngine {
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
            Stance::Fly => self.step_flight(
                dt,
                ScreenBounds {
                    left: screen_left,
                    top: screen_top,
                    right: screen_right,
                    bottom: screen_bottom,
                },
                ledges,
                &mut events,
            ),
            Stance::Skid => self.step_skid(dt),
            Stance::Dazed => self.step_dazed(dt),
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

    fn step_flight(
        &mut self,
        dt: f32,
        screen: ScreenBounds,
        ledges: &[LedgeSurface],
        events: &mut Vec<EngineEvent>,
    ) {
        let mut should_land = None;
        let mut impact_squash = None;

        if let Some(flight) = &mut self.flight {
            flight.update(dt);
            self.center_pos = flight.center;
            self.rotation = flight.angle;

            let reach = ReachBounds::compute(flight.angle);

            if flight.center.x + reach.min_x < screen.left && flight.velocity.x < 0.0 {
                flight.center.x = screen.left - reach.min_x;
                let impact_speed = flight.velocity.length();
                let hard = (flight.velocity.x.abs() - WALL_IMPACT_MIN_SPEED_PX_SEC)
                    .clamp(0.0, WALL_IMPACT_MAX_SPEED_PX_SEC)
                    / WALL_IMPACT_MAX_SPEED_PX_SEC;
                flight.bounce(Vec2::new(1.0, 0.0), hard);
                impact_squash = Some((Vec2::new(1.0, 0.0), hard));
                events.push(EngineEvent::Impact {
                    normal: Vec2::new(1.0, 0.0),
                    speed: impact_speed,
                    point: Vec2::new(screen.left, flight.center.y),
                    window_title: None,
                });
            } else if flight.center.x + reach.max_x > screen.right && flight.velocity.x > 0.0 {
                flight.center.x = screen.right - reach.max_x;
                let impact_speed = flight.velocity.length();
                let hard = (flight.velocity.x.abs() - WALL_IMPACT_MIN_SPEED_PX_SEC)
                    .clamp(0.0, WALL_IMPACT_MAX_SPEED_PX_SEC)
                    / WALL_IMPACT_MAX_SPEED_PX_SEC;
                flight.bounce(Vec2::new(-1.0, 0.0), hard);
                impact_squash = Some((Vec2::new(-1.0, 0.0), hard));
                events.push(EngineEvent::Impact {
                    normal: Vec2::new(-1.0, 0.0),
                    speed: impact_speed,
                    point: Vec2::new(screen.right, flight.center.y),
                    window_title: None,
                });
            }

            if flight.center.y + reach.min_y < screen.top && flight.velocity.y < 0.0 {
                flight.center.y = screen.top - reach.min_y;
                let impact_speed = flight.velocity.length();
                let hard = (flight.velocity.y.abs() - WALL_IMPACT_MIN_SPEED_PX_SEC)
                    .clamp(0.0, WALL_IMPACT_MAX_SPEED_PX_SEC)
                    / WALL_IMPACT_MAX_SPEED_PX_SEC;
                flight.bounce(Vec2::new(0.0, 1.0), hard);
                impact_squash = Some((Vec2::new(0.0, 1.0), hard));
                events.push(EngineEvent::Impact {
                    normal: Vec2::new(0.0, 1.0),
                    speed: impact_speed,
                    point: Vec2::new(flight.center.x, screen.top),
                    window_title: None,
                });
            }

            let ground_y = screen.bottom - FLOOR_SURFACE_OFFSET_PX;
            let feet_y = flight.center.y + reach.max_y;

            let mut hit_surface = None;
            if flight.velocity.y > 0.0 {
                for ledge in ledges {
                    if ledge.contains_x(flight.center.x, SPRITE_WIDTH * 0.25)
                        && feet_y >= ledge.y
                        && feet_y - flight.velocity.y * dt <= ledge.y + LEDGE_LANDING_TOLERANCE_PX
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

        if let Some((target_y, reach_max_y, is_upright, vy, vx, speed, hit_surface)) = should_land {
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
                self.skid = Some(SkidState::new(Vec2::new(self.center_pos.x, target_y), vx));
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

    fn step_skid(&mut self, dt: f32) {
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

    fn step_dazed(&mut self, dt: f32) {
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
