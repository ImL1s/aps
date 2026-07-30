use ps_core::system::PS1System;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::Canvas;
use sdl2::video::Window;
use std::time::{Duration, Instant};

pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 512;
pub const DISPLAY_WIDTH: u32 = 640;
pub const DISPLAY_HEIGHT: u32 = 480;
pub const CPU_CYCLES_PER_FRAME: u32 = 564_480; // 33.8688 MHz / 60 FPS

pub struct Sdl2Frontend {
    sdl_context: sdl2::Sdl,
    canvas: Canvas<Window>,
    frame_buffer: Vec<u32>,
}

impl Sdl2Frontend {
    pub fn new(display_mode: &str) -> anyhow::Result<Self> {
        let sdl_context = sdl2::init().map_err(|e| anyhow::anyhow!(e))?;
        let video_subsystem = sdl_context.video().map_err(|e| anyhow::anyhow!(e))?;

        let (win_w, win_h) = if display_mode == "vram_debug" {
            (1024, 512)
        } else {
            (DISPLAY_WIDTH, DISPLAY_HEIGHT)
        };

        let window = video_subsystem
            .window("aps — Clean-Room PlayStation 1 Emulator", win_w, win_h)
            .position_centered()
            .resizable()
            .build()?;

        let canvas = window
            .into_canvas()
            .present_vsync()
            .build()
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(Self {
            sdl_context,
            canvas,
            frame_buffer: vec![0xFF000000; VRAM_WIDTH * VRAM_HEIGHT],
        })
    }

    pub fn run_loop(&mut self, mut system: PS1System) -> anyhow::Result<()> {
        let mut event_pump = self
            .sdl_context
            .event_pump()
            .map_err(|e| anyhow::anyhow!(e))?;
        let target_frame_duration = Duration::from_nanos(16_666_667); // ~60 FPS

        let texture_creator = self.canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                PixelFormatEnum::ARGB8888,
                VRAM_WIDTH as u32,
                VRAM_HEIGHT as u32,
            )
            .map_err(|e| anyhow::anyhow!(e))?;

        'running: loop {
            let frame_start = Instant::now();

            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'running,
                    Event::KeyDown {
                        keycode: Some(k), ..
                    } => {
                        if let Some(btn) = map_key_to_button(k) {
                            system.bus.controller.set_button(btn, true);
                        }
                    }
                    Event::KeyUp {
                        keycode: Some(k), ..
                    } => {
                        if let Some(btn) = map_key_to_button(k) {
                            system.bus.controller.set_button(btn, false);
                        }
                    }
                    _ => {}
                }
            }

            // Step PS1 system hardware core by 1 frame (~564,480 cycles)
            system.step_batch(CPU_CYCLES_PER_FRAME);

            // Render VRAM to ARGB32 buffer & update SDL2 canvas
            system.bus.gpu.render_vram_to_argb32(&mut self.frame_buffer);
            texture
                .update(
                    None,
                    bytemuck::cast_slice(&self.frame_buffer),
                    VRAM_WIDTH * 4,
                )
                .map_err(|e| anyhow::anyhow!(e))?;
            self.canvas.clear();
            self.canvas
                .copy(&texture, None, None)
                .map_err(|e| anyhow::anyhow!(e))?;
            self.canvas.present();

            let elapsed = frame_start.elapsed();
            if elapsed < target_frame_duration {
                std::thread::sleep(target_frame_duration - elapsed);
            }
        }

        Ok(())
    }
}

pub use ps_core::controller::map_key_to_button;

#[cfg(test)]
mod tests {
    use super::*;
    use ps_core::controller::PadButton;

    #[test]
    fn test_map_key_to_button() {
        assert_eq!(map_key_to_button(Keycode::Z), Some(PadButton::Cross));
        assert_eq!(map_key_to_button(Keycode::Space), Some(PadButton::Start));
        assert_eq!(map_key_to_button(Keycode::Up), Some(PadButton::Up));
        assert_eq!(map_key_to_button(Keycode::F1), None);
    }
}
