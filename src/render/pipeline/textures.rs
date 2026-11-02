use std::collections::HashSet;

use crate::core::error::{EngineError, EngineResult};
use crate::render::sprites::SpriteStrip;

use super::Renderer;

const COPY_BYTES_PER_ROW_ALIGNMENT: usize = 256;

pub(super) struct GpuSprite {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub fn prepare_strip(&mut self, strip: &SpriteStrip) -> EngineResult<()> {
        let _ = self.sprite_bind_group(strip)?;
        Ok(())
    }

    pub fn retain_sprites(&mut self, names: &HashSet<String>) {
        self.sprite_cache.retain(|name, _| names.contains(name));
    }

    pub(super) fn sprite_bind_group(
        &mut self,
        strip: &SpriteStrip,
    ) -> EngineResult<wgpu::BindGroup> {
        if !self.sprite_cache.contains_key(&strip.name) {
            let texture = self.create_sprite_texture(strip)?;
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Capri Sprite Array View"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Capri Sprite Bind Group"),
                layout: &self.sprite_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.sprite_cache.insert(
                strip.name.clone(),
                GpuSprite {
                    _texture: texture,
                    bind_group,
                },
            );
        }

        self.sprite_cache
            .get(&strip.name)
            .map(|sprite| sprite.bind_group.clone())
            .ok_or_else(|| {
                EngineError::gpu(
                    "resolve sprite bind group",
                    format!("sprite '{}' was not cached", strip.name),
                )
            })
    }

    fn create_sprite_texture(&self, strip: &SpriteStrip) -> EngineResult<wgpu::Texture> {
        let frame_size = strip.frame_width as usize * strip.frame_height as usize * 4;
        if strip.rgba_frames.len() != strip.frame_count
            || strip
                .rgba_frames
                .iter()
                .any(|frame| frame.len() != frame_size)
        {
            return Err(EngineError::asset(
                "upload sprite",
                format!(
                    "sprite '{}' has inconsistent RGBA frame dimensions",
                    strip.name
                ),
            ));
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Capri Sprite Array Texture"),
            size: wgpu::Extent3d {
                width: strip.frame_width,
                height: strip.frame_height,
                depth_or_array_layers: strip.frame_count as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let unpadded_row_bytes = strip.frame_width as usize * 4;
        let padded_row_bytes = unpadded_row_bytes.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT)
            * COPY_BYTES_PER_ROW_ALIGNMENT;
        for (layer, frame) in strip.rgba_frames.iter().enumerate() {
            let mut texture_data = vec![0; padded_row_bytes * strip.frame_height as usize];
            for row in 0..strip.frame_height as usize {
                let source = row * unpadded_row_bytes;
                let destination = row * padded_row_bytes;
                texture_data[destination..destination + unpadded_row_bytes]
                    .copy_from_slice(&frame[source..source + unpadded_row_bytes]);
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &texture_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes as u32),
                    rows_per_image: Some(strip.frame_height),
                },
                wgpu::Extent3d {
                    width: strip.frame_width,
                    height: strip.frame_height,
                    depth_or_array_layers: 1,
                },
            );
        }
        Ok(texture)
    }
}
