//! The HUD's 3D widgets: the bottom-left bubble that shows the last thing rolled up as a slowly
//! turning model, and the bottom-right Prince corner. Each is a small render-to-texture: a second
//! camera on a private [`RenderLayers`] layer draws into a 256² `Image` that [`HudMaterial`] (the
//! HUD's one `UiMaterial`, `hud.wgsl`) cuts into a round glass in the UI.
//!
//! The two pieces are built the engine's own way (`crate::portrait`'s booths): the camera spawns
//! through [`booth_view_shape`] so its pipeline key matches the warmed ones, and the target is
//! [`new_target_image_sized`]'s un-encoded float image, which the shader samples as raw gamma.
//!
//! * The **item bubble** clones what it needs from the entity that was just rolled up (a prop's
//!   mesh and material parts, or a creature's [`PortraitPart`] bind-pose twins) onto a stage far
//!   above the world, normalised to a fixed size from [`Collectible::size`] and turned on a
//!   spinner. It is rebuilt only on a pickup and its camera sleeps while the bubble is hidden.
//!   A creature is only ever cloned in its bind pose (the booth that bakes a Stand frame is the
//!   portrait module's own), so it does not spin: it sways in three-quarter view and breathes,
//!   which reads as alive. Rapid pickups count into a combo ([`ItemBubble::combo`]).
//! * The **Prince** is a second gnome, built for the corner alone ([`StagePrince`]): the world
//!   Prince's display, standing at a fixed spot on a private stage far from the world, at a fixed,
//!   larger scale, seen by a fixed camera. He never translates; his stock run/stand animation is
//!   driven in place from the ball's state (rolling, how fast, dashing, a hard bump, a tier), as
//!   Katamari's corner Prince runs on the spot while the ball rolls. The world Prince, who circles
//!   the clump at his own tiny size, is not drawn here at all.
//!
//! Neither layer is on the world camera, so nothing here appears in the world view.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, PerspectiveProjection, Projection, RenderTarget};
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::ui_render::ui_material::UiMaterial;
use bevy::ui_render::UiMaterialPlugin;

use benilla_assets::materials::WowModelMaterial;

use super::ball::top_speed;
use super::creatures::Prince;
use super::units::{self, UnitSpec};
use super::{
    BallState, Bumped, Collected, Collectible, Dash, Kind, Phase, Run, TierReached, TouchState,
    Touched,
};
use crate::creature_anim::{move_flags, EmoteAnim, MovementState, PlaySeq};
use crate::loading_screen::LoadingScreen;
use crate::net::NetEntity;
use crate::portrait::{booth_view_shape, new_target_image_sized, PortraitPart};

/// Render layers, past the engine's booth ladder (`portrait::UI_MODEL_CAM_LAYER_BASE` + 8 = 34).
const ITEM_LAYER: usize = 40;
const PRINCE_LAYER: usize = 41;
/// The render targets' edge, texels.
const TARGET: u32 = 256;
/// Where the item stage sits: far above the world, so no world camera or streamer meets it.
const STAGE: Vec3 = Vec3::new(0.0, 3000.0, 0.0);
/// Where the corner Prince stands: far from the world and from the item stage, feet on the origin
/// of his own little floor.
const PRINCE_STAGE: Vec3 = Vec3::new(400.0, 3000.0, 0.0);
/// His fixed model scale and facing: a gnome is about 1 yd tall at scale 1, and he is turned a
/// little toward the playfield (screen left), as Katamari's corner Prince is.
const PRINCE_SCALE: f32 = 1.0;
const PRINCE_YAW: f32 = -0.45;
/// The portrait camera: this far from him, this high, looking at this height of him; a little
/// below his middle looking up gives the low three-quarter view. Field of view as [`FOV`].
const PRINCE_DIST: f32 = 2.45;
const PRINCE_EYE: f32 = 0.32;
const PRINCE_LOOK: f32 = 0.5;
/// His cadence while the ball rolls, as the speed the run clip is driven at (the clip runs at that
/// speed over its move speed): the slowest trot, the fastest sprint, and the dash's extra.
const STRIDE: (f32, f32) = (5.0, 13.0);
const STRIDE_DASH: f32 = 1.3;
/// The ball is rolling when its ground speed is above this, yd/s.
const ROLLING: f32 = 0.25;
/// `AnimationData` ids played over his gait: StandWound (a hard bump) and EmoteCheer (a tier).
const ANIM_WINCE: u16 = 8;
const ANIM_CHEER: u16 = 68;
/// The radius the item model is scaled to, and the camera's vertical field of view.
const FIT: f32 = 0.82;
const FOV: f32 = 0.5;
/// The item turns this fast, radians per second.
const SPIN: f32 = 1.1;
/// Seconds a pickup stays in the bubble before it fades, and the fade's length.
const HOLD: f32 = 3.2;
const FADE: f32 = 0.8;
/// The name under the bubble shows this long after a swap, then fades over `NAME_FADE`.
const NAME_HOLD: f32 = 1.5;
const NAME_FADE: f32 = 0.6;
/// Pickups closer together than this chain into one combo, and its counter fades `COMBO_HOLD`
/// seconds after the last link.
const COMBO_GAP: f32 = 1.4;
const COMBO_HOLD: f32 = 1.2;
/// A bump or wall touch this hard (closing yd/s) makes the Prince wince.
const WINCE_BUMP: f32 = 2.0;
const WINCE_TOUCH: f32 = 3.5;
/// The Prince wince is not retriggered within this many seconds.
const WINCE_GAP: f32 = 0.45;
/// A new pickup replaces the bubble at most this often, seconds.
const SWAP_EVERY: f32 = 0.14;

