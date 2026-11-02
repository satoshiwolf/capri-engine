use glam::Vec2;

/// Sprite dimensions in pixels.
pub const SPRITE_WIDTH: f32 = 240.0;
pub const SPRITE_HEIGHT: f32 = 384.0;

/// Center of mass and default pivot in local sprite pixels.
pub const MID_POINT: Vec2 = Vec2::new(118.0, 232.0);

/// Feet position in local sprite pixels.
pub const FEET_POINT: Vec2 = Vec2::new(118.0, 380.0);

/// Head position in local sprite pixels.
pub const HEAD_POINT: Vec2 = Vec2::new(118.0, 48.0);

/// Ballistic parameters. Speeds use pixels per second and gravity uses pixels per second squared.
pub const THROW_MIN_SPEED: f32 = 650.0;
pub const THROW_MAX_SPEED: f32 = 4200.0;
pub const THROW_GRAVITY: f32 = 2200.0;
pub const LEAP_GRAVITY: f32 = 1800.0;
pub const AIR_RESISTANCE: f32 = 0.80;
pub const THROW_SPIN_FROM_VX: f32 = 0.32;
pub const THROW_VERTICAL_BOUNCE_X_DAMP: f32 = 0.85;
pub const POINTER_VELOCITY_WINDOW_SECS: f64 = 0.08;

/// Pendulum spring parameters. The angle limit is expressed in degrees.
pub const PENDULUM_STIFFNESS: f32 = 95.0;
pub const PENDULUM_DAMPING: f32 = 8.5;
pub const PENDULUM_MAX_ANGLE: f32 = 42.0;
pub const PENDULUM_TARGET_DEG_PER_PX_SEC: f32 = 0.014;
pub const PENDULUM_SPEED_NORMALIZER: f32 = 2200.0;

/// Dimensionless impulse and angular damping coefficients.
pub const BOUNCE_RESTITUTION_X: f32 = 0.42;
pub const BOUNCE_RESTITUTION_Y: f32 = 0.35;
pub const BOUNCE_ANGULAR_DAMP: f32 = 0.55;

/// Skid deceleration in pixels per second squared and dazed duration in seconds.
pub const SKID_DECELERATION: f32 = 2800.0;
pub const DAZED_DURATION_SECS: f32 = 2.4;

pub const DEFAULT_WS_PORT: u16 = 47823;
pub const DEFAULT_RENDER_HZ: u32 = 60;
pub const GATEWAY_COMMAND_CAPACITY: usize = 256;
pub const GATEWAY_EVENT_CAPACITY: usize = 256;
pub const GATEWAY_MAX_CONNECTIONS: usize = 16;
pub const GATEWAY_MAX_MESSAGE_BYTES: usize = 256 * 1024;
pub const GATEWAY_MAX_FRAME_BYTES: usize = 64 * 1024;
pub const GATEWAY_MAX_WRITE_BUFFER_BYTES: usize = GATEWAY_MAX_MESSAGE_BYTES * 2;
pub const GATEWAY_ACCEPT_RETRY_DELAY_MS: u64 = 100;

/// Time spent sitting before the avatar settles into the lay-down loop.
pub const SIT_TO_LAYDOWN_SECS: f32 = 60.0;

/// Maximum resident alpha-mask payload for decoded sprite clips.
pub const MAX_SPRITE_RESIDENT_BYTES: usize = 25 * 1024 * 1024;

/// Ambient animation cadence and reaction decay.
pub const BLINK_CHANCE: f32 = 0.55;
pub const GLITCH_BLINK_CHANCE: f32 = 0.10;
pub const AMBIENT_GLITCH_MIN_SECS: f32 = 30.0;
pub const AMBIENT_GLITCH_MAX_SECS: f32 = 60.0;
pub const IMPACT_FLASH_DECAY: f32 = 8.0;
pub const IMPACT_GLITCH_DECAY: f32 = 5.0;
pub const IMPACT_FLASH_DURATION_SECS: f32 = 0.18;
pub const IMPACT_GLITCH_DURATION_SECS: f32 = 0.32;

/// Visual smear tuning in pixels and speed thresholds in pixels per second.
pub const VISUAL_SMEAR_MIN_SPEED: f32 = 80.0;
pub const VISUAL_SMEAR_MAX_SPEED: f32 = 2400.0;
pub const VISUAL_SMEAR_MAX_OFFSET: f32 = 34.0;

/// Default voice-driven mouth cadence.
pub const MOUTH_FPS: f32 = 12.0;
pub const LIP_SYNC_HOLD_SECS: f32 = 0.16;
pub const ANIME_FRAME_SECS: f32 = 1.0 / 24.0;
pub const RELEASE_STREAK_SECS: f32 = 2.0 * ANIME_FRAME_SECS;
pub const LEDGE_SCAN_INTERVAL_MS: u64 = 250;
/// Horizontal space reserved at each side of a ledge for stable seating.
pub const LEDGE_SUPPORT_MARGIN_RATIO: f32 = 0.20;
pub const SKID_SPARK_INTERVAL_SECS: f32 = 0.04;
pub const SKID_LEAN_PER_PX_SEC: f32 = 0.012;
pub const SKID_MAX_LEAN_DEGREES: f32 = 16.0;
pub const SPARK_GRAVITY_PX_PER_SEC2: f32 = 980.0;
pub const SHOCKWAVE_EXPANSION_PX_PER_SEC: f32 = 240.0;
pub const SHOCKWAVE_DECAY_PER_SEC: f32 = 1.8;
