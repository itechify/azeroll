//! The heart of the game: what the ball does to everything it touches.
//!
//! Every frame while playing, each [`Collectible`] not already stuck to the ball is tested against
//! it. Diameter >= 2.15 times the maximum full axis: it is **rolled up**, re-parented
//! under [`BallSpin`] at its contact pose, and the ball grows by filled bbox volume. Intermediate
//! objects (diameter/axis >= 1.5) dislodge into a ballistic hop. Too big but
//! `solid`: the ball **bumps** off its trunk, and a hard enough bump **sheds** the most recent small
//! trophies. Trophies the debris shell has hidden are **buried** (despawned), which keeps the
//! live count bounded however long the run goes.
//!
//! Terrain and engine-owned geometry are the ball's own business ([`super::ball`]); this module
//! only knows the mode's collectibles, which have no engine colliders (decision D6).

use benilla_world::schedule::WorldStage;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::ball::{top_speed, CorePop};
use super::level::{COLLECT_RATIO, DISLODGE_RATIO, FILL_FACTOR};
use super::{
    Attached, BallSpin, BallState, Bumped, Collected, Collectible, Phase, Rng, Run, RunEntity,
    Source,
};
use crate::entities::Carried;
use crate::net::NetEntity;

/// Permanent blocker footprints hide bait covered by the authored replacement.
/// Project the target's own extents into the box frame: a tall harness/post
/// protruding above a haystack must not become a fresh target after growth.
/// Separate storeys and food on any upward walkable face remain eligible.
pub(super) fn hidden_by_blocker(
    target_tf: &Transform,
    target: &Collectible,
    blocker_tf: &Transform,
    blocker: &Collectible,
) -> bool {
    if blocker.pickup.is_finite() || blocker.solid <= 0. {
        return false;
    }
    let inverse = blocker_tf.rotation.inverse();
    let centre = inverse
        * (target_tf.transform_point(target.center) - blocker_tf.transform_point(blocker.center));
    let rotation = inverse * target_tf.rotation;
    let half = target.half_extents * target_tf.scale.abs();
    let extent = (rotation * Vec3::X).abs() * half.x
        + (rotation * Vec3::Y).abs() * half.y
        + (rotation * Vec3::Z).abs() * half.z;
    let bounds = blocker.half_extents * blocker_tf.scale.abs();
    if !centre.abs().cmple(bounds + extent).all() {
        return false;
    }
    for (axis, direction) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
        let up = (blocker_tf.rotation * direction).y;
        if up.abs() > 0.6 && up.signum() * centre[axis] - extent[axis] >= bounds[axis] - 0.02 {
            return false;
        }
    }
    true
}

/// A thing touches the ball when its centre is within `radius + size * TOUCH` of the ball's.
const TOUCH: f32 = 0.9;

use super::lod::MAX_TROPHIES;
/// Bounce off a solid thing keeps this much of the closing speed.
const RESTITUTION: f32 = 0.35;
/// A bump faster than this (yards/s along the normal) is a [`Bumped`] message and a stun.
const BUMP_MIN: f32 = 2.0;
/// The stun a bump causes, seconds per yard/s of closing speed, and its cap.
const STUN_PER_SPEED: f32 = 0.05;
const STUN_CAP: f32 = 0.6;
/// A bump faster than this fraction of the top speed sheds trophies.
const SHED_SPEED: f32 = 0.6;
/// Only trophies at most this fraction of the radius are shed (the little ones, not the barrel
/// that took a minute to find).
const SHED_MAX_SIZE: f32 = 0.35;
/// A shed trophy cannot be rolled up again for this long, seconds.
const SHED_LOCKOUT: f32 = 1.5;
/// Gravity for a shed trophy's hop, yards/s².
const SHED_GRAVITY: f32 = 20.0;
/// The most a single hard bump sheds.
const SHED_MAX_COUNT: usize = 4;
/// Pickup squash: added per pickup, and the cap.
const POP_BASE: f32 = 0.03;
const POP_PER_SIZE: f32 = 0.08;
const POP_CAP: f32 = 0.14;

/// A trophy knocked off the ball: it hops to the ground and cannot be collected until `until`
/// (seconds of [`Time::elapsed_secs`]). Removed once it has landed and the time is up.
#[derive(Component)]
pub(super) struct Shed {
    pub until: f32,
    /// Its flight velocity; zero once it has landed.
    pub vel: Vec3,
}

/// Authored planted props keep their solid base until edible instead of being
/// launched in the intermediate dislodge band. Pickup size and credit are unchanged.
#[derive(Component)]
pub(super) struct Planted;

// Preserve existing sibling imports while the shared vocabulary lives in mod.rs.
pub(super) use super::{Contact, Ram};
use super::{TouchState, Touched};

/// The knock a ram gives the ball, yards/s along `Ram::dir`, and the small hop with it.
const RAM_KNOCK: f32 = 7.0;
const RAM_HOP: f32 = 2.5;
const RAM_STUN: f32 = 0.35;
/// A ram counts as a bump this fraction of the ball's top speed: above [`SHED_SPEED`], so it sheds
/// (two trophies at most speeds).
const RAM_STRENGTH: f32 = 0.8;

/// The attached things, as the shedding and burying passes see them.
type Trophies<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Attached,
        &'static Collectible,
    ),
>;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (restore_reactions, seat_debug_trophies)
            .chain()
            .before(super::lod::CacheClump)
            .before(WorldStage::Input),
    )
    .add_systems(Update, (shed_fly, log_bumps).after(WorldStage::Input))
    .add_systems(
        Update,
        animate_reactions.after(shed_fly).after(WorldStage::Input),
    );
}

/// Long axes lie on the tangent plane; retain the model's other orientation cues.
fn tangent_rotation(rotation: Quat, half: Vec3, normal: Vec3, twist: f32) -> Quat {
    let axis = if half.x >= half.y && half.x >= half.z {
        Vec3::X
    } else if half.y >= half.z {
        Vec3::Y
    } else {
        Vec3::Z
    };
    if half.max_element() < half.min_element().max(0.001) * 1.6 {
        return rotation;
    }
    let long = rotation * axis;
    let tangent = (long - normal * long.dot(normal)).normalize_or(normal.any_orthonormal_vector());
    Quat::from_axis_angle(normal, twist) * Quat::from_rotation_arc(long, tangent) * rotation
}

