// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameState { Title, LevelSelect, Playing, Win }
struct Assets {
    initial: Texture2D,
    win: Texture2D,
    wall_grid: Texture2D,
    floor: Texture2D,
}


mod line;
mod framebuffer;
mod maze;
mod caster;
mod player;

use line::line;
use maze::{Maze,load_maze};
use caster::{cast_ray, Intersect};
use framebuffer::Framebuffer;
use player::{Player, process_events};

use raylib::prelude::*;
use std::thread;
use std::time::Duration;
use std::f32::consts::PI;

use crate::maze::world_to_cell;

fn cell_to_color(cell: char) -> Color {
  match cell {
    '+' => {
      return Color::BLUEVIOLET;
    },
    '-' => {
      return Color::VIOLET;
    },
    '|' => {
      return Color::VIOLET;
    },
    'g' => {
      return Color::GREEN;
    },
    _ => {
      return Color::WHITE;
    },
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
    origin_x: usize, // esquina donde va el minimapa
    origin_y: usize,
    scale: usize,    // tamaño de celda del minimapa (p.ej. 4 px)
) {
    // 1) celdas del laberinto
    for (i, row) in maze.iter().enumerate() {
        for (j, &cell) in row.iter().enumerate() {
            let color = match cell {
                ' ' => Color::new(0, 0, 0, 0),       // transparente (no dibujar)
                'g' => Color::GREEN,                  // meta
                '+' => Color::BLUEVIOLET,
                '-' | '|' => Color::VIOLET,
                _ => Color::WHITE,
            };
            if color.a > 0 {
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
    }

    // 2) jugador (punto)
    framebuffer.set_current_color(Color::YELLOW);
    let px = (player.pos.x / block_size as f32) * scale as f32;
    let py = (player.pos.y / block_size as f32) * scale as f32;
    let pxi = origin_x as f32 + px;
    let pyi = origin_y as f32 + py;
    framebuffer.set_pixel(pxi as u32, pyi as u32);

    // 3) dirección del jugador (línea corta)
    let dir_len = (scale as f32 * 2.0).max(6.0);
    let dx = player.a.cos() * dir_len;
    let dy = player.a.sin() * dir_len;

    // dibuja línea con el método que uses (aquí a mano, pasos Bresenham simple)
    framebuffer.set_current_color(Color::ORANGE);
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
) {
  let num_rays = framebuffer.width;

  // let hw = framebuffer.width as f32 / 2.0;   // precalculated half width
  let hh = framebuffer.height as f32 / 2.0;  // precalculated half height

  framebuffer.set_current_color(Color::WHITESMOKE);

  for i in 0..num_rays {
    let current_ray = i as f32 / num_rays as f32; // current ray divided by total rays
    let a = player.a - (player.fov / 2.0) + (player.fov * current_ray);
    let intersect = cast_ray(framebuffer, &maze, &player, a, block_size, false);

    // --- Parche anti-freeze (drop-in) ---
    let mut dist = intersect.distance;
    // evita infinito/NaN y distancias casi cero
    if !dist.is_finite() { dist = 1.0; }
    if dist < 0.0005 { dist = 0.0005; }

    let distance_to_projection_plane = 70.0;
    let stake_height = (hh / dist) * distance_to_projection_plane;

    // Calcula top/bottom como i32 para poder recortar
    let stake_top_i32 = (hh - (stake_height / 2.0)) as i32;
    let stake_bottom_i32 = (hh + (stake_height / 2.0)) as i32;

    // Recorta a límites de pantalla [0, height]
    let h_i32 = framebuffer.height as i32;
    let start = stake_top_i32.clamp(0, h_i32);
    let end   = stake_bottom_i32.clamp(0, h_i32);

    // Dibuja seguro dentro de la pantalla
    for y in start..end {
        framebuffer.set_pixel(i, y as u32);
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


fn main() {
  let window_width = 1300;
  let window_height = 900;
  let block_size = 100;

  let (mut window, raylib_thread) = raylib::init()
    .size(window_width, window_height)
    .title("Raycaster Example")
    .log_level(TraceLogLevel::LOG_WARNING)
    .build();

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
    wall_grid: window
        .load_texture(&raylib_thread, "assets/textures/wall_grid.jpg")
        .expect("wall_grid.png no encontrada"),
    floor: window
        .load_texture(&raylib_thread, "assets/textures/floor.jpg")
        .expect("floor.png no encontrada"),
  };

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

  while !window.window_should_close() {
      framebuffer.clear();

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

              // Fondo simple (2D) para no dejar la pantalla vacía
              render_maze(&mut framebuffer, &maze, block_size, &player);

              // Presentar con overlay de UI
              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
                  draw_fullscreen(d, &assets.initial, window_width, window_height); // 👈 fondo
                  d.draw_text("RAYCASTER", 500, 300, 48, Color::YELLOW);
                  d.draw_text("Presiona ENTER para empezar", 400, 540, 24, Color::WHITE);
                  d.draw_text("Presiona M para alternar 2D/3D durante el juego", 400, 570, 20, Color::GRAY);
                  d.draw_text("Controles: W/S mover, A/D girar, Mouse mirar", 400, 600, 20, Color::GRAY);
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
                player.pos = Vector2::new(150.0, 150.0);
                player.a = PI / 3.0;
                mode_2d = false;

                state = GameState::Playing;
                window.disable_cursor();
              }
              if window.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
                  state = GameState::Title;
              }

              render_maze(&mut framebuffer, &maze, block_size, &player);

              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
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

              // WIN check
              if player_on_goal(&player, &maze, block_size) {
                  state = GameState::Win;     // 👈 ir a Win, no a LevelSelect
                  window.enable_cursor();
                  mode_2d = false;
                  continue;                   // saltar el render del frame actual
              }


              // Toggle 2D/3D con M (persistente)
              if window.is_key_pressed(KeyboardKey::KEY_M) {
                  mode_2d = !mode_2d;
                  // (opcional) cursor libre en 2D, capturado en 3D
                  if mode_2d { window.enable_cursor(); } else { window.disable_cursor(); }
              }

              if mode_2d {
                  // Vista 2D top-down
                  render_maze(&mut framebuffer, &maze, block_size, &player);
              } else {
                  // Vista 3D + minimapa
                  render_world(&mut framebuffer, &maze, block_size, &player);
                  render_minimap(&mut framebuffer, &maze, block_size, &player, 1200, 10, 8);
              }

              framebuffer.present_with_ui(&mut window, &raylib_thread, |_| {});
          }

          GameState::Win => {
              // Volver a Title o repetir
              if window.is_key_pressed(KeyboardKey::KEY_ENTER) {
                  state = GameState::Title;
                  window.enable_cursor();
              }

              // Pantalla simple de victoria
              render_maze(&mut framebuffer, &maze, block_size, &player);

              framebuffer.present_with_ui(&mut window, &raylib_thread, |d| {
                draw_fullscreen(d, &assets.win, window_width, window_height); // 👈 fondo
                d.draw_text("¡FELICIDADES!", 40, 40, 36, Color::GOLD);
                d.draw_text("¡Nivel completado!", 40, 80, 36, Color::GOLD);
                d.draw_text("ENTER: volver al menu", 40, 130, 24, Color::WHITE);
                });
          }
      }

      thread::sleep(Duration::from_millis(16));
  }


}



