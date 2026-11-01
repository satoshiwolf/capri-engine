use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    Grabbed {
        screen_pos: Vec2,
        grab_offset: Vec2,
    },
    Dropped {
        screen_pos: Vec2,
    },
    Thrown {
        velocity: Vec2,
        speed: f32,
    },
    Impact {
        normal: Vec2,
        speed: f32,
        point: Vec2,
        window_title: Option<String>,
    },
    Landed {
        surface_y: f32,
        window_title: Option<String>,
        skid: bool,
    },
    Fell,
    MonitorChanged {
        monitor_index: usize,
        name: String,
    },
    Speak {
        text: String,
        duration_ms: u64,
    },
    /// Viseme values map from 0 (closed) through 4 (fully open).
    LipSync {
        viseme: u8,
    },
}
