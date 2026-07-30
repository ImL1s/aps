//! GP0 Command Parser and FIFO State Machine

use super::rasterizer::{Rasterizer, RectClip, Vertex};
use super::vram::VRam;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gp0State {
    Command,
    CpuToVram {
        dst_x: u32,
        dst_y: u32,
        width: u32,
        height: u32,
        current_x: u32,
        current_y: u32,
        words_remaining: u32,
    },
}

#[derive(Debug, Clone)]
pub struct Gp0 {
    pub state: Gp0State,
    pub cmd_buf: Vec<u32>,
    pub draw_offset_x: i32,
    pub draw_offset_y: i32,
    pub clip: RectClip,
    pub draw_mode: u32,
}

impl Default for Gp0 {
    fn default() -> Self {
        Self {
            state: Gp0State::Command,
            cmd_buf: Vec::with_capacity(16),
            draw_offset_x: 0,
            draw_offset_y: 0,
            clip: RectClip {
                x1: 0,
                y1: 0,
                x2: 1023,
                y2: 511,
            },
            draw_mode: 0,
        }
    }
}

impl Gp0 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn write_word(&mut self, word: u32, vram: &mut VRam) {
        match self.state {
            Gp0State::CpuToVram {
                dst_x,
                dst_y: _,
                width,
                height: _,
                ref mut current_x,
                ref mut current_y,
                ref mut words_remaining,
            } => {
                // Low 16 bits = pixel 1, High 16 bits = pixel 2
                let pix1 = (word & 0xFFFF) as u16;
                let pix2 = (word >> 16) as u16;

                vram.set_pixel(*current_x, *current_y, pix1);
                *current_x += 1;
                if *current_x >= dst_x + width {
                    *current_x = dst_x;
                    *current_y += 1;
                }

                if *words_remaining > 1 || width % 2 == 0 || *current_x > dst_x {
                    vram.set_pixel(*current_x, *current_y, pix2);
                    *current_x += 1;
                    if *current_x >= dst_x + width {
                        *current_x = dst_x;
                        *current_y += 1;
                    }
                }

                *words_remaining = words_remaining.saturating_sub(1);
                if *words_remaining == 0 {
                    self.state = Gp0State::Command;
                }
            }
            Gp0State::Command => {
                self.cmd_buf.push(word);
                let expected = get_expected_cmd_words(self.cmd_buf[0]);

                if self.cmd_buf.len() >= expected {
                    self.execute_command(vram);
                    self.cmd_buf.clear();
                }
            }
        }
    }

    fn execute_command(&mut self, vram: &mut VRam) {
        let cmd = self.cmd_buf[0];
        let opcode = ((cmd >> 24) & 0xFF) as u8;

        match opcode {
            0x00 => {} // NOP
            0x01 => {} // Clear cache
            0x02 => {
                // Fill Rectangle in VRAM
                if self.cmd_buf.len() >= 3 {
                    let r = (cmd & 0xFF) as u8;
                    let g = ((cmd >> 8) & 0xFF) as u8;
                    let b = ((cmd >> 16) & 0xFF) as u8;
                    let x = (self.cmd_buf[1] & 0xFFFF) as i32;
                    let y = ((self.cmd_buf[1] >> 16) & 0xFFFF) as i32;
                    let w = (self.cmd_buf[2] & 0xFFFF) as i32;
                    let h = ((self.cmd_buf[2] >> 16) & 0xFFFF) as i32;

                    Rasterizer::draw_rect(
                        vram,
                        x,
                        y,
                        w,
                        h,
                        r,
                        g,
                        b,
                        0,
                        0,
                        RectClip {
                            x1: 0,
                            y1: 0,
                            x2: 1023,
                            y2: 511,
                        },
                    );
                }
            }
            0x20..=0x3F => {
                // Polygon Rendering
                self.execute_polygon(vram, opcode);
            }
            0x60..=0x7F => {
                // Rectangle Rendering
                self.execute_rectangle(vram, opcode);
            }
            0xA0 => {
                // CPU to VRAM transfer
                if self.cmd_buf.len() >= 3 {
                    let dst_x = self.cmd_buf[1] & 0x3FF;
                    let dst_y = (self.cmd_buf[1] >> 16) & 0x1FF;
                    let width = self.cmd_buf[2] & 0x3FF;
                    let height = (self.cmd_buf[2] >> 16) & 0x1FF;

                    let total_pixels = width * height;
                    let words_remaining = total_pixels.div_ceil(2);

                    if words_remaining > 0 {
                        self.state = Gp0State::CpuToVram {
                            dst_x,
                            dst_y,
                            width,
                            height,
                            current_x: dst_x,
                            current_y: dst_y,
                            words_remaining,
                        };
                    }
                }
            }
            0xE1 => {
                self.draw_mode = cmd & 0x00FF_FFFF;
            }
            0xE3 => {
                self.clip.x1 = (cmd & 0x3FF) as i32;
                self.clip.y1 = ((cmd >> 10) & 0x1FF) as i32;
            }
            0xE4 => {
                self.clip.x2 = (cmd & 0x3FF) as i32;
                self.clip.y2 = ((cmd >> 10) & 0x1FF) as i32;
            }
            0xE5 => {
                // Sign-extend 11-bit offset
                let mut sx = (cmd & 0x7FF) as i32;
                if (sx & 0x400) != 0 {
                    sx |= !0x7FF;
                }
                let mut sy = ((cmd >> 11) & 0x7FF) as i32;
                if (sy & 0x400) != 0 {
                    sy |= !0x7FF;
                }
                self.draw_offset_x = sx;
                self.draw_offset_y = sy;
            }
            _ => {}
        }
    }

    fn execute_polygon(&mut self, vram: &mut VRam, opcode: u8) {
        let is_gouraud = (opcode & 0x10) != 0;
        let is_quad = (opcode & 0x08) != 0;

        let extract_color = |word: u32| -> (u8, u8, u8) {
            (
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
            )
        };

        let extract_vertex = |word: u32, color: (u8, u8, u8)| -> Vertex {
            let sx = (word & 0xFFFF) as i16 as i32;
            let sy = (word >> 16) as i16 as i32;
            Vertex {
                x: sx,
                y: sy,
                r: color.0,
                g: color.1,
                b: color.2,
            }
        };

        if !is_gouraud {
            let color = extract_color(self.cmd_buf[0]);
            let v0 = extract_vertex(self.cmd_buf[1], color);
            let v1 = extract_vertex(self.cmd_buf[2], color);
            let v2 = extract_vertex(self.cmd_buf[3], color);

            Rasterizer::draw_triangle(
                vram,
                v0,
                v1,
                v2,
                self.draw_offset_x,
                self.draw_offset_y,
                self.clip,
                false,
            );

            if is_quad && self.cmd_buf.len() >= 5 {
                let v3 = extract_vertex(self.cmd_buf[4], color);
                Rasterizer::draw_triangle(
                    vram,
                    v1,
                    v2,
                    v3,
                    self.draw_offset_x,
                    self.draw_offset_y,
                    self.clip,
                    false,
                );
            }
        } else {
            let c0 = extract_color(self.cmd_buf[0]);
            let v0 = extract_vertex(self.cmd_buf[1], c0);

            let c1 = extract_color(self.cmd_buf[2]);
            let v1 = extract_vertex(self.cmd_buf[3], c1);

            let c2 = extract_color(self.cmd_buf[4]);
            let v2 = extract_vertex(self.cmd_buf[5], c2);

            Rasterizer::draw_triangle(
                vram,
                v0,
                v1,
                v2,
                self.draw_offset_x,
                self.draw_offset_y,
                self.clip,
                true,
            );

            if is_quad && self.cmd_buf.len() >= 8 {
                let c3 = extract_color(self.cmd_buf[6]);
                let v3 = extract_vertex(self.cmd_buf[7], c3);
                Rasterizer::draw_triangle(
                    vram,
                    v1,
                    v2,
                    v3,
                    self.draw_offset_x,
                    self.draw_offset_y,
                    self.clip,
                    true,
                );
            }
        }
    }

    fn execute_rectangle(&mut self, vram: &mut VRam, opcode: u8) {
        if self.cmd_buf.len() < 2 {
            return;
        }

        let cmd = self.cmd_buf[0];
        let color_r = (cmd & 0xFF) as u8;
        let color_g = ((cmd >> 8) & 0xFF) as u8;
        let color_b = ((cmd >> 16) & 0xFF) as u8;

        let x = (self.cmd_buf[1] & 0xFFFF) as i16 as i32;
        let y = (self.cmd_buf[1] >> 16) as i16 as i32;

        let size_code = (opcode >> 3) & 3;
        let (w, h) = match size_code {
            0 => {
                if self.cmd_buf.len() >= 3 {
                    (
                        (self.cmd_buf[2] & 0xFFFF) as i32,
                        (self.cmd_buf[2] >> 16) as i32,
                    )
                } else {
                    (1, 1)
                }
            }
            1 => (1, 1),
            2 => (8, 8),
            3 => (16, 16),
            _ => (1, 1),
        };

        Rasterizer::draw_rect(
            vram,
            x,
            y,
            w,
            h,
            color_r,
            color_g,
            color_b,
            self.draw_offset_x,
            self.draw_offset_y,
            self.clip,
        );
    }
}

fn get_expected_cmd_words(first_word: u32) -> usize {
    let opcode = (first_word >> 24) & 0xFF;
    match opcode {
        0x00 | 0x01 | 0xE1..=0xE6 => 1,
        0x02 => 3,
        0x20..=0x3F => {
            let is_gouraud = (opcode & 0x10) != 0;
            let is_quad = (opcode & 0x08) != 0;
            match (is_gouraud, is_quad) {
                (false, false) => 4,
                (false, true) => 5,
                (true, false) => 6,
                (true, true) => 8,
            }
        }
        0x60..=0x7F => {
            let size_code = (opcode >> 3) & 3;
            if size_code == 0 {
                3
            } else {
                2
            }
        }
        0xA0 | 0xC0 => 3,
        _ => 1,
    }
}
