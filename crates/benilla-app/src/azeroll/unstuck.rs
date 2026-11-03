//! Escape only to remembered ground with current-radius clearance, never through a door.
use super::*;
use benilla_world::{collision::WorldCollision, world_point::WorldPoint};
use bevy::prelude::*;
#[derive(Resource, Default)]
struct History {
    points: std::collections::VecDeque<Vec3>,
    last: Vec3,
    sample: f32,
    wedged: f32,
    /// Seconds of input against a wall the ball could have turned away from.
    leaning: f32,
    held: f32,
    next_debug: f32,
}
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Recovery;
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<History>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(
            Update,
            recover
                .in_set(Recovery)
                .after(collect::collect)
                .before(ball::publish_focus)
                .in_set(WorldStage::Input)
                .run_if(playing)
                .run_if(super::finale::mobile),
        );
}
fn reset(mut h: ResMut<History>) {
    *h = History::default();
}
fn clear(world: &WorldPoint, collision: &WorldCollision, pos: Vec3, radius: f32) -> Option<Vec3> {
    clear_on(world, collision, pos, radius, None)
}
fn clear_on(
    world: &WorldPoint,
    collision: &WorldCollision,
    pos: Vec3,
    radius: f32,
    top: Option<f32>,
) -> Option<Vec3> {
    let terrain = world
        .terrain_height_under(pos)
        .map(|ground| ground + radius);
    let terrain = terrain.filter(|h| *h - radius <= pos.y + 0.1);
    let rest = match (terrain, ball::floor_rest(collision, pos, radius)) {
        (Some(a), Some(b)) => a.max(b),
        (a, b) => a.or(b)?,
    };
    let rest = top.map_or(rest, |h| rest.max(h));
    let centre = Vec3::new(pos.x, rest, pos.z);
    // Rays sample both waist and head room. Claimed prop solids are checked separately.
    for y in [-0.35, 0.0, 0.65] {
        for i in 0..16 {
            let a = i as f32 * std::f32::consts::TAU / 16.0;
            let dir = Dir3::new(Vec3::new(a.cos(), 0.0, a.sin())).ok()?;
            if collision
                .ray_body(centre + Vec3::Y * radius * y, dir, radius * 1.1)
                .is_some()
            {
                return None;
            }
        }
    }
    if collision.ray_body(centre, Dir3::Y, radius * 1.1).is_some() {
        return None;
    }
    Some(centre)
}
/// A validated high recovery for a clump that no longer fits its old street. Only city
/// recovery asks for this: the normal small-scale remembered-ground policy stays unchanged.
/// D10's rewind separation: far enough that the autopilot does not return to the same doorway
/// (4 r for a small ball), but no longer growing as 4 r at city sizes, where it sent an r 64 ball
/// out through the gate.
fn rewind_separation(radius: f32) -> f32 {
    (4.0 * radius).min(24.0 + radius).max(12.0)
}

/// Whether no horizontal direction lets the mover's 0.85 r sphere travel 0.4 r into room
/// clear of the mode's solids.
fn boxed_in(
    collision: &WorldCollision,
    pos: Vec3,
    radius: f32,
    room: &impl Fn(Vec3) -> bool,
) -> bool {
    let shape = avian3d::prelude::Collider::sphere(radius * 0.85);
    let reach = 0.4 * radius + 0.3;
    !(0..8).any(|i| {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        collision
            .cast_body(&shape, pos, dir * reach, 0.02)
            .is_none_or(|hit| hit.distance >= 0.75 * reach)
            && room(pos + dir * reach)
    })
}

