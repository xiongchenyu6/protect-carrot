//! The playfield for the currently-loaded level: the enemy path (as world-space
//! waypoints) and which grid cells are buildable. Ported from the original
//! `generatePathAndBuildable`.

use crate::data::{COLS, Level, ROWS, cell_center};
use bevy::prelude::*;
use std::collections::HashSet;

/// Resource describing the active level's geometry. Rebuilt whenever a level loads.
#[derive(Resource, Default, Clone)]
#[allow(dead_code)]
pub struct Board {
    pub level_index: usize,
    /// Path waypoints in grid coordinates (col,row), in order.
    pub path_cells: Vec<(i32, i32)>,
    /// Path waypoints converted to world-space centers — enemies walk these.
    pub path_world: Vec<Vec2>,
    /// Cells where towers may be placed (everything not on the path).
    pub buildable: HashSet<(i32, i32)>,
}

impl Board {
    /// Build the board geometry for a level. `path` from the level is a list of
    /// turning points; we expand the straight segments between them into the full
    /// set of occupied cells (same logic as the JS version), then everything else
    /// becomes buildable.
    pub fn from_level(level_index: usize, level: &Level) -> Self {
        let pts = &level.path;
        let mut occupied: HashSet<(i32, i32)> = HashSet::new();

        for w in pts.windows(2) {
            let (x1, y1) = w[0];
            let (x2, y2) = w[1];
            if x1 == x2 {
                for y in y1.min(y2)..=y1.max(y2) {
                    occupied.insert((x1, y));
                }
            } else {
                for x in x1.min(x2)..=x1.max(x2) {
                    occupied.insert((x, y1));
                }
            }
        }

        let mut buildable = HashSet::new();
        for x in 0..COLS {
            for y in 0..ROWS {
                if !occupied.contains(&(x, y)) {
                    buildable.insert((x, y));
                }
            }
        }

        // The waypoints (turning points) are what enemies actually steer toward,
        // matching the JS which moves point-to-point along `level.path`.
        let path_world = pts
            .iter()
            .map(|&(c, r)| cell_center(c as f32, r as f32))
            .collect();

        Board {
            level_index,
            path_cells: pts.clone(),
            path_world,
            buildable,
        }
    }

    /// World position of the carrot (path end).
    pub fn carrot_pos(&self) -> Vec2 {
        *self.path_world.last().unwrap_or(&Vec2::ZERO)
    }

    /// World position where enemies spawn (path start).
    pub fn spawn_pos(&self) -> Vec2 {
        *self.path_world.first().unwrap_or(&Vec2::ZERO)
    }
}
