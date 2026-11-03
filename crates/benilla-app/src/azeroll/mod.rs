//! Azeroll: a Katamari-style mode on the benilla world, started by [`crate::run_azeroll`]. There is
//! no server: a ball rolls through Elwynn Forest and everything smaller than it sticks to it. The
//! map's own doodads are claimed from the terrain streamer and re-spawned as mode-owned copies
//! ([`props`]), creatures are local units dressed by the stock attach/animation systems
//! ([`creatures`]), and a collected thing is re-parented onto the ball's spinning frame
//! ([`collect`]). The player controller and FrameXML are off ([`crate::run_mode::CaptureMode`]).
//!
//! The shared vocabulary lives here: [`Ball`] / [`BallSpin`] / [`BallState`] for the ball,
//! [`Collectible`] for anything that can be rolled up, [`Collected`] / [`Bumped`] for what
//! happened this frame, [`Run`] for one attempt. Architecture notes: `docs/azeroll/ARCHITECTURE.md`.

mod auto;
mod ball;
mod buildings;
mod camera;
mod campaign;
mod city;
mod city_shell;
mod city_support;
mod collect;
mod creatures;
mod cutout;
mod debug_fill;
mod design;
mod finale;
mod fizz;
mod furniture;
mod glitch;
mod hud;
pub(crate) mod hud_models;
pub(crate) mod level;
mod lines;
mod lod;
mod music;
mod names;
mod npcs;
mod pad;
mod placement;
mod profile;
mod progress;
mod props;
mod ram;
mod replay;
mod ritual;
pub(crate) mod select;
mod sfx;
mod survey;
mod units;
mod unstuck;

use std::borrow::Cow;

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

/// Set by [`crate::run_azeroll`] before the app builds. wasm32 has no process environment, so the
/// flag lives here; `WOW_AZEROLL` (`?azeroll=1` on the web) still turns it on from outside.
pub(crate) static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed) || crate::webenv::var("WOW_AZEROLL").is_some()
}

pub(crate) struct AzerollPlugin;

impl Plugin for AzerollPlugin {
    fn build(&self, app: &mut App) {
        // Present before `OnEnter(InWorld)`, where the UI lifecycle decides not to load FrameXML.
        let (current, phase) = campaign::boot();
        campaign::plugin(app, current);
        app.insert_resource(crate::run_mode::CaptureMode)
            .insert_state(phase)
            .init_resource::<Rng>()
            .init_resource::<lines::Said>()
            .init_resource::<Run>()
            .init_resource::<ram::RamGate>()
            .init_resource::<BallState>()
            .add_message::<Collected>()
            .add_message::<Bumped>()
            .add_message::<Ram>()
            .add_message::<Touched>()
            .add_message::<GrowthBeat>()
            .add_message::<TierReached>()
            .add_message::<GoalReached>()
            .add_message::<StarEarned>()
            .add_message::<Dash>()
            .add_systems(Update, follow_map)
            .add_systems(
                Update,
                (
                    ball::drive_ball.run_if(finale::mobile),
                    collect::collect.run_if(finale::mobile),
                    ball::publish_focus,
                    camera::place_camera,
                )
                    .chain()
                    .in_set(WorldStage::Input),
            )
            .add_plugins((
                cutout::plugin,
                lod::plugin,
                finale::plugin,
                city::plugin,
                city_support::plugin,
            ))
            .add_plugins((ball::plugin, camera::plugin, collect::plugin))
            .add_plugins((
                design::plugin,
                placement::plugin,
                select::plugin,
                unstuck::plugin,
                props::plugin,
                furniture::plugin,
                progress::plugin,
                replay::plugin,
                ritual::plugin,
                fizz::plugin,
                music::plugin,
                creatures::plugin,
                npcs::plugin,
                pad::plugin,
            ))
            .add_plugins((
                hud::plugin,
                sfx::plugin,
                level::plugin,
                auto::plugin,
                survey::plugin,
                debug_fill::plugin,
                glitch::plugin,
            ))
            .add_systems(OnEnter(Phase::Select), unpause)
            .add_systems(OnEnter(Phase::Playing), unpause)
            .add_systems(OnEnter(Phase::Paused), pause)
            .add_systems(OnEnter(Phase::Ready), (unpause, reset_run))
            .add_systems(Startup, loading_art);
    }
}

/// The screen the mode is on. The app boots into [`Phase::Select`] (or, with
/// `WOW_AZEROLL_LEVEL_ID` / `WOW_AZEROLL_AUTO`, straight into a level's `Ready`).
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum Phase {
    /// The level-select hub: no level is running, the camera orbits the yard.
    #[default]
    Select,
    /// The chosen level's world is loading or waiting for the first input; the ball sits at the
    /// start.
    Ready,
    Playing,
    Paused,
    /// The level is over (goal captured, time up or called): the send-off plays, then the results
    /// panel docks beside the lifted clump (`ritual`).
    Results,
}

