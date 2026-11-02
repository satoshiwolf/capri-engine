pub mod animation;
pub mod hittest;
pub mod pipeline;
pub mod sprites;
pub mod vfx;
pub mod window;

pub use animation::{
    AnimationAction, AnimationController, AnimationInput, AnimationSample, ClipCatalog, ClipId,
    ClipSpec, LoopMode, base_clip_for_stance,
};
pub use hittest::AlphaBitmask;
pub use pipeline::{Renderer, strip_name_for_stance};
pub use sprites::{SpriteManager, SpriteStrip};
pub use vfx::VfxSystem;
#[allow(deprecated)]
pub use window::{
    CompositionOverlay, IS_RUNNING, OverlayInput, OwnedHwnd, WindowSharedState,
    create_overlay_window, pump_messages, should_capture_input, try_create_overlay_owner,
    wait_for_messages,
};
