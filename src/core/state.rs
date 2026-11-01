use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    Stand,
    Sit,
    LayDown,
    Held,
    Fly,
    Skid,
    Dazed,
    Upside,
    Corner,
    Ride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Facing {
    Right = 1,
    Left = -1,
}

impl Facing {
    pub fn as_f32(&self) -> f32 {
        match self {
            Facing::Right => 1.0,
            Facing::Left => -1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Emotion {
    Neutral,
    Happy,
    Jealous,
    Angry,
    Sleepy,
    Focused,
}
