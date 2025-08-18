// player.rs

use raylib::prelude::*;
use std::f32::consts::PI;
use crate::maze::{Maze, is_wall, world_to_cell};

pub struct Player {
    pub pos: Vector2,
    pub a: f32,
    pub fov: f32, // field of view
}

pub fn process_events(player: &mut Player, rl: &RaylibHandle, maze: &Maze, block_size: usize) {
    const MOVE_SPEED: f32 = 4.0;
    const ROTATION_SPEED: f32 = PI / 60.0;

    // --- Rotación con A/D
    if rl.is_key_down(KeyboardKey::KEY_A) {
        player.a -= ROTATION_SPEED;
    }
    if rl.is_key_down(KeyboardKey::KEY_D) {
        player.a += ROTATION_SPEED;
    }

    // --- Movimiento hacia adelante/atrás con W/S
    let mut forward = 0.0;
    if rl.is_key_down(KeyboardKey::KEY_W) { forward += 1.0; }
    if rl.is_key_down(KeyboardKey::KEY_S) { forward -= 1.0; }

    let dir = Vector2::new(player.a.cos(), player.a.sin());
    let step = Vector2::new(dir.x * (MOVE_SPEED * forward), dir.y * (MOVE_SPEED * forward));

    let next_x = player.pos.x + step.x;
    let next_y = player.pos.y + step.y;

    // --- Colisión
    let (ci, cj) = world_to_cell(next_x, next_y, block_size);
    if !is_wall(maze, ci, cj) {
        player.pos.x = next_x;
        player.pos.y = next_y;
    }
}
