//! Elwynn's creatures: the roster ([`level::super::design::Design::roster`]) resolved through the creature DBC catalog,
//! spawned as local units once the terrain under them is up and their model is loaded, sized for
//! the ball ([`Collectible`]), and given something to do: wander, run from a ball that can eat
//! them, chase one that cannot. What they do keys off the one classifier the ball rolls things up
//! with, [`contact`] on the creature's maximum axis, so an animal never flees something that
//! cannot eat it and never charges something that can.
//!
//! The M2 is loaded before the unit is spawned so the collection size (its bounding-sphere radius
//! times the final scale) is known from the first frame, and so Hogger can be scaled to an exact
//! size. The stock attach and animation systems dress the unit; this module only moves its
//! [`Transform`] and sets its [`MovementState`].

use std::borrow::Cow;
use std::collections::HashMap;

use benilla_assets::{m2_url, LockRecover, M2Model, WorldAssets};
use benilla_formats::{load_creature_catalog, CreatureCatalog};
use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;
use benilla_world::world_point::WorldPoint;
use bevy::asset::LoadState;
use bevy::platform::time::Instant;
use bevy::prelude::*;

use super::ball::{floor_rest, top_speed};
use super::collect::{contact, Contact, Ram, Shed};
use super::level::{Behavior, Look, Place, ELITE_SCALE, START};
use super::units::{self, flat, wow_to_bevy, yaw_toward, UnitSpec};
use super::{
    playing, Attached, BallState, Collected, Collectible, Dash, Kind, Phase, Rng, Run, Source,
};
use crate::creature_anim::{move_flags, wrap_pi, CastHold, EmoteAnim, MovementState, PlaySeq};

/// How far a creature wanders from where it spawned, yards. Chasing ignores it.
const WANDER_RANGE: f32 = 40.0;
/// A boss keeps closer to his camp.
const BOSS_RANGE: f32 = 20.0;
/// Units spawned per frame at most, so a rush of terrain does not hitch.
const SPAWNS_PER_FRAME: usize = 6;
/// Pending spawn points tested per frame at most.
const SCANS_PER_FRAME: usize = 48;
/// A hostile rams the ball when it is within this fraction of its size of the ball's surface.
const RAM_REACH: f32 = 0.6;
/// How long a hostile charges before it gives up, seconds.
const CHARGE_MAX: f32 = 4.5;
/// A hostile gives up a chase that has taken it this far from where it spawned, yards.
const CHASE_LEASH: f32 = 70.0;
/// After a ram the hostile backs away for this long, seconds.
const RETREAT_SECS: f32 = 2.2;
/// After a ram or a given-up chase the hostile ignores the ball for a time in this range, seconds.
const COOLDOWN_SECS: (f32, f32) = (6.0, 10.0);
/// A skittish animal starts running when the ball is within this fraction of the radius that would
/// roll it up: it senses the danger a moment early, but is still not catchable-by-contact yet.
const FLEE_MARGIN: f32 = 0.08;
/// A fleeing creature never runs faster than this fraction of the ball's undashed top speed at
/// its current size, so every runner stays catchable (the dash is 1.5x on top).
const FLEE_SPEED_CAP: f32 = 0.7;
/// Seconds between a creature's looks ahead for geometry.
const PROBE_EVERY: f32 = 0.25;
/// Creatures further than this many yards (plus ten per yard of ball radius) from the ball do not
/// probe: nothing there is on screen.
const PROBE_RANGE: f32 = 50.0;
/// How far past its own size a creature looks ahead, plus this many seconds of travel, yards.
const PROBE_AHEAD: f32 = 0.5;
/// A creature that turns away from a wall keeps the new heading this long, seconds.
const STEER_SECS: f32 = 0.8;
/// A creature with no way round stands and cowers this long, seconds.
const COWER_SECS: f32 = 0.8;
/// A step onto ground higher or lower than this is a cliff or a wall face, yards.
const STEP_LIMIT: f32 = 0.7;

// Hogger's script. The early pass: after `VISIT_FROM` seconds (or the recipe's `patrol_at`, if
// later) and while the ball is outdoors near the village, he is set down at the edge of the
// camera's view, charges the ball, rams it if he catches it, then lumbers home. At the camp he
// walks a small loop, and the ball's size decides the rest ([`react`]).
/// Earliest second of play for the pass: the village has been seen first.
const VISIT_FROM: f32 = 15.0;
/// The pass may slip past the recipe's `patrol_until` by this long while the ball is indoors.
const VISIT_GRACE: f32 = 30.0;
/// The ball must be this close to the recipe's `patrol` point, yards, for the pass to start.
const VISIT_VILLAGE: f32 = 150.0;
/// Where he comes in: `(bearing off the heading in radians, yards from the ball)`, tried in order.
const VISIT_ENTRIES: [(f32, f32); 8] = [
    (0.35, 34.0),
    (-0.35, 34.0),
    (0.6, 32.0),
    (-0.6, 32.0),
    (0.15, 30.0),
    (-0.15, 30.0),
    (0.9, 28.0),
    (-0.9, 28.0),
];
/// The view must have turned slower than this (radians per second) for `VISIT_STEADY` seconds
/// before he is set down: a ball being steered through a corner has its camera elsewhere.
const VISIT_STEADY_RATE: f32 = 0.35;
const VISIT_STEADY: f32 = 0.4;
/// He stalks in at this speed (the Walk gait, a menacing amble) until he is within `VISIT_ROAR`
/// yards, stops to roar for `ROAR_HOLD`, then charges at a run.
const VISIT_STALK: f32 = 3.2;
const VISIT_ROAR: f32 = 16.0;
/// How long he charges before giving up, seconds.
const VISIT_CHASE: f32 = 14.0;
/// His lumber home after the pass, yards per second (the Walk gait).
const LUMBER: f32 = 2.8;
/// He is put back at the camp once the ball is further than this from him...
const VISIT_VANISH_DIST: f32 = 55.0;
/// ...and he has been walking away this long, seconds. Further than `VISIT_VANISH_FAR` always.
const VISIT_VANISH_AFTER: f32 = 12.0;
const VISIT_VANISH_FAR: f32 = 110.0;
/// His patrol at the camp: loop radius (yards), stroll speed and waypoints.
const PATROL_RADIUS: f32 = 8.0;
const PATROL_SPEED: f32 = 1.9;
const PATROL_POINTS: usize = 6;
/// He senses the ball this far (plus three per yard of its radius) from the camp, so the roar and
/// the retreat are seen from the approach.
const BOSS_SCARE: f32 = 12.0;
/// Once he has taken a stand (a roar, a retreat, a flight) he holds it until the ball is this much
/// further off, so he does not flicker at the edge of his range.
const BOSS_HYSTERESIS: f32 = 6.0;
/// He stands and roars for this long on first giving ground, seconds, then again this often.
const ROAR_HOLD: f32 = 1.0;
const ROAR_EVERY: f32 = 5.0;
/// His backward walk while giving ground, yards per second.
const BACK_OFF_SPEED: f32 = 2.4;
/// `AnimationData` ids played as one-shots over his gait: EmoteRoar and EmoteChicken.
const ANIM_ROAR: u16 = 74;
const ANIM_PANIC: u16 = 78;
/// Seconds between his flapping while he flees.
const PANIC_EMOTE: f32 = 2.6;

/// A roster entry once the catalog has answered for it.
#[derive(Clone)]
struct Resolved {
    /// The `CreatureDisplayInfo` row to dress the unit with.
    display: u32,
    /// The display's M2 (`.mdx` spelling, as the DBC has it).
    model_path: String,
    /// `CreatureModelData.modelScale * CreatureDisplayInfo.creatureModelScale`.
    scale: f32,
    /// `CreatureModelData.collisionHeight`, raw model units: the size fallback.
    height: f32,
}

/// The roster's models, and the M2 handles kept alive for sizing.
#[derive(Resource, Default)]
struct Catalog {
    /// Parallel to [`super::design::Design::roster`]; `None` where the display or model did not resolve.
    species: Vec<Option<Resolved>>,
    prince: Option<Resolved>,
    /// The catalog has been read (successfully or not): `species` is final.
    ready: bool,
    /// M2 handles by URL.
    models: HashMap<String, Handle<M2Model>>,
}

/// One creature waiting for its terrain and its model.
#[derive(Clone, Copy)]
struct Planned {
    /// An index into [`super::design::Design::roster`].
    species: usize,
    /// Bevy space; `y` is a hint until the terrain answers.
    pos: Vec3,
    elite: bool,
}

/// This run's creatures not yet spawned, nearest the start first.
#[derive(Resource, Default)]
struct Pending(Vec<Planned>);

/// What a creature is doing: its leash, its gaits and its wander state.
#[derive(Component)]
struct Brain {
    behavior: Behavior,
    /// Where it spawned, Bevy space.
    home: Vec3,
    leash: f32,
    walk: f32,
    run: f32,
    /// The wander direction, a unit vector in the ground plane.
    heading: Vec3,
    /// Walking along `heading` rather than standing.
    walking: bool,
    /// Seconds until the wander state changes.
    timer: f32,
    /// A hostile's seconds spent charging the ball in the current attack.
    charge: f32,
    /// Seconds a hostile ignores the ball after a ram or a given-up chase.
    cooldown: f32,
    /// Seconds a hostile still backs away from the ball after a ram.
    retreat: f32,
    /// What it last decided about the ball.
    reaction: Reaction,
    /// Seconds until the next look ahead for geometry.
    probe: f32,
    /// The heading it turned onto to get round something, held for `steer_left` seconds.
    steer: Vec3,
    steer_left: f32,
    /// Seconds it stands blocked.
    cower: f32,
}

