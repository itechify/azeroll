//! Embedded level recipe; native overrides are read atomically on R, never during a run.
use super::{
    level::{Behavior, Look, Place, Species},
    *,
};
use benilla_world::{collision::WorldCollision, world_point::WorldPoint};
use bevy::prelude::*;
use serde::Deserialize;

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Design {
    /// The campaign's view of this level: title, briefing, unlock rule.
    pub level: LevelMeta,
    pub start: [f32; 3],
    pub start_radius: f32,
    /// Optional terminal traversal limit, including Free Roll.
    #[serde(default)]
    pub live_radius_limit: Option<f32>,
    /// The start is on a building's floor: hold its authored height until the floor has loaded,
    /// rather than settling on the terrain under it.
    #[serde(default)]
    pub start_indoors: bool,
    /// Local herd actors may use nearby WMO decks over terrain holes.
    #[serde(default)]
    pub wmo_floors: bool,
    /// Debug autopilot may plan through real walking collision within these authored zones.
    #[serde(default)]
    pub collision_navigation: Vec<ClaimZone>,
    /// Radians; the ball's tank heading at the start (0 faces Bevy -Z, i.e. WoW north).
    #[serde(default)]
    pub start_heading: f32,
    pub time_limit: f32,
    /// The soft leash: where the ball may roam. Absent: centred on `start`, 680 yd.
    #[serde(default)]
    pub bounds: Option<Bounds>,
    /// Doodads inside any zone are claimed from the streamer (union over every level, installed
    /// once at startup). Absent: one circle at `start`, [`DEFAULT_CLAIM_RADIUS`].
    #[serde(default)]
    pub claim: Vec<ClaimZone>,
    pub goal: Goal,
    /// Whole MODF targets, opt-in per recipe; see D25.
    #[serde(default)]
    pub buildings: Vec<u32>,
    /// Hogger's camp and early-pass village point; without them there is no staging.
    #[serde(default)]
    pub camp: Option<[f32; 3]>,
    #[serde(default)]
    pub patrol: Option<[f32; 3]>,
    #[serde(default = "default_patrol_at")]
    pub patrol_at: f32,
    #[serde(default = "default_patrol_until")]
    pub patrol_until: f32,
    #[serde(default)]
    pub tiers: Vec<TierSpec>,
    #[serde(default)]
    pub challenges: Vec<ChallengeSpec>,
    /// The level's hidden parcel (D33): the reward the scene item flagged `present` unlocks.
    #[serde(default)]
    pub present: Option<PresentSpec>,
    #[serde(default)]
    pub barriers: Vec<Barrier>,
    #[serde(default)]
    pub areas: Vec<Area>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub trails: Vec<Trail>,
    #[serde(default)]
    pub herds: Vec<Herd>,
    #[serde(skip)]
    pub roster: Vec<Species>,
    /// FNV-1a of the recipe text this was parsed from, so a report names the level it measured.
    #[serde(skip)]
    pub recipe_hash: u64,
}

fn default_patrol_at() -> f32 {
    5.0
}
fn default_patrol_until() -> f32 {
    90.0
}

/// Claim radius of a recipe that lists no zones, yards (Goldshire's, the original).
pub(super) const DEFAULT_CLAIM_RADIUS: f32 = 650.0;
/// Leash radius of a recipe with no `bounds`, yards.
pub(super) const DEFAULT_LEASH: f32 = 680.0;

/// The `[present]` table: what rolling up the level's parcel unlocks. `id` is the wardrobe key the
/// save stores, so it never changes; `display` is the gnome `CreatureDisplayInfo` the Prince wears.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PresentSpec {
    pub id: String,
    pub title: String,
    pub display: u32,
}

/// The `[level]` table: what the level hub shows and how it unlocks.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct LevelMeta {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    /// A short in-character request.
    #[serde(default)]
    pub briefing: String,
    /// One line: "Reach 1.5 yd".
    #[serde(default)]
    pub objective: String,
    /// The level whose first win opens this one; absent: open from the start.
    #[serde(default)]
    pub unlock_after: Option<String>,
    /// Preferred opening track ([`super::music`] track name).
    #[serde(default)]
    pub music: Option<String>,
    /// Name of the NPC cast (informational; the registry embeds the file).
    #[serde(default)]
    pub cast: Option<String>,
    /// The verdict's size grades as multiples of the goal diameter: Respectable, Splendid,
    /// Preposterous (default 1.2, 1.5, 2.0). See `progress::judge_win`.
    #[serde(default)]
    pub grades: Option<[f32; 3]>,
    /// The verdict's pace grades as shares of the time limit used (default 0.8, 0.6, 0.4).
    #[serde(default)]
    pub pace: Option<[f32; 3]>,
    /// Par, seconds: a win under it earns the time-bonus comet (default half the time limit).
    #[serde(default)]
    pub par: Option<f32>,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Bounds {
    /// Absent: the start.
    #[serde(default)]
    pub centre: Option<[f32; 3]>,
    pub radius: f32,
}

/// One claim shape: a disc, or (with `to`) a capsule from `centre` to `to`. Flat distance.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct ClaimZone {
    pub centre: [f32; 3],
    pub radius: f32,
    #[serde(default)]
    pub to: Option<[f32; 3]>,
}

impl ClaimZone {
    /// Whether a Bevy-space point is inside.
    pub fn contains(&self, p: Vec3) -> bool {
        let a = units::wow_to_bevy(self.centre);
        let b = self.to.map_or(a, units::wow_to_bevy);
        let ab = Vec2::new(b.x - a.x, b.z - a.z);
        let ap = Vec2::new(p.x - a.x, p.z - a.z);
        let len2 = ab.length_squared();
        let t = if len2 > 1e-6 {
            (ap.dot(ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (ap - ab * t).length_squared() <= self.radius * self.radius
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GoalKind {
    /// Roll up a named creature (a herd `species`), which is the level's boss.
    Collect,
    /// Reach a ball diameter.
    Size,
    /// Scripted assembly pickup at a size-gated safe trigger.
    Finale,
}

/// What wins the level.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Goal {
    pub kind: GoalKind,
    /// `collect`: the herd species to roll up.
    #[serde(default)]
    pub target: String,
    /// `size`: the diameter that wins, yards.
    #[serde(default)]
    pub diameter: f32,
    /// Optional structural pickups required before a terminal assembly can trigger.
    #[serde(default)]
    pub city_pieces: u32,
    /// `collect` of a scene prop (the `goal = true` item, named by `target`): its pickup axis,
    /// yards after scale. The tier table and the autopilot need it before the model has loaded;
    /// a mismatch with the loaded model is logged.
    #[serde(default)]
    pub axis: f32,
    /// The area the goal lies in (the autopilot's primary route ends there).
    #[serde(default)]
    pub area: Option<String>,
    /// Headline when won; absent: "<TARGET>, ROLLED UP!" or "BIG ENOUGH!".
    #[serde(default)]
    pub title: Option<String>,
    /// Results line when time runs out; absent: derived.
    #[serde(default)]
    pub fail: Option<String>,
}

/// One growth tier: announced when the ball's diameter reaches `at`, or (with `reps`) when every
/// representative is edible, whichever is larger.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct TierSpec {
    pub name: String,
    /// Default diameter, yards (used while a representative's size is unknown).
    pub at: f32,
    /// `species:<Name>` or `adults`.
    #[serde(default)]
    pub reps: Vec<String>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ChallengeKind {
    /// Every one of a species (or every placed prop of a model / display name) rolled up.
    CollectAll,
    /// Win in under `secs`.
    Time,
    /// Every prop of the scene named `scene` rolled up.
    Scene,
    /// Win without a ram.
    Unrammed,
    /// Finish at least `diameter` yards across.
    Size,
    /// Every member of the set (`species` / `name`) was rolled up before the first pickup of the
    /// gate (`before_species` / `before_name`), or the gate never was.
    Before,
    /// Every prop of the scene was rolled up inside `secs` of its first.
    Within,
}

/// An optional challenge. `id` is what the save file stores, so it never changes.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct ChallengeSpec {
    pub id: String,
    pub title: String,
    pub kind: ChallengeKind,
    #[serde(default)]
    pub species: String,
    /// A model path or display name (`collect-all`).
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub secs: f32,
    #[serde(default)]
    pub scene: String,
    /// `scene`: how far from the anchor its things lie, yards.
    #[serde(default = "default_scene_radius")]
    pub radius: f32,
    #[serde(default)]
    pub diameter: f32,
    /// `before`: the gate, by species or by display name.
    #[serde(default)]
    pub before_species: String,
    #[serde(default)]
    pub before_name: String,
    /// `collect-all`: every one of every listed display name / species.
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub species_list: Vec<String>,
    /// `scene`: every scene whose name starts with this, as the union.
    #[serde(default)]
    pub prefix: String,
}

fn default_scene_radius() -> f32 {
    4.0
}

/// An invisible wall segment: a mode-owned solid that blocks the ball at every size and is never
/// collected. `from`/`to` are WoW points (the z of the lower one sets the base, a yard below).
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Barrier {
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// Full height from the base, yards.
    #[serde(default = "default_barrier_height")]
    pub height: f32,
    /// Full thickness, yards. Thick enough that a dash cannot cross the box in one frame.
    #[serde(default = "default_barrier_thickness")]
    pub thickness: f32,
    /// Extra length past each end (jambs), yards.
    #[serde(default = "default_barrier_extend")]
    pub extend: f32,
}

fn default_barrier_height() -> f32 {
    6.0
}
fn default_barrier_thickness() -> f32 {
    2.0
}
fn default_barrier_extend() -> f32 {
    0.5
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Area {
    pub name: String,
    pub centre: [f32; 3],
    pub radius: f32,
    #[serde(default)]
    pub enclosed: bool,
    #[serde(default)]
    pub clearance: f32,
    #[serde(default)]
    pub doors: Vec<[f32; 3]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scene {
    pub name: String,
    pub anchor: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub indoors: bool,
    pub items: Vec<Item>,
}
/// One group of props in a scene. Items are the unit of composition: a scene is a few
/// groups that tell a story, each a different arrangement and usually a mix of models.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Item {
    /// One model, or `models` for a mix (a creature item uses `species` instead).
    #[serde(default)]
    pub model: String,
    /// Mixed models for the group, chosen per prop by `pick`; repeat an entry to weight it.
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub pick: Pick,
    #[serde(default)]
    pub species: String,
    pub count: usize,
    pub arrangement: Arrangement,
    /// Arrangement size, yards: step for `line`/`grid`/`spill`, radius for `ring`, `cluster`,
    /// `heap` and `table-set`, density for `pile`/`scatter`.
    #[serde(default = "one")]
    pub spacing: f32,
    #[serde(default = "one")]
    pub scale: f32,
    /// Random scale, plus or minus this fraction.
    #[serde(default)]
    pub jitter: f32,
    #[serde(default)]
    pub solid: bool,
    /// Permanently solid, visible and inedible at every ball size.
    #[serde(default)]
    pub blocker: bool,
    /// Planted scenery stays anchored until it reaches ordinary RollUp eligibility.
    #[serde(default)]
    pub rooted: bool,
    /// Fixed pitch in radians about local X, after yaw (unlike random tilt).
    #[serde(default)]
    pub pitch: f32,
    /// Fixed roll in radians about local Z.
    #[serde(default)]
    pub roll: f32,
    /// This (single) prop is the level's goal: a `collect` goal whose `target` is not a herd
    /// species. It becomes a `Kind::Boss` named `target`, and solid until it can be rolled up.
    #[serde(default)]
    pub goal: bool,
    /// This (single) prop is the level's parcel (`[present]`): a Winter Veil gift box that unlocks
    /// the recipe's outfit when rolled up. Never solid, never the goal.
    #[serde(default)]
    pub present: bool,
    /// Display-name override (the parcel's "Fizzwick's Parcel").
    #[serde(default)]
    pub name: Option<String>,
    /// Where the group sits relative to the scene anchor (east, south), yards.
    #[serde(default)]
    pub offset: [f32; 2],
    /// Height above the ground of the group's base, yards.
    #[serde(default)]
    pub lift: f32,
    /// A fixed yaw for every prop; omitted, each prop gets its own.
    #[serde(default)]
    pub yaw: Option<f32>,
    /// Random lean of up to this many radians, for small things that were dropped.
    #[serde(default)]
    pub tilt: f32,
    /// Fraction of props lying on their side, and the height a toppled prop is raised by.
    #[serde(default)]
    pub topple: f32,
    #[serde(default)]
    pub topple_lift: f32,
    /// Sideways noise on a `line`, `grid` or `ring`, yards.
    #[serde(default)]
    pub wobble: f32,
    /// Closest two props of a `cluster`, `heap` or `table-set` may sit, yards.
    #[serde(default)]
    pub gap: f32,
    /// Height of each `heap` layer, yards.
    #[serde(default)]
    pub step: f32,
    /// Direction of a `spill` or a table row: radians, 0 east, pi/2 south.
    #[serde(default)]
    pub heading: f32,
    /// How much a `spill` may turn per prop, radians.
    #[serde(default)]
    pub bend: f32,
    /// A `table-set` puts its props on this model's top, `surface` yards up.
    #[serde(default)]
    pub table: String,
    #[serde(default = "one_count")]
    pub tables: usize,
    #[serde(default = "one")]
    pub table_gap: f32,
    #[serde(default = "one")]
    pub table_scale: f32,
    #[serde(default)]
    pub surface: f32,
    /// The surface this group is meant to rest on (WoW z, before `lift`) when it is not the scene
    /// anchor's floor: the bar top, a hearth counter. Only the placement validator reads it.
    #[serde(default)]
    pub on: Option<f32>,
    /// The group stands on one level surface (a table top on sloping ground): the spawn samples the
    /// ground once, at the group's `offset`, instead of under each prop.
    #[serde(default)]
    pub level: bool,
    /// The smallest ball radius a `stairs` run or a pitched plank must carry onto its surface
    /// (default the level's start radius). Only the placement validator reads it.
    #[serde(default)]
    pub climb: Option<f32>,
}
impl Item {
    /// The display name of one of this item's props laid from `model`: the item's `name` override
    /// (the parcel's), else the model's.
    pub fn label(&self, model: &str) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| super::names::display_name(model))
    }
}
fn one() -> f32 {
    1.0
}
fn one_count() -> usize {
    1
}
#[derive(Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Pick {
    #[default]
    Random,
    Cycle,
}
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Arrangement {
    /// A sunflower spiral, even density.
    Pile,
    Ring,
    Line,
    /// A run starting at offset, rising `step` yards per prop. Always solid.
    Stairs,
    Grid,
    /// Props on a table or bench model (or the floor when none is given).
    TableSet,
    /// Even density with random positions.
    Scatter,
    /// A dense centre with a falloff and a few stragglers.
    Cluster,
    /// Props resting on props, in layers.
    Heap,
    /// A wandering trail of uneven steps, for things that were dropped on the way.
    Spill,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Trail {
    /// Optional tight route waypoint, in yards (e.g. a bridge mouth).
    #[serde(default)]
    pub arrival_radius: Option<f32>,
    /// Route connection across support without adding loose props (e.g. a city canal).
    #[serde(default)]
    pub navigation_only: bool,
    pub name: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub spacing: f32,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub pick: Pick,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default = "trail_jitter")]
    pub jitter: f32,
    /// How far the trail wanders off the straight line, yards.
    #[serde(default)]
    pub wobble: f32,
    /// Plus or minus this fraction of `spacing` between props.
    #[serde(default = "trail_variance")]
    pub variance: f32,
    /// Chance a drop is a pair of props rather than one.
    #[serde(default)]
    pub clump: f32,
    #[serde(default = "trail_tilt")]
    pub tilt: f32,
}
fn trail_jitter() -> f32 {
    0.12
}
fn trail_variance() -> f32 {
    0.4
}
fn trail_tilt() -> f32 {
    0.15
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Herd {
    pub species: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub display: u32,
    pub count: u32,
    pub at: [f32; 3],
    pub radius: f32,
    pub fit: f32,
    pub behaviour: String,
    /// A legendary creature (Hogger): staged, heavy, and a `Kind::Boss` even when it is not the
    /// goal. Rolling it up wins only when it is the goal's `target`.
    #[serde(default)]
    pub boss: bool,
}
struct Prop {
    model: String,
    src: Authored,
    /// Where the spawn samples the ground, when not under the prop (`Item::level`).
    ground_at: Option<Vec3>,
    tf: Transform,
    indoors: bool,
    solid: bool,
    blocker: bool,
    rooted: bool,
    /// The level's goal prop (a `Kind::Boss`, see [`GoalProp`]).
    goal: bool,
    /// The level's parcel, named for the Catalogue (see [`Present`]).
    present: Option<String>,
    lift: f32,
}
#[derive(Resource, Default)]
pub(super) struct Pending {
    props: Vec<Prop>,
    /// A level has been chosen and planned (the hub phase has nothing to validate).
    armed: bool,
    settled: bool,
    frames: u32,
    inventory: std::collections::HashMap<Entity, (Vec3, f32, f32)>,
    planned: Vec<(Vec3, Vec3, Handle<benilla_assets::M2Model>)>,
    /// Where each trail's props sit in `props`/`planned` (trails are planned after the scenes).
    trail_ranges: Vec<std::ops::Range<usize>>,
    /// Each planned prop's authored lift, parallel to `planned` (`props` shrinks as they spawn).
    lifts: Vec<f32>,
    /// Permanently inedible authored props never enter progression food totals.
    blockers: Vec<bool>,
    report: ReportState,
    /// Claimed map doodads seen so far (they stream in with the terrain).
    claimed: usize,
}
impl Pending {
    /// Every planned prop is spawned and resolved (see `validate`), so a report can read them.
    pub(super) fn settled(&self) -> bool {
        self.armed && self.settled
    }
    /// Props the spawn never placed (no floor under them yet), with their authored spot.
    pub(super) fn unplaced(&self) -> impl Iterator<Item = (Authored, &str, Vec3)> {
        self.props
            .iter()
            .map(|p| (p.src, p.model.as_str(), p.tf.translation))
    }
}
#[derive(Default)]
struct ReportState {
    frames: u32,
    emitted: u32,
    /// Inventory size at the last report.
    seen: usize,
}
#[derive(Component)]
struct Designed;
/// Where a spawned recipe prop came from and where the spawn ray put it, for the placement
/// validator (`placement.rs`). `scene`/`item` index the recipe; a trail prop has `trail` set.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct Authored {
    pub scene: usize,
    pub item: usize,
    pub n: usize,
    pub trail: bool,
    /// The authored height (the scene anchor's or trail end's floor), Bevy y.
    pub hint: f32,
    /// The surface the spawn found (Bevy y), before `lift`.
    pub ground: f32,
    pub lift: f32,
}
#[derive(Component)]
pub(super) struct Gate;
#[derive(Component)]
pub(super) struct Blocker;
/// On the recipe's goal prop: `props::assemble` makes it a `Kind::Boss` named for the goal, so the
/// HUD guidance, the sound and the finale treat it like Hogger.
#[derive(Component)]
pub(super) struct GoalProp(pub String);

