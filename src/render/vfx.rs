use glam::Vec2;

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
        self.vel.y += 980.0 * dt;
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
            radius: 8.0,
            max_radius: 30.0 + hard * 70.0,
            intensity: hard.clamp(0.2, 1.0),
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.radius += 240.0 * dt;
        self.intensity -= 1.8 * dt;
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
pub struct VfxSystem {
    pub sparks: Vec<Spark>,
    pub shockwaves: Vec<Shockwave>,
    pub cracks: Vec<CrackSegment>,
}

impl VfxSystem {
    pub fn new() -> Self {
        Self {
            sparks: Vec::with_capacity(128),
            shockwaves: Vec::with_capacity(8),
            cracks: Vec::with_capacity(32),
        }
    }

    pub fn emit_sparks(&mut self, pos: Vec2, count: usize, base_dir: Vec2) {
        for i in 0..count {
            let angle_offset = ((i as f32) - (count as f32) / 2.0) * 0.25;
            let speed = 200.0 + ((i * 37) % 300) as f32;
            let dir = Vec2::new(
                base_dir.x + angle_offset,
                base_dir.y - 0.5 - ((i % 3) as f32) * 0.3,
            )
            .normalize();

            self.sparks.push(Spark {
                pos,
                vel: dir * speed,
                life: 0.35 + ((i % 4) as f32) * 0.1,
                max_life: 0.5,
                color_rgb: [1.0, 0.75, 0.2],
            });
        }
    }

    pub fn emit_cracks(&mut self, center: Vec2, normal: Vec2, hard: f32) {
        if hard < 0.25 {
            return;
        }

        let rays = (3.0 + hard * 5.0) as usize;
        let length = 40.0 + hard * 110.0;

        for r in 0..rays {
            let angle = (r as f32) * (std::f32::consts::PI * 2.0 / (rays as f32)) + normal.y * 0.5;
            let dir = Vec2::new(angle.cos(), angle.sin());

            let mut current = center;
            for seg in 0..3 {
                let seg_len = length / 3.0;
                let jitter = Vec2::new(
                    ((seg * 17) % 11) as f32 - 5.0,
                    ((seg * 23) % 11) as f32 - 5.0,
                );
                let next = current + dir * seg_len + jitter;

                self.cracks.push(CrackSegment {
                    start: current,
                    end: next,
                    life: 2.0 + hard * 2.0,
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

    pub fn update(&mut self, dt: f32) {
        self.sparks.retain_mut(|s| s.update(dt));
        self.shockwaves.retain_mut(|w| w.update(dt));

        self.cracks.retain_mut(|c| {
            c.life -= dt;
            c.life > 0.0
        });
    }
}

impl Default for VfxSystem {
    fn default() -> Self {
        Self::new()
    }
}
