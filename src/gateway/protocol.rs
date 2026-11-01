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
