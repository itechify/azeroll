//! D27: keep the outdoor city run on permanent decks, including after growth and recovery.
use super::*;
use benilla_assets::coords::bevy_to_wow;
use benilla_world::{collision::WorldCollision, terrain_stream::WmoParts, world_point::WorldPoint};

#[derive(Resource, Default)]
pub(super) struct CityDeck {
    pub entered: bool,
    last: Option<Vec3>,
    radius: f32,
    check: f32,
}
pub(super) fn inside(parts: &WmoParts, at: Vec3) -> bool {
    let Some(p) = parts.0.iter().find(|p| p.map == 0 && p.uid == 10047) else {
        return false;
    };
    let local = bevy_to_wow(p.transform.compute_affine().inverse().transform_point3(at));
    (local[0] < -48. && local[0] > -580. && local[1].abs() < 175.)
        || (local[0] >= -48. && local[0] < 220. && local[1].abs() < 45.)
}
pub(super) fn ceiling(parts: &WmoParts, r: f32) -> f32 {
    parts
        .0
        .iter()
        .filter(|p| p.uid == 10047)
        .map(|p| p.transform.transform_point(p.max).y + r + 32.)
        .fold(0., f32::max)
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CityDeck>()
        .add_systems(OnEnter(Phase::Ready), |mut deck: ResMut<CityDeck>| {
            *deck = default()
        })
        .add_systems(
            Update,
            guard
                .after(unstuck::Recovery)
                .before(ball::publish_focus)
                .in_set(WorldStage::Input)
                .run_if(playing)
                .run_if(finale::mobile),
        );
}
fn deck_rest(
    collision: &WorldCollision<'_, '_>,
    at: Vec3,
    r: f32,
    deck_ceiling: f32,
    support: &[Entity],
    excluded: &[Entity],
) -> Option<(f32, f32)> {
    // Probe from just above this city's outdoor decks, below decorative skyline roofs.
    // A high ray from the camera/ball can hit a distant facade cap above the player and
    // mistake perfectly safe growth below it for a fall through the street.
    let mut rest: Option<(f32, f32)> = None;
    for fraction in [0.0f32, 0.65, 0.9] {
        let distance = r * fraction;
        let lift = (r * r - distance * distance).sqrt();
        for i in 0..if fraction == 0. { 1 } else { 8 } {
            let a = i as f32 * std::f32::consts::TAU / 8.;
            let origin = Vec3::new(
                at.x + distance * a.cos(),
                deck_ceiling,
                at.z + distance * a.sin(),
            );
            if let Some(hit) = collision.ray_body_excluding(origin, Dir3::NEG_Y, 60., excluded) {
                if hit.normal.y > 0.6 && support.contains(&hit.entity) {
                    let surface = origin.y - hit.distance;
                    let y = surface + lift;
                    if rest.is_none_or(|old| y > old.0) {
                        rest = Some((y, surface));
                    }
                }
            }
        }
    }
    rest
}
pub(super) fn permanent_rest(
    parts: &WmoParts,
    collision: &WorldCollision<'_, '_>,
    at: Vec3,
    radius: f32,
) -> Option<(f32, f32)> {
    let p = parts
        .0
        .iter()
        .find(|p| p.uid == 10047 && !p.support_colliders.is_empty())?;
    let mut excluded: Vec<_> = parts
        .0
        .iter()
        .filter(|p| p.uid == 10047)
        .flat_map(|p| p.colliders.iter().copied())
        .collect();
    excluded.sort_unstable();
    excluded.dedup();
    let local = bevy_to_wow(p.transform.compute_affine().inverse().transform_point3(at));
    // The entrance hall continues into Trade District past raw X=-48. Its
    // overhead ledge is not a street: keep recovery rays below it until -90.
    let ceiling = p.transform.translation.y + if local[0] >= -90. { -1.4 } else { 6. };
    deck_rest(
        collision,
        at,
        radius,
        ceiling,
        &p.support_colliders,
        &excluded,
    )
}
pub(super) fn supported(
    parts: &WmoParts,
    collision: &WorldCollision<'_, '_>,
    at: Vec3,
    radius: f32,
) -> bool {
    permanent_rest(parts, collision, at, radius).is_some_and(|(_, surface)| at.y >= surface - 0.5)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn guard(
    time: Res<Time>,
    design: Res<design::Design>,
    parts: Res<WmoParts>,
    world: WorldPoint,
    collision: WorldCollision,
    mut deck: ResMut<CityDeck>,
    mut ball: ResMut<BallState>,
    props: Query<
        (&Transform, &Collectible, Has<design::Gate>),
        (
            Without<Attached>,
            Without<collect::Shed>,
            Without<city::CityTarget>,
        ),
    >,
) {
    if !design.wmo_floors || !inside(&parts, ball.pos) {
        return;
    }
    // The movement probe deliberately measures failures without recovery concealing them.
    if crate::webenv::var("WOW_AZEROLL_PATH").is_some() {
        return;
    }
    deck.check -= time.delta_secs();
    if deck.check > 0. {
        return;
    }
    deck.check = 0.12;
    let rest = permanent_rest(&parts, &collision, ball.pos, ball.radius);
    if let Some((rest, surface)) = rest {
        deck.entered = true;
        let grew = ball.radius > deck.radius + 0.01;
        deck.radius = ball.radius;
        if needs_lift(ball.pos.y, rest, surface, grew) {
            // Growth can outpace the next floor probe. A real fall is the centre crossing
            // the permanent deck. Merely approaching a raised terrace is left to the mover:
            // otherwise its rooftop would pull the ball up through a gate wall.
            ball.pos.y = rest;
            ball.vel.y = 0.;
        }
        deck.last = Some(ball.pos);
        return;
    }
    // Valid terrain or a still-live piece is a real surface, including outside the city's
    // street footprint. Missing permanent support must not become an invisible boundary.
    if world.terrain_height_under(ball.pos).is_some()
        || ball::floor_rest(&collision, ball.pos, ball.radius).is_some()
    {
        return;
    }
    // The approach is ordinary terrain until an actual city deck answers. Its broad
    // recipe bounds are not a wall: walking beside the Valley must remain possible.
    let local_x = parts.0.first().map(|p| {
        bevy_to_wow(
            p.transform
                .compute_affine()
                .inverse()
                .transform_point3(ball.pos),
        )[0]
    });
    if !deck.entered || local_x.is_none_or(|x| x >= -48.) {
        return;
    }
    let room = |pos: Vec3| {
        props
            .iter()
            .all(|(tf, c, gate)| collect::solid_push(tf, c, pos, ball.radius, gate).is_none())
    };
    let reference = deck.last.unwrap_or(ball.pos);
    let mut safe = None;
    for ring in [0., 1., 2., 3.] {
        for i in 0..if ring == 0. { 1 } else { 12 } {
            let a = i as f32 * std::f32::consts::TAU / 12.;
            let candidate = reference + Vec3::new(a.cos(), 0., a.sin()) * ring * (ball.radius + 4.);
            if !inside(&parts, candidate) {
                continue;
            }
            if let Some(pos) = unstuck::rooftop(
                &world,
                &collision,
                candidate,
                ball.radius,
                ceiling(&parts, ball.radius),
            )
            .filter(|p| room(*p))
            {
                safe = Some(pos);
                break;
            }
        }
        if safe.is_some() {
            break;
        }
    }
    if let Some(pos) = safe {
        warn!(
            "azeroll city deck recovery: {:?} -> {pos:?}, r={:.2}",
            ball.pos, ball.radius
        );
        ball.pos = pos;
        ball.vel = Vec3::ZERO;
        ball.stun = 0.;
        ball.grounded = true;
        deck.last = Some(pos);
    } else if let Some(last) = deck.last {
        // A failed recovery never trades a supported point for the terrain below a city hole.
        ball.pos = last;
        ball.vel = Vec3::ZERO;
    }
}

// Distinguish a missed deck from a nearby higher terrace that the mover must climb.
fn needs_lift(center: f32, rest: f32, surface: f32, grew: bool) -> bool {
    center < rest - 2. && (grew || center < surface - 0.5)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_gate_guard_keeps_the_ball_below_the_overhead_ledge() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let (mut vertices, mut indices) = (vec![], vec![]);
        for group in [29, 45, 46, 285] {
            let path = format!(r"world\wmo\azeroth\buildings\stormwind\stormwind_{group:03}.wmo");
            benilla_formats::accumulate_wmo_group_collision(
                &chain.read_file(&path).unwrap(),
                &mut vertices,
                &mut indices,
            );
        }
        let transform =
            Transform::from_translation(units::wow_to_bevy([-8931.634, 539.294, 101.957]))
                .with_rotation(benilla_assets::coords::placement_rotation([0., -141.5, 0.]));
        let at = move |p| transform.transform_point(units::wow_to_bevy(p));
        let vertices = vertices.into_iter().map(at).collect();
        let triangles = indices
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]])
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
        let collider = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::trimesh(vertices, triangles),
                Transform::default(),
            ))
            .id();
        app.update();
        let parts = WmoParts(vec![benilla_world::terrain_stream::WmoPartSnapshot {
            map: 0,
            uid: 10047,
            part: 1,
            group: 306,
            model: default(),
            original: default(),
            instance: Entity::PLACEHOLDER,
            transform,
            min: Vec3::ZERO,
            max: Vec3::ONE,
            colliders: vec![],
            support_colliders: vec![collider],
            ready: true,
        }]);
        app.world_mut().run_system_once(move |world:WorldCollision| {
            for r in [2.3,2.915,2.932,2.96,3.2] {
                for y in [-2.,-1.,0.,1.,2.] {
                    let p=at([-49.5,y,-9.3]);
                    let center=Vec3::new(p.x,p.y+r,p.z);
                    let (rest,surface)=permanent_rest(&parts,&world,center,r).expect("permanent gate street");
                    assert!(!needs_lift(center.y,rest,surface,false),"r={r} y={y}: guard falsely lifts {center:?} to {rest} from surface{surface}");
                    assert!(surface<transform.translation.y-1.4,"must remain below entrance ledge");
                }
            }
        }).unwrap();
    }
    /// D34 cost check (`--ignored`): the edge-refined support against the old fixed rings over
    /// the installed gate's decks, curbs and walls.
    #[test]
    #[ignore]
    fn installed_gate_support_cost() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let (mut vertices, mut indices) = (vec![], vec![]);
        for group in [29, 45, 46, 285] {
            let path = format!(r"world\wmo\azeroth\buildings\stormwind\stormwind_{group:03}.wmo");
            benilla_formats::accumulate_wmo_group_collision(
                &chain.read_file(&path).unwrap(),
                &mut vertices,
                &mut indices,
            );
        }
        let transform =
            Transform::from_translation(units::wow_to_bevy([-8931.634, 539.294, 101.957]))
                .with_rotation(benilla_assets::coords::placement_rotation([0., -141.5, 0.]));
        let at = move |p| transform.transform_point(units::wow_to_bevy(p));
        let vertices = vertices.into_iter().map(at).collect();
        let triangles = indices
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]])
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
        app.world_mut()
            .run_system_once(move |world: WorldCollision| {
                let old = |centre: Vec3, radius: f32| {
                    let origin_y = centre.y - radius + ball::step_height(radius) + 0.05;
                    let mut rest: Option<f32> = None;
                    for fraction in [0.0_f32, 0.3, 0.5, 0.7, 0.85, 0.95, 0.99] {
                        let d = radius * fraction;
                        let lift = (radius * radius - d * d).sqrt();
                        for i in 0..if fraction == 0.0 { 1 } else { 12 } {
                            let a = i as f32 * std::f32::consts::TAU / 12.0;
                            let o = Vec3::new(centre.x + d * a.cos(), origin_y, centre.z + d * a.sin());
                            if let Some(hit) = world.ray_body(o, Dir3::NEG_Y, 4.0 + 2.0 * radius) {
                                if hit.normal.y > 0.6 {
                                    let h = o.y - hit.distance + lift;
                                    rest = Some(rest.map_or(h, |v| v.max(h)));
                                }
                            }
                        }
                    }
                    rest
                };
                for r in [1.0_f32, 3.0, 12.0] {
                    let mut points = vec![];
                    for x in 0..40 {
                        for y in 0..40 {
                            let p = at([-80. + x as f32 * 2.0, -40. + y as f32 * 2.0, -9.3]);
                            points.push(Vec3::new(p.x, p.y + r, p.z));
                        }
                    }
                    let t = bevy::platform::time::Instant::now();
                    let a: usize = points.iter().filter(|p| old(**p, r).is_some()).count();
                    let old_ms = t.elapsed().as_secs_f64() * 1000. / points.len() as f64;
                    let t = bevy::platform::time::Instant::now();
                    let b: usize = points
                        .iter()
                        .filter(|p| ball::floor_support(&world, **p, r).is_some())
                        .count();
                    let new_ms = t.elapsed().as_secs_f64() * 1000. / points.len() as f64;
                    println!("r={r}: fixed rings {old_ms:.4} ms/call, edge-refined {new_ms:.4} ms/call ({a} / {b} supported)");
                }
            })
            .unwrap();
    }
    #[test]
    fn a_nearby_terrace_does_not_pull_a_supported_ball_up() {
        assert!(!super::needs_lift(99.6, 102.7, 99.4, false));
    }
    #[test]
    fn growth_and_a_real_deck_crossing_restore_vertical_support() {
        assert!(super::needs_lift(99., 115., 95., true));
        assert!(super::needs_lift(85., 98.3, 95., false));
        assert!(!super::needs_lift(99., 98.3, 95., false));
    }
}
