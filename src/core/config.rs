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
pub const THROW_GRAB_MARGIN_PX: f32 = 20.0;
pub const SLOW_RELEASE_RETENTION: f32 = 0.35;
pub const SLOW_RELEASE_DOWN_PX_SEC: f32 = 1.0;
pub const FACING_TURN_THRESHOLD_PX_PER_SEC: f32 = 25.0;

/// Pendulum spring parameters. The angle limit is expressed in degrees.
pub const PENDULUM_STIFFNESS: f32 = 95.0;
pub const PENDULUM_DAMPING: f32 = 8.5;
pub const PENDULUM_MAX_ANGLE: f32 = 42.0;
pub const PENDULUM_TARGET_DEG_PER_PX_SEC: f32 = 0.014;
pub const PENDULUM_SPEED_NORMALIZER: f32 = 2200.0;
pub const PENDULUM_SMEAR_THRESHOLD_PX_SEC: f32 = 80.0;
pub const PENDULUM_SMEAR_THRESHOLD_NORMALIZED: f32 = 0.35;
pub const PENDULUM_SMEAR_STRENGTH_PX: f32 = 20.0;
pub const PENDULUM_MAX_HAIR_LEAN: f32 = 7.0;

/// Dimensionless impulse and angular damping coefficients.
pub const BOUNCE_RESTITUTION_X: f32 = 0.42;
pub const BOUNCE_RESTITUTION_Y: f32 = 0.35;
pub const BOUNCE_ANGULAR_DAMP: f32 = 0.55;

/// Skid deceleration in pixels per second squared and dazed duration in seconds.
pub const SKID_DECELERATION: f32 = 2800.0;
pub const SKID_START_SPEED_PX_SEC: f32 = 50.0;
pub const DAZED_DURATION_SECS: f32 = 2.4;

pub const DEFAULT_WS_PORT: u16 = 47823;
pub const DEFAULT_RENDER_HZ: u32 = 60;
pub const METRICS_SAMPLE_CAPACITY: usize = 600;
pub const GATEWAY_COMMAND_CAPACITY: usize = 256;
pub const GATEWAY_EVENT_CAPACITY: usize = 256;
pub const GATEWAY_MAX_CONNECTIONS: usize = 16;
pub const GATEWAY_MAX_MESSAGE_BYTES: usize = 256 * 1024;
pub const GATEWAY_MAX_FRAME_BYTES: usize = 64 * 1024;
pub const GATEWAY_MAX_WRITE_BUFFER_BYTES: usize = GATEWAY_MAX_MESSAGE_BYTES * 2;
pub const GATEWAY_ACCEPT_RETRY_DELAY_MS: u64 = 100;
pub const GATEWAY_HANDSHAKE_TIMEOUT_MS: u64 = 5_000;

/// Command-driven suspension position below the top edge of the virtual desktop.
pub const CEILING_HANG_OFFSET_PX: f32 = 60.0;

/// Cursor-ride vertical offset in screen pixels.
pub const RIDE_CURSOR_OFFSET_PX: f32 = 86.0;
pub const RIDE_CURSOR_TILT_DEG_PER_PX_SEC: f32 = 0.012;
pub const RIDE_CURSOR_MAX_TILT_DEGREES: f32 = 24.0;

/// Time spent sitting before the avatar settles into the lay-down loop.
pub const SIT_TO_LAYDOWN_SECS: f32 = 60.0;

/// Collision and landing thresholds in pixels, pixels per second, or degrees.
pub const WALL_IMPACT_MIN_SPEED_PX_SEC: f32 = 250.0;
pub const WALL_IMPACT_MAX_SPEED_PX_SEC: f32 = 2300.0;
pub const FLOOR_SURFACE_OFFSET_PX: f32 = 2.0;
pub const LEDGE_LANDING_TOLERANCE_PX: f32 = 10.0;
pub const UPRIGHT_ANGLE_MARGIN_DEGREES: f32 = 35.0;
pub const LANDING_SKID_MAX_VY_PX_SEC: f32 = 1200.0;
pub const LANDING_IMPACT_SPEED_NORMALIZER: f32 = 2400.0;

/// Flight orientation and collision-response tuning.
pub const FLIGHT_REVERSAL_ANGLE_DEGREES: f32 = 180.0;
pub const FLIGHT_MAX_ANGULAR_SPEED_DEG_PER_SEC: f32 = 900.0;
pub const FLIGHT_FLOOR_ALIGN_DEG_PER_SEC: f32 = 540.0;
pub const FLIGHT_ZERO_HORIZONTAL_ANGLE_DEGREES: f32 = 90.0;
pub const FLIGHT_OPPOSITE_HORIZONTAL_ANGLE_DEGREES: f32 = 270.0;
pub const FLIGHT_HITSTOP_MIN_HARDNESS: f32 = 0.12;
pub const FLIGHT_HITSTOP_FRAME_BASE: f32 = 1.0;
pub const FLIGHT_HITSTOP_FRAME_SCALE: f32 = 3.0;
pub const FLIGHT_SURFACE_AXIS_THRESHOLD: f32 = 0.5;
pub const FLIGHT_VERTICAL_ANGULAR_DAMP: f32 = 0.70;

