//! D22: resident MODD copies and run-long tombstones; engine hulls never see these props.
use super::{Attached, Ball, Collected, Phase, Source};
use benilla_world::terrain_stream::{
    WmoPropClaimed, WmoPropClaims, WmoPropKey, WmoPropQuery, WmoPropReleased,
};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Component)]
pub(super) struct Furniture(pub WmoPropKey);
struct Placement {
    query: WmoPropQuery,
    resident: bool,
    live: Option<Entity>,
}
#[derive(Resource, Default)]
struct FurnitureState {
    known: HashMap<WmoPropKey, Placement>,
    tombstones: HashSet<WmoPropKey>,
}
#[derive(serde::Deserialize)]
struct Manifest {
    placements: Vec<Rule>,
}
#[derive(serde::Deserialize)]
struct Rule {
    map: u32,
    uid: u32,
    root: String,
    include: Vec<String>,
    /// Optional audited MODD indices. Empty retains the model-family rule.
    #[serde(default)]
    modds: Vec<u32>,
    exclude: Vec<String>,
}
impl Rule {
    fn matches(&self, q: &WmoPropQuery) -> bool {
        let model = q.model.to_ascii_lowercase();
        q.key.map == self.map
            && q.key.placement == self.uid
            && (self.modds.is_empty() || self.modds.contains(&q.key.modd))
            && self.include.iter().any(|p| model.contains(p))
            && !self.exclude.iter().any(|p| model.contains(p))
    }
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FurnitureState>()
        .add_systems(Startup, install)
        .add_systems(OnEnter(Phase::Ready), restart.after(super::reset_run))
        .add_systems(
            Update,
            events.after(benilla_world::schedule::WorldStage::Stream),
        );
}
fn install(mut claims: ResMut<WmoPropClaims>) {
    // Capture stock MODD rendering at the same placements for D30 comparisons.
    if crate::webenv::var("WOW_AZEROLL_STOCK_FURNITURE").is_some() {
        return;
    }
    let manifest: Manifest =
        toml::from_str(include_str!("wmo-claims.toml")).expect("embedded WMO claim manifest");
    for r in &manifest.placements {
        info!(
            "azeroll furniture allowlist: map={} uid={} root={}",
            r.map, r.uid, r.root
        );
    }
    claims.predicate = Some(std::sync::Arc::new(move |q| {
        manifest.placements.iter().any(|r| r.matches(q))
    }));
}
fn spawn(commands: &mut Commands, assets: &AssetServer, q: &WmoPropQuery) -> Entity {
    let e = super::props::spawn_copy(commands, assets, &q.model, q.transform, None);
    commands
        .entity(e)
        .insert((Furniture(q.key), q.light.clone()));
    e
}
fn events(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut state: ResMut<FurnitureState>,
    mut claimed: MessageReader<WmoPropClaimed>,
    mut released: MessageReader<WmoPropReleased>,
    mut collected: MessageReader<Collected>,
    attached: Query<(), With<Attached>>,
    parents: Query<&ChildOf>,
    balls: Query<(), With<Ball>>,
    copies: Query<(), With<Furniture>>,
) {
    for e in collected.read() {
        if let Source::WmoProp(key) = e.source {
            state.tombstones.insert(key);
            info!("azeroll furniture collected: {:?} {}", key, e.name);
        }
    }
    for e in released.read() {
        if let Some(p) = state.known.get_mut(&e.key) {
            p.resident = false;
            if let Some(entity) = p.live {
                if !super::props::carried(entity, &attached, &parents, &balls) {
                    commands.entity(entity).try_despawn();
                    p.live = None;
                }
            }
        }
    }
    for WmoPropClaimed(q) in claimed.read() {
        let gone = state.tombstones.contains(&q.key);
        let p = state.known.entry(q.key).or_insert_with(|| Placement {
            query: q.clone(),
            resident: true,
            live: None,
        });
        p.resident = true;
        if !gone && !p.live.is_some_and(|e| copies.contains(e)) {
            p.live = Some(spawn(&mut commands, &assets, q));
            info!("azeroll furniture claimed: {:?} {}", q.key, q.model);
        }
    }
}
fn restart(mut commands: Commands, assets: Res<AssetServer>, mut state: ResMut<FurnitureState>) {
    state.tombstones.clear();
    for p in state.known.values_mut() {
        if let Some(e) = p.live.take() {
            commands.entity(e).try_despawn();
        }
        if p.resident {
            p.live = Some(spawn(&mut commands, &assets, &p.query));
        }
    }
}

