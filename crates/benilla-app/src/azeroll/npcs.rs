//! Authored Goldshire life, and the village that loses its mind as the ball grows. Units never
//! contribute a ball collider; what an NPC makes of the ball is decided by the one classifier the
//! ball rolls things up with, [`contact`], on the NPC's maximum axis (`Collectible::pickup`).
//!
//! # The mood ladder (village-wide, from [`TierReached`])
//!
//! | mood      | tier rows (`level::TierTable`)  | the village                                              |
//! |-----------|----------------------------------|----------------------------------------------------------|
//! | `Curious` | 0 Tiny things                    | glance, point, chat about it; kids run up and cheer      |
//! | `Nervous` | 1 Critters, 2 Creatures          | back away from a close ball; guards ready weapons       |
//! | `Panic`   | 3 NPCs and horses                | adults are edible: freeze, bolt, drop what they carry    |
//! | `Chaos`   | 4 Forest giants and up           | circles, hiding behind buildings, no one is calm         |
//!
//! The mood only climbs inside a run. [`GrowthBeat`] steps (every 25 % of diameter) are the
//! village's murmur between tiers: a mild [`Cry::Gasp`] ripple, and the `heat` the census reports.
//!
//! # Individual fear, keyed to `contact` (see [`fear`])
//!
//! | `contact(r, axis)` | villager                         | kid                       | guard                         |
//! |--------------------|----------------------------------|---------------------------|-------------------------------|
//! | `Wall` (D < 1.5 a) | calm (curious, nervous by mood)  | calm: runs up and cheers  | holds; bumps a ball in reach  |
//! | `Dislodge`         | notice, flee from D >= 1.96 a    | notice, flee from 1.72 a  | braces; one brave guard charges, bounces, flees |
//! | `RollUp` (>= 2.15) | flee                             | flee                      | flee                          |
//!
//! The comic margins are `VILLAGER_MARGIN` / `KID_MARGIN` on the ball's radius (0.10 and 0.25;
//! guards 0): an NPC bolts when `contact(r * (1 + margin + contagion), axis)` is `RollUp`, which
//! can only happen inside the `Dislodge` band: nobody runs from a ball that is a wall to them.
//!
//! # Contagion (see [`Alarm`])
//!
//! Screams, flights, dislodged NPCs, eaten creatures, growth beats and tier steps raise an
//! [`Alarm`]: a ripple that expands at [`WAVE`] yd/s. Each NPC it reaches hears it once. Hearing
//! raises the NPC's `panic` (decays), which widens its flee margin by up to [`PANIC_GAIN`] (never
//! out of the Dislodge band), makes it turn and point or shout, and, from a loud one, freeze before
//! it bolts. Fleeing raises the next alarm: panic spreads from person to person, not by radius.
//!
//! # Budgets
//!
//! Voices come from a token bucket ([`VOICE_RATE`], burst [`VOICE_CAP`]) with three priorities;
//! each NPC also keeps its own voice cooldown. Alarms have their own bucket, a cap on live ripples
//! ([`MAX_ALARMS`]) and on witnesses per ripple ([`MAX_WITNESS`]); an NPC reacts to one at most
//! every [`REACT_COOLDOWN`] seconds. Only the newest eight carried NPCs animate (`park_carried`).
use super::collect::{contact, Shed};
use super::{
    units::{self, flat, wow_to_bevy, UnitSpec},
    Attached, BallState, Collected, Collectible, Contact, GrowthBeat, Kind, Phase, Run, RunEntity,
    Source, TierReached, TouchState, Touched,
};
use crate::creature_anim::{move_flags, EmoteAnim, MovementState, PlaySeq, SpellKitSound};
use benilla_assets::{LockRecover, M2Model, WorldAssets};
use benilla_world::{collision::WorldCollision, schedule::WorldStage, world_point::WorldPoint};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;

/// An NPC pays attention to the ball within this many yards plus the ball's radius.
const AWARE: f32 = 12.0;
/// Calm NPCs glance and chat at a ball this far (plus its radius).
const CURIOUS_RANGE: f32 = 10.0;
/// In a nervous village a calm NPC watches warily, and backs off, at this range (plus radius).
const NERVOUS_RANGE: f32 = 7.0;
/// ... and starts walking backwards inside this one.
const BACKAWAY_RANGE: f32 = 5.0;
/// Kids run up to the ball and stop this far from its surface.
const KID_STOP: f32 = 2.2;
/// A sleeper wakes only when the ball is this close to its surface.
const SLEEPER_WAKE: f32 = 3.0;
/// Radius margins at which a notice becomes flight: `contact(r * (1 + margin))` is `RollUp`.
const VILLAGER_MARGIN: f32 = 0.10;
const KID_MARGIN: f32 = 0.25;
/// Extra margin at full panic. The result never leaves the Dislodge band.
const PANIC_GAIN: f32 = 0.30;
/// Panic lost per second.
const PANIC_DECAY: f32 = 0.12;
/// Fraction of the ball's undashed top speed, at the NPC's own edible size, an NPC may flee at.
const FLEE_CAP: f32 = 0.5;
/// Ripple speed, yd/s.
const WAVE: f32 = 16.0;
/// Live alarms at most, and witnesses a single ripple may recruit.
const MAX_ALARMS: usize = 10;
const MAX_WITNESS: u8 = 7;
/// An NPC reacts to a ripple at most this often, seconds.
const REACT_COOLDOWN: f32 = 2.5;
/// An NPC raises its own flight alarm at most this often, seconds.
const SHOUT_COOLDOWN: f32 = 5.0;
/// Voice bucket: refill per second and size.
const VOICE_RATE: f32 = 0.8;
const VOICE_CAP: f32 = 2.0;
/// Alarm bucket (cries that are neither screams nor mood steps).
const ALARM_RATE: f32 = 3.0;
const ALARM_CAP: f32 = 4.0;
/// Voices are only heard from NPCs this close to the ball, yards.
const HEARD_FROM: f32 = 28.0;
/// A brave guard charges a ball this close, and no more often than this, seconds.
const CHARGE_FROM: f32 = 14.0;
const CHARGE_COOLDOWN: f32 = 30.0;
/// The most drops a run may litter.
const MAX_DROPS: u32 = 24;
/// In a panicking village a fleeing NPC calms down only once the ball is this far (plus radius).
const CALM_DOWN_RANGE: f32 = 30.0;
/// Flee leash from the NPC's post, yards.
const LEASH: f32 = 35.0;
/// Fraction of fleeing NPCs that freeze before they bolt (all of them after a scream).
const FREEZE_ODDS: f32 = 0.45;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Role {
    Guard,
    Villager,
    Kid,
}
#[derive(Clone, Deserialize)]
struct Spec {
    name: String,
    scene: String,
    role: Role,
    activity: String,
    display: u32,
    at: [f32; 3],
    yaw: f32,
    #[serde(default)]
    indoors: bool,
    #[serde(default)]
    path: Vec<[f32; 3]>,
    /// What this NPC drops when it first bolts ([`carry_model`]).
    #[serde(default)]
    carry: String,
    /// Another NPC's name: they flee holding hands.
    #[serde(default)]
    partner: String,
    /// `polish`, `sleeper` or `brave` ([`Quirk`]).
    #[serde(default)]
    quirk: String,
}
#[derive(Resource, Default, Deserialize)]
struct Cast {
    npc: Vec<Spec>,
}
#[derive(Resource, Default)]
pub(super) struct Pending {
    rows: Vec<Spec>,
    catalog: Option<benilla_formats::CreatureCatalog>,
    models: Vec<Handle<M2Model>>,
}

/// What an NPC is doing about the ball.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Authored life: activity, patrol, back to its post.
    Ambient,
    /// A small ball: glance, point, chat, and for kids run up and cheer.
    Curious,
    /// Wary: face it, back off, brace.
    Notice,
    /// Gaping for a moment before running.
    Freeze,
    Flee,
    /// Hiding or cornered: begs, waits for the ball to leave.
    Cower,
    /// A brave guard running at a ball that can only dislodge him.
    Charge,
    /// Knocked back by the ball; flees when he lands.
    Bounced,
}
const STATES: usize = 8;

/// How a fleeing NPC runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    /// Away from the threat, fanned out by a per-NPC angle, zigzagging.
    Scatter,
    /// Round and round its post.
    Circle,
    /// To a spot the ball cannot see.
    Hide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quirk {
    None,
    Polish,
    Sleeper,
    Brave,
}
impl Quirk {
    fn parse(text: &str) -> Self {
        match text {
            "polish" => Self::Polish,
            "sleeper" => Self::Sleeper,
            "brave" => Self::Brave,
            _ => Self::None,
        }
    }
}

/// The village's collective mood, from the tier the ball has reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Mood {
    #[default]
    Curious,
    Nervous,
    Panic,
    Chaos,
}
impl Mood {
    /// `tier` is a zero-based `level::TierTable` row: tiny things, critters, creatures, NPCs and
    /// horses, forest giants, Hogger.
    fn of_tier(tier: usize) -> Self {
        match tier {
            0 => Self::Curious,
            1 | 2 => Self::Nervous,
            3 => Self::Panic,
            _ => Self::Chaos,
        }
    }
}

/// What the ball is to one NPC on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fear {
    Calm,
    Wary,
    Flee,
}

/// The one rule: notice at `Dislodge`, flee a little before `RollUp` (guards exactly at it),
/// `panic` (0..1, heard alarms) pulling the flight earlier but never out of the Dislodge band.
fn fear(role: Role, radius: f32, axis: f32, panic: f32) -> Fear {
    match contact(radius, axis) {
        Contact::Wall => Fear::Calm,
        Contact::RollUp => Fear::Flee,
        Contact::Dislodge => {
            let (margin, infected) = match role {
                Role::Guard => (0.0, 0.0),
                Role::Villager => (VILLAGER_MARGIN, PANIC_GAIN * panic),
                Role::Kid => (KID_MARGIN, PANIC_GAIN * panic),
            };
            if contact(radius * (1.0 + margin + infected), axis) == Contact::RollUp {
                Fear::Flee
            } else {
                Fear::Wary
            }
        }
    }
}

