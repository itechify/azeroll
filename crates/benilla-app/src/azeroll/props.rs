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
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::MeshTag;
use bevy::prelude::*;

use super::camera::CamRig;
use super::{Attached, Ball, BallState, Collected, Collectible, Kind, Phase, Source};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Props>()
        .add_systems(Startup, install_claims)
        .add_systems(OnEnter(Phase::Ready), restart.after(super::reset_run))
        .add_systems(
            Update,
            (placements, assemble, lighting_anchors, convert_fades)
                .chain()
                .after(WorldStage::Stream)
                .before(benilla_world::interior::classify_entity_interior),
        )
        .add_systems(
            Update,
            (occlusion_fade, trophy_unit_fade)
                .after(benilla_world::interior::classify_entity_interior),
        );
}

// Only trophies ghost whole parts (blend twins). Everything else that stands between the lens and
// the ball is seen through per pixel by the screen-space cutout (`cutout.rs`).

/// The sightline's thickness, as a fraction of the ball's radius.
const SIGHT_RADIUS: f32 = 1.0;
/// A fading copy keeps its fade while still within this much more reach (hysteresis).
const HYSTERESIS: f32 = 1.3;
/// Alpha rates per second.
const FADE_OUT_RATE: f32 = 6.0;
const FADE_IN_RATE: f32 = 2.5;
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
    cutout: Handle<WowModelMaterial>,
    blend: Handle<WowModelMaterial>,
    alpha: f32,
    /// The 6-bit alpha last written to the part.
    applied: u32,
}

/// A part still carrying the engine's `DoodadFade`, to be replaced by an `OccPart`.
#[derive(Component)]
struct ConvertFade;

#[derive(Component)]
struct InteriorBlend(Handle<WowModelMaterial>);

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

