use glam::Vec2;

use crate::core::config::{
    DAZED_STAR_PHASE_RATE_RAD_PER_SEC, SHOCKWAVE_DECAY_PER_SEC, SHOCKWAVE_EXPANSION_PX_PER_SEC,
    SPARK_GRAVITY_PX_PER_SEC2, VFX_CRACK_JITTER_PX, VFX_CRACK_LENGTH_BASE_PX,
    VFX_CRACK_LENGTH_RANGE_PX, VFX_CRACK_LIFE_BASE_SECS, VFX_CRACK_LIFE_RANGE_SECS,
    VFX_CRACK_MIN_HARDNESS, VFX_CRACK_RAY_BASE, VFX_CRACK_RAY_RANGE, VFX_CRACK_SEGMENTS,
    VFX_DIZZY_LIFE_SECS, VFX_SHOCKWAVE_MIN_INTENSITY, VFX_SHOCKWAVE_MIN_RADIUS_PX,
    VFX_SHOCKWAVE_RADIUS_RANGE_PX, VFX_SHOCKWAVE_START_RADIUS_PX, VFX_SPARK_ANGLE_STEP_RAD,
    VFX_SPARK_DOWNWARD_BIAS, VFX_SPARK_LIFE_BASE_SECS, VFX_SPARK_LIFE_STEP_SECS,
    VFX_SPARK_MAX_LIFE_SECS, VFX_SPARK_SPEED_BASE_PX_SEC, VFX_SPARK_SPEED_VARIATION,
    VFX_SPARK_VERTICAL_JITTER,
};

#[derive(Debug, Clone)]
pub struct Spark {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub color_rgb: [f32; 3],
}

impl Spark {
    pub fn update(&mut self, dt: f32) -> bool {
        self.pos += self.vel * dt;
        self.vel.y += SPARK_GRAVITY_PX_PER_SEC2 * dt;
        self.life -= dt;
        self.life > 0.0
    }
}

#[derive(Debug, Clone)]
pub struct Shockwave {
    pub center: Vec2,
    pub radius: f32,
    pub max_radius: f32,
    pub intensity: f32,
}

impl Shockwave {
    pub fn new(center: Vec2, hard: f32) -> Self {
        Self {
            center,
            radius: VFX_SHOCKWAVE_START_RADIUS_PX,
            max_radius: VFX_SHOCKWAVE_MIN_RADIUS_PX + hard * VFX_SHOCKWAVE_RADIUS_RANGE_PX,
            intensity: hard.clamp(VFX_SHOCKWAVE_MIN_INTENSITY, 1.0),
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.radius += SHOCKWAVE_EXPANSION_PX_PER_SEC * dt;
        self.intensity -= SHOCKWAVE_DECAY_PER_SEC * dt;
        self.radius < self.max_radius && self.intensity > 0.0
    }
}

#[derive(Debug, Clone)]
pub struct CrackSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub life: f32,
}

#[derive(Debug, Clone)]
pub struct DizzyStars {
    pub center: Vec2,
    pub phase: f32,
    pub life: f32,
}

#[derive(Debug, Clone)]
pub struct VfxSystem {
    pub sparks: Vec<Spark>,
    pub shockwaves: Vec<Shockwave>,
    pub cracks: Vec<CrackSegment>,
    pub dizzy: Option<DizzyStars>,
}

impl VfxSystem {
    pub fn new() -> Self {
        Self {
            sparks: Vec::with_capacity(128),
            shockwaves: Vec::with_capacity(8),
            cracks: Vec::with_capacity(32),
            dizzy: None,
        }
    }

    pub fn emit_sparks(&mut self, pos: Vec2, count: usize, base_dir: Vec2) {
        for i in 0..count {
            let angle_offset = ((i as f32) - (count as f32) / 2.0) * VFX_SPARK_ANGLE_STEP_RAD;
            let speed = VFX_SPARK_SPEED_BASE_PX_SEC
                + ((i * 37) % VFX_SPARK_SPEED_VARIATION as usize) as f32;
            let dir = Vec2::new(
                base_dir.x + angle_offset,
                base_dir.y - VFX_SPARK_DOWNWARD_BIAS - ((i % 3) as f32) * VFX_SPARK_VERTICAL_JITTER,
            )
            .normalize();

            self.sparks.push(Spark {
                pos,
                vel: dir * speed,
                life: VFX_SPARK_LIFE_BASE_SECS + ((i % 4) as f32) * VFX_SPARK_LIFE_STEP_SECS,
                max_life: VFX_SPARK_MAX_LIFE_SECS,
                color_rgb: [1.0, 0.75, 0.2],
            });
        }
    }

