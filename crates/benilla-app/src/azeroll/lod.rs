//! Scale-relative visual retirement. Credit and collectible metadata outlive render geometry.
use super::{Attached, BallState, Collectible, Kind, Phase, Run, RunEntity, Source};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CacheClump;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ParkCarried;

pub(super) const MAX_TROPHIES: usize = 160;
pub(super) fn micro(axis: f32, radius: f32) -> bool {
    axis.is_finite() && axis > 0.0 && axis <= tier_ceiling(radius)
}
// Strict power-of-two size tiers; D must eclipse the entire tier by 24x.
fn tier_ceiling(radius: f32) -> f32 {
    let limit = radius * 2.0 / 24.0;
    if limit <= 0.0 {
        return 0.0;
    }
    2.0_f32.powf(limit.log2().floor())
}

/// Ball-local geometric quantities, independent of the parent's spin/translation.
#[derive(Resource, Default)]
pub(super) struct Clump {
    pub moment: Vec3,
    pub reach: f32,
    first_seen: HashMap<Entity, f32>,
}
impl Clump {
    pub fn settled(&self, e: Entity, run_age: f32, now: f32) -> bool {
        // Free Roll freezes Run.elapsed, but its newly collected trophies must still age.
        run_age >= 0.6 || self.first_seen.get(&e).is_some_and(|at| now - at >= 0.6)
    }
}
#[allow(clippy::type_complexity)]
fn cache_clump(
    mut cache: ResMut<Clump>,
    time: Res<Time>,
    dirty: Query<
        (),
        (
            With<Attached>,
            Or<(Changed<Transform>, Changed<Attached>, Changed<Collectible>)>,
        ),
    >,
    mut removed: RemovedComponents<Attached>,
    trophies: Query<(Entity, &Transform, &Attached, &Collectible)>,
) {
    let removed = removed.read().count() > 0;
    if dirty.is_empty() && !removed {
        return;
    }
    let _profile = super::profile::span("lod::cache_clump");
    cache.moment = Vec3::ZERO;
    cache.reach = 0.0;
    cache.first_seen.retain(|e, _| trophies.contains(*e));
    for (e, tf, a, c) in &trophies {
        cache.first_seen.entry(e).or_insert(time.elapsed_secs());
        let centre = tf.transform_point(c.center);
        cache.moment += centre * a.volume;
        cache.reach = cache
            .reach
            .max(centre.length() + (c.half_extents * tf.scale.abs()).length());
    }
}

#[derive(Resource, Default)]
struct Retired(
    HashSet<u32>,
    Option<f32>,
    HashSet<benilla_world::terrain_stream::WmoPropKey>,
);
#[derive(Component)]
pub(super) struct Cleaned;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Retired>()
        .init_resource::<Clump>()
        .add_systems(
            Update,
            surface_large
                .after(super::ball::drive_ball)
                .before(super::collect::collect),
        )
        .add_systems(
            Update,
            cache_clump
                .in_set(CacheClump)
                .before(benilla_world::schedule::WorldStage::Input),
        )
        .add_systems(OnEnter(Phase::Ready), clear)
        .add_systems(
            Update,
            clean_world.after(benilla_world::schedule::WorldStage::Stream),
        )
        .add_systems(
            PostUpdate,
            park_carried
                .in_set(bevy::app::AnimationSystems)
                .in_set(ParkCarried)
                .before(bevy::animation::advance_animations),
        );
}
fn clear(mut retired: ResMut<Retired>) {
    *retired = Retired::default();
}

