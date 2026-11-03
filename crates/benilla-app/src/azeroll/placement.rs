//! The placement validator (`WOW_AZEROLL_PLACEMENT=1`): once a level's recipe props have spawned
//! on the real world (the same spawn ray, terrain and claimed furniture the game uses), check
//! every authored prop and every ramp, and write a report.
//!
//! Per prop: (a) the surface the spawn ray found (terrain, WMO, doodad hull) and its gap to the
//! authored hint (the scene anchor's floor, or the item's `on`); (b) a box that cuts into the
//! world's collision (a wall, a counter) or sits inside a solid prop; (c) a prop floating above,
//! or sunk below, whatever is under it. Per `stairs` run and pitched solid plank: (d) a ball of the
//! item's `climb` radius (default the level's start radius) rolls from the floor at the foot to
//! the surface beyond the top with the mover's own support rule (`ball::floor_rest` plus every
//! solid's `collect::solid_rest`, the step limit `ball::step_height`) and must arrive on top.
//!
//! The report goes to `/tmp/azeroll-placement-<level>.txt` (or the variable's value when it ends in
//! `.txt`); `WOW_AZEROLL_PLACEMENT_EXIT=1` quits once it is written.
use super::design::{Arrangement, Authored, Design, Gate, Pending};
use super::*;
use avian3d::prelude::{Collider, SpatialQuery};
use benilla_world::{collision::WorldCollision, world_point::WorldPoint};
use bevy::prelude::*;
use std::fmt::Write as _;

pub(super) fn plugin(app: &mut App) {
    if crate::webenv::var("WOW_AZEROLL_PLACEMENT").is_none() {
        return;
    }
    app.add_systems(
        Update,
        check.after(benilla_world::schedule::WorldStage::Stream),
    );
}

