use glam::Vec2;

use crate::core::config::{SKID_LEAN_PER_PX_SEC, SKID_MAX_LEAN_DEGREES};
use crate::core::error::EngineResult;
use crate::core::state::{Facing, Stance};
use crate::core::visual::VisualTransform;
use crate::physics::PhysicsSnapshot;
use crate::render::animation::{AnimationSample, ClipCatalog, ClipId};
use crate::render::pipeline::Renderer;
use crate::render::sprites::SpriteManager;

#[derive(Clone, Copy)]
pub struct RenderedBody {
    clip: ClipId,
    flipped: bool,
    frame_index: usize,
    pivot: Vec2,
    display_scale: f32,
}

impl RenderedBody {
    pub fn initial() -> Self {
        let spec = ClipCatalog::spec(ClipId::Idle);
        Self {
            clip: ClipId::Idle,
            flipped: false,
            frame_index: 0,
            pivot: spec.pivot,
            display_scale: spec.display_scale,
        }
    }

    pub fn clip(self) -> ClipId {
        self.clip
    }

    pub fn flipped(self) -> bool {
        self.flipped
    }

    pub fn frame_index(self) -> usize {
        self.frame_index
    }

    pub fn pivot(self) -> Vec2 {
        self.pivot
    }

    pub fn display_scale(self) -> f32 {
        self.display_scale
    }
}

pub fn prepare_loaded_strip(
    renderer: &mut Renderer,
    manager: &mut SpriteManager,
    clip: ClipId,
    flipped: bool,
) -> EngineResult<()> {
    if let Some(strip) = manager.get_mut(clip, flipped)
        && !strip.rgba_frames.is_empty()
    {
        renderer.try_prepare_strip(strip)?;
        strip.release_rgba_frames();
    }
    Ok(())
}

pub fn select_body(
    manager: &mut SpriteManager,
    sample: AnimationSample,
    fallback: RenderedBody,
) -> RenderedBody {
    manager.request(sample.clip, sample.use_flipped_asset);
    manager.request(ClipId::Idle, false);

    if manager.get(sample.clip, sample.use_flipped_asset).is_some() {
        return RenderedBody {
            clip: sample.clip,
            flipped: sample.use_flipped_asset,
            frame_index: sample.frame_index,
            pivot: sample.pivot,
            display_scale: sample.display_scale,
        };
    }

    if sample.use_flipped_asset && manager.get(sample.clip, false).is_some() {
        let spec = ClipCatalog::spec(sample.clip);
        return RenderedBody {
            clip: sample.clip,
            flipped: false,
            frame_index: sample.frame_index.min(spec.frame_count.saturating_sub(1)),
            pivot: spec.pivot,
            display_scale: spec.display_scale,
        };
    }

    if manager.get(fallback.clip, fallback.flipped).is_some() {
        return fallback;
    }

    let spec = ClipCatalog::spec(ClipId::Idle);
    RenderedBody {
        clip: ClipId::Idle,
        flipped: false,
        frame_index: sample.frame_index % spec.frame_count,
        pivot: spec.pivot,
        display_scale: spec.display_scale,
    }
}

pub fn prewarm_animation(manager: &mut SpriteManager, physics: &PhysicsSnapshot) {
    let flipped = physics.facing() == Facing::Left;
    let next = match physics.stance() {
        Stance::Skid if physics.has_landing_surface() => Some(ClipId::Perch),
        Stance::Dazed => Some(ClipId::Laydown),
        Stance::Sit | Stance::Corner => Some(ClipId::Laydown),
        Stance::Held | Stance::Fly | Stance::Upside | Stance::Ride => Some(ClipId::Idle),
        Stance::Stand | Stance::LayDown | Stance::Skid => None,
    };
    if let Some(clip) = next {
        manager.request(clip, flipped);
    }
}

pub fn render_transform(physics: &PhysicsSnapshot, actual_flipped_asset: bool) -> VisualTransform {
    let mut transform = physics.visual_transform();
    if actual_flipped_asset {
        transform.facing = Facing::Right;
    }
    if physics.stance() == Stance::Skid
        && let Some(skid) = physics.skid()
    {
        transform.rotation_degrees = (-skid.velocity_x * SKID_LEAN_PER_PX_SEC)
            .clamp(-SKID_MAX_LEAN_DEGREES, SKID_MAX_LEAN_DEGREES);
    }
    transform
}

pub fn current_motion(physics: &PhysicsSnapshot, cursor_motion: Vec2) -> Vec2 {
    match physics.stance() {
        Stance::Held | Stance::Ride => cursor_motion,
        Stance::Fly => physics
            .flight()
            .map_or(Vec2::ZERO, |flight| flight.velocity),
        _ => Vec2::ZERO,
    }
}