/// The state an NPC wants to be in for this ball. `near` is the awareness gate (distance and
/// height); the mood only colours what a calm NPC does.
fn reaction(
    role: Role,
    mood: Mood,
    radius: f32,
    axis: f32,
    panic: f32,
    distance: f32,
    near: bool,
) -> State {
    if !near {
        return State::Ambient;
    }
    match fear(role, radius, axis, panic) {
        Fear::Flee => State::Flee,
        Fear::Wary => State::Notice,
        Fear::Calm => {
            if role != Role::Kid && mood >= Mood::Nervous && distance < NERVOUS_RANGE + radius {
                State::Notice
            } else if distance < CURIOUS_RANGE + radius {
                State::Curious
            } else {
                State::Ambient
            }
        }
    }
}

/// Fastest flee burst for an NPC of this maximum axis: always below the ball's undashed speed at
/// the size where it can roll the NPC up, so every runner is catchable. `urgency` (0..1) is panic.
fn flee_speed(axis: f32, phase: f32, urgency: f32) -> f32 {
    let edible = axis / super::level::COLLECT_RATIO;
    ((2.5 + 1.6 * phase.sin().max(0.0)) * (1.0 + 0.2 * urgency))
        .min(super::ball::top_speed(edible) * FLEE_CAP)
}

/// Stable per-NPC dice in `0..1`; `salt` picks the die.
fn unit(seed: u32, salt: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// The things NPCs drop (`carry` in the cast file), all from the level's own prop palette.
fn carry_model(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "mug" => r"World\Generic\Human\Passive Doodads\Mugs\Mug01.m2",
        "stein" => r"World\Generic\Human\Passive Doodads\Mugs\Stein01.m2",
        "bread" => r"World\Generic\Human\Passive Doodads\Food\bread01.m2",
        "apple" => r"World\Generic\PassiveDoodads\Fruits\Fruit_Apple.m2",
        "hammer" => r"World\Generic\Human\Passive Doodads\Weapons&Armor\HumanHammer02.m2",
        "cleaver" => r"World\Generic\Human\Passive Doodads\Food&Utensils\Cleaver01.m2",
        "bottle" => r"World\Generic\Human\Passive Doodads\Bottles\Bottle01.m2",
        "jug" => r"World\Azeroth\Elwynn\PassiveDoodads\jugs\jug01.m2",
        _ => return None,
    })
}

#[derive(Component)]
pub(super) struct Npc {
    spec: Spec,
    quirk: Quirk,
    home: Vec3,
    state: State,
    waypoint: usize,
    timer: f32,
    wait: f32,
    phase: f32,
    voice_at: f32,
    sex: u32,
    carried_pose: Option<Quat>,
    /// Stable identity for the dice and for pairing.
    seed: u32,
    /// Contagion, 0..1: heard alarms, decaying.
    panic: f32,
    /// Minimum seconds before a calm-state change (stops glance/notice flicker).
    dwell: f32,
    react_cd: f32,
    shout_cd: f32,
    /// Keeps fleeing until this run time, whatever the ball does.
    flee_until: f32,
    style: Style,
    scatter: f32,
    hide: Option<Vec3>,
    probe: f32,
    cower_until: f32,
    flee_count: u32,
    /// Side-step after being blocked: radians off the flee heading, until `turn_until`.
    turn: f32,
    turn_until: f32,
    /// Last whole 4-second slice a stumble happened in.
    stumbled: i64,
    partner: Option<Entity>,
    grieved: bool,
    dropped: bool,
    polish_logged: bool,
    charge_until: f32,
    bounce: Vec3,
    /// Where it was last told to look, until `look_until`.
    look_at: Vec3,
    look_until: f32,
    /// The last place an alarm came from, and when.
    alarm_at: Vec3,
    alarm_t: f32,
    /// Carried-NPC reactions (#22).
    was_carried: bool,
    pulse: u32,
    cry_at: f32,
    react_at: f32,
}

/// An expanding cry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cry {
    /// A growth step, or something small eaten: heads turn.
    Gasp,
    /// Someone running or knocked loose: point and shout.
    Shout,
    /// A person rolled up: everyone nearby hears it.
    Scream,
    /// The village crossed a tier.
    Mood,
}
impl Cry {
    /// `(range yd, power 0..1)`
    fn shape(self) -> (f32, f32) {
        match self {
            Self::Gasp => (26.0, 0.35),
            Self::Shout => (14.0, 0.6),
            Self::Scream => (38.0, 1.0),
            Self::Mood => (55.0, 0.5),
        }
    }
}
struct Alarm {
    at: Vec3,
    born: f32,
    cry: Cry,
    src: Option<Entity>,
    hits: u8,
    heard: Vec<Entity>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Prio {
    Low,
    Normal,
    High,
}

/// A read-only copy of one NPC, taken at the top of the frame so partners can see each other.
#[derive(Clone, Copy)]
struct Snap {
    e: Entity,
    pos: Vec3,
    state: State,
    seed: u32,
    attached: bool,
}

#[derive(Default)]
struct Stats {
    reactions: u32,
    voices: u32,
    alarms: u32,
    drops: u32,
    charges: u32,
    bounces: u32,
    cries: u32,
}

/// The village: its mood, its live alarms and its budgets.
#[derive(Resource)]
struct Village {
    mood: Mood,
    tier: usize,
    /// Highest `GrowthBeat` step this run.
    heat: u32,
    /// Counts tier steps; carried NPCs react once to each.
    pulse: u32,
    alarms: Vec<Alarm>,
    alarm_tokens: f32,
    voices: f32,
    names: HashMap<String, Entity>,
    snap: Vec<Snap>,
    /// NPCs the ball touched as a Dislodge this frame.
    touched: Vec<Entity>,
    charge_cd: f32,
    touch_cd: f32,
    census_at: f32,
    stats: Stats,
}
impl Default for Village {
    fn default() -> Self {
        Self {
            mood: Mood::Curious,
            tier: 0,
            heat: 0,
            pulse: 0,
            alarms: Vec::new(),
            alarm_tokens: ALARM_CAP,
            voices: VOICE_CAP,
            names: HashMap::new(),
            snap: Vec::new(),
            touched: Vec::new(),
            charge_cd: 0.0,
            touch_cd: 0.0,
            census_at: 5.0,
            stats: Stats::default(),
        }
    }
}
impl Village {
    fn refill(&mut self, dt: f32) {
        self.voices = (self.voices + VOICE_RATE * dt).min(VOICE_CAP);
        self.alarm_tokens = (self.alarm_tokens + ALARM_RATE * dt).min(ALARM_CAP);
    }
    /// Start a ripple. Screams and mood steps are free; the rest spend an alarm token.
    fn raise(&mut self, now: f32, at: Vec3, cry: Cry, src: Option<Entity>) -> bool {
        if self.alarms.len() >= MAX_ALARMS {
            return false;
        }
        if !matches!(cry, Cry::Scream | Cry::Mood) {
            if self.alarm_tokens < 1.0 {
                return false;
            }
            self.alarm_tokens -= 1.0;
        }
        self.stats.alarms += 1;
        self.alarms.push(Alarm {
            at,
            born: now,
            cry,
            src,
            hits: 0,
            heard: Vec::new(),
        });
        true
    }
    /// A voice costs a token; low priority needs spare tokens, high priority may dip to empty.
    fn take_voice(&mut self, prio: Prio) -> bool {
        let need = match prio {
            Prio::Low => 1.6,
            Prio::Normal => 1.0,
            Prio::High => 0.3,
        };
        if self.voices < need {
            return false;
        }
        self.voices = (self.voices - 1.0).max(0.0);
        self.stats.voices += 1;
        true
    }
}

/// Sound and animation requests.
#[derive(SystemParam)]
struct Fx<'w> {
    emotes: Option<Res<'w, crate::sound::EmoteSounds>>,
    rams: MessageWriter<'w, super::collect::Ram>,
    seq: ResMut<'w, PlaySeq>,
    anim: MessageWriter<'w, EmoteAnim>,
    sound: MessageWriter<'w, SpellKitSound>,
}
impl Fx<'_> {
    fn emote(&mut self, e: Entity, id: u16) {
        self.anim.write(EmoteAnim {
            entity: e,
            anim_id: id,
            seq: self.seq.next(),
            via_player: false,
        });
    }
    /// One voice line (`FLEE`, `HELPME`, ...) at `at`, if the budget allows and the kit exists.
    fn voice(
        &mut self,
        village: &mut Village,
        text: &str,
        kid: bool,
        sex: u32,
        at: Vec3,
        prio: Prio,
    ) -> bool {
        let (race, sex) = if kid { (7, 1) } else { (1, sex) };
        let Some(kit) = self
            .emotes
            .as_ref()
            .and_then(|c| c.0.text_id(text).and_then(|id| c.0.voice(id, race, sex)))
        else {
            return false;
        };
        if !village.take_voice(prio) {
            return false;
        }
        self.sound.write(SpellKitSound::PlayAt {
            pos: at,
            kit_sound: kit,
        });
        true
    }
}

#[derive(SystemParam)]
struct Senses<'w> {
    time: Res<'w, Time>,
    run: Res<'w, Run>,
    ball: Res<'w, BallState>,
    design: Res<'w, super::design::Design>,
    cast: Res<'w, Cast>,
    assets: Res<'w, AssetServer>,
    village: ResMut<'w, Village>,
}

