use glam::Vec2;

use super::catalog::{ClipCatalog, ClipId, LoopMode, base_clip_for_stance};
use crate::core::config::{
    AMBIENT_GLITCH_MAX_SECS, AMBIENT_GLITCH_MIN_SECS, BLINK_CHANCE, GLITCH_BLINK_CHANCE,
    IMPACT_FLASH_DECAY, IMPACT_FLASH_DURATION_SECS, IMPACT_GLITCH_DECAY,
    IMPACT_GLITCH_DURATION_SECS, LIP_SYNC_HOLD_SECS, THROW_MAX_SPEED, VISUAL_SMEAR_MAX_OFFSET,
    VISUAL_SMEAR_MAX_SPEED, VISUAL_SMEAR_MIN_SPEED,
};
use crate::core::events::EngineEvent;
use crate::core::state::{Emotion, Facing, Stance};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationAction {
    Grin,
    Glitch,
    PerchGlitch,
    Wake,
    Arrive,
    Leave,
    PerchArrive,
    LaydownGlitch,
}

impl AnimationAction {
    pub fn clip_id(self) -> ClipId {
        match self {
            Self::Grin => ClipId::Grin,
            Self::Glitch => ClipId::Glitch,
            Self::PerchGlitch => ClipId::PerchGlitch,
            Self::Wake => ClipId::Wake,
            Self::Arrive => ClipId::Arrive,
            Self::Leave => ClipId::Leave,
            Self::PerchArrive => ClipId::PerchArrive,
            Self::LaydownGlitch => ClipId::LaydownGlitch,
        }
    }

