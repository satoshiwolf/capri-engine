use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use glam::Vec2;
use tokio::sync::{broadcast, mpsc as tokio_mpsc};

use capri_engine::core::config::{
    DEFAULT_RENDER_HZ, DEFAULT_WS_PORT, DIZZY_VFX_OFFSET_Y_PX, GATEWAY_COMMAND_CAPACITY,
    GATEWAY_EVENT_CAPACITY, IMPACT_HARDNESS_NORMALIZER_PX_SEC, IMPACT_SPARK_COUNT,
    METRICS_SAMPLE_CAPACITY, MOUTH_GLITCH_SCALE, SKID_LANDING_SPARK_COUNT,
    SKID_SPARK_INTERVAL_SECS,
};
use capri_engine::core::events::EngineEvent;
use capri_engine::core::metrics::FrameMetrics;
use capri_engine::core::state::{Facing, Stance};
use capri_engine::core::visual::VisualTransform;
use capri_engine::desktop::monitors::{enumerate_monitors, virtual_desktop_bounds};
use capri_engine::desktop::{LedgeScanner, ledge_surfaces, process_working_set_bytes};
use capri_engine::gateway::protocol::{IncomingCommand, OutgoingEvent};
use capri_engine::gateway::{
    CommandContext, WsGatewayServer, dispatch_command, send_engine_telemetry,
};
use capri_engine::physics::collision::LedgeSurface;
use capri_engine::physics::{PhysicsSnapshot, PhysicsWorker};
use capri_engine::render::animation::{
    AnimationAction, AnimationController, AnimationInput, ClipCatalog, ClipId,
};
use capri_engine::render::coordinator::{
    RenderedBody, current_motion, prepare_loaded_strip, prewarm_animation, render_transform,
    select_body,
};
use capri_engine::render::hittest::{AlphaBitmask, create_default_mask};
use capri_engine::render::pipeline::{Renderer, SpriteInstance};
use capri_engine::render::sprites::SpriteManager;
use capri_engine::render::vfx::VfxSystem;
use capri_engine::render::window::{
    CompositionOverlay, IS_RUNNING, pump_messages, should_capture_input, wait_for_messages,
};

#[derive(Clone, Copy)]
enum DragState {
    Idle,
    PendingPickup(Vec2),
    Held,
}

