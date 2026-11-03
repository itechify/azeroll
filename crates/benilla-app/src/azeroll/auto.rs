//! `WOW_AZEROLL_AUTO=<seconds>`: a hands-off run for testing. Once the world is up it starts the
//! level, plays it along an authored route, logs the run every two seconds, and exits after
//! `<seconds>` of play (or at the results screen). `WOW_AZEROLL_SHOTS=<dir>` also saves a
//! screenshot every `WOW_AZEROLL_SHOT_EVERY` seconds (default 5); it needs no window focus.
//!
//! The player is a fair one, not a good one. It follows the level's progression
//! (`WOW_AZEROLL_AUTO_ROUTE=primary|tour|free`, default `primary`): clear the area it is in, walk
//! the authored trail to the next area (eating what it passes), and move on when the area has
//! nothing worth taking. `primary` is the shortest trail walk from the start to the camp, `tour`
//! adds every other open area first, `free` is the old value-for-distance hunt with no route.
//! Whatever it cannot reach it gives up on, and does not try again until it has grown.
//!
//! Evidence: `WOW_AZEROLL_TELEMETRY=<path>` writes one CSV row per second of play (time, radius,
//! count, position, area, speed, frame-time percentiles over the last ten seconds, trophies,
//! carried NPCs, nearest edible, stuck/unstuck/escape counts, the route step, events) and, beside
//! it as `<path>.summary.txt`, the run's summary: time to each tier, to Hogger edible and to
//! Hogger, frame-time percentiles overall and per tier, the longest plateau, volume by area.
//! `WOW_AZEROLL_REPORT=<path>` is the level's volume report (see `design.rs`).
//!
//! Finale and replay testing: `WOW_AZEROLL_AUTO_FREE=<seconds>` presses Free Roll on a winning
//! results screen and plays that long untimed before exiting; `WOW_AZEROLL_AUTO_TITLE=shot|free`
//! waits on the title for two seconds, saves `title.png`, then begins (`free`: as a Free Roll, when
//! Hogger has been rolled up before). `WOW_AZEROLL_DEBUG_RADIUS` is in `progress.rs`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use super::ball::top_speed;
use super::collect::{contact, Contact, Shed};
use super::design::{self, RouteKind, Step};
use super::progress::{self, Flow, Progress};
use super::{units, Attached, BallState, Collectible, Kind, Phase, Rng, Run};
use crate::loading_screen::LoadingScreen;

/// Seconds between looks at the whole level for a target. Steering uses the cached target every
/// frame; the scan is the autopilot's own cost, kept out of the frame times it is measuring.
const SCAN_EVERY: f32 = 0.12;
/// How far off a trail's line a thing may sit and still be worth a detour, yards.
const CORRIDOR: f32 = 10.0;
/// The longest the player lingers in one area, seconds, however much is left.
const DWELL_CAP: f32 = 75.0;
/// Seconds between checks that an area is still paying, and the growth it must show.
const STALL_EVERY: f32 = 20.0;
const STALL_GROWTH: f32 = 1.015;
/// A chased creature is given up after this many seconds, however it zigzags.
const PREY_CHASE: f32 = 8.0;
/// Unreachable clusters are written off as discs, up to this radius, yards.
const DEAD_CAP: f32 = 14.0;
/// An area is done when what is left in it is worth less than this fraction of the ball's volume.
const CRUMBS: f32 = 0.03;
/// A target that has come no closer for this many seconds (plus half the radius) is given up.
const NO_PROGRESS: f32 = 3.5;
/// A skipped thing is tried again once the ball is this much bigger than when it was skipped.
const RETRY_GROWTH: f32 = 1.4;
/// Seconds of rolling window for the frame-time percentiles in each row.
const WINDOW: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Primary,
    Tour,
    Free,
}