/// True while gameplay should advance: playing, and the loading screen is not covering the world.
pub(crate) fn playing(
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
) -> bool {
    *phase.get() == Phase::Playing && !loading.covering()
}

/// Everything spawned for a run that is not world-owned, despawned when the next begins.
#[derive(Component)]
pub(crate) struct RunEntity;

/// The ball's root: world translation only, never rotated or scaled, so children's local space is
/// world-aligned yards centred on the ball. [`BallState::pos`] is the authority; the root follows.
#[derive(Component)]
pub(crate) struct Ball;

/// The ball's rolling frame, the one child of [`Ball`]: carries the accumulated roll rotation
/// ([`BallState::spin`]). The core mesh and every attached trophy are its children, so one rotation
/// turns the whole katamari.
#[derive(Component)]
pub(crate) struct BallSpin;

/// The ball's live physical state. Owned by [`ball`]; read by everything else.
#[derive(Resource, Debug, Clone)]
pub(crate) struct BallState {
    /// Centre of the ball, Bevy world space (yards).
    pub pos: Vec3,
    /// Yards per second, Bevy world space.
    pub vel: Vec3,
    /// Accumulated roll rotation of [`BallSpin`].
    pub spin: Quat,
    /// Tank heading: forward is (-sin heading, 0, -cos heading).
    pub heading: f32,
    /// Remaining dash and recovery time, seconds.
    pub dash: f32,
    pub dash_cooldown: f32,
    /// The ball's effective radius in yards: what collides, what decides collectability, what the
    /// camera frames. Grows as `cbrt(volume)`.
    pub radius: f32,
    /// The radius this level started at (the floor shedding cannot go below, the core's size).
    pub start_radius: f32,
    /// The "volume" growth accumulates into (yards³, unitless scale factor dropped): `radius³`.
    pub volume: f32,
    /// Credited volume compacted from fully buried trophies, never live surface objects.
    pub buried_volume: f32,
    /// On the ground this frame (terrain or a floor hit).
    pub grounded: bool,
    /// The start's support is known. False only for an indoor start (`Design::start_indoors`)
    /// until the building's floor collision has loaded; play cannot begin before then, or the ball
    /// would settle on the terrain under the floor.
    pub floor_ready: bool,
    /// Seconds of reduced control left after a hard bump.
    pub stun: f32,
}

impl Default for BallState {
    fn default() -> Self {
        // A placeholder until a level is chosen; `reset_run` sets the real start.
        let r = 0.25;
        Self {
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            spin: Quat::IDENTITY,
            heading: 0.0,
            dash: 0.0,
            dash_cooldown: 0.0,
            radius: r,
            start_radius: r,
            volume: r * r * r,
            buried_volume: 0.0,
            grounded: false,
            floor_ready: true,
            stun: 0.0,
        }
    }
}

impl BallState {
    /// Diameter in yards, what the HUD shows.
    pub fn diameter(&self) -> f32 {
        self.radius * 2.0
    }
}

/// What kind of thing a [`Collectible`] is, for the HUD, the collection log and sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    /// A map doodad or a scattered copy of one: barrels, mugs, fences, trees.
    Prop,
    /// A local creature unit: rabbits, wolves, Defias.
    Creature,
    /// The level's goal creature, when its goal is `collect` (Hogger in Market Day).
    Boss,
}

/// Where a [`Collectible`] came from, so collecting it can retire its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Source {
    /// An ADT MDDF placement claimed from the streamer, by `uniqueId`.
    Doodad(u32),
    WmoProp(benilla_world::terrain_stream::WmoPropKey),
    /// Spawned by the mode itself (scatter copies, creatures).
    Spawned,
}

/// Anything the ball can roll up. Lives on the entity whose [`Transform`] places the thing in the
/// world (a prop's copy root, a creature's unit root). Inserted only once the size is known (the
/// model has loaded); until then the thing is scenery.
#[derive(Component, Debug, Clone)]
pub(crate) struct Collectible {
    /// Display name for the HUD ("Barrel", "Young Wolf").
    pub name: Cow<'static, str>,
    pub kind: Kind,
    pub source: Source,
    /// Visual bounding-sphere radius in yards after scale; not the pickup gate.
    pub size: f32,
    /// Full maximum axis of the scaled oriented bbox, in yards.
    pub pickup: f32,
    /// Unscaled local bbox half-extents for rotating protrusion support.
    pub half_extents: Vec3,
    /// The bounding sphere's centre, in the entity's local space (before its scale), so
    /// `transform.transform_point(center)` is the world centre.
    pub center: Vec3,
    /// Radius in yards of the solid part that blocks the ball while the thing is too big to collect
    /// (a tree's trunk, not its canopy). `0.0` = never blocks (bushes, grass, critters).
    pub solid: f32,
}

