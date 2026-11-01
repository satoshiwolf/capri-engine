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

/// Pendulum spring parameters. The angle limit is expressed in degrees.
pub const PENDULUM_STIFFNESS: f32 = 95.0;
pub const PENDULUM_DAMPING: f32 = 8.5;
pub const PENDULUM_MAX_ANGLE: f32 = 42.0;

/// Dimensionless impulse and angular damping coefficients.
pub const BOUNCE_RESTITUTION_X: f32 = 0.42;
pub const BOUNCE_RESTITUTION_Y: f32 = 0.35;
pub const BOUNCE_ANGULAR_DAMP: f32 = 0.55;

/// Skid deceleration in pixels per second squared and dazed duration in seconds.
pub const SKID_DECELERATION: f32 = 2800.0;
pub const DAZED_DURATION_SECS: f32 = 2.4;

pub const DEFAULT_WS_PORT: u16 = 47823;
