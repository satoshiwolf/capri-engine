use crate::core::config::MAX_SPRITE_RESIDENT_BYTES;
use crate::core::error::{EngineError, EngineResult};
use crate::render::animation::{ClipCatalog, ClipId};
use crate::render::hittest::AlphaBitmask;
use image::GenericImageView;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

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
    ) -> EngineResult<Self> {
        if frame_width == 0 || frame_height == 0 {
            return Err(EngineError::asset(
                "load sprite",
                format!("sprite '{name}' has an empty cell size"),
            ));
        }
        let img = image::open(&path).map_err(|source| EngineError::Image {
            operation: "open sprite",
            source,
        })?;
        let (total_w, total_h) = img.dimensions();
        if total_h != frame_height || total_w % frame_width != 0 {
            return Err(EngineError::asset(
                "validate sprite",
                format!(
                    "sprite '{name}' dimensions {}x{} do not match {frame_width}x{frame_height} cells",
                    total_w, total_h
                ),
            ));
        }

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

    pub fn from_spec(path: &Path, spec: &crate::render::animation::ClipSpec) -> EngineResult<Self> {
        let asset_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                EngineError::asset(
                    "resolve sprite path",
                    format!("sprite path {:?} has no UTF-8 filename", path),
                )
            })?;
        Self::load_from_file(
            path,
            asset_name,
            spec.frame_width,
            spec.frame_height,
            spec.fps,
        )
    }

    pub fn frame_index_at(&self, time_secs: f64) -> usize {
        if self.frame_count == 0 {
            return 0;
        }
        let total_frames = (time_secs.max(0.0) * (self.fps as f64)) as usize;
        total_frames % self.frame_count
    }

    pub fn frame_mask(&self, frame_index: usize) -> Option<&AlphaBitmask> {
        if self.masks.is_empty() {
            None
        } else {
            self.masks.get(frame_index % self.masks.len())
        }
    }

    pub fn release_rgba_frames(&mut self) {
        self.rgba_frames.clear();
        self.rgba_frames.shrink_to_fit();
    }

    pub fn mask_bytes(&self) -> usize {
        self.masks.iter().map(|mask| mask.alpha_data().len()).sum()
    }

    pub fn decoded_bytes(&self) -> usize {
        self.mask_bytes() + self.rgba_frames.iter().map(Vec::len).sum::<usize>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteAssetKey {
    pub clip: ClipId,
    pub flipped: bool,
}

impl SpriteAssetKey {
    pub fn name(self) -> &'static str {
        let spec = ClipCatalog::spec(self.clip);
        if self.flipped {
            spec.flipped_filename.unwrap_or(spec.filename)
        } else {
            spec.filename
        }
    }
}

struct LoadRequest {
    key: SpriteAssetKey,
    path: PathBuf,
    spec: crate::render::animation::ClipSpec,
}

struct LoadResponse {
    key: SpriteAssetKey,
    result: EngineResult<SpriteStrip>,
}

pub struct SpriteManager {
    #[deprecated(
        since = "0.3.28",
        note = "Use `strips()` for immutable access or the manager methods. This field will be removed in 0.4.0."
    )]
    pub strips: HashMap<String, SpriteStrip>,
    base_dir: PathBuf,
    request_tx: Option<Sender<LoadRequest>>,
    response_rx: Option<Receiver<LoadResponse>>,
    pending: HashSet<SpriteAssetKey>,
    usage: HashMap<String, u64>,
    usage_tick: u64,
}

#[allow(deprecated)]
impl SpriteManager {
    pub fn new() -> Self {
        Self::with_assets_dir("assets/sprites")
    }

    pub fn with_assets_dir<P: AsRef<Path>>(base_dir: P) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<LoadRequest>();
        let (response_tx, response_rx) = mpsc::channel::<LoadResponse>();

        thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                let result = SpriteStrip::from_spec(&request.path, &request.spec);
                let _ = response_tx.send(LoadResponse {
                    key: request.key,
                    result,
                });
            }
        });

        Self {
            strips: HashMap::new(),
            base_dir: base_dir.as_ref().to_path_buf(),
            request_tx: Some(request_tx),
            response_rx: Some(response_rx),
            pending: HashSet::new(),
            usage: HashMap::new(),
            usage_tick: 0,
        }
    }

    pub fn request(&mut self, clip: ClipId, prefer_flipped: bool) -> SpriteAssetKey {
        let spec = ClipCatalog::spec(clip);
        let flipped = prefer_flipped && spec.flipped_filename.is_some();
        let key = SpriteAssetKey { clip, flipped };
        let name = key.name();
        if self.strips.contains_key(name) || self.pending.contains(&key) {
            return key;
        }
        if let Some(sender) = &self.request_tx {
            self.pending.insert(key);
            let _ = sender.send(LoadRequest {
                key,
                path: self.base_dir.join(name),
                spec,
            });
        }
        key
    }

    pub fn poll_loads(&mut self) -> Vec<String> {
        let mut loaded_names = Vec::new();
        let mut failures = Vec::new();
        if let Some(receiver) = &self.response_rx {
            while let Ok(response) = receiver.try_recv() {
                self.pending.remove(&response.key);
                match response.result {
                    Ok(strip) => {
                        loaded_names.push(strip.name.clone());
                        self.strips.insert(strip.name.clone(), strip);
                    }
                    Err(error) => failures.push(error),
                }
            }
        }
        for error in failures {
            eprintln!("[ASSET ERROR] {error}");
        }
        loaded_names
    }

    pub fn load_blocking(&mut self, clip: ClipId, prefer_flipped: bool) -> EngineResult<()> {
        let spec = ClipCatalog::spec(clip);
        let flipped = prefer_flipped && spec.flipped_filename.is_some();
        let key = SpriteAssetKey { clip, flipped };
        let name = key.name();
        if self.strips.contains_key(name) {
            return Ok(());
        }
        let strip = SpriteStrip::from_spec(&self.base_dir.join(name), &spec)?;
        self.strips.insert(name.to_string(), strip);
        Ok(())
    }

    pub fn get(&mut self, clip: ClipId, prefer_flipped: bool) -> Option<&SpriteStrip> {
        let spec = ClipCatalog::spec(clip);
        let flipped = prefer_flipped && spec.flipped_filename.is_some();
        let name = if flipped {
            spec.flipped_filename
        } else {
            Some(spec.filename)
        }?;
        self.touch(name);
        self.strips.get(name)
    }

    pub fn get_mut(&mut self, clip: ClipId, prefer_flipped: bool) -> Option<&mut SpriteStrip> {
        let spec = ClipCatalog::spec(clip);
        let flipped = prefer_flipped && spec.flipped_filename.is_some();
        let name = if flipped {
            spec.flipped_filename
        } else {
            Some(spec.filename)
        }?;
        self.touch(name);
        self.strips.get_mut(name)
    }

    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut SpriteStrip> {
        self.touch(name);
        self.strips.get_mut(name)
    }

    pub fn get_strip(&self, name: &str) -> Option<&SpriteStrip> {
        self.strips.get(name)
    }

    pub fn strips(&self) -> &HashMap<String, SpriteStrip> {
        &self.strips
    }

    pub fn load_essential_sprites<P: AsRef<Path>>(&mut self, base_dir: P) {
        self.base_dir = base_dir.as_ref().to_path_buf();
        for id in [
            ClipId::Idle,
            ClipId::Walk,
            ClipId::Dangle,
            ClipId::Perch,
            ClipId::Mouth,
        ] {
            let spec = ClipCatalog::spec(id);
            match SpriteStrip::from_spec(&self.base_dir.join(spec.filename), &spec) {
                Ok(strip) => {
                    self.strips.insert(strip.name.clone(), strip);
                }
                Err(error) => eprintln!("[ASSET ERROR] {error}"),
            }
        }
    }

    pub fn resident_mask_bytes(&self) -> usize {
        self.strips.values().map(SpriteStrip::mask_bytes).sum()
    }

    pub fn trim_to_budget(&mut self, keep: &[SpriteAssetKey]) -> Vec<String> {
        let keep_names: HashSet<&str> = keep.iter().map(|key| key.name()).collect();
        while self.resident_mask_bytes() > MAX_SPRITE_RESIDENT_BYTES {
            let candidate = self
                .usage
                .iter()
                .filter(|(name, _)| !keep_names.contains(name.as_str()))
                .min_by_key(|(_, used)| **used)
                .map(|(name, _)| name.clone());
            let Some(name) = candidate else {
                break;
            };
            self.strips.remove(&name);
            self.usage.remove(&name);
        }
        self.strips.keys().cloned().collect()
    }

    fn touch(&mut self, name: &str) {
        self.usage_tick = self.usage_tick.wrapping_add(1);
        self.usage.insert(name.to_string(), self.usage_tick);
    }
}