/// Claim the doodads inside the union of every level's zones, once: the streamer cannot re-evaluate
/// a loaded tile, so switching levels never re-claims.
fn install_claims(campaign: Res<super::campaign::Campaign>, mut claims: ResMut<PlacementClaims>) {
    let zones: Vec<_> = campaign
        .levels
        .iter()
        .flat_map(|l| l.claims.iter().copied())
        .collect();
    claims.predicate = Some(Box::new(move |q| {
        zones.iter().any(|z| z.contains(q.transform.translation))
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
            benilla_world::interior::ContainmentAttach,
            PropCopy {
                uid,
                model: model.to_owned(),
                handle: assets.load(m2_url(model)),
            },
        ))
        .id()
}

// Shared ball anchor avoids a floor ray and probe update for every spinning trophy.
fn lighting_anchors(
    mut commands: Commands,
    copies: Query<
        (Entity, Has<Attached>, &Children),
        (
            With<PropCopy>,
            With<Assembled>,
            Or<(
                Added<Assembled>,
                Added<Attached>,
                Changed<ChildOf>,
                Changed<Transform>,
            )>,
        ),
    >,
    balls: Query<(Entity, Has<benilla_world::interior::BodyBakeCenter>), With<Ball>>,
    parts: Query<&benilla_world::interior::ClassifiedBy>,
) {
    let Ok((ball, centered)) = balls.single() else {
        return;
    };
    if !centered {
        commands
            .entity(ball)
            .insert(benilla_world::interior::BodyBakeCenter(Vec3::ZERO));
    }
    for (root, attached, children) in &copies {
        let anchor = if attached { ball } else { root };
        if attached {
            commands.entity(root).remove::<(
                benilla_world::terrain_stream::PropLight,
                benilla_world::lighting::PropProbeSlot,
                benilla_world::interior::InteriorAnchor,
                benilla_world::interior::BakeState,
            )>();
        }
        for child in children.iter() {
            if parts.get(child).is_ok_and(|by| by.0 != anchor) {
                commands
                    .entity(child)
                    .insert(benilla_world::interior::ClassifiedBy(anchor));
                // Requeue after reparenting even when the ball's law is already settled.
                commands.queue(move |w: &mut World| {
                    w.resource_mut::<benilla_world::interior::InteriorReauthor>()
                        .0
                        .push(child);
                });
            }
        }
    }
}

// Check ancestry as well as the marker: a release must never delete a trophy mid-attachment.
pub(super) fn carried(
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
    copies: Query<Has<super::lod::Cleaned>, With<PropCopy>>,
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
                // Retired visuals retain only a food ledger; preserve it across streaming.
                if copies.get(entity).unwrap_or(false) {
                    continue;
                }
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
        (
            Entity,
            &PropCopy,
            &Transform,
            Option<&super::design::Gate>,
            Option<&super::furniture::Furniture>,
            Option<&super::design::GoalProp>,
            Has<super::design::Blocker>,
            Option<&super::design::Present>,
        ),
        Without<Assembled>,
    >,
) {
    let Some((cache, materials, light)) = mats.pieces() else {
        return;
    };
    for (entity, copy, transform, gate, furniture, goal, blocker, present) in &pending {
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
        if crate::webenv::var("WOW_AZEROLL_NO_PROP_INTERIOR").is_none() {
            commands
                .entity(entity)
                .insert(benilla_world::interior::BodyBakeCenter(center));
            for (i, (sub, part)) in model.submeshes.iter().zip(&spawned.by_batch).enumerate() {
                let Some(part) = part else { continue };
                let (bake, blend) = benilla_world::terrain_stream::frozen_interior_materials(
                    cache,
                    materials,
                    &light,
                    sub,
                    (i + 1) as u16,
                );
                // Exterior handles come from the original spawner, preserving outdoor shading.
                commands.entity(*part).insert(InteriorBlend(blend));
                commands.entity(*part).queue(move |mut e: EntityWorldMut| {
                    let Some(mat) = e.get::<MeshMaterial3d<WowModelMaterial>>() else {
                        return;
                    };
                    let exterior = mat.0.clone();
                    if let Some(lit) = benilla_world::interior::part_interior_lit(
                        &exterior,
                        Some(&exterior),
                        Some(&bake),
                        center,
                        entity,
                    ) {
                        e.insert(lit);
                    }
                });
            }
        }
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
        let name = present.map_or_else(|| readable_name(&copy.model), |p| p.0.clone());
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
            info!(
                "azeroll trunk: uid={:?} name={name} origin={:?} radius={solid:.3} canopy_centre={center:?} half={half_extents:?}",
                copy.uid, transform.translation
            );
        }

        if model.bounds.is_some() {
            commands.entity(entity).insert(Occluder {
                center,
                half: half_extents,
                alpha: 1.0,
            });
        }
        if let Some(goal) = goal {
            info!(
                "azeroll: goal prop {:?} ({name}) pickup={pickup:.2}",
                goal.0
            );
        }
        commands.entity(entity).insert(Collectible {
            name: goal.map_or_else(|| name.into(), |g| g.0.clone().into()),
            kind: if goal.is_some() {
                Kind::Boss
            } else {
                Kind::Prop
            },
            source: furniture.map_or_else(
                || copy.uid.map_or(Source::Spawned, Source::Doodad),
                |f| Source::WmoProp(f.0),
            ),
            size,
            pickup: if blocker { f32::INFINITY } else { pickup },
            half_extents,
            center,
            // Scattered tabletop props must not form an impassable cage at the start.
            solid: if furniture.is_some_and(|f| f.0.placement == 10047 && f.0.map == 0) {
                // Outdoor city MODDs use their real basal hull approximation. A canopy, statue
                // bow or cloth banner must not become an inn-style floor-to-ceiling box.
                solid.min(pickup * 0.15)
            } else if furniture.is_some() {
                pickup * 0.5
            } else if gate.is_some() {
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
                cutout: cutout.clone(),
                blend: blend.clone(),
                alpha: 1.0,
                applied: 63,
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

/// Fade the big trophies that poke out toward the lens, so the clump keeps a readable silhouette,
/// and ease any part that was ghosted back to opaque. World geometry between the lens and the ball
/// is not handled here: the screen-space window (`cutout.rs`) sees through it per pixel, so the
/// camera never moves for it.
fn occlusion_fade(
    time: Res<Time<Real>>,
    rig: Res<CamRig>,
    ball: Res<BallState>,
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
        &mut MeshTag,
        &mut MeshMaterial3d<WowModelMaterial>,
        Option<&benilla_world::interior::InteriorLit>,
        Option<&InteriorBlend>,
    )>,
) {
    let _profile = super::profile::span("props::occlusion_fade");
    if rig.eye == Vec3::ZERO {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let (eye, look, r) = (rig.eye, rig.look, ball.radius);
    let sight = r * SIGHT_RADIUS;
    let to_eye = (eye - ball.pos).normalize_or_zero();
    for (gt, mut occ, children, trophy) in &mut copies {
        if !trophy && occ.alpha >= 1.0 - 1.0e-4 {
            continue;
        }
        let wc = gt.transform_point(occ.center);
        let m = gt.affine().matrix3;
        let scale = Vec3::new(m.x_axis.length(), m.y_axis.length(), m.z_axis.length());
        let wr = (occ.half * scale).length();
        let slack = if occ.alpha < 1.0 { HYSTERESIS } else { 1.0 };
        // A tiny trophy can fill the screen when the lens sits close to the clump. The normal
        // silhouette size/facing gate does not apply to geometry around the lens itself.
        let trophy_near_eye = trophy
            && wc.distance(eye) - wr < benilla_world::collision::CAMERA_PROBE_RADIUS + r * 0.25;
        let line_maybe = trophy
            && segment_distance(wc, eye, look) < wr + sight * slack
            && wc.distance(ball.pos) + wr > r * TROPHY_REACH
            && (occ.half * scale).max_element() * 2.0 > r * TROPHY_SIZE
            && (wc - ball.pos).dot(to_eye) > r * TROPHY_FACING;
        // A trophy is judged as a whole, so it ghosts in one piece.
        let target = if trophy_near_eye {
            TROPHY_GHOST
        } else if line_maybe {
            let (s, q, t) = gt.to_scale_rotation_translation();
            let s = s.abs().max(Vec3::splat(1.0e-3));
            let inv = q.inverse();
            let local = |p: Vec3| inv * (p - t) / s;
            let half = occ.half + Vec3::splat(sight * 0.3 * slack) / s;
            if segment_hits_box(local(eye) - occ.center, local(look) - occ.center, half) {
                TROPHY_GHOST
            } else {
                1.0
            }
        } else {
            1.0
        };
        let mut faintest = 1.0_f32;
        for &child in children.map(|c| &**c).unwrap_or_default() {
            let Ok((mut part, mut tag, mut mat, lit, indoor_blend)) = parts.get_mut(child) else {
                continue;
            };
            if (part.alpha - target).abs() > 1.0e-4 {
                let step = if target < part.alpha {
                    FADE_OUT_RATE
                } else {
                    FADE_IN_RATE
                } * dt;
                part.alpha += (target - part.alpha).clamp(-step, step);
            }
            faintest = faintest.min(part.alpha);
            let bits = (part.alpha.clamp(0.0, 1.0) * 63.0).round() as u32;
            if bits != part.applied {
                part.applied = bits;
                tag.0 = benilla_world::mesh_tag::with_alpha(tag.0, bits as f32 / 63.0);
            }
            {
                let want = if bits >= 63 {
                    lit.map_or(&part.cutout, |l| l.steady_material())
                } else if lit.is_some_and(|l| l.is_bake()) {
                    indoor_blend.map_or(&part.blend, |b| &b.0)
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
    let _profile = super::profile::span("props::trophy_unit_fade");
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
    fn boom_segment_enters_at_the_near_face_from_either_side() {
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
    fn parallel_segment_outside_the_box_misses() {
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

#[cfg(test)]
mod lighting_tests {
    use super::*;
    use benilla_world::interior::{ClassifiedBy, InteriorReauthor};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn azeroll_trophy_light_follows_ball_and_returns_to_shed_root() {
        let mut w = World::new();
        w.init_resource::<InteriorReauthor>();
        let ball = w.spawn(Ball).id();
        let root = w
            .spawn((
                PropCopy {
                    uid: None,
                    model: String::new(),
                    handle: Handle::default(),
                },
                Assembled,
                Transform::IDENTITY,
                benilla_world::terrain_stream::PropLight::Exterior,
            ))
            .id();
        let part = w.spawn(ClassifiedBy(root)).id();
        w.entity_mut(root).add_child(part).insert(Attached {
            radial: 1.,
            volume: 1.,
            size: 1.,
            at: 0.,
        });
        w.run_system_once(lighting_anchors).unwrap();
        assert_eq!(w.get::<ClassifiedBy>(part).unwrap().0, ball);
        assert!(w
            .get::<benilla_world::terrain_stream::PropLight>(root)
            .is_none());
        assert_eq!(w.resource::<InteriorReauthor>().0, vec![part]);
        w.entity_mut(root).remove::<Attached>();
        w.entity_mut(root)
            .get_mut::<Transform>()
            .unwrap()
            .translation
            .x = 2.;
        w.run_system_once(lighting_anchors).unwrap();
        assert_eq!(w.get::<ClassifiedBy>(part).unwrap().0, root);
    }
}
