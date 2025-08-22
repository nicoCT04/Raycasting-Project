// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]
#[derive(Copy, Debug, PartialEq, Eq)]
#[derive(Clone)]
enum GameState { Title, LevelSelect, Playing, Win }
struct Assets {
    initial: Texture2D,
    win: Texture2D,
}


mod line;
mod framebuffer;
mod maze;
mod caster;
mod player;
mod sprite;

use line::line;
use maze::{Maze,load_maze};
use caster::{cast_ray, Intersect};
use framebuffer::Framebuffer;
use player::{Player, process_events};
use raylib::audio::{RaylibAudio, Music, Sound};
use sprite::{Sprite, load_frames, render_sprites};

use raylib::prelude::*;
use std::thread;
use std::time::Duration;
use std::f32::consts::PI;
use std::collections::HashMap;

use crate::maze::world_to_cell;
#[derive(Clone)]
pub struct CpuImage {
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<Color>,
}

impl CpuImage {
    pub fn from_path(path: &str) -> Self {
        // Carga
        let img = Image::load_image(path).expect("No pude cargar la imagen de pared");
        let w = img.width as usize;
        let h = img.height as usize;

        // Obtén los colores (ImageColors -> Vec<Color>)
        let colors = img.get_image_data();              
        let pixels: Vec<Color> = colors.to_vec();

        Self { w, h, pixels }
    }

    #[inline]
    pub fn sample_repeat(&self, u: f32, v: f32) -> Color {
        let uu = u.rem_euclid(1.0);
        let vv = v.clamp(0.0, 1.0);
        let x = (uu * (self.w as f32 - 1.0)) as usize;
        let y = (vv * (self.h as f32 - 1.0)) as usize;
        self.pixels[y * self.w + x]
    }
}

struct WallTex {
    default: CpuImage,
    map: HashMap<char, CpuImage>, // por ejemplo: '+', '-', '|', '1', '2', 'g'
}

impl WallTex {
    fn new(default: CpuImage) -> Self {
        Self { default, map: HashMap::new() }
    }

    fn insert(&mut self, ch: char, tex: CpuImage) {
        self.map.insert(ch, tex);
    }

    #[inline]
    fn for_cell(&self, ch: char) -> &CpuImage {
        self.map.get(&ch).unwrap_or(&self.default)
    }
}



fn scale_color(c: Color, f: f32) -> Color {
    let fr = (c.r as f32 * f).clamp(0.0, 255.0) as u8;
    let fg = (c.g as f32 * f).clamp(0.0, 255.0) as u8;
    let fb = (c.b as f32 * f).clamp(0.0, 255.0) as u8;
    Color::new(fr, fg, fb, c.a)
}

fn tron_wall_color(cell: char) -> Color {
    match cell {
        '+' | '|' | '-' => Color::new(0, 255, 255, 255),     // cian neón
        'g'             => Color::new(255, 140, 0, 255),     // naranja meta
        _               => Color::new(180, 180, 255, 255),   // fallback
    }
}

fn draw_scanlines(d: &mut RaylibDrawHandle, w: i32, h: i32, spacing: i32, alpha: u8) {
    let line_color = Color::new(0, 0, 0, alpha);
    let mut y = 0;
    while y < h {
        d.draw_rectangle(0, y, w, 1, line_color);
        y += spacing;
    }
}


fn cell_to_color(cell: char) -> Color {
    match cell {
        '+' | '|' | '-' => Color::new(0, 210, 255, 255),  // cian más suave (no tan chillón)
        'g'             => Color::new(255, 130, 20, 255), // naranja un poco más cálido
        _               => Color::new(16, 20, 32, 255),   // fallback oscuro (poco probable)
    }
}



fn draw_cell(
  framebuffer: &mut Framebuffer,
  xo: usize,
  yo: usize,
  block_size: usize,
  cell: char,
) {
  if cell == ' ' {
    return;
  }
  let color = cell_to_color(cell);
  framebuffer.set_current_color(color);

  for x in xo..xo + block_size {
    for y in yo..yo + block_size {
      framebuffer.set_pixel(x as u32, y as u32);
    }
  }
}