/// The HUD's one `UiMaterial`: three uniforms' worth of widget state and the image the bubble shows
/// (a 1×1 stand-in for the dial and the clock). See `hud.wgsl` for what each field means.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct HudMaterial {
    /// kind, seconds, alpha, pulse.
    #[uniform(0)]
    pub kt: Vec4,
    #[uniform(1)]
    pub p: Vec4,
    #[texture(2)]
    #[sampler(3)]
    pub tex: Handle<Image>,
}

impl UiMaterial for HudMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/azeroll/hud.wgsl".into()
    }
}

/// The widgets' images and the cameras that draw them.
#[derive(Resource)]
pub(super) struct HudModels {
    pub item: Handle<Image>,
    pub prince: Handle<Image>,
    /// A 1×1 texture for the materials that sample nothing.
    pub blank: Handle<Image>,
    item_cam: Entity,
    prince_cam: Entity,
    /// The item stage's turning root.
    spinner: Entity,
}

/// What the bubble shows now, for the HUD to label and fade.
#[derive(Resource, Default)]
pub(super) struct ItemBubble {
    pub name: String,
    pub kind: Option<Kind>,
    /// Visual bounding-sphere radius, yards.
    pub size: f32,
    /// Full maximum-axis pickup gate, yards.
    pub pickup: f32,
    /// 0..1: how visible the bubble is (it fades when idle).
    pub alpha: f32,
    /// 0..1: how visible the name under it is (it fades sooner than the bubble).
    pub name_alpha: f32,
    /// Pickups in the current rapid chain, and 0..1 how visible its counter is.
    pub combo: u32,
    pub combo_alpha: f32,
    /// 1 on each link of the chain, falling to 0: the counter's pop.
    pub combo_pop: f32,
    /// 0..1: the Prince is dashing (eased), so his corner leans in.
    pub prince_effort: f32,
    /// 0..1 and its colour: a flash on his bubble's rim (dash, dash ready, a wince).
    pub prince_flash: f32,
    pub prince_flash_rgb: [f32; 3],
    /// 0..1: a wince's shake, falling off.
    pub prince_shake: f32,
    /// 1 just after a swap, falling to 0: the pop.
    pub pop: f32,
    /// A model is on the stage.
    pub has_model: bool,
    /// The stage Prince has parts on his layer.
    pub prince_ready: bool,
    /// 0..1: his bubble fades in after the camera wakes, so its first (empty) frames stay hidden.
    pub prince_alpha: f32,
    since: f32,
    since_swap: f32,
    since_link: f32,
    since_wince: f32,
    /// The run clip's current speed, smoothed: 0 when the ball is still.
    stride: f32,
    pivot: Option<Entity>,
    yaw: f32,
    /// The stage's own clock, seconds, for the sway and the breathing.
    idle: f32,
}

#[derive(Component)]
struct HudCam;