/// A prop an NPC let go of, still in flight.
#[derive(Component)]
struct Dropped {
    vel: Vec3,
    ttl: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        park_carried
            .in_set(bevy::app::AnimationSystems)
            .before(bevy::animation::advance_animations),
    );
    app.add_systems(
        Update,
        capture_tour
            .in_set(WorldStage::Input)
            .before(super::ball::drive_ball)
            .run_if(super::playing),
    );
    app.add_systems(
        Update,
        capture_camera
            .in_set(WorldStage::Input)
            .after(super::camera::place_camera)
            .run_if(super::playing),
    );
    app.init_resource::<Cast>()
        .init_resource::<Pending>()
        .init_resource::<Village>()
        .add_systems(OnEnter(Phase::Ready), reload.after(super::reset_run))
        .add_systems(Update, spawn)
        .add_systems(
            Update,
            (witness, behave, toss_drops)
                .chain()
                .after(WorldStage::Input)
                .run_if(super::playing),
        );
}
fn reload(mut cast: ResMut<Cast>, mut pending: ResMut<Pending>, mut village: ResMut<Village>) {
    let embedded = include_str!("goldshire-npcs.toml");
    #[cfg(not(target_arch = "wasm32"))]
    let override_text =
        crate::webenv::var("WOW_AZEROLL_NPCS").and_then(|p| match std::fs::read_to_string(p) {
            Ok(s) => Some(s),
            Err(e) => {
                warn!("azeroll NPC override: {e}");
                None
            }
        });
    #[cfg(target_arch = "wasm32")]
    let override_text: Option<String> = None;
    *cast = override_text
        .as_deref()
        .and_then(|s| {
            toml::from_str(s)
                .map_err(|e| warn!("azeroll NPC parse: {e}"))
                .ok()
        })
        .unwrap_or_else(|| toml::from_str(embedded).expect("embedded NPC recipe"));
    pending.rows = cast.npc.clone();
    *village = Village::default();
    info!("azeroll NPC cast: {}", cast.npc.len());
}
#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    assets: Option<Res<WorldAssets>>,
    server: Res<AssetServer>,
    models: Res<Assets<M2Model>>,
    world: WorldPoint,
    collision: WorldCollision,
    mut run: ResMut<Run>,
    mut village: ResMut<Village>,
) {
    if pending.rows.is_empty() {
        return;
    }
    if pending.catalog.is_none() {
        let Some(assets) = assets else { return };
        pending.catalog =
            benilla_formats::load_creature_catalog(&mut assets.chain.lock_recover()).ok();
    }
    let mut spawned = 0;
    for i in (0..pending.rows.len()).rev() {
        if spawned == 3 {
            break;
        }
        let spec = pending.rows[i].clone();
        let Some(m) = pending.catalog.as_ref().and_then(|c| c.model(spec.display)) else {
            continue;
        };
        let handle = server.load(benilla_assets::m2_url(&m.model_path));
        if !pending.models.contains(&handle) {
            pending.models.push(handle.clone());
        }
        let Some(bounds) = models.get(&handle).and_then(|m| m.bounds) else {
            continue;
        };
        let m = pending
            .catalog
            .as_ref()
            .unwrap()
            .model(spec.display)
            .unwrap();
        let scale = m.scale;
        let sex = m.npc_appearance.as_ref().map_or(0, |a| a.sex as u32);
        let half = wow_to_bevy(bounds.stand_extent).abs() * 0.5;
        if half.min_element() <= 0.0 {
            continue;
        }
        let mut pos = wow_to_bevy(spec.at);
        let ground = if spec.indoors {
            collision
                .ray_body(pos + Vec3::Y * 0.8, Dir3::NEG_Y, 2.0)
                .map(|h| pos.y + 0.8 - h.distance)
        } else {
            world.terrain_height_under(pos)
        };
        let Some(y) = ground else { continue };
        pos.y = y;
        let entity = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: run.guid(),
                display: spec.display,
                scale,
                level: 1,
                faction: units::FACTION_FRIENDLY,
                bytes0: 1 | (1 << 8) | (sex << 16),
            },
            pos,
            spec.yaw,
        );
        info!(
            "azeroll NPC: {} scene={} axis={:.2}",
            spec.name,
            spec.scene,
            2.0 * half.max_element() * scale
        );
        village.names.insert(spec.name.clone(), entity);
        let quirk = Quirk::parse(&spec.quirk);
        commands.entity(entity).insert((
            Collectible {
                name: Cow::Owned(spec.name.clone()),
                kind: Kind::Creature,
                source: Source::Spawned,
                size: half.length() * scale,
                pickup: 2.0 * half.max_element() * scale,
                half_extents: half,
                center: Vec3::Y * half.y,
                solid: 0.0,
            },
            Npc {
                spec,
                quirk,
                home: pos,
                state: State::Ambient,
                waypoint: 0,
                timer: 1.0 + i as f32 * 0.23,
                wait: 0.0,
                phase: i as f32 * 1.71,
                voice_at: 0.0,
                sex,
                carried_pose: None,
                seed: i as u32 + 1,
                panic: 0.0,
                dwell: 0.0,
                react_cd: 0.0,
                shout_cd: 0.0,
                flee_until: 0.0,
                style: Style::Scatter,
                scatter: 0.0,
                hide: None,
                probe: 0.0,
                cower_until: 0.0,
                flee_count: 0,
                turn: 0.0,
                turn_until: 0.0,
                stumbled: -1,
                partner: None,
                grieved: false,
                dropped: false,
                polish_logged: false,
                charge_until: 0.0,
                bounce: Vec3::ZERO,
                look_at: Vec3::ZERO,
                look_until: 0.0,
                alarm_at: pos,
                alarm_t: -100.0,
                was_carried: false,
                pulse: 0,
                cry_at: 0.0,
                react_at: 0.0,
            },
        ));
        pending.rows.remove(i);
        spawned += 1;
    }
}

/// Mood steps, growth murmurs, eaten NPCs and dislodged ones become ripples.
#[allow(clippy::too_many_arguments)]
fn witness(
    mut village: ResMut<Village>,
    ball: Res<BallState>,
    run: Res<Run>,
    time: Res<Time>,
    mut collected: MessageReader<Collected>,
    mut touched: MessageReader<Touched>,
    mut beats: MessageReader<GrowthBeat>,
    mut tiers: MessageReader<TierReached>,
    npcs: Query<(&Npc, &Transform)>,
) {
    let now = run.elapsed;
    let v = &mut *village;
    v.touched.clear();
    v.touch_cd = (v.touch_cd - time.delta_secs()).max(0.0);
    for tier in tiers.read() {
        let mood = Mood::of_tier(tier.tier);
        v.tier = v.tier.max(tier.tier);
        v.pulse += 1;
        let climbed = mood > v.mood;
        v.mood = v.mood.max(mood);
        v.raise(now, ball.pos, Cry::Mood, None);
        info!(
            "azeroll village mood: {:?}{} at tier {} {:?} (D={:.2}, row {:.2}), t={now:.1}s",
            v.mood,
            if climbed { " (climbed)" } else { "" },
            tier.tier,
            tier.name,
            tier.diameter,
            tier.threshold,
        );
    }
    for beat in beats.read() {
        v.heat = v.heat.max(beat.step);
        v.raise(now, ball.pos, Cry::Gasp, None);
        info!(
            "azeroll village growth beat: step {} (D={:.2}, threshold {:.2}), mood {:?}",
            beat.step, beat.diameter, beat.threshold, v.mood
        );
    }
    for got in collected.read() {
        if let Ok((npc, _)) = npcs.get(got.entity) {
            v.raise(now, ball.pos, Cry::Scream, Some(got.entity));
            info!(
                "azeroll village scream: {} rolled up, t={now:.1}s",
                npc.spec.name
            );
        } else if got.kind != Kind::Prop {
            v.raise(now, ball.pos, Cry::Gasp, None);
        }
    }
    for touch in touched.read() {
        if touch.state != TouchState::Dislodge {
            continue;
        }
        let Ok((npc, tf)) = npcs.get(touch.entity) else {
            continue;
        };
        v.touched.push(touch.entity);
        if touch.strength > 1.5 && v.touch_cd == 0.0 {
            v.touch_cd = 1.5;
            v.raise(now, tf.translation, Cry::Shout, Some(touch.entity));
            info!(
                "azeroll village shout: {} knocked loose ({:.1} yd/s)",
                npc.spec.name, touch.strength
            );
        }
    }
}

/// The animation a witness plays, if any (kids have no clips).
fn witness_emote(role: Role, cry: Cry, mood: Mood, seed: u32) -> Option<u16> {
    if role == Role::Kid {
        return None;
    }
    let pick = |ids: &[u16]| ids[seed as usize % ids.len()];
    Some(match (role, cry) {
        (Role::Guard, Cry::Mood) => 55,
        (Role::Guard, Cry::Gasp) => 26,
        (Role::Guard, _) => 24,
        (_, Cry::Gasp) if mood == Mood::Curious => pick(&[84, 60, 70]),
        (_, Cry::Gasp) => pick(&[65, 84]),
        (_, Cry::Shout) => pick(&[64, 84]),
        (_, Cry::Scream) => pick(&[84, 81, 64]),
        (_, Cry::Mood) => {
            if mood >= Mood::Panic {
                81
            } else {
                65
            }
        }
    })
}

/// A clear step, or `false`: a forward body ray, no pinch to either side, no entering a doorway's
/// exclusion circle, and a floor within 0.6 yd. Doors and narrow passages are scenery for NPCs.
#[allow(clippy::too_many_arguments)]
fn try_step(
    tf: &mut Transform,
    dir: Vec3,
    speed: f32,
    dt: f32,
    indoors: bool,
    world: &WorldPoint,
    collision: &WorldCollision,
    doors: &[Vec3],
) -> bool {
    let Ok(forward) = Dir3::new(dir) else {
        return false;
    };
    let pos = tf.translation;
    let next = pos + dir * speed * dt;
    let clear = collision
        .ray_body(pos + Vec3::Y * 0.8, forward, speed * dt + 0.4)
        .is_none();
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let pinched = [side, -side].into_iter().all(|side| {
        Dir3::new(side).is_ok_and(|side| {
            collision
                .ray_body(next + Vec3::Y * 0.8, side, 1.8)
                .is_some_and(|hit| hit.normal.y.abs() < 0.6)
        })
    });
    // Leaving a doorway's circle is fine; entering one is not.
    let doorway = doors.iter().any(|p| {
        let (before, after) = (flat(pos - *p).length(), flat(next - *p).length());
        after < 1.8 && after < before
    });
    let floor = ground_at(next, indoors, world, collision);
    match floor.filter(|y| clear && !doorway && !pinched && (*y - pos.y).abs() < 0.6) {
        Some(y) => {
            tf.translation = Vec3::new(next.x, y, next.z);
            true
        }
        None => false,
    }
}

