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
//!   larger scale, seen by a fixed camera. He never translates: he copies the world Prince's
//!   movement, pose hold and lean ([`prince_motion`]) and reacts to hits and tiers, so he runs,
//!   scrambles and strains on the spot as Katamari's corner Prince does. The world Prince, who
//!   circles the clump at his own tiny size, is not drawn here at all.
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
use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::load_creature_catalog;

use super::creatures::Prince;
use super::units::{self, UnitSpec};
use super::{
    BallState, Bumped, Collected, Collectible, Dash, Kind, Phase, Run, TierReached, TouchState,
    Touched,
};
use crate::creature_anim::{CastHold, EmoteAnim, MovementState, PlaySeq};
use crate::loading_screen::LoadingScreen;
use crate::net::NetEntity;
use crate::portrait::{booth_view_shape, new_target_image_sized, PortraitPart};

/// Render layers, past the engine's booth ladder (`portrait::UI_MODEL_CAM_LAYER_BASE` + 8 = 34).
const ITEM_LAYER: usize = 40;
const PRINCE_LAYER: usize = 41;
const COMMISSIONER_LAYER: usize = 42;
/// The render targets' edge, texels.
const TARGET: u32 = 256;
/// Where the item stage sits: far above the world, so no world camera or streamer meets it.
const STAGE: Vec3 = Vec3::new(0.0, 3000.0, 0.0);
/// Where the corner Prince stands: far from the world and from the item stage, feet on the origin
/// of his own little floor.
const PRINCE_STAGE: Vec3 = Vec3::new(400.0, 3000.0, 0.0);
/// Where the hub's commissioner stands: a third private stage, far from the other two. His render
/// target is a 3:4 portrait for the speech-bubble window (`select::orrery`), and his camera frames
/// a gnome from the knees up, a little low, so he looms.
const COMMISSIONER_STAGE: Vec3 = Vec3::new(800.0, 3000.0, 0.0);
const COMMISSIONER_TARGET: (u32, u32) = (288, 384);
const COMMISSIONER_SCALE: f32 = 1.0;
const COMMISSIONER_YAW: f32 = 0.18;
const COMMISSIONER_DIST: f32 = 2.55;
const COMMISSIONER_EYE: f32 = 0.55;
const COMMISSIONER_LOOK: f32 = 0.78;
/// `AnimationData` ids: EmoteTalk, EmoteTalkExclamation, EmoteCheer, EmoteLaugh.
const ANIM_TALK: u16 = 60;
const ANIM_EXCLAIM: u16 = 64;
/// While a line types out he gestures at this interval, seconds.
const TALK_EVERY: f32 = 1.7;
/// His model scale, the world Prince's own, and his facing, turned a little toward the playfield
/// (screen left) as Katamari's corner Prince is. The animation driver divides the run clip's rate
/// by the model scale and picks Run or Sprint from the raw speed, so only the same scale lets the
/// copied movement give the same gait at the same cadence; the camera is brought in to match.
const PRINCE_SCALE: f32 = super::creatures::PRINCE_SCALE;
const PRINCE_YAW: f32 = -0.45;
/// How much of the world Prince's lean the corner shows: he leans toward the camera, so the full
/// tilt pushes his head out of the frame.
const LEAN_SHOWN: f32 = 0.6;
/// The portrait camera: this far from him, this high, looking at this height of him, framed for a
/// scale-1 gnome (about 1 yd tall) and multiplied by [`PRINCE_SCALE`]; a little below his middle
/// looking up gives the low three-quarter view. Field of view as [`FOV`].
const PRINCE_DIST: f32 = 2.75 * PRINCE_SCALE;
const PRINCE_EYE: f32 = 0.32 * PRINCE_SCALE;
const PRINCE_LOOK: f32 = 0.5 * PRINCE_SCALE;
/// `AnimationData` ids played over his gait: StandWound (a hard bump) and EmoteCheer (a tier).
const ANIM_WINCE: u16 = 8;
const ANIM_CHEER: u16 = 68;
/// The radius the item model is scaled to, and the camera's vertical field of view.
/// Seconds a stage camera is awake before its bubble fades in: the first frames after a model
/// appears or a camera wakes are empty glass.
const PRINCE_SETTLE: f32 = 1.0;
const WHO_SETTLE: f32 = 0.3;
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
    /// The hub's commissioner, a portrait target.
    pub commissioner: Handle<Image>,
    /// A 1×1 texture for the materials that sample nothing.
    pub blank: Handle<Image>,
    item_cam: Entity,
    prince_cam: Entity,
    commissioner_cam: Entity,
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
    /// Seconds his stage has been ready and awake: his bubble waits for his textures to stream in
    /// (`PRINCE_SETTLE`) rather than show an empty glass for the first moments of a run.
    prince_for: f32,
    since: f32,
    since_swap: f32,
    since_link: f32,
    since_wince: f32,
    pivot: Option<Entity>,
    yaw: f32,
    /// The stage's own clock, seconds, for the sway and the breathing.
    idle: f32,
}