/// On the level's parcel (D33): `props::assemble` names it, `replay` watches for its `Collected`.
#[derive(Component)]
pub(super) struct Present(pub String);

/// A recipe barrier's solid. It is a [`Collectible`] only so the ball's existing solid paths see it
/// (`collect::solid_push`, Unstuck's safe-history, the autopilot's blockers); its pickup axis is
/// so large that `collect::contact` calls it a `Wall` at any radius, so it is never rolled up and
/// never knocked loose.
#[derive(Component)]
pub(super) struct BarrierWall;

/// The pickup axis of a barrier: no ball is ever this big.
const BARRIER_AXIS: f32 = 1.0e6;

pub(super) fn plugin(app: &mut App) {
    let current = app
        .world()
        .resource::<super::campaign::CurrentLevel>()
        .entry();
    app.insert_resource(load(current))
        .init_resource::<Pending>()
        .add_systems(OnEnter(Phase::Ready), reload.before(super::reset_run))
        .add_systems(
            OnEnter(Phase::Ready),
            spawn_barriers.after(super::reset_run).after(reload),
        )
        .add_systems(
            Update,
            (spawn, seat, validate, report)
                .chain()
                .after(benilla_world::schedule::WorldStage::Stream),
        );
}
/// The recipe for `entry`: the native `WOW_AZEROLL_LEVEL` override when it reads and parses, else
/// the embedded text.
pub(super) fn load(entry: &super::campaign::LevelEntry) -> Design {
    let embedded = entry.recipe;
    #[cfg(not(target_arch = "wasm32"))]
    let text = std::env::var("WOW_AZEROLL_LEVEL")
        .ok()
        .map(|p| {
            std::fs::read_to_string(&p).unwrap_or_else(|e| {
                error!("azeroll LEVEL read {p}: {e}; using embedded");
                embedded.into()
            })
        })
        .unwrap_or_else(|| embedded.into());
    #[cfg(target_arch = "wasm32")]
    let text = embedded.to_owned();
    let d = parse(&text).unwrap_or_else(|e| {
        error!("azeroll LEVEL parse: {e}; using embedded");
        parse(embedded).expect("embedded level")
    });
    info!(
        "azeroll LEVEL {} ({}): {} areas, {} scenes, {} trails, {} herds, {} barriers; goal={:?} limit={} cast={:?}",
        entry.id,
        d.level.title,
        d.areas.len(),
        d.scenes.len(),
        d.trails.len(),
        d.herds.len(),
        d.barriers.len(),
        d.goal.kind,
        d.time_limit,
        d.level.cast
    );
    d
}

/// Parse a recipe and resolve what is derived from it (the creature roster). No logging, so tests
/// can use it.
pub(super) fn parse(text: &str) -> Result<Design, String> {
    let mut d: Design = toml::from_str(text).map_err(|e| e.to_string())?;
    if !(d.start_radius.is_finite() && d.start_radius > 0.0 && d.time_limit > 0.0) {
        return Err("start_radius and time_limit must be positive".into());
    }
    if d.goal.kind == GoalKind::Finale && d.goal_area().is_none() {
        return Err("finale goal requires a named trigger area".into());
    }
    if matches!(d.goal.kind, GoalKind::Size | GoalKind::Finale)
        && !(d.goal.diameter > 2.0 * d.start_radius)
    {
        return Err("a size goal must be bigger than the start".into());
    }
    if d.live_radius_limit
        .is_some_and(|r| !r.is_finite() || r <= d.start_radius)
    {
        return Err("live_radius_limit must be finite and above the start radius".into());
    }
    let parcels: usize = d
        .scenes
        .iter()
        .flat_map(|s| &s.items)
        .filter(|i| i.present)
        .map(|i| i.count)
        .sum();
    if parcels != usize::from(d.present.is_some()) {
        return Err(format!(
            "a recipe with [present] needs exactly one item with present = true (found {parcels})"
        ));
    }
    d.recipe_hash = fnv(text);
    d.roster = d
        .herds
        .iter()
        .map(|h| {
            Ok(Species {
                name: h.species.clone(),
                look: if h.display > 0 {
                    Look::DisplayId(h.display)
                } else {
                    Look::ModelPath(h.model.clone())
                },
                count: h.count,
                scale: 1.0,
                fit: Some(h.fit),
                place: Place::Around {
                    at: h.at,
                    radius: h.radius,
                },
                behavior: match h.behaviour.as_str() {
                    "passive" => Behavior::Passive,
                    "skittish" => Behavior::Skittish,
                    "hostile" => Behavior::Hostile,
                    other => return Err(format!("unknown behaviour {other}")),
                },
                kind: if h.boss || (d.goal.kind == GoalKind::Collect && h.species == d.goal.target)
                {
                    Kind::Boss
                } else {
                    Kind::Creature
                },
                walk: 1.0,
                run: 5.8,
                elite: 0.0,
            })
        })
        .collect::<Result<_, String>>()?;
    if d.goal.kind == GoalKind::Collect && !d.roster.iter().any(|s| s.name == d.goal.target) {
        // Not a herd species: it must be the one scene prop flagged `goal`, with its axis given.
        let flagged = d
            .scenes
            .iter()
            .flat_map(|s| &s.items)
            .filter(|i| i.goal)
            .count();
        let count: usize = d
            .scenes
            .iter()
            .flat_map(|s| &s.items)
            .filter(|i| i.goal)
            .map(|i| i.count)
            .sum();
        if flagged != 1 || count != 1 || !(d.goal.axis > 0.0) {
            return Err(format!(
                "goal target {:?} is neither a herd species nor one scene item with goal = true and [goal] axis",
                d.goal.target
            ));
        }
    }
    let mut extras = Vec::new();
    for scene in &d.scenes {
        for (ix, item) in scene.items.iter().enumerate() {
            if item.species.is_empty() {
                continue;
            }
            let template = d
                .roster
                .iter()
                .find(|s| s.name == item.species)
                .ok_or_else(|| format!("unknown scene species {}", item.species))?;
            for placed in plan_item(scene, ix, item) {
                let mut spec = template.clone();
                spec.count = 1;
                spec.place = Place::Around {
                    at: units::bevy_to_wow(placed.tf.translation),
                    radius: 0.0,
                };
                extras.push(spec);
            }
        }
    }
    d.roster.extend(extras);
    Ok(d)
}

