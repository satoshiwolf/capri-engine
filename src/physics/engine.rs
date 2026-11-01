use glam::Vec2;
use std::collections::VecDeque;

use crate::core::config::{
    MID_POINT, SPRITE_HEIGHT, SPRITE_WIDTH, THROW_MAX_SPEED, THROW_MIN_SPEED,
};
use crate::core::events::EngineEvent;
use crate::core::state::{Facing, Stance};
use crate::physics::ballistics::FlightState;
use crate::physics::collision::{ReachBounds, SquashState};
use crate::physics::pendulum::PendulumState;
use crate::physics::reactions::{DazedState, SkidState};

#[derive(Debug, Clone, Copy)]
struct PointerSample {
    pos: Vec2,
    timestamp: f64,
}

pub struct PhysicsEngine {
    pub stance: Stance,
    pub center_pos: Vec2,
    pub rotation: f32,
    pub scale: f32,
    pub facing: Facing,

    pub pendulum: Option<PendulumState>,
    pub flight: Option<FlightState>,
    pub squash: Option<SquashState>,
    pub skid: Option<SkidState>,
    pub dazed: Option<DazedState>,

    pointer_samples: VecDeque<PointerSample>,
    last_cursor_pos: Vec2,
}

impl PhysicsEngine {
    pub fn new(initial_pos: Vec2) -> Self {
        Self {
            stance: Stance::Stand,
            center_pos: initial_pos,
            rotation: 0.0,
            scale: 1.0,
            facing: Facing::Right,
            pendulum: None,
            flight: None,
            squash: None,
            skid: None,
            dazed: None,
            pointer_samples: VecDeque::with_capacity(16),
            last_cursor_pos: initial_pos,
        }
    }

    pub fn pick_up(&mut self, cursor_pos: Vec2) -> EngineEvent {
        self.stance = Stance::Held;
        self.flight = None;
        self.skid = None;
        self.dazed = None;

        let top_left = self.center_pos - MID_POINT * self.scale;
        let local_grab = (cursor_pos - top_left) / self.scale;

        let grab_point = Vec2::new(
            local_grab.x.clamp(20.0, SPRITE_WIDTH - 20.0),
            local_grab.y.clamp(20.0, SPRITE_HEIGHT - 20.0),
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
        while self.pointer_samples.len() > 10 {
            self.pointer_samples.pop_front();
        }

        let cursor_velocity = self.calculate_pointer_velocity();

        if let Some(pendulum) = &mut self.pendulum {
            pendulum.update(cursor_velocity, dt);
            self.rotation = pendulum.angle;

            // Keep the selected grab point fixed beneath the cursor.
            let grab_offset = (pendulum.grab_point - MID_POINT) * self.scale;
            self.center_pos = cursor_pos - grab_offset;
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
            self.flight = Some(flight);

            EngineEvent::Thrown {
                velocity,
                speed: speed_val,
            }
        } else {
            self.stance = Stance::Stand;
            self.rotation = 0.0;
            EngineEvent::Dropped {
                screen_pos: self.center_pos,
            }
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
        let mut events = Vec::new();

        if let Some(squash) = &mut self.squash
            && squash.update().is_none()
        {
            self.squash = None;
        }

        match self.stance {
            Stance::Fly => {
                let mut should_land = None;

                if let Some(flight) = &mut self.flight {
                    flight.update(dt);
                    self.center_pos = flight.center;
                    self.rotation = flight.angle;

                    let reach = ReachBounds::compute(flight.angle);

                    if flight.center.x + reach.min_x < screen_left && flight.velocity.x < 0.0 {
                        flight.center.x = screen_left - reach.min_x;
                        let hard = (flight.velocity.x.abs() - 250.0).clamp(0.0, 2300.0) / 2300.0;
                        flight.bounce(Vec2::new(1.0, 0.0), hard);
                        self.squash = Some(SquashState::new(Vec2::new(1.0, 0.0), hard));
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(1.0, 0.0),
                            speed: flight.velocity.length(),
                            point: Vec2::new(screen_left, flight.center.y),
                            window_title: None,
                        });
                    } else if flight.center.x + reach.max_x > screen_right
                        && flight.velocity.x > 0.0
                    {
                        flight.center.x = screen_right - reach.max_x;
                        let hard = (flight.velocity.x.abs() - 250.0).clamp(0.0, 2300.0) / 2300.0;
                        flight.bounce(Vec2::new(-1.0, 0.0), hard);
                        self.squash = Some(SquashState::new(Vec2::new(-1.0, 0.0), hard));
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(-1.0, 0.0),
                            speed: flight.velocity.length(),
                            point: Vec2::new(screen_right, flight.center.y),
                            window_title: None,
                        });
                    }

                    if flight.center.y + reach.min_y < screen_top && flight.velocity.y < 0.0 {
                        flight.center.y = screen_top - reach.min_y;
                        let hard = (flight.velocity.y.abs() - 250.0).clamp(0.0, 2300.0) / 2300.0;
                        flight.bounce(Vec2::new(0.0, 1.0), hard);
                        self.squash = Some(SquashState::new(Vec2::new(0.0, 1.0), hard));
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(0.0, 1.0),
                            speed: flight.velocity.length(),
                            point: Vec2::new(flight.center.x, screen_top),
                            window_title: None,
                        });
                    }

