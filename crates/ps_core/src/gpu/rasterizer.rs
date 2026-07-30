//! Software Triangle and Rectangle Rasterizer with Gouraud shading and Scissor clipping

use super::vram::{rgb888_to_bgr555, VRam};

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct RectClip {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

pub struct Rasterizer;

impl Rasterizer {
    #[allow(clippy::too_many_arguments)]
    pub fn draw_triangle(
        vram: &mut VRam,
        mut v0: Vertex,
        mut v1: Vertex,
        mut v2: Vertex,
        offset_x: i32,
        offset_y: i32,
        clip: RectClip,
        gouraud: bool,
    ) {
        // Apply drawing offset
        v0.x += offset_x;
        v0.y += offset_y;
        v1.x += offset_x;
        v1.y += offset_y;
        v2.x += offset_x;
        v2.y += offset_y;

        // Determine bounding box clipped to scissor box
        let min_x = (v0.x.min(v1.x).min(v2.x)).max(clip.x1).max(0);
        let max_x = (v0.x.max(v1.x).max(v2.x)).min(clip.x2).min(1023);
        let min_y = (v0.y.min(v1.y).min(v2.y)).max(clip.y1).max(0);
        let max_y = (v0.y.max(v1.y).max(v2.y)).min(clip.y2).min(511);

        if min_x > max_x || min_y > max_y {
            return;
        }

        // Edge function calculation helper
        let edge = |ax: i32, ay: i32, bx: i32, by: i32, px: i32, py: i32| -> i64 {
            (px as i64 - ax as i64) * (by as i64 - ay as i64)
                - (py as i64 - ay as i64) * (bx as i64 - ax as i64)
        };

        let area = edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
        if area == 0 {
            return;
        }

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let e0 = edge(v1.x, v1.y, v2.x, v2.y, x, y);
                let e1 = edge(v2.x, v2.y, v0.x, v0.y, x, y);
                let e2 = edge(v0.x, v0.y, v1.x, v1.y, x, y);

                // Pixel inside triangle if all edge functions have the same sign as area
                let is_inside = if area > 0 {
                    e0 >= 0 && e1 >= 0 && e2 >= 0
                } else {
                    e0 <= 0 && e1 <= 0 && e2 <= 0
                };

                if is_inside {
                    let color = if gouraud {
                        let w0 = e0 as f64 / area as f64;
                        let w1 = e1 as f64 / area as f64;
                        let w2 = e2 as f64 / area as f64;

                        let r = (w0 * v0.r as f64 + w1 * v1.r as f64 + w2 * v2.r as f64)
                            .clamp(0.0, 255.0) as u8;
                        let g = (w0 * v0.g as f64 + w1 * v1.g as f64 + w2 * v2.g as f64)
                            .clamp(0.0, 255.0) as u8;
                        let b = (w0 * v0.b as f64 + w1 * v1.b as f64 + w2 * v2.b as f64)
                            .clamp(0.0, 255.0) as u8;

                        rgb888_to_bgr555(r, g, b)
                    } else {
                        rgb888_to_bgr555(v0.r, v0.g, v0.b)
                    };

                    vram.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect(
        vram: &mut VRam,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        color_r: u8,
        color_g: u8,
        color_b: u8,
        offset_x: i32,
        offset_y: i32,
        clip: RectClip,
    ) {
        let start_x = (x + offset_x).max(clip.x1).max(0);
        let end_x = (x + offset_x + w).min(clip.x2 + 1).min(1024);
        let start_y = (y + offset_y).max(clip.y1).max(0);
        let end_y = (y + offset_y + h).min(clip.y2 + 1).min(512);

        if start_x >= end_x || start_y >= end_y {
            return;
        }

        let c555 = rgb888_to_bgr555(color_r, color_g, color_b);

        for py in start_y..end_y {
            for px in start_x..end_x {
                vram.set_pixel(px as u32, py as u32, c555);
            }
        }
    }
}