impl Design {
    /// The soft leash's centre, Bevy space.
    pub fn leash_centre(&self) -> Vec3 {
        units::wow_to_bevy(self.bounds.and_then(|b| b.centre).unwrap_or(self.start))
    }
    pub fn leash_radius(&self) -> f32 {
        self.bounds.map_or(DEFAULT_LEASH, |b| b.radius)
    }
    /// The claim zones, defaulted.
    pub fn claim_zones(&self) -> Vec<ClaimZone> {
        if self.claim.is_empty() {
            vec![ClaimZone {
                centre: self.start,
                radius: DEFAULT_CLAIM_RADIUS,
                to: None,
            }]
        } else {
            self.claim.clone()
        }
    }
    /// A `collect` goal's target: its fitted maximum axis, yards.
    pub fn goal_axis(&self) -> Option<f32> {
        (self.goal.kind == GoalKind::Collect)
            .then(|| {
                self.herds
                    .iter()
                    .find(|h| h.species == self.goal.target)
                    .map(|h| h.fit)
                    .or((self.goal.axis > 0.0).then_some(self.goal.axis))
            })
            .flatten()
    }
    /// The ball diameter at which the goal is met, or (for a creature) first possible.
    pub fn goal_diameter(&self) -> f32 {
        match self.goal.kind {
            GoalKind::Size | GoalKind::Finale => self.goal.diameter,
            GoalKind::Collect => level::edible_diameter(self.goal_axis().unwrap_or(4.0)),
        }
    }
    /// The area named by the goal.
    pub fn goal_area(&self) -> Option<&Area> {
        let name = self.goal.area.as_deref()?;
        self.areas.iter().find(|a| a.name == name)
    }
    /// Where the goal lies in Bevy space: its area's centre, else the boss's camp.
    pub fn goal_point(&self) -> Option<Vec3> {
        self.goal_area()
            .map(|a| a.centre)
            .or(self.camp)
            .map(units::wow_to_bevy)
    }
    /// The word for what the goal is: "Hogger", "1.50 yd".
    pub fn goal_name(&self) -> String {
        match self.goal.kind {
            GoalKind::Collect => self.goal.target.clone(),
            GoalKind::Size | GoalKind::Finale => format!("{:.2} yd", self.goal.diameter),
        }
    }
    /// The headline when the goal is met.
    pub fn win_title(&self) -> String {
        self.goal
            .title
            .clone()
            .unwrap_or_else(|| match self.goal.kind {
                GoalKind::Collect => format!("{}, rolled up!", self.goal.target),
                GoalKind::Size | GoalKind::Finale => {
                    format!("Reached {:.2} yd!", self.goal.diameter)
                }
            })
    }
    /// The results line when time ran out.
    pub fn fail_line(&self) -> String {
        self.goal
            .fail
            .clone()
            .unwrap_or_else(|| match self.goal.kind {
                GoalKind::Collect => format!("{} got away.", self.goal.target),
                GoalKind::Size | GoalKind::Finale => "Not big enough yet.".to_string(),
            })
    }
}
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}
/// Small deterministic generator (splitmix64): a level edit moves only what it touches.
struct Rng(u64);
impl Rng {
    fn new(seed: &str, salt: usize) -> Self {
        let mut h = 0xcbf29ce484222325_u64;
        for b in seed.bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        Rng(h ^ (salt as u64).wrapping_mul(0x9e3779b97f4a7c15))
    }
    fn bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn f(&mut self) -> f32 {
        (self.bits() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Uniform in (-1, 1).
    fn signed(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }
    fn angle(&mut self) -> f32 {
        self.f() * std::f32::consts::TAU
    }
    /// A point in the disc of radius `r`, evenly spread.
    fn disc(&mut self, r: f32) -> Vec2 {
        Vec2::from_angle(self.angle()) * r * self.f().sqrt()
    }
}

/// A prop the plan wants: the model (empty for a creature), where, and its height off the ground.
struct Placed {
    model: String,
    tf: Transform,
    lift: f32,
}

/// Picks the model for prop `i` of a group.
fn pick_model(model: &str, models: &[String], pick: Pick, i: usize, rng: &mut Rng) -> String {
    match models.len() {
        0 => model.to_owned(),
        n => match pick {
            Pick::Cycle => models[i % n].clone(),
            Pick::Random => models[rng.bits() as usize % n].clone(),
        },
    }
}

/// A prop's rotation: its yaw, a lean for things that were dropped, or on its side.
fn tumble(rng: &mut Rng, yaw: f32, tilt: f32, topple: f32) -> (Quat, bool) {
    let axis = Quat::from_rotation_y(rng.angle()) * Vec3::X;
    let toppled = rng.f() < topple;
    let lean = if toppled {
        1.25 + 0.4 * rng.f()
    } else {
        rng.f() * tilt
    };
    (
        Quat::from_rotation_y(yaw) * Quat::from_axis_angle(axis, lean),
        toppled,
    )
}

/// Spots (east/south, yards) and heights for a group's props, before the scene's own yaw.
fn spots(item: &Item, rng: &mut Rng) -> Vec<(Vec2, f32)> {
    let n = item.count;
    let nf = n as f32;
    let sp = item.spacing;
    let turn = Vec2::from_angle(item.heading);
    let mut out: Vec<(Vec2, f32)> = Vec::with_capacity(n);
    // Rejection sampling so a cluster's props do not sit inside each other.
    let free = |out: &[(Vec2, f32)], rng: &mut Rng, draw: &dyn Fn(&mut Rng) -> Vec2| {
        let mut p = draw(rng);
        for _ in 0..10 {
            if item.gap <= 0.0 || out.iter().all(|(q, _)| q.distance(p) >= item.gap) {
                break;
            }
            p = draw(rng);
        }
        p
    };
    match item.arrangement {
        Arrangement::Stairs => {
            for i in 0..n {
                out.push((
                    turn.rotate(Vec2::new(i as f32 * sp, 0.)),
                    i as f32 * item.step,
                ));
            }
        }
        Arrangement::Line => {
            for i in 0..n {
                let mut along = (i as f32 - (nf - 1.0) * 0.5) * sp;
                if item.wobble > 0.0 {
                    along += rng.signed() * sp * 0.15;
                }
                out.push((
                    turn.rotate(Vec2::new(along, rng.signed() * item.wobble)),
                    0.0,
                ));
            }
        }
        Arrangement::Grid => {
            let w = nf.sqrt().ceil();
            for i in 0..n {
                let t = i as f32;
                let p = Vec2::new(
                    (t % w - (w - 1.0) * 0.5) * sp,
                    ((t / w).floor() - (w - 1.0) * 0.5) * sp,
                );
                out.push((p + Vec2::new(rng.signed(), rng.signed()) * item.wobble, 0.0));
            }
        }
        Arrangement::Ring => {
            for i in 0..n {
                let a = (i as f32 + rng.signed() * 0.3) / nf * std::f32::consts::TAU;
                out.push((Vec2::from_angle(a) * (sp + rng.signed() * item.wobble), 0.0));
            }
        }
        Arrangement::Pile => {
            for i in 0..n {
                let t = i as f32;
                out.push((Vec2::from_angle(t * 2.399963) * t.sqrt() * sp, 0.0));
            }
        }
        Arrangement::Scatter => {
            for i in 0..n {
                let t = i as f32;
                let p = Vec2::from_angle(t * 2.399963) * t.sqrt() * sp;
                out.push((p + Vec2::new(rng.signed(), rng.signed()) * sp * 0.45, 0.0));
            }
        }
        Arrangement::Cluster => {
            for _ in 0..n {
                let straggler = rng.f() < 0.12;
                let p = free(&out, rng, &|rng: &mut Rng| {
                    let r = if straggler {
                        sp * (1.4 + 1.2 * rng.f())
                    } else {
                        sp * 1.15 * rng.f().powf(1.6)
                    };
                    Vec2::from_angle(rng.angle()) * r
                });
                out.push((p, 0.0));
            }
        }
        Arrangement::Heap => {
            let low = (nf * 0.55).ceil() as usize;
            let mid = low + (nf * 0.3).ceil() as usize;
            for i in 0..n {
                let layer = if i < low {
                    0
                } else if i < mid {
                    1
                } else {
                    2
                };
                let r = sp * (1.0 - 0.4 * layer as f32);
                let p = free(&out, rng, &|rng: &mut Rng| rng.disc(r));
                out.push((p, layer as f32 * item.step));
            }
        }
        Arrangement::TableSet => {
            let per = n.div_ceil(item.tables.max(1)).max(1);
            for i in 0..n {
                let k = (i / per) as f32 - (item.tables.max(1) as f32 - 1.0) * 0.5;
                let centre = turn.rotate(Vec2::new(k * item.table_gap, 0.0));
                let p = free(&out, rng, &|rng: &mut Rng| {
                    centre + Vec2::new(rng.signed(), rng.signed()) * sp
                });
                out.push((p, item.surface));
            }
        }
        Arrangement::Spill => {
            let mut p = Vec2::ZERO;
            let mut h = item.heading;
            for _ in 0..n {
                out.push((p, 0.0));
                h += rng.signed() * item.bend;
                let mut step = sp * (0.3 + 1.4 * rng.f());
                if rng.f() < 0.25 {
                    step *= 0.25;
                }
                let dir = Vec2::from_angle(h);
                p += dir * step + dir.perp() * rng.signed() * sp * 0.3;
            }
        }
    }
    let at = Vec2::from(item.offset);
    for (p, _) in out.iter_mut() {
        *p += at;
    }
    out
}

/// The props of one scene item, deterministic for its scene name and position in the scene.
fn plan_item(scene: &Scene, ix: usize, item: &Item) -> Vec<Placed> {
    let mut rng = Rng::new(&scene.name, ix);
    let anchor = units::wow_to_bevy(scene.anchor);
    let scene_rot = Quat::from_rotation_y(scene.yaw);
    let mut out = Vec::new();
    let spots = spots(item, &mut rng);
    if matches!(item.arrangement, Arrangement::TableSet) && !item.table.is_empty() {
        let turn = Vec2::from_angle(item.heading);
        let tables = item.tables.max(1);
        for k in 0..tables {
            let along = (k as f32 - (tables as f32 - 1.0) * 0.5) * item.table_gap;
            let p = Vec2::from(item.offset) + turn.rotate(Vec2::new(along, 0.0));
            out.push(Placed {
                model: item.table.clone(),
                tf: Transform {
                    translation: anchor + scene_rot * Vec3::new(p.x, 0.0, p.y),
                    rotation: scene_rot
                        * Quat::from_rotation_y(-item.heading + rng.signed() * 0.12),
                    scale: Vec3::splat(item.table_scale),
                },
                lift: 0.0,
            });
        }
    }
    for (i, (p, layer_lift)) in spots.into_iter().enumerate() {
        let model = pick_model(&item.model, &item.models, item.pick, i, &mut rng);
        let scale = item.scale * (1.0 + item.jitter * rng.signed());
        let yaw = item.yaw.unwrap_or_else(|| rng.angle());
        let (rot, toppled) = tumble(&mut rng, yaw, item.tilt, item.topple);
        out.push(Placed {
            model,
            tf: Transform {
                translation: anchor + scene_rot * Vec3::new(p.x, 0.0, p.y),
                rotation: scene_rot
                    * rot
                    * Quat::from_rotation_x(item.pitch)
                    * Quat::from_rotation_z(item.roll),
                scale: Vec3::splat(scale),
            },
            lift: item.lift
                + layer_lift
                + if toppled {
                    item.topple_lift * scale
                } else {
                    0.0
                },
        });
    }
    out
}

/// A trail that wanders: uneven steps, a drift off the straight line, the odd pair of drops.
fn plan_trail(t: &Trail) -> Vec<Placed> {
    if t.navigation_only {
        return Vec::new();
    }
    let a = units::wow_to_bevy(t.from);
    let b = units::wow_to_bevy(t.to);
    let line = units::flat(b - a);
    let len = line.length().max(0.01);
    let dir = line / len;
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let mut rng = Rng::new(&t.name, 0);
    let (p1, p2) = (rng.angle(), rng.angle());
    let spacing = t.spacing.max(0.5);
    let mut out = Vec::new();
    let mut s = 0.0;
    let mut i = 0;
    while s <= len {
        let drift = t.wobble * (0.6 * (s * 0.17 + p1).sin() + 0.4 * (s * 0.41 + p2).sin());
        let at = a.lerp(b, s / len) + side * (drift + rng.signed() * t.wobble * 0.2);
        let drops = if rng.f() < t.clump { 2 } else { 1 };
        for d in 0..drops {
            let scale = t.scale * (1.0 + t.jitter * rng.signed());
            let yaw = rng.angle();
            let (rot, _) = tumble(&mut rng, yaw, t.tilt, 0.0);
            let nudge = if d == 0 {
                Vec3::ZERO
            } else {
                Vec3::new(rng.signed(), 0.0, rng.signed()) * spacing * 0.3
            };
            out.push(Placed {
                model: pick_model(&t.model, &t.models, t.pick, i, &mut rng),
                tf: Transform {
                    translation: at + nudge,
                    rotation: rot,
                    scale: Vec3::splat(scale),
                },
                lift: 0.0,
            });
            i += 1;
        }
        s += spacing * (1.0 + t.variance * rng.signed());
    }
    out
}
pub(super) fn reload(
    mut d: ResMut<Design>,
    current: Res<super::campaign::CurrentLevel>,
    mut pending: ResMut<Pending>,
    assets: Res<AssetServer>,
) {
    *d = load(current.entry());
    pending.armed = true;
    pending.props.clear();
    pending.settled = false;
    pending.frames = 0;
    pending.inventory.clear();
    pending.planned.clear();
    pending.trail_ranges.clear();
    pending.lifts.clear();
    pending.blockers.clear();
    pending.report = ReportState::default();
    pending.claimed = 0;
    for (si, scene) in d.scenes.iter().enumerate() {
        let mut total = 0;
        for (ix, item) in scene.items.iter().enumerate() {
            if !item.species.is_empty() {
                continue;
            }
            let ground_at = item.level.then(|| {
                units::wow_to_bevy(scene.anchor)
                    + Quat::from_rotation_y(scene.yaw)
                        * Vec3::new(item.offset[0], 0.0, item.offset[1])
            });
            for (n, placed) in plan_item(scene, ix, item).into_iter().enumerate() {
                pending.props.push(Prop {
                    ground_at,
                    src: Authored {
                        scene: si,
                        item: ix,
                        n,
                        trail: false,
                        hint: placed.tf.translation.y,
                        ground: f32::NAN,
                        lift: placed.lift,
                    },
                    model: placed.model,
                    tf: placed.tf,
                    indoors: scene.indoors,
                    solid: item.solid
                        || item.goal
                        || item.blocker
                        || matches!(item.arrangement, Arrangement::Stairs),
                    blocker: item.blocker,
                    rooted: item.rooted,
                    goal: item.goal,
                    present: item.present.then(|| {
                        item.name
                            .clone()
                            .unwrap_or_else(|| super::replay::PARCEL_NAME.into())
                    }),
                    lift: placed.lift,
                });
                total += 1;
            }
        }
        info!("azeroll scene: {} ({total} props)", scene.name);
    }
    for (ti, trail) in d.trails.iter().enumerate() {
        let mut total = 0;
        let first = pending.props.len();
        for (n, placed) in plan_trail(trail).into_iter().enumerate() {
            pending.props.push(Prop {
                ground_at: None,
                src: Authored {
                    scene: ti,
                    item: 0,
                    n,
                    trail: true,
                    hint: placed.tf.translation.y,
                    ground: f32::NAN,
                    lift: placed.lift,
                },
                model: placed.model,
                tf: placed.tf,
                indoors: false,
                solid: false,
                blocker: false,
                rooted: false,
                goal: false,
                present: None,
                lift: placed.lift,
            });
            total += 1;
        }
        let end = pending.props.len();
        pending.trail_ranges.push(first..end);
        info!("azeroll trail: {} ({total} props)", trail.name);
    }
    pending.lifts = pending.props.iter().map(|p| p.lift).collect();
    pending.blockers = pending.props.iter().map(|p| p.blocker).collect();
    pending.planned = pending
        .props
        .iter()
        .map(|p| {
            (
                p.tf.translation,
                p.tf.scale,
                assets.load(benilla_assets::m2_url(&p.model)),
            )
        })
        .collect();
}
/// The transform and solid of one recipe barrier: an oriented box along the segment from `from` to
/// `to` (local +X along it), its base a yard under the lower end so the floor overlaps, a half yard
/// past each end for the jambs.
fn barrier_parts(b: &Barrier) -> (Transform, Collectible) {
    let (from, to) = (units::wow_to_bevy(b.from), units::wow_to_bevy(b.to));
    let along = Vec2::new(to.x - from.x, to.z - from.z);
    let dir = along.normalize_or(Vec2::X);
    // rotation_y(yaw) takes X to (cos yaw, 0, -sin yaw).
    let yaw = (-dir.y).atan2(dir.x);
    let base = from.y.min(to.y) - 1.0;
    let half = Vec3::new(
        along.length() * 0.5 + b.extend,
        b.height * 0.5,
        b.thickness * 0.5,
    );
    let centre = Vec3::new((from.x + to.x) * 0.5, base + half.y, (from.z + to.z) * 0.5);
    (
        Transform::from_translation(centre).with_rotation(Quat::from_rotation_y(yaw)),
        Collectible {
            name: "Invisible wall".into(),
            kind: Kind::Prop,
            source: Source::Spawned,
            size: half.length(),
            pickup: BARRIER_AXIS,
            half_extents: half,
            center: Vec3::ZERO,
            solid: 1.0,
        },
    )
}

/// One invisible box per recipe barrier, owned by the run (despawned on the next reset).
fn spawn_barriers(mut commands: Commands, d: Res<Design>) {
    for (i, b) in d.barriers.iter().enumerate() {
        let (tf, thing) = barrier_parts(b);
        info!(
            "azeroll barrier {i}: {:?} -> {:?} half={:?} base={:.2}",
            b.from,
            b.to,
            thing.half_extents,
            tf.translation.y - thing.half_extents.y
        );
        commands.spawn((tf, Visibility::Hidden, thing, Gate, BarrierWall, RunEntity));
    }
}

fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    world: WorldPoint,
    progress: Res<benilla_world::terrain_stream::WorldLoadProgress>,
    collision: WorldCollision,
    mut pending: ResMut<Pending>,
    design: Res<Design>,
) {
    if !progress.complete {
        return;
    }
    let goal_name = design.goal.target.clone();
    let mut budget = 24;
    pending.props.retain(|prop| {
        if budget == 0 {
            return true;
        }
        let tf = &prop.tf;
        let at = prop
            .ground_at
            .map_or(tf.translation, |g| Vec3::new(g.x, tf.translation.y, g.z));
        let ground = if prop.indoors {
            collision
                .ray_body(at + Vec3::Y * 4.0, Dir3::NEG_Y, 8.0)
                .map(|h| at.y + 4.0 - h.distance)
        } else {
            world.terrain_height_under(at)
        };
        let Some(y) = ground else {
            return true;
        };
        let mut tf = *tf;
        tf.translation.y = y + prop.lift;
        let e = super::props::spawn_copy(&mut commands, &assets, &prop.model, tf, None);
        commands.entity(e).insert((
            RunEntity,
            Designed,
            Authored {
                ground: y,
                ..prop.src
            },
        ));
        if prop.solid {
            commands.entity(e).insert(Gate);
        }
        if prop.blocker {
            commands.entity(e).insert(Blocker);
        }
        if prop.rooted {
            commands.entity(e).insert(super::collect::Planted);
        }
        if prop.goal {
            commands.entity(e).insert(GoalProp(goal_name.clone()));
        }
        if let Some(name) = &prop.present {
            commands
                .entity(e)
                .insert((Present(name.clone()), super::replay::HumanOnly));
        }
        budget -= 1;
        false
    });
}
/// Seat a small loose recipe prop whose model hangs below its origin (a helm's origin is the head
/// it was worn on, a half loaf's its middle) on the floor it was spawned on, once its box is known.
/// Solids, ramps, planted props and big models keep their authored heights: their geometry is
/// placed by the recipe on purpose.
fn seat(
    mut fresh: Query<
        (&mut Transform, &Collectible),
        (
            With<Designed>,
            Added<Collectible>,
            Without<Gate>,
            Without<super::collect::Planted>,
        ),
    >,
) {
    for (mut tf, c) in &mut fresh {
        let half = c.half_extents * tf.scale.abs();
        let centre = tf.rotation * (c.center * tf.scale);
        let reach: f32 = (0..3)
            .map(|i| {
                let mut a = Vec3::ZERO;
                a[i] = half[i];
                (tf.rotation * a).y.abs()
            })
            .sum();
        let (below, height) = (-(centre.y - reach), 2.0 * reach);
        if height < 2.0 && below > (0.15 * height).max(0.02) {
            tf.translation.y += below;
        }
    }
}
/// Conservative max-axis cube bound for rooms whose exact object boxes are unavailable.
/// Volume addition is order independent.
#[cfg(test)]
pub(super) fn exit_bound(entry: f32, sizes: &[f32]) -> f32 {
    (entry.powi(3)
        + sizes
            .iter()
            .map(|s| super::collect::growth(*s, entry))
            .sum::<f32>())
    .cbrt()
}
/// What the level holds, as `(position, pickup axis, credited volume)`.
struct Inventory {
    /// Everything, trails included: what the per-area band check counts.
    all: Vec<(Vec3, f32, f32)>,
    /// Everything but the trails' props.
    loose: Vec<(Vec3, Food)>,
    /// Each trail's props in walking order from its `from` end.
    trails: Vec<Vec<Food>>,
    /// Planned props whose model bounds have not loaded yet.
    missing: usize,
    /// Creatures counted from the roster by the max-axis cube bound, not by a spawned box.
    estimated: usize,
}

/// A prop's pickup axis and credited volume from its model bounds, by the live law
/// ([`Collectible::volume`]) so the report cannot drift from what the ball is credited.
fn planned_food(bbox_min: [f32; 3], bbox_max: [f32; 3], scale: Vec3, rise: f32) -> Food {
    let half = units::wow_to_bevy(std::array::from_fn(|i| {
        (bbox_max[i] - bbox_min[i]).abs() * 0.5
    }))
    .abs();
    let probe = Collectible {
        name: "".into(),
        kind: Kind::Prop,
        source: Source::Spawned,
        size: 0.0,
        pickup: 0.0,
        half_extents: half,
        center: Vec3::ZERO,
        solid: 0.0,
    };
    Food {
        pickup: (2.0 * half * scale.abs()).max_element(),
        volume: probe.volume(scale),
        rise,
    }
}

fn inventory(
    pending: &Pending,
    d: &Design,
    models: &Assets<benilla_assets::M2Model>,
    ground: &dyn Fn(Vec3) -> Option<f32>,
) -> Inventory {
    let mut loose: Vec<(Vec3, Food)> = pending
        .inventory
        .values()
        .map(|&(pos, pickup, volume)| {
            let rise = ground(pos).map_or(0.0, |g| (pos.y - g).max(0.0));
            (
                pos,
                Food {
                    pickup,
                    volume,
                    rise,
                },
            )
        })
        .collect();
    let mut trails = vec![Vec::new(); d.trails.len()];
    let mut missing = 0;
    for (i, (pos, scale, handle)) in pending.planned.iter().enumerate() {
        if pending.blockers.get(i).copied().unwrap_or(false) {
            continue;
        }
        let Some(bounds) = models.get(handle).and_then(|m| m.bounds) else {
            missing += 1;
            continue;
        };
        let rise = pending.lifts.get(i).copied().unwrap_or(0.0);
        let food = planned_food(bounds.bbox_min, bounds.bbox_max, *scale, rise);
        match pending.trail_ranges.iter().position(|r| r.contains(&i)) {
            Some(t) => trails[t].push((*pos, food)),
            None => loose.push((*pos, food)),
        }
    }
    let mut estimated = 0;
    for spec in &d.roster {
        let Place::Around { at, .. } = spec.place;
        for _ in 0..spec.count {
            let axis = spec.fit.unwrap_or(0.0);
            loose.push((
                units::wow_to_bevy(at),
                Food {
                    pickup: axis,
                    volume: super::collect::growth(axis, 0.0),
                    rise: 0.0,
                },
            ));
            estimated += 1;
        }
    }
    let mut all: Vec<(Vec3, f32, f32)> = loose
        .iter()
        .map(|(p, f)| (*p, f.pickup, f.volume))
        .collect();
    all.extend(
        trails
            .iter()
            .flatten()
            .map(|(p, f)| (*p, f.pickup, f.volume)),
    );
    Inventory {
        all,
        loose,
        trails: trails
            .into_iter()
            .map(|t| t.into_iter().map(|(_, f)| f).collect())
            .collect(),
        missing,
        estimated,
    }
}
type InventoryQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Collectible,
        Has<super::npcs::Npc>,
    ),
    Without<Attached>,