                    let ground_y = screen_bottom - 2.0;
                    let feet_y = flight.center.y + reach.max_y;

                    let mut hit_surface_y = None;
                    if flight.velocity.y > 0.0 {
                        for &ledge_y in ledges_y {
                            if feet_y >= ledge_y
                                && feet_y - flight.velocity.y * dt <= ledge_y + 10.0
                            {
                                hit_surface_y = Some(ledge_y);
                                break;
                            }
                        }
                    }
                    if hit_surface_y.is_none() && feet_y >= ground_y {
                        hit_surface_y = Some(ground_y);
                    }

                    if let Some(target_y) = hit_surface_y {
                        let upright = flight.angle.rem_euclid(360.0);
                        let is_upright = upright < 35.0 || upright > 325.0;
                        let vy = flight.velocity.y;
                        let vx = flight.velocity.x;
                        let speed = flight.velocity.length();
                        let angle = flight.angle;
                        should_land =
                            Some((target_y, reach.max_y, is_upright, vy, vx, speed, angle));
                    }
                }

                if let Some((target_y, reach_max_y, is_upright, vy, vx, speed, angle)) = should_land
                {
                    self.center_pos.y = target_y - reach_max_y;
                    self.flight = None;

                    if is_upright && vy < 1200.0 {
                        self.stance = Stance::Skid;
                        self.rotation = 0.0;
                        self.skid =
                            Some(SkidState::new(Vec2::new(self.center_pos.x, target_y), vx));
                        events.push(EngineEvent::Landed {
                            surface_y: target_y,
                            window_title: None,
                            skid: true,
                        });
                    } else {
                        self.stance = Stance::Dazed;
                        self.rotation = if angle > 180.0 { 270.0 } else { 90.0 };
                        self.dazed = Some(DazedState::new());
                        events.push(EngineEvent::Impact {
                            normal: Vec2::new(0.0, -1.0),
                            speed,
                            point: Vec2::new(self.center_pos.x, target_y),
                            window_title: None,
                        });
                    }
                }
            }
            Stance::Skid => {
                if let Some(skid) = &mut self.skid {
                    let finished = skid.update(dt);
                    self.center_pos.x = skid.feet_pos.x;
                    if finished {
                        self.stance = Stance::Stand;
                        self.skid = None;
                    }
                }
            }
            Stance::Dazed => {
                if let Some(dazed) = &mut self.dazed {
                    let recovered = dazed.update(dt);
                    if recovered {
                        self.stance = Stance::Stand;
                        self.rotation = 0.0;
                        self.dazed = None;
                    }
                }
            }
            _ => {}
        }

        events
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
}
