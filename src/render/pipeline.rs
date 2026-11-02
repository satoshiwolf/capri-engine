use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::core::config::MID_POINT;
use crate::core::state::{Facing, Stance};
use crate::core::visual::VisualTransform;
use crate::render::animation::{ClipCatalog, base_clip_for_stance};
use crate::render::sprites::SpriteStrip;
use crate::render::vfx::VfxSystem;
use crate::render::window::CompositionOverlay;

const SPRITE_VERTEX_COUNT: usize = 6;
const MAX_SPRITE_VERTICES: usize = 4096;
const MAX_EFFECT_VERTICES: usize = 16_384;
const SHOCKWAVE_SEGMENTS: usize = 64;
const COPY_BYTES_PER_ROW_ALIGNMENT: usize = 256;

const SPRITE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x2,
    2 => Uint32,
    3 => Float32x4,
    4 => Float32x4
];
const EFFECT_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32];

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub layer: u32,
    pub tint: [f32; 4],
    pub params: [f32; 4],
}

impl SpriteVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &SPRITE_ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EffectVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub edge: f32,
}

impl EffectVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &EFFECT_ATTRIBUTES,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpriteDraw<'a> {
    pub strip: &'a SpriteStrip,
    pub frame_index: usize,
    pub center: Vec2,
    pub rotation: f32,
    pub scale: f32,
    pub facing: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SpriteInstance<'a> {
    pub strip: &'a SpriteStrip,
    pub frame_index: usize,
    pub transform: VisualTransform,
    pub pivot: Vec2,
    pub mirror_uv: bool,
    pub tint: [f32; 4],
    pub flash: f32,
    pub glitch: f32,
    pub alpha: f32,
    pub smear: Vec2,
    pub echo: Vec2,
}

impl<'a> SpriteInstance<'a> {
    pub fn new(
        strip: &'a SpriteStrip,
        frame_index: usize,
        transform: VisualTransform,
        pivot: Vec2,
    ) -> Self {
        Self {
            strip,
            frame_index,
            transform,
            pivot,
            mirror_uv: false,
            tint: [1.0, 1.0, 1.0, 1.0],
            flash: 0.0,
            glitch: 0.0,
            alpha: 1.0,
            smear: Vec2::ZERO,
            echo: Vec2::ZERO,
        }
    }
}

struct GpuSprite {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    sprite_pipeline: wgpu::RenderPipeline,
    effect_pipeline: wgpu::RenderPipeline,
    sprite_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    sprite_buffer: wgpu::Buffer,
    effect_buffer: wgpu::Buffer,
    sprite_cache: HashMap<String, GpuSprite>,
    origin: Vec2,
    viewport: Vec2,
}

