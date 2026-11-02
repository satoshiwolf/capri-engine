pub mod core;
pub mod desktop;
pub mod gateway;
pub mod physics;
pub mod render;

pub use core::error::{EngineError, EngineResult, ErrorCategory};
pub use core::metrics::FrameMetrics;
