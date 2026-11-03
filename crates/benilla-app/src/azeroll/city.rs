//! D27: audited structural city pieces. The engine owns collision/removal; this mode owns credit and trophies.
use super::*;
use benilla_assets::WmoModel;
use benilla_world::{
    collision::WorldCollision,
    model_forms::FormSlices,
    model_render::{ModelKind, ShadeSel},
    schedule::WorldStage,
    terrain_stream::{
        spawn_model_entities, RestoreWmos, RetireWmoPart, WmoPartRegion, WmoPartRetired,
        WmoPartSpec, WmoPartitionSpec, WmoPartitions, WmoParts,
    },
};
use std::{collections::HashMap, sync::Arc};
// Large enough to read as roofs and facades on a district-scale clump. Credit and
// collision always use the full original piece, independently of this visual reduction.
const TROPHY_SCALE: f32 = 0.45;

#[derive(Resource, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    map: u32,
    uid: u32,
    root_fingerprint: String,
    groups: usize,
    #[serde(default)]
    support_surfaces: Vec<Region>,
    #[serde(default)]
    omit_render_groups: Vec<u16>,
    parts: Vec<Part>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Part {
    id: u16,
    name: String,
    groups: Vec<u16>,
    floor: f32,
    foundation: Option<[f32; 4]>,
    #[serde(default)]
    regions: Vec<Region>,
    #[serde(default)]
    finale: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Region {
    groups: Vec<u16>,
    min: [f32; 3],
    max: [f32; 3],
}
fn install(
    mut commands: Commands,
    mut partitions: ResMut<WmoPartitions>,
    mut debug: ResMut<benilla_world::dev_state::DebugState>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native A/B only; unlike the mode fog span this disables the actual shader fog.
        if crate::webenv::var("WOW_AZEROLL_NO_FOG").is_some() {
            debug.lighting.disable_fog = true;
        }
        if crate::webenv::var("WOW_AZEROLL_NO_SKY").is_some() {
            debug.lighting.disable_sky_dome = true;
        }
        for (flag, kind) in [
            ("WOW_AZEROLL_NO_WMO", ModelKind::Wmo),
            ("WOW_AZEROLL_NO_M2", ModelKind::Doodad),
        ] {
            if crate::webenv::var(flag).is_some() {
                debug.models.kind_visible[benilla_world::model_render::kind_index(kind)] = false;
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = &mut debug;
    let m: Manifest =
        toml::from_str(include_str!("stormwind-pieces.toml")).expect("audited city pieces");
    assert_eq!(m.version, 1);
    // Native source-isolation capture: leave the stock placement and portal graph intact.
    #[cfg(not(target_arch = "wasm32"))]
    if crate::webenv::var("WOW_AZEROLL_STOCK_CITY").is_some() {
        commands.insert_resource(m);
        return;
    }
    partitions.plans.insert(
        (m.map, m.uid),
        WmoPartitionSpec {
            expected_groups: m.groups,
            fingerprint: u64::from_str_radix(&m.root_fingerprint, 16).expect("root fingerprint"),
            exterior_only: true,
            solid_parts: true,
            omit_render_groups: m.omit_render_groups.clone(),
            support_surfaces: m
                .support_surfaces
                .iter()
                .map(|r| WmoPartRegion {
                    groups: r.groups.clone(),
                    min: r.min,
                    max: r.max,
                })
                .collect(),
            parts: m
                .parts
                .iter()
                .map(|p| WmoPartSpec {
                    id: p.id,
                    groups: p.groups.clone(),
                    floor: p.floor,
                    foundation: p.foundation,
                    regions: p
                        .regions
                        .iter()
                        .map(|r| WmoPartRegion {
                            groups: r.groups.clone(),
                            min: r.min,
                            max: r.max,
                        })
                        .collect(),
                })
                .collect(),
        },
    );
    commands.insert_resource(m);
}
#[derive(Resource, Default)]
struct Pending(HashMap<(u32, u32, u16), Prepared>);

struct Prepared {
    root: Entity,
    decor: Vec<Entity>,
    axes: Vec3,
    volume: f32,
    axis: f32,
    at: f32,
    rotation: Quat,
    floor: f32,
    name: String,
    origin: Vec3,
}

pub(super) fn plugin(app: &mut App) {
    super::city_shell::plugin(app);
    #[cfg(not(target_arch = "wasm32"))]
    if crate::webenv::var("WOW_AZEROLL_TREE_AUDIT").is_some() {
        app.add_systems(Update, audit_trees.after(collect::collect).run_if(playing));
    }
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        PostUpdate,
        hide_capture_geometry
            .after(benilla_world::exterior_cull::ExteriorCullSet)
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate)
            .run_if(|| {
                crate::webenv::var("WOW_AZEROLL_NO_TERRAIN").is_some()
                    || crate::webenv::var("WOW_AZEROLL_NO_BALL").is_some()
            }),
    );
    app.init_resource::<Pending>()
        .init_resource::<Targets>()
        .add_systems(Startup, install)
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(
            Update,
            (targets, acknowledge, pickup, super::city_shell::dress)
                .chain()
                .after(WorldStage::Stream)
                .run_if(playing)
                .run_if(super::finale::mobile),
        )
        .add_systems(Update, animate.run_if(playing));
}
fn reset(
    mut pending: ResMut<Pending>,
    mut targets: ResMut<Targets>,
    mut restore: MessageWriter<RestoreWmos>,
) {
    pending.0.clear();
    targets.0.clear();
    restore.write(RestoreWmos);
}

// Exact point-to-OBB distance in model space; buildings never dislodge.
fn nearest(p: Vec3, transform: &Transform, lo: Vec3, hi: Vec3) -> Vec3 {
    transform.transform_point(
        transform
            .compute_affine()
            .inverse()
            .transform_point3(p)
            .clamp(lo, hi),
    )
}

// Same sphere footprint as floor support, queried against the post-removal world. This also
// handles a huge ball spanning two canal banks: a centre-only ray would wrongly reject it.
fn remaining_support(
    world: &WorldCollision<'_, '_>,
    at: Vec3,
    r: f32,
    excluded: &[Entity],
) -> Option<f32> {
    let origin_y = at.y - r + ball::step_height(r) + 0.05;
    let mut rest: Option<f32> = None;
    for fraction in [0.0f32, 0.3, 0.5, 0.7, 0.85, 0.95, 0.99] {
        let d = r * fraction;
        let lift = (r * r - d * d).sqrt();
        for i in 0..if fraction == 0. { 1 } else { 12 } {
            let angle = i as f32 * std::f32::consts::TAU / 12.;
            let origin = Vec3::new(at.x + d * angle.cos(), origin_y, at.z + d * angle.sin());
            if let Some(hit) = world.ray_body_excluding(origin, Dir3::NEG_Y, 4. + 2. * r, excluded)
            {
                if hit.normal.y > 0.6 {
                    let y = origin.y - hit.distance + lift;
                    rest = Some(rest.map_or(y, |old| old.max(y)));
                }
            }
        }
    }
    rest
}
#[allow(clippy::too_many_arguments)]
fn pickup(
    mut commands: Commands,
    design: Res<design::Design>,
    mut ball: ResMut<BallState>,
    placements: Res<WmoParts>,
    partitions: Res<WmoPartitions>,
    models: Res<Assets<WmoModel>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: benilla_world::model_render::M2BatchMaterials,
    mut uv: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut tint: ResMut<benilla_world::doodad_anim::TintAnimMaterials>,
    mut table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
    mut pending: ResMut<Pending>,
    mut requests: MessageWriter<RetireWmoPart>,
    world: WorldCollision,
    run: Res<Run>,
    manifest: Res<Manifest>,
) {
    let Some((cache, materials, light)) = mats.pieces() else {
        return;
    };
    if design.level.id != "stormwind" {
        return;
    }
    for p in &placements.0 {
        if manifest
            .parts
            .iter()
            .any(|part| part.id == p.part && part.finale)
        {
            continue;
        }
        if !p.ready
            || partitions.removed(p.map, p.uid, p.part)
            || pending.0.contains_key(&(p.map, p.uid, p.part))
        {
            continue;
        }
        let Some(m) = models.get(&p.model) else {
            continue;
        };
        let (lo, hi) = (p.min, p.max);
        let axes = hi - lo;
        if !axes.is_finite() || axes.min_element() <= 0.0 {
            continue;
        }
        let axis = axes.max_element();
        if collect::contact(ball.radius, axis) != Contact::RollUp {
            continue;
        }
        let point = nearest(ball.pos, &p.transform, lo, hi);
        if point.distance(ball.pos) > ball.radius {
            continue;
        }
        // A removed floor must not strand the player over a terrain hole or a deep basement.
        let Some(rest) = remaining_support(&world, ball.pos, ball.radius, &p.colliders) else {
            continue;
        };
        let floor = rest - ball.radius;
        if !floor.is_finite()
            || (floor - (ball.pos.y - ball.radius)).abs() > 2.0 + 0.5 * ball.radius
        {
            continue;
        }
        if !forms.require_static(&p.model, 0) {
            continue;
        }
        let slices = forms.slices(&p.model);
        let indices: Vec<_> = m
            .submesh_group
            .iter()
            .enumerate()
            .filter(|(i, g)| **g == p.group && !m.submeshes[*i].interior)
            .map(|(i, _)| i)
            .collect();
        if indices.is_empty() {
            continue;
        }
        let subs: Vec<_> = indices.iter().map(|&i| m.submeshes[i].clone()).collect();
        let stat: Vec<_> = indices.iter().map(|&i| slices.stat[i].clone()).collect();
        let center = (lo + hi) * 0.5;
        let object = Arc::new(benilla_world::interact::WorldObject {
            kind: ModelKind::Wmo,
            label: "City building trophy".into(),
            id: p.uid,
            detail: String::new(),
        });
        let mut geometry = || {
            spawn_model_entities(
                &mut commands,
                cache,
                materials,
                &light,
                &subs,
                FormSlices {
                    stat: &stat,
                    skin: None,
                },
                Transform::from_translation(-center),
                &object,
                ShadeSel::Matte,
                None,
                None,
                axes.length() * 0.5,
                center,
                None,
                None,
                &mut uv,
                &mut tint,
                &mut table,
                true,
                None,
                None,
                None,
            )
        };
        let spawned = geometry();
        let copies: Vec<_> = if (1..=14).contains(&p.part) || (40..=49).contains(&p.part) {
            (0..3).map(|_| geometry().entities).collect()
        } else {
            vec![]
        };
        if spawned.entities.is_empty() {
            continue;
        }
        let root = commands
            .spawn((
                RunEntity,
                CityTrophy,
                Transform::IDENTITY,
                Visibility::Hidden,
            ))
            .add_children(&spawned.entities)
            .id();
        let decor: Vec<_> = copies
            .into_iter()
            .map(|entities| {
                commands
                    .spawn((RunEntity, Transform::IDENTITY, Visibility::Hidden))
                    .add_children(&entities)
                    .id()
            })
            .collect();
        let volume =
            axes.x * axes.y * axes.z * level::FILL_FACTOR * 3.0 / (4.0 * std::f32::consts::PI);
        pending.0.insert(
            (p.map, p.uid, p.part),
            Prepared {
                root,
                decor,
                axes,
                volume,
                axis,
                at: run.elapsed,
                rotation: p.transform.rotation,
                floor,
                name: manifest
                    .parts
                    .iter()
                    .find(|part| part.id == p.part)
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                origin: p.transform.transform_point(center),
            },
        );
        // Eating keeps the roll's momentum (a dead stop on every building read as a snag);
        // the support was validated here and the retirement answers within a frame or two.
        ball.vel.y = 0.0;
        requests.write(RetireWmoPart {
            map: p.map,
            uid: p.uid,
            part: p.part,
        });
        info!(
            "azeroll city prepared: part={} axis={axis:.2} D={:.2} colliders={:?}",
            p.part,
            ball.diameter(),
            p.colliders
        );
        break;
    }
}
fn acknowledge(
    mut commands: Commands,
    mut replies: MessageReader<WmoPartRetired>,
    mut pending: ResMut<Pending>,
    mut ball: ResMut<BallState>,
    spin: Query<Entity, With<BallSpin>>,
    mut run: ResMut<Run>,
    mut collected: MessageWriter<Collected>,
    mut next: ResMut<NextState<Phase>>,
    mut reset_probed: Local<bool>,
    partitions: Res<WmoPartitions>,
    mut skin: ResMut<super::city_shell::Skin>,
    design: Res<design::Design>,
) {
    for reply in replies.read() {
        let Some(Prepared {
            root,
            decor,
            axes,
            volume,
            axis,
            at,
            rotation: world_rotation,
            floor,
            name,
            origin,
        }) = pending.0.remove(&(reply.map, reply.uid, reply.part))
        else {
            continue;
        };
        let Ok(spin) = spin.single() else {
            commands.entity(root).despawn();
            for e in decor {
                commands.entity(e).try_despawn();
            }
            continue;
        };
        if !reply.accepted {
            commands.entity(root).despawn();
            for e in decor {
                commands.entity(e).try_despawn();
            }
            continue;
        }
        for e in decor {
            skin.attach(&mut commands, e, spin, axes, world_rotation);
        }
        ball.volume += volume;
        ball.radius = ball.volume.cbrt();
        // The drop onto the validated floor is eased on screen by `ball::Shown`.
        ball.pos.y = floor + ball.radius;
        ball.vel.y = 0.0;
        // Reduced silhouette: the full building was the eligibility and volume target.
        commands.entity(root).insert((
            Visibility::Inherited,
            Transform {
                translation: origin,
                rotation: world_rotation,
                scale: Vec3::ONE,
            },
            Lift {
                elapsed: 0.,
                start: origin,
                rotation: world_rotation,
                spin,
                volume,
                axis,
                at,
            },
        ));
        run.count += 1;
        collected.write(Collected {
            entity: root,
            name: name.into(),
            kind: Kind::Prop,
            source: Source::Spawned,
            size: axis * 0.5,
            pickup: axis,
        });
        let required = design.goal.city_pieces as usize;
        let count = partitions.removed_count(reply.map, reply.uid);
        let banner = if required > 0 {
            format!("STORMWIND, TO GO!  {}/{}", count.min(required), required)
        } else {
            "STORMWIND, TO GO!".into()
        };
        run.banner = Some((banner, 3.5));
        // Native unattended lifecycle acceptance: exercise the actual Ready transition once,
        // then collect the restored placement again. Never enabled by ordinary gameplay.
        #[cfg(not(target_arch = "wasm32"))]
        if !*reset_probed
            && crate::webenv::var("WOW_AZEROLL_CITY_RESET_TEST").is_some_and(|count| {
                let count = count.parse::<usize>().unwrap_or(1).max(1);
                partitions.removed_count(reply.map, reply.uid) >= count
            })
        {
            *reset_probed = true;
            info!(
                "azeroll city probe: Ready after retirement part={}",
                reply.part
            );
            next.set(Phase::Ready);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (&mut next, &mut reset_probed, &partitions);
        info!(
            "azeroll city retired: part={} credit={volume:.2} r={:.2}",
            reply.part, ball.radius
        );
    }
}

#[derive(Component)]
pub(super) struct CityTrophy;

#[derive(Component)]
struct Lift {
    elapsed: f32,
    start: Vec3,
    rotation: Quat,
    spin: Entity,
    volume: f32,
    axis: f32,
    at: f32,
}
fn animate(
    mut commands: Commands,
    time: Res<Time<Real>>,
    ball: Res<BallState>,
    mut lifts: Query<(Entity, &mut Transform, &mut Lift)>,
) {
    for (entity, mut tf, mut lift) in &mut lifts {
        lift.elapsed += time.delta_secs();
        let u = (lift.elapsed / 1.15).clamp(0., 1.);
        let t = u * u * (3. - 2. * u);
        let scale = 1. + (TROPHY_SCALE - 1.) * t;
        let radial = ball.radius + 0.25 * lift.axis * TROPHY_SCALE;
        tf.translation = lift.start.lerp(ball.pos + Vec3::Y * radial, t)
            + Vec3::Y * (std::f32::consts::PI * u).sin() * lift.axis * 0.6;
        tf.scale = Vec3::splat(scale);
        if u >= 1. {
            let rot = ball.spin.inverse();
            commands.entity(entity).remove::<Lift>().insert((
                Transform {
                    translation: rot * Vec3::Y * radial,
                    rotation: rot * lift.rotation,
                    scale: Vec3::splat(TROPHY_SCALE),
                },
                Attached {
                    radial,
                    volume: lift.volume,
                    size: lift.axis * TROPHY_SCALE * 0.5,
                    at: lift.at,
                },
                ChildOf(lift.spin),
            ));
        }
    }
}

/// A planner-only target. Real collection is gated by the engine retirement acknowledgement.
#[derive(Component)]
pub(super) struct CityTarget;
#[derive(Resource, Default)]
struct Targets(HashMap<(u32, u32, u16), Entity>);
fn targets(
    mut commands: Commands,
    parts: Res<WmoParts>,
    partitions: Res<WmoPartitions>,
    manifest: Res<Manifest>,
    design: Res<design::Design>,
    mut targets: ResMut<Targets>,
    mut proxies: Query<&mut Transform, With<CityTarget>>,
    ball: Res<BallState>,
) {
    if design.level.id != "stormwind" {
        return;
    }
    targets.0.retain(|&(map, uid, part), e| {
        let resident = parts
            .0
            .iter()
            .any(|p| p.map == map && p.uid == uid && p.part == part);
        if !resident {
            commands.entity(*e).try_despawn();
        }
        resident
    });
    for p in &parts.0 {
        let key = (p.map, p.uid, p.part);
        if partitions.removed(p.map, p.uid, p.part) {
            if let Some(e) = targets.0.remove(&key) {
                commands.entity(e).try_despawn();
            }
            continue;
        }
        if !p.ready {
            continue;
        }
        // Aim at the near face rather than an interior pivot behind the building's own wall.
        // This is only a planner point; real eligibility/contact still uses the immutable OBB.
        let point = nearest(ball.pos, &p.transform, p.min, p.max);
        if let Some(&e) = targets.0.get(&key) {
            if let Ok(mut tf) = proxies.get_mut(e) {
                tf.translation = point;
            }
            continue;
        }
        let Some(part) = manifest.parts.iter().find(|x| x.id == p.part && !x.finale) else {
            continue;
        };
        let axes = p.max - p.min;
        let entity = commands
            .spawn((
                RunEntity,
                CityTarget,
                Transform::from_translation(point).with_rotation(p.transform.rotation),
                Collectible {
                    name: part.name.clone().into(),
                    kind: Kind::Prop,
                    source: Source::Spawned,
                    size: 0.0, // planner-only near-face point, not a second physical pickup sphere
                    pickup: axes.max_element(),
                    half_extents: axes * 0.5,
                    center: Vec3::ZERO,
                    solid: 0.,
                },
            ))
            .id();
        targets.0.insert(key, entity);
    }
}

// Native render-source isolation. Runs after the exterior owner and before propagation,
// touching only the requested class; collision, support and pickup metadata stay intact.
#[cfg(not(target_arch = "wasm32"))]
fn hide_capture_geometry(
    mut commands: Commands,
    mut terrain: Query<
        &mut Visibility,
        With<MeshMaterial3d<benilla_assets::materials::TerrainMaterial>>,
    >,
    balls: Query<Entity, With<Ball>>,
) {
    if crate::webenv::var("WOW_AZEROLL_NO_TERRAIN").is_some() {
        for mut visibility in &mut terrain {
            *visibility = Visibility::Hidden;
        }
    }
    if crate::webenv::var("WOW_AZEROLL_NO_BALL").is_some() {
        for entity in &balls {
            commands.entity(entity).insert(Visibility::Hidden);
        }
    }
}

/// Native content diagnosis: inspect future fuel after intermediate dislodge contacts.
#[cfg(not(target_arch = "wasm32"))]
fn audit_trees(
    run: Res<Run>,
    ball: Res<BallState>,
    mut next: Local<f32>,
    trees: Query<
        (
            Entity,
            &Transform,
            &Collectible,
            Has<collect::Shed>,
            Has<collect::Planted>,
        ),
        Without<Attached>,
    >,
) {
    if run.elapsed < 30.0 || run.elapsed < *next {
        return;
    }
    *next = run.elapsed + 15.0;
    let Some(path) = crate::webenv::var("WOW_AZEROLL_TREE_AUDIT") else {
        return;
    };
    let mut text = format!(
        "# t={} r={} ball={:?}\n",
        run.elapsed, ball.radius, ball.pos
    );
    for (entity, tf, c, shed, rooted) in &trees {
        if c.source != Source::Spawned || !c.name.contains("Tree") || c.pickup < 35.0 {
            continue;
        }
        let centre = tf.transform_point(c.center);
        text.push_str(&format!("{entity:?},axis={},root={:?},centre={centre:?},rotation={:?},dy={},distance={},shed={shed},rooted={rooted}\n",c.pickup,tf.translation,tf.rotation,centre.y-ball.pos.y,units::flat(centre-ball.pos).length()));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = std::io::Write::write_all(&mut file, text.as_bytes());
    }
}
