use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use capri_engine::core::state::Facing;
use capri_engine::core::visual::VisualTransform;
use capri_engine::render::hittest::AlphaBitmask;
use capri_engine::render::window::OverlayInput;
use glam::Vec2;

const TARGET_NANOS_PER_HIT: f64 = 10_000.0;
const DEFAULT_ITERATIONS: u64 = 2_000_000;

fn main() {
    let iterations = std::env::var("CAPRI_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ITERATIONS)
        .max(100_000);
    let mut alpha = vec![0; 240 * 384];
    for y in 40..344 {
        for x in 40..200 {
            alpha[y * 240 + x] = 255;
        }
    }
    let mask = AlphaBitmask::new(240, 384, alpha);
    let transform = VisualTransform::new(
        Vec2::new(960.0, 540.0),
        17.0,
        Vec2::new(1.15, 0.92),
        Facing::Right,
    );
    let pivot = Vec2::new(118.0, 232.0);
    let input = OverlayInput::new();
    input.set_hit_test(Some(Arc::new(mask)), transform, pivot);

    for index in 0..20_000 {
        let point = sample_point(index);
        black_box(input.hit_test(point));
    }

    let started = Instant::now();
    let mut hits = 0_u64;
    for index in 0..iterations {
        let point = sample_point(index);
        hits += u64::from(black_box(input.hit_test(point)));
    }
    let elapsed = started.elapsed();
    let nanos_per_hit = elapsed.as_nanos() as f64 / iterations as f64;

    println!(
        "hit_test: {:.1} ns/op, {:.1} us/op, {} hits, {} iterations",
        nanos_per_hit,
        nanos_per_hit / 1_000.0,
        hits,
        iterations
    );
    if nanos_per_hit >= TARGET_NANOS_PER_HIT {
        eprintln!(
            "hit_test exceeded target: {:.1} ns/op >= {:.1} ns/op",
            nanos_per_hit, TARGET_NANOS_PER_HIT
        );
        std::process::exit(1);
    }
}

fn sample_point(index: u64) -> Vec2 {
    let x = 650.0 + ((index.wrapping_mul(37) % 620) as f32);
    let y = 220.0 + ((index.wrapping_mul(53) % 640) as f32);
    Vec2::new(x, y)
}