impl DragState {
    fn advance(
        self,
        mouse_down: bool,
        cursor_pos: Vec2,
        timestamp: f64,
        dt: f32,
        worker: &PhysicsWorker,
    ) -> Self {
        match self {
            Self::PendingPickup(_) if !mouse_down => Self::Idle,
            Self::PendingPickup(grab_pos) => {
                if worker.pick_up(grab_pos) {
                    Self::Held
                } else {
                    Self::PendingPickup(grab_pos)
                }
            }
            Self::Held if mouse_down => {
                let _ = worker.update_held(cursor_pos, timestamp, dt);
                Self::Held
            }
            Self::Held => {
                if worker.release() {
                    Self::Idle
                } else {
                    Self::Held
                }
            }
            Self::Idle => Self::Idle,
        }
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=========================================================");
    println!("          CAPRI ENGINE v0.3.53 - Cyberpunk Physics Host  ");
    println!("=========================================================");

    let (cmd_tx, mut cmd_rx) = tokio_mpsc::channel::<IncomingCommand>(GATEWAY_COMMAND_CAPACITY);
    let (event_tx, _event_rx) = broadcast::channel::<OutgoingEvent>(GATEWAY_EVENT_CAPACITY);

    let server_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let server =
            WsGatewayServer::with_bounded_channel(DEFAULT_WS_PORT, cmd_tx, server_event_tx);
        if let Err(error) = server.try_run().await {
            eprintln!("[GATEWAY ERROR] Error en servidor WebSocket: {error}");
        }
    });

    let mut sprite_manager = SpriteManager::with_assets_dir("assets/sprites");
    sprite_manager.try_load_blocking(ClipId::Idle, false)?;
    sprite_manager.try_load_blocking(ClipId::Arrive, false)?;
    println!("[ASSETS] Catalogo de animacion inicializado.");

    let monitors = enumerate_monitors();
    println!("[DESKTOP] Monitores detectados: {}", monitors.len());
    for (index, monitor) in monitors.iter().enumerate() {
        println!(
            "  -> Monitor {}: {} | Límites: {:?} | Primario: {}",
            index, monitor.name, monitor.bounds, monitor.is_primary
        );
    }
    let (vx, vy, vr, vb) = virtual_desktop_bounds();
    let initial_pos = Vec2::new((vx + (vr - vx) / 2) as f32, (vb - 400) as f32);
    let target_fps = std::env::var("CAPRI_TARGET_FPS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|fps| *fps == 60 || *fps == 120)
        .unwrap_or(DEFAULT_RENDER_HZ);

    let mut overlay = CompositionOverlay::try_create()?;
    let hwnd = overlay.hwnd();
    println!(
        "[WINDOW] Ventana overlay transparente creada (HWND: {:?})",
        hwnd
    );
    let mut renderer = Renderer::try_new(&overlay).await?;

    let physics_worker = PhysicsWorker::try_spawn(
        initial_pos,
        (vx as f32, vy as f32, vr as f32, vb as f32),
        target_fps,
    )?;
    let mut physics = PhysicsSnapshot::initial(initial_pos);
    let mut animation = AnimationController::new();
    animation.play_action(AnimationAction::Arrive);
    let mut vfx = VfxSystem::new();
    let default_mask = create_default_mask();

    let overlay_hwnd = hwnd.0 as isize;
    let ledge_scanner = LedgeScanner::new(overlay_hwnd);

    let mut cached_ledges = Vec::new();
    let mut current_monitor_idx = 0;
    let target_frame_duration = Duration::from_secs_f32(1.0 / target_fps as f32);
    let start_time = Instant::now();
    let mut last_frame = Instant::now();
    let mut last_cursor = Vec2::ZERO;
    let mut was_mouse_down = false;
    let mut drag_state = DragState::Idle;
    let mut submitted_surfaces: Option<Vec<LedgeSurface>> = None;
    let mut was_dazed = false;
    let mut last_skid_spark = Instant::now() - Duration::from_secs(1);
    let mut frame_metrics = FrameMetrics::new(METRICS_SAMPLE_CAPACITY);
    let mut reported_stance = physics.stance();
    let mut last_hit_mask_key = None;
    let mut hit_mask_cache: Option<Arc<AlphaBitmask>> = None;
    let mut last_body = RenderedBody::initial();

    println!("[ENGINE] Bucle visual iniciado a {target_fps} Hz. Listo.");

    while IS_RUNNING.load(Ordering::SeqCst) {
        if physics_worker.is_finished() {
            eprintln!("[PHYSICS ERROR] El worker de fisica termino inesperadamente");
            break;
        }
        let frame_start = Instant::now();
        let dt = (frame_start - last_frame).as_secs_f32().clamp(0.001, 0.05);
        last_frame = frame_start;
        let timestamp = (frame_start - start_time).as_secs_f64();

        if !pump_messages() {
            break;
        }
        sprite_manager.poll_loads();
        while let Ok(ledges) = ledge_scanner.try_recv() {
            cached_ledges = ledges;
        }
        let surfaces = ledge_surfaces(&cached_ledges);
        if submitted_surfaces.as_ref() != Some(&surfaces)
            && physics_worker.set_surfaces(surfaces.clone())
        {
            submitted_surfaces = Some(surfaces.clone());
        }

        let (cursor_x, cursor_y) = overlay.try_poll_cursor()?;
        let cursor_screen = Vec2::new(cursor_x as f32, cursor_y as f32);
        let cursor_motion = (cursor_screen - last_cursor) / dt;
        last_cursor = cursor_screen;
        let mouse_down = overlay.input().mouse_down();

        let input_sample = animation.sample();
        let input_body = select_body(&mut sprite_manager, input_sample, last_body);
        prepare_loaded_strip(
            &mut renderer,
            &mut sprite_manager,
            input_body.clip(),
            input_body.flipped(),
        )?;

        let input_transform = render_transform(&physics, input_body.flipped());
        let hit_mask = sprite_manager
            .get(input_body.clip(), input_body.flipped())
            .and_then(|strip| strip.frame_mask(input_body.frame_index()));
        let hit_mask_key = hit_mask.map(|_| {
            (
                input_body.clip(),
                input_body.flipped(),
                input_body.frame_index(),
            )
        });
        if hit_mask_key != last_hit_mask_key {
            last_hit_mask_key = hit_mask_key;
            let mask = hit_mask.unwrap_or(&default_mask).clone();
            hit_mask_cache = Some(Arc::new(mask));
        }
        let input_transform = VisualTransform {
            scale: input_transform.scale * input_body.display_scale(),
            ..input_transform
        };
        overlay
            .input()
            .set_hit_test(hit_mask_cache.clone(), input_transform, input_body.pivot());
        let is_over_capri = overlay.input().hit_test(cursor_screen);
        let capture_input = should_capture_input(is_over_capri, mouse_down, physics.stance());
        overlay.try_sync_input_capture(capture_input)?;

        if mouse_down && !was_mouse_down && is_over_capri && matches!(drag_state, DragState::Idle) {
            drag_state = DragState::PendingPickup(cursor_screen);
        }
        drag_state = drag_state.advance(mouse_down, cursor_screen, timestamp, dt, &physics_worker);
        was_mouse_down = mouse_down;

        if physics.stance() == Stance::Ride {
            let _ = physics_worker.update_ride(cursor_screen, cursor_motion, dt);
        }

        while let Ok(command) = cmd_rx.try_recv() {
            let mut context = CommandContext {
                worker: &physics_worker,
                animation: &mut animation,
                surfaces: &surfaces,
                monitors: &monitors,
                screen_top: vy as f32,
            };
            dispatch_command(command, &mut context);
        }

        while let Ok(output) = physics_worker.try_recv() {
            physics = output.snapshot;
            for event in output.events {
                animation.on_engine_event(&event);
                send_engine_telemetry(&event_tx, &event, physics.center_pos().x);
                if let EngineEvent::Impact {
                    normal,
                    speed,
                    point,
                    ..
                } = &event
                {
                    let hard = (*speed / IMPACT_HARDNESS_NORMALIZER_PX_SEC).clamp(0.0, 1.0);
                    vfx.emit_sparks(*point, IMPACT_SPARK_COUNT, *normal);
                    vfx.emit_cracks(*point, *normal, hard);
                    vfx.emit_shockwave(*point, hard);
                }
                if let EngineEvent::Landed {
                    surface_y, skid, ..
                } = &event
                    && *skid
                {
                    vfx.emit_sparks(
                        Vec2::new(physics.center_pos().x, *surface_y),
                        SKID_LANDING_SPARK_COUNT,
                        Vec2::new(-1.0, 0.0),
                    );
                }
            }
        }

        if physics.stance() == Stance::Dazed && !was_dazed {
            vfx.emit_dizzy(physics.center_pos() + Vec2::new(0.0, DIZZY_VFX_OFFSET_Y_PX));
        }
        was_dazed = physics.stance() == Stance::Dazed;
        if physics.stance() == Stance::Skid
            && last_skid_spark.elapsed() >= Duration::from_secs_f32(SKID_SPARK_INTERVAL_SECS)
            && let Some(skid) = physics.skid()
        {
            vfx.emit_sparks(skid.feet_pos, 2, Vec2::new(-skid.velocity_x.signum(), 0.0));
            last_skid_spark = Instant::now();
        }

        for (index, monitor) in monitors.iter().enumerate() {
            if monitor.contains_point(physics.center_pos()) && index != current_monitor_idx {
                current_monitor_idx = index;
                let _ = event_tx.send(OutgoingEvent::MonitorChanged {
                    screen_index: index,
                    screen_name: monitor.name.clone(),
                });
                break;
            }
        }

        if physics.stance() != reported_stance {
            reported_stance = physics.stance();
            let _ = event_tx.send(OutgoingEvent::StateChanged {
                stance: format!("{:?}", physics.stance()).to_lowercase(),
            });
        }

        vfx.update(dt);
        animation.update(
            dt,
            AnimationInput {
                stance: physics.stance(),
                facing: physics.facing(),
                motion: current_motion(&physics, cursor_motion),
                release_streak: physics
                    .flight()
                    .is_some_and(|flight| flight.release_streak > 0),
            },
        );
        let sample = animation.sample();
        let rendered_body = select_body(&mut sprite_manager, sample, last_body);
        prewarm_animation(&mut sprite_manager, &physics);
        prepare_loaded_strip(
            &mut renderer,
            &mut sprite_manager,
            rendered_body.clip(),
            rendered_body.flipped(),
        )?;

        let mouth_viseme = sample.mouth_viseme.filter(|_| {
            !matches!(
                rendered_body.clip(),
                ClipId::PerchTalk | ClipId::LaydownTalk
            )
        });
        let mouth_key = mouth_viseme.map(|_| sprite_manager.request(ClipId::Mouth, false));
        if mouth_viseme.is_some() {
            prepare_loaded_strip(&mut renderer, &mut sprite_manager, ClipId::Mouth, false)?;
        }

        let body_task = sprite_manager.request(rendered_body.clip(), rendered_body.flipped());
        sprite_manager.request(ClipId::Idle, false);
        let keep = match mouth_key {
            Some(mouth) => [body_task, mouth],
            None => [body_task, body_task],
        };
        let kept_names = sprite_manager.trim_to_budget(&keep);
        renderer.retain_sprites(&kept_names.into_iter().collect::<HashSet<_>>());

        let transform = render_transform(&physics, rendered_body.flipped());
        let transform = VisualTransform {
            scale: transform.scale * rendered_body.display_scale(),
            ..transform
        };
        let body_strip = sprite_manager.get_strip(body_task.name());
        let mouth_strip = mouth_key.and_then(|key| sprite_manager.get_strip(key.name()));
        if let Some(strip) = body_strip {
            let mut body = SpriteInstance::new(
                strip,
                rendered_body.frame_index(),
                transform,
                rendered_body.pivot(),
            );
            body.mirror_uv = physics.facing() == Facing::Left && !rendered_body.flipped();
            body.flash = sample.flash;
            body.glitch = sample.glitch;
            body.smear = sample.smear;
            body.echo = sample.echo;

            let mouth = mouth_viseme.and_then(|viseme| {
                mouth_strip.map(|strip| {
                    let spec = ClipCatalog::spec(ClipId::Mouth);
                    let mut mouth = SpriteInstance::new(
                        strip,
                        viseme as usize,
                        VisualTransform {
                            facing: Facing::Right,
                            ..transform
                        },
                        spec.pivot,
                    );
                    mouth.glitch = sample.glitch * MOUTH_GLITCH_SCALE;
                    mouth.mirror_uv = physics.facing() == Facing::Left && !rendered_body.flipped();
                    mouth
                })
            });

            renderer.try_render_animation(body, mouth, &vfx)?;
        }

        last_body = rendered_body;

        let frame_time = frame_start.elapsed();
        frame_metrics.record(frame_time.as_secs_f64());
        if frame_time < target_frame_duration {
            let remaining = target_frame_duration - frame_time;
            wait_for_messages(remaining.as_millis().max(1) as u32);
        }
    }

    drop(ledge_scanner);
    log_frame_metrics(&frame_metrics, sprite_manager.resident_mask_bytes());
    println!("[ENGINE] Finalizando Capri Engine limpiamente.");
    Ok(())
}

fn log_frame_metrics(metrics: &FrameMetrics, resident_mask_bytes: usize) {
    let Some(average) = metrics.average_secs() else {
        return;
    };
    let p95 = metrics.percentile_secs(95.0).unwrap_or(average);
    let maximum = metrics.max_secs().unwrap_or(average);
    let jitter = metrics.jitter_secs().unwrap_or(0.0);
    let working_set_bytes = process_working_set_bytes().unwrap_or(0);
    println!(
        "[METRICS] frames={} avg={:.3}ms p95={:.3}ms max={:.3}ms jitter={:.3}ms masks={}KiB rss={}KiB",
        metrics.len(),
        average * 1_000.0,
        p95 * 1_000.0,
        maximum * 1_000.0,
        jitter * 1_000.0,
        resident_mask_bytes.div_ceil(1024),
        working_set_bytes.div_ceil(1024)
    );
}