fn render_minimap(
    framebuffer: &mut Framebuffer,
    maze: &Maze,
    block_size: usize,
    player: &Player,
    origin_x: usize,
    origin_y: usize,
    scale: usize,
) {
    // (opcional) fondo oscuro para que resalten los neones
    let map_w = maze[0].len() * scale;
    let map_h = maze.len() * scale;
    framebuffer.set_current_color(Color::new(8, 10, 18, 255));
    for x in origin_x..origin_x + map_w {
        for y in origin_y..origin_y + map_h {
            framebuffer.set_pixel(x as u32, y as u32);
        }
    }

    // celdas
    for (i, row) in maze.iter().enumerate() {
        for (j, &cell) in row.iter().enumerate() {
            if cell == ' ' { continue; } // ← no pintes vacío
            let color = tron_wall_color(cell);
            framebuffer.set_current_color(color);

            let xo = origin_x + j * scale;
            let yo = origin_y + i * scale;
            for x in xo..xo + scale {
                for y in yo..yo + scale {
                    framebuffer.set_pixel(x as u32, y as u32);
                }
            }
        }
    }

    // jugador
    framebuffer.set_current_color(Color::YELLOW);
    let px = (player.pos.x / block_size as f32) * scale as f32;
    let py = (player.pos.y / block_size as f32) * scale as f32;
    let pxi = origin_x as f32 + px;
    let pyi = origin_y as f32 + py;
    framebuffer.set_pixel(pxi as u32, pyi as u32);

    // dirección del jugador
    framebuffer.set_current_color(Color::ORANGE);
    let dir_len = (scale as f32 * 2.0).max(6.0);
    let dx = player.a.cos() * dir_len;
    let dy = player.a.sin() * dir_len;
    let steps = dir_len as i32;
    for t in 0..=steps {
        let x = pxi + dx * (t as f32 / steps as f32);
        let y = pyi + dy * (t as f32 / steps as f32);
        framebuffer.set_pixel(x as u32, y as u32);
    }
}

pub fn render_maze(
  framebuffer: &mut Framebuffer,
  maze: &Maze,
  block_size: usize,
  player: &Player,
) {
  for (row_index, row) in maze.iter().enumerate() {
    for (col_index, &cell) in row.iter().enumerate() {
      let xo = col_index * block_size;
      let yo = row_index * block_size;
      draw_cell(framebuffer, xo, yo, block_size, cell);
    }
  }

  framebuffer.set_current_color(Color::WHITESMOKE);

  // draw what the player sees
  let num_rays = 5;
  for i in 0..num_rays {
    let current_ray = i as f32 / num_rays as f32; // current ray divided by total rays
    let a = player.a - (player.fov / 2.0) + (player.fov * current_ray);
    cast_ray(framebuffer, &maze, &player, a, block_size, true);
  }
}

