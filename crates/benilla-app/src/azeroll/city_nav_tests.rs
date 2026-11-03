//! Installed-data regression: city entrance planning must produce a route the mover can execute.
use super::*;
use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
use bevy::ecs::system::RunSystemOnce;

fn city(p: [f32; 3]) -> Vec3 {
    let (sin, cos) = (-141.5_f32).to_radians().sin_cos();
    units::wow_to_bevy([
        cos * p[0] - sin * p[1] - 8931.634,
        sin * p[0] + cos * p[1] + 539.294,
        p[2] + 101.957,
    ])
}
#[test]
fn installed_stormwind_gate_route_executes_with_the_live_body() {
    let Some(data) = benilla_formats::wow_data() else {
        return;
    };
    let mut chain = benilla_formats::open_chain(&data).unwrap();
    let (mut vertices, mut indices) = (vec![], vec![]);
    // The permanent Valley, entrance hall and Trade street. No edible group intersects this gate.
    for group in [29, 45, 46, 285] {
        let path = format!(r"world\wmo\azeroth\buildings\stormwind\stormwind_{group:03}.wmo");
        let bytes = chain.read_file(&path).unwrap();
        benilla_formats::accumulate_wmo_group_collision(&bytes, &mut vertices, &mut indices);
    }
    let vertices = vertices.into_iter().map(city).collect();
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
    let design = design::parse(include_str!("stormwind.toml")).unwrap();
    app.world_mut().run_system_once(move |world: benilla_world::collision::WorldCollision| {
        for r in [2.3, 2.96] {
            for (name, from, to) in [
                ("outer approach", [145.,0.,-8.9], [40.,0.,-5.6]),
                ("main arch", [40.,0.,-5.6], [-25.,0.,-8.22]),
                ("inner street", [-25.,0.,-8.22], [-70.,0.,-8.96]),
            ] {
                let stance = |p| {
                    let p = city(p);
                    let origin = p + Vec3::Y * 0.6;
                    let hit = world.ray_body(origin, Dir3::NEG_Y, 1.2).expect("audited gate floor");
                    Vec3::new(p.x, origin.y - hit.distance + r, p.z)
                };
                let mut pos = stance(from);
                let target = stance(to);
                let ball = BallState { pos, radius:r, ..Default::default() };
                let path = indoor_path_to(&world, &design, &ball, target, 0., &[], None)
                    .unwrap_or_else(|| panic!("{name} r={r}: no route {pos:?} -> {target:?}"));
                let mut nav = IndoorNav {
                    entity:Entity::PLACEHOLDER, at:target, radius:r,
                    path, next:0, blockers:vec![],
                };
                let (mut vy, mut grounded, mut arrived) = (0., true, false);
                let shape = Collider::sphere(0.85*r);
                for _ in 0..6000 {
                    nav.advance(pos, (0.4*r).max(0.12), r);
                    if nav.next + 1 == nav.path.len()
                        && units::flat(pos - nav.path[nav.next]).length() < 0.2
                        && (pos.y - nav.path[nav.next].y).abs() < 0.25+0.1*r {
                        arrived = true; break;
                    }
                    let velocity = units::flat(nav.path[nav.next] - pos).normalize_or_zero()*6.;
                    if let Some(rest) = super::super::ball::floor_rest(&world, pos+velocity/60., r) {
                        if grounded && rest > pos.y && rest-pos.y <= super::super::ball::step_height(r) {
                            pos.y = rest;
                        }
                    }
                    let out = world.slide_body(&shape, pos, velocity,
                        std::time::Duration::from_secs_f32(1./60.),
                        &avian3d::character_controller::move_and_slide::MoveAndSlideConfig::default(),
                        |hit| {
                            let n:Vec3 = **hit.normal;
                            if n.y > 0.6 {
                                let flat = Vec3::new(hit.velocity.x,0.,hit.velocity.z);
                                hit.velocity.y = -flat.dot(n)/n.y;
                            }
                            avian3d::character_controller::move_and_slide::MoveAndSlideHitResponse::Accept
                        });
                    pos.x = out.position.x; pos.z = out.position.z;
                    let rest = super::super::ball::floor_rest(&world,pos,r).expect("permanent street support");
                    (pos.y,vy,grounded) = super::super::ball::settle_vertical(
                        pos.y,vy,grounded,rest,0.05+0.02*r+0.8*6./60.,1./60.);
                }
                assert!(arrived,"{name} r={r}: mover {pos:?}, node {}/{} {:?}",
                    nav.next,nav.path.len(),nav.path[nav.next]);
            }
        }
    }).unwrap();
}