pub(super) fn rooftop(
    world: &WorldPoint<'_, '_>,
    collision: &WorldCollision<'_, '_>,
    at: Vec3,
    r: f32,
    sky: f32,
) -> Option<Vec3> {
    let high = 100. + 2. * r;
    let origin = Vec3::new(at.x, (at.y + high).max(sky), at.z);
    let hit = collision.ray_body(origin, Dir3::NEG_Y, origin.y - at.y + 2. * r + 80.)?;
    if hit.normal.y <= 0.6 {
        return None;
    }
    let center = Vec3::new(at.x, origin.y - hit.distance + r, at.z);
    clear(world, collision, center, r)
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Disjoint loose solids.
fn recover(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    trace: Res<ball::StallTrace>,
    world: WorldPoint,
    collision: WorldCollision,
    mut h: ResMut<History>,
    parts: Res<benilla_world::terrain_stream::WmoParts>,
    deck: Res<city_support::CityDeck>,
    autopilot: Option<Res<auto::Auto>>,
    mut ball: ResMut<BallState>,
    props: Query<
        (&Transform, &Collectible, Has<design::Gate>),
        (Without<Attached>, Without<collect::Shed>),
    >,
) {
    let dt = time.delta_secs();
    if crate::webenv::var("WOW_AZEROLL_STALL_DEBUG").is_some()
        && trace.input
        && time.elapsed_secs() >= h.next_debug
    {
        h.next_debug = time.elapsed_secs() + 1.0;
        let moved = units::flat(ball.pos - h.last).length();
        if units::flat(ball.vel).length() < 0.3 || moved < 0.15 {
            let mut nearest: Vec<_> = props
                .iter()
                .map(|(tf, c, _)| {
                    let at = tf.transform_point(c.center);
                    (at.distance(ball.pos), c, at, tf)
                })
                .collect();
            nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
            let contacts: Vec<_> = nearest
                .iter()
                .take(8)
                .map(|(d, c, at, tf)| {
                    format!(
                        "{} {:?} distance={d:.3} dy={:.3} axis={:.3} solid={:.3} half={:?} local={:?}",
                        c.name,
                        collect::contact(ball.radius, c.pickup),
                        at.y - ball.pos.y,
                        c.pickup,
                        c.solid, c.half_extents * tf.scale.abs(), tf.rotation.inverse() * (ball.pos-*at)
                    )
                })
                .collect();
            info!(
                "azeroll stall: pos={:?} r={:.3} velocity={:?} moved={moved:.3} grounded={} floor={:?} terrain={:?} wedge={:.2} held={:.2} history={} hits={:?} contacts={contacts:?}",
                ball.pos,
                ball.radius,
                ball.vel,
                ball.grounded,
                ball::floor_rest(&collision, ball.pos, ball.radius),
                world
                    .terrain_height_under(ball.pos)
                    .map(|y| y + ball.radius),
                h.wedged,
                h.held,
                h.points.len(),
                trace.hits
            );
        }
    }
    h.sample += dt;
    h.held = if keys.pressed(KeyCode::Backspace) {
        h.held + dt
    } else {
        0.0
    };
    if h.sample < 0.5 {
        return;
    }
    let moved = units::flat(ball.pos - h.last).length();
    let intent = trace.input
        || keys.any_pressed([
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
        ]);
    // Large outdoor clumps can jitter in a fence/tree equilibrium without making a
    // useful trip. The inn keeps its miniature tolerance; city recovery must count this.
    let tolerance = if deck.entered {
        (0.1 * ball.radius).clamp(0.35, 1.)
    } else {
        0.12
    };
    h.wedged = if intent && moved < tolerance {
        h.wedged + h.sample
    } else {
        0.0
    };
    h.sample = 0.0;
    h.last = ball.pos;
    let safe_clear = |p: Vec3| {
        let top = props
            .iter()
            .filter_map(|(tf, c, gate)| collect::solid_rest(tf, c, p, ball.radius, gate))
            .max_by(f32::total_cmp);
        clear_on(&world, &collision, p, ball.radius, top)
    };
    let room = |p: Vec3| {
        props
            .iter()
            .all(|(tf, c, gate)| collect::solid_push(tf, c, p, ball.radius, gate).is_none())
    };
    // Fixtures measure contact continuity without the fallback concealing a failure.
    if crate::webenv::var("WOW_AZEROLL_PATH").is_some() {
        return;
    }
    // A ball leaning on a wall it could turn away from is not wedged: a human holding W against
    // a building must never be teleported. The automatic rewind waits for a ball boxed in on
    // every side; Backspace still asks for it outright.
    // The autopilot cannot always find the way it could turn: after 12 s it gets the Backspace a
    // frustrated player would press (still a counted Unstuck).
    if h.wedged == 0.0 {
        h.leaning = 0.0;
    }
    let mut asked = h.held >= 1.0;
    if !asked && h.wedged >= 4.0 && !boxed_in(&collision, ball.pos, ball.radius, &room) {
        h.leaning += h.wedged;
        h.wedged = 0.0;
        if autopilot.is_some() && h.leaning >= 12.0 {
            asked = true;
        } else {
            info!(
                "azeroll unstuck: wedged {:.1}s at {:?} r={:.2} but free to turn; no rewind",
                h.leaning, ball.pos, ball.radius
            );
        }
    }
    if asked || h.wedged >= 4.0 {
        h.leaning = 0.0;
        let city = deck.entered;
        let roofs = city && ball.radius >= 12.;
        let sky = city_support::ceiling(&parts, ball.radius);
        let remembered = h
            .points
            .iter()
            .rev()
            .filter(|p| units::flat(**p - ball.pos).length() > rewind_separation(ball.radius))
            .filter(|p| !city || city_support::inside(&parts, **p))
            .find_map(|p| {
                let pos = if roofs {
                    rooftop(&world, &collision, *p, ball.radius, sky)
                } else {
                    safe_clear(*p)
                };
                pos.filter(|p| {
                    room(*p)
                        && (!city || city_support::supported(&parts, &collision, *p, ball.radius))
                })
            });
        let recovered = remembered.or_else(|| {
            if !roofs {
                return None;
            }
            for ring in [0., 1., 2., 3.] {
                for i in 0..if ring == 0. { 1 } else { 12 } {
                    let a = i as f32 * std::f32::consts::TAU / 12.;
                    let p = ball.pos + Vec3::new(a.cos(), 0., a.sin()) * ring * (ball.radius + 4.);
                    if !city_support::inside(&parts, p) {
                        continue;
                    }
                    if let Some(p) =
                        rooftop(&world, &collision, p, ball.radius, sky).filter(|p| room(*p))
                    {
                        return Some(p);
                    }
                }
            }
            None
        });
        if let Some(pos) = recovered {
            let cleared = collect::resolve_solid_overlaps(pos, ball.radius, props.iter());
            if cleared.distance_squared(pos) > 1e-6 {
                info!(
                    "azeroll solid overlap resolved after Unstuck: {pos:?} -> {cleared:?} r={}",
                    ball.radius
                );
            }
            let pos = cleared;
            warn!(
                "azeroll unstuck {:?} -> {:?}, r={}",
                ball.pos, pos, ball.radius
            );
            ball.pos = pos;
            ball.vel = Vec3::ZERO;
            ball.stun = 0.0;
        } else {
            warn!("azeroll unstuck: no remembered position has current-radius clearance");
        }
        h.held = 0.0;
        h.wedged = 0.0;
    } else if moved > 0.3
        && safe_clear(ball.pos).is_some()
        && room(ball.pos)
        && (!deck.entered || city_support::supported(&parts, &collision, ball.pos, ball.radius))
    {
        h.points.push_back(ball.pos);
        if h.points.len() > 1200 {
            h.points.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rewind_no_longer_scales_with_a_city_sized_ball() {
        assert_eq!(rewind_separation(1.0), 12.0);
        assert_eq!(rewind_separation(6.0), 24.0);
        assert!(rewind_separation(64.0) < 100.0);
    }
}