fn render_world(
    framebuffer: &mut Framebuffer,
    maze: &Maze,
    block_size: usize,
    player: &Player,
    walls: &WallTex,          
    floor_tex: &CpuImage,
    sky_tex: &CpuImage,
    tron_time: f32,
    depth: &mut [f32],
) {
    let w = framebuffer.width as i32;
    let h = framebuffer.height as i32;
    let hh = h as f32 * 0.5;

    for i in 0..w {
        // Ángulo del rayo para esta columna
        let t = i as f32 / w as f32;
        let a = player.a - (player.fov * 0.5) + (player.fov * t);
        let dir = Vector2::new(a.cos(), a.sin());

        // Raycast
        let intersect = cast_ray(framebuffer, maze, player, a, block_size, false);

        // --- Parche anti-freeze ---
        let mut dist = intersect.distance;
        if !dist.is_finite() { dist = 1.0; }
        if dist < 0.0005 { dist = 0.0005; }

        // Guarda la distancia del muro para esta columna:
        if let Some(slot) = depth.get_mut(i as usize) {
            *slot = dist;
        }

        // Proyección de pared
        let dpp = 70.0;
        let stake_h = (hh / dist) * dpp;
        let wall_top = (hh - stake_h * 0.5) as i32;
        let wall_bot = (hh + stake_h * 0.5) as i32;

        let start = wall_top.clamp(0, h);
        let end   = wall_bot.clamp(0, h);

        // ---------------------------
        //  A) CIELO / FONDO (0..start)
        // ---------------------------
        if start > 0 {
            for y in 0..start {
                let denom = hh - y as f32;
                if denom.abs() < 0.0001 { continue; }
                let row_dist = (hh / denom) * dpp;

                let wx = player.pos.x + dir.x * row_dist;
                let wy = player.pos.y + dir.y * row_dist;

                let u = ((wx / block_size as f32).fract() + 1.0).fract();
                let v = ((wy / block_size as f32).fract() + 1.0).fract();

                let mut col = sky_tex.sample_repeat(u, v);
                let sky_gain = (0.65 + (y as f32 / hh) * 0.2).clamp(0.5, 0.95);
                col = scale_color(col, sky_gain);

                framebuffer.set_current_color(col);
                framebuffer.set_pixel(i as u32, y as u32);
            }
        }

        // ---------------------------------
        //  B) PARED (start..end) con textura
        // ---------------------------------
        if end > start {
            // Punto de impacto para u/v en pared
            let hit_x = player.pos.x + dist * dir.x;
            let hit_y = player.pos.y + dist * dir.y;

            let fx = ((hit_x / block_size as f32).fract() + 1.0).fract();
            let fy = ((hit_y / block_size as f32).fract() + 1.0).fract();

            // ¿vertical u horizontal?
            let edge_x = fx.min(1.0 - fx);
            let edge_y = fy.min(1.0 - fy);
            let u_wall = if edge_x < edge_y { fy } else { fx };

            // Celda golpeada (elige textura)
            let (ci, cj) = world_to_cell(hit_x, hit_y, block_size);
            let cell_ch = if ci < maze.len() && cj < maze[ci].len() { maze[ci][cj] } else { ' ' };
            let wall_img = walls.for_cell(cell_ch);

            // Sombreado suave tipo TRON
            let pulse = (tron_time * 3.0).sin() * 0.06;
            let dist_falloff = (1.15 / (1.0 + dist * 0.025)).clamp(0.22, 1.0);
            let column_gain = (dist_falloff + pulse).clamp(0.18, 1.0);

            let denom = (end - start).max(1) as f32;
            for y in start..end {
                let v_wall = (y - start) as f32 / denom; // 0..1
                let mut col = wall_img.sample_repeat(u_wall, v_wall);

                // (Opcional) si quieres teñir la meta 'g' aunque tenga su propia textura, deja esto:
                if cell_ch == 'g' {
                    let tint = Color::new(255, 140, 0, 255);
                    col = Color::new(
                        (col.r as f32 * 0.4 + tint.r as f32 * 0.6) as u8,
                        (col.g as f32 * 0.4 + tint.g as f32 * 0.6) as u8,
                        (col.b as f32 * 0.4 + tint.b as f32 * 0.6) as u8,
                        255
                    );
                }

                col = scale_color(col, column_gain);
                framebuffer.set_current_color(col);
                framebuffer.set_pixel(i as u32, y as u32);
            }
        }

        // ---------------------------
        //  C) PISO (end..h)
        // ---------------------------
        if end < h {
            for y in end..h {
                let denom = y as f32 - hh;
                if denom.abs() < 0.0001 { continue; }
                let row_dist = (hh / denom) * dpp;

                let wx = player.pos.x + dir.x * row_dist;
                let wy = player.pos.y + dir.y * row_dist;

                let u = ((wx / block_size as f32).fract() + 1.0).fract();
                let v = ((wy / block_size as f32).fract() + 1.0).fract();

                let mut col = floor_tex.sample_repeat(u, v);

                let floor_gain = (0.95 / (1.0 + row_dist * 0.01)).clamp(0.25, 0.9);
                col = scale_color(col, floor_gain);

                framebuffer.set_current_color(col);
                framebuffer.set_pixel(i as u32, y as u32);
            }
        }
    }
}

fn player_on_goal(player: &Player, maze: &Maze, block_size: usize) -> bool {
    let (i, j) = world_to_cell(player.pos.x, player.pos.y, block_size);
    if i >= maze.len() || j >= maze[i].len() { return false; }
    maze[i][j] == 'g'
}

fn draw_fullscreen(d: &mut RaylibDrawHandle, tex: &Texture2D, w: i32, h: i32) {
    // Dibuja la textura escalada a toda la ventana
    let src = Rectangle::new(0.0, 0.0, tex.width as f32, tex.height as f32);
    let dest = Rectangle::new(0.0, 0.0, w as f32, h as f32);
    d.draw_texture_pro(tex, src, dest, Vector2::new(0.0, 0.0), 0.0, Color::WHITE);
}