fn ground_at(
    p: Vec3,
    indoors: bool,
    world: &WorldPoint,
    collision: &WorldCollision,
) -> Option<f32> {
    if indoors {
        collision
            .ray_body(p + Vec3::Y * 0.7, Dir3::NEG_Y, 1.5)
            .map(|h| p.y + 0.7 - h.distance)
    } else {
        world.terrain_height_under(p)
    }
}

/// A spot within a few yards that the ball cannot see: a building or a prop is between.
fn find_hide(
    pos: Vec3,
    ball: Vec3,
    seed: u32,
    indoors: bool,
    world: &WorldPoint,
    collision: &WorldCollision,
    doors: &[Vec3],
) -> Option<Vec3> {
    for reach in [4.5, 7.5] {
        for k in 0..8 {
            let angle = (k as f32 + unit(seed, 3)) * std::f32::consts::TAU / 8.0;
            let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
            let probe = pos + dir * reach;
            let Some(y) = ground_at(probe, indoors, world, collision) else {
                continue;
            };
            if (y - pos.y).abs() > 0.6 {
                continue;
            }
            let spot = Vec3::new(probe.x, y, probe.z);
            if doors.iter().any(|p| flat(spot - *p).length() < 2.6) {
                continue;
            }
            let Ok(way) = Dir3::new(dir) else { continue };
            if collision
                .ray_body(pos + Vec3::Y * 0.8, way, reach + 0.4)
                .is_some()
            {
                continue;
            }
            let line = spot + Vec3::Y * 0.9 - ball;
            let len = line.length();
            let Ok(sight) = Dir3::new(line / len.max(1e-4)) else {
                continue;
            };
            if len > 0.5 && collision.ray_body(ball, sight, len - 0.3).is_some() {
                return Some(spot);
            }
        }
    }
    None
}