/// The hub's commissioner, a gnome inventor on his own stage, driven by `select::orrery`: whether
/// he is mid-line, a counter that bumps when something worth a cheer happens, and whether his
/// parts exist yet.
#[derive(Resource, Default)]
pub(crate) struct Commissioner {
    pub talking: bool,
    /// Fizzwick's porthole is popping in during play (or the lost-run moment): his stage runs.
    pub pop: bool,
    /// Bumped by the hub for a cheer (a level unlocked, a star lit).
    pub cheer: u32,
    /// A one-shot gesture request from the verdict screen: an `AnimationData` id.
    pub gesture: Option<u16>,
    /// His mesh parts are on his layer.
    pub ready: bool,
    /// 0..1: fades in after the camera wakes, so his first (empty) frames stay hidden.
    pub alpha: f32,
    display: Option<u32>,
    /// Seconds his camera has been awake, for the same settling.
    awake_for: f32,
    cheered: u32,
    since_gesture: f32,
    was_talking: bool,
}

/// The gnome displays a commissioner can wear: male gnome NPC appearances, lowest id first. The
/// world Prince wears the very lowest, so the commissioner is the next with a helm if there is one
/// (goggles and a tall hat suit an inventor), else the next.
fn commissioner_display(catalog: &benilla_formats::CreatureCatalog) -> Option<u32> {
    let mut gnomes: Vec<(u32, bool)> = catalog
        .display_models()
        .filter_map(|(d, _)| {
            let m = catalog.model(d)?;
            let path = m.model_path.to_ascii_lowercase().replace('/', "\\");
            (path.starts_with("character\\gnome\\male\\gnomemale") && m.scale > 0.0)
                .then(|| {
                    m.npc_appearance
                        .as_ref()
                        .map(|a| (d, a.equipment[0] != 0 && a.equipment[3] != 0))
                })
                .flatten()
        })
        .collect();
    gnomes.sort();
    info!(
        "azeroll commissioner: {} gnome displays, helmed ones: {:?}",
        gnomes.len(),
        gnomes
            .iter()
            .filter(|g| g.1)
            .map(|g| g.0)
            .take(12)
            .collect::<Vec<_>>()
    );
    if let Some(over) = crate::webenv::var("WOW_AZEROLL_COMMISSIONER").and_then(|v| v.parse().ok())
    {
        return Some(over);
    }
    gnomes
        .iter()
        .skip(1)
        .find(|g| g.1)
        .or_else(|| gnomes.get(1))
        .or_else(|| gnomes.first())
        .map(|g| g.0)
}

#[derive(Component)]
struct HudCam;