impl Renderer {
    pub async fn new(overlay: &CompositionOverlay) -> Result<Self, String> {
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(instance_descriptor);
        let surface = overlay.create_surface(&instance)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .map_err(|error| format!("request DX12 adapter: {error}"))?;
        let adapter_info = adapter.get_info();
        println!(
            "[RENDER] Adaptador DX12: {} ({:?})",
            adapter_info.name, adapter_info.backend
        );

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Capri Renderer Device"),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("request DX12 device: {error}"))?;

        let capabilities = surface.get_capabilities(&adapter);
        if !capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            return Err("DX12 composition does not support premultiplied alpha".to_string());
        }
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| "DX12 surface exposes no texture formats".to_string())?;
        let (width, height) = overlay.size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: Default::default(),
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        overlay.commit()?;

        let sprite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Capri Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../assets/shaders/sprite.wgsl"
            ))),
        });
        let effect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Capri Effects Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../assets/shaders/effects.wgsl"
            ))),
        });

        let sprite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Capri Sprite Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let sprite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Capri Sprite Pipeline Layout"),
                bind_group_layouts: &[Some(&sprite_bind_group_layout)],
                immediate_size: 0,
            });

        let alpha_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let additive_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let sprite_color_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(alpha_blend),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let effect_color_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(additive_blend),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        };

        let sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Capri Sprite Pipeline"),
            layout: Some(&sprite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sprite_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(SpriteVertex::layout())],
            },
            primitive,
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &sprite_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &sprite_color_target,
            }),
            multiview_mask: None,
            cache: None,
        });
        let effect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Capri Effects Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &effect_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(EffectVertex::layout())],
            },
            primitive,
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &effect_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &effect_color_target,
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Capri Sprite Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let sprite_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Capri Sprite Vertex Buffer"),
            size: (MAX_SPRITE_VERTICES * std::mem::size_of::<SpriteVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let effect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Capri Effect Vertex Buffer"),
            size: (MAX_EFFECT_VERTICES * std::mem::size_of::<EffectVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (origin_x, origin_y) = overlay.origin();

        Ok(Self {
            surface,
            device,
            queue,
            config,
            sprite_pipeline,
            effect_pipeline,
            sprite_bind_group_layout,
            sampler,
            sprite_buffer,
            effect_buffer,
            sprite_cache: HashMap::new(),
            origin: Vec2::new(origin_x as f32, origin_y as f32),
            viewport: Vec2::new(width as f32, height as f32),
        })
    }

    pub fn prepare_strip(&mut self, strip: &SpriteStrip) -> Result<(), String> {
        let _ = self.sprite_bind_group(strip)?;
        Ok(())
    }

    pub fn retain_sprites(&mut self, names: &HashSet<String>) {
        self.sprite_cache.retain(|name, _| names.contains(name));
    }

    pub fn render_animation(
        &mut self,
        body: SpriteInstance<'_>,
        mouth: Option<SpriteInstance<'_>>,
        vfx: &VfxSystem,
    ) -> Result<(), String> {
        let body_bind_group = self.sprite_bind_group(body.strip)?;
        let mouth_bind_group = mouth
            .as_ref()
            .map(|instance| self.sprite_bind_group(instance.strip))
            .transpose()?;

        let mut sprite_vertices = Vec::with_capacity(MAX_SPRITE_VERTICES.min(64));
        append_sprite_layers(&mut sprite_vertices, body, self.origin, self.viewport);
        let body_vertex_count = sprite_vertices.len() as u32;
        if let Some(mouth) = mouth {
            append_plain_sprite(&mut sprite_vertices, mouth, self.origin, self.viewport);
        }
        let total_sprite_vertices = sprite_vertices.len() as u32;
        if total_sprite_vertices > 0 {
            self.queue.write_buffer(
                &self.sprite_buffer,
                0,
                bytemuck::cast_slice(&sprite_vertices),
            );
        }

        let effect_vertices = effect_vertices(vfx, self.origin, self.viewport);
        if !effect_vertices.is_empty() {
            self.queue.write_buffer(
                &self.effect_buffer,
                0,
                bytemuck::cast_slice(&effect_vertices),
            );
        }

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.reconfigure()?;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("DX12 surface validation failed".to_string());
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Capri Frame Encoder"),
            });
        {
            let color_attachment = Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            });
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Capri Transparent Render Pass"),
                color_attachments: &[color_attachment],
                ..Default::default()
            });

            if body_vertex_count > 0 {
                render_pass.set_pipeline(&self.sprite_pipeline);
                render_pass.set_bind_group(0, &body_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.sprite_buffer.slice(..));
                render_pass.draw(0..body_vertex_count, 0..1);
            }

            if let Some(mouth_bind_group) = mouth_bind_group
                && total_sprite_vertices > body_vertex_count
            {
                render_pass.set_pipeline(&self.sprite_pipeline);
                render_pass.set_bind_group(0, &mouth_bind_group, &[]);
                render_pass.draw(body_vertex_count..total_sprite_vertices, 0..1);
            }

            if !effect_vertices.is_empty() {
                render_pass.set_pipeline(&self.effect_pipeline);
                render_pass.set_vertex_buffer(0, self.effect_buffer.slice(..));
                render_pass.draw(0..effect_vertices.len() as u32, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(surface_texture);
        Ok(())
    }

    pub fn render(&mut self, sprite: SpriteDraw<'_>, vfx: &VfxSystem) -> Result<(), String> {
        let facing = if sprite.facing < 0.0 {
            Facing::Left
        } else {
            Facing::Right
        };
        let instance = SpriteInstance::new(
            sprite.strip,
            sprite.frame_index,
            VisualTransform::new(
                sprite.center,
                sprite.rotation,
                Vec2::splat(sprite.scale),
                facing,
            ),
            MID_POINT,
        );
        self.render_animation(instance, None, vfx)
    }

    fn reconfigure(&self) -> Result<(), String> {
        self.surface.configure(&self.device, &self.config);
        Ok(())
    }

    fn sprite_bind_group(&mut self, strip: &SpriteStrip) -> Result<wgpu::BindGroup, String> {
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
            .ok_or_else(|| format!("sprite '{}' was not cached", strip.name))
    }

    fn create_sprite_texture(&self, strip: &SpriteStrip) -> Result<wgpu::Texture, String> {
        let frame_size = strip.frame_width as usize * strip.frame_height as usize * 4;
        if strip.rgba_frames.len() != strip.frame_count
            || strip
                .rgba_frames
                .iter()
                .any(|frame| frame.len() != frame_size)
        {
            return Err(format!(
                "sprite '{}' has inconsistent RGBA frame dimensions",
                strip.name
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

pub fn strip_name_for_stance(stance: Stance) -> &'static str {
    ClipCatalog::spec(base_clip_for_stance(stance)).filename
}

fn append_sprite_layers(
    vertices: &mut Vec<SpriteVertex>,
    body: SpriteInstance<'_>,
    origin: Vec2,
    viewport: Vec2,
) {
    if body.echo != Vec2::ZERO {
        append_tinted_sprite(
            vertices,
            body,
            -body.echo,
            [0.20, 1.0, 0.48, 0.30 * body.alpha],
            origin,
            viewport,
        );
        append_tinted_sprite(
            vertices,
            body,
            body.echo,
            [1.0, 0.13, 0.32, 0.26 * body.alpha],
            origin,
            viewport,
        );
    }
    if body.smear != Vec2::ZERO {
        append_tinted_sprite(
            vertices,
            body,
            -body.smear,
            [0.82, 0.88, 1.0, 0.20 * body.alpha],
            origin,
            viewport,
        );
    }
    append_plain_sprite(vertices, body, origin, viewport);
}

fn append_tinted_sprite(
    vertices: &mut Vec<SpriteVertex>,
    mut instance: SpriteInstance<'_>,
    offset: Vec2,
    tint: [f32; 4],
    origin: Vec2,
    viewport: Vec2,
) {
    instance.transform.center += offset;
    instance.tint = tint;
    instance.flash = 0.0;
    append_plain_sprite(vertices, instance, origin, viewport);
}

fn append_plain_sprite(
    vertices: &mut Vec<SpriteVertex>,
    instance: SpriteInstance<'_>,
    origin: Vec2,
    viewport: Vec2,
) {
    if vertices.len() + SPRITE_VERTEX_COUNT > MAX_SPRITE_VERTICES {
        return;
    }
    let frame_count = instance.strip.frame_count;
    if frame_count == 0 {
        return;
    }
    let frame_index = instance.frame_index % frame_count;
    let width = instance.strip.frame_width as f32;
    let height = instance.strip.frame_height as f32;
    let corners = [
        (0.0, 0.0, 0.0, 0.0),
        (width, 0.0, 1.0, 0.0),
        (0.0, height, 0.0, 1.0),
        (width, 0.0, 1.0, 0.0),
        (width, height, 1.0, 1.0),
        (0.0, height, 0.0, 1.0),
    ];
    for (local_x, local_y, u, v) in corners {
        let local_delta = Vec2::new(local_x, local_y) - instance.pivot;
        let screen = instance.transform.apply(local_delta);
        vertices.push(SpriteVertex {
            position: world_to_clip(screen, origin, viewport).into(),
            uv: [if instance.mirror_uv { 1.0 - u } else { u }, v],
            layer: frame_index as u32,
            tint: instance.tint,
            params: [
                instance.flash,
                instance.glitch,
                instance.alpha,
                instance.glitch * 2.5,
            ],
        });
    }
}

pub fn sprite_vertices(sprite: SpriteDraw<'_>, origin: Vec2, viewport: Vec2) -> Vec<SpriteVertex> {
    let facing = if sprite.facing < 0.0 {
        Facing::Left
    } else {
        Facing::Right
    };
    let mut instance = SpriteInstance::new(
        sprite.strip,
        sprite.frame_index,
        VisualTransform::new(
            sprite.center,
            sprite.rotation,
            Vec2::splat(sprite.scale),
            facing,
        ),
        MID_POINT,
    );
    instance.mirror_uv = sprite.facing < 0.0;
    let mut vertices = Vec::with_capacity(SPRITE_VERTEX_COUNT);
    append_plain_sprite(&mut vertices, instance, origin, viewport);
    vertices
}

pub fn effect_vertices(vfx: &VfxSystem, origin: Vec2, viewport: Vec2) -> Vec<EffectVertex> {
    let mut vertices = Vec::with_capacity(MAX_EFFECT_VERTICES.min(4096));
    let mut push = |line| append_line(&mut vertices, line, origin, viewport);

    for spark in &vfx.sparks {
        let alpha = (spark.life / spark.max_life).clamp(0.0, 1.0);
        push((
            spark.pos - Vec2::splat(2.0),
            spark.pos + Vec2::splat(2.0),
            3.5,
            premultiply(spark.color_rgb, alpha),
        ));
        push((
            spark.pos - Vec2::splat(0.7),
            spark.pos + Vec2::splat(0.7),
            1.2,
            premultiply([1.0, 0.95, 0.72], alpha),
        ));
    }
    for wave in &vfx.shockwaves {
        let alpha = wave.intensity.clamp(0.0, 1.0) * 0.7;
        for segment in 0..SHOCKWAVE_SEGMENTS {
            let start_angle = segment as f32 * std::f32::consts::TAU / SHOCKWAVE_SEGMENTS as f32;
            let end_angle =
                (segment + 1) as f32 * std::f32::consts::TAU / SHOCKWAVE_SEGMENTS as f32;
            let start = wave.center + Vec2::new(start_angle.cos(), start_angle.sin()) * wave.radius;
            let end = wave.center + Vec2::new(end_angle.cos(), end_angle.sin()) * wave.radius;
            push((
                start,
                end,
                7.0,
                premultiply([0.12, 0.55, 0.95], alpha * 0.32),
            ));
            push((start, end, 2.2, premultiply([0.36, 0.95, 1.0], alpha)));
        }
    }
    for crack in &vfx.cracks {
        let alpha = (crack.life / 4.0).clamp(0.0, 1.0) * 0.8;
        push((
            crack.start,
            crack.end,
            4.0,
            premultiply([0.22, 0.48, 1.0], alpha * 0.35),
        ));
        push((
            crack.start,
            crack.end,
            1.25,
            premultiply([0.75, 0.95, 1.0], alpha),
        ));
    }
    if let Some(stars) = &vfx.dizzy {
        let alpha = (stars.life / 2.4).clamp(0.0, 1.0);
        for index in 0..4 {
            let angle = stars.phase + index as f32 * std::f32::consts::FRAC_PI_2;
            let center = stars.center + Vec2::new(angle.cos() * 36.0, angle.sin() * 12.0 - 28.0);
            let size = 4.0 + (angle.sin() + 1.0) * 1.5;
            push((
                center - Vec2::new(size, size * 0.25),
                center + Vec2::new(size, size * 0.25),
                2.8,
                premultiply([1.0, 0.72, 0.18], alpha),
            ));
            push((
                center - Vec2::new(size * 0.25, size),
                center + Vec2::new(size * 0.25, size),
                2.8,
                premultiply([1.0, 0.72, 0.18], alpha),
            ));
        }
    }

    vertices.truncate(MAX_EFFECT_VERTICES);
    vertices
}

fn append_line(
    vertices: &mut Vec<EffectVertex>,
    (start, end, width, color): (Vec2, Vec2, f32, [f32; 4]),
    origin: Vec2,
    viewport: Vec2,
) {
    if vertices.len() + 6 > MAX_EFFECT_VERTICES {
        return;
    }
    let delta = end - start;
    let length = delta.length();
    if length < f32::EPSILON {
        return;
    }
    let normal = Vec2::new(-delta.y, delta.x) / length * (width * 0.5);
    let a = world_to_clip(start + normal, origin, viewport).into();
    let b = world_to_clip(start - normal, origin, viewport).into();
    let c = world_to_clip(end + normal, origin, viewport).into();
    let d = world_to_clip(end - normal, origin, viewport).into();
    let points = [
        (a, 1.0),
        (b, -1.0),
        (c, 1.0),
        (c, 1.0),
        (b, -1.0),
        (d, -1.0),
    ];
    for (position, edge) in points {
        vertices.push(EffectVertex {
            position,
            color,
            edge,
        });
    }
}

fn premultiply(color: [f32; 3], alpha: f32) -> [f32; 4] {
    [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha]
}

fn world_to_clip(point: Vec2, origin: Vec2, viewport: Vec2) -> Vec2 {
    Vec2::new(
        (point.x - origin.x) / viewport.x * 2.0 - 1.0,
        1.0 - (point.y - origin.y) / viewport.y * 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::hittest::AlphaBitmask;

    fn strip() -> SpriteStrip {
        SpriteStrip {
            name: "test".to_string(),
            frame_count: 2,
            frame_width: 10,
            frame_height: 20,
            fps: 1.0,
            masks: vec![
                AlphaBitmask::new(10, 20, vec![255; 200]),
                AlphaBitmask::new(10, 20, vec![255; 200]),
            ],
            rgba_frames: vec![vec![255; 800], vec![255; 800]],
        }
    }

    #[test]
    fn stance_selects_expected_sprite() {
        assert_eq!(strip_name_for_stance(Stance::Held), "dangle.png");
        assert_eq!(strip_name_for_stance(Stance::Sit), "perch.png");
        assert_eq!(strip_name_for_stance(Stance::Fly), "dangle.png");
        assert_eq!(strip_name_for_stance(Stance::Dazed), "laydown.png");
    }

    #[test]
    fn sprite_vertices_select_matching_frame_layer_and_uv() {
        let vertices = sprite_vertices(
            SpriteDraw {
                strip: &strip(),
                frame_index: 1,
                center: Vec2::new(100.0, 100.0),
                rotation: 0.0,
                scale: 1.0,
                facing: 1.0,
            },
            Vec2::ZERO,
            Vec2::new(200.0, 200.0),
        );
        assert_eq!(vertices.len(), SPRITE_VERTEX_COUNT);
        assert!(vertices.iter().all(|vertex| vertex.layer == 1));
        assert!(
            vertices
                .iter()
                .all(|vertex| (0.0..=1.0).contains(&vertex.uv[0]))
        );
    }

    #[test]
    fn nonuniform_scale_changes_width_and_height_independently() {
        let strip = strip();
        let mut instance = SpriteInstance::new(
            &strip,
            0,
            VisualTransform::new(Vec2::ZERO, 0.0, Vec2::new(2.0, 0.5), Facing::Right),
            Vec2::new(5.0, 10.0),
        );
        let mut vertices = Vec::new();
        append_plain_sprite(&mut vertices, instance, Vec2::ZERO, Vec2::splat(100.0));
        let width = vertices.iter().map(|v| v.position[0]).fold(0.0, f32::max)
            - vertices
                .iter()
                .map(|v| v.position[0])
                .fold(f32::INFINITY, f32::min);
        let height = vertices.iter().map(|v| v.position[1]).fold(0.0, f32::max)
            - vertices
                .iter()
                .map(|v| v.position[1])
                .fold(f32::INFINITY, f32::min);
        assert!(width > height);

        instance.echo = Vec2::X;
        let mut layered = Vec::new();
        append_sprite_layers(&mut layered, instance, Vec2::ZERO, Vec2::splat(100.0));
        assert_eq!(layered.len(), SPRITE_VERTEX_COUNT * 3);
    }

    #[test]
    fn empty_effects_generate_no_geometry() {
        assert!(effect_vertices(&VfxSystem::new(), Vec2::ZERO, Vec2::ONE).is_empty());
    }

    #[test]
    fn spark_generates_two_quads() {
        let mut vfx = VfxSystem::new();
        vfx.emit_sparks(Vec2::new(20.0, 20.0), 1, Vec2::new(1.0, 0.0));
        assert_eq!(
            effect_vertices(&vfx, Vec2::ZERO, Vec2::splat(100.0)).len(),
            SPRITE_VERTEX_COUNT * 2
        );
    }

    #[test]
    fn shockwave_generates_full_ring_without_exceeding_capacity() {
        let mut vfx = VfxSystem::new();
        vfx.emit_shockwave(Vec2::new(50.0, 50.0), 1.0);
        assert_eq!(vfx.shockwaves.len(), 1);
        assert_eq!(
            effect_vertices(&vfx, Vec2::ZERO, Vec2::splat(100.0)).len(),
            SHOCKWAVE_SEGMENTS * SPRITE_VERTEX_COUNT * 2
        );
    }
}