/// Outer intersection of a radial ray with a box. Parallel slabs must contain the ray.
fn ray_box(origin: Vec3, dir: Vec3, half: Vec3) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = f32::INFINITY;
    for i in 0..3 {
        if dir[i].abs() < 1e-6 {
            if origin[i].abs() > half[i] {
                return None;
            }
        } else {
            let a = (-half[i] - origin[i]) / dir[i];
            let b = (half[i] - origin[i]) / dir[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
    }
    (far >= near && far.is_finite()).then_some(far)
}

/// Five samples along the longest axis find neighbouring boxes, with thickness overlap so
/// trophies nestle rather than balance on bounding spheres. Same-frame pickups participate.
fn seat(
    thing: &Collectible,
    scale: Vec3,
    rotation: Quat,
    dir: Vec3,
    shell: f32,
    radius: f32,
    trophies: &[(Transform, Collectible)],
) -> Transform {
    let half = thing.half_extents * scale.abs();
    let axis = if half.x >= half.y && half.x >= half.z {
        Vec3::X
    } else if half.y >= half.z {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let thickness = half.min_element().max(0.005);
    let mut radial = shell + thickness * 0.6;
    for (tf, other) in trophies {
        let inv = tf.rotation.inverse();
        let centre = tf.transform_point(other.center);
        let bounds = other.half_extents * tf.scale.abs() + Vec3::splat(thickness * 0.6);
        for fraction in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let sample = rotation * axis * half.max_element() * fraction;
            if let Some(hit) = ray_box(inv * (sample - centre), inv * dir, bounds) {
                radial = radial.max(hit);
            }
        }
    }
    // Adhesion may find a long branch of nearly empty model boxes. Compress that contact
    // into the filled-volume envelope instead of growing a loose chain farther each pickup.
    let radial = radial.min((radius - thickness * 0.2).max(shell + thickness * 0.35));
    Transform {
        translation: dir * radial - rotation * (scale * thing.center),
        rotation,
        scale,
    }
}

/// Debug-fill fixtures use the shipping seating algorithm too, without changing their credit.
#[allow(clippy::type_complexity)] // One query owns both change detection and mutable seating poses.
fn seat_debug_trophies(
    mut commands: Commands,
    ball: Res<BallState>,
    mut rng: ResMut<Rng>,
    mut trophies: Query<(
        Entity,
        &mut Transform,
        &mut Attached,
        &Collectible,
        Has<Reaction>,
    )>,
) {
    let _profile = super::profile::span("collect::seat_debug_trophies");
    let fresh: Vec<_> = trophies
        .iter_mut()
        .filter(|(_, _, a, _, reacting)| a.is_added() && !reacting)
        .map(|(entity, _, _, _, _)| entity)
        .collect();
    if fresh.is_empty() {
        return;
    }
    let mut seats: Vec<_> = trophies
        .iter()
        .filter(|(e, _, _, _, _)| !fresh.contains(e))
        .map(|(_, tf, _, c, _)| (*tf, c.clone()))
        .collect();
    for entity in fresh {
        let Ok((_, mut tf, mut attached, thing, _)) = trophies.get_mut(entity) else {
            continue;
        };
        let dir = tf.transform_point(thing.center).normalize_or(Vec3::Y);
        let rotation = tangent_rotation(
            tf.rotation,
            thing.half_extents * tf.scale.abs(),
            dir,
            rng.range(-0.5, 0.5),
        );
        *tf = seat(
            thing,
            tf.scale,
            rotation,
            dir,
            super::ball::shell_radius(ball.start_radius, ball.buried_volume),
            ball.radius,
            &seats,
        );
        attached.radial = tf.transform_point(thing.center).length();
        seats.push((*tf, thing.clone()));
        commands.entity(entity).insert(Reaction::pickup());
    }
}

/// Presentation-only pose, restored before the next physical contact/CoM pass.
#[derive(Component)]
struct Reaction {
    age: f32,
    pickup: bool,
    axis: Vec3,
    strength: f32,
    rest: Option<Transform>,
}
impl Reaction {
    fn pickup() -> Self {
        Self {
            age: 0.0,
            pickup: true,
            axis: Vec3::Y,
            strength: 1.0,
            rest: None,
        }
    }
}
fn restore_reactions(mut query: Query<(&mut Transform, &mut Reaction)>) {
    for (mut tf, mut reaction) in &mut query {
        if let Some(rest) = reaction.rest.take() {
            *tf = rest;
        }
    }
}
#[allow(clippy::type_complexity)] // Carried/loose reaction poses are disjoint from ballistic motion.
fn animate_reactions(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    ball: Res<BallState>,
    mut touches: MessageReader<Touched>,
    mut query: Query<
        (
            Entity,
            &mut Transform,
            &Collectible,
            Option<&Attached>,
            &mut Reaction,
        ),
        Without<Shed>,
    >,
    loose: Query<(&Collectible, Has<Shed>), Without<Reaction>>,
) {
    if *phase.get() != Phase::Playing {
        touches.clear();
        return;
    }
    // A live reaction also supplies a per-entity 0.5s cooldown. Bound the global budget.
    for touch in touches.read().take(32) {
        if let Ok((thing, false)) = loose.get(touch.entity) {
            commands.entity(touch.entity).insert(Reaction {
                age: 0.0,
                pickup: false,
                axis: Vec3::Y.cross(touch.point - ball.pos).normalize_or(Vec3::X),
                strength: (touch.strength / thing.pickup.max(0.2)).clamp(0.3, 1.0),
                rest: None,
            });
        }
    }
    for (entity, mut tf, thing, attached, mut reaction) in &mut query {
        reaction.age += time.delta_secs();
        if reaction.age > 0.5 {
            commands.entity(entity).remove::<Reaction>();
            continue;
        }
        if reaction.pickup != attached.is_some() {
            continue;
        }
        reaction.rest = Some(*tf);
        let centre = tf.transform_point(thing.center);
        if reaction.pickup {
            let wave = (reaction.age * 28.0).sin() * (-reaction.age * 12.0).exp();
            tf.scale *= 1.0 - 0.22 * wave;
            // Outward settling pulse makes the contact lobe briefly bulge.
            tf.translation = centre + centre.normalize_or(Vec3::Y) * thing.size * 0.18 * wave
                - tf.rotation * (tf.scale * thing.center);
        } else {
            let angle = 0.10
                * reaction.strength
                * (reaction.age * 48.0).sin()
                * (-reaction.age * 9.0).exp();
            tf.rotation = Quat::from_axis_angle(reaction.axis, angle) * tf.rotation;
            tf.translation = centre - tf.rotation * (tf.scale * thing.center);
        }
    }
}

/// How a ball of this radius treats an object whose longest axis is `axis`: `D >= 2.15 axis`
/// rolls it up, `1.5 axis <= D` knocks it loose, anything smaller meets it as a wall.
pub(super) fn contact(radius: f32, axis: f32) -> Contact {
    if axis <= COLLECT_RATIO * radius {
        Contact::RollUp
    } else if 2.0 * radius >= DISLODGE_RATIO * axis {
        Contact::Dislodge
    } else {
        Contact::Wall
    }
}

fn loose_contact(radius: f32, axis: f32, planted: bool) -> Contact {
    match contact(radius, axis) {
        Contact::Dislodge if planted => Contact::Wall,
        state => state,
    }
}

/// Claimed fences use the same thin oriented bounds as authored gates.
pub(super) fn boxed_solid(thing: &Collectible, gate: bool) -> bool {
    gate || (thing.solid > 0.0 && !super::props::trunk_solid(&thing.name))
}

/// D29: loose Wall/Dislodge objects support the sphere until they move or become edible.
pub(super) fn solid_rest(
    tf: &Transform,
    c: &Collectible,
    pos: Vec3,
    r: f32,
    gate: bool,
) -> Option<f32> {
    if c.solid <= 0. || contact(r, c.pickup) == Contact::RollUp {
        return None;
    }
    // Tree collision remains a basal trunk, never a canopy-sized platform.
    if !gate && super::props::trunk_solid(&c.name) {
        return None;
    }
    let center = tf.transform_point(c.center);
    let half = c.half_extents * tf.scale.abs();
    if super::units::flat(center - pos).length() > half.length() + r {
        return None;
    }
    if super::furniture::shaped(c) {
        // Leg tops meet the underside of the slab: they are internal faces,
        // not platforms that can hold a sphere inside the seat.
        super::furniture::world_boxes(tf, c, r)
            .into_iter()
            .take(1)
            .filter_map(|(center, half)| {
                super::furniture::box_rest(center, half, tf.rotation, pos, r)
            })
            .max_by(f32::total_cmp)
    } else {
        super::furniture::box_rest(center, half, tf.rotation, pos, r)
    }
}

/// Horizontal correction against the same authored solid used by recovery clearance.
/// A ball whose centre is inside a fence must exit through its nearest face, including
/// the distance to that face; a radius-only correction can leave it inside every frame.
pub(super) fn solid_push(
    tf: &Transform,
    thing: &Collectible,
    pos: Vec3,
    radius: f32,
    boxed: bool,
) -> Option<Vec3> {
    solid_push_with_contact(tf, thing, pos, radius, boxed, contact(radius, thing.pickup))
}

fn solid_push_with_contact(
    tf: &Transform,
    thing: &Collectible,
    pos: Vec3,
    radius: f32,
    boxed: bool,
    state: Contact,
) -> Option<Vec3> {
    if thing.solid <= 0.0 || state != Contact::Wall {
        return None;
    }
    // A supported sphere is outside the side wall, including the rounded edge cap.
    if solid_rest(tf, thing, pos, radius, boxed).is_some_and(|h| pos.y >= h - 0.003) {
        return None;
    }
    if super::furniture::shaped(thing) {
        let reach = radius + (thing.half_extents * tf.scale.abs()).length();
        if pos.distance_squared(tf.transform_point(thing.center)) >= reach * reach {
            return None;
        }
        let mut corrected = pos;
        for (center, half) in super::furniture::world_boxes(tf, thing, radius) {
            let mut part = thing.clone();
            part.source = Source::Spawned;
            part.name = "".into();
            part.center = Vec3::ZERO;
            part.half_extents = half;
            let box_tf = Transform::from_translation(center).with_rotation(tf.rotation);
            if let Some(push) =
                solid_push_with_contact(&box_tf, &part, corrected, radius, true, state)
            {
                corrected += push;
            }
        }
        return (corrected != pos).then_some(corrected - pos);
    }
    let mut centre = tf.transform_point(thing.center);
    if super::props::trunk_solid(&thing.name) {
        centre.x = tf.translation.x;
        centre.z = tf.translation.z;
    }
    if boxed_solid(thing, boxed) {
        let half = thing.half_extents * tf.scale.abs();
        let local = tf.rotation.inverse() * (pos - centre);
        if local.y - radius >= half.y || local.y + radius <= -half.y {
            return None;
        }
        let offset = Vec3::new(
            local.x - local.x.clamp(-half.x, half.x),
            0.0,
            local.z - local.z.clamp(-half.z, half.z),
        );
        let distance = offset.length();
        if distance >= radius {
            return None;
        }
        let push = if distance > 1e-6 {
            offset * ((radius - distance) / distance)
        } else if half.x - local.x.abs() < half.z - local.z.abs() {
            Vec3::X
                * if local.x >= 0.0 {
                    half.x - local.x + radius
                } else {
                    -half.x - local.x - radius
                }
        } else {
            Vec3::Z
                * if local.z >= 0.0 {
                    half.z - local.z + radius
                } else {
                    -half.z - local.z - radius
                }
        };
        Some(tf.rotation * push)
    } else {
        if pos.y - radius >= centre.y + thing.size || pos.y + radius <= centre.y - thing.size {
            return None;
        }
        let offset = Vec3::new(pos.x - centre.x, 0.0, pos.z - centre.z);
        let distance = offset.length();
        let reach = radius + thing.solid;
        (distance < reach).then(|| offset.normalize_or(Vec3::X) * (reach - distance))
    }
}

/// Resolve a spawn/recovery overlap before an engine sweep begins inside a wall.
/// Reuse the live solid shapes, but Dislodge solids also need clearance at spawn.
pub(super) fn resolve_solid_overlaps<'a>(
    start: Vec3,
    r: f32,
    solids: impl Iterator<Item = (&'a Transform, &'a Collectible, bool)>,
) -> Vec3 {
    let solids: Vec<_> = solids
        .filter(|(_, c, _)| c.solid > 0. && contact(r, c.pickup) != Contact::RollUp)
        .collect();
    let push = |p: Vec3, &(tf, c, gate): &(&Transform, &Collectible, bool)| {
        if p.distance_squared(tf.transform_point(c.center))
            > ((c.half_extents * tf.scale.abs()).length() + r).powi(2)
        {
            return None;
        }
        if contact(r, c.pickup) == Contact::Wall {
            solid_push(tf, c, p, r, gate)
        } else {
            let mut wall = c.clone();
            wall.pickup = f32::INFINITY;
            solid_push(tf, &wall, p, r, gate)
        }
    };
    let clear = |p| solids.iter().all(|s| push(p, s).is_none());
    let mut pos = start;
    for _ in 0..16 {
        let Some(delta) = solids
            .iter()
            .filter_map(|s| push(pos, s))
            .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        else {
            return pos;
        };
        pos += delta + delta.normalize_or_zero() * 0.002;
    }
    // Adjacent boxes can push toward each other. Try the outer faces of the
    // intersecting boxes and choose the nearest position clear of their union.
    let mut candidates = Vec::new();
    for &(tf, c, gate) in &solids {
        if push(start, &(tf, c, gate)).is_none() && push(pos, &(tf, c, gate)).is_none() {
            continue;
        }
        let centre = tf.transform_point(c.center);
        let half = c.half_extents * tf.scale.abs();
        let local = tf.rotation.inverse() * (start - centre);
        for axis in [0, 2] {
            for sign in [-1., 1.] {
                let mut p = local;
                p[axis] = sign * (half[axis] + r + 0.002);
                candidates.push(centre + tf.rotation * p);
            }
        }
    }
    if let Some(p) = candidates.into_iter().filter(|p| clear(*p)).min_by(|a, b| {
        a.distance_squared(start)
            .total_cmp(&b.distance_squared(start))
    }) {
        return p;
    }
    warn!("azeroll solid overlap: adjacent solids still intersect after bounded resolution at {pos:?} r={r}");
    pos
}