/// Keep the tiny root as a logical collectible: removing its volume would change progression,
/// census and report totals. All render/animation descendants are destroyed permanently this run.
/// Only claimed scenery qualifies: authored spill, creatures, NPCs, gates and goals are protected.
#[allow(clippy::type_complexity)]
fn clean_world(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    ball: Res<BallState>,
    mut retired: ResMut<Retired>,
    mut next: Local<f32>,
    things: Query<
        (Entity, &Transform, &Collectible, &Children),
        (
            Without<Attached>,
            Without<Cleaned>,
            Without<super::collect::Shed>,
            Without<super::design::Gate>,
        ),
    >,
) {
    if *phase.get() != Phase::Playing {
        return;
    }
    let now = time.elapsed_secs();
    let started = *retired.1.get_or_insert(now);
    if run.elapsed < 10.0 && now - started < 10.0 {
        return;
    }
    if now < *next && now + 2.0 >= *next {
        return;
    }
    *next = now + 1.0;
    let _profile = super::profile::span("lod::clean_world");
    let mut count = 0;
    for (e, tf, c, children) in &things {
        let seen = match c.source {
            Source::Doodad(uid) => retired.0.contains(&uid),
            Source::WmoProp(key) => retired.2.contains(&key),
            Source::Spawned => continue,
        };
        if c.kind != Kind::Prop {
            continue;
        }
        let distant = tf.transform_point(c.center).distance_squared(ball.pos)
            > (60.0_f32.max(8.0 * ball.radius)).powi(2);
        if !seen && !(micro(c.pickup, ball.radius) && distant) {
            continue;
        }
        match c.source {
            Source::Doodad(uid) => {
                retired.0.insert(uid);
            }
            Source::WmoProp(key) => {
                retired.2.insert(key);
            }
            Source::Spawned => unreachable!(),
        }
        for child in children.iter() {
            commands.entity(child).despawn();
        }
        commands
            .entity(e)
            .insert((Cleaned, RunEntity))
            .remove::<super::props::Occluder>();
        count += 1;
    }
    if count > 0 {
        info!("azeroll lod: retired {count} world visuals; {} placement tombstones, r={:.2}; logical food retained", retired.0.len()+retired.2.len(), ball.radius);
    }
}

#[derive(Component)]
struct Frozen;
// NPCs already have a newest-eight budget; this adds the size gate for creatures and NPCs.
fn park_carried(
    mut commands: Commands,
    ball: Res<BallState>,
    rigs: Query<
        (Entity, Option<&Attached>, &Collectible, Has<Frozen>),
        (
            With<crate::net::NetEntity>,
            Or<(With<Attached>, With<Frozen>)>,
        ),
    >,
) {
    for (e, a, c, ours) in &rigs {
        if a.is_some_and(|a| micro(c.pickup, ball.radius) || a.radial + a.size < 0.65 * ball.radius)
        {
            commands
                .entity(e)
                .insert((Frozen, benilla_world::rig_anim::AnimParked));
        } else if ours {
            commands
                .entity(e)
                .remove::<(Frozen, benilla_world::rig_anim::AnimParked)>();
        }
    }
}