/// Hogger's script, on him alone: where he is in the pass, and the small clocks of his camp acting.
#[derive(Component)]
struct Stage {
    beat: Beat,
    /// Seconds in the current beat.
    t: f32,
    /// The pass has been run (or its window has closed).
    visited: bool,
    /// The next waypoint of the camp loop, and the seconds spent walking to it.
    waypoint: usize,
    leg: f32,
    /// Seconds he still stands roaring at the ball, and until he roars again.
    roar_hold: f32,
    roar_next: f32,
    /// Seconds spent fleeing, for the zigzag and the stumbles.
    panic: f32,
    /// The pass: he has roared and is charging; and the steadiness of the camera's yaw.
    roared: bool,
    view: f32,
    steady: f32,
}

/// Where Hogger is in his script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// At the camp, on his loop. The ball's size decides how he treats it.
    Camp,
    /// The early pass: charging the ball from the edge of the view.
    Visit,
    /// Lumbering home after the pass.
    Return,
}

impl Stage {
    fn new() -> Self {
        Self {
            beat: Beat::Camp,
            t: 0.0,
            visited: false,
            waypoint: 0,
            leg: 0.0,
            roar_hold: 0.0,
            roar_next: 0.0,
            panic: 0.0,
            roared: false,
            view: 0.0,
            steady: 0.0,
        }
    }
}

/// What a creature makes of the ball, from the contact state of the ball against the creature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Reaction {
    /// Not worried: wanders.
    Idle,
    /// The ball is nearly big enough to knock it loose: it stops and watches.
    Notice,
    /// The ball can eat it, or nearly: it runs.
    Flee,
    /// The ball is a wall to it: a hostile charges.
    Charge,
    /// The ball is bigger than it but cannot eat it yet: a hostile gives ground.
    BackOff,
}

/// How a creature of this temper reacts to a ball of `radius`, given the creature's maximum axis.
/// The only place that decides, so the animals and the pickup rule cannot disagree.
///
/// | contact   | skittish                          | hostile  |
/// |-----------|-----------------------------------|----------|
/// | `Wall`    | idle (or flee, within the margin) | charge   |
/// | `Dislodge`| notice (or flee, within margin)   | back off |
/// | `RollUp`  | flee                              | flee     |
fn react(behavior: Behavior, radius: f32, axis: f32) -> Reaction {
    let state = contact(radius, axis);
    match behavior {
        Behavior::Passive => Reaction::Idle,
        Behavior::Skittish => {
            if state == Contact::RollUp
                || contact(radius * (1.0 + FLEE_MARGIN), axis) == Contact::RollUp
            {
                Reaction::Flee
            } else if state == Contact::Dislodge {
                Reaction::Notice
            } else {
                Reaction::Idle
            }
        }
        Behavior::Hostile => match state {
            Contact::Wall => Reaction::Charge,
            Contact::Dislodge => Reaction::BackOff,
            Contact::RollUp => Reaction::Flee,
        },
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Catalog>()
        .init_resource::<Pending>()
        .add_systems(OnEnter(Phase::Ready), plan_spawns.after(super::reset_run))
        .add_systems(Update, (resolve_roster, spawn_creatures).chain())
        .add_systems(
            Update,
            prince.after(spawn_creatures).after(WorldStage::Input),
        )
        .add_systems(Update, on_collected)
        .add_systems(
            Update,
            (hogger_pass, creature_ai, camp_test)
                .chain()
                .after(WorldStage::Input)
                .run_if(playing),
        );
}

// ---------------------------------------------------------------------------------------------
// Resolving the roster
// ---------------------------------------------------------------------------------------------

/// A model path as the catalog and the roster can both be compared by: lowercase, backslashes,
/// no extension.
fn norm(path: &str) -> String {
    let p = path.to_ascii_lowercase().replace('/', "\\");
    for ext in [".mdx", ".mdl", ".m2"] {
        if let Some(stem) = p.strip_suffix(ext) {
            return stem.to_string();
        }
    }
    p
}

/// Read the creature catalog from the archives once they are open, and resolve every species.
/// The stock catalog is private to `entities`, so this loads a second copy (two small DBCs).
fn resolve_roster(
    mut cat: ResMut<Catalog>,
    assets: Option<Res<WorldAssets>>,
    design: Res<super::design::Design>,
) {
    if design.is_changed() {
        cat.ready = false;
    }
    if cat.ready {
        return;
    }
    let Some(assets) = assets else { return };
    let loaded = {
        let mut chain = assets.chain.lock_recover();
        load_creature_catalog(&mut chain)
    };
    cat.species = match loaded {
        Ok(catalog) => {
            let display = catalog
                .display_models()
                .filter_map(|(d, _)| {
                    let m = catalog.model(d)?;
                    (norm(&m.model_path) == "character\\gnome\\male\\gnomemale"
                        && m.npc_appearance.is_some()
                        && m.scale > 0.0)
                        .then_some(d)
                })
                .min();
            cat.prince = display.and_then(|d| resolve_one(&catalog, d));
            if let Some(p) = &cat.prince {
                info!("azeroll: Prince gnome display={}", p.display);
            }
            resolve(&catalog, &design.roster)
        }
        Err(e) => {
            warn!("azeroll: creature catalog unavailable, no creatures: {e:#}");
            design.roster.iter().map(|_| None).collect()
        }
    };
    cat.ready = true;
}

fn resolve(catalog: &CreatureCatalog, roster: &[super::level::Species]) -> Vec<Option<Resolved>> {
    // For each model path the roster names, the display that wears it: one with a skin texture
    // first (an unskinned display of a skinned model draws untextured), then the lowest id.
    let wanted: Vec<String> = roster
        .iter()
        .filter_map(|s| match &s.look {
            Look::ModelPath(p) => Some(norm(p)),
            Look::DisplayId(_) => None,
        })
        .collect();
    let mut by_path: HashMap<String, (bool, u32)> = HashMap::new();
    for (display, _) in catalog.display_models() {
        let Some(m) = catalog.model(display) else {
            continue;
        };
        if m.scale <= 0.0 || m.npc_appearance.is_some() {
            continue;
        }
        let key = norm(&m.model_path);
        if !wanted.contains(&key) {
            continue;
        }
        let skinned = m.textures[0].is_some();
        let better = match by_path.get(&key) {
            None => true,
            Some(&(had_skin, had)) => {
                (skinned && !had_skin) || (skinned == had_skin && display < had)
            }
        };
        if better {
            by_path.insert(key, (skinned, display));
        }
    }

    roster
        .iter()
        .map(|s| {
            let display = match &s.look {
                Look::DisplayId(d) => Some(*d),
                Look::ModelPath(p) => by_path.get(&norm(p)).map(|&(_, d)| d),
            };
            let resolved = display.and_then(|d| resolve_one(catalog, d));
            if resolved.is_none() {
                warn!("azeroll: no creature display for {}", s.name);
            }
            resolved
        })
        .collect()
}

fn resolve_one(catalog: &CreatureCatalog, display: u32) -> Option<Resolved> {
    let m = catalog.model(display)?;
    if m.scale <= 0.0 {
        return None;
    }
    Some(Resolved {
        display,
        model_path: m.model_path,
        scale: m.scale,
        height: m.collision_height,
    })
}

// ---------------------------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------------------------

/// Roll this run's spawn points. After [`super::reset_run`], which despawns the last run's units.
fn plan_spawns(
    mut pending: ResMut<Pending>,
    mut rng: ResMut<Rng>,
    design: Res<super::design::Design>,
) {
    let start = wow_to_bevy(START);
    let mut plan = Vec::new();
    for (i, s) in design.roster.iter().enumerate() {
        for _ in 0..s.count {
            let pos = match s.place {
                Place::Around { at, radius } => {
                    let r = rng.range(0.0, radius * radius).sqrt();
                    wow_to_bevy(at) + on_circle(rng.range(0.0, std::f32::consts::TAU)) * r
                }
            };
            let elite = s.kind != Kind::Boss && rng.f32() < s.elite;
            plan.push(Planned {
                species: i,
                pos,
                elite,
            });
        }
    }
    plan.sort_by(|a, b| {
        flat(a.pos - start)
            .length_squared()
            .total_cmp(&flat(b.pos - start).length_squared())
    });
    pending.0 = plan;
}

/// The unit vector at `angle` in the ground plane.
fn on_circle(angle: f32) -> Vec3 {
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

/// Spawn the pending creatures whose ground has streamed in and whose model has loaded.
#[allow(clippy::too_many_arguments)]
fn spawn_creatures(
    mut commands: Commands,
    design: Res<super::design::Design>,
    mut pending: ResMut<Pending>,
    mut cat: ResMut<Catalog>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    asset_server: Res<AssetServer>,
    models: Res<Assets<M2Model>>,
    world: WorldPoint,
) {
    if !cat.ready || pending.0.is_empty() {
        return;
    }
    let cat = &mut *cat;
    let mut spawned = 0;
    let mut scanned = 0;
    let mut i = 0;
    while i < pending.0.len() && spawned < SPAWNS_PER_FRAME && scanned < SCANS_PER_FRAME {
        scanned += 1;
        let Planned {
            species,
            pos,
            elite,
        } = pending.0[i];
        let spec = &design.roster[species];
        let Some(res) = cat.species.get(species).cloned().flatten() else {
            // Unresolvable: never spawns.
            pending.0.remove(i);
            continue;
        };
        let Some(ground) = world.terrain_height_under(pos) else {
            i += 1;
            continue;
        };

        // The model: its bounds size the unit, so it spawns once they are known.
        let url = m2_url(&res.model_path);
        let handle = cat
            .models
            .entry(url.clone())
            .or_insert_with(|| asset_server.load(url))
            .clone();
        let model = models.get(&handle);
        if model.is_none() && !matches!(asset_server.load_state(&handle), LoadState::Failed(_)) {
            i += 1;
            continue;
        }
        // The header sphere covers every animation (a rat's is 1.3 yd, a frog's 1.3: leaps and
        // flails), so it sizes a creature three times too big. What the ball meets is the Stand
        // pose: half the diagonal of its box, standing on the feet.
        let (radius, center, fit_radius) = match model.and_then(|m| m.bounds) {
            Some(b) if b.sphere_radius > 0.0 => {
                let [dx, dy, dz] = b.stand_extent;
                let stand = 0.5 * (dx * dx + dy * dy + dz * dz).sqrt();
                if stand > 0.0 {
                    (stand, wow_to_bevy([0.0, 0.0, dz * 0.5]), b.sphere_radius)
                } else {
                    let mid = [
                        (b.bbox_min[0] + b.bbox_max[0]) * 0.5,
                        (b.bbox_min[1] + b.bbox_max[1]) * 0.5,
                        (b.bbox_min[2] + b.bbox_max[2]) * 0.5,
                    ];
                    (b.sphere_radius, wow_to_bevy(mid), b.sphere_radius)
                }
            }
            // No bounds: half the collision height around a point half way up.
            _ => {
                let half = res.height.max(0.4) * 0.5;
                (half, Vec3::new(0.0, half, 0.0), half)
            }
        };
        let half_extents = model
            .and_then(|m| m.bounds)
            .map_or(Vec3::splat(radius), |b| {
                let extent = wow_to_bevy(b.stand_extent).abs() * 0.5;
                if extent.min_element() > 0.0 {
                    extent
                } else {
                    Vec3::splat(radius)
                }
            });
        // Fits now name the maximum full axis too, so visual size and the gate agree.
        let scale = spec.fit.map_or_else(
            || res.scale * spec.scale * if elite { ELITE_SCALE } else { 1.0 },
            |axis| axis / (2.0 * half_extents.max_element()),
        );
        let size = radius * scale;
        let pickup = 2.0 * half_extents.max_element() * scale;
        if std::env::var_os("WOW_AZEROLL_DUMP").is_some() {
            info!(
                "azeroll dump: {} scale={scale:.3} size={size:.3} (sphere={fit_radius:.2})",
                spec.name
            );
        }
        let heavy = spec.behavior == Behavior::Hostile || spec.kind == Kind::Boss;

        pending.0.remove(i);
        spawned += 1;
        let at = Vec3::new(pos.x, ground, pos.z);
        if spec.kind == Kind::Boss {
            info!("azeroll: Hogger spawned at {:?}, pickup={pickup}", at);
        }
        let entity = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: run.guid(),
                display: res.display,
                scale,
                level: 1,
                faction: if heavy {
                    units::FACTION_MONSTER
                } else {
                    units::FACTION_FRIENDLY
                },
                bytes0: 1 << 8,
            },
            at,
            rng.range(0.0, std::f32::consts::TAU),
        );
        let name: Cow<'static, str> = if elite {
            Cow::Owned(format!("Elite {}", spec.name))
        } else {
            Cow::Owned(spec.name.clone())
        };
        commands.entity(entity).insert((
            Collectible {
                name,
                kind: spec.kind,
                source: Source::Spawned,
                size,
                pickup,
                half_extents,
                center,
                // Never a wall: a hostile that blocked the ball pinned it against geometry.
                // It rams ([`Ram`]) and backs off instead.
                solid: 0.0,
            },
            Brain {
                behavior: spec.behavior,
                home: at,
                leash: if spec.kind == Kind::Boss {
                    BOSS_RANGE
                } else {
                    WANDER_RANGE
                },
                walk: spec.walk,
                run: spec.run,
                heading: Vec3::X,
                walking: false,
                timer: rng.range(0.0, 3.0),
                charge: 0.0,
                cooldown: 0.0,
                retreat: 0.0,
                reaction: Reaction::Idle,
                probe: rng.range(0.0, PROBE_EVERY),
                steer: Vec3::ZERO,
                steer_left: 0.0,
                cower: 0.0,
            },
        ));
        if spec.kind == Kind::Boss {
            commands.entity(entity).insert(Stage::new());
        }
    }
}

