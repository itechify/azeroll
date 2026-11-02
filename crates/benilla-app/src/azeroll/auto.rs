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
use super::{level, units, Attached, BallState, Collectible, Kind, Phase, Rng, Run};
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

/// A solid thing the ball cannot take: an authored fence (an oriented box) or a trunk (a disc).
#[derive(Clone)]
struct Blocker {
    centre: Vec3,
    rotation: Quat,
    half: Vec3,
    radius: f32,
    boxed: bool,
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
        tel: Telemetry::from_env(),
    })
}

pub(super) fn plugin(app: &mut App) {
    if let Some(auto) = from_env() {
        app.insert_resource(auto)
            .add_systems(
                Update,
                drive
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
    auto.steps = match auto.mode {
        Mode::Free => Vec::new(),
        Mode::Primary => design::plan_route(design, RouteKind::Primary),
        Mode::Tour => design::plan_route(design, RouteKind::Tour),
    };
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
    (Without<Attached>, Without<Shed>),
>;

/// Where the current step wants the food to be.
enum Region {
    Everywhere,
    Area { centre: Vec3, radius: f32 },
    Trail { from: Vec3, to: Vec3 },
}

impl Region {
    fn of(auto: &Auto, design: &design::Design) -> Self {
        if auto.hunt {
            return Self::Everywhere;
        }
        match auto.steps.get(auto.step) {
            Some(&Step::Area(i)) => Self::Area {
                centre: units::wow_to_bevy(design.areas[i].centre),
                radius: design.areas[i].radius + 6.0,
            },
            Some(&Step::Trail { index, reversed }) => {
                let (from, to) = design::trail_ends(design, index, reversed);
                Self::Trail { from, to }
            }
            None => Self::Everywhere,
        }
    }

    /// Whether food at `at` is this step's business, given where the ball is.
    fn holds(&self, at: Vec3, ball: Vec3, radius: f32) -> bool {
        match *self {
            Self::Everywhere => units::flat(at - ball).length() < 80.0 + radius * 4.0,
            Self::Area { centre, radius } => units::flat(at - centre).length() <= radius,
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
    auto: &Auto,
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
    let mut best_score = 0.0_f32;
    let mut threat_d = 14.0_f32;
    // Things in the way that the ball cannot take. The world's collision rays do not see them
    // (they are collectibles), so a fence between the ball and its supper has to be found here.
    let blockers: Vec<Blocker> = things
        .iter()
        .filter(|(_, c, t)| {
            c.solid > 0.0
                && !can(c.pickup)
                && units::flat(t.translation - ball.pos).length() < 60.0 + c.pickup
        })
        .map(|(e, c, t)| Blocker {
            centre: t.transform_point(c.center),
            rotation: t.rotation,
            half: (c.half_extents * t.scale.abs()).max(Vec3::splat(0.05)),
            radius: c.solid,
            boxed: gates.contains(e),
        })
        .collect();
    for (e, c, t) in things {
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
            || auto.dead.contains(at, ball.radius)
        {
            continue;
        }
        scan.region_vol += c.volume(t.scale);
        // Best value for the trip: growth falls off as (s/r)^2.5, so a bigger thing a little
        // further away beats a crumb underfoot.
        let mut score = (c.pickup / ball.radius).powf(2.5) / (d + 2.0 + ball.radius);
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
    scan.blockers = blockers;
    scan
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut commands: Commands,
    mut auto: ResMut<Auto>,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    loading: Res<LoadingScreen>,
    mut ball: ResMut<BallState>,
    run: Res<Run>,
    (mut flow, progress): (ResMut<Flow>, Res<Progress>),
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
    let dt = time.delta_secs();
    if loading.covering() {
        return;
    }
    match phase.get() {
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
                if mode == "free" && progress.best().wins > 0 {
                    progress::start_free_roll(&mut flow, &mut next);
                    return;
                }
            }
            next.set(Phase::Playing);
            return;
        }
        Phase::Paused => return,
        Phase::Results => {
            auto.over_t += dt;
            if auto.over_t > 0.5 && auto.over_t - dt <= 0.5 {
                shoot(&mut commands, &mut auto, "results");
            }
            if auto.free.is_some() && run.won && auto.over_t > 2.0 {
                info!("azeroll auto: starting Free Roll from the results");
                progress::start_free_roll(&mut flow, &mut next);
                return;
            }
            if auto.over_t > 3.0 {
                info!(
                    "azeroll auto: results won={} t={:.1} r={:.2}, exiting",
                    run.won, run.elapsed, ball.radius
                );
                auto.tel.leftovers = leftovers(&things, &ball, &design);
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
    let play_t = auto.play_t;
    let diameter_tier = level::TIERS
        .iter()
        .take_while(|(at, _)| ball.diameter() >= *at)
        .count();
    let hogger_edible = contact(ball.radius, level::HOGGER_SIZE) == Contact::RollUp;
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
    auto.scan_t -= dt;
    let lost = auto
        .scan
        .pick
        .is_some_and(|p| things.get(p.entity).is_err());
    if auto.scan_t <= 0.0 || lost {
        auto.scan_t = SCAN_EVERY;
        let scan = look(&auto, &ball, &design, &things, &collision, &gates, can);
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
    let boss_ready = hogger_edible;
    let boss = boss_ready
        .then(|| {
            things
                .iter()
                .find(|(_, c, _)| c.kind == Kind::Boss && can(c.pickup))
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
            let hopeless = since > NO_PROGRESS + 0.5 * ball.radius || total > cap;
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
    let dir = if auto.unstick_t > 0.0 {
        mode.push_str("escape");
        // Cycle through ways out: toward the goal, either side of it, and back the way it came.
        let to_goal = match goal.or(auto.waypoint) {
            Some(g) => {
                let aim = auto.aim(&ball, g, dt);
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
        let aim = auto.aim(&ball, p.at, dt);
        if aim != p.at {
            mode.push_str("+round");
        }
        units::flat(aim - ball.pos).normalize_or(auto.wander)
    } else if boss_ready {
        mode.push_str("camp");
        let aim = auto.aim(&ball, units::wow_to_bevy(design.camp), dt);
        units::flat(aim - ball.pos).normalize_or(auto.wander)
    } else if let Some(g) = goal.filter(|_| !auto.hunt) {
        mode.push_str("route");
        let aim = auto.aim(&ball, g, dt);
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
            let aim = auto.aim(&ball, c, dt);
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
            let home = units::flat(units::wow_to_bevy(design.start) - ball.pos);
            if home.length() > level::LEASH * 0.6 {
                home.normalize()
            } else {
                auto.wander
            }
        }
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
        auto.tel.leftovers = leftovers(&things, &ball, &design);
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
            auto.tel.leftovers = leftovers(&things, &ball, &design);
            finish(&mut auto, &ball, &run, &design, "free roll done");
            exit.write(AppExit::Success);
        }
    } else if run.elapsed >= auto.seconds {
        info!(
            "azeroll auto: {:.0}s done, r={:.2} count={}, exiting",
            run.elapsed, ball.radius, run.count
        );
        auto.tel.leftovers = leftovers(&things, &ball, &design);
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
    for (n, c, t) in things {
        if units::flat(t.transform_point(c.center) - at).length() < 1.5 + c.size {
            auto.skip.insert(n, ball.radius);
        }
    }
    auto.dead.write_off(at, ball.radius);
}

impl Auto {
    /// The point to steer for to reach `to`: `to` itself, or a way round the fences and trunks
    /// between (looked for a few times a second, and again when `to` has moved on).
    fn aim(&mut self, ball: &BallState, to: Vec3, dt: f32) -> Vec3 {
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
            let inside =
                units::flat(ball.pos - units::wow_to_bevy(a.centre)).length() <= a.radius + 6.0;
            if !inside {
                (false, "")
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
            let near = units::flat(ball.pos - to).length() < 8.0 + ball.radius;
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
fn leftovers(things: &Loose, ball: &BallState, design: &design::Design) -> String {
    let mut groups: BTreeMap<(String, String), (usize, usize, f32)> = BTreeMap::new();
    for (_, c, t) in things {
        if contact(ball.radius, c.pickup) != Contact::RollUp {
            continue;
        }
        let at = t.transform_point(c.center);
        let area = design::area_at(design, at).map_or("-", |i| design.areas[i].name.as_str());
        let g = groups
            .entry((area.to_string(), c.name.to_string()))
            .or_insert((0, 0, f32::INFINITY));
        g.0 += 1;
        g.1 += usize::from(within_reach(ball.radius, at.y - ball.pos.y, c.size));
        g.2 = g.2.min(units::flat(at - ball.pos).length());
    }
    let mut v: Vec<_> = groups.into_iter().collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.1 .0));
    v.iter()
        .take(12)
        .map(|((area, name), (n, reach, near))| {
            format!("{area}: {name} x{n} ({reach} within reach of the ball's height, nearest {near:.0} yd)")
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
            tier_at: vec![None; level::TIERS.len()],
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
            self.event("hogger edible".into());
        }
        if won && self.won_at.is_none() {
            self.won_at = Some(t);
            self.event("hogger rolled up".into());
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
        let tiers: Vec<String> = level::TIERS
            .iter()
            .zip(&self.tier_at)
            .skip(1)
            .map(|((d, name), t)| format!("{name} (d>={d}) {}", at(*t)))
            .collect();
        let _ = writeln!(o, "time to tier: {}", tiers.join("; "));
        let _ = writeln!(
            o,
            "Hogger edible: {}   Hogger rolled up: {}",
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
        for (i, (d, name)) in level::TIERS.iter().enumerate() {
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
    use super::*;

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
            centre: Vec3::new(10.0, 5.0, 10.0),
            radius: 20.0,
        };
        assert!(region.holds(Vec3::new(25.0, 0.0, 10.0), Vec3::ZERO, 1.0));
        assert!(!region.holds(Vec3::new(40.0, 0.0, 10.0), Vec3::ZERO, 1.0));
    }
}
