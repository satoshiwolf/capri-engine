use crate::render::hittest::AlphaBitmask;
use image::GenericImageView;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SpriteStrip {
    pub name: String,
    pub frame_count: usize,
    pub frame_width: u32,
    pub frame_height: u32,
    pub fps: f32,
    pub masks: Vec<AlphaBitmask>,
    pub rgba_frames: Vec<Vec<u8>>,
}

impl SpriteStrip {
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
        name: &str,
        frame_width: u32,
        frame_height: u32,
        fps: f32,
    ) -> Result<Self, String> {
        let img =
            image::open(&path).map_err(|e| format!("Error abriendo {:?}: {}", path.as_ref(), e))?;
        let (total_w, total_h) = img.dimensions();

        let frame_count = (total_w / frame_width) as usize;
        let mut masks = Vec::with_capacity(frame_count);
        let mut rgba_frames = Vec::with_capacity(frame_count);

        let rgba_img = img.to_rgba8();

        for i in 0..frame_count {
            let offset_x = (i as u32) * frame_width;
            let mut alpha_data = Vec::with_capacity((frame_width * frame_height) as usize);
            let mut frame_rgba = Vec::with_capacity((frame_width * frame_height * 4) as usize);

            for y in 0..frame_height {
                for x in 0..frame_width {
                    let pixel = rgba_img.get_pixel(offset_x + x, y);
                    alpha_data.push(pixel[3]);
                    frame_rgba.extend_from_slice(&pixel.0);
                }
            }

            masks.push(AlphaBitmask::new(frame_width, frame_height, alpha_data));
            rgba_frames.push(frame_rgba);
        }

        Ok(Self {
            name: name.to_string(),
            frame_count,
            frame_width,
            frame_height,
            fps,
            masks,
            rgba_frames,
        })
    }

    pub fn frame_index_at(&self, time_secs: f64) -> usize {
        if self.frame_count == 0 {
            return 0;
        }
        let total_frames = (time_secs * (self.fps as f64)) as usize;
        total_frames % self.frame_count
    }
}

pub struct SpriteManager {
    pub strips: HashMap<String, SpriteStrip>,
}

impl SpriteManager {
    pub fn new() -> Self {
        Self {
            strips: HashMap::new(),
        }
    }

    pub fn load_essential_sprites<P: AsRef<Path>>(&mut self, base_dir: P) {
        let base = base_dir.as_ref();

        let idle_path = base.join("idle.png");
        if idle_path.exists()
            && let Ok(strip) = SpriteStrip::load_from_file(&idle_path, "idle", 240, 384, 12.0)
        {
            self.strips.insert("idle".to_string(), strip);
        }

        let walk_path = base.join("walk.png");
        if walk_path.exists()
            && let Ok(strip) = SpriteStrip::load_from_file(&walk_path, "walk", 240, 384, 14.0)
        {
            self.strips.insert("walk".to_string(), strip);
        }

        let dangle_path = base.join("dangle.png");
        if dangle_path.exists()
            && let Ok(strip) = SpriteStrip::load_from_file(&dangle_path, "dangle", 240, 384, 20.0)
        {
            self.strips.insert("dangle".to_string(), strip);
        }

        let perch_path = base.join("perch.png");
        if perch_path.exists()
            && let Ok(strip) = SpriteStrip::load_from_file(&perch_path, "perch", 196, 384, 30.0)
        {
            self.strips.insert("perch".to_string(), strip);
        }

        let mouth_path = base.join("mouth.png");
        if mouth_path.exists()
            && let Ok(strip) = SpriteStrip::load_from_file(&mouth_path, "mouth", 240, 384, 1.0)
        {
            self.strips.insert("mouth".to_string(), strip);
        }
    }

    pub fn get_strip(&self, name: &str) -> Option<&SpriteStrip> {
        self.strips.get(name)
    }
}

impl Default for SpriteManager {
    fn default() -> Self {
        Self::new()
    }
}
