// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]

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

    // Calculate the height of the stake
    let distance_to_wall = intersect.distance;// how far is this wall from the player
    let distance_to_projection_plane = 70.0; // how far is the "player" from the "camera"
    // this ratio doesn't really matter as long as it is a function of distance
    let stake_height = (hh / distance_to_wall) * distance_to_projection_plane;

    // Calculate the position to draw the stake
    let stake_top = (hh - (stake_height / 2.0)) as usize;
    let stake_bottom = (hh + (stake_height / 2.0)) as usize;

    // Draw the stake directly in the framebuffer
    for y in stake_top..stake_bottom {
      framebuffer.set_pixel(i, y as u32); // Assuming white color for the stake
    }
  }
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

  let maze = load_maze("assets/maps/level1.txt");
  let mut player = Player {
    pos: Vector2::new(150.0, 150.0),
    a: PI / 3.0,
    fov: PI / 3.0,
  };

  let mut mode_2d = false;

  while !window.window_should_close() {
      // 1) limpiar
      framebuffer.clear();

      // 2) input / movimiento
      process_events(&mut player, &window, &maze, block_size);

      // 3) toggle modo
      if window.is_key_pressed(KeyboardKey::KEY_M) {
          mode_2d = !mode_2d;
          if mode_2d {
              window.enable_cursor();   // suelta el cursor (modo 2D)
          } else {
              window.disable_cursor();  // captura el cursor (modo 3D)
          }
      }

      // 4) render según modo
      if mode_2d {
          // 2D top-down
          render_maze(&mut framebuffer, &maze, block_size, &player);
      } else {
          // 3D raycasting
          render_world(&mut framebuffer, &maze, block_size, &player);

          // Overlay: minimapa en esquina (ajusta posición/escala)
          render_minimap(&mut framebuffer, &maze, block_size, &player, 1200, 10, 8);
      }

      // 5) presentar
      framebuffer.swap_buffers(&mut window, &raylib_thread);

      // 6) limitar ~60 fps
      thread::sleep(Duration::from_millis(16));
}

}



