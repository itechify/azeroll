//! Mode-owned MDDF copies: the streamer keeps residency, and a run keeps collected-ID tombstones.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::materials::WowModelMaterial;
use benilla_assets::{m2_url, M2Model};
use benilla_world::model_fade::DoodadFade;
use benilla_world::model_render::{ModelKind, ShadeSel};
use benilla_world::schedule::WorldStage;
use benilla_world::terrain_stream::{
    m2_fade, spawn_model_entities, DoodadClaimed, DoodadReleased, PlacementClaims,
};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::MeshTag;
use bevy::prelude::*;

use super::camera::CamRig;
use super::{level, Attached, Ball, BallState, Collected, Collectible, Kind, Phase, Source};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Props>()
        .add_systems(Startup, install_claims)
        .add_systems(OnEnter(Phase::Ready), restart.after(super::reset_run))
        .add_systems(
            Update,
            (
                placements,
                assemble,
                convert_fades,
                occlusion_fade,
                trophy_unit_fade,
            )
                .chain()
                .after(WorldStage::Stream),
        );
}

// Contact-height sightline ghosts and trophy ghosts retain their approved blend twins.
// Overhead foliage uses the depth-gated screen-space cutout (`cutout.rs`) instead.

/// The ball's top, for "above the ball", sits this share of a radius over its real top.
const TOP_MARGIN: f32 = 0.15;
/// Only parts at least this fraction of the ball's radius across can bury it.
const OCCLUDER_SIZE: f32 = 0.25;
/// The near band in front of the lens, yards: a floor plus a share of the boom.
const NEAR_BAND: f32 = 1.2;
const NEAR_BAND_PER_BOOM: f32 = 0.12;
/// The sightline's thickness, as a fraction of the ball's radius.
const SIGHT_RADIUS: f32 = 1.0;
/// A fading copy keeps its fade while still within this much more reach (hysteresis).
const HYSTERESIS: f32 = 1.3;
/// Alpha rates per second.
const FADE_OUT_RATE: f32 = 6.0;
const FADE_IN_RATE: f32 = 2.5;
/// Contact-height geometry between the lens and the ball: see-through, so the clump stays in view.
const NEAR_GHOST: f32 = 0.35;
/// It must end at least this many radii short of the ball's centre along the view axis...
const NEAR_K: f32 = 1.25;
/// ...and stay at least this far (radii, plus `NEAR_GUARD_ABS` yards) from the ball's centre.
const NEAR_GUARD: f32 = 1.5;
const NEAR_GUARD_ABS: f32 = 0.15;
/// Hysteresis on the near rule: a ghosted part keeps its fade through this much more margin (radii).
const NEAR_STICKY: f32 = 0.2;
/// A trophy on the sightline, so the clump keeps a silhouette.
const TROPHY_GHOST: f32 = 0.45;
/// A trophy only ghosts when it is at least this fraction of the ball's radius across, reaches
/// this far past it, and sits on the camera's side of the clump.
const TROPHY_SIZE: f32 = 0.5;
const TROPHY_REACH: f32 = 1.5;
const TROPHY_FACING: f32 = 0.3;

/// A copy's model-space bounds and its fade state.
#[derive(Component)]
pub(super) struct Occluder {
    center: Vec3,
    half: Vec3,
    /// The faintest of its parts' alphas; below 1 the copy is still easing back.
    alpha: f32,
}

/// A part whose two materials the occlusion fade swaps between.
#[derive(Component)]
struct OccPart {
    foliage: bool,
    cutout: Handle<WowModelMaterial>,
    blend: Handle<WowModelMaterial>,
    alpha: f32,
    /// The 6-bit alpha last written to the part.
    applied: u32,
    /// Fading because it sits between the lens and the ball, so it eases back rather than snaps.
    near: bool,
}

/// A part still carrying the engine's `DoodadFade`, to be replaced by an `OccPart`.
#[derive(Component)]
struct ConvertFade;

#[derive(Component)]
pub(super) struct PropCopy {
    uid: Option<u32>,
    /// The M2 path, so the debug filler can copy what lies around the start.
    pub(super) model: String,
    handle: Handle<M2Model>,
}

#[derive(Component)]
struct Assembled;

struct Placement {
    model: String,
    transform: Transform,
    live: Option<Entity>,
}

#[derive(Resource, Default)]
struct Props {
    known: HashMap<u32, Placement>,
    tombstones: HashSet<u32>,
}

fn install_claims(mut claims: ResMut<PlacementClaims>) {
    let start = wow_to_bevy(level::START);
    claims.predicate = Some(Box::new(move |q| {
        let delta = q.transform.translation - start;
        delta.x * delta.x + delta.z * delta.z <= level::CLAIM_RADIUS * level::CLAIM_RADIUS
    }));
}