/// Where a carried NPC's cry is heard from: the far side of the ball from the camera and a little
/// way off, so it sounds muffled by the clump (the sound path has no per-play filter).
fn muffled_at(ball: &BallState, camera: Option<Vec3>) -> Vec3 {
    let away = camera
        .map(|c| (ball.pos - c).normalize_or_zero())
        .unwrap_or(Vec3::ZERO);
    ball.pos + away * (ball.radius * 0.5 + 3.0)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn behave(
    mut commands: Commands,
    senses: Senses,
    world: WorldPoint,
    collision: WorldCollision,
    mut fx: Fx,
    cameras: Query<&GlobalTransform, With<benilla_world::view::WorldCamera>>,
    mut q: Query<
        (
            Entity,
            &mut Npc,
            &Collectible,
            &mut Transform,
            &mut MovementState,
            Option<&Attached>,
        ),
        Without<Shed>,
    >,
) {
    let Senses {
        time,
        run,
        ball,
        design,
        cast,
        assets,
        mut village,
    } = senses;
    let v = &mut *village;
    let dt = time.delta_secs().min(0.1);
    let now = run.elapsed;
    v.refill(dt);
    v.alarms
        .retain(|a| now - a.born < a.cry.shape().0 / WAVE + 0.5);
    v.snap.clear();
    v.snap.extend(q.iter().map(|(e, n, _, tf, _, a)| Snap {
        e,
        pos: tf.translation,
        state: n.state,
        seed: n.seed,
        attached: a.is_some(),
    }));
    let mut carried: Vec<_> = q
        .iter()
        .filter_map(|(e, _, _, _, _, a)| a.map(|a| (e, a.at)))
        .collect();
    carried.sort_by(|a, b| b.1.total_cmp(&a.1));
    let doors: Vec<Vec3> = design
        .areas
        .iter()
        .flat_map(|a| a.doors.iter())
        .map(|p| wow_to_bevy(*p))
        .collect();
    let camera = cameras.iter().next().map(|t| t.translation());
    let mood = v.mood;
    let mut counts = [0u32; STATES];
    let mut probes = 2;

    for (e, mut n, c, mut tf, mut mv, attached) in &mut q {
        n.timer -= dt;
        n.wait = (n.wait - dt).max(0.0);
        let phase = now * 6.0 + n.phase;
        let kid = n.spec.role == Role::Kid;
        if let Some(a) = attached {
            // Carried (#22): newest eight animate (park_carried); the rest ride quietly.
            commands.entity(e).insert(crate::entities::Carried);
            let rank = carried.iter().take(8).position(|(id, _)| *id == e);
            let active = rank.is_some() && a.radial + c.size > ball.radius * 0.65;
            mv.flags = if kid {
                move_flags::FORWARD
            } else {
                move_flags::FALLING | move_flags::FALLING_FAR
            };
            mv.speed = if active { 3.0 } else { 0.0 };
            mv.vertical_speed = -1.0;
            if kid {
                let rest = *n.carried_pose.get_or_insert(tf.rotation);
                tf.rotation =
                    rest * Quat::from_rotation_z(if active { 0.4 * phase.sin() } else { 0.0 });
            }
            if !n.was_carried {
                // The moment of capture: one real scream, then muffled cries now and then.
                n.was_carried = true;
                n.state = State::Ambient;
                n.pulse = v.pulse;
                n.cry_at = now + 3.0 + 4.0 * unit(n.seed, 21);
                n.react_at = 0.0;
                if fx.voice(v, "HELPME", kid, n.sex, ball.pos, Prio::High) {
                    n.voice_at = now + 6.0;
                }
            }
            if active {
                if n.pulse != v.pulse {
                    // The ball grew past a tier: some carried people cheer, the rest wail.
                    n.pulse = v.pulse;
                    n.react_at = now + 0.25 * rank.unwrap_or(0) as f32;
                }
                if n.react_at > 0.0 && now >= n.react_at {
                    n.react_at = 0.0;
                    let cheer = unit(n.seed, 7 + v.pulse) < 0.45;
                    let id = if cheer {
                        68
                    } else if n.sex == 0 {
                        77
                    } else {
                        79
                    };
                    if !kid {
                        fx.emote(e, id);
                    }
                    let said = fx.voice(
                        v,
                        if cheer { "CHEER" } else { "HELPME" },
                        kid,
                        n.sex,
                        muffled_at(&ball, camera),
                        Prio::Low,
                    );
                    if said {
                        v.stats.cries += 1;
                    }
                    info!(
                        "azeroll NPC carried {}: {} (pulse {}, voiced {said})",
                        if cheer { "cheers" } else { "wails" },
                        n.spec.name,
                        v.pulse
                    );
                    n.cry_at = now + 8.0 + 6.0 * unit(n.seed, 23);
                } else if now >= n.cry_at {
                    n.cry_at = now + 8.0 + 8.0 * unit(n.seed, 25 + (now as u32 % 7));
                    if fx.voice(
                        v,
                        "HELPME",
                        kid,
                        n.sex,
                        muffled_at(&ball, camera),
                        Prio::Low,
                    ) {
                        v.stats.cries += 1;
                        if !kid {
                            fx.emote(e, if n.sex == 0 { 78 } else { 95 });
                        }
                    }
                }
            }
            continue;
        }

        // ---- free NPC --------------------------------------------------------------------
        n.was_carried = false;
        n.carried_pose = None;
        n.panic = (n.panic - PANIC_DECAY * dt).max(0.0);
        n.dwell = (n.dwell - dt).max(0.0);
        let role = n.spec.role;
        let axis = c.pickup;
        let pos = tf.translation;
        let delta = flat(ball.pos - pos);
        let distance = delta.length();
        let toward = delta.normalize_or_zero();
        let near = distance < AWARE + ball.radius && (ball.pos.y - pos.y).abs() < ball.radius + 3.0;
        let asleep = n.quirk == Quirk::Sleeper && distance > SLEEPER_WAKE + ball.radius;

        if n.partner.is_none() && !n.spec.partner.is_empty() {
            n.partner = v.names.get(&n.spec.partner).copied();
        }
        let mate = n
            .partner
            .and_then(|p| v.snap.iter().find(|s| s.e == p))
            .copied();
        // Grief: the other half of a pair was rolled up.
        if mate.is_some_and(|m| m.attached) && !n.grieved {
            n.grieved = true;
            n.panic = 1.0;
            n.state = State::Cower;
            n.cower_until = now + 2.5;
            n.timer = 0.0;
            if !kid {
                fx.emote(e, if n.sex == 0 { 77 } else { 79 });
            }
            fx.voice(v, "HELPME", kid, n.sex, pos, Prio::Normal);
            info!(
                "azeroll NPC grief: {} saw {} rolled up",
                n.spec.name, n.spec.partner
            );
        }

        // Hear the ripples that have reached us.
        let mut heard: Option<(Cry, Vec3, f32)> = None;
        if n.quirk != Quirk::Polish && !asleep {
            for a in v.alarms.iter_mut() {
                let d = flat(a.at - pos).length();
                let (range, power) = a.cry.shape();
                if a.heard.contains(&e)
                    || a.src == Some(e)
                    || d > range
                    || (now - a.born) * WAVE < d
                {
                    continue;
                }
                a.heard.push(e);
                if a.hits >= MAX_WITNESS && a.cry != Cry::Scream {
                    continue;
                }
                a.hits += 1;
                let s = power * (1.0 - d / range).max(0.2);
                n.panic = (n.panic + s).min(1.0);
                n.alarm_at = a.at;
                n.alarm_t = now;
                if heard.is_none_or(|h| s > h.2) {
                    heard = Some((a.cry, a.at, s));
                }
            }
        }
        let busy = !matches!(n.state, State::Ambient | State::Curious | State::Notice);
        if let Some((cry, at, s)) = heard.filter(|_| !busy && now >= n.react_cd) {
            n.react_cd = now + REACT_COOLDOWN;
            n.look_at = at;
            n.look_until = now + 1.6;
            if let Some(id) = witness_emote(role, cry, mood, n.seed) {
                fx.emote(e, id);
            }
            if cry == Cry::Scream && s > 0.3 && distance < HEARD_FROM {
                fx.voice(v, "INCOMING", kid, n.sex, pos, Prio::Normal);
            }
            debug!(
                "azeroll NPC witness: {} hears {:?} (s={s:.2}, panic {:.2})",
                n.spec.name, cry, n.panic
            );
        }

        // What it wants to do about the ball.
        let mut wanted = if asleep {
            State::Ambient
        } else {
            reaction(role, mood, ball.radius, axis, n.panic, distance, near)
        };
        if n.quirk == Quirk::Polish {
            if wanted != State::Ambient && !n.polish_logged {
                n.polish_logged = true;
                info!(
                    "azeroll NPC quirk: {} keeps polishing his mug ({:?} wanted, mood {:?})",
                    n.spec.name, wanted, mood
                );
            }
            wanted = State::Ambient;
        }
        // Hand in hand: if the other runs and this ball is no wall to us, so do we.
        if wanted != State::Flee
            && !asleep
            && mate.is_some_and(|m| {
                matches!(m.state, State::Flee | State::Freeze) && flat(m.pos - pos).length() < 10.0
            })
            && fear(role, ball.radius, axis, 1.0) != Fear::Calm
        {
            wanted = State::Flee;
        }

        let prev = n.state;
        let mut next = prev;
        match prev {
            State::Freeze => {
                if n.timer <= 0.0 {
                    next = State::Flee;
                }
            }
            State::Bounced => {}
            State::Charge => {
                if wanted == State::Flee || now > n.charge_until {
                    next = State::Flee;
                    n.flee_until = n.flee_until.max(now + 4.0);
                }
            }
            State::Cower => {
                if now >= n.cower_until || (distance < 4.0 + ball.radius && !asleep) {
                    next = if wanted == State::Flee || n.panic > 0.5 {
                        State::Flee
                    } else {
                        wanted
                    };
                }
            }
            State::Flee => {
                if wanted == State::Flee {
                    n.flee_until = n
                        .flee_until
                        .max(now + 3.0 + 3.0 * unit(n.seed, 13 + n.flee_count));
                }
                // A calm village trots back to its post once the ball is out of sight; a panicking
                // one keeps its distance while the ball is anywhere near.
                let keep_away = mood >= Mood::Panic && distance < CALM_DOWN_RANGE + ball.radius;
                if now >= n.flee_until && wanted != State::Flee && !keep_away {
                    next = wanted;
                }
            }
            _ => {
                if wanted == State::Flee {
                    let freeze = unit(n.seed, 17 + n.flee_count) < FREEZE_ODDS
                        || (now - n.alarm_t < 1.0 && n.panic > 0.6);
                    next = if freeze && role != Role::Guard {
                        State::Freeze
                    } else {
                        State::Flee
                    };
                } else if wanted == State::Notice
                    && n.quirk == Quirk::Brave
                    && contact(ball.radius, axis) == Contact::Dislodge
                    && now >= v.charge_cd
                    && distance < CHARGE_FROM + ball.radius
                {
                    next = State::Charge;
                } else if wanted != prev && n.dwell == 0.0 {
                    next = wanted;
                }
            }
        }
        if next != prev {
            n.timer = 0.0;
            n.dwell = 1.2;
            n.state = next;
            v.stats.reactions += 1;
            let line = format!(
                "azeroll NPC reaction: {} {:?} -> {:?} (contact {:?}, D/axis {:.2}, panic {:.2}, mood {:?})",
                n.spec.name,
                prev,
                next,
                contact(ball.radius, axis),
                ball.diameter() / axis,
                n.panic,
                mood
            );
            // Calm, mood-driven stances come and go with the ball's distance: not worth a line.
            if matches!(next, State::Ambient | State::Curious)
                || (next == State::Notice && contact(ball.radius, axis) == Contact::Wall)
            {
                debug!("{line}");
            } else {
                info!("{line}");
            }
            match next {
                State::Freeze => {
                    n.timer = 0.35 + 0.8 * unit(n.seed, 19 + n.flee_count);
                    if !kid {
                        fx.emote(e, 64);
                    }
                    fx.voice(v, "INCOMING", kid, n.sex, pos, Prio::Normal);
                }
                State::Charge => {
                    n.charge_until = now + 5.0;
                    v.charge_cd = now + CHARGE_COOLDOWN;
                    v.stats.charges += 1;
                    fx.emote(e, 55);
                    fx.voice(v, "CHARGE", kid, n.sex, pos, Prio::High);
                }
                _ => {}
            }
        }
        if next == State::Flee && prev != State::Flee {
            n.flee_count += 1;
            let shared = mate.map_or(n.seed, |m| m.seed.min(n.seed));
            n.scatter = (unit(shared, 11 + n.flee_count) - 0.5) * 2.2;
            n.style = {
                let roll = unit(shared, 29 + n.flee_count);
                match mood {
                    Mood::Chaos if roll < 0.34 => Style::Circle,
                    Mood::Chaos if roll < 0.68 => Style::Hide,
                    Mood::Panic if roll < 0.25 => Style::Hide,
                    _ => Style::Scatter,
                }
            };
            n.hide = None;
            n.probe = 0.0;
            n.turn_until = 0.0;
            n.flee_until = n.flee_until.max(
                now + 3.0
                    + 3.0 * unit(n.seed, 13 + n.flee_count)
                    + if mood == Mood::Chaos { 3.0 } else { 0.0 },
            );
            if now >= n.shout_cd {
                n.shout_cd = now + SHOUT_COOLDOWN;
                v.raise(now, pos, Cry::Shout, Some(e));
            }
            if !n.dropped && !n.spec.carry.is_empty() && v.stats.drops < MAX_DROPS {
                n.dropped = true;
                if let Some(model) = carry_model(&n.spec.carry) {
                    v.stats.drops += 1;
                    let toss =
                        (-toward + Vec3::new(-toward.z, 0.0, toward.x) * 0.6).normalize_or_zero();
                    let drop_at = Transform {
                        translation: pos + toss * 0.4 + Vec3::Y * 1.1,
                        rotation: Quat::from_rotation_y(unit(n.seed, 31) * std::f32::consts::TAU),
                        scale: Vec3::ONE,
                    };
                    let prop =
                        super::props::spawn_copy(&mut commands, &assets, model, drop_at, None);
                    commands.entity(prop).insert((
                        RunEntity,
                        Dropped {
                            vel: toss * 1.4 + Vec3::Y * 2.4,
                            ttl: 4.0,
                        },
                    ));
                    info!(
                        "azeroll NPC drop: {} lets go of the {} at ({:.1}, {:.1})",
                        n.spec.name, n.spec.carry, pos.x, pos.z
                    );
                }
            }
        }
        counts[n.state as usize] += 1;

        // ---- do it -------------------------------------------------------------------------
        let mut dir = Vec3::ZERO;
        let mut speed = 0.0;
        let mut backing = false;
        let mut look: Option<Vec3> = None;
        let mut cheer = false;
        let state = n.state;

        // A guard bumps a ball that is a wall to it and in reach (Ram: the ball is knocked back).
        if role == Role::Guard
            && contact(ball.radius, axis) == Contact::Wall
            && distance < ball.radius + 0.6
            && n.wait == 0.0
            && matches!(state, State::Ambient | State::Curious | State::Notice)
        {
            fx.rams.write(super::collect::Ram {
                entity: e,
                name: c.name.clone(),
                dir: -toward,
            });
            fx.emote(e, 16);
            n.wait = 5.0;
        }

        match state {
            State::Ambient => {
                if now < n.look_until {
                    look = Some(flat(n.look_at - pos));
                }
                if !n.spec.path.is_empty() {
                    if n.wait == 0.0 {
                        let target = wow_to_bevy(n.spec.path[n.waypoint]);
                        let offset = flat(target - pos);
                        if offset.length() < 0.6 {
                            n.waypoint = (n.waypoint + 1) % n.spec.path.len();
                            n.wait = 4.0;
                        } else {
                            dir = offset.normalize_or_zero();
                            speed = 1.1;
                        }
                    }
                } else {
                    let back = flat(n.home - pos);
                    if back.length() > 0.9 {
                        // Sheepishly back to the post.
                        dir = back.normalize_or_zero();
                        speed = 1.5;
                    } else if look.is_none() && n.spec.activity != "chat" {
                        look = Some(-Vec3::new(n.spec.yaw.sin(), 0.0, n.spec.yaw.cos()));
                    }
                }
                if speed == 0.0
                    && look.is_none()
                    && role == Role::Villager
                    && n.spec.activity == "chat"
                {
                    if let Some(partner) = cast
                        .npc
                        .iter()
                        .filter(|p| p.name != n.spec.name && p.scene == n.spec.scene)
                        .map(|p| wow_to_bevy(p.at))
                        .filter(|p| {
                            (p.y - n.home.y).abs() < 2.0 && flat(*p - n.home).length() < 4.0
                        })
                        .min_by(|a, b| {
                            a.distance_squared(n.home)
                                .total_cmp(&b.distance_squared(n.home))
                        })
                    {
                        look = Some(partner - pos);
                    }
                }
                if n.timer <= 0.0 && !kid {
                    let activity = if n.quirk == Quirk::Polish {
                        "work"
                    } else {
                        n.spec.activity.as_str()
                    };
                    let id = match activity {
                        "work" => 62,
                        "sleep" => 100,
                        "eat" => 61,
                        _ => [60, 70, 68, 84][(n.phase as usize + now as usize / 5) % 4],
                    };
                    fx.emote(e, id);
                    n.timer = if id == 62 {
                        2.0
                    } else {
                        5.0 + n.phase.rem_euclid(3.0)
                    };
                }
            }
            State::Curious => {
                look = Some(toward);
                if kid {
                    // Kids run up to a small ball and cheer at it.
                    cheer = true;
                    if distance > ball.radius + KID_STOP {
                        dir = toward;
                        speed = 1.9;
                    }
                    if now >= n.voice_at
                        && distance < HEARD_FROM
                        && fx.voice(v, "CHEER", true, n.sex, pos, Prio::Low)
                    {
                        n.voice_at = now + 10.0 + 6.0 * unit(n.seed, 37 + now as u32 / 10);
                    }
                } else if n.timer <= 0.0 {
                    // A tiny thing is a joke; a bigger one something to point at.
                    let ids: &[u16] = if ball.diameter() < 0.8 {
                        &[70, 84, 60]
                    } else {
                        &[84, 65, 60]
                    };
                    fx.emote(e, ids[(n.seed as usize + now as usize / 4) % ids.len()]);
                    n.timer = 3.5 + 2.5 * unit(n.seed, 41 + now as u32 / 4);
                    if now >= n.voice_at
                        && distance < HEARD_FROM
                        && fx.voice(v, "JOKE", false, n.sex, pos, Prio::Low)
                    {
                        n.voice_at = now + 14.0 + 8.0 * unit(n.seed, 43);
                    }
                }
            }
            State::Notice => {
                look = Some(toward);
                if role == Role::Guard {
                    // Brace in a line: step up from the post toward the ball and stand there.
                    if n.spec.path.is_empty() {
                        let hold = n.home + flat(ball.pos - n.home).normalize_or_zero() * 3.0;
                        let to = flat(hold - pos);
                        if to.length() > 0.5 && distance > ball.radius + 2.5 {
                            dir = to.normalize_or_zero();
                            speed = 1.4;
                        }
                    }
                    if n.timer <= 0.0 {
                        fx.emote(e, [26, 24, 55][(n.seed as usize + now as usize / 3) % 3]);
                        n.timer = 3.5;
                    }
                } else {
                    if mood >= Mood::Nervous && distance < BACKAWAY_RANGE + ball.radius {
                        dir = -toward;
                        speed = 0.9;
                        backing = true;
                    }
                    if n.timer <= 0.0 && !kid {
                        fx.emote(e, [64, 65, 84][(n.seed as usize + now as usize / 3) % 3]);
                        n.timer = 3.0;
                    }
                }
            }
            State::Freeze => {
                look = Some(if now - n.alarm_t < 1.5 {
                    flat(n.alarm_at - pos)
                } else {
                    toward
                });
            }
            State::Cower => {
                look = Some(toward);
                if n.timer <= 0.0 && !kid {
                    fx.emote(e, if n.seed % 2 == 0 { 79 } else { 83 });
                    n.timer = 4.5;
                }
            }
            State::Charge => {
                look = Some(toward);
                dir = toward;
                speed = (0.5 * super::ball::top_speed(ball.radius)).clamp(1.8, 4.5);
                if distance <= ball.radius + 0.8 || v.touched.contains(&e) {
                    // Bounced off a ball that is far too big, then he runs.
                    n.bounce = -toward * 5.5 + Vec3::Y * 4.2;
                    n.state = State::Bounced;
                    v.stats.bounces += 1;
                    fx.emote(e, 121);
                    info!(
                        "azeroll NPC bounce: {} charged D={:.2} (axis {:.2}) and was thrown back",
                        n.spec.name,
                        ball.diameter(),
                        axis
                    );
                    dir = Vec3::ZERO;
                    speed = 0.0;
                }
            }
            State::Bounced => {
                look = Some(toward);
                n.bounce.y -= 14.0 * dt;
                let step = n.bounce * dt;
                let mut next_pos = pos + Vec3::new(step.x, 0.0, step.z);
                if let Ok(d3) = Dir3::new(flat(step).normalize_or_zero()) {
                    if collision
                        .ray_body(pos + Vec3::Y * 0.8, d3, flat(step).length() + 0.4)
                        .is_some()
                    {
                        next_pos = pos;
                        n.bounce.x = 0.0;
                        n.bounce.z = 0.0;
                    }
                }
                next_pos.y = pos.y + step.y;
                let ground = ground_at(next_pos, n.spec.indoors, &world, &collision)
                    .filter(|y| (*y - pos.y).abs() < 3.0);
                if n.bounce.y < 0.0 && ground.is_none_or(|g| next_pos.y <= g) {
                    next_pos.y = ground.unwrap_or(pos.y);
                    tf.translation = next_pos;
                    n.bounce = Vec3::ZERO;
                    n.state = State::Flee;
                    n.timer = 0.0;
                    n.flee_count += 1;
                    n.flee_until = now + 6.0;
                    n.style = Style::Scatter;
                    n.scatter = (unit(n.seed, 47) - 0.5) * 1.6;
                    fx.emote(e, 30);
                    info!(
                        "azeroll NPC reaction: {} Bounced -> Flee (the shaken guard retreats)",
                        n.spec.name
                    );
                } else {
                    tf.translation = next_pos;
                }
            }
            State::Flee => {
                // Run from the ball when it is near, otherwise from the last cry.
                let threat = if distance < AWARE + 10.0 + ball.radius {
                    ball.pos
                } else {
                    n.alarm_at
                };
                let away = -flat(threat - pos).normalize_or_zero();
                let shared = mate.map_or(n.seed, |m| m.seed.min(n.seed));
                let shared_phase = shared as f32 * 1.71;
                let zig = (now * 3.9 + shared_phase).sin();
                let base = Quat::from_rotation_y(n.scatter) * away;
                let side = Vec3::new(-base.z, 0.0, base.x);
                dir = (base + side * zig * 0.7).normalize_or_zero();
                let beyond = flat(pos - n.home).length() > LEASH;
                let style = if beyond { Style::Circle } else { n.style };
                match style {
                    Style::Scatter => {}
                    Style::Circle => {
                        // Round and round the post, shying from a ball that comes close.
                        let radial = flat(pos - n.home);
                        let tangent = Vec3::new(-radial.z, 0.0, radial.x).normalize_or_zero()
                            * if n.seed % 2 == 0 { 1.0 } else { -1.0 };
                        let inward = -radial.normalize_or_zero() * ((radial.length() - 5.0) * 0.3);
                        let shy = if distance < 5.0 + ball.radius {
                            away
                        } else {
                            Vec3::ZERO
                        };
                        dir = (tangent + inward + shy).normalize_or_zero();
                    }
                    Style::Hide => {
                        if let Some(spot) = n.hide {
                            let to = flat(spot - pos);
                            if to.length() < 0.8 {
                                n.state = State::Cower;
                                n.cower_until = now + 4.0 + 3.0 * unit(n.seed, 53);
                                n.timer = 0.0;
                                dir = Vec3::ZERO;
                                info!(
                                    "azeroll NPC hides: {} behind cover at ({:.1}, {:.1})",
                                    n.spec.name, spot.x, spot.z
                                );
                            } else {
                                dir = to.normalize_or_zero();
                            }
                        } else if n.probe <= now && probes > 0 {
                            n.probe = now + 1.2;
                            probes -= 1;
                            n.hide = find_hide(
                                pos,
                                ball.pos,
                                n.seed.wrapping_add(n.flee_count),
                                n.spec.indoors,
                                &world,
                                &collision,
                                &doors,
                            );
                            if n.hide.is_none() {
                                // Nowhere to hide: run like everyone else.
                                n.style = Style::Scatter;
                            }
                        }
                    }
                }
                if now < n.turn_until {
                    dir = (Quat::from_rotation_y(n.turn) * dir).normalize_or_zero();
                }
                // Hand in hand: stay beside the partner. Both use the pair's phase, so they
                // share pace and stumble together.
                if let Some(m) = mate.filter(|m| m.state == State::Flee) {
                    let gap = flat(m.pos - pos);
                    if gap.length() > 1.3 && gap.length() < 10.0 {
                        dir = (dir
                            + gap.normalize_or_zero() * ((gap.length() - 1.0) * 0.5).min(1.0))
                        .normalize_or_zero();
                    }
                }
                speed = if n.state == State::Flee {
                    flee_speed(axis, now * 2.4 + shared_phase, n.panic)
                } else {
                    0.0
                };
                // A half-second stumble every four seconds leaves an easy catch window; every
                // other one is a proper trip.
                let slice = ((now + shared_phase) / 4.0).floor() as i64;
                if (now + shared_phase).rem_euclid(4.0) < 0.55 {
                    speed = 0.0;
                    if n.stumbled != slice {
                        n.stumbled = slice;
                        if slice % 2 == 0 && !kid {
                            fx.emote(e, 121);
                        }
                    }
                }
                if n.timer <= 0.0 && !kid && speed > 0.0 {
                    fx.emote(e, if n.sex == 0 { 78 } else { 68 });
                    n.timer = 2.4;
                }
                if now >= n.voice_at
                    && distance < HEARD_FROM
                    && fx.voice(v, "FLEE", kid, n.sex, pos, Prio::Normal)
                {
                    n.voice_at = now + 6.0 + n.phase.rem_euclid(5.0);
                }
            }
        }

        // ---- move --------------------------------------------------------------------------
        let mut moved = false;
        if speed > 0.0 && n.state == state {
            moved = try_step(
                &mut tf,
                dir,
                speed,
                dt,
                n.spec.indoors,
                &world,
                &collision,
                &doors,
            );
            if !moved {
                speed = 0.0;
                n.wait = n.wait.max(0.8);
                if state == State::Flee {
                    // Walls are scenery: try another way for a moment, and beg while at it.
                    n.turn = if unit(n.seed, 59 + now as u32) < 0.5 {
                        1.3
                    } else {
                        -1.3
                    };
                    n.turn_until = now + 0.9;
                    if n.timer <= 0.8 && !kid {
                        fx.emote(e, 79);
                        n.timer = 3.0;
                    }
                }
            }
        } else if n.state != state {
            speed = 0.0;
        }
        if cheer {
            // Kids have no clips: face the ball, hop and wobble.
            let face = Quat::from_rotation_y(units::yaw_toward(if moved { dir } else { toward }));
            tf.rotation = face * Quat::from_rotation_z(0.35 * phase.sin());
            if let Some(ground) = world.terrain_height_under(tf.translation) {
                tf.translation.y = ground + 0.16 * (now * 9.0 + n.phase).sin().abs();
            }
        } else if moved && !backing {
            tf.rotation = Quat::from_rotation_y(units::yaw_toward(dir));
        } else if let Some(l) = look.filter(|l| l.length_squared() > 1e-4) {
            tf.rotation = tf.rotation.slerp(
                Quat::from_rotation_y(units::yaw_toward(l)),
                (dt * 6.0).min(1.0),
            );
        }
        mv.flags = if speed > 0.0 {
            (if backing {
                move_flags::BACKWARD
            } else {
                move_flags::FORWARD
            }) | if speed < 1.5 {
                move_flags::WALK_MODE
            } else {
                0
            }
        } else {
            0
        };
        mv.speed = speed;
        mv.vertical_speed = 0.0;
    }

    if now >= v.census_at {
        v.census_at = now + 5.0;
        let [amb, cur, not, fre, fle, cow, cha, bou] = counts;
        info!(
            "azeroll village: t={now:.0}s mood={:?} tier={} heat={} D={:.2} ambient={amb} curious={cur} notice={not} freeze={fre} flee={fle} cower={cow} charge={cha} bounced={bou} | reactions={} voices={} cries={} alarms={} drops={} charges={} bounces={} live_alarms={}",
            v.mood,
            v.tier,
            v.heat,
            ball.diameter(),
            v.stats.reactions,
            v.stats.voices,
            v.stats.cries,
            v.stats.alarms,
            v.stats.drops,
            v.stats.charges,
            v.stats.bounces,
            v.alarms.len(),
        );
    }
}

/// Dropped props hop to the ground and then belong to the ball like any other.
fn toss_drops(
    mut commands: Commands,
    time: Res<Time>,
    world: WorldPoint,
    collision: WorldCollision,
    mut drops: Query<(Entity, &mut Transform, &mut Dropped)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut tf, mut drop) in &mut drops {
        drop.vel.y -= 14.0 * dt;
        tf.translation += drop.vel * dt;
        drop.vel.x *= 1.0 - (2.0 * dt).min(1.0);
        drop.vel.z *= 1.0 - (2.0 * dt).min(1.0);
        drop.ttl -= dt;
        let floor = collision
            .ray_body(tf.translation + Vec3::Y * 1.5, Dir3::NEG_Y, 4.0)
            .filter(|h| h.normal.y > 0.6)
            .map(|h| tf.translation.y + 1.5 - h.distance);
        let ground = floor.or_else(|| world.terrain_height_under(tf.translation));
        let landed = ground.is_some_and(|g| drop.vel.y < 0.0 && tf.translation.y <= g);
        if landed {
            tf.translation.y = ground.unwrap_or(tf.translation.y);
        }
        if landed || drop.ttl <= 0.0 {
            commands.entity(e).remove::<Dropped>();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::level::COLLECT_RATIO;
    use super::*;

    /// The ball radius at which D = ratio * axis.
    fn radius_at(ratio: f32, axis: f32) -> f32 {
        ratio * axis * 0.5
    }

    #[test]
    fn cast_and_routes() {
        let c: Cast = toml::from_str(include_str!("goldshire-npcs.toml")).unwrap();
        assert_eq!(c.npc.len(), 33);
        assert_eq!(c.npc.iter().filter(|n| n.role == Role::Kid).count(), 7);
        assert!(
            c.npc
                .iter()
                .find(|n| n.name == "Kira Songshine")
                .unwrap()
                .path
                .len()
                > 6
        );
    }
    #[test]
    fn cast_funny_fields_resolve() {
        let c: Cast = toml::from_str(include_str!("goldshire-npcs.toml")).unwrap();
        for n in &c.npc {
            if !n.carry.is_empty() {
                assert!(carry_model(&n.carry).is_some(), "{}: {}", n.name, n.carry);
            }
            if !n.partner.is_empty() {
                let other = c.npc.iter().find(|p| p.name == n.partner).unwrap();
                assert_eq!(other.partner, n.name, "pairs are mutual");
            }
            if !n.quirk.is_empty() {
                assert_ne!(Quirk::parse(&n.quirk), Quirk::None, "{}", n.name);
            }
        }
        let find = |name: &str| c.npc.iter().find(|n| n.name == name).unwrap();
        assert_eq!(Quirk::parse(&find("Innkeeper Farley").quirk), Quirk::Polish);
        assert_eq!(Quirk::parse(&find("Marshal Dughan").quirk), Quirk::Brave);
        assert_eq!(find("Corina Steele").partner, "Kurran Steele");
    }
    #[test]
    fn mood_ladder_follows_the_tier_table() {
        assert_eq!(Mood::of_tier(0), Mood::Curious);
        assert_eq!(Mood::of_tier(1), Mood::Nervous);
        assert_eq!(Mood::of_tier(2), Mood::Nervous);
        assert_eq!(Mood::of_tier(3), Mood::Panic);
        assert_eq!(Mood::of_tier(4), Mood::Chaos);
        assert_eq!(Mood::of_tier(5), Mood::Chaos);
        assert!(Mood::Curious < Mood::Nervous && Mood::Panic < Mood::Chaos);
        // The ladder is defined over the real table: Panic is the row that names the adults.
        let rows = super::super::level::TIERS;
        assert_eq!(rows[3].1, "NPCs and horses");
    }
    #[test]
    fn fear_is_keyed_to_contact() {
        let axis = 2.0;
        // A wall to every role: nobody runs from it.
        for role in [Role::Guard, Role::Villager, Role::Kid] {
            assert_eq!(fear(role, radius_at(1.49, axis), axis, 1.0), Fear::Calm);
        }
        // Dislodge: notice. The villager bolts a margin before roll-up, the guard not at all.
        let notice = radius_at(1.5, axis);
        for role in [Role::Guard, Role::Villager, Role::Kid] {
            assert_ne!(fear(role, notice, axis, 0.0), Fear::Calm, "{role:?}");
        }
        assert_eq!(fear(Role::Villager, notice, axis, 0.0), Fear::Wary);
        assert_eq!(
            fear(Role::Villager, radius_at(1.9, axis), axis, 0.0),
            Fear::Wary
        );
        assert_eq!(
            fear(Role::Villager, radius_at(1.97, axis), axis, 0.0),
            Fear::Flee
        );
        assert_eq!(
            fear(Role::Kid, radius_at(1.75, axis), axis, 0.0),
            Fear::Flee
        );
        assert_eq!(fear(Role::Kid, radius_at(1.6, axis), axis, 0.0), Fear::Wary);
        // Guards hold until the ball can roll them up, and then run.
        assert_eq!(
            fear(Role::Guard, radius_at(2.14, axis), axis, 1.0),
            Fear::Wary
        );
        assert_eq!(
            fear(Role::Guard, radius_at(2.16, axis), axis, 0.0),
            Fear::Flee
        );
        // The classifier and the margin agree: the villager's flight is a RollUp for a ball
        // grown by the margin.
        let r = radius_at(1.97, axis);
        assert_eq!(contact(r * (1.0 + VILLAGER_MARGIN), axis), Contact::RollUp);
        assert_eq!(contact(r, axis), Contact::Dislodge);
    }
    #[test]
    fn contagion_pulls_flight_earlier_but_not_out_of_dislodge() {
        let axis = 2.0;
        let calm = radius_at(1.8, axis);
        assert_eq!(fear(Role::Villager, calm, axis, 0.0), Fear::Wary);
        assert_eq!(fear(Role::Villager, calm, axis, 1.0), Fear::Flee);
        // Even a fully infected villager ignores a wall.
        assert_eq!(
            fear(Role::Villager, radius_at(1.45, axis), axis, 1.0),
            Fear::Calm
        );
        // Guards are professionals.
        assert_eq!(fear(Role::Guard, calm, axis, 1.0), Fear::Wary);
    }
    #[test]
    fn calm_reaction_follows_the_mood() {
        let axis = 2.0;
        let r = 0.3;
        let near = |role, mood, d| reaction(role, mood, r, axis, 0.0, d, true);
        // Curious village: a glance at 6 yd, nothing at 30.
        assert_eq!(near(Role::Villager, Mood::Curious, 6.0), State::Curious);
        assert_eq!(near(Role::Villager, Mood::Curious, 30.0), State::Ambient);
        // Nervous village: close is wary, middling is curious.
        assert_eq!(near(Role::Villager, Mood::Nervous, 5.0), State::Notice);
        assert_eq!(near(Role::Villager, Mood::Nervous, 9.0), State::Curious);
        // Kids stay delighted at any mood while the ball is a wall.
        assert_eq!(near(Role::Kid, Mood::Chaos, 3.0), State::Curious);
        assert_eq!(
            reaction(Role::Villager, Mood::Curious, r, axis, 0.0, 6.0, false),
            State::Ambient
        );
    }
    #[test]
    fn catchable_bursts() {
        for a in [0.8, 1.1, 1.2, 1.9, 2.0, 2.3] {
            for p in 0..100 {
                for urgency in [0.0, 0.5, 1.0] {
                    assert!(
                        flee_speed(a, p as f32 * 0.37, urgency)
                            < super::super::ball::top_speed(a / COLLECT_RATIO),
                        "axis {a}"
                    );
                }
            }
        }
    }
    #[test]
    fn scatter_angles_differ_between_neighbours() {
        let mut angles: Vec<f32> = (1..=8).map(|s| (unit(s, 12) - 0.5) * 2.2).collect();
        angles.sort_by(f32::total_cmp);
        assert!(angles.windows(2).all(|w| w[1] - w[0] > 0.01));
        assert!(angles.iter().all(|a| a.abs() <= 1.1));
        assert!(angles.last().unwrap() - angles.first().unwrap() > 1.0);
    }
    #[test]
    fn voice_and_alarm_budgets_are_leaky_buckets() {
        let mut v = Village::default();
        assert!(v.take_voice(Prio::Normal));
        assert!(!v.take_voice(Prio::Low), "low priority leaves headroom");
        v.voices = 0.0;
        assert!(!v.take_voice(Prio::High));
        v.refill(10.0);
        assert!(v.voices <= VOICE_CAP);
        // Alarms: screams are free, the rest spend tokens, the live set is capped.
        let mut v = Village::default();
        for _ in 0..40 {
            v.raise(0.0, Vec3::ZERO, Cry::Shout, None);
        }
        assert!(v.alarms.len() <= ALARM_CAP as usize);
        for _ in 0..40 {
            v.raise(0.0, Vec3::ZERO, Cry::Scream, None);
        }
        assert_eq!(v.alarms.len(), MAX_ALARMS);
    }
    #[test]
    fn hashes_are_unit_interval() {
        for s in 0..64 {
            for salt in 0..8 {
                let u = unit(s, salt);
                assert!((0.0..1.0).contains(&u));
            }
        }
    }
}

/// Applied after the engine's visibility LOD, inside its animation window. Only remove parks
/// owned here; an offscreen rig keeps the engine's own park until it is visible again.
#[derive(Component)]
struct NpcFrozen;
#[allow(clippy::type_complexity)] // one Bevy system's component dependencies
fn park_carried(
    mut commands: Commands,
    ball: Res<BallState>,
    q: Query<(Entity, Option<&Attached>, &Collectible, Has<NpcFrozen>), With<Npc>>,
) {
    let mut recent: Vec<_> = q
        .iter()
        .filter_map(|(e, a, _, _)| a.map(|a| (e, a.at)))
        .collect();
    recent.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (e, a, c, ours) in &q {
        let freeze = a.is_some_and(|a| {
            !recent.iter().take(8).any(|(id, _)| *id == e) || a.radial + c.size < ball.radius * 0.65
        });
        if freeze {
            commands
                .entity(e)
                .insert((NpcFrozen, benilla_world::rig_anim::AnimParked));
        } else if ours {
            commands
                .entity(e)
                .remove::<(NpcFrozen, benilla_world::rig_anim::AnimParked)>();
        }
    }
}
pub(super) fn all_spawned(pending: Res<Pending>) -> bool {
    pending.rows.is_empty()
}

/// Seconds per rung of the ladder tour.
const LADDER_STAGE: f32 = 14.0;

/// `WOW_AZEROLL_NPC_TEST=1`: scenes, guard defiance, market panic, then collection.
/// `WOW_AZEROLL_NPC_TEST=ladder`: the mood ladder, one rung per stage, the ball rolling through the
/// crossroads crowd. Opt-in only; normal play never writes ball state here.
fn capture_tour(
    run: Res<Run>,
    mut ball: ResMut<BallState>,
    world: WorldPoint,
    mut last: Local<Option<usize>>,
) {
    let Some(mode) = crate::webenv::var("WOW_AZEROLL_NPC_TEST") else {
        return;
    };
    if mode.starts_with("ladder") {
        // Curious, nervous (critters), nervous (creatures: the brave guard charges), panic
        // (NPC-edible), chaos (forest giants). The ball starts south of the guard post and rolls
        // north through the crowd.
        const RUNGS: [f32; 6] = [0.3, 0.7, 1.6, 2.3, 3.4, 4.2];
        let stage = ((run.elapsed / LADDER_STAGE) as usize).min(RUNGS.len() - 1);
        let t = run.elapsed - stage as f32 * LADDER_STAGE;
        let radius = RUNGS[stage];
        let from = wow_to_bevy([-9468.0, 62.0, 57.0]);
        let to = wow_to_bevy([-9468.0, 94.0, 58.0]);
        let along = (t * 1.6 / (to - from).length()).min(1.0);
        ball.pos = from.lerp(to, along);
        if let Some(y) = world.terrain_height_under(ball.pos) {
            ball.pos.y = y + radius;
        }
        if *last != Some(stage) {
            *last = Some(stage);
            info!("azeroll NPC ladder stage={stage} r={radius}");
        }
        ball.radius = radius;
        ball.volume = radius.powi(3);
        ball.vel = Vec3::ZERO;
        ball.heading = units::yaw_toward(to - from);
        return;
    }
    let stage = (run.elapsed / 20.0) as usize;
    if *last == Some(stage) && stage >= 3 {
        return;
    }
    let changed = *last != Some(stage);
    *last = Some(stage);
    let (at, radius, target) = match stage {
        0 => ([-9465.0, 64.0, 57.0], 0.4, [-9465.5, 74.0, 57.0]),
        1 => ([-9472.0, 55.0, 57.0], 0.4, [-9465.9, 48.1, 57.0]),
        2 => ([-9465.0, 67.0, 57.0], 1.6, [-9465.5, 74.0, 57.0]),
        3 => ([-9464.0, 103.0, 58.0], 2.1, [-9461.9, 109.3, 58.0]),
        _ => (
            [-9465.0, 82.0 + (stage as f32 - 4.0) * 4.0, 58.0],
            2.8,
            [-9464.0, 94.0, 58.0],
        ),
    };
    ball.pos = wow_to_bevy(at);
    if let Some(y) = world.terrain_height_under(ball.pos) {
        ball.pos.y = y + radius;
    }
    ball.radius = radius;
    ball.volume = radius.powi(3);
    ball.vel = Vec3::ZERO;
    ball.heading = units::yaw_toward(wow_to_bevy(target) - ball.pos);
    if changed {
        info!("azeroll NPC capture stage={stage} r={radius}");
    }
}

fn capture_camera(
    run: Res<Run>,
    ball: Res<BallState>,
    mut smooth: Local<Option<(Vec3, Vec3)>>,
    actors: Query<(Entity, &Npc, &GlobalTransform), Without<benilla_world::view::WorldCamera>>,
    mut focus: Local<(Option<Entity>, f32)>,
    mut cameras: Query<&mut Transform, With<benilla_world::view::WorldCamera>>,
) {
    let Some(mode) = crate::webenv::var("WOW_AZEROLL_NPC_TEST") else {
        return;
    };
    if mode == "ladder-focus" {
        // Close on the nearest NPC that is doing something about the ball, from the far side so
        // it runs toward the lens with the ball behind it. Sticks to one NPC for a few seconds.
        let hot = |n: &Npc| {
            matches!(
                n.state,
                State::Flee | State::Freeze | State::Cower | State::Charge | State::Bounced
            )
        };
        if focus.0.is_none_or(|e| actors.get(e).is_err()) || run.elapsed > focus.1 {
            focus.0 = actors
                .iter()
                .filter(|(_, n, _)| hot(n))
                .map(|(e, _, tf)| (e, flat(tf.translation() - ball.pos).length()))
                .filter(|(_, d)| *d < 24.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(e, _)| e);
            focus.1 = run.elapsed + 5.0;
        }
        let Some((_, _, tf)) = focus.0.and_then(|e| actors.get(e).ok()) else {
            return;
        };
        let at = tf.translation();
        let away = flat(at - ball.pos).normalize_or(Vec3::X);
        let side = Vec3::new(-away.z, 0.0, away.x);
        let eye = at + away * 6.0 + side * 3.0 + Vec3::Y * 3.2;
        let target = at.lerp(ball.pos, 0.3) + Vec3::Y * 1.0;
        let (eye, target) = match *smooth {
            Some((e, t)) => (e.lerp(eye, 0.12), t.lerp(target, 0.12)),
            None => (eye, target),
        };
        *smooth = Some((eye, target));
        for mut camera in &mut cameras {
            *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        }
        return;
    }
    if mode == "ladder-side" {
        // From the open road side, looking across at the NPCs near the ball, with the ball in
        // the shot: the portrait window then holds the crowd as it scatters.
        let (mut sum, mut n) = (Vec3::ZERO, 0.0);
        for (_, _, tf) in &actors {
            let at = tf.translation();
            if flat(at - ball.pos).length() < 22.0 && (at.y - ball.pos.y).abs() < 12.0 {
                sum += at;
                n += 1.0;
            }
        }
        let forward = Vec3::new(-ball.heading.sin(), 0.0, -ball.heading.cos());
        let crowd = if n > 0.0 {
            sum / n
        } else {
            ball.pos + forward * 10.0
        };
        let toward = flat(crowd - ball.pos).normalize_or(forward);
        let middle = crowd.lerp(ball.pos, 0.35);
        let side = Vec3::new(toward.z, 0.0, -toward.x);
        let eye = middle + side * (17.0 + 2.0 * ball.radius) + Vec3::Y * (9.0 + 2.0 * ball.radius);
        let (eye, target) = match *smooth {
            Some((e, t)) => (e.lerp(eye, 0.08), t.lerp(middle + Vec3::Y, 0.08)),
            None => (eye, middle + Vec3::Y),
        };
        *smooth = Some((eye, target));
        for mut camera in &mut cameras {
            *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        }
        return;
    }
    if mode == "ladder" {
        // The game's own chase camera frames the ladder: it widens with the ball and fades
        // whatever stands between the lens and the ball, which a fixed shot cannot.
        return;
    }
    let stage = (run.elapsed / 20.0) as usize;
    let name = match stage {
        0 | 2 => "Marshal Dughan",
        1 => "Erma",
        3 => "Lyria Du Lac",
        _ => return,
    };
    let Some((_, _, tf)) = actors.iter().find(|(_, n, _)| n.spec.name == name) else {
        return;
    };
    let target = tf.translation() + Vec3::Y * 0.9;
    let eye = target + Vec3::new(-7.0, 4.0, -3.0);
    for mut camera in &mut cameras {
        *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}
