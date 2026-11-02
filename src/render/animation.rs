mod catalog;
mod controller;

pub use catalog::{ClipCatalog, ClipId, ClipSpec, LoopMode, base_clip_for_stance};
pub use controller::{AnimationAction, AnimationController, AnimationInput, AnimationSample};