/// D26: substantial trophies cover the whole growing surface, rather than all sinking
/// into the pickup-facing patch. No copied geometry/credit; D23's 160-root cap still applies.
#[derive(Component)]
struct SurfaceDirection(Vec3);
fn surface_large(
    mut commands: Commands,
    ball: Res<BallState>,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    mut trophies: Query<(
        Entity,
        &mut Transform,
        &mut Attached,
        &Collectible,
        Option<&SurfaceDirection>,
    )>,
    mut buildings: Query<
        (
            Entity,
            &mut Transform,
            &mut Attached,
            Has<super::city::CityTrophy>,
            Option<&SurfaceDirection>,
        ),
        Without<Collectible>,
    >,
) {
    if *phase.get() != Phase::Playing || ball.radius < 8.0 {
        return;
    }
    // D25's two reduced house trophies deliberately have no Collectible. Keep them outside
    // the growing shell too, preserving their authored orientation and exact existing credit.
    for (e, mut tf, mut a, city, surface) in &mut buildings {
        if city {
            let dir = surface.map(|s| s.0).unwrap_or_else(|| {
                let seed = e.index_u32() as f32;
                let y = (seed * 0.618034).fract() * 1.8 - 0.85;
                let angle = seed * 2.399963;
                let ring = (1. - y * y).sqrt();
                let dir = Vec3::new(ring * angle.cos(), y, ring * angle.sin());
                // Face each roof outward once, then retain its pose in the spinning clump.
                let up = tf.rotation * Vec3::Y;
                tf.rotation = Quat::from_rotation_arc(up, dir) * tf.rotation;
                commands.entity(e).insert(SurfaceDirection(dir));
                dir
            });
            let radial = ball.radius * 0.98 + a.size * 0.15;
            tf.translation = dir * radial;
            a.radial = radial;
            continue;
        }
        let want = (ball.radius * 0.98).max(a.radial);
        if want > a.radial + 0.02 {
            let dir = tf.translation.normalize_or(Vec3::Y);
            tf.translation += dir * (want - a.radial);
            a.radial = want;
        }
    }
    for (e, mut tf, mut a, c, surface) in &mut trophies {
        if a.size < ball.radius / 12.0 || (run.elapsed - a.at > 60.0 && a.size < ball.radius / 5.0)
        {
            continue;
        }
        if run.elapsed > 0.0 && run.elapsed - a.at < 0.6 {
            continue;
        }
        let dir = surface.map(|s| s.0).unwrap_or_else(|| {
            // Stable, independent of query order and frame rate. Upper/lower hemisphere both carry food.
            let seed = e.index_u32() as f32;
            let y = ((seed * 0.618034).fract() * 2.0 - 1.0).clamp(-0.95, 0.95);
            let angle = seed * 2.399963;
            let ring = (1.0 - y * y).sqrt();
            let dir = Vec3::new(ring * angle.cos(), y, ring * angle.sin());
            commands.entity(e).insert(SurfaceDirection(dir));
            dir
        });
        let center = tf.transform_point(c.center);
        let radial = (ball.radius * 0.98 - a.size * 0.2).max(ball.radius * 0.75);
        let delta = dir * radial - center;
        if delta.length_squared() > 0.0004 {
            tf.translation += delta;
            a.radial = radial;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_surface_keeps_credit_and_spreads_existing_objects() {
        let mut app = App::new();
        app.insert_resource(State::new(Phase::Playing))
            .insert_resource(Run {
                elapsed: 10.0,
                ..default()
            })
            .insert_resource(BallState {
                radius: 20.0,
                ..default()
            })
            .add_systems(Update, surface_large);
        let mut roots = Vec::new();
        for _ in 0..2 {
            roots.push(
                app.world_mut()
                    .spawn((
                        Transform::IDENTITY,
                        Attached {
                            radial: 1.0,
                            volume: 123.0,
                            size: 3.0,
                            at: 0.0,
                        },
                        Collectible {
                            name: "Wagon".into(),
                            kind: Kind::Prop,
                            source: Source::Spawned,
                            size: 3.0,
                            pickup: 6.0,
                            half_extents: Vec3::splat(3.0),
                            center: Vec3::ZERO,
                            solid: 0.0,
                        },
                    ))
                    .id(),
            );
        }
        let house = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 10.0, 0.0),
                Attached {
                    radial: 10.0,
                    volume: 685.0,
                    size: 3.0,
                    at: 0.0,
                },
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<Transform>(house)
                .unwrap()
                .translation
                .length()
                > 19.0
        );
        assert_eq!(app.world().get::<Attached>(house).unwrap().volume, 685.0);
        let positions: Vec<_> = roots
            .iter()
            .map(|&e| {
                let a = app.world().get::<Attached>(e).unwrap();
                assert_eq!(a.volume, 123.0);
                let tf = app.world().get::<Transform>(e).unwrap();
                assert!((tf.translation.length() - a.radial).abs() < 0.001);
                assert!(a.radial > 18.0);
                tf.translation
            })
            .collect();
        assert!(positions[0].distance(positions[1]) > 5.0);
        assert_eq!(
            app.world_mut()
                .query::<&Attached>()
                .iter(app.world())
                .count(),
            3
        );
    }
    #[test]
    fn tiers_only_retire_after_the_whole_ceiling_is_eclipsed() {
        assert!(!micro(0.125, 1.49));
        assert!(micro(0.125, 1.5));
        assert!(!micro(0.126, 1.5));
        assert!(!micro(0.0, 8.0));
        assert!(!micro(f32::NAN, 8.0));
    }
    #[test]
    fn world_retirement_destroys_geometry_but_keeps_food_and_protects_authored_items() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Retired>()
            .insert_resource(State::new(Phase::Playing))
            .insert_resource(Run {
                elapsed: 11.0,
                ..Default::default()
            })
            .insert_resource(BallState {
                radius: 8.0,
                ..Default::default()
            })
            .add_systems(Update, clean_world);
        let c = Collectible {
            name: "Mug".into(),
            kind: Kind::Prop,
            source: Source::Doodad(42),
            size: 0.15,
            pickup: 0.25,
            half_extents: Vec3::splat(0.125),
            center: Vec3::ZERO,
            solid: 0.0,
        };
        let tf = Transform::from_xyz(1000.0, 0.0, 0.0);
        let credit = c.volume(tf.scale);
        let root = app.world_mut().spawn((tf, c.clone())).id();
        let mesh = app.world_mut().spawn(ChildOf(root)).id();
        let authored = app
            .world_mut()
            .spawn((
                tf,
                Collectible {
                    source: Source::Spawned,
                    ..c
                },
            ))
            .id();
        let protected_mesh = app.world_mut().spawn(ChildOf(authored)).id();
        let key = benilla_world::terrain_stream::WmoPropKey {
            map: 0,
            placement: 10047,
            modd: 303,
        };
        let city_food = Collectible {
            source: Source::WmoProp(key),
            ..app.world().get::<Collectible>(authored).unwrap().clone()
        };
        let city = app.world_mut().spawn((tf, city_food)).id();
        let city_mesh = app.world_mut().spawn(ChildOf(city)).id();
        app.update();
        assert!(app.world().get_entity(mesh).is_err());
        assert!(app.world().get_entity(city_mesh).is_err());
        assert!(app.world().get::<Collectible>(city).is_some());
        assert!(app.world().resource::<Retired>().2.contains(&key));
        assert!(app.world().get_entity(protected_mesh).is_ok());
        assert!(app.world().get::<RunEntity>(root).is_some());
        assert_eq!(
            app.world()
                .get::<Collectible>(root)
                .unwrap()
                .volume(tf.scale),
            credit
        );
        assert!(app.world().resource::<Retired>().0.contains(&42));
        // Exercise the shipping Ready reset: no cleaned ledger may survive into a smaller run.
        use bevy::ecs::system::RunSystemOnce;
        let design: super::super::design::Design =
            toml::from_str(include_str!("goldshire.toml")).unwrap();
        app.world_mut().insert_resource(design);
        app.world_mut()
            .run_system_once(super::super::reset_run)
            .unwrap();
        app.world_mut().run_system_once(clear).unwrap();
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().resource::<Retired>().0.is_empty());
        assert!(app.world().resource::<Retired>().2.is_empty());
        assert!(app.world().get_entity(authored).is_ok());
        // Fresh Free Roll holds Run.elapsed at zero; world retirement still starts after warmup.
        app.world_mut().resource_mut::<BallState>().radius = 8.0;
        let food = Collectible {
            source: Source::Doodad(43),
            ..app.world().get::<Collectible>(authored).unwrap().clone()
        };
        let free = app.world_mut().spawn((tf, food)).id();
        let free_mesh = app.world_mut().spawn(ChildOf(free)).id();
        app.update();
        assert!(app.world().get_entity(free_mesh).is_ok());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(11));
        app.update();
        assert!(app.world().get_entity(free_mesh).is_err());
        assert!(app.world().get_entity(free).is_ok());
    }
    #[test]
    fn cached_geometry_tracks_pose_and_membership_changes() {
        let mut app = App::new();
        app.init_resource::<Clump>()
            .init_resource::<Time>()
            .add_systems(Update, cache_clump);
        let c = Collectible {
            name: "Pole".into(),
            kind: Kind::Prop,
            source: Source::Spawned,
            size: 2.0,
            pickup: 4.0,
            half_extents: Vec3::new(0.1, 0.1, 2.0),
            center: Vec3::X,
            solid: 0.0,
        };
        let root = app
            .world_mut()
            .spawn((
                Transform::IDENTITY,
                c,
                Attached {
                    radial: 1.0,
                    volume: 2.0,
                    size: 2.0,
                    at: 0.0,
                },
            ))
            .id();
        app.update();
        assert_eq!(app.world().resource::<Clump>().moment, Vec3::X * 2.0);
        // A frozen run clock (Free Roll) still honors the pickup grace and then retires detail.
        assert!(!app.world().resource::<Clump>().settled(root, 0.0, 0.59));
        assert!(app.world().resource::<Clump>().settled(root, 0.0, 0.61));
        app.world_mut()
            .get_mut::<Transform>(root)
            .unwrap()
            .translation = Vec3::Y;
        app.update();
        assert_eq!(
            app.world().resource::<Clump>().moment,
            (Vec3::X + Vec3::Y) * 2.0
        );
        app.world_mut().entity_mut(root).despawn();
        app.update();
        assert_eq!(app.world().resource::<Clump>().moment, Vec3::ZERO);
        assert_eq!(app.world().resource::<Clump>().reach, 0.0);
    }
}
