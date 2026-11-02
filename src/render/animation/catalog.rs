use glam::Vec2;

use crate::core::config::MOUTH_FPS;
use crate::core::state::Stance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipId {
    Idle,
    Blink,
    BlinkGlitch,
    Grin,
    Glitch,
    Dangle,
    Sleep,
    Wake,
    Walk,
    Dance,
    Arrive,
    Leave,
    Perch,
    PerchTalk,
    PerchBlink,
    PerchBlinkGlitch,
    PerchGlitch,
    PerchMusic,
    PerchArrive,
    PerchLeave,
    Laydown,
    LaydownTalk,
    LaydownBlink,
    LaydownBlinkGlitch,
    LaydownSleep,
    LaydownGlitch,
    LaydownLeave,
    Mouth,
}

impl ClipId {
    pub const ALL: [Self; 28] = [
        Self::Idle,
        Self::Blink,
        Self::BlinkGlitch,
        Self::Grin,
        Self::Glitch,
        Self::Dangle,
        Self::Sleep,
        Self::Wake,
        Self::Walk,
        Self::Dance,
        Self::Arrive,
        Self::Leave,
        Self::Perch,
        Self::PerchTalk,
        Self::PerchBlink,
        Self::PerchBlinkGlitch,
        Self::PerchGlitch,
        Self::PerchMusic,
        Self::PerchArrive,
        Self::PerchLeave,
        Self::Laydown,
        Self::LaydownTalk,
        Self::LaydownBlink,
        Self::LaydownBlinkGlitch,
        Self::LaydownSleep,
        Self::LaydownGlitch,
        Self::LaydownLeave,
        Self::Mouth,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Loop,
    OneShot,
}

#[derive(Debug, Clone, Copy)]
pub struct ClipSpec {
    pub id: ClipId,
    pub filename: &'static str,
    pub flipped_filename: Option<&'static str>,
    pub frame_count: usize,
    pub frame_width: u32,
    pub frame_height: u32,
    pub fps: f32,
    pub loop_mode: LoopMode,
    pub pivot: Vec2,
    pub display_scale: f32,
}

impl ClipSpec {
    pub fn duration_secs(self) -> f32 {
        self.frame_count as f32 / self.fps.max(1.0)
    }

    pub fn frame_index_at(self, time_secs: f32) -> usize {
        if self.frame_count == 0 {
            return 0;
        }
        let signed_frame = (time_secs.max(0.0) * self.fps) as usize;
        match self.loop_mode {
            LoopMode::Loop => signed_frame % self.frame_count,
            LoopMode::OneShot => signed_frame.min(self.frame_count - 1),
        }
    }