pub(super) fn spawn_copy(
    commands: &mut Commands,
    assets: &AssetServer,
    model: &str,
    transform: Transform,
    uid: Option<u32>,
) -> Entity {
    commands
        .spawn((
            transform,
            Visibility::default(),
            PropCopy {
                uid,
                model: model.to_owned(),
                handle: assets.load(m2_url(model)),
            },
        ))
        .id()
}

// Check ancestry as well as the marker: a release must never delete a trophy mid-attachment.
fn carried(
    entity: Entity,
    attached: &Query<(), With<Attached>>,
    parents: &Query<&ChildOf>,
    balls: &Query<(), With<Ball>>,
) -> bool {
    if attached.contains(entity) {
        return true;
    }
    let mut at = entity;
    while let Ok(parent) = parents.get(at) {
        at = parent.parent();
        if balls.contains(at) {
            return true;
        }
    }
    false
}

fn placements(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut props: ResMut<Props>,
    mut claimed: MessageReader<DoodadClaimed>,
    mut released: MessageReader<DoodadReleased>,
    mut collected: MessageReader<Collected>,
    copies: Query<(), With<PropCopy>>,
    attached: Query<(), With<Attached>>,
    parents: Query<&ChildOf>,
    balls: Query<(), With<Ball>>,
) {
    for event in collected.read() {
        if let Source::Doodad(uid) = event.source {
            props.tombstones.insert(uid);
        }
    }
    for event in released.read() {
        if let Some(placement) = props.known.get_mut(&event.unique_id) {
            if let Some(entity) = placement.live {
                if !carried(entity, &attached, &parents, &balls) {
                    commands.entity(entity).try_despawn();
                    placement.live = None;
                }
            }
        }
    }
    for event in claimed.read() {
        let tombstoned = props.tombstones.contains(&event.unique_id);
        let placement = props
            .known
            .entry(event.unique_id)
            .or_insert_with(|| Placement {
                model: event.model.clone(),
                transform: event.transform,
                live: None,
            });
        if !tombstoned && !placement.live.is_some_and(|e| copies.contains(e)) {
            placement.live = Some(spawn_copy(
                &mut commands,
                &assets,
                &placement.model,
                placement.transform,
                Some(event.unique_id),
            ));
        }
    }
}

fn restart(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut props: ResMut<Props>,
    copies: Query<(), With<PropCopy>>,
) {
    props.tombstones.clear();
    for (&uid, placement) in &mut props.known {
        if !placement.live.is_some_and(|e| copies.contains(e)) {
            placement.live = Some(spawn_copy(
                &mut commands,
                &assets,
                &placement.model,
                placement.transform,
                Some(uid),
            ));
        }
    }
}