impl Collectible {
    /// Sphere-normalized volume: multiplying by 4π/3 gives physical yd³.
    /// Keeping this normalization makes radius = cbrt(volume) exactly equivalent to 3V/4π.
    pub(crate) fn volume(&self, scale: Vec3) -> f32 {
        let extent = 2.0 * self.half_extents * scale.abs();
        extent.x * extent.y * extent.z * level::FILL_FACTOR * 3.0 / (4.0 * std::f32::consts::PI)
    }
}

/// On a collectible once it is stuck to the ball: its entity is now a child of [`BallSpin`].
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Attached {
    /// Distance of its centre from the ball centre when it stuck, yards.
    pub radial: f32,
    /// Exact credited volume, returned if this trophy is shed.
    pub volume: f32,
    /// Its [`Collectible::size`] at the time.
    pub size: f32,
    /// [`Run::elapsed`] when it stuck.
    pub at: f32,
}

/// Something was rolled up this frame. Written by [`collect`], read by the HUD, props (tombstones),
/// creatures and sound.
#[derive(Message, Debug, Clone)]
pub(crate) struct Collected {
    pub entity: Entity,
    pub name: Cow<'static, str>,
    pub kind: Kind,
    pub source: Source,
    /// Visual bounding-sphere radius in yards; retained for extent/recognition.
    pub size: f32,
    /// Full maximum scaled box axis in yards, copied from `Collectible::pickup`.
    /// Roll-up requires diameter >= 2.15 * pickup; this is not a sphere radius.
    pub pickup: f32,
}

/// The ball hit something too big to roll up. Written by [`collect`]; `strength` is the closing
/// speed in yards/s along the contact normal.
#[derive(Message, Debug, Clone)]
pub(crate) struct Bumped {
    pub entity: Option<Entity>,
    pub name: Cow<'static, str>,
    pub strength: f32,
}

/// Gameplay contact classification; `collect::contact` is the sole classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Contact {
    RollUp,
    Dislodge,
    Wall,
}

/// A non-pickup contact. Unlike `Contact`, this cannot describe a successful roll-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TouchState {
    Dislodge,
    Wall,
}

/// A collectible touched without being picked up, written by `collect`.
/// May repeat each simulation frame: presentation consumers must budget/cool down reactions.
/// Dislodge describes eligibility; a ballistic launch still requires strength > 0.5 yd/s.
/// This is independent of `Bumped` (hard impacts/rams); do not sum the two as damage.
#[derive(Message, Debug, Clone)]
pub(crate) struct Touched {
    pub entity: Entity,
    pub state: TouchState,
    /// Nonnegative closing speed along the contact normal, yards/s, before response.
    pub strength: f32,
    /// Contact point on the ball, Bevy world space in yards (not WoW coordinates).
    pub point: Vec3,
}

/// Hostile ram request, consumed by `collect`; emitted once per charge by creatures/NPCs.
#[derive(Message, Debug, Clone)]
pub(crate) struct Ram {
    pub entity: Entity,
    pub name: Cow<'static, str>,
    /// Ground-plane unit direction away from the attacker.
    pub dir: Vec3,
}

/// A new diameter high-water step: 1.25^step times this run's starting diameter.
/// `level` emits at most one per frame (the highest crossed step), never on shedding/regrowth.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct GrowthBeat {
    /// One-based logarithmic step; zero is the starting diameter and is never emitted.
    pub step: u32,
    /// Diameter threshold crossed and actual diameter, in yards.
    pub threshold: f32,
    pub diameter: f32,
}

/// First attainment of a TierTable row this run, emitted by `level`.
/// A jump emits each newly reached row in order; shedding never repeats a row.
#[derive(Message, Debug, Clone)]
pub(crate) struct TierReached {
    /// Zero-based row index in `level::TierTable` (stable within this level).
    pub tier: usize,
    pub name: &'static str,
    /// Row threshold at emission and actual ball diameter, yards.
    pub threshold: f32,
    pub diameter: f32,
}

/// A size goal was met for the first time this run (the "goal beat"). Written by `level`.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct GoalReached {
    pub diameter: f32,
    pub goal: f32,
}

/// The dial lit another star after the goal: `n` is the count now (2..=4). Written by `level`.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct StarEarned {
    pub n: u8,
}

/// Dash lifecycle, written by `ball`: a held dash input winds up (`WindUp`, once) before it bursts.
/// `Ready` means cooldown crossed zero, not that a dash was requested.
/// Read BallState::dash/dash_cooldown for continuous UI; do not reconstruct timers.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dash {
    /// The wind-up proper has begun (a hold past a tap); the Prince scrambles on the spot.
    WindUp,
    Started,
    Ended,
    Ready,
}

