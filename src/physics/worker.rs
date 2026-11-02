use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use glam::Vec2;

use crate::core::events::EngineEvent;
use crate::core::state::{Facing, Stance};
use crate::core::visual::VisualTransform;
use crate::physics::PhysicsEngine;
use crate::physics::ballistics::FlightState;
use crate::physics::collision::LedgeSurface;
use crate::physics::reactions::SkidState;

const COMMAND_CAPACITY: usize = 64;
const OUTPUT_CAPACITY: usize = 8;

#[derive(Debug, Clone)]
pub struct FlightSnapshot {
    pub velocity: Vec2,
    pub release_streak: u8,
}

#[derive(Debug, Clone)]
pub struct PhysicsSnapshot {
    stance: Stance,
    center_pos: Vec2,
    rotation: f32,
    scale: f32,
    squash_scale: Vec2,
    facing: Facing,
    flight: Option<FlightSnapshot>,
    skid: Option<SkidState>,
    has_landing_surface: bool,
}

impl PhysicsSnapshot {
    pub fn initial(center_pos: Vec2) -> Self {
        Self {
            stance: Stance::Stand,
            center_pos,
            rotation: 0.0,
            scale: 1.0,
            squash_scale: Vec2::ONE,
            facing: Facing::Right,
            flight: None,
            skid: None,
            has_landing_surface: false,
        }
    }