>;

/// Remember each loose thing's first-seen position, pickup axis and credited volume. Designed
/// props and creatures are planned or rostered instead, so only claimed doodads and NPCs go in.
fn remember(pending: &mut Pending, q: &InventoryQuery) {
    let masks: Vec<_> = q
        .iter()
        .filter(|(_, _, c, _)| !c.pickup.is_finite() && c.solid > 0.)
        .map(|(_, tf, c, _)| (tf, c))
        .collect();
    for (e, tf, c, npc) in q {
        if masks
            .iter()
            .any(|(bt, bc)| super::collect::hidden_by_blocker(tf, c, bt, bc))
        {
            // Blockers may finish loading after a claimed doodad was first seen.
            pending.inventory.remove(&e);
            continue;
        }
        if c.source == Source::Spawned && !npc {
            continue;
        }
        if pending
            .inventory
            .insert(e, (tf.translation, c.pickup, c.volume(tf.scale)))
            .is_none()
            && matches!(c.source, Source::Doodad(_))
        {
            pending.claimed += 1;
        }
    }
}

fn validate(
    d: Res<Design>,
    mut pending: ResMut<Pending>,
    q: InventoryQuery,
    unresolved: Query<(), (With<Designed>, Without<Collectible>)>,
    models: Res<Assets<benilla_assets::M2Model>>,
    npcs: Res<super::npcs::Pending>,
) {
    if !pending.armed || pending.settled {
        return;
    }
    remember(&mut pending, &q);
    pending.frames += 1;
    if !pending.frames.is_multiple_of(300) {
        return;
    }
    let Inventory {
        all: inventory,
        missing,
        ..
    } = inventory(&pending, &d, &models, &|_| None);
    let bands =
        [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, f32::INFINITY].map(|r| r * level::COLLECT_RATIO);
    for area in &d.areas {
        let centre = units::wow_to_bevy(area.centre);
        let sizes: Vec<f32> = inventory
            .iter()
            .filter(|(pos, _, _)| {
                units::flat(*pos - centre).length() <= area.radius
                    && (!area.enclosed || (pos.y - centre.y).abs() < 3.0)
            })
            .map(|(_, size, _)| *size)
            .collect();
        let counts: Vec<usize> = bands
            .windows(2)
            .map(|b| sizes.iter().filter(|s| **s >= b[0] && **s < b[1]).count())
            .collect();
        info!(
            "azeroll VALIDATE {} bands={counts:?} pending={} unresolved={}",
            area.name,
            missing,
            unresolved.iter().count()
        );
        if area.enclosed {
            // Exact bbox volumes, independent of pickup order, including every object in
            // the room even if it cannot be eaten yet. The max-axis cube is needlessly loose
            // for long furniture and used only when creature bounds are unavailable.
            let credit: f32 = inventory
                .iter()
                .filter(|(pos, _, _)| {
                    units::flat(*pos - centre).length() <= area.radius
                        && (pos.y - centre.y).abs() < 3.0
                })
                .map(|(_, _, v)| *v)
                .sum();
            let bound = (area.clearance.powi(3) + credit).cbrt();
            // Footing keeps the centre one full radius above the floor, while the
            // swept sphere extends 0.85r upward. Height therefore needs 1.85r.
            let fits = 1.85 * bound <= 2.0 * area.clearance;
            if fits {
                info!(
                    "azeroll D10 PASS {} entry={} upper={} exit={} doors={}",
                    area.name,
                    area.clearance,
                    bound,
                    2.0 * area.clearance / 1.85,
                    area.doors.len()
                );
            } else {
                error!(
                    "azeroll D10 FAIL {} upper={} exit={}",
                    area.name,
                    bound,
                    2.0 * area.clearance / 1.85
                );
            }
        }
    }
    let sizes: Vec<f32> = inventory.iter().map(|(_, size, _)| *size).collect();
    for b in bands.windows(2) {
        if !sizes.iter().any(|s| *s >= b[0] && *s < b[1]) {
            error!("azeroll TIER GAP {}..{} (loaded coverage only)", b[0], b[1]);
        }
    }
    pending.settled = missing == 0 && unresolved.is_empty() && super::npcs::all_spawned(npcs);
}

/// Once the level has settled: write the per-segment volume table and the greedy growth curves
/// ([`progression`]). Claimed doodads stream in with the terrain, so far areas fill in later and
/// the report is redone (at most three more times) when the inventory has grown noticeably.
/// At most this many distinct ramp routes are searched per report.
const RAMP_ROUTES: usize = 48;

fn report(
    mut pending: ResMut<Pending>,
    d: Res<Design>,
    q: InventoryQuery,
    models: Res<Assets<benilla_assets::M2Model>>,
    world: WorldPoint,
    collision: WorldCollision,
    solids: Query<
        (&Transform, &Collectible, Has<Gate>),
        (Without<Attached>, Without<super::collect::Shed>),
    >,
) {
    if !pending.armed || !pending.settled {
        return;
    }
    pending.report.frames += 1;
    if pending.report.emitted > 0 && !pending.report.frames.is_multiple_of(600) {
        return;
    }
    remember(&mut pending, &q);
    let seen = pending.inventory.len();
    let grown = seen >= pending.report.seen + (pending.report.seen / 20).max(100);
    if pending.report.emitted > 0 && (!grown || pending.report.emitted >= 4) {
        return;
    }
    let mut inv = inventory(&pending, &d, &models, &|p| world.terrain_height_under(p));
    // Ramp accounting runs a body-swept A* per raised food position: hundreds of searches on an
    // indoor level, a multi-second stall (or minutes) before the first frame. Players never need
    // it, so only a requested report (`WOW_AZEROLL_REPORT`) pays for it, and even then a bounded
    // number of distinct routes is searched; the rest keep their raised `rise`.
    if d.start_indoors && crate::webenv::var("WOW_AZEROLL_REPORT").is_some() {
        let solids: Vec<_> = solids
            .iter()
            .map(|(tf, c, g)| (*tf, c.clone(), g))
            .collect();
        // Bound telemetry work by quantizing equal food positions/size bands.
        let mut routes = std::collections::HashMap::new();
        let mut account_ramp = |(pos, food): &mut (Vec3, Food)| {
            if food.rise <= 0.2 || !food.pickup.is_finite() {
                return;
            }
            let r = (food.pickup / super::level::COLLECT_RATIO + 0.001).max(d.start_radius);
            let size = food.pickup * 0.5;
            let target = *pos + Vec3::Y * (food.rise + size);
            let key = (
                (target.x * 2.) as i32,
                (target.z * 2.) as i32,
                (target.y * 2.) as i32,
                (r * 10.) as i32,
            );
            if !routes.contains_key(&key) && routes.len() >= RAMP_ROUTES {
                return;
            }
            let reachable = *routes.entry(key).or_insert_with(|| {
                super::auto::ramp_reachable(&collision, &d, target, r, size, &solids)
            });
            if reachable {
                food.rise = 0.;
            }
        };
        inv.loose.iter_mut().for_each(&mut account_ramp);
    }
    let text = progression::render(&d, &inv, pending.claimed);
    pending.report.emitted += 1;
    pending.report.seen = seen;
    progression::emit(&text);
}
/// One thing the ball could eat: the maximum axis it must clear and the volume it credits.
#[derive(Clone, Copy, Debug)]
struct Food {
    pickup: f32,
    volume: f32,
    /// Height of the thing's base above the ground, yards: a plate on a table is not on the floor.
    rise: f32,
}

pub(in crate::azeroll) use progression::{
    area_at, build_id, plan_route, segment_offset, trail_ends, utc_now, RouteKind, Step,
};