/// A rolled-up creature is stuck to the ball and no longer a runner: stop its walk cycle.
fn on_collected(mut collected: MessageReader<Collected>, mut movers: Query<&mut MovementState>) {
    for c in collected.read() {
        if c.kind == Kind::Prop {
            continue;
        }
        if let Ok(mut mv) = movers.get_mut(c.entity) {
            *mv = MovementState::default();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Behaviour
// ---------------------------------------------------------------------------------------------

/// What the look-ahead found.
enum Ahead {
    Clear,
    /// A wall; this is the clear heading along it.
    Turn(Vec3),
    /// A wall with no clear way round on either side.
    Blocked,
}

/// One ray ahead of a creature at its own mid-height, and when it hits something wall-like, one
/// ray each way along the wall. Terrain slopes (normal mostly up) are not walls: the ground
/// height handles those.
fn look_ahead(collision: &WorldCollision, pos: Vec3, dir: Vec3, speed: f32, size: f32) -> Ahead {
    let origin = pos + Vec3::Y * (size * 0.5).clamp(0.25, 1.0);
    let look = size.max(0.3) + speed * PROBE_AHEAD + 0.4;
    let wall = |d: Vec3| {
        Dir3::new(d)
            .ok()
            .and_then(|d| collision.ray_body(origin, d, look))
            .filter(|hit| hit.normal.y.abs() < 0.6)
    };
    let Some(hit) = wall(dir) else {
        return Ahead::Clear;
    };
    let mut along = Vec3::new(-hit.normal.z, 0.0, hit.normal.x).normalize_or_zero();
    if along == Vec3::ZERO {
        along = Vec3::new(-dir.z, 0.0, dir.x);
    }
    if along.dot(dir) < 0.0 {
        along = -along;
    }
    [along, -along]
        .into_iter()
        .find(|d| wall(*d).is_none())
        .map_or(Ahead::Blocked, Ahead::Turn)
}

/// Counters for the cheap parts of the AI, logged every few seconds, and the creature state
/// changes already reported (one line per species and reaction, bosses always).
#[derive(Default)]
struct AiStats {
    probes: u32,
    turns: u32,
    blocked: u32,
    micros: u128,
    since: f32,
    seen: std::collections::HashSet<(String, Reaction)>,
}

/// Wander, flee and chase. Anything stuck to the ball ([`Attached`]), or flying off it after a
/// bump ([`Shed`], which owns its transform until it lands), is left alone.
///
/// What a creature does about the ball is [`react`]'s call, made from [`contact`]; this function
/// only carries it out, with the gaits capped so a runner stays catchable and a look ahead so
/// nothing walks through a wall.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn creature_ai(
    time: Res<Time>,
    design: Res<super::design::Design>,
    ball: Res<BallState>,
    mut rng: ResMut<Rng>,
    world: WorldPoint,
    collision: WorldCollision,
    mut rams: MessageWriter<Ram>,
    mut anim: MessageWriter<EmoteAnim>,
    mut seq: ResMut<PlaySeq>,
    mut stats: Local<AiStats>,
    mut creatures: Query<
        (
            Entity,
            &mut Transform,
            &mut MovementState,
            &mut Brain,
            &Collectible,
            Option<&mut Stage>,
        ),
        (Without<Attached>, Without<Shed>),
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Far from the ball nothing is seen or simulated; they stand where they are.
    let far = 220.0 + 10.0 * ball.radius;
    let probe_range = PROBE_RANGE + 10.0 * ball.radius;
    for (entity, mut tf, mut mv, mut brain, col, mut stage) in &mut creatures {
        // Hogger's pass is [`hogger_pass`]'s; he is ours again once he is home.
        if stage.as_ref().is_some_and(|s| s.beat != Beat::Camp) {
            continue;
        }
        let pos = tf.translation;
        let state = contact(ball.radius, col.pickup);
        // The ball must also be level with the creature to ram it.
        let level = (ball.pos.y - tf.transform_point(col.center).y).abs() < ball.radius + col.size;
        if col.kind == Kind::Boss {
            brain.home = wow_to_bevy(design.camp);
        }
        if let Some(stage) = stage.as_deref_mut() {
            stage.roar_next -= dt;
        }
        let to_ball = flat(ball.pos - pos);
        let dist = to_ball.length();
        if dist > far {
            set_gait(&mut mv, 0.0);
            continue;
        }
        let toward = if dist > 1e-3 { to_ball / dist } else { Vec3::X };
        let scare = if col.kind == Kind::Boss {
            BOSS_SCARE + 3.0 * ball.radius
        } else {
            6.0 + 3.0 * ball.radius + col.size
        };
        let aggro = 18.0 + 6.0 * ball.radius + col.size;
        brain.cooldown = (brain.cooldown - dt).max(0.0);
        brain.retreat = (brain.retreat - dt).max(0.0);
        brain.steer_left = (brain.steer_left - dt).max(0.0);
        brain.cower = (brain.cower - dt).max(0.0);
        brain.probe -= dt;

        let wanted = react(brain.behavior, ball.radius, col.pickup);
        let holding =
            col.kind == Kind::Boss && matches!(brain.reaction, Reaction::BackOff | Reaction::Flee);
        let in_range = dist
            < if wanted == Reaction::Charge {
                aggro
            } else if holding {
                scare + BOSS_HYSTERESIS
            } else {
                scare
            };
        let wanted = if in_range { wanted } else { Reaction::Idle };
        // A hostile attacks in bursts: charge, one ram, back off, ignore the ball for a while.
        let mut chases = wanted == Reaction::Charge
            && brain.retreat <= 0.0
            && brain.cooldown <= 0.0
            && flat(pos - brain.home).length() < CHASE_LEASH;
        if chases {
            brain.charge += dt;
            if dist <= ball.radius + col.size * RAM_REACH && level {
                rams.write(Ram {
                    entity,
                    name: col.name.clone(),
                    dir: toward,
                });
                brain.retreat = RETREAT_SECS;
            }
            if brain.retreat > 0.0 || brain.charge > CHARGE_MAX {
                brain.charge = 0.0;
                brain.cooldown = rng.range(COOLDOWN_SECS.0, COOLDOWN_SECS.1);
                chases = false;
            }
        } else {
            brain.charge = 0.0;
        }
        // After a ram a hostile still gives ground, unless the ball has since grown to eat it.
        let backs_off = wanted == Reaction::BackOff
            || (brain.behavior == Behavior::Hostile
                && brain.retreat > 0.0
                && wanted != Reaction::Flee);
        let flees = wanted == Reaction::Flee;
        let notices = wanted == Reaction::Notice;
        let reaction = if flees {
            Reaction::Flee
        } else if backs_off {
            Reaction::BackOff
        } else if chases {
            Reaction::Charge
        } else if notices {
            Reaction::Notice
        } else {
            Reaction::Idle
        };
        if reaction != brain.reaction {
            if stats.seen.insert((col.name.to_string(), reaction)) || col.kind == Kind::Boss {
                info!(
                    "azeroll creature: {} {:?} -> {:?} contact={state:?} D={:.2} pickup={:.2} dist={dist:.1}",
                    col.name,
                    brain.reaction,
                    reaction,
                    ball.diameter(),
                    col.pickup
                );
            }
            if let Some(stage) = stage.as_deref_mut() {
                match reaction {
                    // The ball is too much for him but not yet food: he roars, standing his
                    // ground a moment, then gives it room.
                    Reaction::BackOff if stage.roar_next <= 0.0 => {
                        stage.roar_hold = ROAR_HOLD;
                        stage.roar_next = ROAR_EVERY;
                        play(&mut anim, &mut seq, entity, ANIM_ROAR);
                    }
                    Reaction::Flee => stage.panic = 0.0,
                    _ => {}
                }
            }
            brain.reaction = reaction;
        }

        let mut dir = Vec3::ZERO;
        let mut speed = 0.0;
        let mut wandering = false;
        let mut backwards = false;
        if flees || backs_off {
            dir = -toward;
            speed = if flees {
                // Fast, but never as fast as the ball: it stays catchable.
                brain.run.min(FLEE_SPEED_CAP * top_speed(ball.radius))
            } else if stage.is_some() {
                BACK_OFF_SPEED
            } else {
                brain.walk * 2.0
            };
            if let Some(stage) = stage.as_deref_mut() {
                if flees {
                    // Comic panic: a zigzag across the line of flight, a stumble every few
                    // seconds and flapping arms. Never faster than [`FLEE_SPEED_CAP`], so the
                    // ball can always run him down.
                    let before = (stage.panic / PANIC_EMOTE).floor();
                    stage.panic += dt;
                    let side = Vec3::new(-toward.z, 0.0, toward.x);
                    dir = (dir + side * (stage.panic * 3.1).sin() * 0.9).normalize_or(dir);
                    if (stage.panic + 0.9).rem_euclid(3.4) < 0.6 {
                        speed = 0.0;
                    }
                    if (stage.panic / PANIC_EMOTE).floor() != before {
                        play(&mut anim, &mut seq, entity, ANIM_PANIC);
                    }
                } else {
                    // He faces the ball and backs away from it, roaring as he goes.
                    backwards = true;
                    stage.roar_hold = (stage.roar_hold - dt).max(0.0);
                    if stage.roar_hold > 0.0 {
                        speed = 0.0;
                    } else if stage.roar_next <= 0.0 {
                        stage.roar_next = ROAR_EVERY;
                        stage.roar_hold = ROAR_HOLD * 0.6;
                        play(&mut anim, &mut seq, entity, ANIM_ROAR);
                    }
                }
            }
            // Cornered at the edge of its range: slide along it rather than run on.
            let out = flat(pos - brain.home);
            if out.length() > brain.leash * 1.5 && out.dot(dir) > 0.0 {
                let radial = out.normalize_or_zero();
                let tangent = Vec3::new(-radial.z, 0.0, radial.x);
                dir = if tangent.dot(dir) < 0.0 {
                    -tangent
                } else {
                    tangent
                };
            }
        } else if chases {
            dir = toward;
            speed = brain.run;
        } else if notices {
            // Stops and watches the ball that is about to be able to knock it loose.
            tf.rotation = tf.rotation.slerp(
                Quat::from_rotation_y(yaw_toward(toward)),
                (dt * 6.0).min(1.0),
            );
        } else {
            let home = flat(brain.home - pos);
            let home_dist = home.length();
            if home_dist > brain.leash * 1.2 {
                // Out of range (a chase ended): go back, at a run if it is a long way.
                dir = home / home_dist;
                speed = if home_dist > brain.leash * 2.0 {
                    brain.run
                } else {
                    brain.walk
                };
                brain.walking = false;
                brain.timer = 0.0;
            } else if let Some(stage) = stage.as_deref_mut() {
                // Hogger does not wander: he walks his camp's loop, where the approach sees him.
                let camp = wow_to_bevy(design.camp);
                let to_point = flat(patrol_point(camp, stage.waypoint) - pos);
                stage.leg += dt;
                if to_point.length() < 2.0 || stage.leg > 14.0 {
                    stage.waypoint = (stage.waypoint + 1) % PATROL_POINTS;
                    stage.leg = 0.0;
                } else {
                    dir = to_point.normalize_or_zero();
                    speed = PATROL_SPEED;
                }
            } else {
                brain.timer -= dt;
                if brain.timer <= 0.0 {
                    if brain.walking {
                        brain.walking = false;
                        brain.timer = rng.range(1.5, 5.0);
                    } else {
                        brain.walking = true;
                        brain.timer = rng.range(1.0, 3.5);
                        let mut heading = on_circle(rng.range(0.0, std::f32::consts::TAU));
                        if home_dist > brain.leash * 0.6 {
                            // Drifting out: lean towards home.
                            heading = (home / home_dist + heading * 0.5).normalize_or_zero();
                        }
                        brain.heading = heading;
                    }
                }
                if brain.walking {
                    dir = brain.heading;
                    speed = brain.walk;
                    wandering = true;
                }
            }
        }

        // Geometry: a creature that is cowering stands; one that has just turned from a wall keeps
        // the turn for a moment; and every quarter second, if it is anywhere near the ball, it
        // looks ahead for the next wall.
        if speed > 0.0 && dir != Vec3::ZERO {
            if brain.cower > 0.0 {
                speed = 0.0;
            } else {
                if brain.steer_left > 0.0 {
                    dir = (brain.steer + dir * 0.25).normalize_or(brain.steer);
                }
                if brain.probe <= 0.0 && dist < probe_range {
                    brain.probe = PROBE_EVERY;
                    let started = Instant::now();
                    let ahead = look_ahead(&collision, pos, dir, speed, col.size);
                    stats.micros += started.elapsed().as_micros();
                    stats.probes += 1;
                    match ahead {
                        Ahead::Clear => {}
                        Ahead::Turn(along) => {
                            stats.turns += 1;
                            brain.steer = along;
                            brain.steer_left = STEER_SECS;
                            dir = along;
                            if wandering {
                                brain.heading = along;
                            }
                        }
                        Ahead::Blocked => {
                            stats.blocked += 1;
                            brain.cower = COWER_SECS;
                            speed = 0.0;
                            if wandering {
                                brain.walking = false;
                                brain.timer = COWER_SECS;
                            }
                        }
                    }
                }
            }
        }

        if backwards {
            tf.rotation = tf.rotation.slerp(
                Quat::from_rotation_y(yaw_toward(toward)),
                (dt * 8.0).min(1.0),
            );
        }
        if speed > 0.0 && dir != Vec3::ZERO {
            if !backwards {
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
            }
            let next = pos + dir * speed * dt;
            match world.terrain_height_under(next) {
                Some(ground) if (ground - pos.y).abs() <= STEP_LIMIT + speed * dt => {
                    tf.translation = Vec3::new(next.x, ground, next.z);
                }
                // A cliff face: stand rather than climb it.
                Some(_) => {
                    brain.cower = COWER_SECS * 0.5;
                    speed = 0.0;
                }
                // Off the streamed ground: stand still rather than walk into the void.
                None => speed = 0.0,
            }
        } else {
            speed = 0.0;
        }
        if backwards && speed > 0.0 {
            set_motion(&mut mv, speed, move_flags::BACKWARD);
        } else {
            set_gait(&mut mv, speed);
        }
    }
    stats.since += dt;
    if stats.since >= 10.0 {
        if stats.probes > 0 {
            info!(
                "azeroll creature probes: {} in {:.0}s ({} turns, {} blocked), {:.1} us each",
                stats.probes,
                stats.since,
                stats.turns,
                stats.blocked,
                stats.micros as f64 / f64::from(stats.probes)
            );
        }
        stats.probes = 0;
        stats.turns = 0;
        stats.blocked = 0;
        stats.micros = 0;
        stats.since = 0.0;
    }
}

/// Which way he lumbers home. Away from the ball while it is close, skirting it on the side the
/// camp is on, so he never walks back through the ball (or the camera behind it); straight for the
/// camp once clear. `home` is the way to the camp, `away` the unit vector from the ball to him.
fn lumber_dir(home: Vec3, away: Vec3, dist: f32, radius: f32) -> Vec3 {
    let home = home.normalize_or_zero();
    let near = 14.0 + 2.0 * radius;
    if dist >= near || home.dot(away) > 0.5 {
        return home;
    }
    let side = Vec3::new(-away.z, 0.0, away.x);
    let side = if side.dot(home) < 0.0 { -side } else { side };
    let closeness = ((near - dist) / near).clamp(0.0, 1.0);
    (home * (1.0 - closeness) + (away * 0.8 + side * 0.6) * closeness).normalize_or(away)
}

/// `WOW_AZEROLL_CAMP_TEST=<yards>`: a capture fixture. A second into play the ball is set down
/// that far from the camp on the approach from the village, facing it, at whatever radius the run
/// started with (`WOW_AZEROLL_DEBUG_RADIUS`), then held there and walked in at a yard a second
/// down to eight, whatever the autopilot does, so Hogger's staging can be seen at each size without
/// rolling there.
fn camp_test(
    time: Res<Time>,
    run: Res<Run>,
    design: Res<super::design::Design>,
    mut ball: ResMut<BallState>,
    world: WorldPoint,
    mut at: Local<Option<f32>>,
    mut since: Local<f32>,
    hogger: Query<(&Transform, &Brain), With<Stage>>,
) {
    let Some(start) =
        crate::webenv::var("WOW_AZEROLL_CAMP_TEST").and_then(|v| v.parse::<f32>().ok())
    else {
        return;
    };
    if run.elapsed < 1.0 {
        return;
    }
    let yards = (*at.get_or_insert(start) - 1.0 * time.delta_secs()).max(8.0);
    *at = Some(yards);
    let camp = wow_to_bevy(design.camp);
    // Off the straight line from the village and looking a little to one side, so the clump is not
    // between the camera and him.
    let away =
        Quat::from_rotation_y(0.55) * flat(wow_to_bevy(design.start) - camp).normalize_or(Vec3::X);
    let spot = camp + away * yards;
    let Some(ground) = world.terrain_height_under(spot) else {
        return;
    };
    ball.pos = Vec3::new(spot.x, ground + ball.radius, spot.z);
    ball.vel = Vec3::ZERO;
    ball.heading = yaw_toward(-away) + 0.5;
    // Where Hogger is, for choosing frames: yards from the ball and degrees off the view.
    *since += time.delta_secs();
    if *since >= 1.0 {
        *since = 0.0;
        for (tf, brain) in &hogger {
            let to = flat(tf.translation - ball.pos);
            let off = to
                .normalize_or(Vec3::X)
                .dot(-away.normalize_or(Vec3::X))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            info!(
                "azeroll camp test: Hogger {:.1} yd from the ball, {off:.0} deg off the view, {:?}",
                to.length(),
                brain.reaction
            );
        }
    }
}

/// One scripted step: `dir` at `speed` with the wanderers' look ahead for geometry and the same
/// cliff rule. Returns the speed made good (0 when blocked or cowering).
#[allow(clippy::too_many_arguments)]
fn walk_step(
    brain: &mut Brain,
    tf: &mut Transform,
    world: &WorldPoint,
    collision: &WorldCollision,
    mut dir: Vec3,
    mut speed: f32,
    size: f32,
    dt: f32,
) -> f32 {
    brain.steer_left = (brain.steer_left - dt).max(0.0);
    brain.cower = (brain.cower - dt).max(0.0);
    brain.probe -= dt;
    if brain.cower > 0.0 || dir == Vec3::ZERO {
        return 0.0;
    }
    if brain.steer_left > 0.0 {
        dir = (brain.steer + dir * 0.25).normalize_or(brain.steer);
    }
    if brain.probe <= 0.0 {
        brain.probe = PROBE_EVERY;
        match look_ahead(collision, tf.translation, dir, speed, size) {
            Ahead::Clear => {}
            Ahead::Turn(along) => {
                brain.steer = along;
                brain.steer_left = STEER_SECS;
                dir = along;
            }
            Ahead::Blocked => {
                brain.cower = COWER_SECS;
                return 0.0;
            }
        }
    }
    let target = Quat::from_rotation_y(yaw_toward(dir));
    tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
    let next = tf.translation + dir * speed * dt;
    match world.terrain_height_under(next) {
        Some(ground) if (ground - tf.translation.y).abs() <= STEP_LIMIT + speed * dt => {
            tf.translation = Vec3::new(next.x, ground, next.z);
        }
        Some(_) => {
            brain.cower = COWER_SECS * 0.5;
            speed = 0.0;
        }
        None => speed = 0.0,
    }
    speed
}

/// Whether something solid roofs `at`: the ball is inside a building, where Hogger's pass would
/// not be seen.
fn roofed(collision: &WorldCollision, at: Vec3) -> bool {
    collision
        .ray_body(at + Vec3::Y * 0.3, Dir3::Y, 14.0)
        .is_some_and(|hit| hit.normal.y < -0.4)
}

/// Hogger's early pass, the foreshadow: while the ball is still too small to take him and is
/// outdoors in the village, he is set down at the edge of the view ahead of it, charges, rams it if
/// he catches it (a growl and a knock through [`Ram`]), then lumbers off toward the camp and is
/// put back there once he is out of sight. He is never forced on the player: the pass happens at
/// most once, and a ball that outruns the charge simply watches him give up.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn hogger_pass(
    time: Res<Time>,
    design: Res<super::design::Design>,
    run: Res<Run>,
    ball: Res<BallState>,
    rig: Option<Res<super::camera::CamRig>>,
    world: WorldPoint,
    collision: WorldCollision,
    mut rams: MessageWriter<Ram>,
    mut anim: MessageWriter<EmoteAnim>,
    mut seq: ResMut<PlaySeq>,
    mut hogger: Query<
        (
            Entity,
            &mut Transform,
            &mut MovementState,
            &mut Brain,
            &Collectible,
            &mut Stage,
        ),
        (Without<Attached>, Without<Shed>),
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let camp = wow_to_bevy(design.camp);
    for (entity, mut tf, mut mv, mut brain, col, mut stage) in &mut hogger {
        let pos = tf.translation;
        let to_ball = flat(ball.pos - pos);
        let dist = to_ball.length();
        let toward = to_ball.normalize_or(Vec3::X);
        stage.t += dt;
        match stage.beat {
            Beat::Camp => {
                if stage.visited || run.elapsed < design.patrol_at.max(VISIT_FROM) {
                    continue;
                }
                // The way the camera looks, and whether it has been looking that way a while.
                let view = ball.heading + rig.as_ref().map_or(0.0, |r| r.yaw);
                let turned = wrap_pi(view - stage.view).abs() / dt;
                stage.view = view;
                stage.steady = if turned < VISIT_STEADY_RATE {
                    stage.steady + dt
                } else {
                    0.0
                };
                if run.elapsed > design.patrol_until + VISIT_GRACE
                    || contact(ball.radius, col.pickup) != Contact::Wall
                {
                    info!(
                        "azeroll: Hogger's pass missed its window (t={:.0}s)",
                        run.elapsed
                    );
                    stage.visited = true;
                    continue;
                }
                // Look for a place to come in only a few times a second, and only with the ball
                // in the village and out of doors.
                stage.leg -= dt;
                if stage.leg > 0.0 {
                    continue;
                }
                stage.leg = 0.3;
                if flat(ball.pos - wow_to_bevy(design.patrol)).length() > VISIT_VILLAGE
                    || stage.steady < VISIT_STEADY
                    || roofed(&collision, ball.pos)
                {
                    continue;
                }
                let (s, c) = stage.view.sin_cos();
                let forward = Vec3::new(-s, 0.0, -c);
                let eye = ball.pos + Vec3::Y;
                let entry = VISIT_ENTRIES.iter().find_map(|&(off, yards)| {
                    let way = Quat::from_rotation_y(off) * forward;
                    let at = ball.pos + way * yards;
                    let ground = world.terrain_height_under(at)?;
                    // A clear line from the ball, so the camera has him from the first step.
                    let line = Dir3::new(way).ok()?;
                    collision
                        .ray_body(eye, line, yards)
                        .filter(|hit| hit.normal.y.abs() < 0.6)
                        .is_none()
                        .then_some((Vec3::new(at.x, ground, at.z), yards))
                });
                let Some((at, yards)) = entry else { continue };
                info!(
                    "azeroll: Hogger's pass begins at t={:.0}s, {yards:.0} yd from the ball",
                    run.elapsed
                );
                tf.translation = at;
                tf.rotation = Quat::from_rotation_y(yaw_toward(flat(ball.pos - at)));
                stage.beat = Beat::Visit;
                stage.t = 0.0;
                stage.roared = false;
                stage.roar_hold = 0.0;
                brain.steer_left = 0.0;
                brain.cower = 0.0;
            }
            Beat::Visit => {
                let level =
                    (ball.pos.y - tf.transform_point(col.center).y).abs() < ball.radius + col.size;
                let rammed = dist <= ball.radius + col.size * RAM_REACH && level;
                if rammed {
                    rams.write(Ram {
                        entity,
                        name: col.name.clone(),
                        dir: toward,
                    });
                }
                if rammed
                    || stage.t > VISIT_CHASE
                    || contact(ball.radius, col.pickup) != Contact::Wall
                {
                    info!(
                        "azeroll: Hogger's pass ends after {:.1}s ({}), dist={dist:.1}",
                        stage.t,
                        if rammed { "rammed" } else { "gave up" }
                    );
                    stage.beat = Beat::Return;
                    stage.t = 0.0;
                    set_gait(&mut mv, 0.0);
                    continue;
                }
                if (stage.t * 2.0).floor() != ((stage.t - dt) * 2.0).floor() {
                    let (s, c) = ball.heading.sin_cos();
                    let off = (flat(pos - ball.pos).normalize_or(Vec3::X))
                        .dot(Vec3::new(-s, 0.0, -c))
                        .clamp(-1.0, 1.0)
                        .acos()
                        .to_degrees();
                    info!(
                        "azeroll: Hogger charging, {dist:.1} yd, {off:.0} deg off the ball's heading"
                    );
                }
                // Aim a little ahead of a moving ball.
                let aim = flat(ball.pos + flat(ball.vel) * 0.6 - pos).normalize_or(toward);
                // Stalks in, stops to roar at the edge of the ball's reach, then charges.
                if !stage.roared && dist <= VISIT_ROAR {
                    stage.roared = true;
                    stage.roar_hold = ROAR_HOLD;
                    play(&mut anim, &mut seq, entity, ANIM_ROAR);
                }
                if stage.roar_hold > 0.0 {
                    stage.roar_hold -= dt;
                    tf.rotation = tf.rotation.slerp(
                        Quat::from_rotation_y(yaw_toward(toward)),
                        (dt * 8.0).min(1.0),
                    );
                    set_gait(&mut mv, 0.0);
                    continue;
                }
                let pace = if stage.roared { brain.run } else { VISIT_STALK };
                let speed = walk_step(
                    &mut brain, &mut tf, &world, &collision, aim, pace, col.size, dt,
                );
                set_gait(&mut mv, speed);
            }
            Beat::Return => {
                let home = flat(camp - pos);
                let done = (dist > VISIT_VANISH_DIST && stage.t > VISIT_VANISH_AFTER)
                    || dist > VISIT_VANISH_FAR
                    || home.length() < 3.0;
                if done {
                    // Out of sight: back on his loop.
                    let at = patrol_point(camp, 0);
                    let ground = world.terrain_height_under(at).unwrap_or(camp.y);
                    tf.translation = Vec3::new(at.x, ground, at.z);
                    stage.beat = Beat::Camp;
                    stage.visited = true;
                    stage.t = 0.0;
                    stage.waypoint = 1;
                    brain.cooldown = 8.0;
                    set_gait(&mut mv, 0.0);
                    info!(
                        "azeroll: Hogger is back at the camp (t={:.0}s)",
                        run.elapsed
                    );
                    continue;
                }
                let speed = walk_step(
                    &mut brain,
                    &mut tf,
                    &world,
                    &collision,
                    lumber_dir(home, -toward, dist, ball.radius),
                    LUMBER,
                    col.size,
                    dt,
                );
                set_gait(&mut mv, speed);
            }
        }
    }
}

/// Forward at `speed`, or standing at zero; written only on a change so the animation driver is
/// not woken every frame.
fn set_gait(mv: &mut Mut<MovementState>, speed: f32) {
    set_motion(mv, speed, if speed > 0.0 { move_flags::FORWARD } else { 0 });
}

/// `flags` at `speed`, written only on a change.
fn set_motion(mv: &mut Mut<MovementState>, speed: f32, flags: u32) {
    if mv.speed != speed || mv.flags != flags {
        mv.speed = speed;
        mv.flags = flags;
    }
}

/// A one-shot clip over the unit's gait.
fn play(out: &mut MessageWriter<EmoteAnim>, seq: &mut PlaySeq, entity: Entity, anim_id: u16) {
    out.write(EmoteAnim {
        entity,
        anim_id,
        seq: seq.next(),
        via_player: false,
    });
}

/// Waypoint `k` of Hogger's loop around the camp at `camp`.
fn patrol_point(camp: Vec3, k: usize) -> Vec3 {
    let a = k as f32 * std::f32::consts::TAU / PATROL_POINTS as f32;
    camp + on_circle(a) * PATROL_RADIUS
}

// ---------------------------------------------------------------------------------------------
// The Prince
// ---------------------------------------------------------------------------------------------

/// Fixed-size local pusher. Never collectible and never participates in the bump query.
#[derive(Component)]
pub(super) struct Prince;

/// The Prince's fixed model scale, forever: he is about 0.35 yards tall and never grows,
/// so against the starting ball (0.5 yd across) he is a little figure pushing it, as Katamari's
/// 5 cm Prince is to a 10 cm ball, and a speck against a huge one. The camera does the work.
const PRINCE_SCALE: f32 = 0.25;
/// The gnome at scale 0.8 stood about 1.1 yd tall; the body measurements below were taken there
/// and shrink with the scale.
const BODY: f32 = PRINCE_SCALE / 0.8;
/// Height of his hands and knees above his feet, yards: the two levels at which the clump's
/// surface and its trophies are measured, so his hands meet the ball and his legs clear a log.
const HAND_HEIGHT: f32 = 0.5 * BODY;
const KNEE_HEIGHT: f32 = 0.22 * BODY;
/// From his body's centre to the surface his hands are on, yards.
const BODY_GAP: f32 = 0.3 * BODY;
/// The clump's trophies are measured at this fraction of their bounding sphere: a box fills its
/// sphere's centre, not its corners.
const TROPHY_FIT: f32 = 0.85;
/// The fastest he moves round the clump, yards per second, and the bounds on his angular rate.
const STRAFE_SPEED: f32 = 13.0;
const STRAFE_RATE: (f32, f32) = (1.8, 9.0);
/// His feet's cadence, as the speed the run clip is driven at (the clip runs at that speed over
/// its move speed): a floor so he never walks, a gain with the ball's speed and a ceiling.
const STRIDE_MIN: f32 = 5.2;
const STRIDE_GAIN: f32 = 0.45;
const STRIDE_MAX: f32 = 16.0;
/// Running on the spot before a dash, yards per second of clip speed, and the longest it lasts.
const STRIDE_WINDUP: f32 = 12.0;
const WINDUP_SECS: f32 = 0.45;
/// Forward lean, radians: hands on the clump at rest, at full push, and the extra for a dash and a
/// wind-up.
const LEAN: (f32, f32) = (0.12, 0.36);
const LEAN_DASH: f32 = 0.12;
const LEAN_WINDUP: f32 = 0.16;

/// Which clip holds the Prince's arms while he pushes. `WOW_AZEROLL_PRINCE_POSE=<AnimationData id>`
/// tries another; `run` leaves only the lean. The chosen default is the one whose screenshots read
/// best (see the architecture notes).
fn push_pose() -> Option<u16> {
    match crate::webenv::var("WOW_AZEROLL_PRINCE_POSE").as_deref() {
        Some("run") => None,
        Some(id) => id.parse().ok().or(Some(PUSH_POSE)),
        None => Some(PUSH_POSE),
    }
}
/// ReadySpellDirected: both hands held out in front, the precast pose.
const PUSH_POSE: u16 = 51;

/// The Prince's stagecraft: where he is round the clump, and the smoothed state of his body.
#[derive(Component)]
struct Push {
    /// His bearing from the ball's centre, radians: the direction `(cos, sin)` in (x, z).
    azimuth: f32,
    /// Distance from the ball's centre to his body, yards.
    dist: f32,
    /// Height of his feet.
    feet: f32,
    /// Forward lean, radians.
    lean: f32,
    /// The speed his run clip is driven at, 0 when he stands.
    stride: f32,
    /// Seconds of dash wind-up left: he runs on the spot.
    windup: f32,
    /// The bearing offset he stands at to keep out of a wall, sticky between looks.
    detour: f32,
    /// Seconds until the next look for room at other bearings.
    look: f32,
    /// Where he and the ball were last frame, for the speeds.
    last: Vec3,
    last_ball: Vec3,
    /// His speed relative to the ball, smoothed: circling the clump.
    circling: f32,
}

/// A trophy as the Prince measures it: bounding-sphere centre and radius, world space.
#[derive(Clone, Copy)]
struct Lump {
    centre: Vec3,
    radius: f32,
}

/// How far from the ball's centre, along the ground direction `u`, the Prince's body must stand to
/// have `gap` between it and the ball's surface and every trophy, measured at each of the world
/// heights `bands`. The ball and each trophy are cut at the band into discs; the answer is where
/// the ray from the centre along `u` leaves the last of them.
fn clump_reach(ball: Vec3, radius: f32, u: Vec3, bands: &[f32], gap: f32, lumps: &[Lump]) -> f32 {
    let mut reach = 0.0_f32;
    for &y in bands {
        let dy = y - ball.y;
        if dy.abs() < radius {
            reach = reach.max((radius * radius - dy * dy).sqrt() + gap);
        }
        for l in lumps {
            let dy = y - l.centre.y;
            if dy.abs() >= l.radius {
                continue;
            }
            let disc = (l.radius * l.radius - dy * dy).sqrt() + gap;
            let c = flat(l.centre - ball);
            let along = c.dot(u);
            let lateral = (c - u * along).length();
            if lateral < disc {
                reach = reach.max(along + (disc * disc - lateral * lateral).sqrt());
            }
        }
    }
    reach
}

/// The free run from `from` along `u`, less a body's margin, up to `want`: `want` when nothing
/// wall-like is in the way.
fn room(collision: &WorldCollision, from: Vec3, u: Vec3, want: f32) -> f32 {
    let Ok(dir) = Dir3::new(u) else { return want };
    collision
        .ray_body(from, dir, want + 0.08)
        .filter(|hit| hit.normal.y.abs() < 0.6)
        .map_or(want, |hit| (hit.distance - 0.08).max(0.0))
}

/// The ground direction at bearing `az`.
fn bearing(az: f32) -> Vec3 {
    Vec3::new(az.cos(), 0.0, az.sin())
}

/// The bearing from the ball at which a ball with this heading has its back.
fn behind(heading: f32) -> f32 {
    std::f32::consts::FRAC_PI_2 - heading
}

/// The Prince pushes: he stands at the clump's back, hands on its surface, leaning in; runs in step
/// with the roll, scrambling faster as it grows; strafes round the clump when it turns or flips,
/// following its outline rather than cutting through it; runs on the spot for a dash wind-up;
/// stands leaning when it stops; and keeps out of walls and on the floor. Fixed size, never
/// collectible.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn prince(
    mut commands: Commands,
    time: Res<Time>,
    cat: Res<Catalog>,
    ball: Res<BallState>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    world: WorldPoint,
    collision: WorldCollision,
    mut dashes: MessageReader<Dash>,
    trophies: Query<(&GlobalTransform, &Collectible), With<Attached>>,
    mut lumps: Local<Vec<Lump>>,
    mut prince: Query<(&mut Transform, &mut MovementState, &mut Push), With<Prince>>,
) {
    let mut windup = false;
    for d in dashes.read() {
        windup |= *d == Dash::WindUp;
    }
    if !matches!(
        phase.get(),
        Phase::Ready | Phase::Playing | Phase::Paused | Phase::Results
    ) {
        return;
    }
    let dt = time.delta_secs();
    let radius = ball.radius;
    let Ok((mut tf, mut mv, mut push)) = prince.single_mut() else {
        // Spawn him at the clump's back, once the ground under it is known.
        let Some(res) = &cat.prince else { return };
        let az = behind(ball.heading);
        let at = ball.pos + bearing(az) * (radius + BODY_GAP + 0.1);
        let Some(h) = world.terrain_height_under(at) else {
            return;
        };
        let at = Vec3::new(at.x, h, at.z);
        let entity = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: run.guid(),
                display: res.display,
                scale: PRINCE_SCALE,
                level: 1,
                faction: units::FACTION_FRIENDLY,
                bytes0: 7 | (1 << 8),
            },
            at,
            ball.heading,
        );
        commands.entity(entity).insert((
            Prince,
            Push {
                azimuth: az,
                dist: radius + BODY_GAP,
                feet: h,
                lean: LEAN.0,
                stride: 0.0,
                windup: 0.0,
                detour: 0.0,
                look: 0.0,
                last: at,
                last_ball: ball.pos,
                circling: 0.0,
            },
        ));
        if let Some(anim_id) = push_pose() {
            commands.entity(entity).insert(CastHold {
                anim_id,
                spell_id: u32::MAX,
                ranged: false,
            });
        }
        return;
    };
    if dt <= 0.0 {
        return;
    }
    if windup {
        push.windup = WINDUP_SECS;
    }
    push.windup = (push.windup - dt).max(0.0);

    // The trophies that could be in his way: the ones sticking out from the ball.
    lumps.clear();
    lumps.extend(trophies.iter().filter_map(|(gt, col)| {
        let centre = gt.transform_point(col.center);
        let l = Lump {
            centre,
            radius: col.size * TROPHY_FIT,
        };
        // Wholly inside the ball's own outline, or too far down or up to touch him: skip.
        ((centre - ball.pos).length() + l.radius > radius * 0.9).then_some(l)
    }));

    // Where he wants to be: the back of the clump, unless a wall is there.
    let hand = HAND_HEIGHT.min(radius);
    let bands = |feet: f32| [feet + hand, feet + KNEE_HEIGHT.min(radius)];
    let back = behind(ball.heading);
    let reach_at = |az: f32, feet: f32| {
        clump_reach(
            ball.pos,
            radius,
            bearing(az),
            &bands(feet),
            BODY_GAP,
            &lumps,
        )
        .max(radius * 0.5 + BODY_GAP)
        .min(radius * 2.5 + 2.0)
    };
    let eye = |feet: f32| Vec3::new(ball.pos.x, feet + hand, ball.pos.z);
    push.look -= dt;
    if push.look <= 0.0 {
        push.look = 0.2;
        // Straight behind if there is room, else the nearest bearing round the clump that has it.
        let mut best = (f32::MIN, push.detour);
        for off in [0.0, push.detour, 0.7, -0.7, 1.4, -1.4, 2.1, -2.1, 3.0] {
            let az = back + off;
            let need = reach_at(az, push.feet);
            let slack = room(&collision, eye(push.feet), bearing(az), need) - need;
            // A bearing with room wins at once; otherwise the one with the least shortfall.
            if slack >= -0.01 {
                best = (0.0, off);
                break;
            }
            if slack > best.0 {
                best = (slack, off);
            }
        }
        push.detour = best.1;
    }
    let want = back + push.detour;

    // Round the clump no faster than he can run, easing in, and out along its outline.
    let rate = (STRAFE_SPEED / push.dist.max(0.3)).clamp(STRAFE_RATE.0, STRAFE_RATE.1);
    let gap = wrap_pi(want - push.azimuth);
    let step = (gap * (1.0 - (-10.0 * dt).exp())).clamp(-rate * dt, rate * dt);
    push.azimuth = wrap_pi(push.azimuth + step);

    // How far out: where the outline is at the bearing he is on, out at once and in slowly, and
    // short of any wall.
    let u = bearing(push.azimuth);
    let need = reach_at(push.azimuth, push.feet);
    let free = room(&collision, eye(push.feet), u, need);
    let goal = need.min(free).max(radius * 0.5);
    let k = if goal > push.dist { 30.0 } else { 6.0 };
    push.dist += (goal - push.dist) * (1.0 - (-k * dt).exp());
    let flat_at = ball.pos + u * push.dist;

    // Feet: the terrain, or a floor a step above where he last stood (the same probe the ball
    // rests on, so a hall floor, a porch or a stair holds him), whichever is higher.
    let Some(ground) = world.terrain_height_under(flat_at) else {
        return;
    };
    let base = push.feet.max(ball.pos.y - radius) + 0.05;
    let floor = floor_rest(&collision, Vec3::new(flat_at.x, base, flat_at.z), 0.05)
        .map(|centre| centre - 0.05);
    let target = floor.map_or(ground, |f| f.max(ground));
    push.feet = if (target - push.feet).abs() > 1.5 {
        target
    } else {
        push.feet + (target - push.feet) * (1.0 - (-18.0 * dt).exp())
    };
    let at = Vec3::new(flat_at.x, push.feet, flat_at.z);

    // Speeds: the ball's, and his own round it.
    let rolling = flat(ball.vel).length();
    let own = flat(at - push.last) - flat(ball.pos - push.last_ball);
    let own = (own.length() / dt).min(40.0);
    push.circling += (own - push.circling) * (1.0 - (-12.0 * dt).exp());
    push.last = at;
    push.last_ball = ball.pos;

    // Feet: running in step with the roll, faster as it grows (a ball's top speed is 10 times a
    // small one's, his legs three), flat out for a wind-up, and round the clump on a turn.
    let dashing = ball.dash > 0.0;
    let wanted = if push.windup > 0.0 {
        STRIDE_WINDUP
    } else if rolling > 0.25 || push.circling > 1.2 {
        let mut stride = (STRIDE_MIN + STRIDE_GAIN * rolling).min(STRIDE_MAX);
        stride = stride.max(STRIDE_MIN + push.circling);
        if dashing {
            stride = (stride * 1.3).min(STRIDE_MAX * 1.2);
        }
        stride
    } else {
        0.0
    };
    push.stride = if wanted == 0.0 {
        0.0
    } else if push.stride == 0.0 {
        wanted
    } else {
        push.stride + (wanted - push.stride) * (1.0 - (-8.0 * dt).exp())
    };
    set_gait(&mut mv, push.stride);

    // Lean into it: a little at rest, more with speed, most for a dash or a wind-up.
    let fraction = (rolling / top_speed(radius).max(0.1)).clamp(0.0, 1.2);
    let lean = LEAN.0
        + (LEAN.1 - LEAN.0) * fraction.min(1.0)
        + if dashing { LEAN_DASH } else { 0.0 }
        + if push.windup > 0.0 { LEAN_WINDUP } else { 0.0 };
    push.lean += (lean - push.lean) * (1.0 - (-8.0 * dt).exp());

    // Facing the ball's centre, hands toward it, tilted from the feet.
    let yaw = Quat::from_rotation_y(yaw_toward(flat(ball.pos - at)));
    let current = {
        // The yaw part of last frame's rotation, so the lean never feeds back into the turn.
        let (y, _, _) = tf.rotation.to_euler(EulerRot::YXZ);
        Quat::from_rotation_y(y)
    };
    let yawed = current.slerp(yaw, (dt * 14.0).min(1.0));
    tf.translation = at;
    tf.rotation = yawed * Quat::from_rotation_x(-push.lean);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::azeroll::level::{COLLECT_RATIO, DISLODGE_RATIO};

    /// A ball radius sitting exactly at each boundary for this axis.
    fn roll_up_radius(axis: f32) -> f32 {
        axis / COLLECT_RATIO
    }
    fn dislodge_radius(axis: f32) -> f32 {
        DISLODGE_RATIO * axis / 2.0
    }

    #[test]
    fn passive_ignores_everything() {
        for r in [0.2, 1.0, 5.0, 20.0] {
            assert_eq!(react(Behavior::Passive, r, 1.0), Reaction::Idle);
        }
    }

    #[test]
    fn hostile_follows_the_contact_state_at_every_representative() {
        // Rabbit, cow, adult NPC-sized, gnoll, Hogger.
        for axis in [0.48, 1.2, 2.01, 2.8, 4.0] {
            let up = roll_up_radius(axis);
            let dis = dislodge_radius(axis);
            assert_eq!(react(Behavior::Hostile, up * 1.001, axis), Reaction::Flee);
            assert_eq!(
                react(Behavior::Hostile, up * 0.999, axis),
                Reaction::BackOff
            );
            assert_eq!(
                react(Behavior::Hostile, dis * 1.001, axis),
                Reaction::BackOff
            );
            assert_eq!(
                react(Behavior::Hostile, dis * 0.999, axis),
                Reaction::Charge
            );
            assert_eq!(react(Behavior::Hostile, dis * 0.2, axis), Reaction::Charge);
        }
    }

    #[test]
    fn skittish_flees_only_at_or_just_before_roll_up() {
        for axis in [0.48, 1.2, 2.01, 4.0] {
            let up = roll_up_radius(axis);
            let dis = dislodge_radius(axis);
            assert_eq!(react(Behavior::Skittish, up, axis), Reaction::Flee);
            // A short margin early...
            assert_eq!(
                react(Behavior::Skittish, up * (1.0 - FLEE_MARGIN * 0.5), axis),
                Reaction::Flee
            );
            // ...but at Dislodge it only watches, and at Wall it carries on.
            assert_eq!(
                react(Behavior::Skittish, up * (1.0 - FLEE_MARGIN * 1.5), axis),
                Reaction::Notice
            );
            assert_eq!(
                react(Behavior::Skittish, dis * 1.001, axis),
                Reaction::Notice
            );
            assert_eq!(react(Behavior::Skittish, dis * 0.999, axis), Reaction::Idle);
        }
    }

    #[test]
    fn the_prince_is_a_little_figure_beside_the_starting_ball() {
        // The gnome stood about 1.1 yd tall at scale 0.8.
        let height = 1.1 / 0.8 * PRINCE_SCALE;
        assert!((0.3..=0.4).contains(&height), "{height}");
        // Below the starting ball's diameter (0.5 yd), and his hands are inside its height.
        assert!(height < 2.0 * super::super::level::START_RADIUS);
        assert!(HAND_HEIGHT < height && KNEE_HEIGHT < HAND_HEIGHT);
    }

    #[test]
    fn the_back_of_the_ball_is_opposite_its_forward() {
        for heading in [0.0_f32, 0.7, 2.0, -1.3] {
            let (s, c) = heading.sin_cos();
            let forward = Vec3::new(-s, 0.0, -c);
            assert!((bearing(behind(heading)) + forward).length() < 1e-5);
        }
    }

    #[test]
    fn the_prince_stands_at_the_surface_at_hand_height() {
        // A ball of radius 2 resting on the ground: at 0.5 yd up its outline is sqrt(2*2*0.5-0.25)
        // from the centre, not 2 as the sphere's radius says.
        let ball = Vec3::new(0.0, 2.0, 0.0);
        let reach = clump_reach(ball, 2.0, Vec3::Z, &[0.5], BODY_GAP, &[]);
        assert!(
            (reach - (1.75_f32.sqrt() + BODY_GAP)).abs() < 1e-5,
            "{reach}"
        );
        // A band below the ball touches nothing of it.
        assert_eq!(clump_reach(ball, 2.0, Vec3::Z, &[-0.1], BODY_GAP, &[]), 0.0);
    }

    #[test]
    fn a_trophy_at_his_back_pushes_him_out_and_one_beside_does_not() {
        let ball = Vec3::new(0.0, 1.0, 0.0);
        let bare = clump_reach(ball, 1.0, Vec3::Z, &[0.5], BODY_GAP, &[]);
        let log = Lump {
            centre: Vec3::new(0.0, 0.5, 1.6),
            radius: 0.5,
        };
        let behind_him = clump_reach(ball, 1.0, Vec3::Z, &[0.5], BODY_GAP, &[log]);
        assert!(behind_him > bare + 0.9, "{behind_him} vs {bare}");
        let aside = Lump {
            centre: Vec3::new(3.0, 0.5, 0.5),
            radius: 0.5,
        };
        assert_eq!(
            clump_reach(ball, 1.0, Vec3::Z, &[0.5], BODY_GAP, &[aside]),
            bare
        );
        // Up on the ball's crown, above his hands and knees: he walks under it.
        let crown = Lump {
            centre: Vec3::new(0.0, 2.4, 0.4),
            radius: 0.5,
        };
        assert_eq!(
            clump_reach(ball, 1.0, Vec3::Z, &[0.5], BODY_GAP, &[crown]),
            bare
        );
    }

    #[test]
    fn the_patrol_is_a_closed_loop_round_the_camp() {
        let camp = Vec3::new(10.0, 0.0, -4.0);
        for k in 0..PATROL_POINTS {
            let p = patrol_point(camp, k);
            assert!((flat(p - camp).length() - PATROL_RADIUS).abs() < 1e-4);
            assert!(flat(patrol_point(camp, k + 1) - p).length() < PATROL_RADIUS * 1.2);
        }
        assert!(flat(patrol_point(camp, PATROL_POINTS) - patrol_point(camp, 0)).length() < 1e-3);
    }

    #[test]
    fn every_entry_for_the_pass_is_in_front_and_in_view() {
        for (off, yards) in VISIT_ENTRIES {
            assert!(off.abs() < 1.2, "bearing {off} would be behind the camera");
            assert!((25.0..=60.0).contains(&yards));
        }
    }

    #[test]
    fn he_never_lumbers_home_through_the_ball() {
        // The camp lies straight beyond the ball: he gives ground and skirts it.
        let away = Vec3::new(-1.0, 0.0, 0.0);
        let home = Vec3::new(1.0, 0.0, 0.0);
        let d = lumber_dir(home, away, 3.0, 0.3);
        assert!(d.dot(away) > 0.5, "{d}");
        // Clear of it, he heads straight for the camp.
        assert_eq!(lumber_dir(home, away, 40.0, 0.3), home);
        // Camp behind him: no detour at all.
        assert_eq!(lumber_dir(away, away, 3.0, 0.3), away);
    }

    #[test]
    fn a_runner_is_always_slower_than_the_ball() {
        // A rabbit-sized and a Hogger-sized runner at the radius that first eats them.
        for axis in [0.35, 0.48, 1.2, 2.8, 4.0] {
            let r = roll_up_radius(axis) * (1.0 - FLEE_MARGIN);
            let flee = 5.8_f32.min(FLEE_SPEED_CAP * top_speed(r));
            assert!(
                flee < top_speed(r),
                "axis {axis}: {flee} vs {}",
                top_speed(r)
            );
        }
    }
}
