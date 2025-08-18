// maze.rs

use std::fs::File;
use std::io::{BufRead, BufReader};

pub type Maze = Vec<Vec<char>>;

pub fn load_maze(filename: &str) -> Maze {
    let file = File::open(filename).unwrap();
    let reader = BufReader::new(file);

    reader
        .lines()
        .map(|line| line.unwrap().chars().collect())
        .collect()
}

pub fn world_to_cell(x: f32, y: f32, block_size: usize) -> (usize, usize) {
    let j = (x / block_size as f32).floor().max(0.0) as usize; // columna
    let i = (y / block_size as f32).floor().max(0.0) as usize; // fila
    (i, j)
}

pub fn is_wall(maze: &Maze, i: usize, j: usize) -> bool {
    if i >= maze.len() { return true; }
    if j >= maze[i].len() { return true; }
    let c = maze[i][j];
    c != ' ' && c != 'g' // 'g' lo reservamos como meta (no pared)
}