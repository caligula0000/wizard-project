//! Authoritative material lives in terrain or a body-local grid, never in coverage.
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Material {
    Empty,
    Rock,
    Wood,
    Sand,
}

impl Material {
    pub fn rgba(self) -> [u8; 4] {
        match self {
            Self::Empty => [18, 22, 34, 255],
            Self::Rock => [72, 94, 104, 255],
            Self::Wood => [219, 161, 77, 255],
            Self::Sand => [245, 220, 130, 255],
        }
    }
}

#[derive(Clone)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Material>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Material::Empty; width * height],
        }
    }

    pub fn count(&self) -> usize {
        self.cells.iter().filter(|&&m| m != Material::Empty).count()
    }
}

pub struct Body {
    pub grid: Grid,
    pub position: [f32; 2],
    pub angle: f32,
    pub angular_velocity: f32,
}

impl Body {
    fn world_point(&self, x: f32, y: f32) -> [f32; 2] {
        let (s, c) = self.angle.sin_cos();
        let dx = x - self.grid.width as f32 * 0.5;
        let dy = y - self.grid.height as f32 * 0.5;
        [
            self.position[0] + c * dx - s * dy,
            self.position[1] + s * dx + c * dy,
        ]
    }

    /// Four-neighbour components. Allocate only on edits, not in the frame loop.
    fn split(self) -> Vec<Body> {
        let mut seen = vec![false; self.grid.cells.len()];
        let mut queue = VecDeque::new();
        let mut component = Vec::new();
        let mut result = Vec::new();
        let w = self.grid.width;
        for start in 0..self.grid.cells.len() {
            if seen[start] || self.grid.cells[start] == Material::Empty {
                continue;
            }
            seen[start] = true;
            queue.push_back(start);
            component.clear();
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, self.grid.height, 0, 0);
            while let Some(i) = queue.pop_front() {
                component.push(i);
                let (x, y) = (i % w, i / w);
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                let neighbours = [
                    (x > 0).then(|| i - 1),
                    (x + 1 < w).then_some(i + 1),
                    (y > 0).then(|| i - w),
                    (y + 1 < self.grid.height).then_some(i + w),
                ];
                for next in neighbours.into_iter().flatten() {
                    if !seen[next] && self.grid.cells[next] != Material::Empty {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
            let mut grid = Grid::new(max_x - min_x + 1, max_y - min_y + 1);
            for &i in &component {
                let target = (i / w - min_y) * grid.width + i % w - min_x;
                grid.cells[target] = self.grid.cells[i];
            }
            let position = self.world_point(
                min_x as f32 + grid.width as f32 * 0.5,
                min_y as f32 + grid.height as f32 * 0.5,
            );
            result.push(Body {
                grid,
                position,
                angle: self.angle,
                angular_velocity: self.angular_velocity,
            });
        }
        result
    }
}

pub struct World {
    pub terrain: Grid,
    pub bodies: Vec<Body>,
    /// Derived coverage: zero means no body, otherwise body index + 1.
    pub coverage: Vec<u32>,
    pub visible: Vec<Material>,
    pub overlaps: usize,
    sand_tick: u64,
}

impl World {
    pub fn demo(width: usize, height: usize, count: usize) -> Self {
        let mut terrain = Grid::new(width, height);
        for y in height - 12..height {
            terrain.cells[y * width..(y + 1) * width].fill(Material::Rock);
        }
        let columns = (count as f32).sqrt().ceil().max(1.0) as usize;
        let rows = count.div_ceil(columns).max(1);
        let bodies = (0..count)
            .map(|i| {
                let mut grid = Grid::new(61, 15);
                grid.cells.fill(Material::Wood);
                // An authored hole demonstrates that geometry comes from material.
                for y in 5..10 {
                    for x in 8..18 {
                        grid.cells[y * grid.width + x] = Material::Empty;
                    }
                }
                Body {
                    grid,
                    position: [
                        (i % columns) as f32 * width as f32 / columns as f32
                            + width as f32 / columns as f32 * 0.5,
                        (i / columns) as f32 * (height - 24) as f32 / rows as f32
                            + (height - 24) as f32 / rows as f32 * 0.5,
                    ],
                    angle: i as f32 * 0.3,
                    angular_velocity: if i % 2 == 0 { 0.35 } else { -0.25 },
                }
            })
            .collect();
        Self {
            coverage: vec![0; width * height],
            visible: vec![Material::Empty; width * height],
            terrain,
            bodies,
            overlaps: 0,
            sand_tick: 0,
        }
    }

    pub fn step(&mut self, dt: f32) {
        for body in &mut self.bodies {
            body.angle =
                (body.angle + dt * body.angular_velocity).rem_euclid(std::f32::consts::TAU);
        }
    }

    /// Inverse sample destination cell centres. Stable first-body-wins overlap policy.
    /// Coverage is lossy and never feeds back into authoritative material.
    pub fn rasterize(&mut self) {
        self.coverage.fill(0);
        self.visible.copy_from_slice(&self.terrain.cells);
        self.overlaps = 0;
        let width = self.terrain.width;
        for (id, body) in self.bodies.iter().enumerate() {
            let bw = body.grid.width as f32;
            let bh = body.grid.height as f32;
            let (s, c) = body.angle.sin_cos();
            let ex = (c.abs() * bw + s.abs() * bh) * 0.5;
            let ey = (s.abs() * bw + c.abs() * bh) * 0.5;
            let x0 = (body.position[0] - ex).floor().max(0.0) as usize;
            let y0 = (body.position[1] - ey).floor().max(0.0) as usize;
            let x1 = ((body.position[0] + ex).ceil().max(0.0) as usize).min(width);
            let y1 = ((body.position[1] + ey).ceil().max(0.0) as usize).min(self.terrain.height);
            for y in y0..y1 {
                let dy = y as f32 + 0.5 - body.position[1];
                for x in x0..x1 {
                    let dx = x as f32 + 0.5 - body.position[0];
                    // After rejecting negative coordinates, integer truncation equals floor.
                    // Avoid two floorf calls per destination cell on baseline x86-64.
                    let lx = c * dx + s * dy + bw * 0.5;
                    let ly = -s * dx + c * dy + bh * 0.5;
                    if lx < 0.0 || ly < 0.0 || lx >= bw || ly >= bh {
                        continue;
                    }
                    if body.grid.cells[ly as usize * body.grid.width + lx as usize]
                        == Material::Empty
                    {
                        continue;
                    }
                    let i = y * width + x;
                    if self.coverage[i] != 0 || self.terrain.cells[i] != Material::Empty {
                        self.overlaps += 1;
                    }
                    if self.coverage[i] == 0 {
                        self.coverage[i] = id as u32 + 1;
                        self.visible[i] =
                            body.grid.cells[ly as usize * body.grid.width + lx as usize];
                    }
                }
            }
        }
    }

    /// Call after rasterize so collision coverage matches the current body transforms.
    /// Downward-only moves and bottom-up traversal guarantee at most one move per grain.
    pub fn update_sand(&mut self) {
        let w = self.terrain.width;
        let h = self.terrain.height;
        self.sand_tick = self.sand_tick.wrapping_add(1);
        for y in (0..h.saturating_sub(1)).rev() {
            for column in 0..w {
                let x = if self.sand_tick & 1 == 0 {
                    column
                } else {
                    w - 1 - column
                };
                let source = y * w + x;
                if self.terrain.cells[source] != Material::Sand || self.coverage[source] != 0 {
                    continue;
                }
                let below = source + w;
                // Deterministic spatial/tick hash varies diagonal preference, without RNG state per cell.
                let mut hash =
                    (source as u64).wrapping_add(self.sand_tick.wrapping_mul(0x9e3779b97f4a7c15));
                hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                let left = (x > 0).then(|| below - 1);
                let right = (x + 1 < w).then_some(below + 1);
                let diagonals = if hash & 1 == 0 {
                    [left, right]
                } else {
                    [right, left]
                };
                for destination in [Some(below), diagonals[0], diagonals[1]]
                    .into_iter()
                    .flatten()
                {
                    if self.terrain.cells[destination] == Material::Empty
                        && self.coverage[destination] == 0
                    {
                        self.terrain.cells.swap(source, destination);
                        self.visible[source] = Material::Empty;
                        self.visible[destination] = Material::Sand;
                        break;
                    }
                }
            }
        }
    }

    /// Paint only into empty, uncovered cells. Returns the number of new grains.
    /// Caller supplies fresh coverage (rasterize after edits/transforms).
    pub fn add_sand(&mut self, point: [f32; 2], radius: f32) -> usize {
        let w = self.terrain.width;
        let h = self.terrain.height;
        let x0 = (point[0] - radius).floor().max(0.0) as usize;
        let y0 = (point[1] - radius).floor().max(0.0) as usize;
        let x1 = ((point[0] + radius).ceil().max(0.0) as usize).min(w);
        let y1 = ((point[1] + radius).ceil().max(0.0) as usize).min(h);
        let mut added = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * w + x;
                if (x as f32 + 0.5 - point[0]).powi(2) + (y as f32 + 0.5 - point[1]).powi(2)
                    <= radius * radius
                    && self.terrain.cells[i] == Material::Empty
                    && self.coverage[i] == 0
                {
                    self.terrain.cells[i] = Material::Sand;
                    self.visible[i] = Material::Sand;
                    added += 1;
                }
            }
        }
        added
    }