    pub fn emit_cracks(&mut self, center: Vec2, normal: Vec2, hard: f32) {
        if hard < VFX_CRACK_MIN_HARDNESS {
            return;
        }

        let rays = (VFX_CRACK_RAY_BASE + hard * VFX_CRACK_RAY_RANGE) as usize;
        let length = VFX_CRACK_LENGTH_BASE_PX + hard * VFX_CRACK_LENGTH_RANGE_PX;

        for r in 0..rays {
            let angle = (r as f32) * (std::f32::consts::PI * 2.0 / (rays as f32)) + normal.y * 0.5;
            let dir = Vec2::new(angle.cos(), angle.sin());

            let mut current = center;
            for seg in 0..VFX_CRACK_SEGMENTS {
                let seg_len = length / VFX_CRACK_SEGMENTS as f32;
                let jitter = Vec2::new(
                    ((seg * 17) % 11) as f32 - VFX_CRACK_JITTER_PX,
                    ((seg * 23) % 11) as f32 - VFX_CRACK_JITTER_PX,
                );
                let next = current + dir * seg_len + jitter;

                self.cracks.push(CrackSegment {
                    start: current,
                    end: next,
                    life: VFX_CRACK_LIFE_BASE_SECS + hard * VFX_CRACK_LIFE_RANGE_SECS,
                });
                current = next;
            }
        }
    }

    pub fn emit_shockwave(&mut self, center: Vec2, hard: f32) {
        if hard >= 0.15 {
            self.shockwaves.push(Shockwave::new(center, hard));
        }
    }

    pub fn emit_dizzy(&mut self, center: Vec2) {
        self.dizzy = Some(DizzyStars {
            center,
            phase: 0.0,
            life: VFX_DIZZY_LIFE_SECS,
        });
    }

    pub fn update(&mut self, dt: f32) {
        self.sparks.retain_mut(|s| s.update(dt));
        self.shockwaves.retain_mut(|w| w.update(dt));

        self.cracks.retain_mut(|c| {
            c.life -= dt;
            c.life > 0.0
        });
        if let Some(stars) = &mut self.dizzy {
            stars.phase += DAZED_STAR_PHASE_RATE_RAD_PER_SEC * dt;
            stars.life -= dt;
            if stars.life <= 0.0 {
                self.dizzy = None;
            }
        }
    }
}

impl Default for VfxSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparks_fall_and_expire() {
        let mut vfx = VfxSystem::new();
        vfx.emit_sparks(Vec2::new(10.0, 20.0), 1, Vec2::X);
        let initial_velocity_y = vfx.sparks[0].vel.y;

        vfx.update(0.1);

        assert!(vfx.sparks[0].vel.y > initial_velocity_y);

        vfx.update(1.0);
        assert!(vfx.sparks.is_empty());
    }

    #[test]
    fn shockwaves_expand_and_expire() {
        let mut vfx = VfxSystem::new();
        vfx.emit_shockwave(Vec2::new(30.0, 40.0), 1.0);
        let initial_radius = vfx.shockwaves[0].radius;

        vfx.update(0.1);

        assert!(vfx.shockwaves[0].radius > initial_radius);
        assert!(vfx.shockwaves[0].intensity < 1.0);

        vfx.update(10.0);
        assert!(vfx.shockwaves.is_empty());
    }

    #[test]
    fn cracks_respect_impact_threshold_and_expire() {
        let mut vfx = VfxSystem::new();
        vfx.emit_cracks(Vec2::ZERO, Vec2::Y, 0.1);
        assert!(vfx.cracks.is_empty());

        vfx.emit_cracks(Vec2::ZERO, Vec2::Y, 1.0);
        assert!(!vfx.cracks.is_empty());

        vfx.update(5.0);
        assert!(vfx.cracks.is_empty());
    }

    #[test]
    fn dizzy_stars_advance_phase_and_expire() {
        let mut vfx = VfxSystem::new();
        vfx.emit_dizzy(Vec2::new(10.0, 10.0));
        let initial_phase = vfx.dizzy.as_ref().expect("dizzy stars").phase;

        vfx.update(0.1);

        assert!(vfx.dizzy.as_ref().expect("dizzy stars").phase > initial_phase);

        vfx.update(3.0);
        assert!(vfx.dizzy.is_none());
    }
}
