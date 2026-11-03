//! D25: live WMO targets stay in the engine until a prepared, render-only trophy can replace them.
use super::*;
use benilla_assets::{coords::wow_to_bevy, WmoModel};
use benilla_world::{
    model_forms::FormSlices,
    model_render::{ModelKind, ShadeSel},
    schedule::WorldStage,
    terrain_stream::{spawn_model_entities, RestoreWmos, RetireWmo, WmoPlacements, WmoRetired},
    world_point::WorldPoint,
};
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Resource, Default)]
struct Pending(HashMap<(u32, u32), Prepared>);

struct Prepared {
    root: Entity,
    volume: f32,
    axis: f32,
    at: f32,
    rotation: Quat,
    floor: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(WmoPlacements {
        enabled: true,
        placements: Vec::new(),
    })
    .init_resource::<Pending>()
    .add_systems(OnEnter(Phase::Ready), reset)
    .add_systems(
        Update,
        (acknowledge, pickup)
            .chain()
            .after(WorldStage::Stream)
            .run_if(playing)
            .run_if(super::finale::mobile),
    );
}
fn reset(mut pending: ResMut<Pending>, mut restore: MessageWriter<RestoreWmos>) {
    pending.0.clear();
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

#[allow(clippy::too_many_arguments)]
fn pickup(
    mut commands: Commands,
    design: Res<design::Design>,
    mut ball: ResMut<BallState>,
    placements: Res<WmoPlacements>,
    models: Res<Assets<WmoModel>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: benilla_world::model_render::M2BatchMaterials,
    mut uv: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut tint: ResMut<benilla_world::doodad_anim::TintAnimMaterials>,
    mut table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
    mut pending: ResMut<Pending>,
    mut requests: MessageWriter<RetireWmo>,
    world: WorldPoint,
    run: Res<Run>,
    mut bounds: Local<HashMap<bevy::asset::AssetId<WmoModel>, (Vec3, Vec3)>>,
) {
    let Some((cache, materials, light)) = mats.pieces() else {
        return;
    };
    for p in &placements.placements {
        if !design.buildings.contains(&p.uid) || pending.0.contains_key(&(p.map, p.uid)) {
            continue;
        }
        let Some(m) = models.get(&p.model) else {
            continue;
        };
        let (lo, hi) = *bounds.entry(p.model.id()).or_insert_with(|| {
            m.submeshes.iter().flat_map(|s| &s.geometry.positions).fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(lo, hi), &v| {
                    let v = wow_to_bevy(v);
                    (lo.min(v), hi.max(v))
                },
            )
        });
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
        let Some(floor) = world.terrain_height_under(ball.pos) else {
            continue;
        };
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
            .filter(|(_, g)| {
                m.group_nav
                    .get(**g as usize)
                    .is_some_and(|nav| nav.flags & 8 != 0)
            })
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
            label: "Building trophy".into(),
            id: p.uid,
            detail: String::new(),
        });
        let spawned = spawn_model_entities(
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
        );
        if spawned.entities.is_empty() {
            continue;
        }
        let root = commands
            .spawn((RunEntity, Transform::IDENTITY, Visibility::Hidden))
            .add_children(&spawned.entities)
            .id();
        let volume =
            axes.x * axes.y * axes.z * level::FILL_FACTOR * 3.0 / (4.0 * std::f32::consts::PI);
        pending.0.insert(
            (p.map, p.uid),
            Prepared {
                root,
                volume,
                axis,
                at: run.elapsed,
                rotation: p.transform.rotation,
                floor,
            },
        );
        ball.vel = Vec3::ZERO;
        requests.write(RetireWmo {
            map: p.map,
            uid: p.uid,
        });
        info!(
            "azeroll building prepared: uid={} axis={axis:.2} D={:.2}",
            p.uid,
            ball.diameter()
        );
        break;
    }
}
fn acknowledge(
    mut commands: Commands,
    mut replies: MessageReader<WmoRetired>,
    mut pending: ResMut<Pending>,
    mut ball: ResMut<BallState>,
    spin: Query<Entity, With<BallSpin>>,
    mut run: ResMut<Run>,
    mut collected: MessageWriter<Collected>,
    mut next: ResMut<NextState<Phase>>,
    mut reset_probed: Local<bool>,
) {
    for reply in replies.read() {
        let Some(Prepared {
            root,
            volume,
            axis,
            at,
            rotation: world_rotation,
            floor,
        }) = pending.0.remove(&(reply.map, reply.uid))
        else {
            continue;
        };
        let Ok(spin) = spin.single() else {
            commands.entity(root).despawn();
            continue;
        };
        if !reply.accepted {
            commands.entity(root).despawn();
            continue;
        }
        ball.volume += volume;
        ball.radius = ball.volume.cbrt();
        ball.pos.y = floor + ball.radius;
        ball.vel = Vec3::ZERO;
        // Reduced silhouette: the full building was the eligibility and volume target.
        let scale = 0.28;
        let radial = ball.radius + 0.25 * axis * scale;
        let rotation = ball.spin.inverse();
        commands
            .entity(root)
            .insert((
                Visibility::Inherited,
                Transform {
                    translation: rotation * Vec3::Y * radial,
                    rotation: rotation * world_rotation,
                    scale: Vec3::splat(scale),
                },
                Attached {
                    radial,
                    volume,
                    size: axis * scale * 0.5,
                    at,
                },
            ))
            .insert(ChildOf(spin));
        run.count += 1;
        collected.write(Collected {
            entity: root,
            name: "Farmhouse".into(),
            kind: Kind::Prop,
            source: Source::Spawned,
            size: axis * 0.5,
            pickup: axis,
        });
        run.banner = Some(("A WHOLE BUILDING!".into(), 3.5));
        // Native unattended lifecycle acceptance: exercise the actual Ready transition once,
        // then collect the restored placement again. Never enabled by ordinary gameplay.
        #[cfg(not(target_arch = "wasm32"))]
        if !*reset_probed && crate::webenv::var("WOW_AZEROLL_BUILDING_RESET_TEST").is_some() {
            *reset_probed = true;
            info!(
                "azeroll building probe: Ready after retirement uid={}",
                reply.uid
            );
            next.set(Phase::Ready);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (&mut next, &mut reset_probed);
        info!(
            "azeroll building retired: uid={} credit={volume:.2} r={:.2}",
            reply.uid, ball.radius
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_uses_rotated_nearest_face_not_origin() {
        let t = Transform::from_xyz(100., 0., 0.).with_rotation(Quat::from_rotation_y(1.2));
        let q = t.transform_point(Vec3::new(12., 0., 0.));
        let hit = nearest(q, &t, Vec3::splat(-10.), Vec3::splat(10.));
        assert!((hit.distance(q) - 2.).abs() < 0.0001);
    }
}