pub(super) fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "hud.wgsl");
    app.add_plugins(UiMaterialPlugin::<HudMaterial>::default())
        .init_resource::<ItemBubble>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(
            Update,
            (
                item_swap,
                stage_prince,
                prince_reactions,
                prince_motion,
                animate,
            )
                .chain()
                .after(benilla_world::schedule::WorldStage::Input),
        );
}

/// The images, the two cameras and the item stage. Runs before the HUD builds, which reads
/// [`HudModels`].
pub(super) fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let item = images.add(new_target_image_sized(TARGET, TARGET));
    let prince = images.add(new_target_image_sized(TARGET, TARGET));
    let blank = images.add(Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));
    let cam = |commands: &mut Commands, target: &Handle<Image>, layer: usize, order: isize| {
        commands
            .spawn((
                HudCam,
                booth_view_shape(),
                Camera {
                    order,
                    is_active: false,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Projection::from(PerspectiveProjection {
                    fov: FOV,
                    near: 0.02,
                    far: 60.0,
                    ..default()
                }),
                Transform::default(),
                RenderLayers::layer(layer),
            ))
            .id()
    };
    let item_cam = cam(&mut commands, &item, ITEM_LAYER, -90);
    let prince_cam = cam(&mut commands, &prince, PRINCE_LAYER, -89);
    // Looking at the stage's origin from in front (the models face -Z) and a little above.
    let dist = FIT / (FOV * 0.5).sin() * 1.04;
    commands.entity(item_cam).insert(
        Transform::from_translation(STAGE + Vec3::new(0.0, 0.5, -dist)).looking_at(STAGE, Vec3::Y),
    );
    // The Prince's camera never moves: in front of him (he faces -Z), a little low.
    commands.entity(prince_cam).insert(
        Transform::from_translation(PRINCE_STAGE + Vec3::new(0.0, PRINCE_EYE, -PRINCE_DIST))
            .looking_at(PRINCE_STAGE + Vec3::Y * PRINCE_LOOK, Vec3::Y),
    );
    let spinner = commands
        .spawn((Transform::from_translation(STAGE), Visibility::default()))
        .id();
    commands.insert_resource(HudModels {
        item,
        prince,
        blank,
        item_cam,
        prince_cam,
        spinner,
    });
}

/// A new run starts with an empty bubble.
fn reset(mut commands: Commands, mut bubble: ResMut<ItemBubble>) {
    if let Some(old) = bubble.pivot.take() {
        commands.entity(old).despawn();
    }
    *bubble = ItemBubble::default();
}