    pub fn render(&self, pixels: &mut [u8]) {
        assert_eq!(pixels.len(), self.terrain.cells.len() * 4);
        for (pixel, &material) in pixels.chunks_exact_mut(4).zip(&self.visible) {
            pixel.copy_from_slice(&material.rgba());
        }
    }

    /// Delete by testing authoritative cell centres, not the lossy coverage map.
    pub fn erase(&mut self, point: [f32; 2], radius: f32) -> usize {
        let mut removed = 0;
        for y in 0..self.terrain.height {
            for x in 0..self.terrain.width {
                if (x as f32 + 0.5 - point[0]).powi(2) + (y as f32 + 0.5 - point[1]).powi(2)
                    <= radius * radius
                {
                    let cell = &mut self.terrain.cells[y * self.terrain.width + x];
                    removed += usize::from(*cell != Material::Empty);
                    *cell = Material::Empty;
                }
            }
        }
        let mut bodies = Vec::new();
        for mut body in self.bodies.drain(..) {
            let mut changed = false;
            for y in 0..body.grid.height {
                for x in 0..body.grid.width {
                    let p = body.world_point(x as f32 + 0.5, y as f32 + 0.5);
                    if (p[0] - point[0]).powi(2) + (p[1] - point[1]).powi(2) <= radius * radius {
                        let cell = &mut body.grid.cells[y * body.grid.width + x];
                        if *cell != Material::Empty {
                            removed += 1;
                            changed = true;
                            *cell = Material::Empty;
                        }
                    }
                }
            }
            if changed {
                bodies.extend(body.split());
            } else {
                bodies.push(body);
            }
        }
        self.bodies = bodies;
        removed
    }

