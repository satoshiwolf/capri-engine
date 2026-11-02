use glam::Vec2;
use std::collections::VecDeque;

use crate::core::state::{Facing, Stance};
use crate::physics::ballistics::FlightState;
use crate::physics::collision::{LedgeSurface, SquashState};
use crate::physics::pendulum::PendulumState;
use crate::physics::reactions::{DazedState, SkidState};

mod simulation;
mod transitions;

#[cfg(test)]
mod tests;

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
}