fn assemble(
    mut commands: Commands,
    models: Res<Assets<M2Model>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: benilla_world::model_render::M2BatchMaterials,
    mut uv: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut tint: ResMut<benilla_world::doodad_anim::TintAnimMaterials>,
    mut table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
    pending: Query<
        (Entity, &PropCopy, &Transform, Option<&super::design::Gate>),
        Without<Assembled>,
    >,
) {
    let Some((cache, materials, light)) = mats.pieces() else {
        return;
    };
    for (entity, copy, transform, gate) in &pending {
        let Some(model) = models.get(&copy.handle) else {
            continue;
        };
        if !forms.require_static(&copy.handle, 0) {
            continue;
        }
        let (size, center) = m2_fade(&model.bounds, transform.scale.abs().max_element());
        let object = Arc::new(benilla_world::interact::WorldObject {
            kind: ModelKind::GameObject,
            label: copy.model.clone(),
            id: copy.uid.unwrap_or_default(),
            detail: String::new(),
        });
        // Frozen mesh parts, without bone hosts, world-static diversion or effect dressing.
        let spawned = spawn_model_entities(
            &mut commands,
            cache,
            materials,
            &light,
            &model.submeshes,
            forms.slices(&copy.handle),
            Transform::IDENTITY,
            &object,
            ShadeSel::Matte,
            None,
            None,
            size,
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
        );
        // Glow cards are world-facing effects; omit them rather than bake a moving pivot.
        let cards: HashSet<Entity> = model
            .submeshes
            .iter()
            .zip(&spawned.by_batch)
            .filter(|(sub, _)| sub.billboard.is_some())
            .filter_map(|(_, entity)| *entity)
            .collect();
        let parts: Vec<Entity> = spawned
            .entities
            .into_iter()
            .filter(|part| {
                if cards.contains(part) {
                    commands.entity(*part).despawn();
                    false
                } else {
                    // No small-prop distance fade or stale frustum verdict on a fast spinning trophy.
                    commands
                        .entity(*part)
                        .insert(ConvertFade)
                        .remove::<benilla_world::doodad_anim::MatAnim>()
                        .insert(NoFrustumCulling);
                    true
                }
            })
            .collect();
        commands
            .entity(entity)
            .add_children(&parts)
            .insert(Assembled);
        if parts.is_empty() || !size.is_finite() || size <= 0.0 || !center.is_finite() {
            continue;
        }
        let name = readable_name(&copy.model);
        let half_extents = model.bounds.map_or(
            Vec3::splat(size) / transform.scale.abs().max(Vec3::splat(0.001)),
            |b| {
                wow_to_bevy(std::array::from_fn(|i| {
                    (b.bbox_max[i] - b.bbox_min[i]).abs() * 0.5
                }))
                .abs()
            },
        );
        let solid = model
            .collision
            .as_ref()
            .filter(|hull| !hull.is_empty())
            .map_or(0.0, |hull| {
                collision_radius(
                    &name,
                    hull.positions.iter().copied().map(wow_to_bevy),
                    transform.scale.abs().max_element(),
                    (0.08
                        * half_extents.x.min(half_extents.z)
                        * transform.scale.abs().max_element())
                    .max(0.35),
                )
            });
        let pickup = (2.0 * half_extents * transform.scale.abs()).max_element();
        if trunk_solid(&name)
            && crate::webenv::var("WOW_AZEROLL_STALL_DEBUG").is_some()
            && transform
                .translation
                .distance(wow_to_bevy([-9450.0, 60.0, 57.0]))
                < 90.0
        {
            info!("azeroll trunk: uid={:?} name={name} origin={:?} radius={solid:.3} canopy_centre={center:?} half={half_extents:?}", copy.uid, transform.translation);
        }

        if model.bounds.is_some() {
            commands.entity(entity).insert(Occluder {
                center,
                half: half_extents,
                alpha: 1.0,
            });
        }
        commands.entity(entity).insert(Collectible {
            name: name.into(),
            kind: Kind::Prop,
            source: copy.uid.map_or(Source::Spawned, Source::Doodad),
            size,
            pickup,
            half_extents,
            center,
            // Scattered tabletop props must not form an impassable cage at the start.
            solid: if gate.is_some() {
                pickup * 0.85
            } else if copy.uid.is_none() {
                0.0
            } else {
                solid.min(pickup * 0.15)
            },
        });
    }
}

/// Standing trees collide at the model's foot, independently of their canopy pickup box.
pub(super) fn trunk_solid(name: &str) -> bool {
    name.split_whitespace()
        .any(|word| word.eq_ignore_ascii_case("tree"))
        && !name
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case("stump") || word.eq_ignore_ascii_case("fallen"))
}

/// Keep low foliage soft; canopy vertices must never enlarge a standing tree's wall.
fn collision_radius(
    name: &str,
    points: impl Iterator<Item = Vec3>,
    scale: f32,
    trunk_limit: f32,
) -> f32 {
    let points: Vec<_> = points.filter(|p| p.is_finite()).collect();
    let lower = name.to_ascii_lowercase();
    if points.is_empty()
        || lower.contains("bush")
        || lower.contains("grass")
        || lower.contains("fern")
        || lower.contains("flower")
    {
        return 0.0;
    }
    let lo = points
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let hi = points
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    if trunk_solid(name) {
        // Ignore buried root vertices. Sparse hulls may have no vertices at ground height;
        // cap their basal flare at 4% of the rendered full footprint, with a 0.35 yd floor.
        // The fallback is a trunk approximation, never the canopy's broad collision disc.
        let basal = points
            .iter()
            .filter(|p| p.y >= 0.0 && p.y <= 1.5 / scale.max(0.001))
            .map(|p| Vec2::new(p.x, p.z).length() * scale)
            .reduce(f32::max)
            .unwrap_or(trunk_limit);
        basal.min(trunk_limit)
    } else {
        let half = (hi - lo) * 0.5;
        Vec2::new(half.x, half.z).length() * scale
    }
}

fn readable_name(path: &str) -> String {
    super::names::display_name(path)
}