    pub fn resident_mask_bytes(self) -> usize {
        self.frame_count * self.frame_width as usize * self.frame_height as usize
    }
}

pub struct ClipCatalog;

impl ClipCatalog {
    pub fn spec(id: ClipId) -> ClipSpec {
        use ClipId::*;
        let mut spec = match id {
            Idle => body("idle.png", 16, 8.0, LoopMode::Loop),
            Blink => body("blink.png", 4, 8.0, LoopMode::OneShot),
            BlinkGlitch => body("blink_glitch.png", 4, 8.0, LoopMode::OneShot),
            Grin => body("grin.png", 16, 8.0, LoopMode::OneShot),
            Glitch => body("glitch.png", 10, 15.0, LoopMode::OneShot),
            Dangle => body("dangle.png", 24, 10.0, LoopMode::Loop),
            Sleep => body("sleep.png", 16, 6.0, LoopMode::Loop),
            Wake => body("wake.png", 13, 15.0, LoopMode::OneShot),
            Walk => body("walk.png", 12, 12.0, LoopMode::Loop),
            Dance => body("dance.png", 16, 8.0, LoopMode::Loop),
            Arrive => body("arrive.png", 14, 15.0, LoopMode::OneShot),
            Leave => body("leave.png", 12, 15.0, LoopMode::OneShot),
            Perch => perch(
                "perch.png",
                Some("perch_flipped.png"),
                60,
                196,
                LoopMode::Loop,
            ),
            PerchTalk => perch(
                "perch_talk.png",
                Some("perch_talk_flipped.png"),
                60,
                196,
                LoopMode::Loop,
            ),
            PerchBlink => perch(
                "perch_blink.png",
                Some("perch_blink_flipped.png"),
                8,
                196,
                LoopMode::OneShot,
            ),
            PerchBlinkGlitch => perch(
                "perch_blink_glitch.png",
                Some("perch_blink_glitch_flipped.png"),
                8,
                196,
                LoopMode::OneShot,
            ),
            PerchGlitch => perch(
                "perch_glitch.png",
                Some("perch_glitch_flipped.png"),
                10,
                288,
                LoopMode::OneShot,
            ),
            PerchMusic => perch(
                "perch_music.png",
                Some("perch_music_flipped.png"),
                60,
                196,
                LoopMode::Loop,
            ),
            PerchArrive => perch(
                "perch_arrive.png",
                Some("perch_arrive_flipped.png"),
                14,
                288,
                LoopMode::OneShot,
            ),
            PerchLeave => perch(
                "perch_leave.png",
                Some("perch_leave_flipped.png"),
                12,
                288,
                LoopMode::OneShot,
            ),
            Laydown => laydown(
                "laydown.png",
                Some("laydown_flipped.png"),
                60,
                372,
                LoopMode::Loop,
            ),
            LaydownTalk => laydown(
                "laydown_talk.png",
                Some("laydown_talk_flipped.png"),
                60,
                372,
                LoopMode::Loop,
            ),
            LaydownBlink => laydown(
                "laydown_blink.png",
                Some("laydown_blink_flipped.png"),
                8,
                372,
                LoopMode::OneShot,
            ),
            LaydownBlinkGlitch => laydown(
                "laydown_blink_glitch.png",
                Some("laydown_blink_glitch_flipped.png"),
                8,
                372,
                LoopMode::OneShot,
            ),
            LaydownSleep => laydown(
                "laydown_sleep.png",
                Some("laydown_sleep_flipped.png"),
                60,
                372,
                LoopMode::Loop,
            ),
            LaydownGlitch => laydown(
                "laydown_glitch.png",
                Some("laydown_glitch_flipped.png"),
                10,
                440,
                LoopMode::OneShot,
            ),
            LaydownLeave => laydown(
                "laydown_leave.png",
                Some("laydown_leave_flipped.png"),
                12,
                440,
                LoopMode::OneShot,
            ),
            Mouth => ClipSpec {
                id,
                filename: "mouth.png",
                flipped_filename: None,
                frame_count: 5,
                frame_width: 240,
                frame_height: 384,
                fps: MOUTH_FPS,
                loop_mode: LoopMode::Loop,
                pivot: Vec2::new(118.0, 232.0),
                display_scale: 1.0,
            },
        };
        spec.id = id;
        spec
    }
}

pub fn base_clip_for_stance(stance: Stance) -> ClipId {
    match stance {
        Stance::Stand | Stance::Skid => ClipId::Idle,
        Stance::Held | Stance::Fly | Stance::Upside | Stance::Ride => ClipId::Dangle,
        Stance::Sit | Stance::Corner => ClipId::Perch,
        Stance::LayDown | Stance::Dazed => ClipId::Laydown,
    }
}

fn body(filename: &'static str, frame_count: usize, fps: f32, loop_mode: LoopMode) -> ClipSpec {
    ClipSpec {
        id: ClipId::Idle,
        filename,
        flipped_filename: None,
        frame_count,
        frame_width: 240,
        frame_height: 384,
        fps,
        loop_mode,
        pivot: Vec2::new(118.0, 232.0),
        display_scale: 1.0,
    }
}

fn perch(
    filename: &'static str,
    flipped_filename: Option<&'static str>,
    frame_count: usize,
    frame_width: u32,
    loop_mode: LoopMode,
) -> ClipSpec {
    ClipSpec {
        id: ClipId::Perch,
        filename,
        flipped_filename,
        frame_count,
        frame_width,
        frame_height: 384,
        fps: 30.0,
        loop_mode,
        pivot: Vec2::new(frame_width as f32 * 0.5 + 26.0, 267.0),
        display_scale: 1.0,
    }
}

fn laydown(
    filename: &'static str,
    flipped_filename: Option<&'static str>,
    frame_count: usize,
    frame_width: u32,
    loop_mode: LoopMode,
) -> ClipSpec {
    ClipSpec {
        id: ClipId::Laydown,
        filename,
        flipped_filename,
        frame_count,
        frame_width,
        frame_height: 250,
        fps: 30.0,
        loop_mode,
        pivot: Vec2::new(frame_width as f32 * 0.5, 241.0),
        display_scale: 0.85,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stance_has_an_explicit_clip() {
        let mappings = [
            (Stance::Stand, ClipId::Idle),
            (Stance::Sit, ClipId::Perch),
            (Stance::LayDown, ClipId::Laydown),
            (Stance::Held, ClipId::Dangle),
            (Stance::Fly, ClipId::Dangle),
            (Stance::Skid, ClipId::Idle),
            (Stance::Dazed, ClipId::Laydown),
            (Stance::Upside, ClipId::Dangle),
            (Stance::Corner, ClipId::Perch),
            (Stance::Ride, ClipId::Dangle),
        ];

        for (stance, expected) in mappings {
            assert_eq!(base_clip_for_stance(stance), expected);
        }
    }

    #[test]
    fn one_shot_frame_sampling_holds_the_last_drawing() {
        let spec = ClipCatalog::spec(ClipId::Wake);
        assert_eq!(spec.frame_index_at(0.0), 0);
        assert_eq!(spec.frame_index_at(spec.duration_secs() * 3.0), 12);
    }

    #[test]
    fn clip_pivots_match_irregular_sprite_cells() {
        let perch = ClipCatalog::spec(ClipId::Perch);
        let laydown = ClipCatalog::spec(ClipId::Laydown);

        assert_eq!(perch.frame_width, 196);
        assert_eq!(perch.pivot.x, 124.0);
        assert_eq!(perch.pivot.y, 267.0);
        assert_eq!(laydown.frame_width, 372);
        assert_eq!(laydown.pivot.y, 241.0);
        assert!((laydown.display_scale - 0.85).abs() < f32::EPSILON);
    }
}
