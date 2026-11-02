use glam::Vec2;

use crate::core::state::Emotion;
use crate::desktop::monitors::MonitorInfo;
use crate::gateway::protocol::IncomingCommand;
use crate::physics::PhysicsWorker;
use crate::physics::collision::LedgeSurface;
use crate::render::animation::{AnimationAction, AnimationController};

pub struct CommandContext<'a> {
    pub worker: &'a PhysicsWorker,
    pub animation: &'a mut AnimationController,
    pub surfaces: &'a [LedgeSurface],
    pub monitors: &'a [MonitorInfo],
    pub screen_top: f32,
}

pub fn dispatch_command(command: IncomingCommand, context: &mut CommandContext<'_>) {
    match command {
        IncomingCommand::Action { action, target } => {
            println!(
                "[GATEWAY COMMAND] Acción solicitada: {} (target: {:?})",
                action, target
            );
            match action.as_str() {
                "perch_on_window" => {
                    if let Some(surface) =
                        find_target_surface(target.as_deref(), context.surfaces).cloned()
                        && !context.worker.perch_on(surface)
                    {
                        eprintln!("[GATEWAY COMMAND] Target ledge is too narrow");
                    }
                }
                "hang_on_ceiling" => {
                    if !context.worker.hang_on_ceiling(context.screen_top) {
                        eprintln!("[GATEWAY COMMAND] Could not request ceiling hang");
                    }
                }
                "hang_on_cursor" => {
                    if !context.worker.ride_cursor() {
                        eprintln!("[GATEWAY COMMAND] Could not request cursor ride");
                    }
                }
                "sleep" => {
                    if !context.worker.sleep() {
                        eprintln!("[GATEWAY COMMAND] Could not request sleep");
                    }
                }
                "wake" => {
                    if context.worker.wake() {
                        context.animation.play_action(AnimationAction::Wake);
                    } else {
                        eprintln!("[GATEWAY COMMAND] Could not request wake");
                    }
                }
                "arrive" => context.animation.play_action(AnimationAction::Arrive),
                "leave" => context.animation.play_action(AnimationAction::Leave),
                "reset_stand" => {
                    report_queue_result(context.worker.reset_stand(), "Could not request reset");
                }
                _ => {}
            }
        }
        IncomingCommand::Speak { text, duration_ms } => {
            println!(
                "[GATEWAY COMMAND] Hablar: '{}' (duración: {:?}ms)",
                text, duration_ms
            );
            context
                .animation
                .begin_speech(duration_ms.unwrap_or(2500) as f32 / 1000.0);
        }
        IncomingCommand::LipSync { viseme } => context.animation.set_viseme(viseme),
        IncomingCommand::Emotion { state } => {
            println!("[GATEWAY COMMAND] Emoción cambiada a: {}", state);
            context.animation.set_emotion(parse_emotion(&state));
        }
        IncomingCommand::PointAt { screen, x, y } => {
            println!(
                "[GATEWAY COMMAND] Señalar pantalla {} en ({}, {})",
                screen, x, y
            );
            let target = resolve_point_at(screen, x, y, context.monitors);
            if !context.worker.face_toward(target.x) {
                eprintln!("[GATEWAY COMMAND] Could not request point-at");
            }
        }
    }
}

fn report_queue_result(queued: bool, message: &str) {
    if !queued {
        eprintln!("[GATEWAY COMMAND] {message}");
    }
}

pub fn find_target_surface<'a>(
    target: Option<&str>,
    surfaces: &'a [LedgeSurface],
) -> Option<&'a LedgeSurface> {
    let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) else {
        return surfaces.first();
    };
    if let Ok(hwnd) = target.parse::<isize>()
        && let Some(surface) = surfaces.iter().find(|surface| surface.hwnd == hwnd)
    {
        return Some(surface);
    }
    surfaces.iter().find(|surface| {
        surface
            .window_title
            .as_deref()
            .is_some_and(|title| title.eq_ignore_ascii_case(target))
    })
}

pub fn resolve_point_at(screen: usize, x: f32, y: f32, monitors: &[MonitorInfo]) -> Vec2 {
    let Some(monitor) = monitors.get(screen) else {
        return Vec2::new(x, y);
    };
    let (left, top, right, bottom) = monitor.bounds;
    let global = Vec2::new(x, y);
    if monitor.contains_point(global) {
        return global;
    }
    let local = Vec2::new(left as f32 + x, top as f32 + y);
    let is_local = x >= 0.0 && y >= 0.0 && x < (right - left) as f32 && y < (bottom - top) as f32;
    if is_local { local } else { global }
}

fn parse_emotion(value: &str) -> Emotion {
    match value.to_ascii_lowercase().as_str() {
        "happy" => Emotion::Happy,
        "jealous" => Emotion::Jealous,
        "angry" => Emotion::Angry,
        "sleepy" => Emotion::Sleepy,
        "focused" => Emotion::Focused,
        _ => Emotion::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(hwnd: isize, title: &str, left: f32, right: f32) -> LedgeSurface {
        LedgeSurface {
            hwnd,
            left,
            right,
            y: 200.0,
            window_title: Some(title.to_string()),
        }
    }

    #[test]
    fn action_target_resolves_hwnd_title_and_default() {
        let surfaces = [
            surface(11, "Editor", 0.0, 300.0),
            surface(22, "Browser", 300.0, 600.0),
        ];

        assert_eq!(
            find_target_surface(None, &surfaces).map(|value| value.hwnd),
            Some(11)
        );
        assert_eq!(
            find_target_surface(Some("22"), &surfaces).map(|value| value.hwnd),
            Some(22)
        );
        assert_eq!(
            find_target_surface(Some("browser"), &surfaces).map(|value| value.hwnd),
            Some(22)
        );
        assert!(find_target_surface(Some("missing"), &surfaces).is_none());
    }

    #[test]
    fn point_at_accepts_global_or_monitor_local_coordinates() {
        let monitors = [MonitorInfo {
            name: "test".to_string(),
            bounds: (100, 50, 900, 650),
            work_area: (100, 50, 900, 650),
            is_primary: true,
        }];

        assert_eq!(
            resolve_point_at(0, 400.0, 300.0, &monitors),
            Vec2::new(400.0, 300.0)
        );
        assert_eq!(
            resolve_point_at(0, 50.0, 40.0, &monitors),
            Vec2::new(150.0, 90.0)
        );
        assert_eq!(
            resolve_point_at(4, 50.0, 40.0, &monitors),
            Vec2::new(50.0, 40.0)
        );
    }
}