/// Recipe telemetry must count an elevated food when actual ramps provide a route at
/// its pickup size. Flood the same support patches used by movement; never grant
/// a blanket height exemption.
pub(super) fn ramp_reachable(
    world: &benilla_world::collision::WorldCollision,
    design: &design::Design,
    target: Vec3,
    r: f32,
    size: f32,
    solids: &[(Transform, Collectible, bool)],
) -> bool {
    if units::flat(target - design.leash_centre()).length() > design.leash_radius() {
        return false;
    }
    let blocks: Vec<_> = solids
        .iter()
        .filter(|(_, c, _)| c.solid > 0. && contact(r, c.pickup) != Contact::RollUp)
        .flat_map(|(tf, c, gate)| {
            let boxes = if super::furniture::shaped(c) {
                super::furniture::world_boxes(tf, c, r)
            } else {
                vec![(
                    tf.transform_point(c.center),
                    c.half_extents * tf.scale.abs(),
                )]
            };
            boxes
                .into_iter()
                .enumerate()
                .map(|(part, (centre, half))| Blocker {
                    centre,
                    half,
                    rotation: tf.rotation,
                    radius: c.solid,
                    walkable: part == 0
                        || super::furniture::shape(&c.name) != super::furniture::SolidShape::Legs,
                    boxed: super::collect::boxed_solid(c, *gate)
                        || matches!(c.source, super::Source::WmoProp(_)),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // The progression report is an estimate, not a full path search per food item.
    // Flood touching support patches; the live autopilot still runs collision-aware A*.
    let mut nodes = Vec::new();
    for (owner, b) in blocks.iter().enumerate().filter(|(_, b)| {
        b.walkable && b.boxed && units::flat(b.centre - target).length() < b.half.length() + 12.
    }) {
        for axis in 0..3 {
            let mut local_normal = Vec3::ZERO;
            local_normal[axis] = 1.;
            let normal = b.rotation * local_normal;
            if normal.y.abs() <= 0.6 {
                continue;
            }
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for x in [-0.95, 0., 0.95] {
                for z in [-0.95, 0., 0.95] {
                    let mut local = Vec3::ZERO;
                    local[axis] = b.half[axis] * normal.y.signum();
                    local[u] = x * b.half[u];
                    local[v] = z * b.half[v];
                    let foot = b.centre + b.rotation * local;
                    let centre = foot + Vec3::Y * r;
                    let Some(rest) = b.support(centre, r) else {
                        continue;
                    };
                    let p = Vec3::new(foot.x, rest, foot.z);
                    let origin = foot + Vec3::Y * 0.02;
                    let grounded = world
                        .ray_body(origin, Dir3::NEG_Y, super::ball::step_height(r) + 0.04)
                        .is_some_and(|h| h.normal.y > 0.6);
                    nodes.push((owner, p, grounded));
                }
            }
        }
    }
    let mut queue: std::collections::VecDeque<_> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.2)
        .map(|(i, _)| i)
        .collect();
    while let Some(i) = queue.pop_front() {
        let (owner, p, _) = nodes[i];
        let b = &blocks[owner];
        // Check the target's closest stance on this reachable patch, not just its centre.
        if let Some(rest) =
            super::furniture::box_rest(b.centre, b.half, b.rotation, target + Vec3::Y * r, r)
        {
            if within_reach(r, target.y - rest, size) {
                return true;
            }
        }
        for j in 0..nodes.len() {
            if nodes[j].2 {
                continue;
            }
            let (other, q, _) = nodes[j];
            if other == owner
                || (units::flat(p - q).length() <= 2. * r + 0.2
                    && q.y - p.y <= super::ball::step_height(r) + 0.03)
            {
                nodes[j].2 = true;
                queue.push_back(j);
            }
        }
    }
    false
}

/// What the scan chose.
#[derive(Clone, Copy)]
struct Pick {
    entity: Entity,
    at: Vec3,
    d: f32,
    kind: Kind,
}

/// Whether a thing whose bounding sphere (radius `size`) centres `dy` yards above the ball's
/// centre can be touched by rolling: not on a shelf or a table the ball cannot climb (the ball
/// reaches up to about 1.5 radii above the ground), and not far down a pit.
fn within_reach(radius: f32, dy: f32, size: f32) -> bool {
    dy <= 0.5 * radius + 0.9 * size + 0.15 && -dy <= radius + 0.9 * size + 0.3
}

fn finale_destination_ready(goal: &design::Goal, diameter: f32, city_pieces: usize) -> bool {
    goal.kind == design::GoalKind::Finale
        && diameter >= goal.diameter
        && city_pieces >= goal.city_pieces as usize
}

/// A solid thing the ball cannot take: an authored fence (an oriented box) or a trunk (a disc).
#[derive(Clone)]
struct Blocker {
    centre: Vec3,
    rotation: Quat,
    half: Vec3,
    radius: f32,
    walkable: bool,
    boxed: bool,
}

/// Floor support for the exact same oriented boxes the mover uses.
fn prop_floor<'a>(
    blockers: impl Iterator<Item = &'a Blocker>,
    centre: Vec3,
    r: f32,
) -> Option<f32> {
    blockers
        .filter(|b| {
            b.walkable && b.boxed && units::flat(b.centre - centre).length() <= b.half.length() + r
        })
        .filter_map(|b| b.support(centre, r))
        .max_by(f32::total_cmp)
}

impl Blocker {
    /// Exact fast paths for the existing footprint sampler: reject unreachable
    /// faces, and return its centre sample when a horizontal face contains it.
    fn support(&self, p: Vec3, r: f32) -> Option<f32> {
        let axes = [
            self.rotation * Vec3::X,
            self.rotation * Vec3::Y,
            self.rotation * Vec3::Z,
        ];
        let extent =
            axes[0].abs() * self.half.x + axes[1].abs() * self.half.y + axes[2].abs() * self.half.z;
        if (p.x - self.centre.x).abs() > extent.x + r || (p.z - self.centre.z).abs() > extent.z + r
        {
            return None;
        }
        let origin = p.y - r + super::ball::step_height(r) + 0.05;
        let mut reachable = false;
        for i in 0..3 {
            let n = axes[i] * axes[i].y.signum();
            if n.y <= 0.6 {
                continue;
            }
            let top = self.centre.y + n.y * self.half[i];
            let low = top
                - (0..3)
                    .filter(|j| *j != i)
                    .map(|j| axes[j].y.abs() * self.half[j])
                    .sum::<f32>();
            if low > origin + 1e-5 {
                continue;
            }
            reachable = true;
            if n.x.abs() + n.z.abs() < 1e-6 && top <= origin && origin - top <= 4. + 2. * r {
                let local = self.rotation.inverse() * (Vec3::new(p.x, top, p.z) - self.centre);
                if (0..3).all(|j| j == i || local[j].abs() <= self.half[j] + 1e-5) {
                    return Some(top + r);
                }
            }
        }
        reachable
            .then(|| super::furniture::box_rest(self.centre, self.half, self.rotation, p, r))
            .flatten()
    }
}

impl Blocker {
    /// Whether the flat segment `a`..`b`, swept by a ball of `margin` yards, runs through it.
    fn crosses(&self, a: Vec3, b: Vec3, margin: f32) -> bool {
        if self.boxed {
            // Slab test in the box's own frame, ground plane only.
            let la = self.rotation.inverse() * (a - self.centre);
            let lb = self.rotation.inverse() * (b - self.centre);
            let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
            for (p, q, h) in [(la.x, lb.x, self.half.x), (la.z, lb.z, self.half.z)] {
                let (h, d) = (h + margin, q - p);
                if d.abs() < 1e-6 {
                    if p.abs() > h {
                        return false;
                    }
                } else {
                    let (t0, t1) = ((-h - p) / d, (h - p) / d);
                    lo = lo.max(t0.min(t1));
                    hi = hi.min(t0.max(t1));
                    if lo > hi {
                        return false;
                    }
                }
            }
            true
        } else {
            let (off, along) = design::segment_offset(self.centre, a, b);
            let len = units::flat(b - a).length();
            off < self.radius + margin && along > 0.0 && along < len
        }
    }
}

impl Blocker {
    /// Whether a ball of `margin` yards at `p` would be inside it.
    fn contains(&self, p: Vec3, margin: f32) -> bool {
        if self.boxed {
            let l = self.rotation.inverse() * (p - self.centre);
            l.x.abs() <= self.half.x + margin && l.z.abs() <= self.half.z + margin
        } else {
            units::flat(p - self.centre).length() < self.radius + margin
        }
    }

    /// Growth can overlap a leg's safety margin. Permit a move that reduces that
    /// overlap, so the planner can walk back out instead of declaring every edge closed.
    fn leaving(&self, a: Vec3, b: Vec3, margin: f32) -> bool {
        if !self.contains(a, margin) {
            return false;
        }
        let penetration = |p: Vec3| {
            if self.boxed {
                let l = self.rotation.inverse() * (p - self.centre);
                (self.half.x + margin - l.x.abs()).min(self.half.z + margin - l.z.abs())
            } else {
                self.radius + margin - units::flat(p - self.centre).length()
            }
        };
        penetration(b) < penetration(a) - 0.001
    }

    /// A circle that holds all of it, for rasterising.
    fn reach(&self) -> f32 {
        if self.boxed {
            self.half.x.hypot(self.half.z)
        } else {
            self.radius
        }
    }
}

/// The next point to head for to get from `from` to `to` round whatever is in the way, found by
/// A* over a flat grid with the blockers rasterised (a ball of `radius` kept clear of them).
/// `None` when the straight line is clear, or no way round exists.
fn steer_round(blockers: &[Blocker], from: Vec3, to: Vec3, radius: f32) -> Option<Vec3> {
    let margin = 0.9 * radius;
    if !blockers.iter().any(|b| b.crosses(from, to, margin)) {
        return None;
    }
    let lo = Vec2::new(from.x.min(to.x), from.z.min(to.z)) - Vec2::splat(12.0);
    let hi = Vec2::new(from.x.max(to.x), from.z.max(to.z)) + Vec2::splat(12.0);
    let extent = hi - lo;
    let cell = (0.8 * radius).max(0.5).max(extent.max_element() / 160.0);
    let (w, h) = (
        (extent.x / cell).ceil() as usize + 1,
        (extent.y / cell).ceil() as usize + 1,
    );
    let at = |x: usize, y: usize| Vec3::new(lo.x + x as f32 * cell, from.y, lo.y + y as f32 * cell);
    let index = |p: Vec3| {
        let c = (Vec2::new(p.x, p.z) - lo) / cell;
        (
            (c.x.round().max(0.0) as usize).min(w - 1),
            (c.y.round().max(0.0) as usize).min(h - 1),
        )
    };
    let mut blocked = vec![false; w * h];
    for b in blockers {
        let r = b.reach() + margin + cell;
        let (x0, y0) = index(b.centre - Vec3::new(r, 0.0, r));
        let (x1, y1) = index(b.centre + Vec3::new(r, 0.0, r));
        for y in y0..=y1 {
            for x in x0..=x1 {
                if b.contains(at(x, y), margin) {
                    blocked[y * w + x] = true;
                }
            }
        }
    }
    let (start, goal) = (index(from), index(to));
    // Where the ball stands, and where it is going, are open whatever the margins say.
    for (cx, cy) in [start, goal] {
        for y in cy.saturating_sub(1)..=(cy + 1).min(h - 1) {
            for x in cx.saturating_sub(1)..=(cx + 1).min(w - 1) {
                blocked[y * w + x] = false;
            }
        }
    }
    let heuristic = |x: usize, y: usize| {
        let (dx, dy) = (x.abs_diff(goal.0) as f32, y.abs_diff(goal.1) as f32);
        dx.max(dy) + 0.414 * dx.min(dy)
    };
    let key = |f: f32| std::cmp::Reverse((f * 1000.0) as u32);
    let mut best = vec![f32::INFINITY; w * h];
    let mut came = vec![usize::MAX; w * h];
    let mut open = std::collections::BinaryHeap::new();
    best[start.1 * w + start.0] = 0.0;
    open.push((key(heuristic(start.0, start.1)), start.0, start.1));
    while let Some((_, x, y)) = open.pop() {
        if (x, y) == goal {
            break;
        }
        let g = best[y * w + x];
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if blocked[ny * w + nx] {
                continue;
            }
            let step = if dx != 0 && dy != 0 { 1.414 } else { 1.0 };
            if g + step < best[ny * w + nx] {
                best[ny * w + nx] = g + step;
                came[ny * w + nx] = y * w + x;
                open.push((key(g + step + heuristic(nx, ny)), nx, ny));
            }
        }
    }
    if !best[goal.1 * w + goal.0].is_finite() {
        return None;
    }
    let mut path = vec![to];
    let mut i = goal.1 * w + goal.0;
    while i != start.1 * w + start.0 {
        i = came[i];
        path.push(at(i % w, i / w));
    }
    path.reverse();
    // Head for the furthest point along it that can be seen without touching a blocker.
    path.iter()
        .skip(1)
        .rev()
        .find(|p| !blockers.iter().any(|b| b.crosses(from, **p, margin)))
        .or_else(|| path.get(1))
        .copied()
}

/// A cached walk through real WMO floors; only the debug autopilot uses this planner.
struct IndoorNav {
    entity: Entity,
    at: Vec3,
    radius: f32,
    path: Vec<Vec3>,
    next: usize,
    blockers: Vec<Blocker>,
}
impl IndoorNav {
    fn advance(&mut self, from: Vec3, tolerance: f32, radius: f32) {
        // Full-radius spherical-cap support can hold the centre above a tread's
        // centre sample. Node arrival needs XZ proximity plus one-step height
        // tolerance, rather than an exact 3D point the mover cannot occupy.
        while self.next + 1 < self.path.len()
            && units::flat(self.path[self.next] - from).length() < tolerance
            && (self.path[self.next].y - from.y).abs() < super::ball::step_height(radius) + 0.05
        {
            self.next += 1;
        }
    }
    fn remaining(&self, from: Vec3) -> f32 {
        let tail = &self.path[self.next.min(self.path.len() - 1)..];
        tail[0].distance(from) + tail.windows(2).map(|p| p[1].distance(p[0])).sum::<f32>()
    }
}

fn indoor_edge(
    world: &benilla_world::collision::WorldCollision,
    shape: &avian3d::prelude::Collider,
    radius: f32,
    a: Vec3,
    b: Vec3,
    blockers: impl Iterator<Item = usize>,
    all: &[Blocker],
) -> bool {
    let step = super::ball::step_height(radius);
    if b.y - a.y > step + 0.045 || a.y - b.y > 4.0 + 2.0 * radius {
        return false;
    }
    let y = a.y.max(b.y) + radius;
    let from = Vec3::new(a.x, y, a.z);
    let to = Vec3::new(b.x, y, b.z);
    for i in blockers {
        let block = &all[i];
        if block.boxed {
            if block.walkable && block.support(to, radius).is_some_and(|h| to.y >= h - 0.003) {
                continue;
            }
            let local = block.rotation.inverse() * (from - block.centre);
            if local.y.abs() >= block.half.y + radius {
                continue;
            }
        }
        if block.crosses(from, to, radius) && !block.leaving(from, to, radius) {
            return false;
        }
    }
    if a.y > b.y + 0.02 {
        // A clear horizontal sweep above a stairwell lip does not prove the
        // destination's descending stance has head room below that lip.
        let down = Vec3::Y * (b.y - a.y);
        if world.cast_body(shape, to, down, 0.02).is_some() {
            return false;
        }
    }
    if world.cast_body(shape, from, to - from, 0.02).is_none() {
        return true;
    }
    // The mover only steps when support ahead actually rises. A speculative
    // lift on a flat/descending edge can jump a stair-side wall that the live
    // ball will never climb, leaving it driving forever into that wall.
    if b.y <= a.y + 0.003 {
        return false;
    }
    // A sphere touches a riser before its centre reaches the upper tread.
    let lift = Vec3::Y * step;
    world.cast_body(shape, from, lift, 0.02).is_none()
        && world
            .cast_body(shape, from + lift, to - from, 0.02)
            .is_none()
}

/// A* carries floor height as part of its state: a lower floor must not poison the same XZ
/// cell's raised landing. Each edge uses the mover's reachable step and a real body sweep.
fn indoor_path(
    world: &benilla_world::collision::WorldCollision,
    design: &design::Design,
    ball: &BallState,
    target: Vec3,
    size: f32,
    blockers: &[Blocker],
) -> Option<Vec<Vec3>> {
    indoor_path_to(world, design, ball, target, size, blockers, None)
}

fn indoor_path_to(
    world: &benilla_world::collision::WorldCollision,
    design: &design::Design,
    ball: &BallState,
    target: Vec3,
    size: f32,
    blockers: &[Blocker],
    arrival: Option<f32>,
) -> Option<Vec<Vec3>> {
    let started = bevy::platform::time::Instant::now();
    indoor_path_search(world, design, ball, target, size, blockers, arrival, || {
        started.elapsed().as_secs_f32() > 1.0
    })
    .inspect_err(|reason| debug!("Azeroll indoor route search ended: {reason:?}"))
    .ok()
}

#[derive(Debug, PartialEq)]
enum IndoorPathError {
    Deadline,
    NodeLimit,
    NoRoute,
}

/// Keep geometry/work bounds independent of scheduling. The live caller supplies
/// a wall-clock cancellation check; behavioral tests can exercise the same search
/// and node cap without failing merely because another test occupied the CPU.
fn indoor_path_search(
    world: &benilla_world::collision::WorldCollision,
    design: &design::Design,
    ball: &BallState,
    target: Vec3,
    size: f32,
    blockers: &[Blocker],
    arrival: Option<f32>,
    mut expired: impl FnMut() -> bool,
) -> Result<Vec<Vec3>, IndoorPathError> {
    use avian3d::prelude::Collider;
    let radius = ball.radius;
    let cell = (radius * 0.55).clamp(0.22, (radius * 0.35).clamp(0.6, 8.0));
    let cell = if design.start_indoors && arrival.is_some() {
        cell.max(0.4)
    } else {
        cell
    };
    let padding = (radius * 1.25).clamp(12., 80.);
    let lo = ball.pos.xz().min(target.xz()) - Vec2::splat(padding);
    let hi = ball.pos.xz().max(target.xz()) + Vec2::splat(padding);
    let w = ((hi.x - lo.x) / cell).ceil() as usize + 1;
    let h = ((hi.y - lo.y) / cell).ceil() as usize + 1;
    let index = |p: Vec3| {
        let p = (p.xz() - lo) / cell;
        (
            (p.x.round().max(0.0) as usize).min(w - 1),
            (p.y.round().max(0.0) as usize).min(h - 1),
        )
    };
    let at =
        |x: usize, z: usize, y: f32| Vec3::new(lo.x + x as f32 * cell, y, lo.y + z as f32 * cell);
    let mut cover = vec![Vec::<usize>::new(); w * h];
    for (i, block) in blockers.iter().enumerate() {
        let reach = block.reach() + radius + cell;
        let (x0, z0) = index(block.centre - Vec3::splat(reach));
        let (x1, z1) = index(block.centre + Vec3::splat(reach));
        for z in z0..=z1 {
            for x in x0..=x1 {
                cover[z * w + x].push(i);
            }
        }
    }
    struct Node {
        pos: Vec3,
        g: f32,
        parent: Option<usize>,
    }
    let start = Vec3::new(ball.pos.x, ball.pos.y - radius, ball.pos.z);
    let (sx, sz) = index(start);
    let key = |x: usize, z: usize, y: f32| (z * w + x, (y * 10.0).round() as i32);
    let mut nodes = vec![Node {
        pos: start,
        g: 0.0,
        parent: None,
    }];
    let mut best = HashMap::<(usize, i32), usize>::default();
    best.insert(key(sx, sz, start.y), 0_usize);
    let mut floors = HashMap::<(usize, i32), Option<f32>>::default();
    let score = |p: Vec3, g: f32| {
        std::cmp::Reverse(((g + units::flat(p - target).length()) * 1000.0) as u32)
    };
    let mut open = std::collections::BinaryHeap::new();
    open.push((score(start, 0.0), 0_usize));
    // Match the mover's D6/D10 horizontal sphere in every mode. A full-radius
    // WMO-street planning sphere rejects gates that the player's 0.85r body fits.
    let shape = Collider::sphere(0.85 * radius);
    let mut expanded = 0;
    while let Some((_, ni)) = open.pop() {
        expanded += 1;
        if expanded > if design.start_indoors { 16000 } else { 6000 } {
            return Err(IndoorPathError::NodeLimit);
        }
        if expanded % 64 == 0 && expired() {
            return Err(IndoorPathError::Deadline);
        }
        let p = nodes[ni].pos;
        let touch = radius + 0.9 * size;
        let dy = target.y - (p.y + radius);
        let reach = arrival.unwrap_or(touch);
        let height = arrival.map_or(touch, |_| 0.25 + 0.1 * radius);
        if units::flat(p - target).length() < reach + cell && dy.abs() < height {
            let stand = arrival.map_or_else(|| (touch * touch - dy * dy).sqrt(), |r| r) * 0.85;
            let away = units::flat(p - target).normalize_or(Vec3::X);
            let end = Vec3::new(target.x, p.y, target.z) + away * stand;
            if indoor_edge(world, &shape, radius, p, end, 0..blockers.len(), blockers) {
                let mut path = vec![end + Vec3::Y * radius];
                let mut current = ni;
                loop {
                    path.push(nodes[current].pos + Vec3::Y * radius);
                    let Some(parent) = nodes[current].parent else {
                        break;
                    };
                    current = parent;
                }
                path.reverse();
                return Ok(path);
            }
        }
        let (x, z) = index(p);
        for (dx, dz) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, nz) = (x as i32 + dx, z as i32 + dz);
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let (nx, nz) = (nx as usize, nz as usize);
            let horizontal = at(nx, nz, p.y);
            if units::flat(horizontal - design.leash_centre()).length()
                > design.leash_radius() - radius
            {
                continue;
            }
            let fk = key(nx, nz, p.y);
            let floor = *floors.entry(fk).or_insert_with(|| {
                let origin = horizontal + Vec3::Y * (super::ball::step_height(radius) + 0.04);
                let engine = if design.start_indoors {
                    // A centre ray misses the spherical-cap lift beside a
                    // tread edge. Plan the same full-radius stance the mover
                    // will occupy, including head clearance under stair lips.
                    super::ball::floor_rest(world, horizontal + Vec3::Y * radius, radius)
                        .map(|h| h - radius)
                } else {
                    world
                        .ray_body(
                            origin,
                            Dir3::NEG_Y,
                            1.5 + radius
                                + if radius > 8. {
                                    super::ball::step_height(radius)
                                } else {
                                    0.
                                },
                        )
                        .filter(|hit| hit.normal.y > 0.6)
                        .map(|hit| origin.y - hit.distance)
                };
                prop_floor(
                    cover[nz * w + nx].iter().map(|i| &blockers[*i]),
                    horizontal + Vec3::Y * radius,
                    radius,
                )
                .map(|h| h - radius)
                .into_iter()
                .chain(engine)
                .max_by(f32::total_cmp)
            });
            let Some(floor) = floor else { continue };
            let floor_band =
                12.0 + (radius - 8.0).max(0.0) + if design.wmo_floors { 8.0 } else { 0. };
            if (floor - start.y).abs() > floor_band {
                continue;
            }
            let q = at(nx, nz, floor);
            if design.start_indoors
                && arrival.is_some()
                && p.y - q.y > super::ball::step_height(radius) + 0.1
            {
                // Room connections follow stair treads, rather than treating a
                // lower storey glimpsed through a hatch as a safe shortcut.
                continue;
            }
            let g = nodes[ni].g + units::flat(q - p).length() + 0.2 * (q.y - p.y).abs();
            let nk = key(nx, nz, floor);
            if best.get(&nk).is_some_and(|&i| nodes[i].g <= g) {
                continue;
            }
            if !indoor_edge(
                world,
                &shape,
                radius,
                p,
                q,
                cover[nz * w + nx].iter().copied(),
                blockers,
            ) {
                continue;
            }
            let next = nodes.len();
            nodes.push(Node {
                pos: q,
                g,
                parent: Some(ni),
            });
            best.insert(nk, next);
            open.push((score(q, g), next));
        }
    }
    Err(IndoorPathError::NoRoute)
}