/// Routes through the level, and what is on them: the autopilot's progression and the per-segment
/// volume report. Everything about contact and growth is asked of the live rules
/// ([`super::super::collect::contact`], [`Collectible::volume`]), never restated.
mod progression {
    use super::super::ball::top_speed;
    use super::super::collect::{contact, Contact};
    use super::*;
    use std::fmt::Write as _;

    // -----------------------------------------------------------------------------------------
    // Areas and routes
    // -----------------------------------------------------------------------------------------

    /// The area a Bevy-space point is inside (nearest by fraction of radius when they overlap);
    /// an enclosed area also needs the point within three yards of its floor.
    pub(in crate::azeroll) fn area_at(d: &Design, p: Vec3) -> Option<usize> {
        d.areas
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let c = units::wow_to_bevy(a.centre);
                if (a.enclosed || d.start_indoors) && (p.y - c.y).abs() >= 3.0 {
                    return None;
                }
                let f = units::flat(p - c).length() / a.radius.max(0.1);
                (f <= 1.0).then_some((i, f))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn nearest_area(d: &Design, p: Vec3) -> Option<usize> {
        d.areas
            .iter()
            .enumerate()
            .filter(|(_, a)| !a.enclosed)
            .min_by(|a, b| {
                let f = |x: &Area| {
                    units::flat(p - units::wow_to_bevy(x.centre)).length() / x.radius.max(0.1)
                };
                f(a.1).total_cmp(&f(b.1))
            })
            .map(|(i, _)| i)
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::azeroll) enum RouteKind {
        /// The shortest trail walk from the start area to the camp.
        Primary,
        /// Every open area on the trail graph, then the camp.
        Tour,
    }

    /// One leg of a route: an area to clear, or a trail to walk (`reversed`: from its `to` end).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::azeroll) enum Step {
        Area(usize),
        Trail { index: usize, reversed: bool },
    }

    /// A trail's two ends in Bevy space, in walking order.
    pub(in crate::azeroll) fn trail_ends(d: &Design, index: usize, reversed: bool) -> (Vec3, Vec3) {
        let t = &d.trails[index];
        let (a, b) = (units::wow_to_bevy(t.from), units::wow_to_bevy(t.to));
        if reversed {
            (b, a)
        } else {
            (a, b)
        }
    }

    /// Where `p` sits relative to the flat segment `a`..`b`: `(distance off the line, yards along
    /// it from `a`)`. The along value is not clamped, so callers can tell before from beyond.
    pub(in crate::azeroll) fn segment_offset(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
        let line = units::flat(b - a);
        let len = line.length().max(0.001);
        let dir = line / len;
        let rel = units::flat(p - a);
        let along = rel.dot(dir);
        ((rel - dir * along).length(), along)
    }

    struct Edge {
        a: usize,
        b: usize,
        trail: usize,
        len: f32,
    }

    /// The trails as a graph. Nodes `0..areas` are the areas (so a node index is an area index);
    /// further nodes are trail ends that lie in no open area, merged when within twelve yards.
    struct Graph {
        pts: Vec<Vec3>,
        edges: Vec<Edge>,
        areas: usize,
    }

    impl Graph {
        fn new(d: &Design) -> Self {
            let mut pts: Vec<Vec3> = d
                .areas
                .iter()
                .map(|a| units::wow_to_bevy(a.centre))
                .collect();
            let areas = pts.len();
            let mut edges = Vec::new();
            let node = |pts: &mut Vec<Vec3>, p: Vec3| -> usize {
                // In an indoor recipe, enclosed rooms ARE destinations. The
                // outdoor graph excludes them to avoid routing into tiny houses.
                if d.start_indoors {
                    if let Some(i) = area_at(d, p) {
                        return i;
                    }
                }

                let inside = d
                    .areas
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| !a.enclosed)
                    .map(|(i, a)| {
                        let f = units::flat(p - units::wow_to_bevy(a.centre)).length()
                            / a.radius.max(0.1);
                        (i, f)
                    })
                    .filter(|(_, f)| *f <= 1.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((i, _)) = inside {
                    return i;
                }
                if let Some(i) = (areas..pts.len()).find(|&i| {
                    units::flat(pts[i] - p).length() < 12.0
                        && (!d.start_indoors || (pts[i].y - p.y).abs() < 0.75)
                }) {
                    return i;
                }
                pts.push(p);
                pts.len() - 1
            };
            for (trail, t) in d.trails.iter().enumerate() {
                let (from, to) = (units::wow_to_bevy(t.from), units::wow_to_bevy(t.to));
                let (a, b) = (node(&mut pts, from), node(&mut pts, to));
                if a != b {
                    edges.push(Edge {
                        a,
                        b,
                        trail,
                        len: units::flat(to - from).length(),
                    });
                }
            }
            Self { pts, edges, areas }
        }

        /// The shortest walk: its length and `(edge, travelled a→b)` hops.
        fn walk(&self, from: usize, to: usize) -> Option<(f32, Vec<(usize, bool)>)> {
            let n = self.pts.len();
            let mut dist = vec![f32::INFINITY; n];
            let mut via: Vec<Option<(usize, usize, bool)>> = vec![None; n];
            let mut done = vec![false; n];
            dist[from] = 0.0;
            while let Some(u) = (0..n)
                .filter(|&i| !done[i] && dist[i].is_finite())
                .min_by(|&a, &b| dist[a].total_cmp(&dist[b]))
            {
                done[u] = true;
                for (ei, e) in self.edges.iter().enumerate() {
                    let (v, fwd) = if e.a == u {
                        (e.b, true)
                    } else if e.b == u {
                        (e.a, false)
                    } else {
                        continue;
                    };
                    if dist[u] + e.len < dist[v] {
                        dist[v] = dist[u] + e.len;
                        via[v] = Some((u, ei, fwd));
                    }
                }
            }
            if !dist[to].is_finite() {
                return None;
            }
            let (mut hops, mut at) = (Vec::new(), to);
            while at != from {
                let (u, ei, fwd) = via[at]?;
                hops.push((ei, fwd));
                at = u;
            }
            hops.reverse();
            Some((dist[to], hops))
        }

        /// The steps of a walk: each trail, and each open area passed through or reached.
        fn steps(&self, from: usize, hops: &[(usize, bool)]) -> Vec<Step> {
            let mut out = Vec::new();
            if from < self.areas {
                out.push(Step::Area(from));
            }
            for &(ei, fwd) in hops {
                let e = &self.edges[ei];
                out.push(Step::Trail {
                    index: e.trail,
                    reversed: !fwd,
                });
                let at = if fwd { e.b } else { e.a };
                if at < self.areas {
                    out.push(Step::Area(at));
                }
            }
            out
        }
    }

    /// The cheapest order for visiting `optional` between `start` and `goal` (exact up to eight).
    fn best_order(g: &Graph, start: usize, goal: usize, optional: &[usize]) -> Vec<usize> {
        let cost = |a: usize, b: usize| g.walk(a, b).map_or(f32::INFINITY, |w| w.0);
        fn go(
            cost: &dyn Fn(usize, usize) -> f32,
            at: usize,
            goal: usize,
            left: &mut Vec<usize>,
            path: &mut Vec<usize>,
            sum: f32,
            best: &mut (f32, Vec<usize>),
        ) {
            if sum >= best.0 {
                return;
            }
            if left.is_empty() {
                let total = sum + cost(at, goal);
                if total < best.0 {
                    *best = (total, path.clone());
                }
                return;
            }
            for i in 0..left.len() {
                let next = left.remove(i);
                path.push(next);
                go(cost, next, goal, left, path, sum + cost(at, next), best);
                path.pop();
                left.insert(i, next);
            }
        }
        if optional.len() <= 8 {
            let mut best = (f32::INFINITY, optional.to_vec());
            go(
                &cost,
                start,
                goal,
                &mut optional.to_vec(),
                &mut Vec::new(),
                0.0,
                &mut best,
            );
            best.1
        } else {
            let (mut at, mut left, mut out) = (start, optional.to_vec(), Vec::new());
            while !left.is_empty() {
                let i = (0..left.len())
                    .min_by(|&a, &b| cost(at, left[a]).total_cmp(&cost(at, left[b])))
                    .unwrap_or(0);
                at = left.remove(i);
                out.push(at);
            }
            out
        }
    }

    /// The legs the autopilot walks, and the report's greedy simulation follows. Empty when the
    /// recipe has no area to start in. Indoor recipes also route through enclosed rooms.
    pub(in crate::azeroll) fn plan_route(d: &Design, kind: RouteKind) -> Vec<Step> {
        let g = Graph::new(d);
        let here = units::wow_to_bevy(d.start);
        let Some(start) = area_at(d, here).or_else(|| nearest_area(d, here)) else {
            return Vec::new();
        };
        let goal = d
            .goal_point()
            .and_then(|at| area_at(d, at).or_else(|| nearest_area(d, at)))
            .unwrap_or(start);
        let mut stops = vec![start];
        if kind == RouteKind::Tour {
            let optional: Vec<usize> = (0..g.areas)
                .filter(|&i| {
                    i != start
                        && i != goal
                        && (!d.areas[i].enclosed || d.start_indoors)
                        && g.walk(start, i).is_some()
                        && g.walk(i, goal).is_some()
                })
                .collect();
            if d.start_indoors {
                // Visit the remote storeys while the ball still fits their
                // stairs and doors; abundant main-floor food comes afterwards.
                let (remote, local): (Vec<_>, Vec<_>) = optional
                    .into_iter()
                    .partition(|&i| (d.areas[i].centre[2] - d.areas[start].centre[2]).abs() > 3.);
                let remote = best_order(&g, start, goal, &remote);
                let last = remote.last().copied().unwrap_or(start);
                stops.extend(remote);
                stops.extend(best_order(&g, last, goal, &local));
            } else {
                stops.extend(best_order(&g, start, goal, &optional));
            }
        }
        stops.push(goal);
        let mut steps: Vec<Step> = Vec::new();
        for pair in stops.windows(2) {
            let Some((_, hops)) = g.walk(pair[0], pair[1]) else {
                continue;
            };
            for s in g.steps(pair[0], &hops) {
                // Crossing a room on the way to a tour stop is navigation,
                // not an invitation to clear it before reaching that stop.
                if d.start_indoors
                    && kind == RouteKind::Tour
                    && !steps.is_empty()
                    && matches!(s, Step::Area(i) if i != pair[1])
                {
                    continue;
                }
                if steps.last() != Some(&s) {
                    steps.push(s);
                }
            }
        }
        if d.start_indoors
            && kind == RouteKind::Tour
            && steps.iter().any(|s| matches!(s, Step::Trail { .. }))
        {
            // Tour explores the remote rooms before clearing the nursery/goal
            // rooms: clearing either first can finish the size goal in the pit.
            let last_goal = steps.iter().rposition(|s| *s == Step::Area(goal));
            steps = steps
                .into_iter()
                .enumerate()
                .filter(|(i, s)| {
                    (*s != Step::Area(start) || *i == 0)
                        && (*s != Step::Area(goal) || Some(*i) == last_goal)
                })
                .map(|(_, s)| s)
                .collect();
        }
        if steps.is_empty() {
            steps.push(Step::Area(start));
        }
        steps
    }