/// One attempt at the level.
#[derive(Resource, Debug, Default)]
pub(crate) struct Run {
    /// D26 terminal presentation only; never changes collision/growth.
    pub final_diameter: Option<f32>,
    /// Seconds of play (virtual time, so pause stops it).
    pub elapsed: f32,
    /// Things rolled up.
    pub count: u32,
    /// The biggest thing rolled up so far: `(name, size)`.
    pub biggest: Option<(Cow<'static, str>, f32)>,
    /// The collection log: name → how many.
    pub log: std::collections::BTreeMap<Cow<'static, str>, u32>,
    /// What the Catalogue needs to know of each name rolled up (`progress::note_finds`).
    pub finds: std::collections::BTreeMap<Cow<'static, str>, Find>,
    /// How many of each name's pickups the Catalogue has already been told of, so a merge on pause,
    /// restart or the results never counts one twice.
    pub merged: std::collections::BTreeMap<Cow<'static, str>, u32>,
    /// Names this run found for the first time ever, in the order they were rolled up.
    pub new_names: Vec<String>,
    /// The goal was met.
    pub won: bool,
    /// Play seconds at which the goal was first met (a size goal keeps rolling after it).
    pub goal_time: Option<f32>,
    /// Stars lit on the dial after a size goal: one at the goal, one per grade boundary beyond.
    pub stars: u8,
    /// The player ended the run by holding the finish key.
    pub called: bool,
    /// A centre-screen announcement and its seconds left.
    pub banner: Option<(String, f32)>,
    /// Creature GUID serials stay unique across resets: caches keyed by guid may outlive a despawn.
    pub next_guid: u64,
    /// Every pickup in order, for the challenges that care about order and pace (`before`, `within`).
    pub picks: Vec<Picked>,
    /// The level's parcel was rolled up (sticky: a shed parcel stays found), and the cousin, with
    /// whether they were caught inside their window (D33).
    pub parcel_found: bool,
    pub cousin: Option<(String, bool)>,
    /// The save has been told (`Progress::merge_replay`), and whether each flag was new to it.
    pub parcel_merged: bool,
    pub cousin_merged: bool,
    pub parcel_new: bool,
    pub cousin_new: bool,
}

/// One pickup of the run's log: what, and when.
#[derive(Debug, Clone)]
pub(crate) struct Picked {
    pub name: Cow<'static, str>,
    /// The creature species when it is one (a herd member's name is its species).
    pub species: Option<Cow<'static, str>>,
    /// Play seconds.
    pub t: f32,
}

/// One name's facts for the Catalogue.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Find {
    /// The biggest axis seen, yards.
    pub axis: f32,
    /// A `names::Category` index.
    pub cat: u8,
    /// Order of first pickup this run.
    pub seq: u32,
}

impl Run {
    pub fn guid(&mut self) -> u64 {
        self.next_guid += 1;
        self.next_guid
    }
}

/// xorshift64*: the mode's dice, seeded from the clock.
#[derive(Resource)]
pub(crate) struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(seed | 1)
    }
}

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
}

/// Keep the engine on the level's map.
fn follow_map(current: Option<ResMut<benilla_world::world_map::CurrentMap>>) {
    if let Some(mut current) = current {
        if current.0 != level::MAP {
            current.0 = level::MAP;
        }
    }
}

/// The user's Azeroll painting as the loading screen's backdrop, in place of the map's art.
fn loading_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let bytes = include_bytes!("../../../../assets/art/azeroll-loading.png");
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(art) => {
            let rgba = art.to_rgba8();
            let (width, height) = rgba.dimensions();
            let art = Image::new(
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                rgba.into_raw(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            commands.insert_resource(crate::loading_screen::LoadingArtOverride(images.add(art)));
        }
        Err(e) => warn!("azeroll: loading art did not decode, the map's art stays: {e}"),
    }
}

fn pause(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn unpause(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

/// Back at the start: everything the last run spawned goes, and the run and ball start over.
/// Claimed map doodads are restored by [`props`] on the same transition.
fn reset_run(
    mut commands: Commands,
    spawned: Query<Entity, With<RunEntity>>,
    mut run: ResMut<Run>,
    mut ball: ResMut<BallState>,
    design: Res<design::Design>,
) {
    for e in &spawned {
        commands.entity(e).despawn();
    }
    let next_guid = run.next_guid;
    *run = Run {
        next_guid,
        ..Run::default()
    };
    *ball = BallState::default();
    ball.pos = units::wow_to_bevy(design.start);
    ball.heading = design.start_heading;
    ball.radius = design.start_radius;
    ball.start_radius = design.start_radius;
    ball.floor_ready = !design.start_indoors;
    ball.volume = design.start_radius.powi(3);
}