fn indoor_blockers(
    things: &Loose,
    gates: &Query<(), With<design::Gate>>,
    radius: f32,
) -> Vec<Blocker> {
    things
        .iter()
        .filter(|(_, c, _)| c.solid > 0.0 && contact(radius, c.pickup) != Contact::RollUp)
        .flat_map(|(e, c, tf)| {
            if super::furniture::shaped(c) {
                super::furniture::world_boxes(tf, c, radius)
                    .into_iter()
                    .enumerate()
                    .map(|(part, (centre, half))| Blocker {
                        centre,
                        half,
                        rotation: tf.rotation,
                        radius: c.solid,
                        walkable: part == 0,
                        boxed: true,
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![Blocker {
                    centre: tf.transform_point(c.center),
                    half: c.half_extents * tf.scale.abs(),
                    rotation: tf.rotation,
                    radius: c.solid,
                    walkable: true,
                    boxed: super::collect::boxed_solid(c, gates.contains(e)),
                }]
            }
        })
        .collect()
}

/// What the last scan of the level saw.
#[derive(Default)]
struct Scan {
    pick: Option<Pick>,
    total: usize,
    edible: usize,
    /// Flat distance to the nearest thing the ball can roll up, in sight or not.
    nearest: f32,
    /// The volume of the food that was candidates for this step's region.
    region_vol: f32,
    /// Edible volume per area, for the hunt after the route is done.
    by_area: Vec<f32>,
    /// The nearest creature too big to eat, within sight: what to run from when rammed.
    threat: Option<Vec3>,
    /// Fences and trunks near the ball, for steering round them.
    blockers: Vec<Blocker>,
}

#[derive(Resource)]
pub(super) struct Auto {
    /// Seconds of play before the run exits.
    seconds: f32,
    shots: Option<std::path::PathBuf>,
    shot_every: f32,
    shot_t: f32,
    shot_n: u32,
    log_t: f32,
    /// Wander heading and seconds until it changes.
    wander: Vec3,
    wander_t: f32,
    /// Seconds spent in the results screen.
    over_t: f32,
    /// Seconds the ball has barely moved while driven; past a threshold it backs off and wanders.
    stuck_t: f32,
    last_pos: Option<Vec3>,
    /// Seconds left of an escape, which overrides targets.
    unstick_t: f32,
    frames: u32,
    /// Seconds of Free Roll to play after the results (`WOW_AZEROLL_AUTO_FREE`), and played so far.
    free: Option<f32>,
    free_t: f32,
    free_started: bool,
    /// Celebration shots taken (0.4 s and 1.8 s in).
    finale_shots: u8,
    /// What to do on the title (`WOW_AZEROLL_AUTO_TITLE`), and seconds spent there.
    title: Option<String>,
    title_t: f32,
    /// Things given up on, and the ball's radius when they were.
    skip: HashMap<Entity, f32>,
    /// Where an escape heads when there is no route: the next open area in the cycle.
    waypoint: Option<Vec3>,
    tour: usize,
    census: bool,
    /// Real seconds of play so far, ticking through the finale's slow motion.
    play_t: f32,
    mode: Mode,
    steps: Vec<Step>,
    /// The route has been laid out for the level now loaded.
    planned: bool,
    step: usize,
    /// Seconds in this step, and the ball's volume when it began.
    step_t: f32,
    step_volume: f32,
    /// The route is walked; hunt what is left.
    hunt: bool,
    scan_t: f32,
    scan: Scan,
    /// Seconds with nothing to pick in this step's region.
    idle_t: f32,
    /// The target held, the closest it has been, and seconds without getting closer.
    chase: Option<(Entity, f32, f32, f32)>,
    escape_kind: usize,
    last_escape: f32,
    /// Discs of ground where targets could not be reached: centre, radius, and the ball's radius
    /// when written off.
    dead: DeadZones,
    /// Seconds into the step and the volume at the last check that the area is still paying.
    stall: (f32, f32),
    /// Rams taken as of the last scan, and seconds left of running from whatever is ramming.
    rams: u32,
    flee_t: f32,
    census_t: f32,
    /// Where the ball was when it last moved a yard (or ate), when, and the count then.
    stuck_anchor: (Vec3, f32, u32),
    /// The way round a fence to the last point asked about, when the straight line is blocked.
    detour: Option<Vec3>,
    detour_to: Vec3,
    detour_t: f32,
    indoor: bool,
    nav: Option<IndoorNav>,
    nav_retry: f32,
    street_nav: Option<IndoorNav>,
    street_retry: f32,
    #[cfg(not(target_arch = "wasm32"))]
    gate_audited: bool,
    tel: Telemetry,
}

pub(super) fn from_env() -> Option<Auto> {
    let seconds = crate::webenv::var("WOW_AZEROLL_AUTO")?
        .parse()
        .unwrap_or(120.0);
    let shots = crate::webenv::var("WOW_AZEROLL_SHOTS").map(std::path::PathBuf::from);
    if let Some(dir) = &shots {
        let _ = std::fs::create_dir_all(dir);
    }
    let shot_every = crate::webenv::var("WOW_AZEROLL_SHOT_EVERY")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5.0);
    let mode = match crate::webenv::var("WOW_AZEROLL_AUTO_ROUTE").as_deref() {
        Some("tour") => Mode::Tour,
        Some("free") => Mode::Free,
        _ => Mode::Primary,
    };
    Some(Auto {
        seconds,
        shots,
        shot_every,
        shot_t: 0.0,
        shot_n: 0,
        log_t: 0.0,
        wander: Vec3::NEG_Z,
        wander_t: 0.0,
        over_t: 0.0,
        stuck_t: 0.0,
        last_pos: None,
        unstick_t: 0.0,
        frames: 0,
        free: crate::webenv::var("WOW_AZEROLL_AUTO_FREE").and_then(|s| s.parse().ok()),
        free_t: 0.0,
        free_started: false,
        finale_shots: 0,
        title: crate::webenv::var("WOW_AZEROLL_AUTO_TITLE"),
        title_t: 0.0,
        skip: Default::default(),
        waypoint: None,
        tour: 0,
        census: false,
        play_t: 0.0,
        mode,
        steps: Vec::new(),
        planned: false,
        step: 0,
        step_t: 0.0,
        step_volume: 0.0,
        hunt: mode == Mode::Free,
        scan_t: 0.0,
        scan: Scan::default(),
        idle_t: 0.0,
        chase: None,
        escape_kind: 0,
        last_escape: f32::NEG_INFINITY,
        dead: DeadZones::default(),
        stall: (0.0, 0.0),
        rams: 0,
        flee_t: 0.0,
        census_t: 0.0,
        stuck_anchor: (Vec3::ZERO, 0.0, 0),
        detour: None,
        detour_to: Vec3::ZERO,
        detour_t: 0.0,
        indoor: false,
        nav: None,
        nav_retry: 0.0,
        street_nav: None,
        street_retry: 0.0,
        #[cfg(not(target_arch = "wasm32"))]
        gate_audited: false,
        tel: Telemetry::from_env(),
    })
}

pub(super) fn plugin(app: &mut App) {
    if let Some(auto) = from_env() {
        app.insert_resource(auto)
            .add_systems(
                Update,
                drive
                    .run_if(super::finale::mobile)
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    .before(super::ball::drive_ball),
            )
            .add_systems(OnEnter(Phase::Ready), |mut auto: ResMut<Auto>| {
                auto.planned = false
            });
    }
}

/// Lay the route out for the level now loaded (a restart reloads the recipe, so it is redone).
fn plan(auto: &mut Auto, design: &design::Design) {
    auto.planned = true;
    auto.indoor = design.start_indoors;
    auto.nav = None;
    auto.nav_retry = 0.0;
    auto.street_nav = None;
    auto.street_retry = 0.0;
    // A size goal with no area to reach has no camp to walk to: tour the level instead.
    let walk_to_goal =
        design.goal.kind == design::GoalKind::Collect || design.goal_area().is_some();
    auto.steps = match auto.mode {
        Mode::Free => Vec::new(),
        Mode::Primary if walk_to_goal => design::plan_route(design, RouteKind::Primary),
        Mode::Primary | Mode::Tour => design::plan_route(design, RouteKind::Tour),
    };
    auto.tel.set_tiers(design);
    auto.step = 0;
    auto.step_t = 0.0;
    auto.step_volume = 0.0;
    auto.hunt = auto.steps.is_empty();
    auto.skip.clear();
    auto.dead.0.clear();
    auto.chase = None;
    auto.scan_t = 0.0;
    let labels: Vec<_> = auto.steps.iter().map(|s| s.label(design)).collect();
    info!(
        "azeroll auto: mode={:?} route: {}",
        auto.mode,
        labels.join(" > ")
    );
}

/// Everything on the ground the ball could roll up, and where.
type Loose<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Collectible, &'static Transform),
    (
        Without<Attached>,
        Without<Shed>,
        Without<super::replay::HumanOnly>,
    ),
>;

/// Where the current step wants the food to be.
enum Region {
    Everywhere,
    Area {
        centre: Vec3,
        radius: f32,
        stacked: bool,
    },
    Trail {
        from: Vec3,
        to: Vec3,
    },
}

impl Region {
    fn of(auto: &Auto, design: &design::Design) -> Self {
        if auto.hunt {
            return Self::Everywhere;
        }
        match auto.steps.get(auto.step) {
            Some(&Step::Area(i)) => Self::Area {
                centre: units::wow_to_bevy(design.areas[i].centre),
                radius: design.areas[i].radius + if design.start_indoors { 0. } else { 6. },
                stacked: design.start_indoors || design.areas[i].enclosed,
            },
            Some(&Step::Trail { index, reversed }) => {
                let (from, to) = design::trail_ends(design, index, reversed);
                Self::Trail { from, to }
            }
            None => Self::Everywhere,
        }
    }

    /// A raised rim's room must not lure the ball back into the lower pit.
    /// Elevated tables remain eligible; compare the target's bottom, not centre.
    fn holds_floor(&self, tf: &Transform, c: &Collectible) -> bool {
        let Self::Area {
            centre,
            stacked: true,
            ..
        } = self
        else {
            return true;
        };
        let half = c.half_extents * tf.scale.abs();
        let height = (tf.rotation * Vec3::X).y.abs() * half.x
            + (tf.rotation * Vec3::Y).y.abs() * half.y
            + (tf.rotation * Vec3::Z).y.abs() * half.z;
        tf.transform_point(c.center).y - height >= centre.y - 0.2
    }

    /// Whether food at `at` is this step's business, given where the ball is.
    fn holds(&self, at: Vec3, ball: Vec3, radius: f32) -> bool {
        match *self {
            Self::Everywhere => units::flat(at - ball).length() < 80.0 + radius * 4.0,
            Self::Area {
                centre,
                radius,
                stacked,
            } => {
                units::flat(at - centre).length() <= radius
                    && (!stacked || (at.y - centre.y).abs() < 3.)
            }
            Self::Trail { from, to } => {
                let (off, along) = design::segment_offset(at, from, to);
                let (_, here) = design::segment_offset(ball, from, to);
                off <= CORRIDOR + radius
                    && along >= here - 4.0
                    && along <= units::flat(to - from).length() + 6.0
            }
        }
    }

    /// The point a step with nothing to take heads for.
    fn goal(&self) -> Option<Vec3> {
        match *self {
            Self::Everywhere => None,
            Self::Area { centre, .. } => Some(centre),
            Self::Trail { to, .. } => Some(to),
        }
    }
}

/// One look over the whole level from the ball: what it can eat, and which of it to go for.
fn look(
    auto: &mut Auto,
    ball: &BallState,
    design: &design::Design,
    things: &Loose,
    collision: &benilla_world::collision::WorldCollision,
    gates: &Query<(), With<design::Gate>>,
    can: impl Fn(f32) -> bool,
) -> Scan {
    let region = Region::of(auto, design);
    let mut scan = Scan {
        nearest: f32::INFINITY,
        by_area: vec![0.0; design.areas.len()],
        ..Default::default()
    };
    let masks: Vec<_> = things
        .iter()
        .filter(|(_, c, _)| !c.pickup.is_finite() && c.solid > 0.)
        .map(|(_, c, t)| (t, c))
        .collect();
    let covered = |t: &Transform, c: &Collectible| {
        masks
            .iter()
            .any(|(bt, bc)| super::collect::hidden_by_blocker(t, c, bt, bc))
    };
    let mut best_score = 0.0_f32;
    let mut threat_d = 14.0_f32;
    // Things in the way that the ball cannot take. The world's collision rays do not see them
    // (they are collectibles), so a fence between the ball and its supper has to be found here.
    let blockers: Vec<Blocker> = things
        .iter()
        .filter(|(_, c, t)| {
            c.solid > 0.0
                && !can(c.pickup)
                && (!matches!(c.source, super::Source::WmoProp(_))
                    || contact(ball.radius, c.pickup) == Contact::Wall)
                && units::flat(t.translation - ball.pos).length() < 60.0 + c.pickup
        })
        .flat_map(|(e, c, t)| {
            if super::furniture::shaped(c) {
                super::furniture::world_boxes(t, c, ball.radius)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (center, half))| {
                        (t.rotation.inverse() * (ball.pos - *center)).y.abs() < half.y + ball.radius
                    })
                    .map(|(part, (centre, half))| Blocker {
                        centre,
                        half,
                        rotation: t.rotation,
                        radius: c.solid,
                        walkable: part == 0,
                        boxed: true,
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![Blocker {
                    centre: t.transform_point(c.center),
                    rotation: t.rotation,
                    half: (c.half_extents * t.scale.abs()).max(Vec3::splat(0.05)),
                    radius: c.solid,
                    walkable: true,
                    boxed: gates.contains(e)
                        || (design.wmo_floors && super::collect::boxed_solid(c, false)),
                }]
            }
        })
        .collect();
    // Indoor navigation-only trails are connections to another storey, not
    // forage corridors. Detouring into nearby pit food defeats the room tour.
    if design.start_indoors
        && !auto.hunt
        && auto.steps.get(auto.step).is_some_and(
            |s| matches!(s,Step::Trail{index,..} if design.trails[*index].navigation_only),
        )
    {
        scan.blockers = blockers;
        return scan;
    }
    for (e, c, t) in things {
        if c.pickup.is_finite() && covered(t, c) {
            continue;
        }
        scan.total += 1;
        if !can(c.pickup) {
            if c.kind != Kind::Prop {
                let at = t.transform_point(c.center);
                let d = units::flat(at - ball.pos).length();
                if d < threat_d {
                    threat_d = d;
                    scan.threat = Some(at);
                }
            }
            continue;
        }
        scan.edible += 1;
        // At city scale a barrel under a spanning roof contributes almost nothing. Keep
        // the driver interested in the trees, wagons and structures that change its size.
        if design.wmo_floors && ball.radius >= 12. && c.pickup < 0.3 * ball.radius {
            continue;
        }
        let at = t.transform_point(c.center);
        let delta = at - ball.pos;
        let d = units::flat(delta).length();
        scan.nearest = scan.nearest.min(d);
        if auto
            .skip
            .get(&e)
            .is_some_and(|r| ball.radius < r * RETRY_GROWTH)
        {
            continue;
        }
        if let Some(i) = design::area_at(design, at) {
            scan.by_area[i] += c.volume(t.scale);
        }
        // Reachable by rolling: not on a shelf or a table the ball cannot climb yet, and not in a
        // patch of ground where earlier targets proved unreachable.
        if !within_reach(ball.radius, at.y - ball.pos.y, c.size)
            || !region.holds(at, ball.pos, ball.radius)
            || !region.holds_floor(t, c)
            || auto.dead.contains(at, ball.radius)
        {
            continue;
        }
        if auto.indoor {
            // At miniature scale a centre ray sees through the gap beside a chair leg or
            // door jamb even when the ball cannot fit. Prefer a genuinely clear local trip;
            // outdoor routes retain their long-distance fence detours.
            if blockers
                .iter()
                .any(|b| b.crosses(ball.pos, at, ball.radius))
            {
                continue;
            }
            let flat = units::flat(delta);
            if let Ok(dir) = Dir3::new(flat) {
                let side = Vec3::new(-dir.z, 0.0, dir.x) * (0.9 * ball.radius);
                let length = flat.length();
                if [-1.0, 0.0, 1.0].into_iter().any(|offset| {
                    [-0.5, 0.3, 0.7].into_iter().any(|height| {
                        let origin = ball.pos + side * offset + Vec3::Y * (height * ball.radius);
                        collision.ray_body(origin, dir, length).is_some_and(|hit| {
                            hit.normal.y.abs() < 0.6 && hit.distance < length - 0.15
                        })
                    })
                }) {
                    continue;
                }
            }
        }
        scan.region_vol += c.volume(t.scale);
        // Best value for the trip: growth falls off as (s/r)^2.5, so a bigger thing a little
        // further away beats a crumb underfoot.
        let mut score = (c.pickup / ball.radius).powf(2.5) / (d + 2.0 + ball.radius);
        // D27: prefer eligible city structures throughout the authored reclamation.
        // Zero-volume proxies are near-face planner targets;
        // real OBB contact, support validation and retirement still decide the pickup.
        if design.goal.city_pieces > 0 && c.size == 0.0 && c.half_extents.max_element() > 0.0 {
            score *= 64.0;
        }
        // The terminal district remains the food destination after the road tour.
        // Otherwise a huge ball prefers distant Elwynn trees to the remaining grove
        // beside its objective, and unnecessarily attempts to leave through the gate.
        if design.goal.city_pieces > 0 && ball.radius >= 12.0 {
            if let Some(area) = design.goal_area() {
                let centre = units::wow_to_bevy(area.centre);
                if units::flat(at - centre).length() <= area.radius + 120.0 {
                    score *= 16.0;
                }
            }
        }
        // A straight line through a fence or a trunk is a long way round: prefer open ground.
        if blockers
            .iter()
            .any(|b| b.crosses(ball.pos, at, 0.8 * ball.radius))
        {
            score *= 0.15;
        }
        // Prey runs faster than the ball rolls: worth it only when it is close.
        if c.kind == Kind::Creature && d > 3.0 + 4.0 * ball.radius {
            score *= 0.25;
        }
        if score <= best_score {
            continue;
        }
        // Choose visible ground pickups rather than repeatedly steering through WMO walls.
        if let Ok(dir) = Dir3::new(delta) {
            if collision
                .ray_body(ball.pos, dir, delta.length())
                .is_some_and(|h| h.normal.y.abs() < 0.6 && h.distance < delta.length() - 0.5)
            {
                continue;
            }
        }
        best_score = score;
        scan.pick = Some(Pick {
            entity: e,
            at,
            d,
            kind: c.kind,
        });
    }
    if auto.indoor {
        auto.nav_retry = (auto.nav_retry - SCAN_EVERY).max(0.0);
        let keep = auto.nav.as_ref().is_some_and(|nav| {
            (ball.radius / nav.radius - 1.0).abs() < 0.1
                && things.get(nav.entity).is_ok_and(|(_, c, tf)| {
                    !design.wmo_floors
                        || ball.radius < 12.
                        || (c.pickup >= 0.3 * ball.radius
                            && within_reach(
                                ball.radius,
                                tf.transform_point(c.center).y - ball.pos.y,
                                c.size,
                            ))
                })
        });
        if !keep {
            auto.nav = None;
        }
        if let Some(nav) = &auto.nav {
            if let Ok((entity, c, tf)) = things.get(nav.entity) {
                scan.pick = Some(Pick {
                    entity,
                    at: tf.transform_point(c.center),
                    d: nav.remaining(ball.pos),
                    kind: c.kind,
                });
            }
        } else if scan.pick.is_none() && auto.nav_retry <= 0.0 {
            auto.nav_retry = 0.6;
            let mut candidates: Vec<_> = things
                .iter()
                .filter_map(|(e, c, tf)| {
                    if c.kind != Kind::Prop
                        || covered(tf, c)
                        || !can(c.pickup)
                        || auto
                            .skip
                            .get(&e)
                            .is_some_and(|r| ball.radius < r * RETRY_GROWTH)
                    {
                        return None;
                    }
                    let at = tf.transform_point(c.center);
                    if design.wmo_floors
                        && (!region.holds(at, ball.pos, ball.radius) || !region.holds_floor(tf, c))
                    {
                        return None;
                    }
                    if design.wmo_floors
                        && ball.radius >= 12.
                        && (c.pickup < 0.3 * ball.radius
                            || !within_reach(ball.radius, at.y - ball.pos.y, c.size))
                    {
                        return None;
                    }
                    let proxy = c.size == 0.0;
                    let origin = at + Vec3::Y * if proxy { ball.radius + 2. } else { 0.05 };
                    let hit = collision.ray_body(
                        origin,
                        Dir3::NEG_Y,
                        if proxy { ball.radius * 2. + 8. } else { 3.0 },
                    )?;
                    if hit.normal.y <= 0.6 {
                        return None;
                    }
                    let engine_floor = origin.y - hit.distance;
                    let top = things
                        .iter()
                        .filter_map(|(e, c, tf)| {
                            super::collect::solid_rest(
                                tf,
                                c,
                                at + Vec3::Y * ball.radius,
                                ball.radius,
                                gates.contains(e),
                            )
                        })
                        .map(|h| h - ball.radius)
                        .max_by(f32::total_cmp);
                    let floor = top.map_or(engine_floor, |h| h.max(engine_floor));
                    let rise = (floor - (ball.pos.y - ball.radius)).abs();
                    if rise > 12.0 + (ball.radius - 8.).max(0.)
                        || !within_reach(ball.radius, at.y - (floor + ball.radius), c.size)
                    {
                        return None;
                    }
                    let d = units::flat(at - ball.pos).length();
                    let reach = if design.wmo_floors {
                        45.0_f32.max(2. * ball.radius)
                    } else {
                        45.
                    };
                    if d > reach {
                        return None;
                    }
                    let score = (c.pickup / ball.radius).powf(2.5)
                        / ((d + 2.0 + ball.radius) * (1.0 + 4.0 * rise));
                    Some((e, c, at, score))
                })
                .collect();
            candidates.sort_by(|a, b| b.3.total_cmp(&a.3));
            let full = indoor_blockers(things, gates, ball.radius);
            for (entity, c, at, _) in candidates.into_iter().take(3) {
                if let Some(path) = indoor_path(collision, design, ball, at, c.size, &full) {
                    info!(
                        "azeroll indoor route: {} nodes={} r={:.2}",
                        c.name,
                        path.len(),
                        ball.radius
                    );
                    scan.pick = Some(Pick {
                        entity,
                        at,
                        d: units::flat(at - ball.pos).length(),
                        kind: c.kind,
                    });
                    auto.nav = Some(IndoorNav {
                        entity,
                        at,
                        radius: ball.radius,
                        path,
                        next: 0,
                        blockers: full,
                    });
                    break;
                }
                // A closed room or shelf stays closed until growth changes the route.
                auto.skip.insert(entity, ball.radius);
            }
        }
    }
    scan.blockers = blockers;
    scan
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn drive(
    mut commands: Commands,
    mut auto: ResMut<Auto>,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    loading: Res<LoadingScreen>,
    mut ball: ResMut<BallState>,
    run: Res<Run>,
    (mut flow, progress, current, partitions, ritual, mut wishes): (
        ResMut<Flow>,
        Res<Progress>,
        Res<super::campaign::CurrentLevel>,
        Res<benilla_world::terrain_stream::WmoPartitions>,
        Res<super::ritual::Ritual>,
        MessageWriter<super::ritual::Wish>,
    ),
    design: Res<super::design::Design>,
    collision: benilla_world::collision::WorldCollision,

    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut rng: ResMut<Rng>,
    things: Loose,
    (attached, carried, gates): (
        Query<(), With<Attached>>,
        Query<(), (With<Attached>, With<super::npcs::Npc>)>,
        Query<(), With<design::Gate>>,
    ),
    mut exit: MessageWriter<AppExit>,
) {
    let _profile = super::profile::span("auto::drive");
    let dt = time.delta_secs();
    if loading.covering() {
        return;
    }
    match phase.get() {
        // The autopilot plays a level; it has no business in the hub.
        Phase::Select => return,
        Phase::Ready => {
            if let Some(mode) = auto.title.clone() {
                auto.title_t += dt;
                if auto.title_t < 2.0 {
                    return;
                }
                if auto.title_t - dt < 2.0 {
                    shoot(&mut commands, &mut auto, "title");
                    return;
                }
                if auto.title_t < 2.6 {
                    return;
                }
                if mode == "free" && progress.wins(current.id()) > 0 {
                    progress::start_free_roll(&mut flow, &mut next);
                    return;
                }
            }
            // An indoor start waits for its floor to load.
            if ball.floor_ready {
                next.set(Phase::Playing);
            }
            return;
        }
        Phase::Paused => return,
        Phase::Results => {
            // The send-off plays first (3.4 s); the results clock starts when the panel docks, so
            // the screenshot sees the verdict and the exit stays a few seconds after it.
            if !ritual.docked {
                return;
            }
            auto.over_t += dt;
            if auto.over_t > 0.5 && auto.over_t - dt <= 0.5 {
                shoot(&mut commands, &mut auto, "results");
            }
            if auto.free.is_some() && !auto.free_started && run.won && auto.over_t > 2.0 {
                auto.free_started = true;
                info!("azeroll auto: starting Free Roll from the results");
                wishes.write(super::ritual::Wish::Free);
                return;
            }
            if auto.over_t > 3.0 {
                info!(
                    "azeroll auto: results won={} t={:.1} r={:.2}, exiting",
                    run.won, run.elapsed, ball.radius
                );
                auto.tel.leftovers = leftovers(&things, &ball, &design, &collision, &gates);
                finish(&mut auto, &ball, &run, &design, "results screen");
                exit.write(AppExit::Success);
            }
            return;
        }
        Phase::Playing => {}
    }
    if !auto.planned {
        plan(&mut auto, &design);
    }
    auto.play_t += dt;
    #[cfg(not(target_arch = "wasm32"))]
    if !auto.gate_audited
        && run.elapsed > 2.
        && crate::webenv::var("WOW_AZEROLL_GATE_AUDIT").is_some()
    {
        auto.gate_audited = true;
        for r in [2., 2.5, 3.2, 4., 4.5, 5., 6.5] {
            let start = units::wow_to_bevy([-8962.938, 514.393, 96.357]) + Vec3::Y * r;
            let goal = units::wow_to_bevy([-8896.417, 567.307, 93.057]) + Vec3::Y * r;
            let probe = BallState {
                pos: start,
                radius: r,
                volume: r * r * r,
                ..default()
            };
            let blocks = indoor_blockers(&things, &gates, r);
            let path = indoor_path(&collision, &design, &probe, goal, 0., &blocks);
            info!(
                "azeroll gate audit: r={r} path={:?}",
                path.map(|p| p.into_iter().map(units::bevy_to_wow).collect::<Vec<_>>())
            );
        }
    }

    let play_t = auto.play_t;
    let diameter_tier = design
        .tiers
        .iter()
        .take_while(|t| ball.diameter() >= t.at)
        .count();
    // A creature goal is ready once the ball can roll it up; a size goal, once it is that big.
    let hogger_edible = match design.goal_axis() {
        Some(axis) => contact(ball.radius, axis) == Contact::RollUp,
        None => ball.diameter() >= design.goal.diameter,
    };
    let finale_ready = !flow.free_roll
        && finale_destination_ready(
            &design.goal,
            ball.diameter(),
            partitions.removed_count(0, 10047),
        );
    let boss_ready = (hogger_edible && design.goal_axis().is_some()) || finale_ready;
    auto.tel.frame(
        play_t,
        dt * 1000.0,
        diameter_tier,
        run.elapsed,
        hogger_edible,
        run.won,
    );

    if !auto.census && run.elapsed > 6.0 {
        auto.census = true;
        // The level's size budget: things per maximum-axis band, and the volume they credit.
        let edges = [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, f32::INFINITY];
        let mut bins = [(0usize, 0.0_f32); 8];
        for (_, c, t) in &things {
            let i = edges.iter().position(|e| c.pickup < *e).unwrap_or(7);
            bins[i].0 += 1;
            bins[i].1 += c.volume(t.scale);
        }
        let mut text = String::new();
        let mut lo = 0.0;
        for (i, (n, v)) in bins.iter().enumerate() {
            text += &format!(" [{lo}-{}): n={n} vol={v:.1}", edges[i]);
            lo = edges[i];
        }
        info!("azeroll census:{text}");
    }

    // A teleport is the game's own recovery (unstuck.rs) at work: the ball covered far more
    // ground in one frame than its top speed allows.
    if let Some(prev) = auto.last_pos {
        let jump = units::flat(ball.pos - prev).length();
        if jump > 4.0 && jump > 4.0 * (top_speed(ball.radius) * dt + 0.5) {
            auto.tel.unstuck += 1;
            let note = format!(
                "unstuck {:.0}yd at t={:.1} ({}) r={:.2}",
                jump,
                run.elapsed,
                design::area_at(&design, ball.pos).map_or("-", |i| &design.areas[i].name),
                ball.radius
            );
            auto.tel.event(note.clone());
            auto.tel.unstuck_log.push(note);
            auto.stuck_t = 0.0;
            auto.chase = None;
        }
    }

    // Look at the level a few times a second, not every frame; between looks, steer at the
    // target's current position.
    let can = |pickup: f32| contact(ball.radius, pickup) == Contact::RollUp;
    auto.indoor = design.start_indoors
        || design
            .collision_navigation
            .iter()
            .any(|zone| zone.contains(ball.pos));
    auto.scan_t -= dt;
    let lost = auto
        .scan
        .pick
        .is_some_and(|p| things.get(p.entity).is_err());
    if auto.scan_t <= 0.0 || lost {
        auto.scan_t = SCAN_EVERY;
        let scan = look(&mut auto, &ball, &design, &things, &collision, &gates, can);
        auto.scan = scan;
        auto.tel.nearest = auto.scan.nearest;
        auto.tel.edible = auto.scan.edible;
        auto.tel.total = auto.scan.total;
        auto.idle_t = if auto.scan.pick.is_none() {
            auto.idle_t + SCAN_EVERY
        } else {
            0.0
        };
        advance(&mut auto, &ball, &run, &design);
    }
    // The boss overrides value-for-distance, including the route and the skip list.
    let boss = boss_ready
        .then(|| {
            things
                .iter()
                .find(|(_, c, _)| {
                    c.kind == Kind::Boss && c.name == design.goal.target.as_str() && can(c.pickup)
                })
                .map(|(e, c, t)| {
                    let at = t.transform_point(c.center);
                    Pick {
                        entity: e,
                        at,
                        d: units::flat(at - ball.pos).length(),
                        kind: c.kind,
                    }
                })
        })
        .flatten();
    let mut pick = boss.or_else(|| {
        auto.scan.pick.and_then(|p| {
            things.get(p.entity).ok().map(|(_, c, t)| {
                let at = t.transform_point(c.center);
                Pick {
                    entity: p.entity,
                    at,
                    d: units::flat(at - ball.pos).length(),
                    kind: c.kind,
                }
            })
        })
    });
    if finale_ready {
        pick = None;
        auto.nav = None;
        auto.street_nav = None;
        auto.chase = None;
    }
    if let (Some(p), Some(nav)) = (pick.as_mut(), auto.nav.as_ref()) {
        if p.entity == nav.entity {
            p.d = nav.remaining(ball.pos);
        }
    }
    // Hold a target only while it is getting closer: unreachable ones (on a shelf, behind a
    // wall) are given up on early and, with their neighbours, not tried again until the ball
    // has grown.
    if let Some(p) = pick {
        auto.chase = match auto.chase {
            Some((e, best, since, total)) if e == p.entity => {
                if p.d < best - 0.75 {
                    Some((e, p.d, 0.0, total + dt))
                } else {
                    Some((e, best, since + dt, total + dt))
                }
            }
            _ => Some((p.entity, p.d, 0.0, 0.0)),
        };
        if let Some((e, _, since, total)) = auto.chase {
            let cap = if p.kind == Kind::Creature {
                PREY_CHASE
            } else {
                20.0 + 2.0 * ball.radius
            };
            // Tight interiors need an earlier change of target than the three-second
            // stuck detector; a failed shelf/door approach is ordinary navigation.
            let patience = if auto.indoor { 1.2 } else { NO_PROGRESS };
            let hopeless = since > patience + 0.5 * ball.radius || total > cap;
            if hopeless && boss.is_none() {
                give_up(&mut auto, e, p.at, &ball, &things);
                auto.tel.event(format!("skip {}", name_of(&things, e)));
                auto.chase = None;
                pick = None;
            }
        }
    } else {
        auto.chase = None;
    }

    // Wedged in a doorway or against a wall: give up the target and head somewhere else.
    // Judged over a few seconds, not a frame: pushed against a fence the ball jitters in place
    // fast enough to look like it is moving.
    let (anchor, since, count) = auto.stuck_anchor;
    if auto.unstick_t > 0.0 || run.count != count || units::flat(ball.pos - anchor).length() > 1.2 {
        auto.stuck_anchor = (ball.pos, play_t, run.count);
        auto.stuck_t = 0.0;
    } else {
        auto.stuck_t = play_t - since;
    }
    auto.last_pos = Some(ball.pos);
    let goal = Region::of(&auto, &design).goal();
    if auto.stuck_t > 3.0 {
        let blockers: Vec<_> =
            things
                .iter()
                .filter_map(|(_, c, t)| {
                    let d = units::flat(t.transform_point(c.center) - ball.pos).length();
                    (!can(c.pickup) && c.solid > 0.0 && d < c.solid + ball.radius + 1.0)
                        .then_some((c.name.to_string(), d, c.solid))
                })
                .collect();
        info!(
            "azeroll auto: escape at {:?}, blockers={blockers:?}",
            ball.pos
        );
        auto.tel.stuck += 1;
        auto.tel.escapes += 1;
        auto.tel.event(format!(
            "stuck at {} r={:.2} blockers={}",
            design::area_at(&design, ball.pos).map_or("-", |i| &design.areas[i].name),
            ball.radius,
            blockers.len()
        ));
        auto.stuck_t = 0.0;
        auto.street_nav = None;
        auto.street_retry = 0.;
        auto.nav = None;
        auto.unstick_t = 5.0;
        auto.wander_t = 0.0;
        auto.escape_kind = if auto.play_t - auto.last_escape > 25.0 {
            0
        } else {
            auto.escape_kind + 1
        };
        auto.last_escape = auto.play_t;
        let open: Vec<_> = design.areas.iter().filter(|a| !a.enclosed).collect();
        if !open.is_empty() {
            auto.tour = (auto.tour + 1) % open.len();
            auto.waypoint = Some(units::wow_to_bevy(open[auto.tour].centre));
        }
        if let Some((e, ..)) = auto.chase.take() {
            let at = things
                .get(e)
                .map_or(ball.pos, |(_, c, t)| t.transform_point(c.center));
            give_up(&mut auto, e, at, &ball, &things);
        }
        pick = None;
    }
    auto.unstick_t = (auto.unstick_t - dt).max(0.0);
    auto.step_t += dt;

    // Rammed: run from whatever is too big to eat for a couple of seconds, a player's reflex.
    if flow.rams > auto.rams {
        auto.flee_t = 2.5;
        auto.tel.event("rammed".into());
    }
    auto.rams = flow.rams;
    auto.flee_t = (auto.flee_t - dt).max(0.0);
    let fleeing = (auto.flee_t > 0.0 && boss.is_none())
        .then_some(auto.scan.threat)
        .flatten();
    let mut mode = String::new();
    let dir = if finale_ready {
        mode.push_str("finale destination");
        units::flat(design.goal_point().unwrap_or(ball.pos) - ball.pos).normalize_or(auto.wander)
    } else if auto.unstick_t > 0.0 {
        mode.push_str("escape");
        // Cycle through ways out: toward the goal, either side of it, and back the way it came.
        let to_goal = match goal.or(auto.waypoint) {
            Some(g) => {
                let aim = auto.aim(&ball, g, dt, &collision);
                units::flat(aim - ball.pos).normalize_or(auto.wander)
            }
            None => auto.wander,
        };
        let turn = match auto.escape_kind % 4 {
            0 => 0.0,
            1 => std::f32::consts::FRAC_PI_2,
            2 => -std::f32::consts::FRAC_PI_2,
            _ => std::f32::consts::PI,
        };
        Quat::from_rotation_y(turn) * to_goal
    } else if let Some(threat) = fleeing {
        mode.push_str("flee");
        units::flat(ball.pos - threat).normalize_or(auto.wander)
    } else if let Some(p) = pick {
        mode.push_str(if boss.is_some() { "boss" } else { "target" });
        let aim = auto.aim(&ball, p.at, dt, &collision);
        if aim != p.at {
            mode.push_str("+round");
        }
        units::flat(aim - ball.pos).normalize_or(auto.wander)
    } else if let (true, Some(camp)) = (boss_ready, design.goal_point()) {
        mode.push_str("camp");
        let aim = auto.aim(&ball, camp, dt, &collision);
        units::flat(aim - ball.pos).normalize_or(auto.wander)
    } else if let Some(g) = goal.filter(|_| !auto.hunt) {
        mode.push_str("route");
        let aim = auto.aim(&ball, g, dt, &collision);
        if aim != g {
            mode.push_str("+round");
        }
        units::flat(aim - ball.pos).normalize_or(auto.wander)
    } else {
        // The route is done, or there is none: go where the most is still edible, else drift.
        let richest = auto
            .scan
            .by_area
            .iter()
            .enumerate()
            .filter(|(_, v)| **v > CRUMBS * ball.volume)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| units::wow_to_bevy(design.areas[i].centre));
        if let Some(c) = richest {
            mode.push_str("hunt");
            let aim = auto.aim(&ball, c, dt, &collision);
            units::flat(aim - ball.pos).normalize_or(auto.wander)
        } else {
            mode.push_str("wander");
            auto.wander_t -= dt;
            if auto.wander_t <= 0.0 {
                let a = rng.range(0.0, std::f32::consts::TAU);
                auto.wander = Vec3::new(a.sin(), 0.0, a.cos());
                auto.wander_t = 6.0;
            }
            // Drift back toward the start when far out.
            let home = units::flat(design.leash_centre() - ball.pos);
            if home.length() > design.leash_radius() * 0.6 {
                home.normalize()
            } else {
                auto.wander
            }
        }
    };
    let street_goal = (design.wmo_floors
        && auto.indoor
        && pick.is_none()
        && auto.unstick_t <= 0.
        && !finale_ready)
        .then(|| goal.or_else(|| design.goal_point()))
        .flatten();
    let street_aim = street_goal.map(|goal| auto.street_aim(&ball, &design, goal, dt, &collision));
    let dir = if let Some(aim) = street_aim {
        units::flat(aim - ball.pos).normalize_or(dir)
    } else if auto.indoor && pick.is_none() && !finale_ready {
        // Continuous obstacle avoidance also covers the no-pickup fallback. Never blindly
        // steer at an area's centre through a low counter: feel ahead at belly/shoulder
        // height and follow the free side until the direct bearing opens again.
        // Hold the free bearing between scans. Re-attracting the driver to a blocked
        // area centre every frame makes it circle the same bar-front corner forever.
        // Authored outdoor WMO streets still have a destination. Reuse the interior
        // wall feelers, but attract them to that destination rather than retaining a
        // bearing from the gate after the market has been cleared.
        let city_street = design
            .collision_navigation
            .iter()
            .any(|zone| zone.contains(ball.pos));
        let dir = if city_street { dir } else { auto.wander };
        let reach = (2.0 * ball.radius + 0.5).max(1.0);
        let clear = |dir: Vec3| {
            let to = ball.pos + dir * reach;
            let margin = 0.9 * ball.radius;
            if auto.scan.blockers.iter().any(|b| {
                b.crosses(ball.pos, to, margin)
                    && !(b.contains(ball.pos, margin) && !b.contains(to, margin))
            }) {
                return false;
            }
            let side = Vec3::new(-dir.z, 0.0, dir.x) * margin;
            [-1.0, 0.0, 1.0].into_iter().all(|offset| {
                [-0.5, 0.3, 0.7].into_iter().all(|height| {
                    let from = ball.pos + side * offset + Vec3::Y * (height * ball.radius);
                    !collision
                        .ray_body(from, Dir3::new(dir).unwrap(), reach)
                        .is_some_and(|h| h.normal.y.abs() < 0.6)
                })
            })
        };
        let free = (0..=12)
            .map(|i| i as f32 * std::f32::consts::PI / 12.0)
            .chain((1..12).map(|i| -(i as f32) * std::f32::consts::PI / 12.0))
            .map(|angle| Quat::from_rotation_y(angle) * dir)
            .find(|candidate| clear(*candidate))
            .unwrap_or(-dir);
        auto.wander = free;
        free
    } else {
        dir
    };
    // Forward for the ball is `(-sin yaw, 0, -cos yaw)`.
    ball.heading = (-dir.x).atan2(-dir.z);
    keys.press(KeyCode::KeyW);

    auto.frames += 1;
    auto.tel.frames_row += 1;
    auto.log_t -= dt;
    if auto.log_t <= 0.0 {
        let fps = auto.frames as f32 / (2.0 - auto.log_t);
        auto.frames = 0;
        auto.log_t = 2.0;
        let wow = units::bevy_to_wow(ball.pos);
        info!(
            "azeroll auto: fps={:.0} t={:.1} pos=({:.1},{:.1},{:.1}) r={:.2} speed={:.1} count={} trophies={} \
             collectibles={} edible={} target={:?} biggest={:?} mode={mode}",
            fps,
            run.elapsed,
            wow[0],
            wow[1],
            wow[2],
            ball.radius,
            units::flat(ball.vel).length(),
            run.count,
            attached.iter().count(),
            auto.scan.total,
            auto.scan.edible,
            pick.map(|p| p.d),
            run.biggest,
        );
    }

    // A minute-by-minute look at what is edible and still uneaten, for the log and the summary.
    auto.census_t -= dt;
    if auto.census_t <= 0.0 {
        auto.census_t = 60.0;
        auto.tel.leftovers = leftovers(&things, &ball, &design, &collision, &gates);
        for l in auto.tel.leftovers.lines() {
            info!(
                "azeroll auto census t={:.0} r={:.2}: {l}",
                run.elapsed, ball.radius
            );
        }
    }

    // One telemetry row a second of play.
    if auto.play_t >= auto.tel.next_row {
        let step = if auto.hunt {
            "hunt".to_string()
        } else {
            auto.steps
                .get(auto.step)
                .map_or("-".to_string(), |s| s.label(&design))
        };
        let area =
            design::area_at(&design, ball.pos).map_or("-", |i| design.areas[i].name.as_str());
        let phase_name = if flow.finale.is_some() {
            "finale"
        } else if flow.free_roll {
            "free"
        } else {
            "play"
        };
        let target = pick.map_or("-".to_string(), |p| {
            format!("{}@{:.0}", name_of(&things, p.entity), p.d)
        });
        let sample = Sample {
            t: run.elapsed,
            wall: auto.play_t,
            phase: phase_name,
            mode: &mode,
            step: &step,
            area,
            radius: ball.radius,
            count: run.count,
            trophies: attached.iter().count(),
            npcs: carried.iter().count(),
            pos: units::bevy_to_wow(ball.pos),
            speed: units::flat(ball.vel).length(),
            volume: ball.volume,
            rams: flow.rams,
            target: &target,
        };
        auto.tel.row(&sample);
    }

    auto.shot_t -= dt;
    if auto.shot_t <= 0.0 {
        auto.shot_t = auto.shot_every;
        let name = format!("play-{:03}", auto.shot_n);
        shoot(&mut commands, &mut auto, &name);
    }

    if let Some(t) = flow.finale {
        let due = if t > 1.8 {
            2
        } else if t > 0.4 {
            1
        } else {
            0
        };
        while auto.finale_shots < due {
            auto.finale_shots += 1;
            let name = format!("finale-{}", auto.finale_shots);
            shoot(&mut commands, &mut auto, &name);
        }
    }
    if flow.free_roll {
        auto.free_t += dt;
        if auto.free_t > 3.0 && auto.free_t - dt <= 3.0 {
            shoot(&mut commands, &mut auto, "freeroll");
        }
        if auto.free_t >= auto.free.unwrap_or(10.0) {
            info!(
                "azeroll auto: free roll done, r={:.2} count={} won={}, exiting",
                ball.radius, run.count, run.won
            );
            auto.tel.leftovers = leftovers(&things, &ball, &design, &collision, &gates);
            finish(&mut auto, &ball, &run, &design, "free roll done");
            exit.write(AppExit::Success);
        }
    } else if run.elapsed >= auto.seconds {
        info!(
            "azeroll auto: {:.0}s done, r={:.2} count={}, exiting",
            run.elapsed, ball.radius, run.count
        );
        auto.tel.leftovers = leftovers(&things, &ball, &design, &collision, &gates);
        finish(&mut auto, &ball, &run, &design, "time up");
        exit.write(AppExit::Success);
    }
}

fn name_of(things: &Loose, e: Entity) -> String {
    things
        .get(e)
        .map_or("?".to_string(), |(_, c, _)| c.name.to_string())
}

/// Stop going for `e`, and for whatever sits within a yard or two of it (the same shelf, the
/// same wall), until the ball has grown.
fn give_up(auto: &mut Auto, e: Entity, at: Vec3, ball: &BallState, things: &Loose) {
    auto.skip.insert(e, ball.radius);
    if auto.indoor {
        auto.nav = None;
        return;
    }
    for (n, c, t) in things {
        if units::flat(t.transform_point(c.center) - at).length() < 1.5 + c.size {
            auto.skip.insert(n, ball.radius);
        }
    }
    auto.dead.write_off(at, ball.radius);
}

impl Auto {
    /// Route waypoints need the same floor/clearance planning as edible props. A ray fan
    /// at city scale otherwise sends the ball around an entire district to avoid a porch
    /// that the mover can step onto. Failed plans keep the authored bearing; physics slides.
    fn street_aim(
        &mut self,
        ball: &BallState,
        design: &design::Design,
        goal: Vec3,
        dt: f32,
        world: &benilla_world::collision::WorldCollision,
    ) -> Vec3 {
        self.street_retry -= dt;
        let keep = self.street_nav.as_ref().is_some_and(|nav| {
            units::flat(nav.at - goal).length() < 0.5 && (nav.radius / ball.radius - 1.).abs() < 0.1
        });
        if !keep {
            self.street_nav = None;
        }
        if self.street_nav.is_none() && self.street_retry <= 0. {
            self.street_retry = 1.5;
            let origin = Vec3::new(
                goal.x,
                goal.y
                    + super::ball::step_height(ball.radius)
                    + if design.start_indoors { 0.05 } else { 1. },
                goal.z,
            );
            if let Some(hit) = world
                .ray_body(origin, Dir3::NEG_Y, 2. * ball.radius + 12.)
                .filter(|h| h.normal.y > 0.6)
            {
                let target = Vec3::new(goal.x, origin.y - hit.distance + ball.radius, goal.z);
                if let Some(path) = indoor_path_to(
                    world,
                    design,
                    ball,
                    target,
                    0.,
                    &self.scan.blockers,
                    design.start_indoors.then_some(1.5),
                ) {
                    self.street_nav = Some(IndoorNav {
                        entity: Entity::PLACEHOLDER,
                        at: goal,
                        radius: ball.radius,
                        path,
                        next: 0,
                        blockers: self.scan.blockers.clone(),
                    });
                }
            }
        }
        if let Some(nav) = &mut self.street_nav {
            nav.advance(ball.pos, (0.4 * ball.radius).max(0.12), ball.radius);
            return nav.path[nav.next];
        }
        self.aim(ball, goal, dt, world)
    }

    /// The point to steer for to reach `to`: `to` itself, or a way round the fences and trunks
    /// between (looked for a few times a second, and again when `to` has moved on).
    fn aim(
        &mut self,
        ball: &BallState,
        to: Vec3,
        dt: f32,
        world: &benilla_world::collision::WorldCollision,
    ) -> Vec3 {
        if let Some(nav) = self.nav.as_mut() {
            if units::flat(nav.at - to).length() < 0.5 {
                nav.advance(ball.pos, (ball.radius * 0.5).max(0.18), ball.radius);
                let shape = avian3d::prelude::Collider::sphere(0.85 * ball.radius);
                let from = ball.pos - Vec3::Y * ball.radius;
                let reach = (3.0 * ball.radius + 1.0).clamp(1.5, 4.0);
                for (i, &p) in nav.path.iter().enumerate().skip(nav.next).rev() {
                    if units::flat(p - ball.pos).length() > reach {
                        continue;
                    }
                    if indoor_edge(
                        world,
                        &shape,
                        ball.radius,
                        from,
                        p - Vec3::Y * ball.radius,
                        0..nav.blockers.len(),
                        &nav.blockers,
                    ) {
                        // The progress metric must start at the waypoint we actually steer
                        // toward. Keeping skipped nodes behind us makes remaining distance
                        // grow during a successful shortcut and falsely abandons the trip.
                        nav.next = i;

                        return p;
                    }
                }

                return nav.path[nav.next];
            }
        }
        self.nav = None;
        self.detour_t -= dt;
        if self.detour_t <= 0.0 || units::flat(to - self.detour_to).length() > 2.0 {
            self.detour = steer_round(&self.scan.blockers, ball.pos, to, ball.radius);
            self.detour_to = to;
            self.detour_t = 0.25;
        }
        self.detour.unwrap_or(to)
    }
}

/// Discs of ground where targets were given up on: centre, radius, and the ball's radius then.
#[derive(Default)]
struct DeadZones(Vec<(Vec3, f32, f32)>);

impl DeadZones {
    /// Whether `at` lies in ground where targets were given up on, and the ball has not grown
    /// enough since to try again.
    fn contains(&self, at: Vec3, radius: f32) -> bool {
        self.0
            .iter()
            .any(|(c, r, then)| radius < then * RETRY_GROWTH && units::flat(at - *c).length() < *r)
    }

    /// Write a failed target's ground off. A failure beside an earlier one widens that disc, so
    /// a room full of unreachable things is written off after a few tries, not one by one.
    fn write_off(&mut self, at: Vec3, radius: f32) {
        for (c, r, then) in self.0.iter_mut() {
            let d = units::flat(at - *c).length();
            if d < *r + 6.0 && radius < *then * RETRY_GROWTH {
                *r = r.max(d + 3.0).min(DEAD_CAP);
                return;
            }
        }
        self.0.push((at, 3.0, radius));
    }
}

fn route_floor(ball: &BallState, feet: Vec3) -> bool {
    (ball.pos.y - ball.radius - feet.y).abs() < 0.35 + 0.1 * ball.radius
}

/// Move to the next step of the route when this one is done: an area when it is cleared (or only
/// crumbs, or the player has lingered), a trail when its end is reached.
fn advance(auto: &mut Auto, ball: &BallState, run: &Run, design: &design::Design) {
    if auto.hunt {
        return;
    }
    let Some(&step) = auto.steps.get(auto.step) else {
        auto.hunt = true;
        return;
    };
    if auto.step_volume == 0.0 {
        auto.step_volume = ball.volume;
        let label = step.label(design);
        info!(
            "azeroll auto: step {}/{} {label} at t={:.1} r={:.2}",
            auto.step + 1,
            auto.steps.len(),
            run.elapsed,
            ball.radius
        );
        auto.stall = (0.0, ball.volume);
        auto.tel
            .steps
            .push((label.clone(), run.elapsed, ball.radius));
        auto.tel.event(format!("step {label}"));
    }
    let (done, why) = match step {
        Step::Area(i) => {
            let a = &design.areas[i];
            // A district-scale ball covers a market before its centre reaches the
            // old street waypoint. Treat that footprint as arriving at the area.
            let footprint = if design.wmo_floors && ball.radius >= 12. {
                ball.radius
            } else {
                0.
            };
            let inside = units::flat(ball.pos - units::wow_to_bevy(a.centre)).length()
                <= a.radius + 6.0 + footprint
                && (!(design.start_indoors || a.enclosed)
                    || route_floor(ball, units::wow_to_bevy(a.centre)));
            if !inside && auto.step_t > DWELL_CAP {
                (true, "approach timeout")
            } else if !inside {
                (false, "")
            } else if design.start_indoors
                && auto.mode == Mode::Tour
                && auto.step == 0
                && auto.steps.len() > 1
                && ball.radius >= 0.3
            {
                // A tiny nursery ball has less step height than the inn's
                // risers. Take starter food, then tour before clearing it all.
                (true, "ready for stairs")
            } else if auto.scan.pick.is_none() && auto.idle_t >= 0.6 {
                (true, "cleared")
            } else if auto.scan.region_vol < CRUMBS * ball.volume && auto.idle_t >= 0.6 {
                (true, "crumbs")
            } else if auto.step_t - auto.stall.0 >= STALL_EVERY {
                if ball.volume < auto.stall.1 * STALL_GROWTH {
                    (true, "stalled")
                } else {
                    auto.stall = (auto.step_t, ball.volume);
                    (false, "")
                }
            } else if auto.step_t > DWELL_CAP {
                (true, "dwell")
            } else {
                (false, "")
            }
        }
        Step::Trail { index, reversed } => {
            let (from, to) = design::trail_ends(design, index, reversed);
            let near = if let Some(arrival) = design.trails[index].arrival_radius {
                units::flat(ball.pos - to).length() < arrival.max(0.95 * ball.radius)
                    && (!design.start_indoors || route_floor(ball, to))
            } else if design.start_indoors {
                units::flat(ball.pos - to).length() < 8.0 + ball.radius && route_floor(ball, to)
            } else {
                units::flat(ball.pos - to).length() < 8.0 + ball.radius
            };
            let cap = units::flat(to - from).length() / 3.0 + 40.0;
            if near {
                (true, "end")
            } else if auto.step_t > cap {
                (true, "timeout")
            } else {
                (false, "")
            }
        }
    };
    if !done {
        return;
    }
    let gained = (ball.volume - auto.step_volume) / auto.step_volume.max(1e-6);
    if matches!(step, Step::Area(_)) && gained < 0.01 {
        let note = format!(
            "starved at {} (r={:.2}, t={:.1}): nothing edible taken",
            step.label(design),
            ball.radius,
            run.elapsed
        );
        info!("azeroll auto: {note}");
        auto.tel.event(note.clone());
        auto.tel.starved.push(note);
    }
    info!(
        "azeroll auto: step done ({why}) {} after {:.0}s, volume +{:.0}%",
        step.label(design),
        auto.step_t,
        gained * 100.0
    );
    auto.step += 1;
    auto.step_t = 0.0;
    auto.step_volume = 0.0;
    auto.idle_t = 0.0;
    auto.chase = None;
    if auto.step >= auto.steps.len() {
        auto.hunt = true;
        auto.tel.event("route done".into());
    }
}

/// What the ball could eat and has not: grouped by area and name, with how high above the ball
/// the group sits (in ball radii) and how near the closest is. The tell for food the level shows
/// and the ball cannot get: on a table, behind a wall, in a room it does not fit.
fn leftovers(
    things: &Loose,
    ball: &BallState,
    design: &design::Design,
    collision: &benilla_world::collision::WorldCollision,
    gates: &Query<(), With<design::Gate>>,
) -> String {
    // Census is telemetry, not movement: use the report's support-patch flood
    // rather than running a full body-swept A* for every food in one frame.
    let solids: Vec<_> = things
        .iter()
        .filter(|(_, c, _)| c.solid > 0.)
        .map(|(e, c, t)| ((*t).clone(), c.clone(), gates.contains(e)))
        .collect();

    let mut groups: BTreeMap<(String, String), (usize, usize, f32)> = BTreeMap::new();
    for (_, c, t) in things {
        if contact(ball.radius, c.pickup) != Contact::RollUp
            || solids
                .iter()
                .any(|(bt, bc, _)| super::collect::hidden_by_blocker(t, c, bt, bc))
        {
            continue;
        }
        let at = t.transform_point(c.center);
        let area = design::area_at(design, at).map_or("-", |i| design.areas[i].name.as_str());
        let g = groups
            .entry((area.to_string(), c.name.to_string()))
            .or_insert((0, 0, f32::INFINITY));
        g.0 += 1;
        let direct = within_reach(ball.radius, at.y - ball.pos.y, c.size);
        let via_ramp = !direct
            && units::flat(at - ball.pos).length() < 25.
            && (at.y - ball.pos.y).abs() < 12.
            && ramp_reachable(collision, design, at, ball.radius, c.size, &solids);
        g.1 += usize::from(direct || via_ramp);
        g.2 = g.2.min(units::flat(at - ball.pos).length());
    }
    let mut v: Vec<_> = groups.into_iter().collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.1 .0));
    v.iter()
        .take(12)
        .map(|((area, name), (n, reach, near))| {
            format!("{area}: {name} x{n} ({reach} within reach directly or by a ramp, nearest {near:.0} yd)")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn shoot(commands: &mut Commands, auto: &mut Auto, name: &str) {
    let Some(dir) = &auto.shots else {
        return;
    };
    auto.shot_n += 1;
    let path = dir.join(format!("{name}.png"));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

// -------------------------------------------------------------------------------------------------
// Telemetry
// -------------------------------------------------------------------------------------------------

/// What one CSV row records about the ball this second.
struct Sample<'a> {
    t: f32,
    wall: f32,
    phase: &'a str,
    mode: &'a str,
    step: &'a str,
    area: &'a str,
    radius: f32,
    count: u32,
    trophies: usize,
    npcs: usize,
    pos: [f32; 3],
    speed: f32,
    volume: f32,
    rams: u32,
    target: &'a str,
}

/// The nearest-rank percentile of an ascending sample.
fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((p / 100.0) * (sorted.len() - 1) as f32).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    v
}

struct Telemetry {
    path: Option<String>,
    out: Option<std::io::BufWriter<std::fs::File>>,
    started: String,
    /// Real seconds of play at which the next row is due.
    next_row: f32,
    last_row: f32,
    frames_row: u32,
    /// Every frame of play: `(real seconds of play, milliseconds, tier index)`.
    samples: Vec<(f32, f32, usize)>,
    /// Index of the first sample in the rolling window.
    window: usize,
    /// Notes since the last row, written in its last column.
    events: Vec<String>,
    stuck: u32,
    unstuck: u32,
    escapes: u32,
    nearest: f32,
    edible: usize,
    total: usize,
    max_trophies: usize,
    max_npcs: usize,
    /// First play time at which the ball reached each tier, and Hogger edible, and the win.
    tier_at: Vec<Option<f32>>,
    /// The recipe's tiers `(diameter, name)`, for the summary.
    tiers: Vec<(f32, String)>,
    /// "hogger" (lower case) for a creature goal, else "goal".
    goal_word: String,
    edible_at: Option<f32>,
    won_at: Option<f32>,
    /// Longest stretch without a pickup, and without 2 % radius growth: `(seconds, from, area, r)`.
    gap: (f32, f32, String, f32),
    plateau: (f32, f32, String, f32),
    last_count: u32,
    last_pickup: f32,
    mark: (f32, f32),
    last_volume: f32,
    /// Volume gained and seconds spent, by area name.
    gained: BTreeMap<String, f32>,
    spent: BTreeMap<String, f32>,
    unstuck_log: Vec<String>,
    starved: Vec<String>,
    steps: Vec<(String, f32, f32)>,
    area_now: String,
    radius_now: f32,
    last_t: f32,
    /// The last census of edible things left uneaten (see [`leftovers`]).
    leftovers: String,
}

impl Telemetry {
    fn from_env() -> Self {
        let path = crate::webenv::var("WOW_AZEROLL_TELEMETRY");
        let out = path.as_ref().and_then(|p| {
            if let Some(dir) = std::path::Path::new(p).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::File::create(p) {
                Ok(f) => Some(std::io::BufWriter::new(f)),
                Err(e) => {
                    warn!("azeroll auto: telemetry {p}: {e}");
                    None
                }
            }
        });
        let mut tel = Self {
            path,
            out,
            started: design::utc_now(),
            next_row: 0.0,
            last_row: 0.0,
            frames_row: 0,
            samples: Vec::new(),
            window: 0,
            events: Vec::new(),
            stuck: 0,
            unstuck: 0,
            escapes: 0,
            nearest: f32::INFINITY,
            edible: 0,
            total: 0,
            max_trophies: 0,
            max_npcs: 0,
            tier_at: Vec::new(),
            tiers: Vec::new(),
            goal_word: "goal".to_string(),
            edible_at: None,
            won_at: None,
            gap: (0.0, 0.0, String::new(), 0.0),
            plateau: (0.0, 0.0, String::new(), 0.0),
            last_count: 0,
            last_pickup: 0.0,
            mark: (0.0, 0.0),
            last_volume: 0.0,
            gained: BTreeMap::new(),
            spent: BTreeMap::new(),
            unstuck_log: Vec::new(),
            starved: Vec::new(),
            steps: Vec::new(),
            area_now: "-".into(),
            radius_now: 0.0,
            last_t: 0.0,
            leftovers: String::new(),
        };
        tel.write_line(
            "t,wall,phase,mode,step,area,radius,diameter,count,trophies,npcs,x,y,z,speed,edible,\
             collectibles,nearest_edible,fps,ft_p50_ms,ft_p95_ms,ft_p99_ms,ft_max_ms,stuck,unstuck,\
             escapes,rams,target,events",
        );
        tel
    }

    fn write_line(&mut self, line: &str) {
        if let Some(out) = self.out.as_mut() {
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
    }

    fn event(&mut self, note: String) {
        self.events.push(note.replace([',', '\n'], ";"));
    }

    /// This level's tiers and goal word, once its recipe is loaded. The words are the tooling's
    /// events: "hogger edible", "hogger rolled up" for a creature goal, "goal ..." for a size.
    fn set_tiers(&mut self, design: &design::Design) {
        self.tiers = design
            .tiers
            .iter()
            .map(|t| (t.at, t.name.clone()))
            .collect();
        self.tier_at = vec![None; self.tiers.len()];
        self.goal_word = match design.goal.kind {
            design::GoalKind::Collect => design.goal.target.to_lowercase(),
            design::GoalKind::Size | design::GoalKind::Finale => "goal".to_string(),
        };
    }

    /// Per frame of play: the frame time, and the milestones that need no row.
    fn frame(&mut self, wall: f32, ms: f32, tier: usize, t: f32, hogger_edible: bool, won: bool) {
        self.samples.push((wall, ms, tier.saturating_sub(1)));
        self.last_t = t;
        for (i, slot) in self.tier_at.iter_mut().enumerate() {
            if slot.is_none() && i < tier {
                *slot = Some(t);
            }
        }
        if hogger_edible && self.edible_at.is_none() {
            self.edible_at = Some(t);
            self.event(format!("{} edible", self.goal_word));
        }
        if won && self.won_at.is_none() {
            self.won_at = Some(t);
            self.event(format!("{} rolled up", self.goal_word));
        }
    }

    fn row(&mut self, s: &Sample) {
        let lag = s.wall - self.last_row;
        self.last_row = s.wall;
        self.next_row = s.wall + 1.0;
        let fps = self.frames_row as f32 / lag.max(0.001);
        self.frames_row = 0;
        while self.window < self.samples.len() && self.samples[self.window].0 < s.wall - WINDOW {
            self.window += 1;
        }
        let win = sorted(self.samples[self.window..].iter().map(|x| x.1).collect());
        self.max_trophies = self.max_trophies.max(s.trophies);
        self.max_npcs = self.max_npcs.max(s.npcs);

        // Progress: when the last pickup was, and whether the radius has stopped growing.
        if s.count > self.last_count {
            self.last_count = s.count;
            self.last_pickup = s.t;
        }
        let pickup_gap = s.t - self.last_pickup;
        if pickup_gap > self.gap.0 {
            self.gap = (pickup_gap, self.last_pickup, s.area.to_string(), s.radius);
        }
        if self.mark.0 == 0.0 || s.radius >= self.mark.0 * 1.02 {
            self.mark = (s.radius, s.t);
        }
        let flat = s.t - self.mark.1;
        if flat > self.plateau.0 {
            self.plateau = (flat, self.mark.1, s.area.to_string(), s.radius);
        }
        *self.gained.entry(s.area.to_string()).or_default() +=
            (s.volume - self.last_volume).max(0.0);
        self.last_volume = s.volume;
        *self.spent.entry(s.area.to_string()).or_default() += lag;
        self.area_now = s.area.to_string();
        self.radius_now = s.radius;

        let events = std::mem::take(&mut self.events).join("|");
        let line = format!(
            "{:.1},{:.1},{},{},{},{},{:.3},{:.3},{},{},{},{:.1},{:.1},{:.1},{:.2},{},{},{:.1},{:.0},{:.1},{:.1},{:.1},{:.1},{},{},{},{},{},{}",
            s.t,
            s.wall,
            s.phase,
            s.mode,
            s.step.replace(',', ";"),
            s.area.replace(',', ";"),
            s.radius,
            2.0 * s.radius,
            s.count,
            s.trophies,
            s.npcs,
            s.pos[0],
            s.pos[1],
            s.pos[2],
            s.speed,
            self.edible,
            self.total,
            if self.nearest.is_finite() {
                self.nearest
            } else {
                -1.0
            },
            fps,
            percentile(&win, 50.0),
            percentile(&win, 95.0),
            percentile(&win, 99.0),
            win.last().copied().unwrap_or(0.0),
            self.stuck,
            self.unstuck,
            self.escapes,
            s.rams,
            s.target.replace(',', ";"),
            events
        );
        self.write_line(&line);
    }

    fn summary(&self, ball: &BallState, run: &Run, design: &design::Design, why: &str) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "== azeroll autopilot summary ==");
        let _ = writeln!(o, "build: {}", design::build_id());
        let _ = writeln!(
            o,
            "recipe fnv={:016x}  started {}  finished {}  ended by: {why}",
            design.recipe_hash,
            self.started,
            design::utc_now()
        );
        let _ = writeln!(
            o,
            "play {:.1}s (wall {:.1}s)  final r={:.2} (d={:.2})  count={}  max trophies={}  max carried NPCs={}",
            run.elapsed,
            self.samples.last().map_or(0.0, |s| s.0),
            ball.radius,
            ball.diameter(),
            run.count,
            self.max_trophies,
            self.max_npcs
        );
        let at = |t: Option<f32>| t.map_or("never".to_string(), |t| format!("{t:.1}s"));
        let tiers: Vec<String> = self
            .tiers
            .iter()
            .zip(&self.tier_at)
            .skip(1)
            .map(|((d, name), t)| format!("{name} (d>={d}) {}", at(*t)))
            .collect();
        let _ = writeln!(o, "time to tier: {}", tiers.join("; "));
        let word = {
            let mut c = self.goal_word.chars();
            c.next()
                .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
        };
        let _ = writeln!(
            o,
            "{word} edible: {}   {word} rolled up: {}",
            at(self.edible_at),
            at(self.won_at)
        );
        let all = sorted(self.samples.iter().map(|s| s.1).collect());
        let over = |ms: f32| self.samples.iter().filter(|s| s.1 > ms).count();
        let _ = writeln!(
            o,
            "frame time ms over {} frames: p50 {:.1}  p95 {:.1}  p99 {:.1}  max {:.1}   >33ms {}  >50ms {}  >100ms {}",
            all.len(),
            percentile(&all, 50.0),
            percentile(&all, 95.0),
            percentile(&all, 99.0),
            all.last().copied().unwrap_or(0.0),
            over(33.3),
            over(50.0),
            over(100.0)
        );
        for (i, (d, name)) in self.tiers.iter().enumerate() {
            let v = sorted(
                self.samples
                    .iter()
                    .filter(|s| s.2 == i)
                    .map(|s| s.1)
                    .collect(),
            );
            if v.is_empty() {
                continue;
            }
            let _ = writeln!(
                o,
                "  tier {name} (d>={d}): {} frames  p50 {:.1}  p95 {:.1}  p99 {:.1}  max {:.1}",
                v.len(),
                percentile(&v, 50.0),
                percentile(&v, 95.0),
                percentile(&v, 99.0),
                v.last().copied().unwrap_or(0.0)
            );
        }
        let _ = writeln!(
            o,
            "stuck events: {}   game unstuck teleports: {}   autopilot escapes: {}",
            self.stuck, self.unstuck, self.escapes
        );
        for u in &self.unstuck_log {
            let _ = writeln!(o, "  {u}");
        }
        let _ = writeln!(
            o,
            "longest wait for a pickup: {:.0}s from t={:.0}s in {} (r={:.2})",
            self.gap.0, self.gap.1, self.gap.2, self.gap.3
        );
        let _ = writeln!(
            o,
            "longest radius plateau (<2% growth): {:.0}s from t={:.0}s in {} (r={:.2})",
            self.plateau.0, self.plateau.1, self.plateau.2, self.plateau.3
        );
        for s in &self.starved {
            let _ = writeln!(o, "  {s}");
        }
        let _ = writeln!(
            o,
            "edible but uneaten at the end (area: name, count, how many are not on a shelf or table, nearest):"
        );
        for l in self.leftovers.lines() {
            let _ = writeln!(o, "  {l}");
        }
        let _ = writeln!(o, "route steps (entered at t, ball r):");
        for (name, t, r) in &self.steps {
            let _ = writeln!(o, "  t={t:>6.1}s r={r:.2}  {name}");
        }
        let _ = writeln!(
            o,
            "by area: seconds spent, volume gained (ball volume units, r^3):"
        );
        for (name, secs) in &self.spent {
            let _ = writeln!(
                o,
                "  {name:<24} {secs:>6.0}s  +{:.2}",
                self.gained.get(name).copied().unwrap_or(0.0)
            );
        }
        o
    }
}