pub(super) fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "hud.wgsl");
    app.add_plugins(UiMaterialPlugin::<HudMaterial>::default())
        .init_resource::<ItemBubble>()
        .init_resource::<Commissioner>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(
            Update,
            (
                item_swap,
                stage_prince,
                prince_reactions,
                prince_motion,
                stage_commissioner,
                stage_family,
                commissioner_acts,
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
    let commissioner = images.add(new_target_image_sized(
        COMMISSIONER_TARGET.0,
        COMMISSIONER_TARGET.1,
    ));
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
    let commissioner_cam = cam(&mut commands, &commissioner, COMMISSIONER_LAYER, -88);
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
    // Likewise the commissioner's: in front of him, low and close, looking up at his chest.
    commands.entity(commissioner_cam).insert(
        Transform::from_translation(
            COMMISSIONER_STAGE
                + Vec3::new(
                    0.0,
                    COMMISSIONER_EYE * COMMISSIONER_SCALE,
                    -COMMISSIONER_DIST * COMMISSIONER_SCALE,
                ),
        )
        .looking_at(
            COMMISSIONER_STAGE + Vec3::Y * COMMISSIONER_LOOK * COMMISSIONER_SCALE,
            Vec3::Y,
        ),
    );
    let spinner = commands
        .spawn((Transform::from_translation(STAGE), Visibility::default()))
        .id();
    commands.insert_resource(HudModels {
        item,
        prince,
        commissioner,
        blank,
        item_cam,
        prince_cam,
        commissioner_cam,
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
            Option<&benilla_world::interior::InteriorLit>,
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
        } else if let Ok((pgt, mesh, mat, tag, lit)) = plain.get(d) {
            let rel = Mat4::from(to_local * pgt.affine());
            // An interior-baked part's material and tag name a world probe slot that pickup
            // releases (D30); the off-world stage takes the exterior lane instead.
            let (mat, tag) = match lit {
                Some(lit) if lit.is_bake() => (
                    lit.exterior_material(),
                    benilla_world::mesh_tag::spawn_tag(0, 1.0),
                ),
                _ => (
                    &mat.0,
                    tag.map_or(benilla_world::mesh_tag::spawn_tag(0, 1.0), |t| {
                        benilla_world::mesh_tag::with_alpha(t.0, 1.0)
                    }),
                ),
            };
            parts.push((mesh.0.clone(), mat.clone(), tag, rel));
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
///   stagecraft) queries him;
/// * [`StageRig`](crate::portrait::StageRig): `creature_anim::lod` parks (stops posing) every rig
///   outside the world camera's frustum after half a second, which a stage 3000 yd up always is.
///   That left him frozen in his first-frame pose though the driver played Run.
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
    // The corner gnome wears what the world Prince wears (D33): an outfit or a cousin chosen in the
    // Catalogue changes the next run's Prince, so the portrait is rebuilt when the two disagree.
    if let Some(now) = world_prince.iter().find_map(|n| n.display_id) {
        if *display != Some(now) {
            if let Ok(root) = stage.single() {
                commands.entity(root).despawn();
                bubble.prince_ready = false;
                *display = Some(now);
                return;
            }
            *display = Some(now);
        }
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
                // The world-view animation LOD parks any rig outside the world camera's
                // frustum, and a stage 3000 yd up always is: this marker exempts him.
                crate::portrait::StageRig,
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

/// The corner Prince mirrors the world Prince's body, standing still: the world Prince's
/// [`MovementState`] (his stride speed, forward flags, and with them the Run/Sprint pick, the dash
/// scramble and the wind-up), his [`CastHold`] (ReadySpellDirected, full-body when he stands and a
/// torso overlay while he moves) and the lean of his root about the feet are copied each frame onto
/// the stage gnome. Only the lean touches the stage transform, which keeps its place and yaw: he runs
/// on the spot. Both have the same model scale, so the same speed gives the same gait and cadence.
///
/// Before the world Prince exists (the menu, a restart) he stands.
#[allow(clippy::type_complexity)]
fn prince_motion(
    mut commands: Commands,
    world: Query<
        (&MovementState, &Transform, Option<&CastHold>),
        (With<Prince>, Without<StagePrince>),
    >,
    mut stage: Query<
        (Entity, &mut MovementState, &mut Transform, Has<CastHold>),
        (With<StagePrince>, Without<Prince>),
    >,
) {
    let Ok((root, mut mv, mut tf, held)) = stage.single_mut() else {
        return;
    };
    let (speed, flags, hold, lean) = match world.single() {
        Ok((m, wtf, hold)) => {
            // The world Prince's rotation is yaw × pitch(-lean): read the pitch back.
            let (_, pitch, _) = wtf.rotation.to_euler(EulerRot::YXZ);
            (m.speed, m.flags, hold.copied(), -pitch)
        }
        Err(_) => (0.0, 0, None, 0.0),
    };
    // Written only on a change, so the animation driver is not woken every frame.
    if mv.speed != speed || mv.flags != flags {
        mv.speed = speed;
        mv.flags = flags;
    }
    match (hold, held) {
        (Some(h), false) => {
            commands.entity(root).insert(h);
        }
        (None, true) => {
            commands.entity(root).remove::<CastHold>();
        }
        _ => {}
    }
    let want = Quat::from_rotation_y(PRINCE_YAW) * Quat::from_rotation_x(-lean * LEAN_SHOWN);
    if tf.rotation.angle_between(want) > 1e-4 {
        tf.rotation = want;
    }
}

/// The commissioner's marker.
#[derive(Component)]
struct StageCommissioner;

/// Put one inventor gnome on the commissioner stage the first time the hub or the results are up
/// (and keep him: the corner Prince's rig is kept too), isolated exactly as the corner Prince
/// is (see [`stage_prince`]), his parts on the commissioner layer alone.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn stage_commissioner(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    mut who: ResMut<Commissioner>,
    mut run: ResMut<Run>,
    assets: Option<Res<WorldAssets>>,
    mut tried: Local<bool>,
    stage: Query<Entity, With<StageCommissioner>>,
    with_unit: Query<
        (),
        (
            With<StageCommissioner>,
            With<benilla_world::world_unit::WorldUnit>,
        ),
    >,
    children: Query<&Children>,
    bare: Query<(), (With<Mesh3d>, Without<RenderLayers>)>,
    layered: Query<(), (With<Mesh3d>, With<RenderLayers>)>,
) {
    let wanted = matches!(
        phase.get(),
        Phase::Select | Phase::Results | Phase::Playing | Phase::Paused
    );
    // Read the catalog once, when the archives are open.
    if who.display.is_none() && !*tried {
        if let Some(assets) = &assets {
            *tried = true;
            let loaded = {
                let mut chain = assets.chain.lock_recover();
                load_creature_catalog(&mut chain)
            };
            match loaded {
                Ok(catalog) => who.display = commissioner_display(&catalog),
                Err(e) => warn!("azeroll commissioner: creature catalog unavailable: {e:#}"),
            }
            if let Some(d) = who.display {
                info!("azeroll commissioner: display {d}");
            }
        }
    }
    let Ok(root) = stage.single() else {
        who.ready = false;
        if !wanted {
            return;
        }
        let Some(display) = who.display else { return };
        let entity = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: run.guid(),
                display,
                scale: COMMISSIONER_SCALE,
                level: 1,
                faction: units::FACTION_FRIENDLY,
                bytes0: 7 | (1 << 8),
            },
            COMMISSIONER_STAGE,
            COMMISSIONER_YAW,
        );
        commands
            .entity(entity)
            .remove::<super::RunEntity>()
            .insert((
                StageCommissioner,
                crate::portrait::StageRig,
                crate::entities::Carried,
                Visibility::Visible,
                Transform::from_translation(COMMISSIONER_STAGE)
                    .with_rotation(Quat::from_rotation_y(COMMISSIONER_YAW))
                    .with_scale(Vec3::splat(COMMISSIONER_SCALE)),
            ));
        return;
    };
    // He stays once he is there (a respawn on the results screen would take longer than the
    // verdict to load): only his camera sleeps outside the hub and the results.
    if with_unit.contains(root) {
        commands
            .entity(root)
            .remove::<benilla_world::world_unit::WorldUnit>();
    }
    let only = RenderLayers::layer(COMMISSIONER_LAYER);
    let mut any = false;
    for d in children.iter_descendants(root) {
        if bare.contains(d) {
            commands.entity(d).insert(only.clone());
            any = true;
        } else if layered.contains(d) {
            any = true;
        }
    }
    if who.ready != any {
        who.ready = any;
    }
}

/// A cousin in the Family Portrait (D33), standing behind Fizzwick on his stage once all three have
/// been found: `slot` is where.
#[derive(Component)]
struct StageFamily {
    slot: usize,
}

/// Where the portrait's cousins stand, relative to Fizzwick, in his scale-1 yards: either side of him
/// and a step behind, small enough to peek over his shoulders.
const FAMILY_SPOTS: [(f32, f32); 3] = [(-0.47, 0.35), (0.47, 0.35), (0.0, 0.75)];
const FAMILY_SCALE: f32 = 0.72;

/// The Family Portrait: when every cousin has been found, put them on the commissioner's stage behind
/// him (his parts' layer rule is [`stage_commissioner`]'s). They cheer when he does. Built from
/// `Progress` and `Campaign` alone; it is the hub's trinket for the whole family.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn stage_family(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    campaign: Res<super::campaign::Campaign>,
    progress: Res<super::progress::Progress>,
    who: Res<Commissioner>,
    mut run: ResMut<Run>,
    stage: Query<(Entity, &StageFamily)>,
    children: Query<&Children>,
    bare: Query<(), (With<Mesh3d>, Without<RenderLayers>)>,
    with_unit: Query<
        (),
        (
            With<StageFamily>,
            With<benilla_world::world_unit::WorldUnit>,
        ),
    >,
    mut anim: MessageWriter<EmoteAnim>,
    mut seq: ResMut<PlaySeq>,
    mut cheered: Local<u32>,
) {
    let cousins: Vec<u32> = campaign
        .levels
        .iter()
        .filter_map(|l| l.cousin.as_ref().map(|c| (l, c)))
        .filter(|(l, _)| progress.best(&l.meta.id).cousin)
        .map(|(_, c)| c.display)
        .collect();
    let all = campaign.levels.iter().all(|l| l.cousin.is_some())
        && cousins.len() == campaign.levels.len();
    let all = all || crate::webenv::var("WOW_AZEROLL_FAMILY_PORTRAIT").is_some();
    let cousins = if cousins.len() == campaign.levels.len() {
        cousins
    } else {
        // The capture override: whoever the recipes hide, found or not.
        campaign
            .levels
            .iter()
            .filter_map(|l| l.cousin.as_ref().map(|c| c.display))
            .collect()
    };
    if !all || *phase.get() != Phase::Select || !who.ready {
        if !all {
            for (e, _) in &stage {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    if stage.iter().count() < cousins.len().min(FAMILY_SPOTS.len()) {
        for (slot, &display) in cousins.iter().enumerate().take(FAMILY_SPOTS.len()) {
            if stage.iter().any(|(_, f)| f.slot == slot) {
                continue;
            }
            let (dx, dz) = FAMILY_SPOTS[slot];
            let at = COMMISSIONER_STAGE + Vec3::new(dx, 0.0, dz) * COMMISSIONER_SCALE;
            let entity = units::spawn_unit(
                &mut commands,
                &UnitSpec {
                    serial: run.guid(),
                    display,
                    scale: FAMILY_SCALE,
                    level: 1,
                    faction: units::FACTION_FRIENDLY,
                    bytes0: 7 | (1 << 8),
                },
                at,
                COMMISSIONER_YAW - 0.15 * dx.signum(),
            );
            commands
                .entity(entity)
                .remove::<super::RunEntity>()
                .insert((
                    StageFamily { slot },
                    crate::portrait::StageRig,
                    crate::entities::Carried,
                    Visibility::Visible,
                    Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_y(COMMISSIONER_YAW - 0.15 * dx.signum()))
                        .with_scale(Vec3::splat(FAMILY_SCALE)),
                ));
            // Spawn one a frame; the layer pass below runs on the next.
            return;
        }
    }
    let only = RenderLayers::layer(COMMISSIONER_LAYER);
    for (root, _) in &stage {
        if with_unit.contains(root) {
            commands
                .entity(root)
                .remove::<benilla_world::world_unit::WorldUnit>();
        }
        for d in children.iter_descendants(root) {
            if bare.contains(d) {
                commands.entity(d).insert(only.clone());
            }
        }
        if who.cheer != *cheered {
            anim.write(EmoteAnim {
                entity: root,
                anim_id: ANIM_CHEER,
                seq: seq.next(),
                via_player: false,
            });
        }
    }
    *cheered = who.cheer;
}

/// His gestures: a flourish on arrival and on a cheer, and a talking gesture every so often while
/// a line types out.
fn commissioner_acts(
    time: Res<Time<Real>>,
    mut who: ResMut<Commissioner>,
    stage: Query<Entity, With<StageCommissioner>>,
    mut anim: MessageWriter<EmoteAnim>,
    mut seq: ResMut<PlaySeq>,
) {
    let dt = time.delta_secs();
    who.since_gesture += dt;
    let Ok(entity) = stage.single() else { return };
    let mut play = |anim_id: u16, who: &mut Commissioner| {
        who.since_gesture = 0.0;
        anim.write(EmoteAnim {
            entity,
            anim_id,
            seq: seq.next(),
            via_player: false,
        });
    };
    if let Some(id) = who.gesture.take() {
        play(id, &mut who);
    } else if who.cheer != who.cheered {
        who.cheered = who.cheer;
        play(ANIM_CHEER, &mut who);
    } else if who.talking && (!who.was_talking || who.since_gesture >= TALK_EVERY) && who.ready {
        let id = if who.was_talking {
            ANIM_TALK
        } else {
            ANIM_EXCLAIM
        };
        play(id, &mut who);
    }
    who.was_talking = who.talking;
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
    mut who: ResMut<Commissioner>,
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
    bubble.prince_for = if prince_on {
        bubble.prince_for + dt
    } else {
        0.0
    };
    bubble.prince_alpha = if prince_on && bubble.prince_for > PRINCE_SETTLE {
        (bubble.prince_alpha + dt * 3.0).min(1.0)
    } else {
        0.0
    };
    let hub_on = matches!(phase.get(), Phase::Select | Phase::Results) && !loading.covering();
    // Fizzwick's stage stays awake through play, too, so his porthole (and the results panel's
    // window) never shows the empty frames of a camera that has only just been woken.
    let who_on = (hub_on || hud_on) && who.ready;
    who.awake_for = if who_on { who.awake_for + dt } else { 0.0 };
    who.alpha = if who_on && who.awake_for > WHO_SETTLE {
        (who.alpha + dt * 3.0).min(1.0)
    } else {
        0.0
    };
    for (cam, on) in [
        (models.item_cam, item_on),
        (models.prince_cam, prince_on),
        (models.commissioner_cam, who_on),
    ] {
        if let Ok(mut camera) = cameras.get_mut(cam) {
            if camera.is_active != on {
                camera.is_active = on;
            }
        }
    }
}
