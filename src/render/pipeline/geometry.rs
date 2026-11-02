use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::core::config::MID_POINT;
use crate::core::state::Facing;
use crate::core::visual::VisualTransform;
use crate::render::sprites::SpriteStrip;

pub(super) const SPRITE_VERTEX_COUNT: usize = 6;
pub(super) const MAX_SPRITE_VERTICES: usize = 4096;

const SPRITE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x2,
    2 => Uint32,
    3 => Float32x4,
    4 => Float32x4
];

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
    pub(super) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &SPRITE_ATTRIBUTES,
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

pub(super) fn append_sprite_layers(
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

pub(super) fn append_plain_sprite(
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

pub(super) fn world_to_clip(point: Vec2, origin: Vec2, viewport: Vec2) -> Vec2 {
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
}
