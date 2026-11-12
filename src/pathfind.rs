use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::world::World;

pub type Pos = (i32, i32, i32);

struct Node {
    cost: f64,
    estimate: f64,
    pos: Pos,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.estimate == other.estimate
    }
}
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap on estimated total cost (reverse so BinaryHeap pops smallest).
        other
            .estimate
            .partial_cmp(&self.estimate)
            .unwrap_or(Ordering::Equal)
    }
}

fn heuristic(a: Pos, b: Pos) -> f64 {
    let dx = (a.0 - b.0) as f64;
    let dy = (a.1 - b.1) as f64;
    let dz = (a.2 - b.2) as f64;
    (dx * dx + dz * dz).sqrt() + dy.abs() * 0.5
}

/// Walkable neighbours of a standing cell: flat moves, a single step up, and
/// drops of up to three blocks. A cell with no standable neighbour (a wall) is
/// simply never expanded, so A* routes around it.
fn neighbours(world: &World, (x, y, z): Pos, out: &mut Vec<(Pos, f64)>) {
    out.clear();
    const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for (dx, dz) in DIRS {
        let nx = x + dx;
        let nz = z + dz;
        if world.standable(nx, y, nz) {
            out.push(((nx, y, nz), 1.0));
        } else if world.standable(nx, y + 1, nz) && !world.is_solid(x, y + 2, z) {
            out.push(((nx, y + 1, nz), 1.5)); // step up
        } else {
            for drop in 1..=3 {
                if world.standable(nx, y - drop, nz) {
                    out.push(((nx, y - drop, nz), 1.0 + drop as f64 * 0.3));
                    break;
                }
            }
        }
    }
}

/// A* from `start` to `goal` over walkable cells. Returns the waypoint list
/// (excluding the start) or None when no path is found within `max_nodes`.
pub fn find_path(world: &World, start: Pos, goal: Pos, max_nodes: usize) -> Option<Vec<Pos>> {
    if start == goal {
        return Some(Vec::new());
    }
    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<Pos, Pos> = HashMap::new();
    let mut g_score: HashMap<Pos, f64> = HashMap::new();
    let mut buffer = Vec::with_capacity(4);

    g_score.insert(start, 0.0);
    open.push(Node {
        cost: 0.0,
        estimate: heuristic(start, goal),
        pos: start,
    });

    let mut expanded = 0usize;
    while let Some(Node { cost, pos, .. }) = open.pop() {
        if pos == goal {
            return Some(reconstruct(&came_from, goal));
        }
        if cost > *g_score.get(&pos).unwrap_or(&f64::INFINITY) {
            continue;
        }
        expanded += 1;
        if expanded > max_nodes {
            break;
        }
        neighbours(world, pos, &mut buffer);
        for (next, step_cost) in buffer.drain(..) {
            let tentative = cost + step_cost;
            if tentative < *g_score.get(&next).unwrap_or(&f64::INFINITY) {
                came_from.insert(next, pos);
                g_score.insert(next, tentative);
                open.push(Node {
                    cost: tentative,
                    estimate: tentative + heuristic(next, goal),
                    pos: next,
                });
            }
        }
    }
    None
}

fn reconstruct(came_from: &HashMap<Pos, Pos>, goal: Pos) -> Vec<Pos> {
    let mut path = vec![goal];
    let mut current = goal;
    while let Some(previous) = came_from.get(&current) {
        current = *previous;
        path.push(current);
    }
    path.pop(); // drop the start cell
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_around_a_wall() {
        // Floor of stone at y=-1 across a 7x7 area, with a wall of blocks from
        // x=3 spanning z=0..=5 (leaving z=6 open) between start and goal.
        let mut world = World::new(774);
        for x in 0..7 {
            for z in 0..7 {
                world.ensure_floor(x, 64, z);
            }
        }
        for z in 0..6 {
            world.ensure_wall(3, 65, z);
            world.ensure_wall(3, 66, z);
        }
        let path = find_path(&world, (0, 65, 3), (6, 65, 3), 5000).expect("path exists");
        // Must detour through the gap at z=6, so the path visits a high z.
        assert!(path.iter().any(|p| p.2 >= 6), "path should route around the wall");
        assert_eq!(*path.last().unwrap(), (6, 65, 3));
    }

    #[test]
    fn straight_line_when_clear() {
        let mut world = World::new(774);
        for x in 0..5 {
            for z in 0..3 {
                world.ensure_floor(x, 64, z);
            }
        }
        let path = find_path(&world, (0, 65, 1), (4, 65, 1), 2000).expect("path");
        assert_eq!(path.len(), 4);
        assert_eq!(*path.last().unwrap(), (4, 65, 1));
    }
}
