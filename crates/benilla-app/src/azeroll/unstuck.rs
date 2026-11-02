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
    held: f32,
    next_debug: f32,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<History>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(
            Update,
            recover
                .after(collect::collect)
                .before(ball::publish_focus)
                .in_set(WorldStage::Input)
                .run_if(playing),
        );
}
fn reset(mut h: ResMut<History>) {
    *h = History::default();
}
fn clear(world: &WorldPoint, collision: &WorldCollision, pos: Vec3, radius: f32) -> Option<Vec3> {
    let terrain = world
        .terrain_height_under(pos)
        .map(|ground| ground + radius);
    let rest = match (terrain, ball::floor_rest(collision, pos, radius)) {
        (Some(a), Some(b)) => a.max(b),
        (a, b) => a.or(b)?,
    };
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
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Disjoint loose solids.
fn recover(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    trace: Res<ball::StallTrace>,
    world: WorldPoint,
    collision: WorldCollision,
    mut h: ResMut<History>,
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
    h.wedged = if intent && moved < 0.12 {
        h.wedged + h.sample
    } else {
        0.0
    };
    h.sample = 0.0;
    h.last = ball.pos;
    let room = |p: Vec3| {
        props
            .iter()
            .all(|(tf, c, gate)| collect::solid_push(tf, c, p, ball.radius, gate).is_none())
    };
    // Fixtures measure contact continuity without the fallback concealing a failure.
    if crate::webenv::var("WOW_AZEROLL_PATH").is_some() {
        return;
    }
    if h.held >= 1.0 || h.wedged >= 4.0 {
        if let Some(pos) = h
            .points
            .iter()
            .rev()
            .filter(|p| units::flat(**p - ball.pos).length() > (4.0 * ball.radius).max(12.0))
            .find_map(|p| clear(&world, &collision, *p, ball.radius).filter(|p| room(*p)))
        {
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
        && clear(&world, &collision, ball.pos, ball.radius).is_some()
        && room(ball.pos)
    {
        h.points.push_back(ball.pos);
        if h.points.len() > 1200 {
            h.points.pop_front();
        }
    }
}