    impl Step {
        pub(in crate::azeroll) fn label(&self, d: &Design) -> String {
            match *self {
                Step::Area(i) => d.areas[i].name.clone(),
                Step::Trail { index, reversed } => format!(
                    "{}{}",
                    d.trails[index].name,
                    if reversed { " (back)" } else { "" }
                ),
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Build identity
    // -----------------------------------------------------------------------------------------

    /// `YYYY-MM-DD HH:MM:SSZ` for seconds since the Unix epoch.
    fn utc(secs: u64) -> String {
        let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
    }

    pub(in crate::azeroll) fn utc_now() -> String {
        utc(web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()))
    }

    /// The running binary's path, size and modification time: what ties a report to a build.
    pub(in crate::azeroll) fn build_id() -> String {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Ok(exe) = std::env::current_exe() else {
                return "unknown binary".into();
            };
            let Ok(meta) = std::fs::metadata(&exe) else {
                return format!("{} (unreadable)", exe.display());
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or_else(|| "?".into(), |d| utc(d.as_secs()));
            format!("{} {} bytes, mtime {mtime}", exe.display(), meta.len())
        }
        #[cfg(target_arch = "wasm32")]
        {
            "wasm build".into()
        }
    }

    // -----------------------------------------------------------------------------------------
    // The volume report
    // -----------------------------------------------------------------------------------------

    /// Ball radii that separate the report's size bands, by the radius needed to roll a thing up.
    const EDGES: [f32; 6] = [0.5, 1.0, 1.5, 2.5, 4.0, 6.0];
    const BANDS: [&str; 7] = ["<0.5", "0.5-1", "1-1.5", "1.5-2.5", "2.5-4", "4-6", "6-12"];
    /// A thing that needs a ball this big is scenery for pacing: three times what Hogger asks.
    /// It is counted, not summed (one house-sized box outweighs everything the player eats).
    const SCENERY_NEED: f32 = 12.0;

    /// The smallest ball radius that rolls up a thing with this maximum axis, by the live rule.
    fn need(pickup: f32) -> f32 {
        let (mut lo, mut hi) = (0.0_f32, 64.0_f32);
        if contact(hi, pickup) != Contact::RollUp {
            return f32::INFINITY;
        }
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if contact(mid, pickup) == Contact::RollUp {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// The smallest ball radius that gets at a thing: big enough to roll it up (the live rule) and
    /// to reach it. A ball can touch what sits up to about 1.5 radii (plus the thing's own reach)
    /// above the ground, and has no way up a table.
    fn food_need(f: &Food) -> f32 {
        need(f.pickup).max((f.rise - 0.14 * f.pickup) / 1.5)
    }

    fn band(need: f32) -> usize {
        EDGES.iter().position(|e| need < *e).unwrap_or(EDGES.len())
    }

    /// The level sorted into the places a route can pass through.
    struct Level {
        areas: Vec<Vec<Food>>,
        /// Each trail's props in walking order from its `from` end.
        trails: Vec<Vec<Food>>,
        /// Not inside any open or enclosed area (farms between the areas, forest).
        elsewhere: Vec<Food>,
        /// How many things per area, trail and elsewhere (last) are scenery ([`SCENERY_NEED`]).
        scenery: Vec<usize>,
    }

    impl Level {
        /// Drop the scenery from `foods`, returning how much there was.
        fn split(foods: Vec<Food>) -> (Vec<Food>, usize) {
            let (keep, scenery): (Vec<Food>, Vec<Food>) =
                foods.into_iter().partition(|f| food_need(f) < SCENERY_NEED);
            (keep, scenery.len())
        }
    }

    fn level(d: &Design, inv: &Inventory) -> Level {
        let mut areas = vec![Vec::new(); d.areas.len()];
        let mut elsewhere = Vec::new();
        for &(pos, food) in &inv.loose {
            match area_at(d, pos) {
                Some(i) => areas[i].push(food),
                None => elsewhere.push(food),
            }
        }
        let mut scenery = Vec::new();
        let mut cut = |foods: Vec<Food>| {
            let (keep, n) = Level::split(foods);
            scenery.push(n);
            keep
        };
        let areas: Vec<_> = areas.into_iter().map(&mut cut).collect();
        let trails: Vec<_> = inv.trails.iter().cloned().map(&mut cut).collect();
        let elsewhere = cut(elsewhere);
        Level {
            areas,
            trails,
            elsewhere,
            scenery,
        }
    }

    fn cells(foods: &[Food]) -> ([usize; 7], [f32; 7]) {
        let (mut n, mut v) = ([0; 7], [0.0; 7]);
        for f in foods {
            let b = band(food_need(f));
            n[b] += 1;
            v[b] += f.volume;
        }
        (n, v)
    }

    struct Row {
        label: String,
        arrive_r: f32,
        avail: (usize, f32),
        ate: (usize, f32),
        exit_r: f32,
        left: (usize, f32),
        /// The smallest ball radius that would eat something still left here.
        next_need: f32,
        /// Cumulative pure travel time to the end of this step, seconds.
        clock: f32,
        flag: &'static str,
    }

    /// If a player ate everything edible, step by step, what radius do they carry out of each?
    /// An area is cleared smallest first (it can be roamed in any order); a trail is walked once,
    /// taking what the ball can take as it passes. The clock counts travel only.
    fn simulate(d: &Design, lv: &Level, steps: &[Step]) -> Vec<Row> {
        let mut volume = d.start_radius.powi(3);
        let mut eaten_area: Vec<Vec<bool>> =
            lv.areas.iter().map(|a| vec![false; a.len()]).collect();
        let mut eaten_trail: Vec<Vec<bool>> =
            lv.trails.iter().map(|a| vec![false; a.len()]).collect();
        let mut at = units::wow_to_bevy(d.start);
        let mut clock = 0.0;
        let mut rows = Vec::new();
        for &step in steps {
            let arrive_r = volume.cbrt();
            let edible = |f: &Food, r: f32| r >= food_need(f);
            let (label, foods, eaten, order, reach, travel): (_, &[Food], &mut Vec<bool>, _, _, _) =
                match step {
                    Step::Area(i) => {
                        let centre = units::wow_to_bevy(d.areas[i].centre);
                        let mut order: Vec<usize> = (0..lv.areas[i].len()).collect();
                        order.sort_by(|a, b| {
                            lv.areas[i][*a].pickup.total_cmp(&lv.areas[i][*b].pickup)
                        });
                        let travel = units::flat(centre - at).length();
                        at = centre;
                        (
                            step.label(d),
                            &lv.areas[i][..],
                            &mut eaten_area[i],
                            order,
                            true,
                            travel,
                        )
                    }
                    Step::Trail { index, reversed } => {
                        let (from, to) = trail_ends(d, index, reversed);
                        let mut order: Vec<usize> = (0..lv.trails[index].len()).collect();
                        if reversed {
                            order.reverse();
                        }
                        let travel =
                            units::flat(from - at).length() + units::flat(to - from).length();
                        at = to;
                        (
                            step.label(d),
                            &lv.trails[index][..],
                            &mut eaten_trail[index],
                            order,
                            false,
                            travel,
                        )
                    }
                };
            clock += travel / top_speed(arrive_r).max(0.5);
            let (mut avail, mut ate) = ((0, 0.0), (0, 0.0));
            for (i, f) in foods.iter().enumerate() {
                if !eaten[i] && edible(f, arrive_r) {
                    avail = (avail.0 + 1, avail.1 + f.volume);
                }
            }
            for i in order {
                if eaten[i] {
                    continue;
                }
                if edible(&foods[i], volume.cbrt()) {
                    eaten[i] = true;
                    volume += foods[i].volume;
                    ate = (ate.0 + 1, ate.1 + foods[i].volume);
                } else if reach {
                    // Smallest first: nothing later in an area is any smaller.
                    break;
                }
            }
            let mut left = (0, 0.0);
            let mut next_need = f32::INFINITY;
            for (i, f) in foods.iter().enumerate() {
                if !eaten[i] {
                    left = (left.0 + 1, left.1 + f.volume);
                    next_need = next_need.min(food_need(f));
                }
            }
            let v_in = arrive_r.powi(3);
            let flag = if foods.is_empty() {
                "empty"
            } else if avail.1 < 0.05 * v_in && left.0 > 0 {
                "STARVE"
            } else if avail.1 < 0.25 * v_in && left.0 > 0 {
                "thin"
            } else {
                ""
            };
            rows.push(Row {
                label,
                arrive_r,
                avail,
                ate,
                exit_r: volume.cbrt(),
                left,
                next_need,
                clock,
                flag,
            });
        }
        rows
    }

    /// The whole report as text: the segment-by-band table, then a greedy curve per route.
    pub(super) fn render(d: &Design, inv: &Inventory, claimed: usize) -> String {
        let lv = level(d, inv);
        let mut out = String::new();
        let _ = writeln!(
            out,
            "== azeroll level report: recipe fnv={:016x} start_r={} limit={}s goal={} ==",
            d.recipe_hash,
            d.start_radius,
            d.time_limit,
            d.goal_name()
        );
        let _ = writeln!(out, "build: {}   report time: {}", build_id(), utc_now());
        let _ = writeln!(
            out,
            "items: {} ({} claimed map doodads streamed so far, {} planned props, {} creatures by cube bound, {} planned props unresolved)",
            inv.all.len(),
            claimed,
            inv.all.len() - claimed.min(inv.all.len()) - inv.estimated,
            inv.estimated,
            inv.missing
        );
        let _ = writeln!(
            out,
            "Cells are count/volume. Volume is in ball volume units (a ball of radius r holds r^3), by the live additive box law."
        );
        let _ = writeln!(
            out,
            "Bands are the ball radius needed to roll a thing up (collect::contact), not its size. \
             Scenery (needs r >= {SCENERY_NEED}) is counted, not summed, and never 'left'.\n"
        );
        let _ = write!(out, "{:<28} {:>6} {:>7}", "segment", "kind", "items");
        for b in BANDS {
            let _ = write!(out, " {b:>10}");
        }
        let _ = writeln!(out, " {:>8} {:>8}", "total", "scenery");
        let mut line = |name: &str, kind: &str, foods: &[Food], scenery: usize| {
            let (n, v) = cells(foods);
            let _ = write!(out, "{name:<28.28} {kind:>6} {:>7}", foods.len());
            for i in 0..7 {
                let cell = if n[i] == 0 {
                    "-".to_string()
                } else {
                    format!("{}/{:.2}", n[i], v[i])
                };
                let _ = write!(out, " {cell:>10}");
            }
            let _ = writeln!(out, " {:>8.2} {:>8}", v.iter().sum::<f32>(), scenery);
        };
        let (na, nt) = (d.areas.len(), d.trails.len());
        for (i, a) in d.areas.iter().enumerate() {
            let kind = if a.enclosed { "indoor" } else { "area" };
            line(&a.name, kind, &lv.areas[i], lv.scenery[i]);
        }
        for (i, t) in d.trails.iter().enumerate() {
            line(&t.name, "trail", &lv.trails[i], lv.scenery[na + i]);
        }
        line(
            "(outside every area)",
            "-",
            &lv.elsewhere,
            lv.scenery[na + nt],
        );
        out.push('\n');

        for kind in [RouteKind::Primary, RouteKind::Tour] {
            let steps = plan_route(d, kind);
            let rows = simulate(d, &lv, &steps);
            let _ = writeln!(
                out,
                "== greedy growth, {kind:?} route (eat everything edible, step by step; clock = travel only) =="
            );
            let _ = writeln!(
                out,
                "{:<28} {:>6} {:>12} {:>12} {:>6} {:>12} {:>8} {:>7}  flag",
                "step", "in_r", "edible_in", "ate", "out_r", "left", "next_r", "clock_s"
            );
            for r in &rows {
                let next = if r.next_need.is_finite() {
                    format!("{:.2}", r.next_need)
                } else {
                    "-".into()
                };
                let _ = writeln!(
                    out,
                    "{:<28.28} {:>6.2} {:>12} {:>12} {:>6.2} {:>12} {:>8} {:>7.0}  {}",
                    r.label,
                    r.arrive_r,
                    format!("{}/{:.2}", r.avail.0, r.avail.1),
                    format!("{}/{:.2}", r.ate.0, r.ate.1),
                    r.exit_r,
                    format!("{}/{:.2}", r.left.0, r.left.1),
                    next,
                    r.clock,
                    r.flag
                );
            }
            let (what, need_r) = match d.goal.kind {
                GoalKind::Collect => {
                    let axis = d.goal_axis().unwrap_or(4.0);
                    (format!("{} (axis {axis})", d.goal.target), need(axis))
                }
                GoalKind::Size | GoalKind::Finale => (
                    format!("The size goal ({} yd)", d.goal.diameter),
                    d.goal.diameter / 2.0,
                ),
            };
            let end = rows.last();
            let _ = writeln!(
                out,
                "{what} needs r={need_r:.2}; this route ends at r={:.2} after {:.0}s of travel ({}).\n",
                end.map_or(d.start_radius, |r| r.exit_r),
                end.map_or(0.0, |r| r.clock),
                if end.is_some_and(|r| r.exit_r >= need_r) {
                    "edible on arrival at the goal"
                } else {
                    "NOT edible on this route alone"
                }
            );
        }
        out
    }

    /// Log the report line by line, and write it to `$WOW_AZEROLL_REPORT` when that is set.
    pub(super) fn emit(text: &str) {
        for l in text.lines() {
            info!("azeroll REPORT {l}");
        }
        if let Some(path) = crate::webenv::var("WOW_AZEROLL_REPORT") {
            if let Some(dir) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::write(&path, text) {
                Ok(()) => info!("azeroll REPORT written to {path}"),
                Err(e) => warn!("azeroll REPORT write {path}: {e}"),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn embedded() -> Design {
            parse(include_str!("goldshire.toml")).expect("embedded level parses")
        }
        fn foods(items: &[(f32, f32)]) -> Vec<Food> {
            items
                .iter()
                .map(|&(pickup, volume)| Food {
                    pickup,
                    volume,
                    rise: 0.0,
                })
                .collect()
        }

        fn food(pickup: f32, volume: f32, rise: f32) -> Food {
            Food {
                pickup,
                volume,
                rise,
            }
        }

        #[test]
        fn what_sits_on_a_table_waits_for_a_bigger_ball() {
            let plate = food(0.3, 0.01, 0.0);
            let on_a_table = food(0.3, 0.01, 1.0);
            assert!((food_need(&plate) - need(0.3)).abs() < 1e-6);
            assert!(
                food_need(&on_a_table) > 0.6,
                "a one-yard table is out of reach of a small ball"
            );
            assert!(food_need(&on_a_table) < 1.0);
            // A big thing is limited by its size, not its height.
            assert_eq!(food_need(&food(2.0, 1.0, 0.1)), need(2.0));
        }

        #[test]
        fn utc_matches_known_instants() {
            assert_eq!(utc(0), "1970-01-01 00:00:00Z");
            assert_eq!(utc(86_399), "1970-01-01 23:59:59Z");
            assert_eq!(utc(951_782_400), "2000-02-29 00:00:00Z");
            assert_eq!(utc(1_793_604_833), "2026-11-02 07:33:53Z");
        }

        #[test]
        fn need_is_the_edge_of_the_live_contact_rule() {
            for pickup in [0.1, 0.5, 1.0, 2.8, 4.0] {
                let r = need(pickup);
                assert_eq!(contact(r + 0.001, pickup), Contact::RollUp, "{pickup}");
                assert_ne!(contact(r - 0.01, pickup), Contact::RollUp, "{pickup}");
            }
            assert!(need(1.0e9).is_infinite());
            assert!(band(0.1) < band(1.2) && band(1.2) < band(5.0) && band(5.0) < band(50.0));
        }

        #[test]
        fn routes_run_from_the_start_area_to_the_camp() {
            let d = embedded();
            let primary = plan_route(&d, RouteKind::Primary);
            let tour = plan_route(&d, RouteKind::Tour);
            for route in [&primary, &tour] {
                assert!(matches!(route.first(), Some(Step::Area(_))), "{route:?}");
                assert!(matches!(route.last(), Some(Step::Area(_))), "{route:?}");
                // A trail may chain into another (through a bend that is in no area), but two
                // areas are never adjacent: something is walked between them.
                assert!(
                    route
                        .windows(2)
                        .all(|p| !matches!(p, [Step::Area(_), Step::Area(_)])),
                    "{route:?}"
                );
            }
            assert_eq!(primary.first(), tour.first());
            assert_eq!(primary.last(), tour.last());
            let Some(&Step::Area(start)) = primary.first() else {
                unreachable!()
            };
            assert_eq!(
                area_at(&d, units::wow_to_bevy(d.start)).unwrap_or(start),
                start,
                "the route starts where the ball does"
            );
            // The tour may walk its own trails (the north road skips the inn yard), but it visits
            // every area the primary route does.
            assert!(
                primary
                    .iter()
                    .filter(|s| matches!(s, Step::Area(_)))
                    .all(|s| tour.contains(s)),
                "the tour visits every area the primary route does"
            );
            assert!(tour.len() >= primary.len());
        }

        #[test]
        fn a_route_clears_areas_smallest_first_and_stops_at_what_is_too_big() {
            let d = embedded();
            assert!(d.areas.len() >= 2);
            let mut areas = vec![Vec::new(); d.areas.len()];
            // Crumbs a start ball eats, then a thing only a bigger ball can; the second area
            // holds only a giant.
            areas[0] = foods(&[(0.05, 0.001), (0.05, 0.001), (0.12, 0.5), (500.0, 9.0)]);
            areas[1] = foods(&[(500.0, 9.0)]);
            let lv = Level {
                areas,
                trails: d.trails.iter().map(|_| Vec::new()).collect(),
                elsewhere: Vec::new(),
                scenery: Vec::new(),
            };
            let rows = simulate(&d, &lv, &[Step::Area(0), Step::Area(1)]);
            assert_eq!(rows.len(), 2);
            let v0 = d.start_radius.powi(3);
            assert!((rows[0].arrive_r - d.start_radius).abs() < 1e-5);
            // The crumbs, then the half-volume thing they made edible; never the giant.
            assert_eq!(rows[0].ate.0, 3);
            assert!((rows[0].exit_r.powi(3) - (v0 + 0.502)).abs() < 1e-4);
            assert_eq!(rows[0].left.0, 1);
            assert_eq!(rows[1].ate.0, 0);
            assert_eq!(rows[1].flag, "STARVE");
            assert!((rows[1].exit_r - rows[1].arrive_r).abs() < 1e-6);
            assert!(rows[1].clock >= rows[0].clock);
        }

        #[test]
        fn scenery_is_counted_but_never_summed_or_left() {
            let d = embedded();
            let centre = units::wow_to_bevy(d.areas[1].centre);
            let inv = Inventory {
                all: Vec::new(),
                loose: vec![
                    (centre, food(0.3, 0.01, 0.0)),
                    (centre, food(30.0, 4000.0, 0.0)),
                    (centre + Vec3::X * 1.0e5, food(30.0, 4000.0, 0.0)),
                ],
                trails: d.trails.iter().map(|_| Vec::new()).collect(),
                missing: 0,
                estimated: 0,
            };
            let lv = level(&d, &inv);
            assert_eq!(lv.areas[1].len(), 1, "only the crumb is food");
            assert_eq!(lv.scenery[1], 1);
            assert!(lv.elsewhere.is_empty());
            assert_eq!(lv.scenery[d.areas.len() + d.trails.len()], 1);
            let text = render(&d, &inv, 0);
            assert!(
                text.contains("Primary route") && text.contains("Tour route"),
                "{text}"
            );
            assert!(text.contains("scenery"));
        }

        #[test]
        fn a_trail_is_walked_once_in_order() {
            let d = embedded();
            let Some(t) = d.trails.iter().position(|t| !t.name.is_empty()) else {
                return;
            };
            let mut trails: Vec<Vec<Food>> = d.trails.iter().map(|_| Vec::new()).collect();
            // A boulder first, then crumbs: walked forward it is passed over, never come back to.
            trails[t] = foods(&[(500.0, 9.0), (0.05, 0.001), (0.05, 0.001)]);
            let lv = Level {
                areas: vec![Vec::new(); d.areas.len()],
                trails,
                elsewhere: Vec::new(),
                scenery: Vec::new(),
            };
            let forward = simulate(
                &d,
                &lv,
                &[Step::Trail {
                    index: t,
                    reversed: false,
                }],
            );
            assert_eq!((forward[0].ate.0, forward[0].left.0), (2, 1));
            let back = simulate(
                &d,
                &lv,
                &[Step::Trail {
                    index: t,
                    reversed: true,
                }],
            );
            assert_eq!((back[0].ate.0, back[0].left.0), (2, 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stairs_are_deterministic_rising_recipe_content() {
        let item: Item = toml::from_str(
            r#"
            model = "book.m2"
            count = 9
            arrangement = "stairs"
            spacing = 0.3
            step = 0.125
            heading = 1.5707963
            pitch = 0.25
            blocker = true
        "#,
        )
        .unwrap();
        let points = spots(&item, &mut Rng::new("fixture", 0));
        assert_eq!(points[0], (Vec2::ZERO, 0.));
        assert!((points[8].0.y - 2.4).abs() < 1e-5);
        assert_eq!(points[8].1, 1.0);
        assert!(item.blocker);
        assert_eq!(item.pitch, 0.25);
    }

    fn embedded() -> Design {
        parse(include_str!("goldshire.toml")).expect("embedded level parses")
    }
    #[test]
    fn plans_are_deterministic_and_complete() {
        let d = embedded();
        for scene in &d.scenes {
            for (ix, item) in scene.items.iter().enumerate() {
                let a = plan_item(scene, ix, item);
                let b = plan_item(scene, ix, item);
                let tables = usize::from(
                    matches!(item.arrangement, Arrangement::TableSet) && !item.table.is_empty(),
                ) * item.tables.max(1);
                assert_eq!(a.len(), item.count + tables, "{} item {ix}", scene.name);
                for (p, q) in a.iter().zip(&b) {
                    assert_eq!(p.tf.translation, q.tf.translation);
                    assert_eq!(p.lift, q.lift);
                    assert!(p.tf.translation.is_finite() && p.lift.is_finite());
                }
                if item.species.is_empty() {
                    assert!(
                        a.iter().all(|p| !p.model.is_empty()),
                        "{} item {ix}",
                        scene.name
                    );
                }
            }
        }
        assert!(d.trails.iter().all(|t| plan_trail(t).len() >= 2));
    }
    #[test]
    fn mixed_groups_use_more_than_one_model() {
        let d = embedded();
        for scene in d.scenes.iter().filter(|s| !s.items.iter().any(|i| i.solid)) {
            let models: std::collections::HashSet<_> = scene
                .items
                .iter()
                .enumerate()
                .flat_map(|(ix, item)| plan_item(scene, ix, item))
                .map(|p| p.model)
                .collect();
            assert!(models.len() >= 5, "{} is nearly one model", scene.name);
        }
    }
    #[test]
    fn heaps_stack_and_table_sets_sit_on_the_surface() {
        let scene = Scene {
            name: "t".into(),
            anchor: [0.0; 3],
            yaw: 0.0,
            indoors: false,
            items: vec![],
        };
        let item = |arrangement, step, surface| Item {
            model: "m".into(),
            models: vec![],
            pick: Pick::Random,
            species: String::new(),
            count: 20,
            arrangement,
            spacing: 1.0,
            scale: 1.0,
            jitter: 0.0,
            solid: false,
            blocker: false,
            rooted: false,
            pitch: 0.0,
            roll: 0.0,
            goal: false,
            present: false,
            name: None,
            offset: [0.0; 2],
            lift: 0.0,
            yaw: None,
            tilt: 0.0,
            topple: 0.0,
            topple_lift: 0.0,
            wobble: 0.0,
            gap: 0.0,
            step,
            heading: 0.0,
            bend: 0.0,
            table: String::new(),
            tables: 1,
            table_gap: 1.0,
            table_scale: 1.0,
            surface,
            on: None,
            level: false,
            climb: None,
        };
        let heap = plan_item(&scene, 0, &item(Arrangement::Heap, 0.5, 0.0));
        assert!(heap.iter().any(|p| p.lift == 0.0) && heap.iter().any(|p| p.lift >= 0.5));
        let table = plan_item(&scene, 0, &item(Arrangement::TableSet, 0.0, 1.0));
        assert!(table.iter().all(|p| p.lift == 1.0));
    }
    #[test]
    fn d10_bound_covers_actual_growth_and_rejects_overfilled_room() {
        let items = vec![0.12; 80];
        let mut r = 1.89_f32;
        for s in &items {
            r = (r.powi(3) + super::super::collect::growth(*s, r)).cbrt();
        }
        assert!(exit_bound(1.89, &items) >= r - 0.00001);
        assert!(0.925 * exit_bound(1.89, &items) < 1.89);
        assert!(0.925 * exit_bound(1.89, &vec![1.0; 100]) > 1.89);
    }
}

/// The shipped recipes, checked as data: they parse, agree with the registry, and refer only to
/// things that exist. The barrier and claim shapes are tested on their own geometry.
#[cfg(test)]
mod recipes {
    use super::super::campaign::LEVELS;
    use super::super::collect::{contact, solid_push, Contact};
    use super::*;

    #[test]
    fn installed_stormwind_city_scenes_can_spawn_every_authored_prop() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let Some(data) = benilla_formats::wow_data() else {
            return;
        };
        let mut chain = benilla_formats::open_chain(&data).unwrap();
        // The whole city's collision, as the spawner sees it before any piece is eaten: the
        // scenes stand in the Valley, the Trade District, on the canal banks and in the Square.
        let (mut vertices, mut indices) = (vec![], vec![]);
        for group in 0..306 {
            let path = format!(r"world\wmo\azeroth\buildings\stormwind\stormwind_{group:03}.wmo");
            let bytes = chain.read_file(&path).unwrap();
            benilla_formats::accumulate_wmo_group_collision(&bytes, &mut vertices, &mut indices);
        }
        let (sin, cos) = (-141.5_f32).to_radians().sin_cos();
        let vertices = vertices
            .into_iter()
            .map(|p| {
                units::wow_to_bevy([
                    cos * p[0] - sin * p[1] - 8931.634,
                    sin * p[0] + cos * p[1] + 539.294,
                    p[2] + 101.957,
                ])
            })
            .collect();
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
        let d = parse(include_str!("stormwind.toml")).unwrap();
        app.world_mut()
            .run_system_once(move |world: WorldCollision| {
                // Every city scene (indoors = true: the WMO floor ray) must find geometry under each
                // prop it plans, within the +-4 yd ray the spawner uses; the Square's terrace rises
                // 8 yd over 24, and a flat anchor once lost the last fair rows this way.
                let mut count = 0;
                let mut misses = Vec::new();
                for scene in d.scenes.iter().filter(|s| s.indoors) {
                    for (i, item) in scene.items.iter().enumerate() {
                        for placed in plan_item(scene, i, item) {
                            count += 1;
                            let origin = placed.tf.translation + Vec3::Y * 4.;
                            if world.ray_body(origin, Dir3::NEG_Y, 8.).is_none() {
                                misses.push(format!("{}: {:?}", scene.name, placed.tf.translation));
                            }
                        }
                    }
                }
                let expected: usize = d
                    .scenes
                    .iter()
                    .filter(|s| s.indoors)
                    .flat_map(|s| &s.items)
                    .map(|i| i.count)
                    .sum();
                assert!(count > 0);
                assert_eq!(count, expected);
                assert!(
                    misses.is_empty(),
                    "spawn rays miss the floor:\n{}",
                    misses.join("\n")
                );
            })
            .unwrap();
    }

    fn all() -> Vec<(&'static str, Design)> {
        LEVELS
            .iter()
            .map(|l| {
                (
                    l.id,
                    parse(l.recipe).unwrap_or_else(|e| panic!("{}: {e}", l.id)),
                )
            })
            .collect()
    }

    #[test]
    fn indoor_tour_routes_into_the_cellar_and_upstairs() {
        let d = parse(include_str!("lions-pride.toml")).unwrap();
        let route = plan_route(&d, RouteKind::Tour);
        for name in ["Cellar", "Upstairs"] {
            assert!(
                route.iter().any(|s| s.label(&d) == name),
                "tour must enter {name}: {route:?}"
            );
        }
        let at = |name| route.iter().position(|s| s.label(&d) == name).unwrap();
        assert!(at("Upstairs") < at("Bar") && at("Cellar") < at("Bar"));
        assert!(
            matches!(route.first(), Some(Step::Area(_)))
                && matches!(route.get(1), Some(Step::Trail { .. })),
            "seed stair-sized growth in the nursery, then explore rooms"
        );
    }
    #[test]
    fn every_embedded_recipe_parses_and_agrees_with_the_registry() {
        for (id, d) in all() {
            assert_eq!(d.level.id, id);
            assert!(
                !d.level.title.is_empty() && !d.level.briefing.is_empty(),
                "{id}"
            );
            assert!(
                !d.level.objective.is_empty() && !d.level.subtitle.is_empty(),
                "{id}"
            );
            let entry = LEVELS.iter().find(|l| l.id == id).unwrap();
            assert_eq!(d.level.cast.is_some(), entry.cast.is_some(), "{id}: cast");
        }
    }

    #[test]
    fn a_recipe_hides_at_most_one_parcel_and_asks_four_challenges_or_fewer() {
        for (id, d) in all() {
            let flagged: usize = d
                .scenes
                .iter()
                .flat_map(|s| &s.items)
                .filter(|i| i.present)
                .map(|i| i.count)
                .sum();
            assert_eq!(flagged, usize::from(d.present.is_some()), "{id}: parcel");
            if let Some(p) = &d.present {
                assert!(
                    !p.id.is_empty() && !p.title.is_empty() && p.display > 0,
                    "{id}"
                );
            }
            // D33 dropped the `speed` and `unrammed` kinds from the refreshed recipes (a level
            // still waiting for its refresh keeps whatever it had).
            if d.present.is_some() {
                assert!(
                    d.challenges.len() <= 4,
                    "{id}: {} challenges",
                    d.challenges.len()
                );
                assert!(
                    d.challenges
                        .iter()
                        .all(|c| !matches!(c.kind, ChallengeKind::Time | ChallengeKind::Unrammed)),
                    "{id}: a clock or ram challenge came back"
                );
            }
        }
        // A recipe whose [present] has no item, or whose item has no [present], does not parse.
        let lp = LEVELS
            .iter()
            .find(|l| l.id == "lions-pride")
            .unwrap()
            .recipe;
        assert!(parse(&lp.replacen("[present]", "[present_gone]", 1)).is_err());
        assert!(parse(&lp.replacen("present = true", "present = false", 1)).is_err());
    }

    #[test]
    fn recipes_refer_only_to_what_exists() {
        for (id, d) in all() {
            let area = |name: &str| d.areas.iter().any(|a| a.name == name);
            if let Some(name) = &d.goal.area {
                assert!(area(name), "{id}: goal area {name:?}");
            }
            for tier in &d.tiers {
                for rep in &tier.reps {
                    match level::Rep::parse(rep) {
                        Some(level::Rep::Species(s)) => {
                            assert!(d.herds.iter().any(|h| h.species == s), "{id}: rep {rep}");
                        }
                        Some(level::Rep::Adults) => {
                            assert!(
                                LEVELS.iter().find(|l| l.id == id).unwrap().cast.is_some(),
                                "{id}: adults need a cast"
                            );
                        }
                        None => panic!("{id}: unreadable rep {rep:?}"),
                    }
                }
            }
            let mut ids = std::collections::HashSet::new();
            for c in &d.challenges {
                assert!(ids.insert(&c.id), "{id}: duplicate challenge {}", c.id);
                match c.kind {
                    ChallengeKind::Scene => assert!(
                        d.scenes.iter().any(|s| s.name == c.scene)
                            || (!c.prefix.is_empty()
                                && d.scenes.iter().any(|s| s.name.starts_with(&c.prefix))),
                        "{id}: challenge scene {:?}",
                        c.scene
                    ),
                    ChallengeKind::CollectAll => assert!(
                        d.herds
                            .iter()
                            .any(|h| h.species == c.species && !c.species.is_empty())
                            || !c.model.is_empty()
                            || !c.name.is_empty()
                            || !c.names.is_empty()
                            || !c.species_list.is_empty(),
                        "{id}: collect-all of nothing"
                    ),
                    ChallengeKind::Before => assert!(
                        (!c.species.is_empty() || !c.name.is_empty())
                            && (!c.before_species.is_empty() || !c.before_name.is_empty()),
                        "{id}: {} needs a set and a gate",
                        c.id
                    ),
                    ChallengeKind::Within => assert!(
                        c.secs > 0.0 && d.scenes.iter().any(|s| s.name == c.scene),
                        "{id}: {} needs a scene and seconds",
                        c.id
                    ),
                    ChallengeKind::Time => assert!(c.secs > 0.0, "{id}: {}", c.id),
                    ChallengeKind::Size => assert!(c.diameter > 0.0, "{id}: {}", c.id),
                    ChallengeKind::Unrammed => {}
                }
            }
            for a in d.areas.iter().filter(|a| a.enclosed) {
                assert!(a.clearance > 0.0 && !a.doors.is_empty(), "{id}: {}", a.name);
            }
        }
    }

    #[test]
    fn every_level_starts_inside_its_leash_and_its_claims() {
        for (id, d) in all() {
            let start = units::wow_to_bevy(d.start);
            assert!(
                (d.leash_centre() - start).length() < d.leash_radius(),
                "{id}: start outside the leash"
            );
            assert!(
                d.claim_zones().iter().any(|z| z.contains(start)),
                "{id}: start outside its claims"
            );
            assert!(d.goal_diameter() > 2.0 * d.start_radius, "{id}");
            assert!(d.time_limit >= 60.0, "{id}");
        }
    }

    #[test]
    fn market_day_goal_is_the_prize_pumpkin() {
        let d = all()
            .into_iter()
            .find(|(id, _)| *id == "market-day")
            .unwrap()
            .1;
        assert_eq!(d.start, [-9446.0, 57.0, 56.5]);
        assert_eq!(d.start_radius, 0.32);
        assert_eq!(d.time_limit, 660.0);
        assert_eq!(d.leash_radius(), 680.0);
        assert_eq!(d.leash_centre(), units::wow_to_bevy(d.start));
        let zones = d.claim_zones();
        assert_eq!(
            (zones.len(), zones[0].radius, zones[0].centre),
            (1, 650.0, [-9450.0, 60.0, 57.0])
        );
        // The goal is a scene prop, not a herd species; Hogger is a boss without being the goal.
        assert_eq!(d.goal.kind, GoalKind::Collect);
        assert_eq!(d.goal.target, "Prize Pumpkin");
        assert!(!d.herds.iter().any(|h| h.species == d.goal.target));
        let flagged: Vec<_> = d
            .scenes
            .iter()
            .flat_map(|s| &s.items)
            .filter(|i| i.goal)
            .collect();
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].count, 1);
        assert_eq!(d.goal_axis(), Some(d.goal.axis));
        // The pumpkin outsizes Hogger, so he can be rolled up on the way.
        assert!(d.goal_diameter() > level::edible_diameter(4.0));
        assert_eq!(d.camp, Some([-9568.0, 180.0, 59.5]));
        assert_eq!(d.patrol, Some([-9470.0, 66.0, 57.0]));
        // No invisible walls (playtest 2026-11-03): the trunk pocket, the wagon lane and the fence
        // corner are closed by visible `blocker` props (a hay wagon, the weaver's tent, three stumps,
        // the haystack in the wagon's lee and the awning under the square's tree clump), never by barriers.
        assert!(
            d.barriers.is_empty(),
            "market-day has no invisible barriers"
        );
        let blockers = d
            .scenes
            .iter()
            .flat_map(|s| &s.items)
            .filter(|i| i.blocker)
            .count();
        assert_eq!(
            blockers, 7,
            "the wagon, the tent, three stumps, the lane haystack and the awning"
        );
        assert_eq!(
            d.herds.iter().find(|h| h.species == "Hogger").unwrap().fit,
            4.0
        );
        assert!(d
            .roster
            .iter()
            .any(|s| s.kind == Kind::Boss && s.name == "Hogger"));
        let ids: Vec<_> = d.challenges.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["hogger", "chickens", "garden", "parade"]);
        assert_eq!(d.win_title(), "PRIZE PUMPKIN, ALL ROLLED UP!");
        assert!(d.goal_area().is_some() && d.goal_point().is_some());
        // The stage is the last stop of the primary route, reached only from the livestock fair, so
        // the route sees the whole town (garden, market, south gate, fair) before the parade home.
        let route = plan_route(&d, RouteKind::Primary);
        let stops: Vec<_> = route.iter().map(|s| s.label(&d)).collect();
        assert_eq!(stops.last().map(String::as_str), Some("Judging stage"));
        assert!(stops.iter().any(|s| s == "Livestock fair"), "{stops:?}");
        assert!(stops.iter().any(|s| s == "South gate"), "{stops:?}");
        // Hogger's camp is on the way home: the parade home starts at the fair's gate after it.
        assert!(stops.iter().any(|s| s == "Raiders' camp"), "{stops:?}");
    }

    #[test]
    fn a_size_goal_has_no_boss_and_honours_its_authored_title() {
        let mut recipe: toml::Value = toml::from_str(include_str!("lions-pride.toml")).unwrap();
        recipe["goal"]["diameter"] = toml::Value::Float(3.0);
        recipe["goal"]["title"] = toml::Value::String("GREAT BIG SUPPER!".into());
        // Exercise the default failure line independently of the embedded level's copy.
        recipe["goal"].as_table_mut().unwrap().remove("fail");
        let d = parse(&toml::to_string(&recipe).unwrap()).unwrap();
        assert_eq!(d.goal.kind, GoalKind::Size);
        assert!(d.roster.iter().all(|s| s.kind != Kind::Boss));
        assert_eq!(d.goal_axis(), None);
        assert_eq!(d.goal_diameter(), 3.0);
        assert_eq!(d.win_title(), "GREAT BIG SUPPER!");
        assert_eq!(d.fail_line(), "Not big enough yet.");
        assert_eq!(d.camp, None);
    }

    #[test]
    fn a_bad_goal_is_refused() {
        let mut recipe: toml::Value = toml::from_str(include_str!("lions-pride.toml")).unwrap();
        recipe["goal"]["diameter"] = toml::Value::Float(0.1);
        assert!(parse(&toml::to_string(&recipe).unwrap()).is_err());
        // A prop goal needs its axis, and a herd-less target needs a flagged item.
        let market: toml::Value = toml::from_str(include_str!("goldshire.toml")).unwrap();
        let mut invalid_axis = market.clone();
        invalid_axis["goal"]["axis"] = toml::Value::Float(0.0);
        assert!(parse(&toml::to_string(&invalid_axis).unwrap()).is_err());
        let mut missing_item = market;
        let mut removed = 0;
        for scene in missing_item["scenes"].as_array_mut().unwrap() {
            for item in scene["items"].as_array_mut().unwrap() {
                if item.get("goal").and_then(toml::Value::as_bool) == Some(true) {
                    item["goal"] = toml::Value::Boolean(false);
                    removed += 1;
                }
            }
        }
        assert!(removed > 0, "fixture must contain a goal item");
        assert!(parse(&toml::to_string(&missing_item).unwrap()).is_err());
    }

    #[test]
    fn claim_zones_are_discs_or_capsules() {
        let disc = ClaimZone {
            centre: [-9450.0, 60.0, 57.0],
            radius: 100.0,
            to: None,
        };
        let at = |x: f32, y: f32| units::wow_to_bevy([x, y, 57.0]);
        assert!(disc.contains(at(-9450.0, 60.0)) && disc.contains(at(-9450.0 + 99.0, 60.0)));
        assert!(!disc.contains(at(-9450.0 + 101.0, 60.0)));
        let capsule = ClaimZone {
            to: Some([-9450.0, 460.0, 57.0]),
            ..disc
        };
        // Along the spine and off its side, within the radius; past the far end by more than it.
        assert!(capsule.contains(at(-9450.0, 260.0)));
        assert!(capsule.contains(at(-9450.0 + 90.0, 300.0)));
        assert!(!capsule.contains(at(-9450.0 + 110.0, 300.0)));
        assert!(capsule.contains(at(-9450.0, 460.0 + 99.0)));
        assert!(!capsule.contains(at(-9450.0, 460.0 + 101.0)));
        // The authored route may grow. Every capsule includes its own spine; absent claims
        // still fall back to a single start disc.
        let d = parse(include_str!("stormwind.toml")).unwrap();
        assert!(!d.claim_zones().is_empty());
        for z in d.claim_zones() {
            assert!(z.contains(units::wow_to_bevy(z.centre)));
            if let Some(to) = z.to {
                assert!(z.contains(units::wow_to_bevy(to)));
            }
        }
        assert!(d.claim_zones().iter().all(|z| z.to.is_some()));
        let mut value: toml::Value = toml::from_str(include_str!("stormwind.toml")).unwrap();
        value.as_table_mut().unwrap().remove("claim");
        let no_claim = toml::to_string(&value).unwrap();
        let z = parse(&no_claim).unwrap().claim_zones();
        assert_eq!(z.len(), 1);
        assert_eq!((z[0].radius, z[0].to), (DEFAULT_CLAIM_RADIUS, None));
    }

    /// A ball of this radius resting on the floor `floor` yards up, centred at `at` (Bevy x, z).
    fn rest(at: Vec2, floor: f32, radius: f32) -> Vec3 {
        Vec3::new(at.x, floor + radius, at.y)
    }

    #[test]
    fn a_barrier_is_a_wall_to_every_ball_and_never_collectible() {
        let b = Barrier {
            from: [-9461.35, 44.64, 56.95],
            to: [-9456.96, 44.10, 56.95],
            height: default_barrier_height(),
            thickness: default_barrier_thickness(),
            extend: default_barrier_extend(),
        };
        let (tf, thing) = barrier_parts(&b);
        let floor = 56.95;
        let mid = Vec2::new(tf.translation.x, tf.translation.z);
        for radius in [0.1, 0.13, 0.3, 1.0, 2.5, 6.0, 20.0, 80.0] {
            assert_eq!(contact(radius, thing.pickup), Contact::Wall, "r={radius}");
            // In the doorway's middle, on the floor: pushed out, whatever the size.
            let pos = rest(mid, floor, radius);
            let push = solid_push(&tf, &thing, pos, radius, true);
            assert!(push.is_some(), "r={radius}: the doorway is open");
            let p = push.unwrap();
            assert!(p.y.abs() < 1e-4, "horizontal push");
            // Pushed clear of the box on the far side of the mid-plane it was nearer.
            assert!(solid_push(&tf, &thing, pos + p * 1.001, radius, true).is_none());
        }
        // Not only the middle: a jamb-side ball (just past the end) is stopped, and a ball well
        // clear of it is not.
        let along = Vec2::new(
            (units::wow_to_bevy(b.to) - units::wow_to_bevy(b.from)).x,
            (units::wow_to_bevy(b.to) - units::wow_to_bevy(b.from)).z,
        )
        .normalize();
        let near_jamb = mid + along * (2.2 + 0.3);
        assert!(solid_push(&tf, &thing, rest(near_jamb, floor, 0.3), 0.3, true).is_some());
        let clear = mid + along * 12.0;
        assert!(solid_push(&tf, &thing, rest(clear, floor, 0.3), 0.3, true).is_none());
        // A ball a yard under the floor line (the support dips) is still caught: the base is below.
        assert!(solid_push(&tf, &thing, rest(mid, floor - 0.8, 0.3), 0.3, true).is_some());
    }

    #[test]
    fn a_dash_cannot_cross_a_barrier_between_two_frames() {
        let b = Barrier {
            from: [-9461.35, 44.64, 56.95],
            to: [-9456.96, 44.10, 56.95],
            height: default_barrier_height(),
            thickness: default_barrier_thickness(),
            extend: default_barrier_extend(),
        };
        let (tf, thing) = barrier_parts(&b);
        // The normal of the doorway plane, in the ground plane.
        let across = (tf.rotation * Vec3::Z).normalize();
        let mid = tf.translation;
        for radius in [0.13, 0.5, 1.0, 2.0, 4.0] {
            // The fastest the ball can go is a dash at the radius's top speed; a long frame is
            // capped at 0.05 s (`ball::MAX_DT`). Walk it at that rate, resolving after each step.
            let speed = super::super::ball::top_speed(radius) * 1.5;
            let step = speed * 0.05;
            let mut pos = rest(Vec2::new(mid.x, mid.z), 56.95, radius) - across * (radius + 3.0);
            pos.y = 56.95 + radius;
            for _ in 0..400 {
                pos += across * step;
                if let Some(push) = solid_push(&tf, &thing, pos, radius, true) {
                    pos += push;
                }
            }
            let side = (pos - mid).dot(across);
            assert!(
                side < 0.0,
                "r={radius}: crossed to the far side (offset {side:.2}, step {step:.2})"
            );
        }
    }

    #[test]
    fn lions_pride_uses_a_visible_permanent_door_blocker() {
        let d = all()
            .into_iter()
            .find(|(id, _)| *id == "lions-pride")
            .unwrap()
            .1;
        assert!(d.barriers.is_empty(), "L1 has no invisible internal seals");
        let blockers: Vec<_> = d
            .scenes
            .iter()
            .flat_map(|scene| {
                scene
                    .items
                    .iter()
                    .filter(|i| i.blocker)
                    .map(move |item| (scene, item))
            })
            .collect();
        assert!(!blockers.is_empty(), "the exit is visibly blocked");
        assert!(blockers
            .iter()
            .all(|(_, i)| !i.model.is_empty() && !i.goal && i.species.is_empty()));
        assert!(blockers.iter().any(|(s, _)| {
            units::flat(units::wow_to_bevy(s.anchor) - units::wow_to_bevy([-9459.93, 38.11, 56.95]))
                .length()
                < 1.
        }));
        // props::assemble assigns this infinite pickup gate to visible blocker copies.
        for r in [0.13, 0.5, 1.25, 4., 32., 1000.] {
            assert_eq!(contact(r, f32::INFINITY), Contact::Wall);
        }
        for corner in [[-9480., -13., 49.8], [-9454.5, 38.1, 56.95]] {
            assert!(
                units::flat(units::wow_to_bevy(corner) - d.leash_centre()).length()
                    + d.goal_diameter() * 0.5
                    < d.leash_radius(),
                "emergency leash must contain the whole inn"
            );
        }
    }
}