/// Squash and stretch response and recovery timing.
pub const SQUASH_MIN_COMPRESSION: f32 = 0.84;
pub const SQUASH_COMPRESSION_RANGE: f32 = 0.30;
pub const SQUASH_TRANSVERSE_RATIO: f32 = 0.60;
pub const SQUASH_HOLD_FRAME_BASE: f32 = 1.0;
pub const SQUASH_HOLD_FRAME_SCALE: f32 = 4.0;
pub const SQUASH_RECOVERY_SECS: f32 = 0.12;
pub const SQUASH_FIRST_SETTLE_FRACTION: f32 = 0.35;
pub const SQUASH_SECOND_SETTLE_FRACTION: f32 = 0.68;
pub const SQUASH_FIRST_RECOVERY_SCALE: f32 = 0.50;
pub const SQUASH_OVERSHOOT_SCALE: f32 = 0.97;

/// Dazed star orbit geometry and angular cadence.
pub const DAZED_STAR_RADIUS_X_PX: f32 = 36.0;
pub const DAZED_STAR_RADIUS_Y_PX: f32 = 12.0;
pub const DAZED_STAR_CENTER_OFFSET_Y_PX: f32 = -28.0;
pub const DAZED_STAR_PHASE_RATE_RAD_PER_SEC: f32 = 4.5;

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
pub const VISUAL_RELEASE_SMEAR_BOOST: f32 = 1.35;
pub const VISUAL_ECHO_OFFSET_PX: f32 = 10.0;
pub const VISUAL_ECHO_SPEED_RATIO: f32 = 0.55;

/// Impact response tuning for animation flashes, glitches, and particle counts.
pub const IMPACT_HARDNESS_NORMALIZER_PX_SEC: f32 = 2000.0;
pub const VISUAL_GRAB_FLASH_SECS: f32 = 0.18;
pub const VISUAL_GRAB_GLITCH_SECS: f32 = 0.16;
pub const VISUAL_THROW_FLASH_BASE_SECS: f32 = 0.12;
pub const VISUAL_THROW_FLASH_SPEED_SECS: f32 = 0.22;
pub const VISUAL_THROW_GLITCH_BASE_SECS: f32 = 0.20;
pub const VISUAL_THROW_GLITCH_SPEED_SECS: f32 = 0.30;
pub const VISUAL_IMPACT_HARDNESS_NORMALIZER_PX_SEC: f32 = 2400.0;
pub const VISUAL_IMPACT_DURATION_BIAS: f32 = 0.45;
pub const VISUAL_SKID_LANDING_FLASH_SECS: f32 = 0.12;
pub const VISUAL_HARD_LANDING_FLASH_SECS: f32 = 0.20;
pub const VISUAL_FELL_GLITCH_SECS: f32 = 0.22;
pub const IMPACT_SPARK_COUNT: usize = 12;
pub const SKID_LANDING_SPARK_COUNT: usize = 8;
pub const DIZZY_VFX_OFFSET_Y_PX: f32 = -72.0;
pub const MOUTH_GLITCH_SCALE: f32 = 0.35;

/// Particle and shockwave geometry.
pub const VFX_SHOCKWAVE_START_RADIUS_PX: f32 = 8.0;
pub const VFX_SHOCKWAVE_MIN_RADIUS_PX: f32 = 30.0;
pub const VFX_SHOCKWAVE_RADIUS_RANGE_PX: f32 = 70.0;
pub const VFX_SHOCKWAVE_MIN_INTENSITY: f32 = 0.2;
pub const VFX_SPARK_ANGLE_STEP_RAD: f32 = 0.25;
pub const VFX_SPARK_SPEED_BASE_PX_SEC: f32 = 200.0;
pub const VFX_SPARK_SPEED_VARIATION: f32 = 300.0;
pub const VFX_SPARK_DOWNWARD_BIAS: f32 = 0.5;
pub const VFX_SPARK_VERTICAL_JITTER: f32 = 0.3;
pub const VFX_SPARK_LIFE_BASE_SECS: f32 = 0.35;
pub const VFX_SPARK_LIFE_STEP_SECS: f32 = 0.1;
pub const VFX_SPARK_MAX_LIFE_SECS: f32 = 0.5;
pub const VFX_CRACK_MIN_HARDNESS: f32 = 0.25;
pub const VFX_CRACK_RAY_BASE: f32 = 3.0;
pub const VFX_CRACK_RAY_RANGE: f32 = 5.0;
pub const VFX_CRACK_LENGTH_BASE_PX: f32 = 40.0;
pub const VFX_CRACK_LENGTH_RANGE_PX: f32 = 110.0;
pub const VFX_CRACK_SEGMENTS: usize = 3;
pub const VFX_CRACK_JITTER_PX: f32 = 5.0;
pub const VFX_CRACK_LIFE_BASE_SECS: f32 = 2.0;
pub const VFX_CRACK_LIFE_RANGE_SECS: f32 = 2.0;
pub const VFX_DIZZY_LIFE_SECS: f32 = 2.4;

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
