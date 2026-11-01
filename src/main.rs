use glam::Vec2;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc};

use capri_engine::core::config::DEFAULT_WS_PORT;
use capri_engine::core::events::EngineEvent;
use capri_engine::core::state::Stance;
use capri_engine::desktop::ledges::scan_ledges;
use capri_engine::desktop::monitors::{enumerate_monitors, virtual_desktop_bounds};
use capri_engine::gateway::WsGatewayServer;
use capri_engine::gateway::protocol::{IncomingCommand, OutgoingEvent};
use capri_engine::physics::PhysicsEngine;
use capri_engine::render::hittest::create_default_mask;
use capri_engine::render::pipeline::{Renderer, SpriteDraw, strip_name_for_stance};
use capri_engine::render::sprites::SpriteManager;
use capri_engine::render::vfx::VfxSystem;
use capri_engine::render::window::{
    CompositionOverlay, IS_RUNNING, pump_messages, should_capture_input, wait_for_messages,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=========================================================");
    println!("          CAPRI ENGINE v0.2.2 - Cyberpunk Physics Host   ");
    println!("=========================================================");

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<IncomingCommand>();
    let (event_tx, _event_rx) = broadcast::channel::<OutgoingEvent>(128);

    let server_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let server = WsGatewayServer::new(DEFAULT_WS_PORT, cmd_tx, server_event_tx);
        if let Err(e) = server.run().await {
            eprintln!("[GATEWAY ERROR] Error en servidor WebSocket: {}", e);
        }
    });

    let mut sprite_manager = SpriteManager::new();
    let assets_dir = std::path::Path::new("assets/sprites");
    println!(
        "[ASSETS] Cargando tiras de sprites desde {:?}...",
        assets_dir
    );
    sprite_manager.load_essential_sprites(assets_dir);

    let monitors = enumerate_monitors();
    println!("[DESKTOP] Monitores detectados: {}", monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        println!(
            "  -> Monitor {}: {} | Límites: {:?} | Primario: {}",
            i, m.name, m.bounds, m.is_primary
        );
    }
    let (vx, vy, vr, vb) = virtual_desktop_bounds();
    let initial_pos = Vec2::new((vx + (vr - vx) / 2) as f32, (vb - 400) as f32);

    let mut overlay = CompositionOverlay::create().map_err(std::io::Error::other)?;
    let hwnd = overlay.hwnd();
    println!(
        "[WINDOW] Ventana overlay transparente creada (HWND: {:?})",
        hwnd
    );
    let mut renderer = Renderer::new(&overlay)
        .await
        .map_err(std::io::Error::other)?;

    let mut physics = PhysicsEngine::new(initial_pos);
    let mut vfx = VfxSystem::new();

    let default_mask = create_default_mask();
    let mut current_monitor_idx = 0;
    let mut last_ledge_scan = Instant::now() - Duration::from_secs(5);
    let mut cached_ledges_y = Vec::new();

    let mut last_frame = Instant::now();
    let start_time = Instant::now();
    let target_frame_duration = Duration::from_micros(16666);

    println!("[ENGINE] Bucle principal de físicas iniciado a 60 FPS. Listo.");

    let mut was_mouse_down = false;

    while IS_RUNNING.load(Ordering::SeqCst) {
        let frame_start = Instant::now();
        let dt = (frame_start - last_frame).as_secs_f32().clamp(0.001, 0.05);
        last_frame = frame_start;
        let elapsed_secs = (frame_start - start_time).as_secs_f64();

        if !pump_messages() {
            break;
        }

        let (cursor_x, cursor_y) = overlay.poll_cursor().map_err(std::io::Error::other)?;
        let cursor_screen = Vec2::new(cursor_x as f32, cursor_y as f32);
        let mouse_down = overlay.input().mouse_down();

        let active_strip_name = strip_name_for_stance(physics.stance);
        let active_strip = sprite_manager
            .get_strip(active_strip_name)
            .or_else(|| sprite_manager.get_strip("idle"))
            .ok_or_else(|| std::io::Error::other("idle sprite failed to load"))?;
        let active_frame = active_strip.frame_index_at(elapsed_secs);
        let active_mask = active_strip
            .masks
            .get(active_frame)
            .or_else(|| active_strip.masks.first())
            .unwrap_or(&default_mask);

        let is_over_capri = active_mask.hit_test_screen(
            cursor_screen,
            physics.center_pos,
            physics.rotation,
            physics.scale,
            physics.facing.as_f32(),
        );
        let capture_input = should_capture_input(is_over_capri, mouse_down, physics.stance);
        overlay.input().set_capture_input(capture_input);
        overlay
            .sync_input_capture(capture_input)
            .map_err(std::io::Error::other)?;

        if mouse_down && !was_mouse_down && is_over_capri {
            let _event = physics.pick_up(cursor_screen);
            let _ = event_tx.send(OutgoingEvent::AvatarGrabbed {
                x: cursor_screen.x,
                y: cursor_screen.y,
            });
            println!("[PHYSICS] Capri agarrada en {:?}", cursor_screen);
        } else if mouse_down && was_mouse_down && physics.stance == Stance::Held {
            physics.update_held(cursor_screen, elapsed_secs, dt);
        } else if !mouse_down && was_mouse_down && physics.stance == Stance::Held {
            let event = physics.release();
            if let EngineEvent::Thrown { velocity, speed } = event {
                let _ = event_tx.send(OutgoingEvent::AvatarThrown {
                    speed,
                    vx: velocity.x,
                    vy: velocity.y,
                });
                println!("[PHYSICS] ¡Capri lanzada a {:.1} px/s!", speed);
            }
        }
        was_mouse_down = mouse_down;

        if last_ledge_scan.elapsed() >= Duration::from_millis(1000) {
            let ledges = scan_ledges(&[hwnd.0 as isize]);
            cached_ledges_y = ledges.iter().map(|l| l.y as f32).collect();
            last_ledge_scan = Instant::now();
        }

        let physics_events = physics.step(
            dt,
            vx as f32,
            vy as f32,
            vr as f32,
            vb as f32,
            &cached_ledges_y,
        );

        for ev in physics_events {
            match ev {
                EngineEvent::Impact {
                    normal,
                    speed,
                    point,
                    window_title,
                } => {
                    let hard = (speed / 2000.0).clamp(0.0, 1.0);
                    vfx.emit_sparks(point, 12, normal);
                    vfx.emit_cracks(point, normal, hard);
                    vfx.emit_shockwave(point, hard);
                    let _ = event_tx.send(OutgoingEvent::AvatarImpact {
                        speed,
                        normal_x: normal.x,
                        normal_y: normal.y,
                        window_title: window_title.clone(),
                    });
                    println!(
                        "[PHYSICS] Choque a {:.0} px/s contra superficie en {:?}",
                        speed, point
                    );
                }
                EngineEvent::Landed {
                    surface_y,
                    window_title,
                    skid,
                } => {
                    let _ = event_tx.send(OutgoingEvent::AvatarLanded {
                        surface: if window_title.is_some() {
                            "window".to_string()
                        } else {
                            "floor".to_string()
                        },
                        window_title: window_title.clone(),
                        x: physics.center_pos.x,
                        y: surface_y,
                        skid,
                    });
                    if skid {
                        vfx.emit_sparks(
                            Vec2::new(physics.center_pos.x, surface_y),
                            8,
                            Vec2::new(-1.0, 0.0),
                        );
                    }
                    println!("[PHYSICS] Aterrizó en y={:.0} (skid: {})", surface_y, skid);
                }
                _ => {}
            }
        }

        for (idx, mon) in monitors.iter().enumerate() {
            if mon.contains_point(physics.center_pos) && idx != current_monitor_idx {
                current_monitor_idx = idx;
                let _ = event_tx.send(OutgoingEvent::MonitorChanged {
                    screen_index: idx,
                    screen_name: mon.name.clone(),
                });
                println!("[DESKTOP] Capri cruzó al monitor {} ({})", idx, mon.name);
                break;
            }
        }

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                IncomingCommand::Action { action, target } => {
                    println!(
                        "[GATEWAY COMMAND] Acción solicitada: {} (target: {:?})",
                        action, target
                    );
                    match action.as_str() {
                        "hang_on_ceiling" => {
                            physics.stance = Stance::Upside;
                            physics.rotation = 180.0;
                            physics.center_pos.y = vy as f32 + 60.0;
                        }
                        "hang_on_cursor" => {
                            physics.stance = Stance::Ride;
                        }
                        "reset_stand" => {
                            physics.stance = Stance::Stand;
                            physics.rotation = 0.0;
                        }
                        _ => {}
                    }
                }
                IncomingCommand::Speak { text, duration_ms } => {
                    println!(
                        "[GATEWAY COMMAND] Hablar: '{}' (duración: {:?}ms)",
                        text, duration_ms
                    );
                }
                IncomingCommand::LipSync { viseme: _ } => {}
                IncomingCommand::Emotion { state } => {
                    println!("[GATEWAY COMMAND] Emoción cambiada a: {}", state);
                }
                IncomingCommand::PointAt { screen, x, y } => {
                    println!(
                        "[GATEWAY COMMAND] Señalar pantalla {} en ({}, {})",
                        screen, x, y
                    );
                }
            }
        }

        vfx.update(dt);
        renderer
            .render(
                SpriteDraw {
                    strip: active_strip,
                    frame_index: active_frame,
                    center: physics.center_pos,
                    rotation: physics.rotation,
                    scale: physics.scale,
                    facing: physics.facing.as_f32(),
                },
                &vfx,
            )
            .map_err(std::io::Error::other)?;

        let frame_time = frame_start.elapsed();
        if frame_time < target_frame_duration {
            let remaining = target_frame_duration - frame_time;
            wait_for_messages(remaining.as_millis().max(1) as u32);
        }
    }

    println!("[ENGINE] Finalizando Capri Engine limpiamente.");
    Ok(())
}