/// Print the run's summary, and write it beside the telemetry CSV.
fn finish(auto: &mut Auto, ball: &BallState, run: &Run, design: &design::Design, why: &str) {
    let text = auto.tel.summary(ball, run, design, why);
    for l in text.lines() {
        info!("azeroll auto summary: {l}");
    }
    if let Some(path) = &auto.tel.path {
        let out = format!("{path}.summary.txt");
        if let Err(e) = std::fs::write(&out, &text) {
            warn!("azeroll auto: summary {out}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn planner_support_fast_paths_match_the_mover_sampler() {
        for rotation in [
            Quat::IDENTITY,
            Quat::from_rotation_y(0.7),
            Quat::from_rotation_x(0.3),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        ] {
            let b = Blocker {
                centre: Vec3::Y,
                half: Vec3::new(1., 0.2, 0.6),
                rotation,
                radius: 1.,
                walkable: true,
                boxed: true,
            };
            for r in [0.2, 0.5, 1.] {
                for x in -5..=5 {
                    for y in 0..=5 {
                        let p = Vec3::new(x as f32 * 0.3, y as f32 * 0.4, 0.1);
                        let slow =
                            super::super::furniture::box_rest(b.centre, b.half, b.rotation, p, r);
                        let fast = b.support(p, r);
                        match (slow, fast) {
                            (Some(a), Some(b)) => assert!((a - b).abs() < 1e-5),
                            (None, None) => {}
                            _ => panic!("{p:?} {r}: {slow:?} != {fast:?}"),
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn a_descending_route_checks_clearance_below_a_stairwell_lip() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<benilla_world::collision::MoverTraceExclusions>();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        for (pos, size) in [
            (Vec3::new(0., -0.1, 0.), Vec3::new(10., 0.2, 10.)),
            (Vec3::new(1., 1., 0.), Vec3::new(0.8, 0.1, 0.8)),
        ] {
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(pos),
            ));
        }
        app.update();
        app.world_mut()
            .run_system_once(|world: benilla_world::collision::WorldCollision| {
                let shape = Collider::sphere(0.17);
                let from = Vec3::new(-1., 2., 0.);
                assert!(
                    !indoor_edge(&world, &shape, 0.2, from, Vec3::X, 0..0, &[]),
                    "horizontal clearance above the lip cannot allow a drop through it"
                );
                assert!(
                    indoor_edge(&world, &shape, 0.2, from, Vec3::X * 2., 0..0, &[]),
                    "a clear drop beyond the lip remains possible"
                );
            })
            .unwrap();
    }

    #[test]
    fn installed_inn_stairs_connect_all_three_storeys() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let mut vertices = vec![];
        let mut indices = vec![];
        for group in 0..12 {
            let path =
                format!(r"world\wmo\azeroth\buildings\goldshireinn\goldshireinn_{group:03}.wmo");
            let bytes = chain.read_file(&path).unwrap();
            benilla_formats::accumulate_wmo_group_collision(&bytes, &mut vertices, &mut indices);
        }
        let (sin, cos) = 83_f32.to_radians().sin_cos();
        let vertices: Vec<_> = vertices
            .into_iter()
            .map(|[x, y, z]| {
                units::wow_to_bevy([
                    cos * x - sin * y - 9464.247,
                    sin * x + cos * y + 24.399,
                    z + 56.526,
                ])
            })
            .collect();
        let triangles = indices
            .chunks_exact(3)
            .map(|v| [v[0], v[1], v[2]])
            .collect();
        let mut app = App::new();
        app.init_resource::<benilla_world::collision::MoverTraceExclusions>();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::trimesh(vertices, triangles),
            Transform::default(),
        ));
        app.update();
        let design = design::parse(include_str!("lions-pride.toml")).unwrap();
        app.world_mut()
            .run_system_once(move |world: benilla_world::collision::WorldCollision| {
                for (name, from, to) in [
                    ("cellar ascent", [-9466., -4., 49.8], [-9466., 5., 56.95]),
                    ("cellar descent", [-9466., 5., 56.95], [-9466., -4., 49.8]),
                    ("main ascent", [-9477., 22., 56.339], [-9468., 24., 63.82]),
                    ("main descent", [-9468., 24., 63.82], [-9477., 22., 56.339]),
                ] {
                    for r in [0.3, 0.5] {
                        let on_floor = |p| {
                            let p = units::wow_to_bevy(p);
                            let origin = p + Vec3::Y * 0.6;
                            let h = world
                                .ray_body(origin, Dir3::NEG_Y, 1.2)
                                .expect("authored floor");
                            Vec3::new(p.x, origin.y - h.distance + r, p.z)
                        };
                        let ball = BallState {
                            pos: on_floor(from),
                            radius: r,
                            ..Default::default()
                        };
                        let target = on_floor(to);
                        if name == "cellar descent" && r == 0.3 {
                            // Scheduling delays must cancel the live search without
                            // masquerading as a geometry failure. No sleeps needed.
                            assert_eq!(
                                indoor_path_search(
                                    &world, &design, &ball, target, 0., &[], None, || true,
                                ),
                                Err(IndoorPathError::Deadline),
                            );
                        }
                        for arrival in [None, Some(1.5)] {
                            let path = indoor_path_search(
                                &world, &design, &ball, target, 0., &[], arrival, || false,
                            )
                            .unwrap_or_else(|reason| {
                                panic!(
                                    "{name} r={r} arrival={arrival:?} {reason:?}: {:?} -> {target:?}",
                                    ball.pos
                                )
                            });
                            if arrival.is_some() {
                                // Execute the planned room route with the mover's full-radius
                                // support and 0.85r sweep, not just its centre-floor samples.
                                let mut nav = IndoorNav { entity: Entity::PLACEHOLDER, at: target,
                                    radius: r, path: path.clone(), next: 0, blockers: vec![] };
                                let mut pos = ball.pos;
                                let mut vy = 0.;
                                let mut grounded = true;
                                let shape = Collider::sphere(0.85*r);
                                let mut arrived = false;
                                let mut last_hits = vec![];
                                let mut last_slide = String::new();
                                for _ in 0..9000 {
                                    nav.advance(pos, (0.4*r).max(0.12), r);
                                    if nav.next + 1 == nav.path.len()
                                        && units::flat(pos - nav.path[nav.next]).length() < 0.16
                                        && (pos.y - nav.path[nav.next].y).abs() < 0.25 + 0.1*r {
                                        arrived = true; break;
                                    }
                                    let velocity = units::flat(nav.path[nav.next] - pos).normalize_or_zero()*2.;
                                    let ahead = pos + velocity/60.;
                                    if let Some(rest) = super::super::ball::floor_rest(&world, ahead, r) {
                                        if grounded && rest > pos.y && rest - pos.y <= super::super::ball::step_height(r) {
                                            pos.y = rest;
                                        }
                                    }
                                    last_hits.clear();
                                    let out = world.slide_body(&shape, pos, velocity,
                                        std::time::Duration::from_secs_f32(1./60.),
                                        &avian3d::character_controller::move_and_slide::MoveAndSlideConfig::default(), |hit| {
                                            let n: Vec3 = **hit.normal;
                                            last_hits.push((n, *hit.velocity));
                                            if n.y > 0.6 {
                                                let horizontal = Vec3::new(hit.velocity.x, 0., hit.velocity.z);
                                                hit.velocity.y = -horizontal.dot(n)/n.y;
                                            }
                                            avian3d::character_controller::move_and_slide::MoveAndSlideHitResponse::Accept
                                        });
                                    last_slide = format!("from={pos:?} intent={velocity:?} out={:?} projected={:?}", out.position, out.projected_velocity);
                                    pos.x = out.position.x; pos.z = out.position.z;
                                    let rest = super::super::ball::floor_rest(&world, pos, r).unwrap();
                                    (pos.y, vy, grounded) = super::super::ball::settle_vertical(pos.y, vy,
                                        grounded, rest, 0.05+0.02*r+0.8*2./60., 1./60.);
                                }
                                assert!(arrived, "{name} r={r}: mover stopped at {pos:?}, node {}/{} {:?} previous={:?} hits={last_hits:?} slide={last_slide}", nav.next, nav.path.len(), nav.path[nav.next], nav.path[nav.next.saturating_sub(1)]);
                            }
                            let end = *path.last().unwrap();
                            if let Some(reach) = arrival {
                                assert!(units::flat(end - target).length() < reach + 0.01
                                    && (end.y - target.y).abs() < 0.25 + 0.1*r,
                                    "{name} ends on correct floor: {end:?} -> {target:?}");
                            } else {
                                assert!(end.distance(target) < r + 0.01,
                                    "{name} finishes in pickup contact: {end:?} -> {target:?}");
                            }
                        }
                    }
                }
            })
            .unwrap();
    }
    #[test]
    fn stacked_rooms_and_short_stair_routes_require_the_correct_floor() {
        let region = Region::Area {
            centre: Vec3::Y * 7.,
            radius: 20.,
            stacked: true,
        };
        let food = Collectible {
            name: "test food".into(),
            kind: Kind::Prop,
            source: super::super::Source::Spawned,
            size: 0.5,
            pickup: 1.,
            half_extents: Vec3::splat(0.5),
            center: Vec3::ZERO,
            solid: 0.,
        };
        assert!(
            !region.holds_floor(&Transform::from_xyz(0., 6.9, 0.), &food),
            "food on the lower pit floor cannot lure a raised-room route away"
        );
        assert!(
            region.holds_floor(&Transform::from_xyz(0., 8.5, 0.), &food),
            "food on an elevated table remains eligible"
        );
        assert!(!region.holds(Vec3::ZERO, Vec3::ZERO, 0.5));
        assert!(region.holds(Vec3::Y * 7., Vec3::ZERO, 0.5));
        let mut ball = BallState {
            pos: Vec3::Y * 0.5,
            radius: 0.5,
            ..Default::default()
        };
        assert!(!route_floor(&ball, Vec3::Y * (-6.5)));
        assert!(!route_floor(&ball, Vec3::Y * 7.));
        ball.pos.y = 7.5;
        assert!(route_floor(&ball, Vec3::Y * 7.));
    }
    #[test]
    fn standing_below_a_waypoint_does_not_complete_its_climb() {
        let mut nav = IndoorNav {
            entity: Entity::PLACEHOLDER,
            at: Vec3::X + Vec3::Y,
            radius: 0.2,
            path: vec![Vec3::Y, Vec3::X + Vec3::Y],
            next: 0,
            blockers: vec![],
        };
        nav.advance(Vec3::ZERO, 0.18, 0.3);
        assert_eq!(nav.next, 0, "same XZ below the stair is not arrival");
        assert!(nav.remaining(Vec3::ZERO) > 1.9);
        nav.advance(Vec3::Y * 1.2, 0.18, 0.3);
        assert_eq!(nav.next, 1, "rounded support above a tread still arrives");
    }
    #[test]
    fn a_ready_finale_is_a_destination_without_a_pickup_axis() {
        let mut goal: design::Goal =
            toml::from_str("kind='finale'\ndiameter=160\ncity_pieces=24\n").unwrap();
        assert!(!super::finale_destination_ready(&goal, 159., 24));
        assert!(!super::finale_destination_ready(&goal, 160., 23));
        assert!(super::finale_destination_ready(&goal, 160., 24));
        goal.kind = design::GoalKind::Size;
        assert!(!super::finale_destination_ready(&goal, 160., 24));
    }
    use super::*;

    #[test]
    fn indoor_route_uses_mode_book_stairs_to_reach_table_food() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<benilla_world::collision::MoverTraceExclusions>();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20., 0.2, 20.),
            Transform::from_xyz(0., -0.1, 0.),
        ));
        app.update();
        let mut design = design::parse(include_str!("lions-pride.toml")).unwrap();
        design.bounds = Some(design::Bounds {
            centre: Some([0.; 3]),
            radius: 20.,
        });
        app.world_mut()
            .run_system_once(move |world: benilla_world::collision::WorldCollision| {
                let mut blocks: Vec<_> = (0..7)
                    .map(|i| Blocker {
                        centre: Vec3::new(-1.2 + i as f32 * 0.4, 0.125 * i as f32 + 0.0625, 0.),
                        half: Vec3::new(0.25, 0.0625, 0.6),
                        rotation: Quat::IDENTITY,
                        radius: 0.,
                        walkable: true,
                        boxed: true,
                    })
                    .collect();
                blocks.push(Blocker {
                    centre: Vec3::new(2., 0.4375, 0.),
                    half: Vec3::new(0.8, 0.4375, 0.8),
                    rotation: Quat::IDENTITY,
                    radius: 0.,
                    walkable: true,
                    boxed: true,
                });
                let ball = BallState {
                    pos: Vec3::new(-2., 0.15, 0.),
                    radius: 0.15,
                    ..Default::default()
                };
                let food = Vec3::new(2., 0.895, 0.);
                assert!(!within_reach(ball.radius, food.y - ball.pos.y, 0.05));
                let path = indoor_path(&world, &design, &ball, food, 0.05, &blocks)
                    .expect("mode-only stairs reach table");
                assert!(path.last().unwrap().y > 0.9);
                blocks.truncate(0);
                assert!(
                    indoor_path(&world, &design, &ball, food, 0.05, &blocks).is_none(),
                    "floor alone cannot reach the food"
                );
            })
            .unwrap();
    }
    #[test]
    fn indoor_route_climbs_steps_but_rejects_shelves_and_narrow_exits() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<benilla_world::collision::MoverTraceExclusions>();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        let profile = [
            (-3.0, 0.0),
            (-0.6, 0.0),
            (-0.6, 0.22),
            (0.0, 0.22),
            (0.0, 0.44),
            (0.6, 0.44),
            (0.6, 0.624),
            (3.0, 0.624),
        ];
        let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
        for pair in profile.windows(2) {
            let [(x0, y0), (x1, y1)] = [pair[0], pair[1]];
            let b = vertices.len() as u32;
            vertices.extend([
                Vec3::new(x0, y0, -2.0),
                Vec3::new(x1, y1, -2.0),
                Vec3::new(x1, y1, 2.0),
                Vec3::new(x0, y0, 2.0),
            ]);
            triangles.extend([[b, b + 2, b + 1], [b, b + 3, b + 2]]);
        }
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::trimesh(vertices, triangles),
            Transform::default(),
        ));
        app.update();
        let mut design = design::parse(include_str!("lions-pride.toml")).unwrap();
        design.bounds = Some(design::Bounds {
            centre: Some([0.0; 3]),
            radius: 20.0,
        });
        app.world_mut()
            .run_system_once(move |world: benilla_world::collision::WorldCollision| {
                for (x, y) in [(-2.0, 0.0), (-0.3, 0.22), (0.3, 0.44), (2.0, 0.624)] {
                    let origin = Vec3::new(x, y + 0.3, 0.0);
                    let hit = world
                        .ray_body(origin, Dir3::NEG_Y, 1.0)
                        .expect("fixture floor");
                    assert!((origin.y - hit.distance - y).abs() < 1e-4);
                }
                let blockers: Vec<_> = [-1.0, 1.0]
                    .into_iter()
                    .map(|side| Blocker {
                        centre: Vec3::new(1.6, 1.624, side * 1.825),
                        half: Vec3::new(0.06, 1.0, 1.175),
                        rotation: Quat::IDENTITY,
                        radius: 0.0,
                        walkable: true,
                        boxed: true,
                    })
                    .collect();
                let mut ball = BallState {
                    pos: Vec3::new(-2.0, 0.15, 0.0),
                    radius: 0.15,
                    ..Default::default()
                };
                let goal = Vec3::new(2.46, 0.644, 0.0);
                let path = indoor_path(&world, &design, &ball, goal, 0.07, &blockers)
                    .expect("walk up three treads through the doorway");
                assert!(path.last().unwrap().y > 0.7);
                assert!(
                    path.last().unwrap().distance(goal) < ball.radius + 0.9 * 0.07,
                    "the final stance must actually touch the pickup in 3D"
                );
                assert!(
                    indoor_path(
                        &world,
                        &design,
                        &ball,
                        Vec3::new(goal.x, 0.624 + ball.radius, goal.z),
                        0.0,
                        &blockers,
                    )
                    .is_some(),
                    "a no-pickup fallback can walk to open floor on the landing"
                );
                assert!(
                    indoor_path(
                        &world,
                        &design,
                        &ball,
                        goal + Vec3::Y * 1.2,
                        0.07,
                        &blockers
                    )
                    .is_none(),
                    "food on a high shelf is not a floor target"
                );
                ball.radius = 0.8;
                ball.pos.y = 0.8;
                assert!(
                    indoor_path(&world, &design, &ball, goal, 0.07, &blockers).is_none(),
                    "grown body cannot enter a narrow doorway"
                );
            })
            .unwrap();
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let v: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        assert_eq!(percentile(&v, 50.0), 51.0);
        assert_eq!(percentile(&v, 99.0), 99.0);
        assert_eq!(percentile(&v, 100.0), 100.0);
        assert_eq!(percentile(&[], 95.0), 0.0);
        assert_eq!(percentile(&[7.0], 99.0), 7.0);
    }

    #[test]
    fn trail_regions_keep_to_the_road_and_forward() {
        let region = Region::Trail {
            from: Vec3::ZERO,
            to: Vec3::new(0.0, 0.0, -100.0),
        };
        let ball = Vec3::new(0.0, 0.0, -50.0);
        assert!(region.holds(Vec3::new(3.0, 0.0, -70.0), ball, 1.0));
        assert!(
            !region.holds(Vec3::new(30.0, 0.0, -70.0), ball, 1.0),
            "too far off the road"
        );
        assert!(
            !region.holds(Vec3::new(0.0, 0.0, -20.0), ball, 1.0),
            "behind the ball"
        );
        assert!(
            !region.holds(Vec3::new(0.0, 0.0, -140.0), ball, 1.0),
            "past the end"
        );
    }

    #[test]
    fn failures_widen_a_dead_zone_until_the_ball_has_grown() {
        let mut z = DeadZones::default();
        z.write_off(Vec3::ZERO, 0.4);
        assert!(z.contains(Vec3::new(2.0, 0.0, 0.0), 0.4));
        assert!(
            !z.contains(Vec3::new(5.0, 0.0, 0.0), 0.4),
            "one failure is a small disc"
        );
        z.write_off(Vec3::new(5.0, 0.0, 0.0), 0.4);
        z.write_off(Vec3::new(9.0, 0.0, 0.0), 0.4);
        assert_eq!(z.0.len(), 1, "neighbouring failures merge");
        assert!(z.contains(Vec3::new(10.0, 0.0, 0.0), 0.4));
        assert!(!z.contains(Vec3::new(40.0, 0.0, 0.0), 0.4), "capped");
        assert!(
            !z.contains(Vec3::new(2.0, 0.0, 0.0), 0.4 * RETRY_GROWTH),
            "grown: try again"
        );
    }

    #[test]
    fn fences_and_trunks_block_the_straight_line() {
        let fence = Blocker {
            centre: Vec3::new(0.0, 0.0, -5.0),
            rotation: Quat::IDENTITY,
            half: Vec3::new(3.0, 1.0, 0.1),
            radius: 0.0,
            walkable: true,
            boxed: true,
        };
        let from = Vec3::ZERO;
        assert!(
            fence.crosses(from, Vec3::new(0.0, 0.0, -10.0), 0.3),
            "straight through"
        );
        assert!(
            !fence.crosses(from, Vec3::new(0.0, 0.0, -4.0), 0.3),
            "stops short"
        );
        assert!(
            !fence.crosses(from, Vec3::new(8.0, 0.0, -10.0), 0.3),
            "passes round the end"
        );
        let turned = Blocker {
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            ..fence
        };
        // Turned a quarter, the fence runs along the walk: a path beside it is clear.
        assert!(turned.crosses(from, Vec3::new(0.0, 0.0, -10.0), 0.3));
        let beside = Vec3::new(2.0, 0.0, 0.0);
        assert!(
            !turned.crosses(beside, beside + Vec3::NEG_Z * 10.0, 0.3),
            "the fence runs the other way"
        );
        let trunk = Blocker {
            radius: 1.0,
            walkable: true,
            boxed: false,
            ..turned
        };
        assert!(trunk.crosses(from, Vec3::new(0.5, 0.0, -10.0), 0.3));
        assert!(!trunk.crosses(from, Vec3::new(5.0, 0.0, -10.0), 0.3));
    }

    #[test]
    fn a_way_round_a_fence_is_found_and_none_when_the_line_is_clear() {
        // A long fence across the way north (-z), ten yards on.
        let fence = Blocker {
            centre: Vec3::new(0.0, 0.0, -10.0),
            rotation: Quat::IDENTITY,
            half: Vec3::new(8.0, 1.0, 0.1),
            radius: 0.0,
            walkable: true,
            boxed: true,
        };
        let from = Vec3::ZERO;
        let to = Vec3::new(0.0, 0.0, -20.0);
        assert!(steer_round(&[], from, to, 0.34).is_none());
        assert!(steer_round(
            std::slice::from_ref(&fence),
            from,
            Vec3::new(0.0, 0.0, 20.0),
            0.34
        )
        .is_none());
        let wp = steer_round(std::slice::from_ref(&fence), from, to, 0.34).expect("a way round");
        // The first waypoint is off to one side of the fence, not through it.
        assert!(!fence.crosses(from, wp, 0.3), "{wp:?}");
        // Following waypoints from where the ball is gets it to the far side.
        let mut at = from;
        for _ in 0..80 {
            // `None` means the line is clear: head straight for it.
            let next = steer_round(std::slice::from_ref(&fence), at, to, 0.34).unwrap_or(to);
            at += (next - at).clamp_length_max(2.0);
        }
        assert!(at.z < -12.0, "got to {at:?}");
        // Boxed in on all sides: no way, no waypoint (and no hang).
        let wall = |x: f32, z: f32, r: f32| Blocker {
            centre: Vec3::new(x, 0.0, z),
            rotation: Quat::from_rotation_y(r),
            half: Vec3::new(40.0, 1.0, 0.1),
            radius: 0.0,
            walkable: true,
            boxed: true,
        };
        let walls = [
            wall(0.0, -10.0, 0.0),
            wall(-30.0, -10.0, std::f32::consts::FRAC_PI_2),
            wall(30.0, -10.0, std::f32::consts::FRAC_PI_2),
        ];
        let _ = steer_round(&walls, from, to, 0.34);
    }

    #[test]
    fn area_regions_are_circles() {
        let region = Region::Area {
            stacked: false,
            centre: Vec3::new(10.0, 5.0, 10.0),
            radius: 20.0,
        };
        assert!(region.holds(Vec3::new(25.0, 0.0, 10.0), Vec3::ZERO, 1.0));
        assert!(!region.holds(Vec3::new(40.0, 0.0, 10.0), Vec3::ZERO, 1.0));
    }
}

#[cfg(test)]
#[path = "city_nav_tests.rs"]
mod city_nav_tests;