/// Conservative volume for a box with this full maximum axis (used by D10 validation).
pub(super) fn growth(axis: f32, _radius: f32) -> f32 {
    axis.powi(3) * FILL_FACTOR * 3.0 / (4.0 * std::f32::consts::PI)
}

/// Fully enclosed well below the collision surface; keep protruding trophies alive.
fn buried(trophy: &Attached, radius: f32) -> bool {
    trophy.radial + trophy.size < 0.62 * radius
}

/// Conservative broadphase shared by the runtime and its differential geometry test.
fn outside_clump(to: Vec3, radius: f32, trophy_reach: f32, size: f32) -> bool {
    to.length_squared() >= (radius.max(trophy_reach) + size * TOUCH).powi(2)
}

/// Roll up, bump, shed and bury, once a frame while playing.
#[allow(clippy::type_complexity)] // Loose collectibles exclude carried and shed transforms.
pub(super) fn collect(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut ball: ResMut<BallState>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut pop: ResMut<CorePop>,
    mut collected: MessageWriter<Collected>,
    mut bumped: MessageWriter<Bumped>,
    mut touched: MessageWriter<Touched>,
    maintenance: (Local<std::collections::HashMap<Entity, f32>>, Local<f32>),
    mut rams: MessageReader<Ram>,
    spin_q: Query<Entity, With<BallSpin>>,
    things: Query<
        (
            Entity,
            &Transform,
            &Collectible,
            Has<NetEntity>,
            Has<super::design::Gate>,
            Has<Planted>,
        ),
        (
            Without<Attached>,
            Without<Shed>,
            Without<super::city::CityTarget>,
        ),
    >,
    clump: (Trophies, Res<super::lod::Clump>),
) {
    let _profile = super::profile::span("collect::collect");
    let (mut touch_cooldowns, mut next_bury) = maintenance;
    let (trophies, clump) = clump;
    if *phase.get() != Phase::Playing || loading.covering() {
        return;
    }
    let Ok(spin_entity) = spin_q.single() else {
        return;
    };
    let ball: &mut BallState = &mut ball;
    let spin = ball.spin;
    let unspin = spin.inverse();
    let mut hardest = 0.0_f32;
    let mut added = Vec::new();
    // Seat data is needed only on pickup. A quiet frame must not clone every trophy name.
    let mut seats = None;
    // Conservative envelope for *all* adhesive boxes (not just the collision sphere).
    let trophy_reach = clump.reach;
    let now = time.elapsed_secs();
    touch_cooldowns.retain(|_, until| *until > now);
    let mut emit_touch = |event: Touched| {
        if let std::collections::hash_map::Entry::Vacant(entry) =
            touch_cooldowns.entry(event.entity)
        {
            entry.insert(now + 0.35);
            touched.write(event);
        }
    };

    for ram in rams.read() {
        let away = Vec3::new(ram.dir.x, 0.0, ram.dir.z).normalize_or(Vec3::X);
        ball.vel.x = away.x * RAM_KNOCK;
        ball.vel.z = away.z * RAM_KNOCK;
        ball.vel.y = ball.vel.y.max(RAM_HOP);
        ball.stun = ball.stun.max(RAM_STUN);
        info!("azeroll: {} rams the ball", ram.name);
        let strength = RAM_STRENGTH * top_speed(ball.radius);
        hardest = hardest.max(strength);
        bumped.write(Bumped {
            entity: Some(ram.entity),
            name: ram.name.clone(),
            strength,
        });
    }

    for (entity, tf, thing, is_unit, is_gate, planted) in &things {
        let radius = ball.radius;
        let centre = tf.transform_point(thing.center);
        let to = centre - ball.pos;

        let state = loose_contact(radius, thing.pickup, planted);
        if state == Contact::RollUp {
            // Triangle inequality: outside this sphere no point on any trophy OBB can touch.
            // Keep the exact OBB test below for protrusions inside the envelope.
            if outside_clump(to, radius, trophy_reach, thing.size) {
                continue;
            }
            // Protruding trophies are adhesive too: test their oriented boxes. Otherwise a
            // pole lifts the core out of reach of every small object on the ground.
            let touches_trophy = || {
                trophies.iter().any(|(_, tf, _, carried)| {
                    let local = tf.rotation.inverse() * (unspin * to - tf.translation);
                    let half = carried.half_extents * tf.scale.abs();
                    let gap = local - (tf.scale * carried.center);
                    (gap - gap.clamp(-half, half)).length() < thing.size * TOUCH
                })
            };
            if to.length() >= radius + thing.size * TOUCH && !touches_trophy() {
                continue;
            }
            // Tangent-seat the model box, then sweep inward against the actual clump.
            let dir = (unspin * to).normalize_or(Vec3::Y);
            let rotation = tangent_rotation(
                unspin * tf.rotation,
                thing.half_extents * tf.scale.abs(),
                dir,
                rng.range(-0.5, 0.5),
            );
            let shell = super::ball::shell_radius(ball.start_radius, ball.buried_volume);
            let seats = seats.get_or_insert_with(|| {
                trophies
                    .iter()
                    .map(|(_, t, _, c)| (*t, c.clone()))
                    .collect::<Vec<_>>()
            });
            let local = seat(thing, tf.scale, rotation, dir, shell, radius, seats);
            let radial = local.transform_point(thing.center).length();
            let volume = thing.volume(tf.scale);
            let attachment = Attached {
                radial,
                volume,
                size: thing.size,
                at: run.elapsed,
            };
            let mut stuck = commands.entity(entity);
            stuck.insert((
                ChildOf(spin_entity),
                local,
                attachment,
                RunEntity,
                Reaction::pickup(),
            ));
            seats.push((local, thing.clone()));
            if is_unit {
                stuck.insert(Carried);
            }

            // Grow by volume.
            ball.volume += volume;
            ball.radius = ball.volume.cbrt();
            pop.0 = (pop.0 + POP_BASE + POP_PER_SIZE * thing.size / radius).min(POP_CAP);

            added.push((entity, attachment));
            run.count += 1;
            if thing.kind != super::Kind::Prop {
                info!(
                    "azeroll: rolled up {} (size {:.2}) at ball r={:.2}, {:.1}s",
                    thing.name, thing.size, radius, run.elapsed
                );
            }
            *run.log.entry(thing.name.clone()).or_insert(0) += 1;
            if run
                .biggest
                .as_ref()
                .is_none_or(|(_, size)| thing.size > *size)
            {
                run.biggest = Some((thing.name.clone(), thing.size));
            }
            collected.write(Collected {
                entity,
                name: thing.name.clone(),
                kind: thing.kind,
                source: thing.source,
                size: thing.size,
                pickup: thing.pickup,
            });
        } else if state == Contact::Dislodge {
            // Intermediate objects yield to a moving ball, then settle for a later pickup.
            let local = tf.rotation.inverse() * (ball.pos - tf.translation);
            let half = thing.half_extents * tf.scale.abs();
            let gap = local - tf.scale * thing.center;
            let distance = (gap - gap.clamp(-half, half)).length();
            let away = Vec3::new(to.x, 0.0, to.z).normalize_or(Vec3::X);
            let closing = ball.vel.dot(away);
            if distance < radius {
                emit_touch(Touched {
                    entity,
                    state: TouchState::Dislodge,
                    strength: closing.max(0.0),
                    point: ball.pos + to.normalize_or(Vec3::Y) * radius,
                });
            }
            if distance < radius && closing > 0.5 {
                if is_unit {
                    commands.entity(entity).insert(Carried);
                }
                commands.entity(entity).insert(Shed {
                    until: time.elapsed_secs() + SHED_LOCKOUT,
                    vel: away * (closing * 0.75 + 1.5) + Vec3::Y * if is_unit { 1.8 } else { 3.5 },
                });
                ball.vel *= 0.75;
                info!(
                    "azeroll dislodge: {} (axis {:.2}) at ball r={:.2}, closing {:.1}, {:.1}s",
                    thing.name, thing.pickup, radius, closing, run.elapsed
                );
            }
        } else if thing.solid > 0.0 {
            let Some(push) = solid_push_with_contact(tf, thing, ball.pos, radius, is_gate, state)
            else {
                continue;
            };
            if crate::webenv::var("WOW_AZEROLL_STALL_DEBUG").is_some()
                && time.elapsed_secs().fract() < time.delta_secs()
            {
                info!(
                    "azeroll solid: name={} boxed={} pos={:?} centre={:?} half={:?} local={:?} push={push:?}",
                    thing.name,
                    boxed_solid(thing, is_gate),
                    ball.pos,
                    centre,
                    thing.half_extents * tf.scale.abs(),
                    tf.rotation.inverse() * (ball.pos - centre)
                );
            }
            let n = push.normalize_or(Vec3::X);
            emit_touch(Touched {
                entity,
                state: TouchState::Wall,
                strength: (-ball.vel.dot(n)).max(0.0),
                point: ball.pos - n * radius,
            });
            ball.pos += push;
            let into = ball.vel.x * n.x + ball.vel.z * n.z;
            if into < 0.0 {
                let closing = -into;
                ball.vel.x -= n.x * into * (1.0 + RESTITUTION);
                ball.vel.z -= n.z * into * (1.0 + RESTITUTION);
                if closing > BUMP_MIN {
                    ball.stun = ball.stun.max((closing * STUN_PER_SPEED).min(STUN_CAP));
                    hardest = hardest.max(closing);
                    bumped.write(Bumped {
                        entity: Some(entity),
                        name: thing.name.clone(),
                        strength: closing,
                    });
                }
            }
        } else if to.length() < radius + thing.size * TOUCH {
            emit_touch(Touched {
                entity,
                state: TouchState::Wall,
                strength: ball.vel.dot(to.normalize_or(Vec3::Y)).max(0.0),
                point: ball.pos + to.normalize_or(Vec3::Y) * radius,
            });
        }
    }

    // One retirement batch per quarter second, independent of pickup frequency. Credit stays
    // in ball.volume; only the visual representation moves into the D3 debris shell.
    let mut capped = Vec::new();
    if now >= *next_bury || now + 1.0 < *next_bury {
        *next_bury = now + 0.25;
        let mut visible = Vec::new();
        for (entity, trophy) in trophies.iter().map(|(e, _, a, _)| (e, *a)).chain(added) {
            if clump.settled(entity, run.elapsed - trophy.at, now)
                && (buried(&trophy, ball.radius) || trophy.size < ball.radius / 24.0)
            {
                ball.buried_volume += trophy.volume;
                capped.push(entity);
                commands.entity(entity).despawn();
            } else {
                visible.push((entity, trophy));
            }
        }
        let excess = visible.len().saturating_sub(MAX_TROPHIES);
        if excess > 0 {
            visible.sort_by(|a, b| {
                (a.1.radial + a.1.size)
                    .total_cmp(&(b.1.radial + b.1.size))
                    .then_with(|| a.1.at.total_cmp(&b.1.at))
            });
            for (entity, trophy) in visible.iter().take(excess) {
                ball.buried_volume += trophy.volume;
                capped.push(*entity);
                commands.entity(*entity).despawn();
            }
        }
    }

    // A hard bump knocks the most recent small trophies off.
    if hardest > SHED_SPEED * top_speed(ball.radius) {
        let count = (1.0 + (hardest / top_speed(ball.radius) - SHED_SPEED) * 8.0)
            .clamp(1.0, SHED_MAX_COUNT as f32) as usize;
        shed(
            &mut commands,
            ball,
            &mut run,
            &mut rng,
            time.elapsed_secs(),
            count,
            &trophies,
            &capped,
        );
    }
}