/// Swap a part's `DoodadFade` (distance fade, owned by the engine) for the occlusion fade's own
/// materials, restoring what that fade may have written in its one frame.
#[allow(clippy::type_complexity)]
fn convert_fades(
    mut commands: Commands,
    materials: Res<Assets<WowModelMaterial>>,
    mut parts: Query<
        (
            Entity,
            &DoodadFade,
            &mut Visibility,
            &mut MeshTag,
            &mut MeshMaterial3d<WowModelMaterial>,
        ),
        With<ConvertFade>,
    >,
) {
    for (entity, fade, mut vis, mut tag, mut mat) in &mut parts {
        let (cutout, blend) = fade.materials();
        *vis = Visibility::Inherited;
        tag.0 = benilla_world::mesh_tag::with_alpha(tag.0, 1.0);
        mat.0 = cutout.clone();
        commands
            .entity(entity)
            .insert(OccPart {
                foliage: materials
                    .get(cutout)
                    .is_some_and(|m| m.extension.clutter_fade.z as u32 & (1 << 14) != 0),
                cutout: cutout.clone(),
                blend: blend.clone(),
                alpha: 1.0,
                applied: 63,
                near: false,
            })
            .remove::<(DoodadFade, ConvertFade)>();
    }
}

/// Distance from `p` to the segment `a`..`b`.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1.0e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Whether the segment `a`..`b` (relative to the box centre) crosses the box of `half` extents.
fn segment_hits_box(a: Vec3, b: Vec3, half: Vec3) -> bool {
    segment_box_entry(a, b, half).is_some()
}

fn segment_box_entry(a: Vec3, b: Vec3, half: Vec3) -> Option<f32> {
    segment_box_interval(a, b, half).map(|(entry, _)| entry)
}

