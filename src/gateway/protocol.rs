use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IncomingCommand {
    #[serde(rename = "action")]
    Action {
        /// Known values include `perch_on_window`, `hang_on_cursor`, and `hang_on_ceiling`.
        action: String,
        target: Option<String>,
    },
    #[serde(rename = "speak")]
    Speak {
        text: String,
        duration_ms: Option<u64>,
    },
    #[serde(rename = "lip_sync")]
    LipSync {
        /// Viseme range from 0 (closed) to 4 (fully open).
        viseme: u8,
    },
    #[serde(rename = "emotion")]
    Emotion {
        /// Accepted values are `happy`, `jealous`, `angry`, `sleepy`, `focused`, and `neutral`.
        state: String,
    },
    #[serde(rename = "point_at")]
    PointAt { screen: usize, x: f32, y: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum OutgoingEvent {
    #[serde(rename = "avatar_grabbed")]
    AvatarGrabbed { x: f32, y: f32 },

    #[serde(rename = "avatar_thrown")]
    AvatarThrown { speed: f32, vx: f32, vy: f32 },

    #[serde(rename = "avatar_landed")]
    AvatarLanded {
        surface: String,
        window_title: Option<String>,
        x: f32,
        y: f32,
        skid: bool,
    },

    #[serde(rename = "avatar_impact")]
    AvatarImpact {
        speed: f32,
        normal_x: f32,
        normal_y: f32,
        window_title: Option<String>,
    },

    #[serde(rename = "monitor_changed")]
    MonitorChanged {
        screen_index: usize,
        screen_name: String,
    },

    #[serde(rename = "state_changed")]
    StateChanged { stance: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn incoming_commands_keep_stable_wire_tags_and_fields() {
        assert_eq!(
            serde_json::to_value(IncomingCommand::Action {
                action: "sleep".to_string(),
                target: Some("Editor".to_string()),
            })
            .expect("serialize action"),
            json!({
                "type": "action",
                "action": "sleep",
                "target": "Editor"
            })
        );
        assert_eq!(
            serde_json::to_value(IncomingCommand::Speak {
                text: "hello".to_string(),
                duration_ms: Some(1500),
            })
            .expect("serialize speak"),
            json!({
                "type": "speak",
                "text": "hello",
                "duration_ms": 1500
            })
        );
        assert_eq!(
            serde_json::to_value(IncomingCommand::LipSync { viseme: 3 })
                .expect("serialize lip sync"),
            json!({"type": "lip_sync", "viseme": 3})
        );
        assert_eq!(
            serde_json::to_value(IncomingCommand::Emotion {
                state: "happy".to_string(),
            })
            .expect("serialize emotion"),
            json!({"type": "emotion", "state": "happy"})
        );
        assert_eq!(
            serde_json::to_value(IncomingCommand::PointAt {
                screen: 1,
                x: 120.5,
                y: -40.0,
            })
            .expect("serialize point at"),
            json!({
                "type": "point_at",
                "screen": 1,
                "x": 120.5,
                "y": -40.0
            })
        );
    }

    #[test]
    fn outgoing_events_keep_stable_wire_tags_and_fields() {
        assert_eq!(
            serde_json::to_value(OutgoingEvent::AvatarGrabbed { x: 10.0, y: 20.0 })
                .expect("serialize grabbed"),
            json!({"event": "avatar_grabbed", "x": 10.0, "y": 20.0})
        );
        assert_eq!(
            serde_json::to_value(OutgoingEvent::AvatarThrown {
                speed: 900.0,
                vx: 500.0,
                vy: -300.0,
            })
            .expect("serialize thrown"),
            json!({
                "event": "avatar_thrown",
                "speed": 900.0,
                "vx": 500.0,
                "vy": -300.0
            })
        );
        assert_eq!(
            serde_json::to_value(OutgoingEvent::AvatarLanded {
                surface: "window".to_string(),
                window_title: Some("Editor".to_string()),
                x: 40.0,
                y: 50.0,
                skid: true,
            })
            .expect("serialize landed"),
            json!({
                "event": "avatar_landed",
                "surface": "window",
                "window_title": "Editor",
                "x": 40.0,
                "y": 50.0,
                "skid": true
            })
        );
        assert_eq!(
            serde_json::to_value(OutgoingEvent::AvatarImpact {
                speed: 1200.0,
                normal_x: -1.0,
                normal_y: 0.0,
                window_title: None,
            })
            .expect("serialize impact"),
            json!({
                "event": "avatar_impact",
                "speed": 1200.0,
                "normal_x": -1.0,
                "normal_y": 0.0,
                "window_title": null
            })
        );
        assert_eq!(
            serde_json::to_value(OutgoingEvent::MonitorChanged {
                screen_index: 2,
                screen_name: "DISPLAY3".to_string(),
            })
            .expect("serialize monitor"),
            json!({
                "event": "monitor_changed",
                "screen_index": 2,
                "screen_name": "DISPLAY3"
            })
        );
        assert_eq!(
            serde_json::to_value(OutgoingEvent::StateChanged {
                stance: "held".to_string(),
            })
            .expect("serialize state"),
            json!({"event": "state_changed", "stance": "held"})
        );
    }

    #[test]
    fn protocol_accepts_optional_and_future_fields() {
        let action: IncomingCommand = serde_json::from_value(json!({
            "type": "action",
            "action": "wake",
            "future_field": true
        }))
        .expect("deserialize action");
        let point: IncomingCommand = serde_json::from_value(json!({
            "type": "point_at",
            "screen": 0,
            "x": 1.0,
            "y": 2.0,
            "future_field": "ignored"
        }))
        .expect("deserialize point");

        assert!(matches!(
            action,
            IncomingCommand::Action { target: None, .. }
        ));
        assert!(matches!(point, IncomingCommand::PointAt { screen: 0, .. }));
    }

    #[test]
    fn protocol_rejects_missing_required_fields_and_unknown_tags() {
        assert!(
            serde_json::from_value::<IncomingCommand>(json!({
                "type": "speak",
                "duration_ms": 100
            }))
            .is_err()
        );
        assert!(serde_json::from_value::<IncomingCommand>(json!({"type": "unknown"})).is_err());
        assert!(serde_json::from_value::<OutgoingEvent>(json!({"event": "unknown"})).is_err());
    }
}