/// Knock the `count` most recently attached small trophies off the ball. Each leaves at the world
/// pose it had on the ball, flying outward and up ([`shed_fly`] lands it), and gives its volume back.
fn shed(
    commands: &mut Commands,
    ball: &mut BallState,
    run: &mut Run,
    rng: &mut Rng,
    now: f32,
    count: usize,
    trophies: &Trophies,
    capped: &[Entity],
) {
    let radius = ball.radius;
    let mut recent: Vec<_> = trophies
        .iter()
        .filter(|(e, _, a, _)| {
            !capped.contains(e) && a.size <= SHED_MAX_SIZE * radius && !buried(a, radius)
        })
        .collect();
    recent.sort_by(|a, b| b.2.at.total_cmp(&a.2.at));
    let boost = 0.6 + 0.4 * radius.min(4.0);
    for (entity, tf, trophy, thing) in recent.into_iter().take(count) {
        let world_pos = ball.pos + ball.spin * tf.translation;
        let world_rot = ball.spin * tf.rotation;
        let out = (world_pos - ball.pos).normalize_or(Vec3::Y);
        let vel = (out * rng.range(2.5, 5.0) + Vec3::Y * rng.range(3.5, 6.5)) * boost;
        commands
            .entity(entity)
            .remove::<(ChildOf, Attached)>()
            .insert((
                Transform {
                    translation: world_pos,
                    rotation: world_rot,
                    scale: tf.scale,
                },
                Shed {
                    until: now + SHED_LOCKOUT,
                    vel,
                },
            ));

        // Its volume comes back out of the ball, which never drops below its starting size, and it
        // is no longer rolled up as far as the score goes (it can be picked up again).
        ball.volume = (ball.volume - trophy.volume).max(ball.start_radius.powi(3));
        ball.radius = ball.volume.cbrt();
        run.count = run.count.saturating_sub(1);
        let emptied = run.log.get_mut(&thing.name).map(|n| {
            *n = n.saturating_sub(1);
            *n == 0
        });
        if emptied == Some(true) {
            run.log.remove(&thing.name);
        }
    }
}