fn segment_box_interval(a: Vec3, b: Vec3, half: Vec3) -> Option<(f32, f32)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for i in 0..3 {
        if d[i].abs() < 1.0e-6 {
            if a[i].abs() > half[i] {
                return None;
            }
        } else {
            let inv = 1.0 / d[i];
            let (mut ta, mut tb) = ((-half[i] - a[i]) * inv, (half[i] - a[i]) * inv);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((t0, t1))
}

/// Claimed props no longer have engine colliders. Preserve their opaque, ball-height parts
/// as camera blockers too. Canopies may ghost; a trunk is handled by shortening the boom.
/// Bounds are conservative: this is deliberately not a replacement for WMO `cast_camera`.
pub(super) struct BoomBlocker {
    inverse: bevy::math::Affine3A,
    center: Vec3,
    half: Vec3,
}

/// Gather nearby parts once, so alternative boom probes do not rescan the entire level.
pub(super) fn camera_blockers(
    from: Vec3,
    reach: f32,
    ball_top: f32,
    copies: &Query<(&GlobalTransform, &Occluder, Option<&Children>), Without<Attached>>,
    bounds: &Query<&Aabb>,
) -> Vec<BoomBlocker> {
    let mut blockers = Vec::new();
    for (gt, occ, children) in copies.iter() {
        let m = gt.affine().matrix3;
        let scale = Vec3::new(m.x_axis.length(), m.y_axis.length(), m.z_axis.length())
            .max(Vec3::splat(1.0e-3));
        let wc = gt.transform_point(occ.center);
        let wr = (occ.half * scale).length();
        if wc.distance_squared(from) > (reach + wr + 0.2).powi(2) {
            continue;
        }
        for &child in children.map(|c| &**c).unwrap_or_default() {
            let Ok(a) = bounds.get(child) else {
                continue;
            };
            let c = Vec3::from(a.center);
            let h = Vec3::from(a.half_extents);
            let height = (m.x_axis.abs() * h.x + m.y_axis.abs() * h.y + m.z_axis.abs() * h.z).y;
            if gt.transform_point(c).y - height > ball_top {
                continue;
            }
            blockers.push(BoomBlocker {
                inverse: gt.affine().inverse(),
                center: c,
                half: h + Vec3::splat(0.2) / scale,
            });
        }
    }
    blockers
}

pub(super) fn prop_boom_limit(
    from: Vec3,
    boom: Vec3,
    blockers: &[BoomBlocker],
    min_distance: f32,
) -> f32 {
    let len = boom.length();
    let mut limit = len;
    for b in blockers {
        if let Some(t) = segment_box_entry(
            b.inverse.transform_point3(from) - b.center,
            b.inverse.transform_point3(from + boom) - b.center,
            b.half,
        ) {
            // Broad render boxes overlapping the clump cannot be resolved by shortening:
            // it puts the lens inside the ball. Use their orbit penalty instead. This exemption
            // is only for approximate prop bounds; cast_camera still enforces every world hit.
            if t * len > min_distance {
                limit = limit.min((t * len - 0.1).max(0.0));
            }
        }
    }
    limit
}

/// A broad render box can contain the ball even though its actual trunk does not. In that case
/// shortening cannot help. Prefer a bearing that exits the box sooner and keeps the eye outside.
pub(super) fn prop_view_penalty(from: Vec3, boom: Vec3, blockers: &[BoomBlocker]) -> f32 {
    blockers
        .iter()
        .filter_map(|b| {
            segment_box_interval(
                b.inverse.transform_point3(from) - b.center,
                b.inverse.transform_point3(from + boom) - b.center,
                b.half,
            )
            .map(|(entry, exit)| (exit - entry).max(0.35) + if exit >= 0.999 { 1.0 } else { 0.0 })
        })
        .fold(0.0, f32::max)
}

/// Whether a contact-height part (box `c`/`h` in `gt`'s local space) lies between the lens and the
/// ball: on the eye -> look sightline, wholly nearer the lens than the ball's near side
/// (`NEAR_K` radii short of its centre, along the view axis), and outside the margin around the
/// ball. Anything beside, behind or around the ball fails, so it stays opaque. `sticky` loosens
/// the margins for a part already fading, so it does not flicker at the edge.
#[allow(clippy::too_many_arguments)]
fn sight_ghost(
    gt: &GlobalTransform,
    c: Vec3,
    h: Vec3,
    eye: Vec3,
    look: Vec3,
    ball: Vec3,
    r: f32,
    slack: f32,
    sticky: bool,
) -> bool {
    let sticky = if sticky { NEAR_STICKY } else { 0.0 };
    let view = (look - eye).normalize_or_zero();
    let ball_along = (ball - eye).dot(view);
    let m = gt.affine().matrix3;
    let wp = gt.transform_point(c);
    let along = Vec3::from(m.x_axis).dot(view).abs() * h.x
        + Vec3::from(m.y_axis).dot(view).abs() * h.y
        + Vec3::from(m.z_axis).dot(view).abs() * h.z;
    if (wp - eye).dot(view) + along > ball_along - r * (NEAR_K - sticky) {
        return false;
    }
    let (s, q, t) = gt.to_scale_rotation_translation();
    let s = s.abs().max(Vec3::splat(1.0e-3));
    let inv = q.inverse();
    let local = |p: Vec3| inv * (p - t) / s;
    let gap = (((local(ball) - c).abs() - h).max(Vec3::ZERO) * s).length();
    gap > r * (NEAR_GUARD - sticky) + NEAR_GUARD_ABS
        && segment_hits_box(
            local(eye) - c,
            local(look) - c,
            h + Vec3::splat(r * SIGHT_RADIUS * slack) / s,
        )
}

/// Ghost what stands between the lens and the ball, or in the lens's face, without ever hiding what
/// the ball can run into. Per part: geometry above the ball's top on the sightline fades to a
/// ghost; only parts wholly above the ball can ghost around the lens; the big
/// trophies that poke out toward the lens ghost so the clump stays readable. A bounding-sphere
/// reject first; only the few copies that pass pay for the oriented box tests.
#[allow(clippy::type_complexity)]
fn occlusion_fade(
    time: Res<Time<Real>>,
    rig: Res<CamRig>,
    ball: Res<BallState>,
    collision: benilla_world::collision::WorldCollision,
    mut metric_t: Local<f32>,
    mut copies: Query<
        (
            &GlobalTransform,
            &mut Occluder,
            Option<&Children>,
            Has<Attached>,
        ),
        (With<PropCopy>, With<Assembled>),
    >,
    mut parts: Query<(
        &mut OccPart,
        Option<&Aabb>,
        &mut MeshTag,
        &mut MeshMaterial3d<WowModelMaterial>,
    )>,
) {
    if rig.eye == Vec3::ZERO {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let (eye, look, r) = (rig.eye, rig.look, ball.radius);
    let boom = eye.distance(look);
    let band = NEAR_BAND + NEAR_BAND_PER_BOOM * boom;
    let sight = r * SIGHT_RADIUS;
    // All parts overlapping the full ball height remain opaque.
    let top = ball.pos.y + r * (1.0 + TOP_MARGIN);
    let to_eye = (eye - ball.pos).normalize_or_zero();
    // Nine sight probes stop on the visible core surface, not at its centre. These measure
    // geometry coverage, not pixel opacity (cutout leaves and bounding boxes are conservative).
    *metric_t += time.delta_secs();
    let measure = *metric_t >= 1.0;
    if measure {
        *metric_t %= 1.0;
    }
    let right = to_eye.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(to_eye).normalize_or_zero();
    let targets: [Vec3; 9] = std::array::from_fn(|i| {
        let x = (i % 3) as f32 - 1.0;
        let y = (i / 3) as f32 - 1.0;
        let offset = (right * x + up * y) * r * 0.5;
        ball.pos + offset + to_eye * (r * r - offset.length_squared()).max(0.0).sqrt()
    });
    let mut coverage = [0.0_f32; 9];
    let mut ray_coverage = [0.0_f32; 9];
    let mut external = [0.0_f32; 9];
    if measure {
        for (i, target) in targets.iter().enumerate() {
            let ray = *target - eye;
            // The public bare-ray API traces body geometry (one-sided). The sphere metric
            // below also covers WMO DETAIL/camera faces through cast_camera's own filter.
            if let Ok(dir) = Dir3::new(ray) {
                if collision
                    .ray_body(eye, dir, (ray.length() - 0.05).max(0.0))
                    .is_some()
                {
                    ray_coverage[i] = 1.0;
                    external[i] = 1.0;
                }
            }
            if collision
                .cast_camera(eye, ray, false)
                .is_some_and(|d| d < ray.length() - 0.1)
            {
                coverage[i] = 1.0;
            }
        }
    }
    for (gt, mut occ, children, trophy) in &mut copies {
        let wc = gt.transform_point(occ.center);
        let m = gt.affine().matrix3;
        let scale = Vec3::new(m.x_axis.length(), m.y_axis.length(), m.z_axis.length());
        let wr = (occ.half * scale).length();
        let slack = if occ.alpha < 1.0 { HYSTERESIS } else { 1.0 };
        // A tiny trophy can fill the screen when a wall compresses the boom. The normal
        // silhouette size/facing gate does not apply to geometry around the lens itself.
        let trophy_near_eye = trophy
            && wc.distance(eye) - wr < benilla_world::collision::CAMERA_PROBE_RADIUS + r * 0.25;
        let near_maybe = trophy_near_eye || (!trophy && wc.distance(eye) - wr < band * slack);
        let line_maybe = segment_distance(wc, eye, look) < wr + sight * slack
            && (!trophy
                || (wc.distance(ball.pos) + wr > r * TROPHY_REACH
                    && (occ.half * scale).max_element() * 2.0 > r * TROPHY_SIZE
                    && (wc - ball.pos).dot(to_eye) > r * TROPHY_FACING));
        if !near_maybe && !line_maybe && occ.alpha >= 1.0 - 1.0e-4 {
            continue;
        }
        let (s, q, t) = gt.to_scale_rotation_translation();
        let s = s.abs().max(Vec3::splat(1.0e-3));
        let inv = q.inverse();
        let local = |p: Vec3| inv * (p - t) / s;
        let (le, ll) = (local(eye), local(look));
        // A trophy is judged as a whole, so it ghosts in one piece.
        let trophy_target = if trophy_near_eye {
            TROPHY_GHOST
        } else if trophy && line_maybe {
            let half = occ.half + Vec3::splat(sight * 0.3 * slack) / s;
            if segment_hits_box(le - occ.center, ll - occ.center, half) {
                TROPHY_GHOST
            } else {
                1.0
            }
        } else {
            1.0
        };
        let mut faintest = 1.0_f32;
        for &child in children.map(|c| &**c).unwrap_or_default() {
            let Ok((mut part, aabb, mut tag, mut mat)) = parts.get_mut(child) else {
                continue;
            };
            let (c, h) = aabb.map_or((occ.center, occ.half), |a| {
                (Vec3::from(a.center), Vec3::from(a.half_extents))
            });
            if measure {
                for (i, target) in targets.iter().enumerate() {
                    if segment_hits_box(le - c, local(*target) - c, h) {
                        coverage[i] = coverage[i].max(part.alpha);
                        ray_coverage[i] = ray_coverage[i].max(part.alpha);
                        if !trophy {
                            external[i] = external[i].max(part.alpha);
                        }
                    }
                }
            }
            let mut target = 1.0_f32;
            if trophy {
                target = trophy_target;
            } else if !part.foliage {
                let wp = gt.transform_point(c);
                let wh =
                    Vec3::from(m.x_axis.abs() * h.x + m.y_axis.abs() * h.y + m.z_axis.abs() * h.z);
                let low = wp.y - wh.y;
                let solid = low <= top;
                // Contact-height geometry ghosts only when it sits on the sightline and ends
                // wholly short of the ball's near side, and never within the margin around it.
                let near_ghost = solid
                    && line_maybe
                    && (h * s).max_element() * 2.0 > r * OCCLUDER_SIZE
                    && sight_ghost(gt, c, h, eye, look, ball.pos, r, slack, part.near);
                if near_ghost {
                    target = NEAR_GHOST;
                    part.near = true;
                } else if solid {
                    // Growth/rotation can turn an overhead part into contact geometry, and the
                    // contact cue takes priority over easing its old canopy alpha back in. A part
                    // that was ghosted as lens-side geometry does ease back.
                    if !part.near {
                        part.alpha = 1.0;
                    }
                }
                // Overhead canopy/lens alpha ghosts are replaced by the foliage-only shader
                // window. Non-foliage and the contact band stay solid outside sight_ghost.
            }

            if (part.alpha - target).abs() > 1.0e-4 {
                let step = if target < part.alpha {
                    FADE_OUT_RATE
                } else {
                    FADE_IN_RATE
                } * dt;
                part.alpha += (target - part.alpha).clamp(-step, step);
            }
            if part.alpha >= 0.999 {
                part.near = false;
            }
            faintest = faintest.min(part.alpha);
            let bits = (part.alpha.clamp(0.0, 1.0) * 63.0).round() as u32;
            if bits != part.applied {
                part.applied = bits;
                tag.0 = benilla_world::mesh_tag::with_alpha(tag.0, bits as f32 / 63.0);
                let want = if bits >= 63 {
                    &part.cutout
                } else {
                    &part.blend
                };
                if mat.0 != *want {
                    mat.0 = want.clone();
                }
            }
        }
        occ.alpha = faintest;
    }
    if measure {
        let blocked = coverage.iter().filter(|a| **a > 0.0).count();
        let opaque = coverage.iter().filter(|a| **a >= 0.8).count();
        let rays = ray_coverage.iter().filter(|a| **a >= 0.8).count() as f32 / 9.0;
        let external = external.iter().filter(|a| **a >= 0.8).count() as f32 / 9.0;
        let invalid = collision
            .cast_camera(look, eye - look, false)
            .is_some_and(|d| d < boom - 0.1);
        info!(
            "azeroll visibility: r={r:.2} blocked={:.3} opaque={:.3} coverage={:.3} body_rays={rays:.3} external={external:.3} boom={:.2} open={:.3} invalid={invalid} pos={:?}",
            blocked as f32 / 9.0,
            opaque as f32 / 9.0,
            coverage.iter().sum::<f32>() / 9.0,
            boom,
            rig.open,
            ball.pos
        );
    }
}

/// Units use the engine's composed model alpha, including held weapons, rather than doodad
/// materials. Track our declaration so shedding restores an opaque, collidable creature at once.
#[derive(Component)]
struct UnitGhost;

#[allow(clippy::type_complexity)]
fn trophy_unit_fade(
    mut commands: Commands,
    time: Res<Time<Real>>,
    rig: Res<CamRig>,
    ball: Res<BallState>,
    units: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&Attached>,
            Option<&Collectible>,
            Option<&benilla_world::model_fade::ModelFade>,
            Has<UnitGhost>,
        ),
        (
            Without<PropCopy>,
            Or<(With<crate::entities::Carried>, With<UnitGhost>)>,
        ),
    >,
) {
    if rig.eye == Vec3::ZERO {
        return;
    }
    let facing = (rig.eye - ball.pos).normalize_or_zero();
    for (entity, gt, attached, collectible, declared, ours) in &units {
        let Some(a) = attached else {
            if ours {
                commands
                    .entity(entity)
                    .remove::<(UnitGhost, benilla_world::model_fade::ModelFade)>();
            }
            continue;
        };
        let center = gt.transform_point(collectible.map_or(Vec3::ZERO, |c| c.center));
        let near_eye =
            center.distance(rig.eye) < a.size * 1.5 + benilla_world::collision::CAMERA_PROBE_RADIUS;
        let blocking = segment_distance(center, rig.eye, ball.pos) < a.size
            && (center - ball.pos).dot(facing) > ball.radius * 0.2
            && a.size > ball.radius * 0.3;
        let target = if near_eye || blocking {
            TROPHY_GHOST
        } else {
            1.0
        };
        let old = if ours {
            declared.map_or(1.0, |f| f.0)
        } else {
            1.0
        };
        let rate = if target < old {
            FADE_OUT_RATE
        } else {
            FADE_IN_RATE
        };
        let alpha = old
            + (target - old).clamp(
                -rate * time.delta_secs().min(0.1),
                rate * time.delta_secs().min(0.1),
            );
        if alpha < 1.0 {
            commands
                .entity(entity)
                .insert((UnitGhost, benilla_world::model_fade::ModelFade(alpha)));
        } else if ours {
            commands
                .entity(entity)
                .remove::<(UnitGhost, benilla_world::model_fade::ModelFade)>();
        }
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;

    #[test]
    fn near_clump_bounds_cannot_collapse_the_lens() {
        let b = BoomBlocker {
            inverse: bevy::math::Affine3A::IDENTITY,
            center: Vec3::Z * 0.55,
            half: Vec3::splat(0.2),
        };
        let boom = Vec3::Z * 5.0;
        assert_eq!(prop_boom_limit(Vec3::ZERO, boom, &[b], 1.0), 5.0);
    }

    #[test]
    fn distant_trunk_still_shortens_the_boom() {
        let b = BoomBlocker {
            inverse: bevy::math::Affine3A::IDENTITY,
            center: Vec3::Z * 3.0,
            half: Vec3::splat(0.5),
        };
        assert!((prop_boom_limit(Vec3::ZERO, Vec3::Z * 5.0, &[b], 1.0) - 2.4).abs() < 1.0e-5);
    }

    #[test]
    fn boom_stops_at_near_face_from_either_side() {
        let h = Vec3::ONE;
        assert_eq!(
            segment_box_entry(Vec3::Z * 5.0, -Vec3::Z * 5.0, h),
            Some(0.4)
        );
        assert_eq!(
            segment_box_entry(-Vec3::Z * 5.0, Vec3::Z * 5.0, h),
            Some(0.4)
        );
    }

    #[test]
    fn parallel_boom_outside_trunk_is_clear() {
        let a = Vec3::new(1.1, 0.0, 5.0);
        assert_eq!(segment_box_entry(a, a - Vec3::Z * 10.0, Vec3::ONE), None);
    }

    #[test]
    fn inside_box_is_distinguished_from_near_face() {
        assert_eq!(
            segment_box_entry(Vec3::ZERO, Vec3::Z * 5.0, Vec3::ONE),
            Some(0.0)
        );
    }

    fn post_at(p: Vec3) -> GlobalTransform {
        GlobalTransform::from(Transform::from_translation(p))
    }

    // A 0.5 yd ball at the origin, the lens 4 yd behind it and a little above.
    const EYE: Vec3 = Vec3::new(0.0, 1.5, 4.0);
    const LOOK: Vec3 = Vec3::new(0.0, 0.3, 0.0);
    const POST: Vec3 = Vec3::new(0.0, 0.6, 0.0);
    const POST_HALF: Vec3 = Vec3::new(0.15, 0.6, 0.15);

    fn ghosts(at: Vec3, sticky: bool) -> bool {
        sight_ghost(
            &post_at(at),
            Vec3::ZERO,
            POST_HALF,
            EYE,
            LOOK,
            LOOK,
            0.5,
            1.0,
            sticky,
        )
    }

    #[test]
    fn post_between_lens_and_ball_ghosts() {
        assert!(ghosts(Vec3::new(0.0, 0.0, 2.0), false));
        assert!(ghosts(POST + Vec3::new(0.0, 0.0, 1.8), false));
    }

    #[test]
    fn post_beside_behind_or_around_the_ball_stays_opaque() {
        // ahead of the ball, and behind it from the lens
        assert!(!ghosts(Vec3::new(0.0, 0.0, -1.5), false));
        // beside it, off the sightline
        assert!(!ghosts(Vec3::new(1.2, 0.0, 0.0), false));
        assert!(!ghosts(Vec3::new(-0.9, 0.0, 0.6), false));
        // touching the ball's near side
        assert!(!ghosts(Vec3::new(0.0, 0.0, 0.5), false));
        // a box the ball sits inside
        assert!(!sight_ghost(
            &post_at(Vec3::new(0.0, 0.3, 0.0)),
            Vec3::ZERO,
            Vec3::new(2.0, 1.0, 2.0),
            EYE,
            LOOK,
            LOOK,
            0.5,
            1.0,
            false
        ));
    }

    #[test]
    fn post_around_the_lens_ghosts() {
        assert!(ghosts(Vec3::new(0.0, 1.2, 3.9), false));
    }

    #[test]
    fn ghosted_post_keeps_its_fade_a_little_longer() {
        // Just inside the margin: opaque when fresh, still ghosted once fading.
        let edge = Vec3::new(0.0, 0.0, 1.05);
        assert!(!ghosts(edge, false));
        assert!(ghosts(edge, true));
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;

    #[test]
    fn tree_wall_ignores_canopy_and_soft_foliage_has_no_wall() {
        let hull = [
            Vec3::new(0.4, 0.0, 0.0),
            Vec3::new(-0.4, 1.0, 0.0),
            Vec3::new(12.0, 8.0, 9.0),
        ];
        assert!(
            (collision_radius("Elwynn Canopy Tree", hull.into_iter(), 2.0, 1.0) - 0.8).abs() < 1e-6
        );
        assert_eq!(
            collision_radius("Elwynn Bush", hull.into_iter(), 1.0, 1.0),
            0.0
        );
        assert_eq!(
            collision_radius("Thornbush", hull.into_iter(), 1.0, 1.0),
            0.0
        );
        assert!(!trunk_solid("Street Lamp"));
        let roots = [Vec3::new(8.0, -5.0, 0.0), Vec3::new(4.0, 8.0, 0.0)];
        assert_eq!(
            collision_radius("Elwynn Canopy Tree", roots.into_iter(), 1.0, 1.5),
            1.5
        );

        assert!(collision_radius("Crate", hull.into_iter(), 1.0, 1.0) > 5.0);
    }
}