fn is_walking(win: &RaylibHandle) -> bool {
    win.is_key_down(KeyboardKey::KEY_W) || win.is_key_down(KeyboardKey::KEY_S)
}

fn main() {
  let window_width = 1300;
  let window_height = 900;
  let block_size = 150;

  let (mut window, raylib_thread) = raylib::init()
    .size(window_width, window_height)
    .title("Raycaster Project")
    .log_level(TraceLogLevel::LOG_WARNING)
    .build();

  //Musica
  let audio = RaylibAudio::init_audio_device()
      .expect("No se pudo inicializar el dispositivo de audio");

  // Música de fondo
  let music = audio.new_music("assets/music/tronMusic.ogg")
      .expect("Falta assets/music/tronMusic.ogg");
  music.set_volume(0.3);     // opcional
  music.play_stream();       // ¡a sonar!

  // SFX
  let step_sfx = audio.new_sound("assets/sfx/motor.ogg")
      .expect("Falta assets/sfx/motor.ogg");
  step_sfx.set_volume(0.8);
  let win_sfx = audio.new_sound("assets/sfx/winSound.mp3")
      .expect("Falta assets/sfx/winSound.mp3");


  //Captura el mouse desde el inicio (modo 3D)
  window.disable_cursor();
  
  let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
  framebuffer.set_background_color(Color::new(50, 50, 100, 255));

  let assets = Assets {
    initial: window
        .load_texture(&raylib_thread, "assets/textures/initial_page2.jpg")
        .expect("initial_page.png no encontrada"),
    win: window
        .load_texture(&raylib_thread, "assets/textures/win_page2.jpg")
        .expect("win_page.png no encontrada"),
  };

  let mut walls = WallTex::new(CpuImage::from_path("assets/textures/wall_grid4.jpg")); // default

  // Opcionales por tipo de celda
  walls.insert('+', CpuImage::from_path("assets/textures/wall_grid8.jpg"));
  walls.insert('|', CpuImage::from_path("assets/textures/wall_grid7.jpg"));
  walls.insert('-', CpuImage::from_path("assets/textures/wall_grid3.jpg"));
  // meta 'g' también tenga su propia textura:
  walls.insert('g', CpuImage::from_path("assets/textures/wall_grid6.jpg"));

  let floor_cpu = CpuImage::from_path("assets/textures/floor3.jpg");
  let sky_cpu   = CpuImage::from_path("assets/textures/wall_grid.jpg");

  
  let mut maze = load_maze("assets/maps/level1.txt");
  let mut player = Player {
    pos: Vector2::new(150.0, 150.0),
    a: PI / 3.0,
    fov: PI / 3.0,
  };

  let mut mode_2d = false;
  let mut state = GameState::Title;
  let levels: Vec<&str> = vec![
      "assets/maps/level1.txt",
      "assets/maps/level2.txt",
      "assets/maps/level3.txt"
  ];
  let mut selected_level: usize = 0;

  // cursor: libre en menús, capturado en juego
  window.enable_cursor();
  let mut step_cd: f32 = 0.0;
  let mut tron_time: f32 = 0.0;
  let screen_w = framebuffer.width as usize;
  let mut depth = vec![f32::INFINITY; screen_w];

  let orb_frames = load_frames(&[
    "assets/sprites/moto5.png",
    "assets/sprites/moto5.png",
    "assets/sprites/moto4.png",
    "assets/sprites/moto4.png",
    ]);

    let mut sprites = vec![
        Sprite::new(Vector2::new(450.0, 260.0), orb_frames.clone(), 6.0, 1.0),
        Sprite::new(Vector2::new(700.0, 400.0), orb_frames.clone(), 6.0, 1.0),
        Sprite::new(Vector2::new(300.0, 600.0), orb_frames.clone(), 6.0, 1.0),
    ];

  while !window.window_should_close() {
      framebuffer.clear();

      let dt = window.get_frame_time();
      if step_cd > 0.0 { step_cd -= dt; }
      music.update_stream();
      tron_time += dt;


      match state {
          GameState::Title => {
              // Input
              if window.is_key_pressed(KeyboardKey::KEY_ENTER) {
                  state = if levels.len() > 1 { GameState::LevelSelect } else { GameState::Playing };
                  if state == GameState::Playing {
                      window.disable_cursor();
                  } else {
                      window.enable_cursor();
                  }
              }

              // Presentar con overlay de UI
              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
                  draw_fullscreen(d, &assets.initial, window_width, window_height); // 👈 fondo
                  d.draw_text("RAYCASTER", 500, 300, 48, Color::YELLOW);
                  d.draw_text("Presiona ENTER para empezar", 400, 440, 24, Color::WHITE);
                  d.draw_text("Presiona M para alternar 2D/3D durante el juego", 400, 470, 20, Color::GRAY);
                  d.draw_text("Controles: W/S mover, A/D girar, Mouse mirar", 400, 500, 20, Color::GRAY);
              });
          }

          GameState::LevelSelect => {
              // Navegación
              if window.is_key_pressed(KeyboardKey::KEY_DOWN) {
                  selected_level = (selected_level + 1) % levels.len();
              }
              if window.is_key_pressed(KeyboardKey::KEY_UP) {
                  selected_level = (selected_level + levels.len() - 1) % levels.len();
              }
              if window.is_key_pressed(KeyboardKey::KEY_ENTER) {
                maze = load_maze(levels[selected_level]);
                // reubica jugador  spawn fijo:
                player.pos = Vector2::new(190.0, 190.0);
                player.a = PI / 3.0;
                mode_2d = false;

                state = GameState::Playing;
                window.disable_cursor();
              }
              if window.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
                  state = GameState::Title;
              }

              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
                  draw_fullscreen(d, &assets.initial, window_width, window_height);
                  d.draw_text("Selecciona nivel:", 40, 40, 32, Color::YELLOW);
                  for (idx, path) in levels.iter().enumerate() {
                      let y = 90 + (idx as i32)*28;
                      let color = if idx == selected_level { Color::LIME } else { Color::WHITE };
                      d.draw_text(path, 60, y, 22, color);
                  }
                  d.draw_text("ENTER: jugar   ESC: volver", 40, 140 + (levels.len() as i32)*28, 20, Color::GRAY);
              });
          }

          GameState::Playing => {
            // Input + movimiento
            process_events(&mut player, &window, &maze, block_size);

            // 👇 SFX: pasos al caminar (usa tu helper is_walking)
            if is_walking(&window) && step_cd <= 0.0 {
                step_sfx.play();
                step_cd = 0.15; // 4 pasos por segundo aprox
            }

            // WIN check (antes de dibujar)
            if player_on_goal(&player, &maze, block_size) {
                win_sfx.play(); // 👈 SFX victoria
                state = GameState::Win;
                window.enable_cursor();
                mode_2d = false;
                continue; // saltar el render de este frame
            }


              // Toggle 2D/3D con M (persistente)
              if window.is_key_pressed(KeyboardKey::KEY_M) {
                  mode_2d = !mode_2d;
                  // (opcional) cursor libre en 2D, capturado en 3D
                  if mode_2d { window.enable_cursor(); } else { window.disable_cursor(); }
              }

                if mode_2d {
                    render_maze(&mut framebuffer, &maze, block_size, &player);
                } else {
                    // Vista 3D + minimapa
                    depth.fill(f32::INFINITY); // ← limpia el buffer cada frame

                    render_world(
                        &mut framebuffer, &maze, block_size, &player,
                        &walls, &floor_cpu, &sky_cpu, tron_time,
                        &mut depth, // 👈 pásale el buffer
                    );

                    // Actualiza y dibuja sprites
                    for s in sprites.iter_mut() { s.update(dt); }
                    render_sprites(&mut framebuffer, &player, &mut sprites, block_size, &depth);

                    render_minimap(&mut framebuffer, &maze, block_size, &player, 1200, 10, 8);
                }
                // ...existing code...

              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
                draw_scanlines(d, window_width, window_height, 2, 40);
              });
          }

          GameState::Win => {
              // Volver a Title o repetir
              if window.is_key_pressed(KeyboardKey::KEY_ENTER) {
                  state = GameState::Title;
                  window.enable_cursor();
              }

              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
                draw_fullscreen(d, &assets.win, window_width, window_height); // 👈 fondo
                d.draw_text("¡FELICIDADES!", 450, 180, 50, Color::GOLD);
                d.draw_text("¡Nivel completado!", 40, 320, 36, Color::GOLD);
                d.draw_text("ENTER: volver al menu", 40, 370, 24, Color::WHITE);
                });
          }
      }

      thread::sleep(Duration::from_millis(16));
  }


}