/// A shed trophy's hop: ballistic until it meets the terrain, upright as it goes; the lockout
/// ends once it has landed and its time is up.
fn shed_fly(
    mut commands: Commands,
    time: Res<Time>,
    world: WorldPoint,
    collision: benilla_world::collision::WorldCollision,
    mut sheds: Query<(
        Entity,
        &mut Transform,
        &mut Shed,
        &Collectible,
        Has<NetEntity>,
    )>,
) {
    let dt = time.delta_secs().min(0.05);
    let now = time.elapsed_secs();
    for (entity, mut tf, mut shed, thing, is_unit) in &mut sheds {
        if shed.vel != Vec3::ZERO && dt > 0.0 {
            shed.vel.y -= SHED_GRAVITY * dt;
            tf.translation += shed.vel * dt;
            if !is_unit {
                let axis = Vec3::new(shed.vel.z, 0.0, -shed.vel.x).normalize_or(Vec3::X);
                tf.rotate(Quat::from_axis_angle(axis, dt * 4.0));
            }
            let terrain = world.terrain_height_under(tf.translation);
            let floor = collision
                .ray_body(tf.translation + Vec3::Y * 2.0, Dir3::NEG_Y, 6.0)
                .filter(|h| h.normal.y > 0.6)
                .map(|h| tf.translation.y + 2.0 - h.distance);
            if let Some(ground) = floor.or(terrain) {
                let basis = Mat3::from_quat(tf.rotation);
                let half = thing.half_extents * tf.scale.abs();
                let down = Vec3::new(basis.x_axis.y, basis.y_axis.y, basis.z_axis.y)
                    .abs()
                    .dot(half);
                let support = ground + down - (tf.rotation * (tf.scale * thing.center)).y;
                if shed.vel.y < 0.0 && tf.translation.y <= support {
                    tf.translation.y = support;
                    shed.vel = Vec3::ZERO;
                }
            }
        }
        let landed = shed.vel == Vec3::ZERO;
        // The failsafe: terrain that never loads must not lock a trophy out forever.
        if (landed && now >= shed.until) || now > shed.until + 3.0 {
            commands.entity(entity).remove::<(Shed, Carried)>();
        }
    }
}