/// A prop's oriented box in world space.
#[derive(Clone, Copy)]
struct Obb {
    centre: Vec3,
    half: Vec3,
    rot: Quat,
}
impl Obb {
    fn of(tf: &Transform, c: &Collectible) -> Self {
        Self {
            centre: tf.transform_point(c.center),
            half: c.half_extents * tf.scale.abs(),
            rot: tf.rotation,
        }
    }
    /// World-space half height (the box's vertical reach from its centre).
    fn reach_y(&self) -> f32 {
        (0..3)
            .map(|i| {
                let mut a = Vec3::ZERO;
                a[i] = self.half[i];
                (self.rot * a).y.abs()
            })
            .sum()
    }
    fn bottom(&self) -> f32 {
        self.centre.y - self.reach_y()
    }
    fn top(&self) -> f32 {
        self.centre.y + self.reach_y()
    }
    fn contains(&self, p: Vec3, shrink: f32) -> bool {
        let l = self.rot.inverse() * (p - self.centre);
        (0..3).all(|i| l[i].abs() <= self.half[i] * shrink)
    }
    /// The highest point where a downward ray at (x, from_y, z) meets the box, if it does.
    fn ray_down(&self, x: f32, z: f32, from_y: f32) -> Option<f32> {
        let o = self.rot.inverse() * (Vec3::new(x, from_y, z) - self.centre);
        let d = self.rot.inverse() * Vec3::NEG_Y;
        let (mut t0, mut t1) = (0.0_f32, f32::INFINITY);
        for i in 0..3 {
            if d[i].abs() < 1e-6 {
                if o[i].abs() > self.half[i] {
                    return None;
                }
                continue;
            }
            let a = (-self.half[i] - o[i]) / d[i];
            let b = (self.half[i] - o[i]) / d[i];
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        (t0 > 0.0 && t0 <= t1).then_some(from_y - t0)
    }
    /// Horizontal half length along `dir` (unit, flat).
    fn along(&self, dir: Vec3) -> f32 {
        (0..3)
            .map(|i| {
                let mut a = Vec3::ZERO;
                a[i] = self.half[i];
                (self.rot * a).dot(dir).abs()
            })
            .sum()
    }
}

#[derive(Default)]
struct Tally {
    props: usize,
    fails: usize,
    warns: usize,
    ramps: usize,
    ramp_fails: usize,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn check(
    mut done: Local<bool>,
    mut wait: Local<u32>,
    d: Res<Design>,
    pending: Res<Pending>,
    current: Res<campaign::CurrentLevel>,
    collision: WorldCollision,
    world: WorldPoint,
    spatial: SpatialQuery,
    props: Query<(Entity, &Transform, &Collectible, &Authored, Has<Gate>)>,
    solids: Query<
        (Entity, &Transform, &Collectible, Has<Gate>),
        (
            Without<Attached>,
            Without<collect::Shed>,
            Without<Ball>,
            Without<BallSpin>,
        ),
    >,
    floors: Query<
        Option<&avian3d::prelude::CollisionLayers>,
        With<benilla_world::collision::GroundDecalSurface>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    if *done || !pending.settled() {
        return;
    }
    // Claimed furniture and the last colliders land a moment after the recipe settles.
    *wait += 1;
    if *wait < 240 {
        return;
    }
    *done = true;
    let id = current.entry().id;
    let mut out = String::new();
    let mut t = Tally::default();
    let support = |pos: Vec3, r: f32| {
        solids
            .iter()
            .filter_map(|(_, tf, c, gate)| collect::solid_rest(tf, c, pos, r, gate))
            .chain(ball::floor_rest(&collision, pos, r))
            .max_by(f32::total_cmp)
    };
    // A WMO walk collider is on the walk layers; a terrain tile keeps avian's default layers.
    let walk = |l: Option<&avian3d::prelude::CollisionLayers>| {
        l.is_some_and(|l| *l != avian3d::prelude::CollisionLayers::default())
    };
    let kind_at = |hit: Entity, at: Vec3| {
        if world
            .terrain_height_under(at)
            .is_some_and(|h| (h - at.y).abs() < 0.05)
        {
            "terrain"
        } else if floors.get(hit).is_ok_and(walk) {
            "wmo"
        } else if floors.contains(hit) {
            "terrain"
        } else {
            "doodad"
        }
    };
    // What the mover treats as solid (no tree canopies); furniture by its slab, legs open.
    let boxes: Vec<(Entity, Obb, &Collectible, bool)> = solids
        .iter()
        .filter(|(_, _, c, gate)| c.solid > 0.0 && collect::boxed_solid(c, *gate))
        .map(|(e, tf, c, gate)| {
            let obb = if furniture::shaped(c) {
                let (centre, half) = furniture::world_boxes(tf, c, 0.2)[0];
                Obb {
                    centre,
                    half,
                    rot: tf.rotation,
                }
            } else {
                Obb::of(tf, c)
            };
            (e, obb, c, gate)
        })
        .collect();
    let mut lines: Vec<(usize, usize, String)> = Vec::new();
    let flag = |lines: &mut Vec<(usize, usize, String)>, a: &Authored, fail: bool, text: String| {
        let order = if a.trail { 10_000 + a.scene } else { a.scene };
        lines.push((
            order,
            a.item,
            format!("  {} {text}", if fail { "FAIL" } else { "warn" }),
        ));
    };
    let mut groups: std::collections::BTreeMap<
        (usize, usize),
        Vec<(Obb, Collectible, Transform, Entity)>,
    > = Default::default();
    for (e, tf, c, a, gate) in &props {
        t.props += 1;
        let item = (!a.trail).then(|| &d.scenes[a.scene].items[a.item]);
        let indoors = !a.trail && d.scenes[a.scene].indoors;
        let name = &c.name;
        let obb = Obb::of(tf, c);
        let at = |v: Vec3| format!("({:.1}, {:.1}, {:.2})", -v.z, -v.x, v.y);
        let where_ = format!("#{} {name} at {}", a.n, at(tf.translation));
        let stairs = item.is_some_and(|i| matches!(i.arrangement, Arrangement::Stairs));
        let plank = item.is_some_and(|i| i.solid && i.pitch != 0.0);
        if stairs || plank {
            groups
                .entry((a.scene, a.item))
                .or_default()
                .push((obb, c.clone(), *tf, e));
        }
        // (a) the surface under the spawn ray, against the authored hint.
        if indoors {
            let origin = Vec3::new(tf.translation.x, a.hint + 4.0, tf.translation.z);
            let hit = collision.ray_body(origin, Dir3::NEG_Y, 8.0);
            let kind = hit.map_or("none", |h| kind_at(h.entity, origin - Vec3::Y * h.distance));
            let want = item.and_then(|i| i.on).unwrap_or(a.hint);
            let gap = a.ground - want;
            if gap.abs() > 0.3 {
                flag(
                    &mut lines,
                    a,
                    gap > 1.5 || gap < -1.5,
                    format!(
                        "{where_}: rests on {kind} at {:.2}, {gap:+.2} from the intended {:.2}{}",
                        a.ground,
                        want,
                        if gap > 1.5 {
                            " (a beam, a ledge or a storey above?)"
                        } else if gap < -1.5 {
                            " (a storey below?)"
                        } else {
                            ""
                        }
                    ),
                );
            }
        } else {
            // Outdoors the spawn samples terrain only: anything else under the prop is a roof,
            // a doodad hull or a building floor it has been dropped through.
            let origin = Vec3::new(tf.translation.x, a.ground + 6.0, tf.translation.z);
            if let Some(h) = collision.ray_body(origin, Dir3::NEG_Y, 12.0) {
                let y = origin.y - h.distance;
                if y > a.ground + 0.3 {
                    let kind = kind_at(h.entity, origin - Vec3::Y * h.distance);
                    flag(
                        &mut lines,
                        a,
                        y < a.ground + 5.0 && obb.top() > y,
                        format!(
                            "{where_}: under a {kind} surface at {y:.2} (terrain {:.2}); inside a building or a hull?",
                            a.ground
                        ),
                    );
                }
            }
        }
        // What the prop stands on: the world under its centre, or another solid's top.
        let bottom = obb.bottom();
        let (x, z) = (obb.centre.x, obb.centre.z);
        // From the prop's own top: a slab over it (a bench it lies under) is not its floor.
        let from = obb.top() + 0.01;
        let mut under = collision
            .ray_body(Vec3::new(x, from, z), Dir3::NEG_Y, from - bottom + 3.5)
            .map(|h| from - h.distance);
        for (o, b, _, _) in &boxes {
            if *o == e {
                continue;
            }
            if let Some(y) = b.ray_down(x, z, from) {
                under = Some(under.map_or(y, |u: f32| u.max(y)));
            }
        }
        // (b) the part of the box above that surface cutting into the world's collision (a wall, a
        // counter, a beam), and the centre inside another solid prop.
        let yaw = Quat::from_rotation_y(obb.rot.to_euler(EulerRot::YXZ).0);
        let (hx, hz) = (obb.along(yaw * Vec3::X), obb.along(yaw * Vec3::Z));
        let lo = bottom.max(under.unwrap_or(bottom)) + 0.04;
        let hi = obb.top();
        let blocker = item.is_some_and(|i| i.blocker);
        if hi - lo > 0.02 && !blocker {
            for (shrink, deep) in [(0.5_f32, true), (0.8, false)] {
                let half =
                    Vec3::new(hx * shrink, (hi - lo) * 0.5, hz * shrink).max(Vec3::splat(0.005));
                let centre = Vec3::new(x, (hi + lo) * 0.5, z);
                let hits = spatial.shape_intersections(
                    &Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z),
                    centre,
                    yaw,
                    &WorldCollision::body_filter(),
                );
                // Terrain under a sloped prop always touches its uphill edge; walls, buildings and
                // hulls are the faults. (A WMO walk collider carries layers, a terrain tile none.)
                let terrain = |h: &Entity| floors.get(*h).is_ok_and(|l| !walk(l));
                if let Some(&hit) = hits.iter().find(|h| !terrain(h)) {
                    flag(
                        &mut lines,
                        a,
                        deep,
                        format!(
                            "{where_}: box ({:.2}x{:.2}x{:.2}) {} {} collision above its floor",
                            2.0 * hx,
                            hi - bottom,
                            2.0 * hz,
                            if deep { "deep inside" } else { "cuts into" },
                            if floors.contains(hit) {
                                "wmo"
                            } else {
                                "doodad"
                            }
                        ),
                    );
                    break;
                }
            }
        }
        // A step overlapping the step below it, or a heap's props, are by design.
        let sibling = |o: Entity| {
            props
                .get(o)
                .is_ok_and(|p| p.3.scene == a.scene && p.3.item == a.item && p.3.trail == a.trail)
        };
        if let Some((_, _, other, _)) = boxes.iter().find(|(o, b, _, _)| {
            *o != e && !stairs && !plank && b.contains(obb.centre, 0.9) && !sibling(*o)
        }) {
            flag(
                &mut lines,
                a,
                true,
                format!("{where_}: centre inside solid {}", other.name),
            );
        }
        // (c) floating above, or sunk into, whatever is under it.
        if !stairs {
            let height = (hi - bottom).max(0.01);
            let extra = item.map_or(0.0, |i| (a.lift - i.lift).max(0.0));
            match under {
                None => flag(
                    &mut lines,
                    a,
                    true,
                    format!("{where_}: nothing under it within 3.5 yd"),
                ),
                Some(u) if bottom - u > 0.15 + extra => flag(
                    &mut lines,
                    a,
                    bottom - u > 0.5 + extra,
                    format!(
                        "{where_}: floats {:.2} above the surface at {u:.2}",
                        bottom - u
                    ),
                ),
                // A model whose origin is its middle (the bread halves) is half in its floor
                // wherever it goes: a model quirk, noted once per prop but not a placement fault.
                Some(u)
                    if u - bottom > (0.3 * height).max(0.05)
                        && !gate
                        && !((u - tf.translation.y).abs() < 0.03
                            && ((u - bottom) / height - 0.5).abs() < 0.06) =>
                {
                    flag(
                        &mut lines,
                        a,
                        u - bottom > 0.6 * height,
                        format!(
                            "{where_}: {:.0} % of its {height:.2} height is below the surface at {u:.2}",
                            100.0 * (u - bottom) / height
                        ),
                    )
                }
                _ => {}
            }
        }
    }
    for (a, model, p) in pending.unplaced() {
        flag(
            &mut lines,
            &a,
            true,
            format!(
                "#{} {model} at ({:.1}, {:.1}): never spawned (no floor under its ray)",
                a.n, -p.z, -p.x
            ),
        );
    }
    // (d) ramps: roll a ball up each one with the mover's rule.
    for ((si, ix), mut steps) in groups {
        t.ramps += 1;
        let item = &d.scenes[si].items[ix];
        let a = Authored {
            scene: si,
            item: ix,
            n: 0,
            trail: false,
            hint: 0.0,
            ground: 0.0,
            lift: 0.0,
        };
        let r = item.climb.unwrap_or(d.start_radius);
        let stairs = matches!(item.arrangement, Arrangement::Stairs);
        if crate::webenv::var("WOW_AZEROLL_PLACEMENT_DEBUG").is_some() {
            for (o, c, tf, _) in &steps {
                let pos = Vec3::new(o.centre.x, o.bottom() + r, o.centre.z);
                let near = solids
                    .iter()
                    .filter(|(_, stf, _, _)| stf.translation.distance(tf.translation) < 0.05)
                    .map(|(_, _, sc, gate)| format!("{}:{gate}:{:.2}", sc.name, sc.solid))
                    .collect::<Vec<_>>();
                lines.push((si, ix, format!(
                    "  dbg  step {} centre {:?} half {:?} top {:.2} rest {:?} floor {:?} in-solids {:?} contact {:?}",
                    c.name, o.centre, o.half, o.top(),
                    collect::solid_rest(tf, c, pos, r, true),
                    ball::floor_rest(&collision, pos, r),
                    near,
                    collect::contact(r, c.pickup),
                )));
            }
        }
        let own: Vec<Entity> = steps.iter().map(|s| s.3).collect();
        let (from, to, label, high, dir) = if stairs {
            // Steps in authored order: rising lift.
            steps.sort_by(|p, q| p.0.bottom().total_cmp(&q.0.bottom()));
            let first = steps[0].0;
            let last = steps[steps.len() - 1].0;
            let dir = units::flat(last.centre - first.centre).normalize_or(Vec3::X);
            let mut notes = Vec::new();
            let floor0 = collision
                .ray_body(first.centre + Vec3::Y * 0.3, Dir3::NEG_Y, 3.0)
                .map(|h| first.centre.y + 0.3 - h.distance);
            if let Some(f) = floor0 {
                if (first.bottom() - f).abs() > 0.06 {
                    notes.push(format!(
                        "first step's bottom {:+.2} from the floor {f:.2}",
                        first.bottom() - f
                    ));
                }
                if first.top() - f > ball::step_height(r) {
                    notes.push(format!(
                        "first step {:.2} tall > step {:.2} at r {r}",
                        first.top() - f,
                        ball::step_height(r)
                    ));
                }
            }
            for w in steps.windows(2) {
                let rise = w[1].0.top() - w[0].0.top();
                if rise > ball::step_height(r) {
                    notes.push(format!("a {rise:.2} riser"));
                }
            }
            // A Dislodge-state step is knocked loose by the ball that touches it (`collect`), even
            // though it supports a ball resting on it: a stair must be Wall-state at the radius it
            // serves (L2's 0.4-scale crate stairs at the start radius, live climbs 2026-11-03).
            let loose: Vec<_> = steps
                .iter()
                .map(|(_, c, _, _)| collect::contact(r, c.pickup))
                .filter(|c| *c != Contact::Wall)
                .collect();
            if loose.contains(&Contact::RollUp) {
                notes.push(format!("steps are edible at r {r}"));
            } else if !loose.is_empty() {
                notes.push(format!("steps are knocked loose (Dislodge) at r {r}"));
            }
            if !notes.is_empty() {
                flag(
                    &mut lines,
                    &a,
                    true,
                    format!("stairs: {}", notes.join("; ")),
                );
            }
            let back = first.along(dir) + r + 0.35;
            let on = last.along(dir) + r + 0.3;
            let mut high = last.centre + dir * last.along(dir);
            high.y = last.top();
            (
                first.centre - dir * back,
                last.centre + dir * on,
                format!(
                    "stairs of {} ({} steps, last top {:.2})",
                    steps[0].1.name,
                    steps.len(),
                    last.top()
                ),
                high,
                dir,
            )
        } else {
            let (o, c, _, _) = &steps[0];
            // The uphill direction: against the top face's horizontal lean.
            let normal = (0..3)
                .map(|i| {
                    let mut n = Vec3::ZERO;
                    n[i] = 1.0;
                    let n = o.rot * n;
                    if n.y < 0.0 {
                        -n
                    } else {
                        n
                    }
                })
                .max_by(|p, q| p.y.total_cmp(&q.y))
                .unwrap();
            let up = (-units::flat(normal)).normalize_or(Vec3::X);
            let len = o.along(up);
            let mut high = o.centre + up * len;
            high.y = o.top();
            (
                o.centre - up * (len + r + 0.35),
                o.centre + up * (len + r + 0.3),
                format!(
                    "plank {} (slope {:.2} rad, top {:.2})",
                    c.name,
                    normal.y.acos(),
                    o.top()
                ),
                high,
                up,
            )
        };
        let walls: Vec<(Obb, &str)> = boxes
            .iter()
            .filter(|(o, _, _, _)| !own.contains(o))
            .map(|(_, b, c, _)| (*b, c.name.as_ref()))
            .collect();
        // Where the walk starts and which way it rolls, in WoW terms, for a scripted-climb fixture
        // (`start`, `start_heading`: 0 north, pi/2 west).
        let label = format!(
            "{label} [foot ({:.2}, {:.2}, {:.2}) heading {:.3}]",
            -from.z,
            -from.x,
            from.y,
            (-dir.x).atan2(-dir.z)
        );
        match climb(&support, &collision, &walls, from, to, r) {
            Ok((base, end, target)) => {
                let up = end - base;
                if target.is_some_and(|t| t > end + 0.05) || up < 0.25 {
                    t.ramp_fails += 1;
                    // Where the surface this ramp serves actually starts: the first point past the
                    // high edge with a top within a step of it (world or another solid's box).
                    let step = ball::step_height(r);
                    let found = (0..75).find_map(|k| {
                        let p = high + dir * (0.04 * k as f32);
                        let from = high.y + 1.0;
                        let mut h = collision
                            .ray_body(Vec3::new(p.x, from, p.z), Dir3::NEG_Y, 4.0)
                            .map(|hit| from - hit.distance);
                        for (o, b, _, _) in &boxes {
                            if own.contains(o) {
                                continue;
                            }
                            if let Some(y) = b.ray_down(p.x, p.z, from) {
                                h = Some(h.map_or(y, |u: f32| u.max(y)));
                            }
                        }
                        h.filter(|h| *h > base - r + 0.25 && (*h - high.y).abs() <= step + 0.3)
                            .map(|h| (0.04 * k as f32, h))
                    });
                    let ws = |v: Vec3| format!("({:.2}, {:.2})", -v.z, -v.x);
                    let gap = found.map_or(
                        format!("no surface near its height within 3 yd past the high edge {}", ws(high)),
                        |(g, h)| format!("the nearest surface ({h:.2}) starts {g:.2} yd past the high edge {} heading {:.2}", ws(high), (-dir.z).atan2(-dir.x)),
                    );
                    lines.push((si, ix, format!("  note {gap}")));
                    flag(
                        &mut lines,
                        &a,
                        true,
                        format!(
                            "{label}: r {r} reaches the top end at {:.2} ({up:+.2} above the foot) but the surface beyond is {}",
                            end - r,
                            target.map_or("missing".into(), |t| format!("{:.2}", t - r))
                        ),
                    );
                } else if stairs && end - r > high.y + 0.01 && end - r <= high.y + 0.2 * r {
                    // The live mover stalls at a target's edge that stands just above the last step,
                    // inside the 0.85 r sweep's clearance (it meets the edge before any step-up; a
                    // taller final riser steps up like the others):
                    // scripted climb onto the captain's table, 2026-11-03.
                    t.ramp_fails += 1;
                    flag(
                        &mut lines,
                        &a,
                        true,
                        format!(
                            "{label}: the surface ({:.2}) stands {:.2} over the last step; give the last step >= 0.05 over it",
                            end - r,
                            end - r - high.y
                        ),
                    );
                } else {
                    lines.push((
                        si,
                        ix,
                        format!(
                            "  ok   {label}: r {r} climbs {up:.2} yd onto {:.2}",
                            end - r
                        ),
                    ));
                }
            }
            Err(why) => {
                t.ramp_fails += 1;
                flag(&mut lines, &a, true, format!("{label}: r {r} {why}"));
            }
        }
    }
    // `WOW_AZEROLL_PLACEMENT_GRID="x0 y0 x1 y1 ztop[;...]"` (WoW yards): the top surface every
    // 0.25 yd under `ztop`, world collision or (marked `*`) a solid prop's box, for authoring.
    if let Some(spec) = crate::webenv::var("WOW_AZEROLL_PLACEMENT_GRID") {
        for rect in spec.split(';') {
            let v: Vec<f32> = rect
                .split_whitespace()
                .filter_map(|w| w.parse().ok())
                .collect();
            let [x0, y0, x1, y1, ztop] = v[..] else {
                continue;
            };
            let _ = writeln!(
                out,
                "GRID x {x0}..{x1} (rows, north first) y {y0}..{y1} (columns, west first) under {ztop}"
            );
            let mut x = x0.max(x1);
            while x >= x0.min(x1) - 1e-3 {
                let _ = write!(out, "{x:9.2} |");
                let mut y = y0.max(y1);
                while y >= y0.min(y1) - 1e-3 {
                    let p = units::wow_to_bevy([x, y, ztop]);
                    let mut h = collision
                        .ray_body(p, Dir3::NEG_Y, 12.0)
                        .map(|hit| (ztop - hit.distance, ' '));
                    for (_, b, _, _) in &boxes {
                        if let Some(top) = b.ray_down(p.x, p.z, ztop) {
                            if h.is_none_or(|(u, _)| top > u) {
                                h = Some((top, '*'));
                            }
                        }
                    }
                    let _ = match h {
                        Some((h, m)) => write!(out, "{h:6.2}{m}"),
                        None => write!(out, "   -   "),
                    };
                    y -= 0.25;
                }
                let _ = writeln!(out);
                x -= 0.25;
            }
            let mut y = y0.max(y1);
            let _ = write!(out, "   y     |");
            while y >= y0.min(y1) - 1e-3 {
                let _ = write!(out, "{y:6.2} ");
                y -= 0.25;
            }
            let _ = writeln!(out);
        }
    }
    lines.sort_by_key(|l| (l.0, l.1));
    let mut scene_of = usize::MAX;
    for (order, ix, text) in &lines {
        if text.starts_with("  FAIL") {
            t.fails += 1;
        } else if text.starts_with("  warn") {
            t.warns += 1;
        }
        if *order != scene_of {
            scene_of = *order;
            let name = if *order >= 10_000 {
                format!("trail {}", d.trails[order - 10_000].name)
            } else {
                d.scenes[*order].name.clone()
            };
            let _ = writeln!(out, "{name}");
        }
        let _ = writeln!(out, "  [item {ix}]{text}");
    }
    let head = format!(
        "azeroll PLACEMENT {id}: {} props, {} ramps ({} fail), {} FAIL, {} warn",
        t.props, t.ramps, t.ramp_fails, t.fails, t.warns
    );
    info!("{head}");
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = crate::webenv::var("WOW_AZEROLL_PLACEMENT")
            .filter(|v| v.ends_with(".txt"))
            .unwrap_or_else(|| format!("/tmp/azeroll-placement-{id}.txt"));
        if let Err(e) = std::fs::write(&path, format!("{head}\n{out}")) {
            error!("azeroll PLACEMENT write {path}: {e}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    info!("{out}");
    if crate::webenv::var("WOW_AZEROLL_PLACEMENT_EXIT").is_some() {
        exit.write(AppExit::Success);
    }
}

/// Roll a ball of radius `r` in a straight line from `from` (on whatever floor is there) to `to`,
/// with the mover's step-up rule and its sphere sweep against the world. `Ok((foot, end, beyond))`:
/// the centre's rest at the foot and at the end, and the highest surface rest under the end the
/// ball could stand on from above. `Err` names the first riser or wall that stops it.
fn climb(
    support: &impl Fn(Vec3, f32) -> Option<f32>,
    collision: &WorldCollision,
    walls: &[(Obb, &str)],
    from: Vec3,
    to: Vec3,
    r: f32,
) -> Result<(f32, f32, Option<f32>), String> {
    let step = ball::step_height(r);
    let mut pos = Vec3::new(from.x, from.y + r, from.z);
    pos.y = support(pos, r).ok_or("finds no floor at the foot")?;
    let foot = pos.y;
    let line = units::flat(to - from);
    let len = line.length();
    let dir = line / len.max(1e-3);
    let n = (len / 0.04).ceil() as usize;
    let sphere = Collider::sphere(0.85 * r);
    for i in 1..=n {
        let s = len * i as f32 / n as f32;
        let mut ahead = from + dir * s;
        ahead.y = pos.y;
        let seen = support(ahead, r);
        // What a body this tall would meet: any top up to the ball's crown.
        let crown = support(
            Vec3::new(ahead.x, pos.y + 2.0 * r - step - 0.05, ahead.z),
            r,
        );
        if let Some(top) = crown {
            if top > seen.unwrap_or(f32::NEG_INFINITY) + 0.02 && top > pos.y + 0.01 {
                return Err(format!(
                    "is stopped {s:.2} yd along by a riser needing a {:.2} rise (limit {step:.2}) at bottom {:.2}",
                    top - pos.y,
                    pos.y - r
                ));
            }
        }
        if let Some(rest) = seen {
            if rest > pos.y && rest - pos.y <= step {
                pos.y = rest;
            }
        }
        let movement = Vec3::new(ahead.x - pos.x, 0.0, ahead.z - pos.z);
        if let Some(hit) = collision.cast_body(&sphere, pos, movement, 0.0) {
            if hit.normal1.y.abs() < 0.6 && hit.collision_distance < movement.length() - 1e-4 {
                return Err(format!(
                    "hits a wall {s:.2} yd along at bottom {:.2} (normal {:.2?}, at ({:.2}, {:.2}, {:.2}))",
                    pos.y - r,
                    hit.normal1,
                    -hit.point1.z,
                    -hit.point1.x,
                    hit.point1.y
                ));
            }
        }
        // A solid prop the ball cannot step onto pushes it back (`collect::solid_push`): a chair
        // beside a stair's foot, a crate in its way.
        for (b, name) in walls {
            if b.top() <= pos.y - r + step + 0.02 || b.bottom() >= pos.y + r {
                continue;
            }
            let l = b.rot.inverse() * (Vec3::new(ahead.x, b.centre.y, ahead.z) - b.centre);
            let near = l.clamp(-b.half, b.half);
            if Vec3::new(l.x - near.x, 0.0, l.z - near.z).length() < 0.85 * r {
                return Err(format!(
                    "is pushed back {s:.2} yd along by the solid {name} (top {:.2}, ball bottom {:.2})",
                    b.top(),
                    pos.y - r
                ));
            }
        }
        pos.x = ahead.x;
        pos.z = ahead.z;
        match support(pos, r) {
            Some(y) => pos.y = y,
            None => return Err(format!("loses the floor {s:.2} yd along")),
        }
    }
    let beyond = support(Vec3::new(pos.x, pos.y + 1.0, pos.z), r);
    Ok((foot, pos.y, beyond))
}