/// Clone the thing just rolled up onto the stage. Of a frame's pickups the biggest is shown.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn item_swap(
    mut commands: Commands,
    mut collected: MessageReader<Collected>,
    time: Res<Time<Real>>,
    models: Option<Res<HudModels>>,
    mut bubble: ResMut<ItemBubble>,
    things: Query<(&Collectible, &Transform, &GlobalTransform)>,
    children: Query<&Children>,
    portrait: Query<(&GlobalTransform, &PortraitPart)>,
    plain: Query<
        (
            &GlobalTransform,
            &Mesh3d,
            &MeshMaterial3d<WowModelMaterial>,
            Option<&MeshTag>,
        ),
        (
            Without<PortraitPart>,
            Without<bevy::mesh::skinning::SkinnedMesh>,
        ),
    >,
) {
    bubble.since_swap += time.delta_secs();
    bubble.since_link += time.delta_secs();
    let Some(models) = models else { return };
    let batch: Vec<&Collected> = collected.read().collect();
    let Some(best) = batch
        .iter()
        .max_by(|a, b| a.size.total_cmp(&b.size))
        .map(|c| (*c).clone())
    else {
        return;
    };
    // Every pickup links the chain, whether or not the bubble can swap this frame.
    if bubble.since_link <= COMBO_GAP && bubble.combo > 0 {
        bubble.combo += batch.len() as u32;
    } else {
        bubble.combo = batch.len() as u32;
    }
    bubble.since_link = 0.0;
    bubble.combo_pop = 1.0;
    if bubble.since_swap < SWAP_EVERY {
        return;
    }
    let Ok((thing, tf, gt)) = things.get(best.entity) else {
        return;
    };
    let layers = RenderLayers::layer(ITEM_LAYER);
    // Each part's transform relative to the root, in the root's own (unscaled) frame.
    let to_local = gt.affine().inverse();
    let mut parts: Vec<(Handle<Mesh>, Handle<WowModelMaterial>, u32, Mat4)> = Vec::new();
    for d in children.iter_descendants(best.entity) {
        if let Ok((pgt, part)) = portrait.get(d) {
            let rel = Mat4::from(to_local * pgt.affine());
            parts.push((
                part.static_mesh.clone(),
                part.material.clone(),
                benilla_world::mesh_tag::spawn_tag(0, 1.0),
                rel,
            ));
        } else if let Ok((pgt, mesh, mat, tag)) = plain.get(d) {
            let rel = Mat4::from(to_local * pgt.affine());
            let tag = tag.map_or(benilla_world::mesh_tag::spawn_tag(0, 1.0), |t| {
                benilla_world::mesh_tag::with_alpha(t.0, 1.0)
            });
            parts.push((mesh.0.clone(), mat.0.clone(), tag, rel));
        }
    }
    if parts.is_empty() {
        return;
    }
    if let Some(old) = bubble.pivot.take() {
        commands.entity(old).despawn();
    }
    // Scale to the fit radius about the thing's bounding sphere centre.
    let k = FIT / thing.size.max(1.0e-3);
    let scale = tf.scale * k;
    let pivot = commands
        .spawn((
            Transform {
                translation: -(thing.center * scale),
                rotation: Quat::IDENTITY,
                scale,
            },
            Visibility::default(),
            ChildOf(models.spinner),
        ))
        .id();
    for (mesh, mat, tag, rel) in parts {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            MeshTag(tag),
            Transform::from_matrix(rel),
            Visibility::default(),
            layers.clone(),
            ChildOf(pivot),
        ));
    }
    bubble.pivot = Some(pivot);
    bubble.has_model = true;
    bubble.name = best.name.to_string();
    bubble.kind = Some(best.kind);
    bubble.size = best.size;
    bubble.pickup = best.pickup;
    bubble.since = 0.0;
    bubble.since_swap = 0.0;
    bubble.pop = 1.0;
}

/// The corner Prince's marker: the portrait's own gnome, never the world's.
#[derive(Component)]
struct StagePrince;

/// Keep one gnome standing on the Prince stage, and put his mesh parts on the Prince layer alone as
/// the attach brings them in.
///
/// He is built the way the world Prince is ([`units::spawn_unit`]: a guid, a `NetEntity` and an
/// `ObjectStore` are all the stock attach and animation need), from the world Prince's display, but
/// kept out of everything that moves, grounds, culls or collects a unit:
///
/// * [`Carried`](crate::entities::Carried): the net layer's facing, motion, spline, ground-seating
///   and scale-easing systems all skip a unit whose transform is not theirs;
/// * no `WorldUnit`: its only reader that would matter is the exterior cull, which elects each body
///   by the world camera's frustum and would hide a unit 3000 yd up every frame;
/// * no [`RunEntity`](super::RunEntity): he outlives a restart;
/// * no `Collectible`, `Prince` or `Push`: no azeroll system (AI, bumps, pickup, the world Prince's
///   stagecraft) queries him.
///
/// His parts are on layer 41 only, which the world camera does not draw.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn stage_prince(
    mut commands: Commands,
    mut bubble: ResMut<ItemBubble>,
    mut run: ResMut<Run>,
    mut display: Local<Option<u32>>,
    world_prince: Query<&NetEntity, With<Prince>>,
    stage: Query<Entity, With<StagePrince>>,
    with_unit: Query<
        (),
        (
            With<StagePrince>,
            With<benilla_world::world_unit::WorldUnit>,
        ),
    >,
    children: Query<&Children>,
    bare: Query<(), (With<Mesh3d>, Without<RenderLayers>)>,
    layered: Query<(), (With<Mesh3d>, With<RenderLayers>)>,
) {
    if display.is_none() {
        *display = world_prince.iter().find_map(|n| n.display_id);
    }
    let Ok(root) = stage.single() else {
        bubble.prince_ready = false;
        let Some(display) = *display else { return };
        let entity = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: run.guid(),
                display,
                scale: PRINCE_SCALE,
                level: 1,
                faction: units::FACTION_FRIENDLY,
                bytes0: 7 | (1 << 8),
            },
            PRINCE_STAGE,
            PRINCE_YAW,
        );
        commands
            .entity(entity)
            .remove::<super::RunEntity>()
            .insert((
                StagePrince,
                crate::entities::Carried,
                Visibility::Visible,
                Transform::from_translation(PRINCE_STAGE)
                    .with_rotation(Quat::from_rotation_y(PRINCE_YAW))
                    .with_scale(Vec3::splat(PRINCE_SCALE)),
            ));
        return;
    };
    // `publish_world_units` restates every wire body as a `WorldUnit` whenever its model resolves;
    // this one is not the world's to elect.
    if with_unit.contains(root) {
        commands
            .entity(root)
            .remove::<benilla_world::world_unit::WorldUnit>();
    }
    let only = RenderLayers::layer(PRINCE_LAYER);
    let mut any = false;
    for d in children.iter_descendants(root) {
        if bare.contains(d) {
            commands.entity(d).insert(only.clone());
            any = true;
        } else if layered.contains(d) {
            any = true;
        }
    }
    if bubble.prince_ready != any {
        bubble.prince_ready = any;
    }
}

