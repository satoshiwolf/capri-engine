use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::render::vfx::VfxSystem;

use super::geometry::world_to_clip;

pub(super) const MAX_EFFECT_VERTICES: usize = 16_384;
const SHOCKWAVE_SEGMENTS: usize = 64;
const QUAD_VERTEX_COUNT: usize = 6;

const EFFECT_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32];

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EffectVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub edge: f32,
}

impl EffectVertex {
    pub(super) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &EFFECT_ATTRIBUTES,
        }
    }
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
    if vertices.len() + QUAD_VERTEX_COUNT > MAX_EFFECT_VERTICES {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            QUAD_VERTEX_COUNT * 2
        );
    }

    #[test]
    fn shockwave_generates_full_ring_without_exceeding_capacity() {
        let mut vfx = VfxSystem::new();
        vfx.emit_shockwave(Vec2::new(50.0, 50.0), 1.0);
        assert_eq!(vfx.shockwaves.len(), 1);
        assert_eq!(
            effect_vertices(&vfx, Vec2::ZERO, Vec2::splat(100.0)).len(),
            SHOCKWAVE_SEGMENTS * QUAD_VERTEX_COUNT * 2
        );
    }
}