    pub fn count(&self) -> usize {
        self.terrain.count() + self.bodies.iter().map(|b| b.grid.count()).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sand_falls_one_cell_per_tick_and_settles() {
        let mut world = World::demo(40, 50, 0);
        world.terrain.cells[5 * 40 + 20] = Material::Sand;
        world.rasterize();
        world.update_sand();
        assert_eq!(world.terrain.cells[6 * 40 + 20], Material::Sand);
        let count = world.count();
        for _ in 0..100 {
            world.update_sand();
        }
        assert_eq!(world.terrain.cells[37 * 40 + 20], Material::Sand);
        assert_eq!(world.count(), count);
    }

    #[test]
    fn sand_pile_conserves_material_and_settles_deterministically() {
        let mut a = World::demo(80, 90, 0);
        let mut b = World::demo(80, 90, 0);
        for world in [&mut a, &mut b] {
            world.rasterize();
            let before = world.count();
            let added = world.add_sand([40.0, 15.0], 10.0);
            assert!(added > 0);
            for _ in 0..400 {
                world.update_sand();
            }
            assert_eq!(world.count(), before + added);
            let settled = world.terrain.cells.clone();
            for _ in 0..100 {
                world.update_sand();
            }
            assert_eq!(world.terrain.cells, settled);
        }
        assert_eq!(a.terrain.cells, b.terrain.cells);
    }

    #[test]
    fn sand_respects_body_coverage_and_survives_rotation() {
        let mut world = World::demo(100, 90, 1);
        world.rasterize();
        // Body centre is (50, 33), upper edge is y=25.5.
        world.terrain.cells[24 * 100 + 50] = Material::Sand;
        world.update_sand();
        assert_eq!(world.terrain.cells[24 * 100 + 50], Material::Sand);
        assert_eq!(world.add_sand([50.0, 33.0], 2.0), 0);
        let count = world.count();
        for _ in 0..500 {
            world.step(1.0 / 120.0);
            world.rasterize();
            world.update_sand();
        }
        assert_eq!(world.count(), count);
    }

    #[test]
    fn sand_stays_inside_world_edges() {
        let mut world = World::demo(4, 30, 0);
        world.terrain.cells.fill(Material::Empty);
        world.terrain.cells[0] = Material::Sand;
        world.terrain.cells[3] = Material::Sand;
        world.rasterize();
        for _ in 0..100 {
            world.update_sand();
        }
        assert_eq!(world.count(), 2);
        assert_eq!(world.terrain.cells[29 * 4], Material::Sand);
        assert_eq!(world.terrain.cells[29 * 4 + 3], Material::Sand);
    }

    #[test]
    fn compact_material() {
        assert_eq!(std::mem::size_of::<Material>(), 1);
    }

    #[test]
    fn coverage_never_changes_mass() {
        let mut world = World::demo(160, 90, 8);
        let initial = world.count();
        for _ in 0..300 {
            world.step(1.0 / 60.0);
            world.rasterize();
        }
        assert_eq!(initial, world.count());
    }

    #[test]
    fn cutting_splits_and_preserves_positions() {
        let mut grid = Grid::new(5, 1);
        grid.cells.fill(Material::Wood);
        grid.cells[2] = Material::Empty;
        let body = Body {
            grid,
            position: [30.0, 40.0],
            angle: 0.7,
            angular_velocity: 0.2,
        };
        let expected = [body.world_point(0.5, 0.5), body.world_point(3.5, 0.5)];
        let fragments = body.split();
        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments.iter().map(|b| b.grid.count()).sum::<usize>(), 4);
        for (fragment, expected) in fragments.iter().zip(expected) {
            let actual = fragment.world_point(0.5, 0.5);
            assert!((actual[0] - expected[0]).abs() < 0.0001);
            assert!((actual[1] - expected[1]).abs() < 0.0001);
        }
    }