/// The Prince's corner reacts: a dash leans him in with a gold flash, the dash coming ready is a
/// pale blue one, a hard bump or wall touch makes him wince (a red flash, a shake and his
/// StandWound clip), and a tier crossing makes him cheer.
#[allow(clippy::too_many_arguments)]
fn prince_reactions(
    time: Res<Time<Real>>,
    ball: Res<BallState>,
    mut dashes: MessageReader<Dash>,
    mut bumps: MessageReader<Bumped>,
    mut touches: MessageReader<Touched>,
    mut tiers: MessageReader<TierReached>,
    mut bubble: ResMut<ItemBubble>,
    stage: Query<Entity, With<StagePrince>>,
    mut anim: MessageWriter<EmoteAnim>,
    mut seq: ResMut<PlaySeq>,
) {
    let mut play = |anim_id: u16| {
        if let Ok(entity) = stage.single() {
            anim.write(EmoteAnim {
                entity,
                anim_id,
                seq: seq.next(),
                via_player: false,
            });
        }
    };
    let dt = time.delta_secs();
    bubble.since_wince += dt;
    bubble.prince_flash = (bubble.prince_flash - dt * 2.6).max(0.0);
    bubble.prince_shake = (bubble.prince_shake - dt * 2.4).max(0.0);
    let want = if ball.dash > 0.0 { 1.0 } else { 0.0 };
    bubble.prince_effort += (want - bubble.prince_effort) * (1.0 - (-9.0 * dt).exp());
    for d in dashes.read() {
        match d {
            Dash::Started => {
                bubble.prince_flash = 0.85;
                bubble.prince_flash_rgb = [1.0, 0.82, 0.3];
            }
            Dash::Ready => {
                bubble.prince_flash = bubble.prince_flash.max(0.5);
                bubble.prince_flash_rgb = [0.5, 0.92, 1.0];
            }
            Dash::WindUp | Dash::Ended => {}
        }
    }
    let hits = bumps
        .read()
        .map(|b| (b.strength, WINCE_BUMP))
        .chain(
            touches
                .read()
                .filter(|t| t.state == TouchState::Wall)
                .map(|t| (t.strength, WINCE_TOUCH)),
        )
        .filter(|&(strength, floor)| strength >= floor)
        .map(|(strength, _)| strength)
        .fold(0.0_f32, f32::max);
    if hits > 0.0 && bubble.since_wince >= WINCE_GAP {
        bubble.since_wince = 0.0;
        bubble.prince_shake = (hits / 8.0).clamp(0.4, 1.0);
        bubble.prince_flash = 0.9;
        bubble.prince_flash_rgb = [1.0, 0.32, 0.28];
        play(ANIM_WINCE);
    }
    if tiers.read().count() > 0 {
        play(ANIM_CHEER);
    }
}