    fn from_engine(engine: &PhysicsEngine) -> Self {
        Self {
            stance: engine.stance(),
            center_pos: engine.center_pos(),
            rotation: engine.rotation(),
            scale: engine.scale(),
            squash_scale: engine.squash_scale(),
            facing: engine.facing(),
            flight: engine.flight().map(FlightSnapshot::from_flight),
            skid: engine.skid().cloned(),
            has_landing_surface: engine.has_landing_surface(),
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

    pub fn facing(&self) -> Facing {
        self.facing
    }

    pub fn flight(&self) -> Option<&FlightSnapshot> {
        self.flight.as_ref()
    }

    pub fn skid(&self) -> Option<&SkidState> {
        self.skid.as_ref()
    }

    pub fn has_landing_surface(&self) -> bool {
        self.has_landing_surface
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
}

impl FlightSnapshot {
    fn from_flight(flight: &FlightState) -> Self {
        Self {
            velocity: flight.velocity,
            release_streak: flight.release_streak,
        }
    }
}

#[derive(Debug)]
pub struct PhysicsOutput {
    pub snapshot: PhysicsSnapshot,
    pub events: Vec<EngineEvent>,
}

enum PhysicsCommand {
    SetSurfaces(Vec<LedgeSurface>),
    PickUp(Vec2),
    UpdateHeld {
        cursor_pos: Vec2,
        timestamp: f64,
        dt: f32,
    },
    Release,
    UpdateRide {
        cursor_pos: Vec2,
        cursor_velocity: Vec2,
        dt: f32,
    },
    HangOnCeiling(f32),
    RideCursor,
    Sleep,
    Wake,
    ResetStand,
    FaceToward(f32),
    PerchOn(LedgeSurface),
}

pub struct PhysicsWorker {
    command_tx: Option<SyncSender<PhysicsCommand>>,
    output_rx: Option<Receiver<PhysicsOutput>>,
    worker: Option<JoinHandle<()>>,
}

impl PhysicsWorker {
    pub fn spawn(initial_pos: Vec2, screen_bounds: (f32, f32, f32, f32), tick_hz: u32) -> Self {
        let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (output_tx, output_rx) = mpsc::sync_channel(OUTPUT_CAPACITY);
        let worker = thread::spawn(move || {
            run_worker(
                PhysicsEngine::new(initial_pos),
                screen_bounds,
                tick_hz,
                command_rx,
                output_tx,
            );
        });

        Self {
            command_tx: Some(command_tx),
            output_rx: Some(output_rx),
            worker: Some(worker),
        }
    }

    pub fn set_surfaces(&self, surfaces: Vec<LedgeSurface>) -> bool {
        self.send(PhysicsCommand::SetSurfaces(surfaces))
    }

    pub fn pick_up(&self, cursor_pos: Vec2) -> bool {
        self.send(PhysicsCommand::PickUp(cursor_pos))
    }

    pub fn update_held(&self, cursor_pos: Vec2, timestamp: f64, dt: f32) -> bool {
        self.send(PhysicsCommand::UpdateHeld {
            cursor_pos,
            timestamp,
            dt,
        })
    }

    pub fn release(&self) -> bool {
        self.send(PhysicsCommand::Release)
    }

    pub fn update_ride(&self, cursor_pos: Vec2, cursor_velocity: Vec2, dt: f32) -> bool {
        self.send(PhysicsCommand::UpdateRide {
            cursor_pos,
            cursor_velocity,
            dt,
        })
    }

    pub fn hang_on_ceiling(&self, screen_top: f32) -> bool {
        self.send(PhysicsCommand::HangOnCeiling(screen_top))
    }

    pub fn ride_cursor(&self) -> bool {
        self.send(PhysicsCommand::RideCursor)
    }

    pub fn sleep(&self) -> bool {
        self.send(PhysicsCommand::Sleep)
    }

    pub fn wake(&self) -> bool {
        self.send(PhysicsCommand::Wake)
    }

    pub fn reset_stand(&self) -> bool {
        self.send(PhysicsCommand::ResetStand)
    }

    pub fn face_toward(&self, x: f32) -> bool {
        self.send(PhysicsCommand::FaceToward(x))
    }

    pub fn perch_on(&self, surface: LedgeSurface) -> bool {
        self.send(PhysicsCommand::PerchOn(surface))
    }

    pub fn try_recv(&self) -> Result<PhysicsOutput, TryRecvError> {
        self.output_rx
            .as_ref()
            .map_or(Err(TryRecvError::Disconnected), Receiver::try_recv)
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<PhysicsOutput, RecvTimeoutError> {
        self.output_rx
            .as_ref()
            .map_or(Err(RecvTimeoutError::Disconnected), |receiver| {
                receiver.recv_timeout(timeout)
            })
    }

    fn send(&self, command: PhysicsCommand) -> bool {
        self.command_tx
            .as_ref()
            .is_some_and(|sender| sender.try_send(command).is_ok())
    }
}

impl Drop for PhysicsWorker {
    fn drop(&mut self) {
        self.command_tx.take();
        self.output_rx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct FixedStepClock {
    tick_duration: Duration,
    next_tick: Instant,
}

impl FixedStepClock {
    fn new(tick_hz: u32, now: Instant) -> Self {
        let tick_duration = Duration::from_nanos(1_000_000_000 / tick_hz.max(1) as u64);
        Self {
            tick_duration,
            next_tick: now + tick_duration,
        }
    }

    fn steps_due(&mut self, now: Instant) -> usize {
        if now < self.next_tick {
            return 0;
        }
        let elapsed = now - self.next_tick;
        let steps =
            (elapsed.as_nanos() / self.tick_duration.as_nanos() + 1).min(u32::MAX as u128) as u32;
        self.next_tick += self.tick_duration * steps;
        (steps as usize).min(8)
    }

    fn time_until_next(&self, now: Instant) -> Duration {
        self.next_tick.saturating_duration_since(now)
    }
}

fn run_worker(
    mut engine: PhysicsEngine,
    (screen_left, screen_top, screen_right, screen_bottom): (f32, f32, f32, f32),
    tick_hz: u32,
    command_rx: Receiver<PhysicsCommand>,
    output_tx: SyncSender<PhysicsOutput>,
) {
    let mut clock = FixedStepClock::new(tick_hz, Instant::now());
    let mut surfaces = Vec::new();
    let mut pending_events = Vec::new();

    loop {
        while let Ok(command) = command_rx.try_recv() {
            apply_command(
                command,
                &mut engine,
                &screen_bounds(screen_left, screen_top, screen_right, screen_bottom),
                &mut surfaces,
                &mut pending_events,
            );
        }

        let now = Instant::now();
        let steps = clock.steps_due(now);
        for _ in 0..steps {
            let mut events = engine.step_with_surfaces(
                clock.tick_duration.as_secs_f32(),
                screen_left,
                screen_top,
                screen_right,
                screen_bottom,
                &surfaces,
            );
            events.append(&mut pending_events);
            let output = PhysicsOutput {
                snapshot: PhysicsSnapshot::from_engine(&engine),
                events,
            };
            if output_tx.send(output).is_err() {
                return;
            }
        }

        let timeout = clock.time_until_next(Instant::now());
        match command_rx.recv_timeout(timeout) {
            Ok(command) => apply_command(
                command,
                &mut engine,
                &screen_bounds(screen_left, screen_top, screen_right, screen_bottom),
                &mut surfaces,
                &mut pending_events,
            ),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn screen_bounds(left: f32, top: f32, right: f32, bottom: f32) -> (f32, f32, f32, f32) {
    (left, top, right, bottom)
}

fn apply_command(
    command: PhysicsCommand,
    engine: &mut PhysicsEngine,
    screen_bounds: &(f32, f32, f32, f32),
    surfaces: &mut Vec<LedgeSurface>,
    pending_events: &mut Vec<EngineEvent>,
) {
    match command {
        PhysicsCommand::SetSurfaces(next) => *surfaces = next,
        PhysicsCommand::PickUp(cursor_pos) => pending_events.push(engine.pick_up(cursor_pos)),
        PhysicsCommand::UpdateHeld {
            cursor_pos,
            timestamp,
            dt,
        } => engine.update_held(cursor_pos, timestamp, dt),
        PhysicsCommand::Release => pending_events.push(engine.release()),
        PhysicsCommand::UpdateRide {
            cursor_pos,
            cursor_velocity,
            dt,
        } => engine.update_ride(cursor_pos, cursor_velocity, dt),
        PhysicsCommand::HangOnCeiling(screen_top) => engine.hang_on_ceiling(screen_top),
        PhysicsCommand::RideCursor => engine.ride_cursor(),
        PhysicsCommand::Sleep => {
            engine.sleep(screen_bounds.0, screen_bounds.2, screen_bounds.3);
        }
        PhysicsCommand::Wake => engine.wake(),
        PhysicsCommand::ResetStand => engine.reset_stand(),
        PhysicsCommand::FaceToward(x) => engine.face_toward(x),
        PhysicsCommand::PerchOn(surface) => {
            engine.perch_on(surface);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_produces_60_and_120_steps_per_second() {
        let start = Instant::now();
        let mut at_60 = FixedStepClock::new(60, start);
        let mut at_120 = FixedStepClock::new(120, start);
        let end = start + Duration::from_secs(1);

        let mut steps_60 = 0;
        let mut steps_120 = 0;
        let mut now = start;
        while now < end {
            now += Duration::from_millis(1);
            steps_60 += at_60.steps_due(now);
            steps_120 += at_120.steps_due(now);
        }

        assert_eq!(steps_60, 60);
        assert_eq!(steps_120, 120);
    }

    #[test]
    fn worker_applies_commands_and_emits_snapshots() {
        let worker = PhysicsWorker::spawn(Vec2::new(200.0, 200.0), (0.0, 0.0, 800.0, 600.0), 120);
        assert!(worker.pick_up(Vec2::new(200.0, 200.0)));

        let mut saw_grabbed = false;
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            let output = worker
                .recv_timeout(Duration::from_millis(100))
                .expect("physics output");
            saw_grabbed |= output
                .events
                .iter()
                .any(|event| matches!(event, EngineEvent::Grabbed { .. }));
            if saw_grabbed && output.snapshot.stance() == Stance::Held {
                return;
            }
        }
        panic!("worker did not publish grabbed snapshot");
    }

    #[test]
    fn worker_sleep_survives_multiple_ticks_and_shuts_down() {
        let worker = PhysicsWorker::spawn(Vec2::new(200.0, 200.0), (0.0, 0.0, 800.0, 600.0), 60);
        assert!(worker.sleep());

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut saw_laydown = false;
        while Instant::now() < deadline {
            let output = worker
                .recv_timeout(Duration::from_millis(100))
                .expect("physics output");
            if output.snapshot.stance() == Stance::LayDown {
                saw_laydown = true;
                break;
            }
        }
        assert!(saw_laydown);
        drop(worker);
    }
}