    #[test]
    fn raster_matches_full_grid_reference() {
        let mut world = World::demo(80, 60, 1);
        for position in [[40.0, 30.0], [-3.0, 4.0], [79.0, 59.0]] {
            for angle in [0.0, 0.3, 1.57, 2.8] {
                world.bodies[0].position = position;
                world.bodies[0].angle = angle;
                world.rasterize();
                let body = &world.bodies[0];
                let (s, c) = angle.sin_cos();
                for y in 0..60 {
                    for x in 0..80 {
                        let dx = x as f32 + 0.5 - position[0];
                        let dy = y as f32 + 0.5 - position[1];
                        let lx = (c * dx + s * dy + 30.5).floor() as i32;
                        let ly = (-s * dx + c * dy + 7.5).floor() as i32;
                        let occupied = (0..61).contains(&lx)
                            && (0..15).contains(&ly)
                            && body.grid.cells[ly as usize * 61 + lx as usize] != Material::Empty;
                        assert_eq!(world.coverage[y * 80 + x] != 0, occupied);
                    }
                }
            }
        }
    }

    #[test]
    fn deletion_is_accounted_for() {
        let mut world = World::demo(160, 90, 1);
        let initial = world.count();
        let removed = world.erase(world.bodies[0].position, 9.0);
        assert!(removed > 0);
        assert_eq!(world.bodies.len(), 2);
        assert_eq!(world.count() + removed, initial);
    }
}