impl Default for SpriteManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::MAX_SPRITE_RESIDENT_BYTES;
    use std::path::Path;

    #[test]
    fn catalog_metadata_matches_asset_dimensions() {
        let base = Path::new("assets/sprites");
        for id in ClipId::ALL {
            if id == ClipId::Mouth {
                continue;
            }
            let spec = ClipCatalog::spec(id);
            assert_sprite_dimensions(&base.join(spec.filename), &spec);
            if let Some(flipped) = spec.flipped_filename {
                assert_sprite_dimensions(&base.join(flipped), &spec);
            }
        }
    }

    fn assert_sprite_dimensions(path: &Path, spec: &crate::render::animation::ClipSpec) {
        let (width, height) = image::image_dimensions(path).expect("sprite dimensions");
        assert_eq!(height, spec.frame_height, "{}", path.display());
        assert_eq!(width % spec.frame_width, 0, "{}", path.display());
        assert_eq!(
            width as usize / spec.frame_width as usize,
            spec.frame_count,
            "{}",
            path.display()
        );
    }

    #[test]
    fn resident_mask_budget_is_below_limit_for_common_pose_set() {
        let total = [
            ClipId::Idle,
            ClipId::Dangle,
            ClipId::Perch,
            ClipId::Laydown,
            ClipId::Mouth,
        ]
        .into_iter()
        .map(|id| ClipCatalog::spec(id).resident_mask_bytes())
        .sum::<usize>();

        assert!(total < MAX_SPRITE_RESIDENT_BYTES);
    }

    #[test]
    fn malformed_cell_dimensions_are_rejected_by_spec_validation() {
        let spec = crate::render::animation::ClipSpec {
            id: ClipId::Idle,
            filename: "idle.png",
            flipped_filename: None,
            frame_count: 16,
            frame_width: 239,
            frame_height: 384,
            fps: 8.0,
            loop_mode: crate::render::animation::LoopMode::Loop,
            pivot: glam::Vec2::ZERO,
            display_scale: 1.0,
        };
        let result = SpriteStrip::from_spec(Path::new("assets/sprites/idle.png"), &spec);
        assert!(result.is_err());
    }

    #[test]
    fn flipped_asset_keys_use_artist_drawn_variant() {
        let key = SpriteAssetKey {
            clip: ClipId::PerchTalk,
            flipped: true,
        };
        assert_eq!(key.name(), "perch_talk_flipped.png");
    }

    #[test]
    fn manager_exposes_immutable_sprite_catalog() {
        let manager = SpriteManager::with_assets_dir("assets/sprites");
        assert!(manager.strips().is_empty());
    }

    #[test]
    fn catalog_covers_every_animated_png() {
        let mut expected = HashSet::new();
        for id in ClipId::ALL {
            let spec = ClipCatalog::spec(id);
            expected.insert(spec.filename);
            if let Some(flipped) = spec.flipped_filename {
                expected.insert(flipped);
            }
        }

        for entry in std::fs::read_dir("assets/sprites").expect("sprite directory") {
            let path = entry.expect("sprite entry").path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if path.extension().and_then(|value| value.to_str()) == Some("png")
                && name != "face.png"
            {
                assert!(expected.contains(name), "catalog is missing {name}");
            }
        }
    }
}
