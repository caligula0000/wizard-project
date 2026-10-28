mod world;

use sdl3::{
    event::Event, keyboard::Keycode, mouse::MouseButton, pixels::PixelFormat, render::ScaleMode,
};
use std::time::{Duration, Instant};
use world::World;

const DT: f32 = 1.0 / 120.0;
const WIDTH: usize = 320;
const HEIGHT: usize = 180;

fn sandbox() -> World {
    let mut world = World::demo(WIDTH, HEIGHT, 8);
    world.rasterize();
    world.add_sand([WIDTH as f32 * 0.5, 12.0], 10.0);
    world
}

fn benchmark() {
    println!("Headless CPU benchmark; 120 Hz motion + sand, no rigid-body response or GPU work.");
    for (width, height, bodies) in [(320, 180, 8), (1024, 512, 64), (2048, 1024, 256)] {
        let mut world = World::demo(width, height, bodies);
        world.rasterize();
        world.add_sand(
            [width as f32 * 0.5, height as f32 * 0.1],
            height as f32 * 0.08,
        );
        let mut pixels = vec![0; width * height * 4];
        let initial = world.count();
        let mut samples = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for frame in 0..1100 {
            let start = Instant::now();
            world.step(DT);
            let motion = Instant::now();
            world.rasterize();
            let coverage = Instant::now();
            world.update_sand();
            let sand = Instant::now();
            world.render(&mut pixels);
            std::hint::black_box(&pixels);
            let end = Instant::now();
            if frame >= 100 {
                for (samples, duration) in samples.iter_mut().zip([
                    motion - start,
                    coverage - motion,
                    sand - coverage,
                    end - sand,
                    end - start,
                ]) {
                    samples.push(duration.as_secs_f64() * 1000.0);
                }
            }
        }
        assert_eq!(world.count(), initial);
        println!(
            "\n{width}x{height}, {bodies} bodies; dense buffers {:.2} MiB + body masks",
            (width * height * 10) as f64 / (1024.0 * 1024.0)
        );
        println!(
            "terrain={:p}, coverage={:p}, visible={:p}, rgba={:p}",
            world.terrain.cells.as_ptr(),
            world.coverage.as_ptr(),
            world.visible.as_ptr(),
            pixels.as_ptr()
        );
        for (name, values) in ["motion", "coverage", "sand", "RGBA", "total"]
            .into_iter()
            .zip(&mut samples)
        {
            values.sort_by(f64::total_cmp);
            println!(
                "{name:>8}: p50 {:7.3} ms | p95 {:7.3} | p99 {:7.3} | max {:7.3}",
                values[500], values[950], values[990], values[999]
            );
        }
        let start = Instant::now();
        let removed = world.erase(world.bodies[0].position, 9.0);
        println!(
            "one edit+split: {:.3} ms, {removed} removed (scene-specific, not a worst case)",
            start.elapsed().as_secs_f64() * 1000.0
        );
        assert_eq!(world.count() + removed, initial);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--bench") {
        benchmark();
        return Ok(());
    }
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let window = video
        .window("Cell sandbox", 960, 540)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas();
    let creator = canvas.texture_creator();
    let mut texture =
        creator.create_texture_streaming(PixelFormat::RGBA32, WIDTH as u32, HEIGHT as u32)?;
    texture.set_scale_mode(ScaleMode::Nearest);
    let mut events = sdl.event_pump()?;
    let mut world = sandbox();
    let mut pixels = vec![0; WIDTH * HEIGHT * 4];
    let mut paused = false;
    let mut painting = false;
    let mut pouring = false;
    let mut previous = Instant::now();
    let mut accumulator = 0.0;
    let mut report = Instant::now();
    let mut frames = 0;
    let mut cpu_ms = 0.0;
    let mut max_ms: f64 = 0.0;
    let mut dropped = 0;
    println!(
        "Space pause | N single tick (paused) | left drag erase/split | right drag sand | R reset | Esc exit"
    );
    'running: loop {
        let start = Instant::now();
        let elapsed = start.duration_since(previous).as_secs_f64();
        previous = start;
        let mut single_step = false;
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    repeat: false,
                    ..
                } => {
                    paused = !paused;
                    accumulator = 0.0;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::N),
                    repeat: false,
                    ..
                } => single_step = true,
                Event::KeyDown {
                    keycode: Some(Keycode::R),
                    repeat: false,
                    ..
                } => {
                    world = sandbox();
                    accumulator = 0.0;
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => painting = true,
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => painting = false,
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Right,
                    ..
                } => pouring = true,
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Right,
                    ..
                } => pouring = false,
                _ => {}
            }
        }
        if painting {
            let mouse = events.mouse_state();
            let (w, h) = canvas.window().size();
            world.erase(
                [
                    mouse.x() * WIDTH as f32 / w as f32,
                    mouse.y() * HEIGHT as f32 / h as f32,
                ],
                5.0,
            );
        }
        if pouring {
            // Rebuild after any erase/split so body indices and coverage are current.
            world.rasterize();
            let mouse = events.mouse_state();
            let (w, h) = canvas.window().size();
            world.add_sand(
                [
                    mouse.x() * WIDTH as f32 / w as f32,
                    mouse.y() * HEIGHT as f32 / h as f32,
                ],
                4.0,
            );
        }
        if !paused {
            accumulator += elapsed;
            let mut steps = 0;
            while accumulator >= DT as f64 && steps < 8 {
                world.step(DT);
                world.rasterize();
                world.update_sand();
                accumulator -= DT as f64;
                steps += 1;
            }
            if accumulator >= DT as f64 {
                dropped += 1;
                accumulator %= DT as f64;
            }
        } else if single_step {
            world.step(DT);
            world.rasterize();
            world.update_sand();
        }
        world.rasterize();
        world.render(&mut pixels);
        let work_ms = start.elapsed().as_secs_f64() * 1000.0;
        cpu_ms += work_ms;
        max_ms = max_ms.max(work_ms);
        frames += 1;
        texture.update(None, &pixels, WIDTH * 4)?;
        canvas.clear();
        canvas.copy(&texture, None, None)?;
        canvas.present();
        if report.elapsed() >= Duration::from_secs(1) {
            canvas.window_mut().set_title(&format!("Cell sandbox | {} | {} bodies | CPU avg {:.2} / max {:.2} ms | overlaps {} | catch-up drops {}",
                if paused { "PAUSED" } else { "120 Hz" }, world.bodies.len(), cpu_ms / frames as f64, max_ms, world.overlaps, dropped))?;
            cpu_ms = 0.0;
            max_ms = 0.0;
            frames = 0;
            report = Instant::now();
        }
        // Presentation cap only: simulation uses an independent fixed timestep.
        if let Some(wait) = Duration::from_secs_f64(1.0 / 120.0).checked_sub(start.elapsed()) {
            std::thread::sleep(wait);
        }
    }
    Ok(())
}
