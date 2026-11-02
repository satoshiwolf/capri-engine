use glam::Vec2;

use crate::core::config::{FEET_POINT, MID_POINT};
use crate::core::events::EngineEvent;
use crate::core::state::Stance;
use crate::physics::ballistics::FlightState;
use crate::physics::collision::LedgeSurface;

use super::PhysicsEngine;

#[allow(deprecated)]
#[test]
fn slow_release_enters_fall_instead_of_hovering() {
    let mut physics = PhysicsEngine::new(Vec2::new(100.0, 100.0));
    physics.pick_up(Vec2::new(100.0, 100.0));
    physics.release();

    assert_eq!(physics.stance, Stance::Fly);
    assert!(physics.flight.is_some());
}

#[allow(deprecated)]
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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