/// The corner Prince runs on the spot while the ball rolls (faster as it goes faster, a dash faster
/// still) and stands when it stops. He only ever has his [`MovementState`] written: his transform is
/// fixed, so he is in the same place in every frame.
fn prince_motion(
    time: Res<Time<Real>>,
    ball: Res<BallState>,
    mut bubble: ResMut<ItemBubble>,
    mut stage: Query<&mut MovementState, With<StagePrince>>,
) {
    let Ok(mut mv) = stage.single_mut() else {
        return;
    };
    let rolling = Vec3::new(ball.vel.x, 0.0, ball.vel.z).length();
    let wanted = if rolling > ROLLING {
        let fraction = (rolling / top_speed(ball.radius).max(0.1)).clamp(0.0, 1.0);
        let stride = STRIDE.0 + (STRIDE.1 - STRIDE.0) * fraction;
        if ball.dash > 0.0 {
            stride * STRIDE_DASH
        } else {
            stride
        }
    } else {
        0.0
    };
    let dt = time.delta_secs();
    bubble.stride = if wanted == 0.0 {
        0.0
    } else if bubble.stride == 0.0 {
        wanted
    } else {
        bubble.stride + (wanted - bubble.stride) * (1.0 - (-8.0 * dt).exp())
    };
    // Written only on a change, so the animation driver is not woken every frame.
    let flags = if bubble.stride > 0.0 {
        move_flags::FORWARD
    } else {
        0
    };
    if mv.speed != bubble.stride || mv.flags != flags {
        mv.speed = bubble.stride;
        mv.flags = flags;
    }
}

/// Spin the item, fade it when idle, fade the Prince in and put each camera to sleep when its widget
/// is not drawn.
#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    models: Option<Res<HudModels>>,
    mut bubble: ResMut<ItemBubble>,
    mut transforms: Query<&mut Transform>,
    mut cameras: Query<&mut Camera, With<HudCam>>,
) {
    let Some(models) = models else { return };
    let dt = time.delta_secs();
    let hud_on = matches!(phase.get(), Phase::Playing | Phase::Paused) && !loading.covering();

    bubble.since += dt;
    bubble.pop = (bubble.pop - dt * 3.2).max(0.0);
    bubble.combo_pop = (bubble.combo_pop - dt * 4.0).max(0.0);
    let alpha = if bubble.has_model {
        (1.0 - (bubble.since - HOLD) / FADE).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if (bubble.alpha - alpha).abs() > 1e-4 {
        bubble.alpha = alpha;
    }
    bubble.name_alpha = if bubble.has_model {
        (1.0 - (bubble.since - NAME_HOLD) / NAME_FADE).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // The counter shows from the second link on, and fades a moment after the chain ends.
    bubble.combo_alpha = if bubble.combo >= 2 {
        (1.0 - (bubble.since_link - COMBO_HOLD) / NAME_FADE).clamp(0.0, 1.0) * alpha
    } else {
        0.0
    };
    let item_on = hud_on && alpha > 0.01;
    if item_on {
        bubble.idle += dt;
        let living = matches!(bubble.kind, Some(Kind::Creature | Kind::Boss));
        if let Ok(mut tf) = transforms.get_mut(models.spinner) {
            if living {
                // Three-quarter view, swaying, breathing and bobbing: a bind-pose creature that
                // looks alive instead of a T-posed one turning round.
                let t = bubble.idle;
                tf.rotation = Quat::from_rotation_y(-0.4 + 0.5 * (t * 1.15).sin());
                let breath = 0.035 * (t * 3.3).sin() + 0.12 * bubble.pop * bubble.pop;
                tf.scale = Vec3::new(1.0 - 0.5 * breath, 1.0 + breath, 1.0 - 0.5 * breath);
                tf.translation = STAGE + Vec3::Y * (0.035 * (t * 2.1).sin());
            } else {
                bubble.yaw += dt * SPIN;
                tf.rotation = Quat::from_rotation_y(bubble.yaw);
                tf.scale = Vec3::ONE;
                tf.translation = STAGE;
            }
        }
    }

    // His camera is fixed; it only sleeps with the HUD, or until his parts exist.
    let prince_on = hud_on && bubble.prince_ready;
    let want = if prince_on { 1.0 } else { 0.0 };
    bubble.prince_alpha = if prince_on {
        (bubble.prince_alpha + dt * 3.0).min(want)
    } else {
        0.0
    };
    for (cam, on) in [(models.item_cam, item_on), (models.prince_cam, prince_on)] {
        if let Ok(mut camera) = cameras.get_mut(cam) {
            if camera.is_active != on {
                camera.is_active = on;
            }
        }
    }
}