/// Local boxes: tables/chairs/benches use four legs below a seat/top slab. Other
/// furniture uses its full box. Their rotated top faces also supply floor support (D29).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SolidShape {
    Box,
    Legs,
}
pub(super) fn shape(name: &str) -> SolidShape {
    let n = name.to_ascii_lowercase();
    if ["table", "chair", "bench"].iter().any(|s| n.contains(s)) {
        SolidShape::Legs
    } else {
        SolidShape::Box
    }
}
/// Authored solid furniture uses the same open-underneath shape as resident copies.
pub(super) fn shaped(c: &super::Collectible) -> bool {
    matches!(c.source, Source::WmoProp(_)) || shape(&c.name) == SolidShape::Legs
}
pub(super) fn boxes(name: &str, center: Vec3, half: Vec3) -> Vec<(Vec3, Vec3)> {
    if shape(name) == SolidShape::Box {
        return vec![(center, half * 0.95)];
    }
    let width = (half.x.min(half.z) * 0.14)
        .max(0.025)
        .min(half.x.min(half.z));
    // The bottom half remains open between the legs; the upper half includes chair backs.
    let mut out = vec![(
        center + Vec3::Y * half.y * 0.5,
        Vec3::new(half.x, half.y * 0.5, half.z),
    )];
    for x in [-1.0, 1.0] {
        for z in [-1.0, 1.0] {
            out.push((
                center + Vec3::new(x * (half.x - width), -half.y * 0.5, z * (half.z - width)),
                Vec3::new(width, half.y * 0.5, width),
            ));
        }
    }
    out
}
/// World-oriented boxes shared by movement/recovery and autopilot. Legs yield before
/// growth closes their gaps: entering small must never make a cage the ball cannot leave.
/// The seat/top remains solid and pushes an overgrown body out of the footprint.
pub(super) fn world_boxes(
    tf: &Transform,
    c: &super::Collectible,
    radius: f32,
) -> Vec<(Vec3, Vec3)> {
    if matches!(c.source,super::Source::WmoProp(key) if key.map==0 && key.placement==10047)
        && shape(&c.name) == SolidShape::Box
    {
        let center = tf.transform_point(Vec3::new(0., c.center.y, 0.));
        return vec![(
            center,
            Vec3::new(c.solid, c.half_extents.y * tf.scale.y.abs() * 0.95, c.solid),
        )];
    }
    let mut parts = boxes(&c.name, c.center, c.half_extents);
    if shape(&c.name) == SolidShape::Legs {
        let half = c.half_extents;
        let width = (half.x.min(half.z) * 0.14)
            .max(0.025)
            .min(half.x.min(half.z));
        let gap = ((half.x - 2.0 * width) * tf.scale.x.abs())
            .min((half.z - 2.0 * width) * tf.scale.z.abs());
        if radius + 0.025 >= gap {
            parts.truncate(1);
        }
    }
    parts
        .into_iter()
        .map(|(p, h)| (tf.transform_point(p), h * tf.scale.abs()))
        .collect()
}

