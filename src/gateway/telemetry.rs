use tokio::sync::broadcast;

use crate::core::events::EngineEvent;
use crate::gateway::protocol::OutgoingEvent;

pub fn send_engine_telemetry(
    event_tx: &broadcast::Sender<OutgoingEvent>,
    event: &EngineEvent,
    body_x: f32,
) {
    let outgoing = match event {
        EngineEvent::Grabbed { screen_pos, .. } => Some(OutgoingEvent::AvatarGrabbed {
            x: screen_pos.x,
            y: screen_pos.y,
        }),
        EngineEvent::Thrown { velocity, speed } => Some(OutgoingEvent::AvatarThrown {
            speed: *speed,
            vx: velocity.x,
            vy: velocity.y,
        }),
        EngineEvent::Impact {
            normal,
            speed,
            window_title,
            ..
        } => Some(OutgoingEvent::AvatarImpact {
            speed: *speed,
            normal_x: normal.x,
            normal_y: normal.y,
            window_title: window_title.clone(),
        }),
        EngineEvent::Landed {
            surface_y,
            window_title,
            skid,
        } => Some(OutgoingEvent::AvatarLanded {
            surface: if window_title.is_some() {
                "window".to_string()
            } else {
                "floor".to_string()
            },
            window_title: window_title.clone(),
            x: body_x,
            y: *surface_y,
            skid: *skid,
        }),
        _ => None,
    };

    if let Some(event) = outgoing {
        let _ = event_tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;
    use tokio::sync::broadcast;

    use super::*;

    #[test]
    fn impact_telemetry_preserves_contact_geometry_and_speed() {
        let (event_tx, mut event_rx) = broadcast::channel(2);
        let event = EngineEvent::Impact {
            normal: Vec2::new(-1.0, 0.0),
            speed: 900.0,
            point: Vec2::new(20.0, 40.0),
            window_title: Some("Editor".to_string()),
        };

        send_engine_telemetry(&event_tx, &event, 32.0);

        match event_rx.try_recv().expect("telemetry event") {
            OutgoingEvent::AvatarImpact {
                speed,
                normal_x,
                normal_y,
                window_title,
            } => {
                assert_eq!(speed, 900.0);
                assert_eq!((normal_x, normal_y), (-1.0, 0.0));
                assert_eq!(window_title.as_deref(), Some("Editor"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn landed_telemetry_distinguishes_floor_and_window() {
        let (event_tx, mut event_rx) = broadcast::channel(2);
        let event = EngineEvent::Landed {
            surface_y: 600.0,
            window_title: None,
            skid: false,
        };

        send_engine_telemetry(&event_tx, &event, 42.0);

        match event_rx.try_recv().expect("telemetry event") {
            OutgoingEvent::AvatarLanded {
                surface,
                window_title,
                x,
                y,
                skid,
            } => {
                assert_eq!(surface, "floor");
                assert_eq!(window_title, None);
                assert_eq!((x, y, skid), (42.0, 600.0, false));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