/// Log impacts (wall bounces and solid bumps) so playtests can see the wall state.
fn log_bumps(mut bumps: MessageReader<Bumped>) {
    for bump in bumps.read() {
        info!(
            "azeroll bump: {:?} {} strength={:.2}",
            bump.entity, bump.name, bump.strength
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spawn_inside_a_rotated_post_and_fence_gets_clear() {
        let fence = Collectible {
            name: "Fence".into(),
            kind: super::super::Kind::Prop,
            source: Source::Spawned,
            size: 4.,
            pickup: 8.,
            half_extents: Vec3::new(4., 2., 0.15),
            center: Vec3::ZERO,
            solid: 4.,
        };
        let post = Collectible {
            half_extents: Vec3::new(0.2, 3., 0.2),
            ..fence.clone()
        };
        for yaw in [0., 0.7] {
            let tf = Transform::from_rotation(Quat::from_rotation_y(yaw));
            let start = Vec3::Y * 1.31;
            let solids = [(&tf, &fence, false), (&tf, &post, false)];
            let pos = resolve_solid_overlaps(start, 1.31, solids.into_iter());
            assert!(pos.distance(start) > 1.31);
            assert!(solids
                .iter()
                .all(|(t, c, g)| solid_push(t, c, pos, 1.31, *g).is_none()));
            assert_eq!(resolve_solid_overlaps(pos, 1.31, solids.into_iter()), pos);
            let top = Vec3::Y * 3.31;
            assert_eq!(
                resolve_solid_overlaps(top, 1.31, [(&tf, &fence, false)].into_iter()),
                top,
                "a ball standing on a solid top must not be expelled"
            );
        }
    }

    #[test]
    fn permanent_blockers_mask_covered_bait_but_not_tops_other_floors_or_detours() {
        let mut blocker = Collectible {
            name: "haystack".into(),
            kind: super::super::Kind::Prop,
            source: Source::Spawned,
            size: 4.,
            pickup: f32::INFINITY,
            half_extents: Vec3::new(3.3, 2., 3.4),
            center: Vec3::ZERO,
            solid: 3.3,
        };
        let tf = Transform::from_xyz(12., 3., -7.).with_rotation(Quat::from_rotation_y(0.7));
        let food = Collectible {
            half_extents: Vec3::splat(0.25),
            size: 0.4,
            pickup: 0.5,
            solid: 0.,
            ..blocker.clone()
        };
        let at = |p| {
            Transform::from_translation(tf.transform_point(p))
                .with_rotation(Quat::from_rotation_y(-0.3))
        };
        assert!(hidden_by_blocker(&at(Vec3::X), &food, &tf, &blocker));
        assert!(
            hidden_by_blocker(&at(Vec3::X * 3.2), &food, &tf, &blocker),
            "item extent margins cover bait at the blocker edge"
        );
        assert!(
            !hidden_by_blocker(&at(Vec3::X * 4.), &food, &tf, &blocker),
            "food beyond the extent margin still merits a detour"
        );
        assert!(
            !hidden_by_blocker(&at(Vec3::Y * -4.), &food, &tf, &blocker),
            "a cellar underneath the blocker is a separate storey"
        );
        let harness = Collectible {
            half_extents: Vec3::new(0.3, 3., 0.3),
            ..food.clone()
        };
        assert!(
            hidden_by_blocker(&at(Vec3::ZERO), &harness, &tf, &blocker),
            "a tall model protruding above the replacement is still covered bait"
        );
        assert!(
            !hidden_by_blocker(&at(Vec3::Y * 2.25), &food, &tf, &blocker),
            "food on the climbable top stays food"
        );
        assert!(
            !hidden_by_blocker(&at(Vec3::Z * 7.), &food, &tf, &blocker),
            "food behind the blocker still merits a detour"
        );
        blocker.pickup = 6.8;
        assert!(
            !hidden_by_blocker(&at(Vec3::X), &food, &tf, &blocker),
            "ordinary furniture can be eaten later"
        );
    }

    #[test]
    fn seating_compresses_outward_branches_into_the_growth_envelope() {
        let thing = Collectible {
            name: "test".into(),
            kind: super::super::Kind::Prop,
            source: super::super::Source::Spawned,
            size: 0.18,
            pickup: 0.2,
            half_extents: Vec3::splat(0.1),
            center: Vec3::ZERO,
            solid: 0.0,
        };
        let supports = [(Transform::from_translation(Vec3::Z * 10.0), thing.clone())];
        let tf = seat(
            &thing,
            Vec3::ONE,
            Quat::IDENTITY,
            Vec3::Z,
            0.3,
            1.0,
            &supports,
        );
        assert!(tf.translation.z > 0.3 && tf.translation.z < 1.0);
        let empty = seat(&thing, Vec3::ONE, Quat::IDENTITY, Vec3::Z, 0.3, 1.0, &[]);
        assert!(empty.translation.z < tf.translation.z);
        assert_eq!(tf.scale, Vec3::ONE);
    }

    #[test]
    fn debug_seating_system_initializes_without_query_conflicts() {
        let mut app = App::new();
        app.init_resource::<BallState>()
            .init_resource::<Rng>()
            .add_systems(Update, seat_debug_trophies);
        app.update();
    }

    #[test]
    fn radial_seating_hits_boxes_and_ignores_other_hemisphere() {
        assert_eq!(
            ray_box(Vec3::new(0.0, 0.0, -3.0), Vec3::Z, Vec3::ONE),
            Some(4.0)
        );
        assert_eq!(ray_box(Vec3::new(2.0, 0.0, -3.0), Vec3::Z, Vec3::ONE), None);
        assert_eq!(ray_box(Vec3::new(0.0, 0.0, 3.0), Vec3::Z, Vec3::ONE), None);
        let q = tangent_rotation(Quat::IDENTITY, Vec3::new(3.0, 0.1, 0.1), Vec3::X, 0.3);
        assert!((q * Vec3::X).dot(Vec3::X).abs() < 1e-5);
    }

    #[test]
    fn fence_clearance_uses_faces_and_resolves_an_embedded_centre() {
        let fence = Collectible {
            name: "Wooden Fence".into(),
            kind: super::super::Kind::Prop,
            source: super::super::Source::Spawned,
            size: 4.0,
            pickup: 8.0,
            half_extents: Vec3::new(4.0, 1.0, 0.1),
            center: Vec3::ZERO,
            solid: 6.8,
        };
        for yaw in [0.0, 0.7, std::f32::consts::FRAC_PI_2] {
            let tf = Transform::from_rotation(Quat::from_rotation_y(yaw));
            let beside = tf.rotation * Vec3::new(0.0, 0.0, 1.0);
            assert!(solid_push(&tf, &fence, beside, 0.3, false).is_none());
            let push = solid_push(&tf, &fence, Vec3::ZERO, 0.3, false).unwrap();
            assert!((push.length() - 0.4).abs() < 1e-5);
            assert!(solid_push(&tf, &fence, push * 1.001, 0.3, true).is_none());
            assert!(solid_push(&tf, &fence, Vec3::Y * 3.0, 0.3, true).is_none());
        }
        assert!(
            solid_push(&Transform::IDENTITY, &fence, Vec3::ZERO, 6.0, true).is_none(),
            "dislodge is not a wall"
        );
    }

    #[test]
    fn trunk_wall_is_at_the_foot_not_the_canopy_centre() {
        let tree = Collectible {
            name: "Elwynn Canopy Tree".into(),
            kind: super::super::Kind::Prop,
            source: super::super::Source::Spawned,
            size: 12.0,
            pickup: 20.0,
            half_extents: Vec3::new(8.0, 10.0, 8.0),
            center: Vec3::new(3.0, 8.0, 2.0),
            solid: 0.5,
        };
        for r in [0.3, 1.0, 3.0] {
            let tf = Transform::from_rotation(Quat::from_rotation_y(0.7));
            assert!(solid_push(&tf, &tree, Vec3::new(r + 0.6, r, 0.0), r, false).is_none());
            assert!(solid_push(&tf, &tree, Vec3::new(r + 0.4, r, 0.0), r, false).is_some());
        }
    }

    #[test]
    fn planted_props_keep_their_trunk_wall_until_normal_pickup() {
        let tree = Collectible {
            name: "Elwynn Canopy Tree".into(),
            kind: super::super::Kind::Prop,
            source: Source::Spawned,
            size: 12.,
            pickup: 20.,
            half_extents: Vec3::new(8., 10., 8.),
            center: Vec3::new(3., 8., 2.),
            solid: 0.5,
        };
        let tf = Transform::from_rotation(Quat::from_rotation_y(0.7));
        let volume = tree.volume(tf.scale);
        for r in [3., 15., 20., 21.5, 25.] {
            let ordinary = contact(r, tree.pickup);
            assert_eq!(loose_contact(r, tree.pickup, false), ordinary);
            let planted = loose_contact(r, tree.pickup, true);
            if ordinary == Contact::RollUp {
                assert_eq!(planted, Contact::RollUp);
            } else {
                assert_eq!(planted, Contact::Wall);
            }
            let beside = Vec3::new(r + 0.4, r, 0.);
            assert_eq!(
                solid_push_with_contact(&tf, &tree, beside, r, false, planted).is_some(),
                planted == Contact::Wall,
                "the planted dislodge band must reach the actual basal trunk correction"
            );
            assert!(
                solid_push_with_contact(&tf, &tree, Vec3::new(r + 0.6, r, 0.), r, false, planted,)
                    .is_none(),
                "the canopy must not become a wide wall"
            );
        }
        assert!(solid_push(&tf, &tree, Vec3::new(15.4, 15., 0.), 15., false).is_none());
        assert_eq!(tree.pickup, 20.);
        assert_eq!(tree.volume(tf.scale), volume);
    }

    #[test]
    fn box_volume_is_additive_and_thin_boxes_credit_less() {
        let mut thing = Collectible {
            name: "test".into(),
            kind: super::super::Kind::Prop,
            source: super::super::Source::Spawned,
            size: 1.0,
            pickup: 2.0,
            half_extents: Vec3::ONE,
            center: Vec3::ZERO,
            solid: 0.0,
        };
        let volume = thing.volume(Vec3::ONE);
        assert!((volume - growth(2.0, 1.0)).abs() < 1e-6);
        thing.half_extents.y = 0.1;
        assert!((thing.volume(Vec3::ONE) - volume * 0.1).abs() < 1e-6);
        assert!((thing.volume(Vec3::splat(2.0)) - volume * 0.8).abs() < 1e-6);
        let start = 0.25_f32;
        let r = (start.powi(3) + volume).cbrt();
        let restored = (r.powi(3) - volume).cbrt();
        assert!((restored - start).abs() < 1e-5);
    }

    #[test]
    fn three_states_follow_diameter_over_max_axis() {
        // D/A = 2.2 rolls up, 2.15 is the exact edge, 1.5..2.15 dislodges, below is a wall.
        assert_eq!(contact(1.1, 1.0), Contact::RollUp);
        assert_eq!(contact(1.075, 1.0), Contact::RollUp);
        assert_eq!(contact(1.07, 1.0), Contact::Dislodge);
        assert_eq!(contact(0.75, 1.0), Contact::Dislodge);
        assert_eq!(contact(0.74, 1.0), Contact::Wall);
    }

    #[test]
    fn pickup_and_dislodge_bands_are_distinct() {
        let axis = 1.0;
        assert!(axis > COLLECT_RATIO * 0.749);
        assert!(2.0 * 0.749 < DISLODGE_RATIO * axis);
        assert!(2.0 * 0.75 >= DISLODGE_RATIO * axis);
        assert!(axis > COLLECT_RATIO * 1.074);
        assert!(axis <= COLLECT_RATIO * 1.0751);
    }
}

#[cfg(test)]
mod lod_contact_tests {
    use super::*;

    #[test]
    fn envelope_keeps_offset_rotated_and_nonuniform_trophy_contacts() {
        // Surface samples exercise the actual contact math, including protrusions well beyond r.
        for angle in [0.0, 0.7, 2.1] {
            for scale in [Vec3::ONE, Vec3::new(0.3, 2.0, 1.2)] {
                let tf = Transform {
                    translation: Vec3::new(2.0, -0.3, 0.7),
                    rotation: Quat::from_euler(EulerRot::XYZ, angle, 0.4, -angle),
                    scale,
                };
                let centre = Vec3::new(0.3, 0.7, -0.2);
                let half = Vec3::new(0.2, 0.4, 2.0) * scale;
                let reach = tf.transform_point(centre).length() + half.length();
                for x in [-1.0, 0.0, 1.0] {
                    for y in [-1.0, 0.0, 1.0] {
                        for z in [-1.0, 0.0, 1.0] {
                            let to = tf.transform_point(centre)
                                + tf.rotation * (half * Vec3::new(x, y, z));
                            let local = tf.rotation.inverse() * (to - tf.translation);
                            let gap = local - scale * centre;
                            assert!((gap - gap.clamp(-half, half)).length() < 0.01);
                            assert!(!outside_clump(to, 0.5, reach, 0.02));
                        }
                    }
                }
                assert!(outside_clump(Vec3::splat(1000.0), 0.5, reach, 0.02));
            }
        }
    }
}