/// D29: the same 73 footprint samples and spherical-cap lift as engine floors.
/// Rays begin at reachable foot height, so a ceiling cannot pull a ball through it.
pub(super) fn box_rest(center: Vec3, half: Vec3, rotation: Quat, pos: Vec3, r: f32) -> Option<f32> {
    let origin_y = pos.y - r + super::ball::step_height(r) + 0.05;
    let mut rest: Option<f32> = None;
    // A toppled book can rest on its spine: choose each upward walkable face,
    // rather than assuming the model's original +Y is still its upper surface.
    for axis in 0..3 {
        let mut local_normal = Vec3::ZERO;
        local_normal[axis] = 1.;
        let mut normal = rotation * local_normal;
        if normal.y < 0. {
            normal = -normal;
        }
        if normal.y <= 0.6 {
            continue;
        }
        let top = center + normal * half[axis];
        // The same edge-refined footprint as WMO floors (`ball::ring_support`).
        let face = super::ball::ring_support(r, |offset| {
            let x = pos.x + offset.x;
            let z = pos.z + offset.y;
            let y = top.y - (normal.x * (x - top.x) + normal.z * (z - top.z)) / normal.y;
            if y > origin_y || origin_y - y > 4. + 2. * r {
                return None;
            }
            let local = rotation.inverse() * (Vec3::new(x, y, z) - center);
            (0..3)
                .all(|i| i == axis || local[i].abs() <= half[i] + 1e-5)
                .then_some((y, normal, ()))
        });
        if let Some((h, ())) = face {
            rest = Some(rest.map_or(h, |v| v.max(h)));
        }
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;
    fn crate_prop() -> super::super::Collectible {
        super::super::Collectible {
            name: "Crate".into(),
            kind: super::super::Kind::Prop,
            source: Source::Spawned,
            size: 2.,
            pickup: 4.,
            center: Vec3::ZERO,
            half_extents: Vec3::new(2., 0.1, 2.),
            solid: 2.,
        }
    }
    #[test]
    fn leg_tops_cannot_hold_a_ball_inside_the_seat() {
        let c = super::super::Collectible {
            name: "Chair".into(),
            kind: super::super::Kind::Prop,
            source: Source::Spawned,
            size: 1.,
            pickup: 2.,
            center: Vec3::Y * 0.8,
            half_extents: Vec3::new(0.32, 0.8, 0.32),
            solid: 1.,
        };
        let pos = Vec3::new(0.28, 0.95, 0.28);
        assert!(
            super::super::collect::solid_rest(&Transform::IDENTITY, &c, pos, 0.15, true).is_none()
        );
        assert!(
            super::super::collect::solid_push(&Transform::IDENTITY, &c, pos, 0.15, true).is_some()
        );
    }
    #[test]
    fn toppled_book_supports_on_its_new_upper_face() {
        let rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let rest = box_rest(
            Vec3::ZERO,
            Vec3::new(1., 0.1, 2.),
            rotation,
            Vec3::Y * 2.3,
            0.3,
        )
        .unwrap();
        assert!((rest - 2.3).abs() < 1e-4);
    }
    #[test]
    fn tops_step_limits_edges_and_edible_support() {
        let c = crate_prop();
        let tf = Transform::from_xyz(0., 0.1, 0.);
        let r = 0.3;
        let pos = Vec3::Y * r;
        let rest = super::super::collect::solid_rest(&tf, &c, pos, r, true).unwrap();
        assert!((rest - 0.5).abs() < 1e-5);
        assert!(super::super::collect::solid_push(&tf, &c, Vec3::Y * rest, r, true).is_none());
        assert!(super::super::collect::solid_rest(
            &Transform::from_xyz(0., 0.6, 0.),
            &c,
            pos,
            r,
            true
        )
        .is_none());
        assert!(
            super::super::collect::solid_rest(&tf, &c, Vec3::new(3., rest, 0.), r, true).is_none()
        );
        assert!(
            super::super::collect::solid_rest(&tf, &c, Vec3::Y * rest, 5., true).is_none(),
            "edible support vanishes; mover falls to lower floor"
        );
    }
    #[test]
    fn tilted_top_is_a_walkable_slope_and_sides_do_not_push_it() {
        let c = crate_prop();
        let tf = Transform::from_xyz(0., 0.8, 0.).with_rotation(Quat::from_rotation_z(0.3));
        let a = super::super::collect::solid_rest(&tf, &c, Vec3::new(-0.5, 1.4, 0.), 0.3, true)
            .unwrap();
        let b =
            super::super::collect::solid_rest(&tf, &c, Vec3::new(0.5, 1.4, 0.), 0.3, true).unwrap();
        assert!((b - a - 0.3_f32.tan()).abs() < 0.01);
        assert!(
            super::super::collect::solid_push(&tf, &c, Vec3::new(0.5, b, 0.), 0.3, true).is_none()
        );
        assert!(box_rest(
            Vec3::ZERO,
            Vec3::ONE,
            Quat::from_rotation_arc(Vec3::ONE.normalize(), Vec3::Y),
            Vec3::Y * 2.,
            0.3
        )
        .is_none());
    }
    #[test]
    fn dislodge_objects_support_until_they_move() {
        let c = crate_prop();
        assert_eq!(
            super::super::collect::contact(3.2, c.pickup),
            super::super::collect::Contact::Dislodge
        );
        let tf = Transform::IDENTITY;
        let pos = Vec3::Y * 3.3;
        assert!(super::super::collect::solid_rest(&tf, &c, pos, 3.2, true).is_some());
    }
    #[test]
    fn manifest_is_placement_scoped_and_keeps_lights() {
        let manifest: Manifest = toml::from_str(include_str!("wmo-claims.toml")).unwrap();
        let mut q = WmoPropQuery {
            light: benilla_world::terrain_stream::PropLight::Exterior,
            key: WmoPropKey {
                map: 0,
                placement: 71414,
                modd: 303,
            },
            model: r"world\tables\inntable.m2".into(),
            transform: Transform::IDENTITY,
        };
        assert!(manifest.placements[0].matches(&q));
        q.model = r"world\tables\chandelier.m2".into();
        assert!(!manifest.placements[0].matches(&q));
        q.model = r"world\chairs\chair.m2".into();
        q.key.map = 1;
        assert!(!manifest.placements[0].matches(&q));
    }
    #[test]
    fn city_props_keep_basal_hulls_instead_of_canopy_sized_boxes() {
        let mut c = super::super::Collectible {
            name: "Elwynn Tree".into(),
            kind: super::super::Kind::Prop,
            source: super::super::Source::WmoProp(WmoPropKey {
                map: 0,
                placement: 10047,
                modd: 5,
            }),
            size: 40.,
            pickup: 80.,
            center: Vec3::new(8., 20., 4.),
            half_extents: Vec3::new(30., 20., 30.),
            solid: 0.5,
        };
        let tf = Transform::from_xyz(100., 5., 200.);
        let boxes = world_boxes(&tf, &c, 5.);
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].0, Vec3::new(100., 25., 200.));
        assert_eq!(boxes[0].1.x, 0.5);
        assert_eq!(boxes[0].1.z, 0.5);
        c.source = super::super::Source::WmoProp(WmoPropKey {
            map: 0,
            placement: 71414,
            modd: 5,
        });
        assert!(world_boxes(&tf, &c, 5.)[0].1.x > 20.);
    }
    #[test]
    fn legs_leave_an_open_lower_middle_and_barrels_stay_solid() {
        let h = Vec3::ONE;
        let inside = |p: Vec3, b: &(Vec3, Vec3)| (p - b.0).abs().cmple(b.1).all();
        let table = boxes("Inn Table", Vec3::Y, h);
        assert!(!table.iter().any(|b| inside(Vec3::Y * 0.25, b)));
        assert!(table.iter().any(|b| inside(Vec3::new(0.9, 0.25, 0.9), b)));
        assert!(table.iter().any(|b| inside(Vec3::Y * 1.5, b)));
        assert!(boxes("Barrel", Vec3::Y, h)
            .iter()
            .any(|b| inside(Vec3::Y * 0.25, b)));
    }
    #[test]
    fn tombstones_are_distinct_across_maps_placements_and_modds() {
        let key = WmoPropKey {
            map: 0,
            placement: 71414,
            modd: 303,
        };
        let mut state = FurnitureState::default();
        state.tombstones.insert(key);
        for other in [
            WmoPropKey { map: 1, ..key },
            WmoPropKey {
                placement: 1,
                ..key
            },
            WmoPropKey { modd: 1, ..key },
        ] {
            assert!(!state.tombstones.contains(&other));
        }
        assert!(state.tombstones.contains(&key));
        state.tombstones.clear();
        assert!(!state.tombstones.contains(&key));
    }
    #[test]
    fn movement_uses_rotated_legs_and_stops_blocking_at_rollup_size() {
        let key = WmoPropKey {
            map: 0,
            placement: 71414,
            modd: 303,
        };
        let c = super::super::Collectible {
            name: "Inn Table".into(),
            kind: super::super::Kind::Prop,
            source: Source::WmoProp(key),
            size: 1.8,
            pickup: 2.0,
            half_extents: Vec3::ONE,
            center: Vec3::Y,
            solid: 1.0,
        };
        let tf = Transform::from_rotation(Quat::from_rotation_y(0.7));
        let under = tf.transform_point(Vec3::Y * 0.2);
        assert!(super::super::collect::solid_push(&tf, &c, under, 0.15, false).is_none());
        let leg = tf.transform_point(Vec3::new(0.9, 0.2, 0.9));
        assert!(super::super::collect::solid_push(&tf, &c, leg, 0.15, false).is_some());
        assert!(super::super::collect::solid_push(&tf, &c, leg, 3.0, false).is_none());
    }

    #[test]
    fn growth_inside_chair_legs_keeps_an_exit_and_the_seat_stays_solid() {
        let c = super::super::Collectible {
            name: "Chair".into(),
            kind: super::super::Kind::Prop,
            source: Source::WmoProp(WmoPropKey {
                map: 0,
                placement: 71414,
                modd: 1,
            }),
            size: 1.0,
            pickup: 2.0,
            half_extents: Vec3::new(0.32, 0.8, 0.32),
            center: Vec3::Y * 0.8,
            solid: 1.0,
        };
        let tf = Transform::from_rotation(Quat::from_rotation_y(0.7));
        let push =
            |p, r| super::super::collect::solid_push(&tf, &c, tf.transform_point(p), r, false);
        assert!(
            push(Vec3::new(0.28, 0.1, 0.28), 0.1).is_some(),
            "tiny ball sees individual legs"
        );
        // Enter at r=.10, eat beneath the seat, leave at r=.24 through what was a .46yd gap.
        for i in 0..=12 {
            assert!(
                push(Vec3::new(0.05, 0.24, i as f32 * 0.05), 0.24).is_none(),
                "growth must not close the way out"
            );
        }
        assert!(
            push(Vec3::Y * 0.45, 0.45).is_some(),
            "the overgrown body still meets the seat slab"
        );
        assert_eq!(world_boxes(&tf, &c, 0.1).len(), 5);
        assert_eq!(world_boxes(&tf, &c, 0.24).len(), 1);
    }

    #[test]
    fn residency_tombstones_carried_survival_and_reset() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<benilla_assets::M2Model>()
            .init_resource::<FurnitureState>()
            .add_message::<WmoPropClaimed>()
            .add_message::<WmoPropReleased>()
            .add_message::<Collected>()
            .add_systems(Update, events);
        let key = WmoPropKey {
            map: 0,
            placement: 71414,
            modd: 303,
        };
        let q = WmoPropQuery {
            light: benilla_world::terrain_stream::PropLight::Exterior,
            key,
            model: "chair.m2".into(),
            transform: Transform::IDENTITY,
        };
        app.world_mut().write_message(WmoPropClaimed(q.clone()));
        app.update();
        let first = app.world().resource::<FurnitureState>().known[&key]
            .live
            .unwrap();
        let ball = app.world_mut().spawn(Ball).id();
        app.world_mut().entity_mut(ball).add_child(first);
        app.world_mut().write_message(Collected {
            entity: first,
            name: "Chair".into(),
            kind: super::super::Kind::Prop,
            source: Source::WmoProp(key),
            size: 1.0,
            pickup: 2.0,
        });
        app.world_mut().write_message(WmoPropReleased { key });
        app.update();
        assert!(app.world().get_entity(first).is_ok());
        app.world_mut().write_message(WmoPropClaimed(q.clone()));
        app.update();
        assert_eq!(
            app.world().resource::<FurnitureState>().known[&key].live,
            Some(first)
        );
        app.world_mut().run_system_once(restart).unwrap();
        let second = app.world().resource::<FurnitureState>().known[&key]
            .live
            .unwrap();
        assert_ne!(first, second);
        assert!(app.world().get_entity(first).is_err());
        assert!(app
            .world()
            .resource::<FurnitureState>()
            .tombstones
            .is_empty());
        app.world_mut().write_message(WmoPropReleased { key });
        app.update();
        assert!(app.world().get_entity(second).is_err());
        app.world_mut().run_system_once(restart).unwrap();
        assert!(app.world().resource::<FurnitureState>().known[&key]
            .live
            .is_none());
    }

    /// Pin the reported "floor hole" to the actual installation: the common room is lower
    /// than the entry landing. DETAIL render boards and walk faces occupy the same plane.
    #[test]
    fn inn_common_room_and_landing_have_matching_visible_and_walk_floors() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        for (group, points, expected) in [
            (
                5,
                vec![[-9464.0, 24.0], [-9463.0, 27.0], [-9467.0, 22.0]],
                56.3391,
            ),
            (1, vec![[-9465.0, 35.0], [-9462.0, 36.0]], 56.9633),
        ] {
            let path =
                format!(r"world\wmo\azeroth\buildings\goldshireinn\goldshireinn_{group:03}.wmo");
            let bytes = chain.read_file(&path).unwrap();
            let (mut walk, mut wi, mut visible, mut vi) = (vec![], vec![], vec![], vec![]);
            benilla_formats::accumulate_wmo_group_collision(&bytes, &mut walk, &mut wi);
            benilla_formats::accumulate_wmo_group_camera_collision(&bytes, &mut visible, &mut vi);
            for [x, y] in points {
                // MODF 71414: (0,-97,0) gives an 83-degree local-to-WoW rotation.
                let (sin, cos) = 83_f32.to_radians().sin_cos();
                let (dx, dy) = (x + 9464.247, y - 24.399);
                let (lx, ly) = (cos * dx + sin * dy, -sin * dx + cos * dy);
                let floor = |vertices: &[[f32; 3]], indices: &[u32]| {
                    indices
                        .chunks_exact(3)
                        .filter_map(|t| {
                            let tri = [
                                vertices[t[0] as usize],
                                vertices[t[1] as usize],
                                vertices[t[2] as usize],
                            ];
                            benilla_formats::triangle_z_at(&tri, lx, ly)
                                .map(|z| z + 56.526)
                                .filter(|z| *z < 58.0)
                        })
                        .max_by(f32::total_cmp)
                        .expect("floor triangle")
                };
                assert!((floor(&walk, &wi) - expected).abs() < 0.01, "walk {x},{y}");
                assert!(
                    (floor(&visible, &vi) - expected).abs() < 0.01,
                    "visible {x},{y}"
                );
            }
        }
    }

    #[test]
    fn inn_inventory_from_install() {
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        let bytes = chain
            .read_file(r"world\wmo\azeroth\buildings\goldshireinn\goldshireinn.wmo")
            .unwrap();
        let root = benilla_formats::parse_wmo_root(&bytes).unwrap();
        let manifest: Manifest = toml::from_str(include_str!("wmo-claims.toml")).unwrap();
        let mut selected = 0;
        let mut claimed = 0;
        for set in [root.doodad_sets()[0], root.doodad_sets()[2]] {
            for i in set.start..set.start + set.count {
                let d = &root.doodads()[i as usize];
                selected += 1;
                let q = WmoPropQuery {
                    light: benilla_world::terrain_stream::PropLight::Exterior,
                    key: WmoPropKey {
                        map: 0,
                        placement: 71414,
                        modd: i,
                    },
                    model: d.model.clone(),
                    transform: Transform::IDENTITY,
                };
                let yes = manifest.placements[0].matches(&q);
                claimed += usize::from(yes);
                println!("inn MODD {i} claim={yes} {}", d.model);
            }
        }
        assert_eq!(selected, 426);
        assert_eq!(claimed, 353);
        println!("inn selected={selected} claimed={claimed}");
    }
}