    pub fn priority(self) -> u8 {
        match self {
            Self::Grin | Self::Glitch | Self::PerchGlitch => 1,
            Self::Wake | Self::LaydownGlitch => 2,
            Self::Arrive | Self::PerchArrive => 3,
            Self::Leave => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AnimationInput {
    pub stance: Stance,
    pub facing: Facing,
    pub motion: Vec2,
    pub release_streak: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationSample {
    pub clip: ClipId,
    pub frame_index: usize,
    pub use_flipped_asset: bool,
    pub pivot: Vec2,
    pub display_scale: f32,
    pub smear: Vec2,
    pub echo: Vec2,
    pub flash: f32,
    pub glitch: f32,
    pub mouth_viseme: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
struct ActiveAction {
    action: AnimationAction,
    elapsed: f32,
    priority: u8,
}

#[derive(Debug, Clone, Copy)]
struct AmbientBlink {
    clip: ClipId,
    elapsed: f32,
}

pub struct AnimationController {
    stance: Stance,
    base_clip: ClipId,
    base_elapsed: f32,
    active_action: Option<ActiveAction>,
    ambient_blink: Option<AmbientBlink>,
    speech_remaining: f32,
    viseme: u8,
    facing: Facing,
    motion: Vec2,
    release_streak: bool,
    flash: f32,
    glitch: f32,
    next_glitch_at: f32,
    ambient_time: f32,
    last_base_cycle: u64,
    rng_state: u64,
}

impl AnimationController {
    pub fn new() -> Self {
        Self {
            stance: Stance::Stand,
            base_clip: base_clip_for_stance(Stance::Stand),
            base_elapsed: 0.0,
            active_action: None,
            ambient_blink: None,
            speech_remaining: 0.0,
            viseme: 0,
            facing: Facing::Right,
            motion: Vec2::ZERO,
            release_streak: false,
            flash: 0.0,
            glitch: 0.0,
            next_glitch_at: AMBIENT_GLITCH_MIN_SECS,
            ambient_time: 0.0,
            last_base_cycle: 0,
            rng_state: 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn update(&mut self, dt: f32, input: AnimationInput) {
        let dt = dt.clamp(0.0, 0.05);
        let previous_stance = self.stance;
        self.stance = input.stance;
        self.facing = input.facing;
        self.motion = input.motion;
        self.release_streak = input.release_streak;

        if self.stance != previous_stance {
            self.base_clip = base_clip_for_stance(self.stance);
            self.base_elapsed = 0.0;
            self.ambient_blink = None;
            self.on_stance_changed(previous_stance);
        } else {
            let id = display_base_clip(self.stance, self.speech_remaining > 0.0);
            if id != self.base_clip {
                self.base_clip = id;
                self.base_elapsed = 0.0;
            }
            self.base_elapsed += dt;
        }

        self.ambient_time += dt;
        self.speech_remaining = (self.speech_remaining - dt).max(0.0);
        self.flash = (self.flash - IMPACT_FLASH_DECAY * dt).max(0.0);
        self.glitch = (self.glitch - IMPACT_GLITCH_DECAY * dt).max(0.0);

        if let Some(action) = &mut self.active_action {
            action.elapsed += dt;
            let spec = ClipCatalog::spec(action.action.clip_id());
            if action.elapsed >= spec.duration_secs() {
                self.active_action = None;
            }
        }

        if let Some(blink) = &mut self.ambient_blink {
            blink.elapsed += dt;
            if blink.elapsed >= ClipCatalog::spec(blink.clip).duration_secs() {
                self.ambient_blink = None;
            }
        }

        self.maybe_trigger_ambient();
    }

    pub fn play_action(&mut self, action: AnimationAction) {
        let priority = action.priority();
        if self
            .active_action
            .is_some_and(|active| active.priority > priority)
        {
            return;
        }
        self.active_action = Some(ActiveAction {
            action,
            elapsed: 0.0,
            priority,
        });
        self.ambient_blink = None;
    }

    pub fn begin_speech(&mut self, duration_secs: f32) {
        self.speech_remaining = duration_secs.max(LIP_SYNC_HOLD_SECS);
        self.viseme = 0;
    }

    pub fn set_viseme(&mut self, viseme: u8) {
        self.viseme = viseme.min(4);
        self.speech_remaining = self.speech_remaining.max(LIP_SYNC_HOLD_SECS);
    }

    pub fn set_emotion(&mut self, emotion: Emotion) {
        match emotion {
            Emotion::Happy => self.play_action(AnimationAction::Grin),
            Emotion::Jealous | Emotion::Angry => self.glitch = 0.55,
            Emotion::Sleepy | Emotion::Focused | Emotion::Neutral => {}
        }
    }

    pub fn on_engine_event(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::Grabbed { .. } => {
                self.flash = self.flash.max(0.18);
                self.glitch = self.glitch.max(0.16);
            }
            EngineEvent::Thrown { speed, .. } => {
                let hard = (*speed / THROW_MAX_SPEED).clamp(0.0, 1.0);
                self.flash = self.flash.max(0.12 + hard * 0.22);
                self.glitch = self.glitch.max(0.20 + hard * 0.30);
            }
            EngineEvent::Impact { speed, .. } => {
                let hard = (*speed / 2400.0).clamp(0.0, 1.0);
                self.flash = self.flash.max(IMPACT_FLASH_DURATION_SECS * (0.45 + hard));
                self.glitch = self.glitch.max(IMPACT_GLITCH_DURATION_SECS * (0.45 + hard));
            }
            EngineEvent::Landed { skid, .. } => {
                self.flash = self.flash.max(if *skid { 0.12 } else { 0.20 });
            }
            EngineEvent::Fell => {
                self.glitch = self.glitch.max(0.22);
            }
            EngineEvent::Dropped { .. }
            | EngineEvent::MonitorChanged { .. }
            | EngineEvent::Speak { .. }
            | EngineEvent::LipSync { .. } => {}
        }
    }

    pub fn sample(&self) -> AnimationSample {
        let (clip, elapsed) = self.selected_clip();
        let spec = ClipCatalog::spec(clip);
        let speed = self.motion.length();
        let smear_scale = ((speed - VISUAL_SMEAR_MIN_SPEED)
            / (VISUAL_SMEAR_MAX_SPEED - VISUAL_SMEAR_MIN_SPEED))
            .clamp(0.0, 1.0);
        let direction = if speed > f32::EPSILON {
            self.motion / speed
        } else {
            Vec2::ZERO
        };
        let release_boost = if self.release_streak { 1.35 } else { 1.0 };
        let smear = -direction * VISUAL_SMEAR_MAX_OFFSET * smear_scale * release_boost;
        let echo = if self.release_streak || speed > VISUAL_SMEAR_MAX_SPEED * 0.55 {
            direction * 10.0 * smear_scale
        } else {
            Vec2::ZERO
        };
        let mouth_viseme = (self.speech_remaining > 0.0).then_some(self.viseme.min(4));

        AnimationSample {
            clip,
            frame_index: spec.frame_index_at(elapsed),
            use_flipped_asset: self.facing == Facing::Left && spec.flipped_filename.is_some(),
            pivot: spec.pivot,
            display_scale: spec.display_scale,
            smear,
            echo,
            flash: self.flash.clamp(0.0, 1.0),
            glitch: self.glitch.clamp(0.0, 1.0),
            mouth_viseme,
        }
    }

    pub fn current_clip(&self) -> ClipId {
        self.selected_clip().0
    }

    fn selected_clip(&self) -> (ClipId, f32) {
        if self.speech_remaining > 0.0 {
            match self.stance {
                Stance::Sit | Stance::Corner => return (ClipId::PerchTalk, self.base_elapsed),
                Stance::LayDown | Stance::Dazed => {
                    return (ClipId::LaydownTalk, self.base_elapsed);
                }
                _ => {}
            }
        }
        if let Some(action) = self.active_action {
            return (action.action.clip_id(), action.elapsed);
        }
        if let Some(blink) = self.ambient_blink {
            return (blink.clip, blink.elapsed);
        }
        (self.base_clip, self.base_elapsed)
    }

    fn on_stance_changed(&mut self, previous: Stance) {
        match self.stance {
            Stance::Sit | Stance::Corner => self.play_action(AnimationAction::PerchArrive),
            Stance::LayDown => self.play_action(AnimationAction::LaydownGlitch),
            Stance::Stand if previous == Stance::Dazed => self.play_action(AnimationAction::Wake),
            Stance::Held | Stance::Fly => {
                self.glitch = self.glitch.max(0.16);
            }
            _ => {}
        }
    }

    fn maybe_trigger_ambient(&mut self) {
        if self.speech_remaining > 0.0
            || self.active_action.is_some()
            || self.ambient_blink.is_some()
        {
            return;
        }
        if !matches!(
            self.stance,
            Stance::Stand | Stance::Sit | Stance::Corner | Stance::LayDown
        ) {
            return;
        }

        let spec = ClipCatalog::spec(self.base_clip);
        let cycle = if spec.loop_mode == LoopMode::Loop {
            (self.base_elapsed * spec.fps) as u64 / spec.frame_count as u64
        } else {
            0
        };
        let crossed_loop = cycle != self.last_base_cycle;
        if !crossed_loop && self.base_elapsed > 0.0 {
            return;
        }
        self.last_base_cycle = cycle;

        let roll = self.next_random();
        if roll < GLITCH_BLINK_CHANCE {
            let clip = match self.stance {
                Stance::Sit | Stance::Corner => ClipId::PerchBlinkGlitch,
                Stance::LayDown => ClipId::LaydownBlinkGlitch,
                _ => ClipId::BlinkGlitch,
            };
            self.ambient_blink = Some(AmbientBlink { clip, elapsed: 0.0 });
        } else if roll < BLINK_CHANCE {
            let clip = match self.stance {
                Stance::Sit | Stance::Corner => ClipId::PerchBlink,
                Stance::LayDown => ClipId::LaydownBlink,
                _ => ClipId::Blink,
            };
            self.ambient_blink = Some(AmbientBlink { clip, elapsed: 0.0 });
        }

        if self.ambient_time >= self.next_glitch_at {
            let id = match self.stance {
                Stance::Sit | Stance::Corner => ClipId::PerchGlitch,
                Stance::LayDown => ClipId::LaydownGlitch,
                _ => ClipId::Glitch,
            };
            self.active_action = Some(ActiveAction {
                action: match id {
                    ClipId::PerchGlitch => AnimationAction::PerchGlitch,
                    ClipId::LaydownGlitch => AnimationAction::LaydownGlitch,
                    _ => AnimationAction::Glitch,
                },
                elapsed: 0.0,
                priority: 1,
            });
            self.ambient_blink = None;
            let spread = AMBIENT_GLITCH_MAX_SECS - AMBIENT_GLITCH_MIN_SECS;
            self.next_glitch_at =
                self.ambient_time + AMBIENT_GLITCH_MIN_SECS + self.next_random() * spread;
        }
    }

    fn next_random(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x >> 40) as f32 / (1_u64 << 24) as f32
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new()
    }
}

fn display_base_clip(stance: Stance, speaking: bool) -> ClipId {
    if speaking {
        match stance {
            Stance::Sit | Stance::Corner => return ClipId::PerchTalk,
            Stance::LayDown | Stance::Dazed => return ClipId::LaydownTalk,
            _ => {}
        }
    }
    base_clip_for_stance(stance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_priority_action_is_not_replaced() {
        let mut controller = AnimationController::new();
        controller.play_action(AnimationAction::Leave);
        controller.play_action(AnimationAction::Grin);

        assert_eq!(controller.current_clip(), ClipId::Leave);
    }

    #[test]
    fn talk_and_flipped_clips_follow_stance_and_facing() {
        let mut controller = AnimationController::new();
        controller.update(
            0.016,
            AnimationInput {
                stance: Stance::Sit,
                facing: Facing::Left,
                motion: Vec2::ZERO,
                release_streak: false,
            },
        );
        controller.begin_speech(2.0);
        let sample = controller.sample();

        assert_eq!(sample.clip, ClipId::PerchTalk);
        assert!(sample.use_flipped_asset);
    }

    #[test]
    fn speech_overlay_uses_current_viseme() {
        let mut controller = AnimationController::new();
        controller.begin_speech(1.0);
        controller.set_viseme(3);

        assert_eq!(controller.sample().mouth_viseme, Some(3));
    }

    #[test]
    fn impact_sets_visible_flash_and_glitch() {
        let mut controller = AnimationController::new();
        controller.on_engine_event(&EngineEvent::Impact {
            normal: Vec2::Y,
            speed: 1800.0,
            point: Vec2::ZERO,
            window_title: None,
        });
        let sample = controller.sample();

        assert!(sample.flash > 0.0);
        assert!(sample.glitch > 0.0);
    }

    #[test]
    fn local_clip_clock_is_independent_of_simulation_hz() {
        let input = AnimationInput {
            stance: Stance::Held,
            facing: Facing::Right,
            motion: Vec2::ZERO,
            release_streak: false,
        };
        let mut at_60 = AnimationController::new();
        let mut at_120 = AnimationController::new();
        for _ in 0..60 {
            at_60.update(1.0 / 60.0, input);
        }
        for _ in 0..120 {
            at_120.update(1.0 / 120.0, input);
        }

        assert_eq!(at_60.sample().frame_index, at_120.sample().frame_index);
    }
}
