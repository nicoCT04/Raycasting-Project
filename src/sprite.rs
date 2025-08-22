use raylib::prelude::*;
use std::f32::consts::PI;

use crate::framebuffer::Framebuffer;
use crate::player::Player;
use crate::CpuImage;

pub struct Sprite {
    pub pos: Vector2,        // posición en mundo (misma escala que player.pos)
    pub frames: Vec<CpuImage>,
    pub fps: f32,            // cuadros por segundo
    pub t: f32,              // tiempo acumulado
    pub scale: f32,          // factor extra (1.0 = tamaño base)
}

impl Sprite {
    pub fn new(pos: Vector2, frames: Vec<CpuImage>, fps: f32, scale: f32) -> Self {
        Self { pos, frames, fps, t: 0.0, scale }
    }

    pub fn update(&mut self, dt: f32) {
        self.t += dt;
    }

    #[inline]
    fn current_frame(&self) -> &CpuImage {
        let n = self.frames.len().max(1);
        let idx = ((self.t * self.fps) as usize) % n;
        &self.frames[idx]
    }
}

/// Carga múltiples imágenes como frames de animación
pub fn load_frames(paths: &[&str]) -> Vec<CpuImage> {
    paths.iter().map(|p| CpuImage::from_path(p)).collect()
}

/// Dibuja sprites con prueba de profundidad por columna.
/// `depth[i]` debe contener la distancia del muro más cercano para esa columna (producida por el raycaster).
pub fn render_sprites(
    framebuffer: &mut Framebuffer,
    player: &Player,
    sprites: &mut [Sprite],
    _block_size: usize,
    depth: &[f32],
) {
    let w = framebuffer.width as i32;
    let h = framebuffer.height as i32;
    let hh = h as f32 * 0.5;
    let dpp = 70.0;

    // Ordena de lejos a cerca 
    sprites.sort_by(|a, b| {
        let da = (a.pos.x - player.pos.x).hypot(a.pos.y - player.pos.y);
        let db = (b.pos.x - player.pos.x).hypot(b.pos.y - player.pos.y);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });

    for spr in sprites.iter_mut() {
        let frame = spr.current_frame();

        // Vector al sprite
        let dx = spr.pos.x - player.pos.x;
        let dy = spr.pos.y - player.pos.y;
        let dist = (dx*dx + dy*dy).sqrt().max(0.0005);

        // Ángulo relativo a la mirada del jugador
        let mut angle = dy.atan2(dx) - player.a;
        while angle >  PI { angle -= 2.0*PI; }
        while angle < -PI { angle += 2.0*PI; }

        // Si está muy fuera del FOV
        if angle.abs() > player.fov { continue; }

        // Proyección horizontal (x en pantalla)
        let screen_x = ((angle / player.fov) + 0.5) * w as f32;

        // Altura (y anchura) proyectada del sprite (billboard cuadrado)
        let base_h = (hh / dist) * dpp * spr.scale;
        let sprite_h = base_h as i32;
        let sprite_w = sprite_h; // cuadrado

        // Rectángulo en pantalla
        let left   = (screen_x as i32) - sprite_w / 2;
        let right  = left + sprite_w;
        let top    = (hh - base_h * 0.5) as i32;
        let bottom = (hh + base_h * 0.5) as i32;

        // Clipping
        let cl_left   = left.max(0);
        let cl_right  = right.min(w);
        let cl_top    = top.max(0);
        let cl_bottom = bottom.min(h);
        if cl_left >= cl_right || cl_top >= cl_bottom { continue; }

        // Dibujo columnar con prueba de profundidad
        for sx in cl_left..cl_right {
            let col_idx = sx as usize;
            let u = (sx - left) as f32 / (right - left).max(1) as f32;

            for sy in cl_top..cl_bottom {
                // Mapeo v en [0,1]
                let v = (sy - top) as f32 / (bottom - top).max(1) as f32;

                // Muestra el frame y respeta alpha (transparencia)
                let col = frame.sample_repeat(u, v);
                if col.a < 10 { continue; } // transparencia

                // Profundidad del muro para esta columna
                let wall_dist = depth.get(col_idx).copied().unwrap_or(f32::INFINITY);

                // Si el sprite está detrás del muro, no se dibuja
                if dist >= wall_dist { continue; }

                framebuffer.set_current_color(col);
                framebuffer.set_pixel(sx as u32, sy as u32);
            }
        }
    }
}
