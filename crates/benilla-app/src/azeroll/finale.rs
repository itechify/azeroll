//! D26: terminal render-only Cathedral pickup. Physics and streaming stay at the safe approach.
use super::*;
use benilla_assets::{coords::wow_to_bevy, WmoModel};
use benilla_world::{
    lighting::ModeFogOverride,
    model_forms::FormSlices,
    model_render::{ModelKind, ShadeSel},
    terrain_stream::{spawn_model_entities, WmoPartitions, WmoParts, WmoPlacements},
    view::WorldCamera,
    wmo_portal::{WmoExteriorView, WmoPortalInstance, WmoRenderMask},
};
use std::sync::Arc;

// Manifest v2: hide the Cathedral rooms/crypts, but fly only facade109.
// Interior-only geometry lacks a meaningful exterior silhouette and produces black BSP boxes.
// Shared square/neighbor group112 stays live (see audit notes).
fn selected(g: u16) -> bool {
    matches!(g, 95 | 109 | 134..=151 | 173..=183)
}
#[derive(Resource, Default)]
pub(super) struct Finale {
    pub triggered: bool,
    root: Option<Entity>,
    model: Option<Handle<WmoModel>>,
    instance: Option<Entity>,
    city_group: Option<u16>,
    origin: Vec3,
    rotation: Quat,
    scale: f32,
    axis: f32,
    shot: usize,
    attached: bool,
    /// Native source-isolation still: hold the same authored Cathedral lens for each A/B.
    probe_frames: u32,
    probe_back: Vec3,
    probe_origin: Vec3,
}
impl Finale {
    pub(super) fn reveal_point(&self) -> Option<Vec3> {
        self.triggered.then_some(self.origin)
    }
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Finale>()
        .init_resource::<WmoRenderMask>()
        .init_resource::<WmoExteriorView>()
        .init_resource::<ModeFogOverride>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(OnEnter(Phase::Select), end)
        .add_systems(OnEnter(Phase::Results), end)
        .add_systems(
            Update,
            prepare.after(benilla_world::schedule::WorldStage::Stream),
        )
        .add_systems(
            Update,
            animate
                .after(camera::place_camera)
                .before(benilla_world::view::CameraPoseSet)
                .before(benilla_world::wmo_portal::WmoPvsSet),
        );
}
/// Freeze only the staged terminal shot; Results and Free Roll retain their normal controls.
pub(super) fn mobile(
    state: Res<Finale>,
    phase: Res<State<Phase>>,
    flow: Res<progress::Flow>,
) -> bool {
    !state.triggered || *phase.get() != Phase::Playing || flow.free_roll
}
fn end(
    state: Res<Finale>,
    phase: Res<State<Phase>>,
    mut flow: ResMut<progress::Flow>,
    mut mask: ResMut<WmoRenderMask>,
    mut exterior: ResMut<WmoExteriorView>,
    mut time: ResMut<Time<Virtual>>,
) {
    if state.triggered {
        flow.finale = None;
        time.set_relative_speed(1.0);
    }
    if *phase.get() != Phase::Results || !state.attached || flow.free_roll {
        mask.0.clear();
    }
    exterior.0 = None;
}
fn reset(
    mut state: ResMut<Finale>,
    mut mask: ResMut<WmoRenderMask>,
    mut exterior: ResMut<WmoExteriorView>,
) {
    *state = Finale::default();
    mask.0.clear();
    exterior.0 = None;
}
#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    design: Res<design::Design>,
    phase: Res<State<Phase>>,
    mut ball: ResMut<BallState>,
    placements: Res<WmoPlacements>,
    parts: Res<WmoParts>,
    partitions: Res<WmoPartitions>,
    models: Res<Assets<WmoModel>>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: benilla_world::model_render::M2BatchMaterials,
    mut uv: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut tint: ResMut<benilla_world::doodad_anim::TintAnimMaterials>,
    mut table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
    mut state: ResMut<Finale>,
    flow: Res<progress::Flow>,
) {
    if design.goal.kind != design::GoalKind::Finale
        || design.goal.target != "Cathedral of Light"
        || *phase.get() != Phase::Playing
        || flow.free_roll
    {
        return;
    }
    if state.root.is_none() {
        let Some(p) = placements
            .placements
            .iter()
            .find(|p| p.map == 0 && p.uid == 10047)
        else {
            return;
        };
        let Some(source) = models.get(&p.model) else {
            return;
        };
        // The immutable source guards the authored extraction; the derived city's forms are
        // already resident, so the shot never uploads every closed interior room.
        if source.group_nav.len() != 306 || !(134..=151).all(|g| source.submesh_group.contains(&g))
        {
            return;
        }
        let city = parts
            .0
            .iter()
            .find(|part| part.map == 0 && part.uid == 10047 && part.part == 100);
        let handle = city.map_or(&p.model, |part| &part.model);
        let Some(m) = models.get(handle) else { return };
        if !forms.require_static(handle, 0) {
            return;
        }
        let Some((cache, materials, light)) = mats.pieces() else {
            return;
        };
        let indices: Vec<_> = m
            .submesh_group
            .iter()
            .enumerate()
            .filter(|(i, g)| **g == city.map_or(109, |p| p.group) && !m.submeshes[*i].interior)
            .map(|(i, _)| i)
            .collect();
        let subs: Vec<_> = indices.iter().map(|&i| m.submeshes[i].clone()).collect();
        let (lo, hi) = subs.iter().flat_map(|s| &s.geometry.positions).fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), &v| {
                let v = wow_to_bevy(v);
                (lo.min(v), hi.max(v))
            },
        );
        if !(hi - lo).is_finite() {
            return;
        }
        let center = (lo + hi) * 0.5;
        let slices = forms.slices(handle);
        let stat: Vec<_> = indices.iter().map(|&i| slices.stat[i].clone()).collect();
        let object = Arc::new(benilla_world::interact::WorldObject {
            kind: ModelKind::Wmo,
            label: "Cathedral of Light".into(),
            id: 10047,
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
            (hi - lo).length() * 0.5,
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
            return;
        }
        state.origin = p.transform.transform_point(center);
        state.rotation = p.transform.rotation;
        // Fixed source-comparison lens, independent of the stock/partitioned trophy bounds.
        state.probe_origin = p
            .transform
            .transform_point(Vec3::new(-8.0367775, 84.626945, 495.95532));
        state.probe_back =
            units::flat(design.goal_point().unwrap_or(ball.pos) - state.probe_origin)
                .normalize_or(Vec3::Z);
        state.axis = (hi - lo).max_element();
        state.scale = (ball.radius * 1.6 / (hi - lo).max_element()).clamp(0.05, 0.4);
        state.model = Some(handle.clone());
        state.instance = p.instance;
        state.city_group = parts
            .0
            .iter()
            .find(|part| part.uid == 10047 && part.part == 100)
            .map(|p| p.group);
        state.root = Some(
            commands
                .spawn((
                    RunEntity,
                    Transform::from_translation(state.origin).with_rotation(state.rotation),
                    Visibility::Hidden,
                ))
                .add_children(&spawned.entities)
                .id(),
        );
        info!(
            "azeroll Cathedral prepared: groups={} bounds={:?}..{:?}",
            indices.len(),
            lo,
            hi
        );
    }
    if !state.triggered
        && partitions.removed_count(0, 10047) >= design.goal.city_pieces as usize
        && ball.diameter() >= design.goal.diameter
        && design.goal_area().is_some_and(|a| {
            let p = units::wow_to_bevy(a.centre);
            Vec2::new(p.x - ball.pos.x, p.z - ball.pos.z).length() <= a.radius
        })
    {
        state.scale = (ball.radius * 1.6 / state.axis).clamp(0.05, 0.4);
        ball.vel = Vec3::ZERO;
        ball.dash = 0.0;
        state.triggered = true;
        info!(
            "azeroll Cathedral finale triggered at D={:.2}",
            ball.diameter()
        );
    }
}
// A partitioned city has already separated the Cathedral from Square streets/support.
// Mask only that piece; the original mixed group112 must remain visible under the shot.
fn hidden_groups(city_group: Option<u16>) -> Vec<u16> {
    city_group.map_or_else(|| (0..306).filter(|&g| selected(g)).collect(), |g| vec![g])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn animate(
    mut commands: Commands,
    mut state: ResMut<Finale>,
    phase: Res<State<Phase>>,
    mut flow: ResMut<progress::Flow>,
    mut run: ResMut<Run>,
    ball: Res<BallState>,
    (mut mask, mut exterior): (ResMut<WmoRenderMask>, ResMut<WmoExteriorView>),
    instances: Query<(Entity, &WmoPortalInstance)>,
    mut roots: Query<(&mut Transform, &mut Visibility), Without<WorldCamera>>,
    mut cameras: Query<&mut Transform, With<WorldCamera>>,
    spin: Query<Entity, With<BallSpin>>,
    mut fog: ResMut<ModeFogOverride>,
    mut rig: ResMut<camera::CamRig>,
    view: Res<benilla_world::view::ViewDistance>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<Phase>>,
) {
    // Includes Results -> Free Roll, level selection, pause and all interruption paths.
    let active = *phase.get() == Phase::Playing && !flow.free_roll && flow.finale.is_some();
    if !state.triggered || !active {
        if flow.free_roll {
            run.final_diameter = None;
        }
        // Results still display the attached Cathedral: do not also resurrect it in
        // the city behind the clump. Ready, selection and Free Roll restore the source.
        if *phase.get() != Phase::Results || !state.attached || flow.free_roll {
            mask.0.clear();
        }
        exterior.0 = None;
        if let Some(root) = state.root {
            if let Ok((_, mut vis)) = roots.get_mut(root) {
                *vis = if *phase.get() == Phase::Results && state.attached {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        fog.0 = if ball.radius >= 8.0 {
            Some((
                (rig.eye.distance(ball.pos) + ball.radius * 2.0).min(view.farclip * 0.72),
                view.farclip,
            ))
        } else {
            None
        };
        return;
    }
    for (e, inst) in &instances {
        if Some(e) == state.instance || Some(&inst.handle) == state.model.as_ref() {
            exterior.0 = Some(e);
            mask.0.insert(e, hidden_groups(state.city_group));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let visual_probe = crate::webenv::var("WOW_AZEROLL_VISUAL_AB");
    #[cfg(target_arch = "wasm32")]
    let visual_probe: Option<String> = None;
    let probing = visual_probe.is_some() && state.probe_frames < 180;
    if probing {
        state.probe_frames += 1;
        flow.finale = Some(0.0);
    }
    let skip = keys.just_pressed(KeyCode::Enter);
    let t = if probing {
        0.0
    } else if skip {
        2.6
    } else {
        flow.finale.unwrap()
    };
    let flight = ((t - 0.3) / 2.0).clamp(0.0, 1.0);
    let k = flight * flight * (3.0 - 2.0 * flight);
    let end = ball.pos + Vec3::Y * (ball.radius * 1.25);
    let at = state.origin.lerp(end, k) + Vec3::Y * (std::f32::consts::PI * flight).sin() * 100.0;
    let scale = 1.0 + (state.scale - 1.0) * k;
    if !state.attached {
        if let Some(root) = state.root {
            if let Ok((mut tf, mut vis)) = roots.get_mut(root) {
                *vis = Visibility::Inherited;
                *tf = Transform::from_translation(at)
                    .with_rotation(state.rotation)
                    .with_scale(Vec3::splat(scale));
            }
        }
    }
    // Frame the assembly, then pull back to the ball. Physical radius remains unchanged.
    let look = if probing {
        state.probe_origin
    } else {
        at.lerp(ball.pos, 0.25 * k)
    };
    // Establish the Cathedral against Stormwind, looking inward from its rear quarter.
    // A front-facing aerial lens looks beyond the north wall at stock, unfinished ADT
    // base terrain. Keep all world geometry intact and show the city being reclaimed.
    let back = if probing && crate::webenv::var("WOW_AZEROLL_CITY_LENS").is_none() {
        state.probe_back
    } else {
        state.rotation * Vec3::new(-0.893, 0.0, 0.45).normalize()
    };
    let direction = (back + Vec3::Y * 0.65).normalize();
    let distance = (350.0 * (1.0 - k) + (ball.radius * 6.0) * k).min(500.0);
    let eye = look + direction * distance + Vec3::Y * 35.0;
    for mut cam in &mut cameras {
        *cam = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    }
    // The lifting pivot is high above the Square. Reserve the ground behind it too:
    // a near fog ramp turns the intact deck into a sky-coloured plane under the trees.
    fog.0 = Some((
        (distance + 200.0 + 2.0 * ball.radius).min(view.farclip * 0.93),
        view.farclip,
    ));
    if flight >= 1.0 {
        run.final_diameter = Some(595.0);
        if !state.attached {
            if let (Some(root), Ok(spin)) = (state.root, spin.single()) {
                let rotation = ball.spin.inverse();
                commands.entity(root).insert((
                    ChildOf(spin),
                    Transform {
                        translation: rotation * (end - ball.pos),
                        rotation: rotation * state.rotation,
                        scale: Vec3::splat(state.scale),
                    },
                ));
                state.attached = true;
                run.count += 1;
                *run.log.entry("Cathedral of Light".into()).or_default() += 1;
                run.biggest = Some(("Cathedral of Light".into(), 297.35));
                info!(
                    "azeroll Cathedral attached: final D=595 physical D={:.2}",
                    ball.diameter()
                );
            }
        }
    }
    if probing && state.probe_frames == 150 {
        info!("city source capture eye={eye:?} look={look:?}");
        for (entity, inst) in &instances {
            if Some(entity) == state.instance {
                info!(
                    "city source capture PVS {}/{} groups: {:?}",
                    inst.visible.iter().filter(|v| **v).count(),
                    inst.visible.len(),
                    inst.visible
                        .iter()
                        .enumerate()
                        .filter_map(|(g, v)| v.then_some(g))
                        .collect::<Vec<_>>()
                );
            }
        }
        if let Some(path) = visual_probe {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(path));
        }
    }
    if let Some(dir) = crate::webenv::var("WOW_AZEROLL_FINALE_SHOTS") {
        let due = [0.1, 0.8, 1.5, 2.6];
        if state.shot < due.len() && t >= due[state.shot] {
            let path = std::path::Path::new(&dir).join(format!("cathedral-{}.png", state.shot));
            state.shot += 1;
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(path));
        }
    }
    if skip {
        flow.finale = None;
        next.set(Phase::Results);
    }
    rig.look = look;
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_city_finale_keeps_permanent_square_geometry_visible() {
        assert_eq!(super::hidden_groups(Some(900)), vec![900]);
        assert!(!super::hidden_groups(None).contains(&112));
        assert!(super::hidden_groups(None).contains(&109));
    }
    use super::*;
    #[test]
    fn finale_requires_a_trigger_and_cannot_win_as_a_size_goal() {
        let text = include_str!("stormwind.toml");
        let d = design::parse(text).unwrap();
        assert_eq!(d.goal.kind, design::GoalKind::Finale);
        assert!(!level::goal_met(&d.goal, true, 1000.0));
        let mut missing: toml::Value = toml::from_str(text).unwrap();
        missing["goal"]["area"] = toml::Value::String("Missing trigger area".into());
        assert!(design::parse(&toml::to_string(&missing).unwrap()).is_err());
        assert!(!selected(112));
    }
    #[test]
    fn results_keep_the_cathedral_retired_until_free_roll() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        world.insert_resource(Finale {
            triggered: true,
            attached: true,
            ..default()
        });
        world.insert_resource(State::new(Phase::Results));
        world.insert_resource(progress::Flow::default());
        world.insert_resource(Time::<Virtual>::default());
        world.insert_resource(WmoRenderMask([(entity, vec![342])].into()));
        world.insert_resource(WmoExteriorView(Some(entity)));
        world.run_system_once(end).unwrap();
        assert_eq!(
            world.resource::<WmoRenderMask>().0.get(&entity),
            Some(&vec![342])
        );
        world.resource_mut::<progress::Flow>().free_roll = true;
        world.run_system_once(end).unwrap();
        assert!(world.resource::<WmoRenderMask>().0.is_empty());
    }
    #[test]
    fn ready_clears_pending_terminal_state_and_render_mask() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        world.insert_resource(Finale {
            triggered: true,
            root: Some(entity),
            ..default()
        });
        let mut mask = WmoRenderMask::default();
        mask.0.insert(entity, vec![109]);
        world.insert_resource(mask);
        world.insert_resource(WmoExteriorView(Some(entity)));
        world.run_system_once(reset).unwrap();
        assert!(!world.resource::<Finale>().triggered);
        assert!(world.resource::<Finale>().root.is_none());
        assert!(world.resource::<WmoRenderMask>().0.is_empty());
        assert!(world.resource::<WmoExteriorView>().0.is_none());
        world.resource_mut::<Finale>().triggered = true;
        world.resource_mut::<WmoExteriorView>().0 = Some(entity);
        let mut flow = progress::Flow::default();
        flow.finale = Some(0.8);
        world.insert_resource(flow);
        let mut time = Time::<Virtual>::default();
        time.set_relative_speed(0.2);
        world.insert_resource(time);
        world.insert_resource(State::new(Phase::Select));
        world.run_system_once(end).unwrap();
        assert!(world.resource::<progress::Flow>().finale.is_none());
        assert!(world.resource::<WmoExteriorView>().0.is_none());
        assert_eq!(world.resource::<Time<Virtual>>().relative_speed(), 1.0);
    }
}
