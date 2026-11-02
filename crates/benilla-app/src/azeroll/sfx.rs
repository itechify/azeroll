//! Azeroll's pickup audio: what rolling things up, and bumping into things, sounds like.
//!
//! The engine's kit player has no per-play pitch (only a kit's own `VARY_PITCH` flag), so the
//! Katamari "pitch climbs with a streak" idea is a ladder of different short kits instead. Plays go
//! out through the two message lanes the sound layer already serves: [`ServerSoundMessage`] for a
//! flat 2D one-shot by `SoundEntries` id and [`SpellKitSound::PlayAt`] for one at a world point.
//! Both pass the kit player's own gates, so a kit that does not exist or has no file is logged by it
//! and costs nothing here.
//!
//! Per pickup there are up to three layers, throttled to one flush per [`WINDOW`] so a pile of
//! things rolled up at once cannot become a wall of sound:
//!
//! * **the pop**: a short 2D kit that climbs [`LADDER`] while pickups keep coming within
//!   [`COMBO_SECS`] of each other;
//! * **the thing's own sound**: a creature's voice from `CreatureSoundData` (wound first, never the
//!   death cry), or a material kit chosen from a prop's name ([`material_of`]);
//! * **the thump**, for things big next to the ball: a `DeathImpacts` kit, heavier as they grow.
//!
//! A wall bump or a ram gets a dull thud scaled by its strength.
//!
//! Around the pickups sit the rest of the vocabulary (Batch 2, gaps #16 and #22):
//!
//! * **touches** ([`Touched`]): a thing the ball is too small to take but knocks loose gets the
//!   `PutDown*` kit of its material; a thing it only leans on gets a dull thud, and one just short
//!   of being knocked loose also wobbles (a second, quieter knock a beat later). Each entity is
//!   throttled on its own clock, and a touch the hard-bump path already covers is skipped;
//! * **cues** (2D): a chime per [`GrowthBeat`], a longer flourish per [`TierReached`] (the tier
//!   wins when both land together), and the [`Dash`] launch whoosh, wind-up and ready tick. None of
//!   them uses the finale's `QUESTCOMPLETED`, the record's `LEVELUP` or the ladder's kits, and all
//!   are held back once Hogger is rolled up;
//! * **the roll**: a quiet `Footsteps*` tick per turn of the ball, see [`roll`].
//!
//! Everything shares one voice budget ([`Budget`]) with the priority pickup > touch > roll, and every
//! decision is logged at info level (`azeroll sfx:`), since the audio can't be heard on the build
//! host.
//!
//! The kit player has no per-play volume either, so a quiet sound is placed on the ray from the
//! listener to its source at the distance where the kit's own 3D rolloff gives the wanted level
//! ([`emission_distance`]); it stays where it should be in the stereo field.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{CreatureVoice, CreatureVoiceCatalog, FootstepCatalog, SoundKitCatalog};
use benilla_world::world_point::{Subject, WorldPoint};

use super::collect::Ram;
use super::level::COLLECT_RATIO;
use super::progress::Flow;
use super::{
    BallState, Bumped, Collected, Collectible, Dash, GrowthBeat, Kind, Phase, Run, TierReached,
    TouchState, Touched,
};
use crate::creature_anim::SpellKitSound;
use crate::net::{NetEntity, ServerSoundKind, ServerSoundMessage};
use crate::sound::footsteps::Footsteps;
use crate::sound::AudioListener;

/// Pickups closer together than this many seconds keep a streak going.
const COMBO_SECS: f32 = 1.0;
/// The throttle: sounds are decided once per window, the biggest first.
const WINDOW: f32 = 0.1;
/// Most 3D voice or material sounds per flush.
const MAX_VOICES: usize = 2;
/// A bump quieter than this (yards/s) makes no sound; the same bump may not repeat inside
/// [`BUMP_GAP`].
const BUMP_MIN: f32 = 2.0;
const BUMP_GAP: f32 = 0.22;
/// Relative to the ball's capacity, a thing this full draws a thump, and a heavy one.
const THUMP_FILL: f32 = 0.82;
const HEAVY_FILL: f32 = 0.94;
/// At most one thump per this many seconds.
const THUMP_GAP: f32 = 0.3;

// --- The voice budget. ---
// The kit player keeps 12 software channels and, when full, lets a louder newcomer steal the
// quietest voice. Estimated live voices are tracked here so the quiet layers give way first: a
// play is admitted only while fewer than its priority's ceiling are live.
const CEIL_PICKUP: usize = 10;
const CEIL_TOUCH: usize = 6;
const CEIL_ROLL: usize = 3;
/// Rough lengths of what each layer plays, seconds, for the live-voice estimate.
const EST_POP: f32 = 0.5;
const EST_VOICE: f32 = 0.8;
const EST_THUMP: f32 = 0.9;
const EST_TOUCH: f32 = 0.5;
const EST_ROLL: f32 = 0.5;
const EST_DASH: f32 = 0.9;
const EST_GROWTH: f32 = 1.6;
const EST_TIER: f32 = 4.9;

// --- Touches. ---
/// Closing speed (yards/s) under which a dislodged thing makes no knock; the launch itself needs 0.5.
const KNOCK_MIN: f32 = 0.6;
/// The same for a wall touch, a dull thud. Harder ones are `Bumped` and handled above.
const LEAN_MIN: f32 = 1.0;
/// Seconds before the same entity may sound again, per kind of touch.
const KNOCK_GAP: f32 = 0.9;
const LEAN_GAP: f32 = 1.2;
/// Touches are decided once per window, the hardest first, at most this many per window.
const TOUCH_WINDOW: f32 = 0.12;
const MAX_TOUCHES: usize = 2;
/// A leaned-on thing whose axis is within `ball diameter / ALMOST_FROM` of knock-loose size wobbles
/// ("almost"): `DISLODGE_RATIO` is the knock-loose line, this is the ratio where the hint starts.
const ALMOST_FROM: f32 = 1.2;
/// The wobble's second knock: seconds after the first, and how much of its level it keeps.
const WOBBLE_DELAY: f32 = 0.13;
const WOBBLE_KEEP: f32 = 0.6;
/// A prop with a longest axis (yards) from here up takes its material's large kit.
const LARGE_AXIS: f32 = 0.9;

// --- The roll. ---
/// Under this fraction of top speed the ball is as good as still: no sound.
const ROLL_MIN_FRAC: f32 = 0.2;
/// Wanted level of a tick (a final amplitude, before the SFX slider) at that and at top speed.
const ROLL_AMP_MIN: f32 = 0.09;
const ROLL_AMP_MAX: f32 = 0.26;
/// How much louder each size band is than the one before it, as a fraction of the amplitude.
const ROLL_BAND_BOOST: f32 = 0.1;
/// One tick per turn of the ball, kept between these many seconds.
const ROLL_GAP_MIN: f32 = 0.32;
const ROLL_GAP_MAX: f32 = 1.1;
/// The ground is asked again this often while the ball rolls.
const GROUND_EVERY: f32 = 0.35;
/// `FootstepTerrainLookup`'s class for the `CharacterMediumLarge*` kits.
const CHARACTER_CLASS: u32 = 7;
/// The roll steps aside this long after a louder sound, so it never covers one.
const HOLD_PICKUP: f32 = 0.3;
const HOLD_TOUCH: f32 = 0.2;
const HOLD_DASH: f32 = 0.6;
const HOLD_CHIME: f32 = 0.5;
const HOLD_FLOURISH: f32 = 1.0;

/// The kit player's 3D rolloff, `sound::math::ROLLOFF_FACTOR`: gain = min / (min + 4 (d - min)).
const ROLLOFF_FACTOR: f32 = 4.0;
/// The quietest attenuation worth asking for: below it the sound nears the kit's cutoff.
const MIN_ATTENUATION: f32 = 0.1;

/// A kit: `SoundEntries` id and its name, for the log.
#[derive(Clone, Copy)]
struct Kit(u32, &'static str);

/// Who gets a voice first when the budget is tight: pickups and cues, then touches, then the roll.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Prio {
    Pickup,
    Touch,
    Roll,
}

impl Prio {
    fn ceiling(self) -> usize {
        match self {
            Prio::Pickup => CEIL_PICKUP,
            Prio::Touch => CEIL_TOUCH,
            Prio::Roll => CEIL_ROLL,
        }
    }
}

/// Estimated live voices: when each play is expected to end, in real seconds.
#[derive(Default)]
struct Budget {
    ends: Vec<f32>,
}

impl Budget {
    fn live(&mut self, now: f32) -> usize {
        self.ends.retain(|end| *end > now);
        self.ends.len()
    }

    /// Admit a play of `secs` if the budget has room at its priority.
    fn admit(&mut self, now: f32, prio: Prio, secs: f32) -> bool {
        if self.live(now) >= prio.ceiling() {
            return false;
        }
        self.ends.push(now + secs);
        true
    }

    /// Record a play that is made regardless (the pop, a cue).
    fn force(&mut self, now: f32, secs: f32) {
        self.ends.push(now + secs);
    }
}

// --- The cues. 2D, so they do not depend on where the camera is. ---
/// A growth step: the quest-objective tick (1.6 s).
const GROWTH_CHIME: Kit = Kit(3201, "igBonusBarOpen");
/// A tier: the new-area discovery sting (4.9 s), longer and fuller than the chime. The tier
/// banner's own `igQuestListOpen` plays under it; `QUESTCOMPLETED` and `LEVELUP` stay the finale's
/// and the record's.
const TIER_FLOURISH: Kit = Kit(1519, "TaxiNodeDiscovered");
/// The dash launch: a warrior's special swing, five whooshes of 0.6 to 0.9 s.
const DASH_LAUNCH: Kit = Kit(3091, "SwingWeaponSpecialWarrior");
/// The dash wind-up, for when the ball has one (`Dash::WindUp` is reserved and not sent today).
const DASH_WINDUP: Kit = Kit(5674, "Whirlwind600%");
/// The dash is ready again: a short, clean tick (0.14 s).
const DASH_READY: Kit = Kit(1115, "UChatScrollButton");

/// The ground the ball rolls on, as far as `Footsteps*` kits tell surfaces apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Ground {
    #[default]
    Dirt,
    Grass,
    Stone,
    Wood,
    Snow,
}

impl Ground {
    /// The surface a `CharacterMediumLarge*` kit stands for; mud and anything unknown count as dirt.
    fn of_character_kit(kit: u32) -> Ground {
        match kit {
            562 => Ground::Grass,
            563 => Ground::Snow,
            564 => Ground::Stone,
            649 => Ground::Wood,
            _ => Ground::Dirt,
        }
    }
}

/// The size bands of the roll, by ball diameter: the tiers of `level::TIERS` where the world
/// changes scale (critters, NPCs and horses, Hogger), so the sound steps up with the HUD's tiers.
const ROLL_BANDS: [f32; 3] = [1.04, 4.33, 8.6];

/// Which band a ball of this diameter is in, 0 (tiny) .. 3 (Hogger-sized).
fn size_band(diameter: f32) -> usize {
    ROLL_BANDS.iter().filter(|edge| diameter >= **edge).count()
}

/// The roll's kit: the footstep kit of a size (small, medium-large, huge, giant) on a ground.
fn roll_kit(band: usize, ground: Ground) -> Kit {
    match (band.min(3), ground) {
        (0, Ground::Dirt) => Kit(650, "CharacterSmallDirt"),
        (0, Ground::Grass) => Kit(651, "CharacterSmallGrass"),
        (0, Ground::Stone) => Kit(653, "CharacterSmallStone"),
        (0, Ground::Wood) => Kit(654, "CharacterSmallWood"),
        (0, Ground::Snow) => Kit(652, "CharacterSmallSnow"),
        (1, Ground::Dirt) => Kit(560, "CharacterMediumLargeDirt"),
        (1, Ground::Grass) => Kit(562, "CharacterMediumLargeGrass"),
        (1, Ground::Stone) => Kit(564, "CharacterMediumLargeStone"),
        (1, Ground::Wood) => Kit(649, "CharacterMediumLargeWood"),
        (1, Ground::Snow) => Kit(563, "CharacterMediumLargeSnow"),
        (2, Ground::Dirt) => Kit(1417, "FootStepsHugeDirt"),
        (2, Ground::Grass) => Kit(1421, "FootStepsHugeGrass"),
        (2, Ground::Stone) => Kit(1418, "FootStepsHugeStone"),
        (2, Ground::Wood) => Kit(1420, "FootStepsHugeWood"),
        (2, Ground::Snow) => Kit(1419, "FootStepsHugeSnow"),
        (_, Ground::Dirt) => Kit(732, "CharacterGiantDirt"),
        (_, Ground::Grass) => Kit(731, "CharacterGiantGrass"),
        (_, Ground::Stone) => Kit(733, "CharacterGiantStone"),
        (_, Ground::Wood) => Kit(735, "CharacterGiantWood"),
        (_, Ground::Snow) => Kit(734, "CharacterGiantSnow"),
    }
}

const GROUNDS: [Ground; 5] = [
    Ground::Dirt,
    Ground::Grass,
    Ground::Stone,
    Ground::Wood,
    Ground::Snow,
];

/// Seconds between roll ticks: one per turn of the ball, so a big ball rolls slower and heavier.
fn roll_interval(radius: f32, speed: f32) -> f32 {
    (std::f32::consts::TAU * radius / speed.max(1.0e-3)).clamp(ROLL_GAP_MIN, ROLL_GAP_MAX)
}

/// Wanted amplitude of a roll tick at `frac` of top speed, in a size `band`.
fn roll_amp(frac: f32, band: usize) -> f32 {
    let f = frac.clamp(0.0, 1.0);
    (ROLL_AMP_MIN + (ROLL_AMP_MAX - ROLL_AMP_MIN) * f) * (1.0 + ROLL_BAND_BOOST * band as f32)
}

/// How far from the listener to put a source so that a kit with this `MinDistance` plays at
/// `attenuation` (0..1) of its level: the inverse of the kit player's rolloff.
fn emission_distance(min_dist: f32, attenuation: f32) -> f32 {
    let a = attenuation.clamp(MIN_ATTENUATION, 1.0);
    if a >= 1.0 {
        return min_dist * 0.9;
    }
    min_dist + (min_dist / a - min_dist) / ROLLOFF_FACTOR
}

/// The point on the ray from `listener` to `source` where a kit of `min_dist` and `volume` plays at
/// the final amplitude `amp`; the source itself when the kit is not known.
fn emission_point(
    kits: Option<&SoundKitCatalog>,
    kit: u32,
    listener: Vec3,
    source: Vec3,
    amp: f32,
) -> Vec3 {
    let Some(k) = kits.and_then(|k| k.get(kit)) else {
        return source;
    };
    let dir = (source - listener).normalize_or(Vec3::NEG_Z);
    listener + dir * emission_distance(k.min_distance, amp / k.volume.max(1.0e-3))
}

/// The pop's ladder, brightest last: a gem tink, a ring, a coin, a wand sparkle.
const LADDER: [Kit; 4] = [
    Kit(1221, "PickupGems"),
    Kit(1193, "PickUpRing"),
    Kit(895, "LOOTWINDOWCOINSOUND"),
    Kit(1196, "PickUpWand"),
];

/// What a prop is made of, as far as its name says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Material {
    Wood,
    Metal,
    Cloth,
    Food,
    Meat,
    Glass,
    Paper,
    Plant,
    Stone,
    Chain,
}

impl Material {
    /// The `PickUp*` interface kit of this material, the small or the large one.
    fn kit(self, large: bool) -> Kit {
        match (self, large) {
            (Material::Wood, false) => Kit(1198, "PickUpWoodSmall"),
            (Material::Wood, true) => Kit(1199, "PickUpWoodLarge"),
            (Material::Metal, false) => Kit(1191, "PickUpMetalSmall"),
            (Material::Metal, true) => Kit(1190, "PickUpMetalLarge"),
            (Material::Cloth, false) => Kit(1185, "PickUpCloth_Leather"),
            (Material::Cloth, true) => Kit(1183, "PickUpBag"),
            (Material::Food, _) => Kit(1186, "PickUpFoodGeneric"),
            (Material::Meat, _) => Kit(1189, "PickUpMeat"),
            (Material::Glass, _) => Kit(1197, "PickUpWater_Liquid"),
            (Material::Paper, false) => Kit(1192, "PickUpParchment_Paper"),
            (Material::Paper, true) => Kit(1184, "PickupBook"),
            (Material::Plant, _) => Kit(1187, "PickUpHerb"),
            (Material::Stone, _) => Kit(1194, "PickUpRocks_Ore"),
            (Material::Chain, false) => Kit(1195, "PickUpSmallChain"),
            (Material::Chain, true) => Kit(1188, "PickUpLargeChain"),
        }
    }

    /// The `PutDown*` interface kit of this material: what it sounds like knocked down onto
    /// something, the pickup's counterpart. Glass clinks like gems.
    fn knock(self, large: bool) -> Kit {
        match (self, large) {
            (Material::Wood, false) => Kit(1217, "PutDownWoodSmall"),
            (Material::Wood, true) => Kit(1216, "PutDownWoodLarge"),
            (Material::Metal, false) => Kit(1213, "PutDownSmallMEtal"),
            (Material::Metal, true) => Kit(1207, "PutDownLargeMetal"),
            (Material::Cloth, false) => Kit(1202, "PutDownClothLeather"),
            (Material::Cloth, true) => Kit(1200, "PutDownBag"),
            (Material::Food, _) => Kit(1203, "PutDownFoodGeneric"),
            (Material::Meat, _) => Kit(1208, "PutDownMeat"),
            (Material::Glass, _) => Kit(1204, "PutDownGems"),
            (Material::Paper, false) => Kit(1209, "PutDownParchment_Paper"),
            (Material::Paper, true) => Kit(1201, "PutDownBook"),
            (Material::Plant, _) => Kit(1205, "PutDownHerb"),
            (Material::Stone, _) => Kit(1211, "PutDownRock_Ore"),
            (Material::Chain, false) => Kit(1212, "PutDownSmallChain"),
            (Material::Chain, true) => Kit(1206, "PutDownLargeChain"),
        }
    }

    /// Which surface family a thump of this material lands as.
    fn thump_surface(self) -> Surface {
        match self {
            Material::Wood | Material::Paper | Material::Cloth => Surface::Wood,
            Material::Metal | Material::Stone | Material::Chain | Material::Glass => Surface::Stone,
            Material::Plant | Material::Food | Material::Meat => Surface::Grass,
        }
    }
}

/// The surface family of a `DeathImpacts` kit.
#[derive(Clone, Copy)]
enum Surface {
    Wood,
    Stone,
    Grass,
}

/// A `DeathImpacts` thump by weight (0 small .. 3 giant) and surface.
fn thump_kit(weight: usize, surface: Surface) -> Kit {
    match (weight.min(3), surface) {
        (0, Surface::Wood) => Kit(911, "DeathThudSmallWood"),
        (0, Surface::Stone) => Kit(910, "DeathThudSmallStone"),
        (0, Surface::Grass) => Kit(908, "DeathThudSmallGrass"),
        (1, Surface::Wood) => Kit(916, "DeathThudMediumWood"),
        (1, Surface::Stone) => Kit(915, "DeathThudMediumStone"),
        (1, Surface::Grass) => Kit(913, "DeathThudMediumGrass"),
        (2, Surface::Wood) => Kit(921, "DeathThudLargeWood"),
        (2, Surface::Stone) => Kit(920, "DeathThudLargeStone"),
        (2, Surface::Grass) => Kit(918, "DeathThudLargeGrass"),
        (_, Surface::Wood) => Kit(926, "DeathThudGiantWood"),
        (_, Surface::Stone) => Kit(925, "DeathThudGiantStone"),
        (_, Surface::Grass) => Kit(923, "DeathThudGiantGrass"),
    }
}

/// The material a prop's readable name suggests, or `None` for a name that says nothing.
fn material_of(name: &str) -> Option<Material> {
    let n = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    // Order matters: the specific before the general ("wine barrel" is wood, "wine bottle" glass).
    Some(if has(&["chain"]) {
        Material::Chain
    } else if has(&[
        "bottle", "flask", "vial", "potion", "jar", "jug", "pitcher", "glass", "urn", "lantern",
    ]) {
        Material::Glass
    } else if has(&[
        "book",
        "scroll",
        "paper",
        "map",
        "letter",
        "tome",
        "parchment",
        "note",
    ]) {
        Material::Paper
    } else if has(&[
        "sack", "bag", "banner", "flag", "tent", "rug", "carpet", "cloth", "pillow", "cushion",
        "curtain", "tapestry", "sheet", "bedroll", "blanket", "tabard", "hat", "robe", "net",
        "shirt", "boot", "leather", "rope", "yarn", "pants", "glove", "cape", "cloak", "tunic",
        "vest", "belt", "dress", "bracer",
    ]) {
        Material::Cloth
    } else if has(&[
        "ham",
        "meat",
        "steak",
        "mutton",
        "turkey",
        "fish",
        "sausage",
        "ribs",
        "haunch",
        "drumstick",
        "bacon",
        "jerky",
    ]) {
        Material::Meat
    } else if has(&["barrel", "cask", "keg", "crate", "chest"]) {
        // A container noun beats the food or drink word it holds: "Wine Barrel" is wood, not wine.
        Material::Wood
    } else if has(&["bucket", "pail"]) {
        // The same goes for a metal bucket of ale: the bucket decides.
        Material::Metal
    } else if has(&[
        "bread", "loaf", "pie", "cheese", "apple", "pumpkin", "melon", "cabbage", "turnip", "corn",
        "cake", "egg", "stew", "food", "carrot", "pear", "grape", "ale", "wine", "mug", "tankard",
        "goblet", "cup", "stein", "muffin", "orange", "banana", "bun", "biscuit", "cookie",
        "berry", "peach", "plum", "lemon", "tomato", "potato", "onion", "radish", "mango",
        "coconut",
    ]) {
        if has(&["mug", "tankard", "goblet", "cup", "stein"]) {
            Material::Metal
        } else {
            Material::Food
        }
    } else if has(&[
        "bush", "flower", "grass", "plant", "hay", "weed", "fern", "mushroom", "shrub", "herb",
        "vine", "leaf", "wheat", "stalk", "tree", "pine", "oak", "bramble", "thistle",
    ]) {
        Material::Plant
    } else if has(&[
        "rock", "stone", "boulder", "brick", "pillar", "statue", "well", "grave", "tomb", "wall",
        "ore", "column", "chimney", "mill",
    ]) {
        Material::Stone
    } else if has(&[
        "anvil", "sword", "axe", "helm", "shield", "armor", "kettle", "pot", "pan", "bell", "lamp",
        "hammer", "tool", "pick", "shovel", "forge", "cauldron", "cannon", "nail", "grate", "key",
        "lock", "plate", "fork", "knife", "spoon", "tong", "weapon", "bucket", "pail",
    ]) {
        Material::Metal
    } else if has(&[
        "barrel", "crate", "box", "chest", "fence", "log", "plank", "table", "chair", "bench",
        "stool", "cart", "wagon", "wood", "stump", "door", "shelf", "cabinet", "keg", "sign",
        "post", "pole", "stake", "ladder", "trunk", "bed", "wheel", "gate", "board", "tray",
        "basket", "bookcase", "cask", "arrow", "quiver", "bowl", "crossbow", "bow",
    ]) {
        Material::Wood
    } else {
        return None;
    })
}

/// A creature's own voice from its `CreatureSoundData` row: the wound cry first, then the aggro
/// bark, the alert, the stand vocal, a fidget and the attack grunt. Never the death cry.
fn voice_kit(v: &CreatureVoice, kits: Option<&SoundKitCatalog>) -> Option<(u32, &'static str)> {
    [
        (v.injury[0], "wound"),
        (v.aggro, "aggro"),
        (v.alert, "alert"),
        (v.stand, "stand"),
        (v.fidget[0], "fidget"),
        (v.exertion[0], "attack"),
    ]
    .into_iter()
    // A row can name a kit `SoundEntries` does not have (the rabbit's wound, 1025): skip it.
    .find(|(kit, _)| {
        *kit != 0 && kits.is_none_or(|k| k.get(*kit).is_some_and(|k| !k.files.is_empty()))
    })
}

/// The creature voice catalog, read once the archives are open.
#[derive(Resource, Default)]
struct Voices {
    catalog: Option<CreatureVoiceCatalog>,
    /// The kit table, to skip a voice column that names a kit that is not there.
    kits: Option<SoundKitCatalog>,
    tried: bool,
}

/// One pickup waiting for the next flush.
struct Pickup {
    name: String,
    kind: Kind,
    size: f32,
    /// Maximum-axis pickup gate against what the ball can take.
    fill: f32,
    /// Where it was, Bevy space.
    at: Vec3,
    /// Its creature display, when it has one.
    display: Option<u32>,
}

/// A bump or ram waiting for the next flush.
struct Knock {
    strength: f32,
    name: String,
    /// A ram's attacker, whose aggro bark goes with the thud.
    attacker: Option<u32>,
}

/// A touch waiting for the next touch window.
struct Touch {
    name: String,
    kind: Kind,
    state: TouchState,
    strength: f32,
    /// Where on the ball, Bevy space.
    point: Vec3,
    /// The thing's longest axis, yards.
    axis: f32,
    /// Ball diameter over that axis: 1.5 and up would knock it loose.
    ratio: f32,
}

/// A knock that was held back a beat: the second half of a wobble.
struct Later {
    /// Real seconds at which it plays.
    at: f32,
    kit: Kit,
    point: Vec3,
    amp: f32,
    why: String,
}

/// A one-off sound from another system's event, played the frame it is seen.
enum Cue {
    Growth {
        step: u32,
        diameter: f32,
    },
    Tier {
        tier: usize,
        name: &'static str,
        diameter: f32,
    },
    DashWindUp,
    DashStart,
    DashReady,
}

/// What the run has sounded, for the summary line.
#[derive(Default)]
struct Stats {
    pops: u32,
    voices: u32,
    thumps: u32,
    bumps: u32,
    knocks: u32,
    leans: u32,
    wobbles: u32,
    touches_dropped: u32,
    rolls: u32,
    rolls_dropped: u32,
    cues: u32,
    cues_held: u32,
}

/// The throttle's state and the streak.
#[derive(Resource, Default)]
struct Mix {
    pickups: Vec<Pickup>,
    knocks: Vec<Knock>,
    touches: Vec<Touch>,
    later: Vec<Later>,
    cues: Vec<Cue>,
    /// Real seconds each entity last sounded a touch.
    touched_at: HashMap<Entity, f32>,
    /// Seconds until the next flush may run.
    wait: f32,
    /// Pickups in the current streak, and seconds since the last one.
    combo: u32,
    since_pickup: f32,
    since_bump: f32,
    since_thump: f32,
    /// Seconds until the next touch window, and until the next roll tick.
    touch_wait: f32,
    roll_wait: f32,
    /// Seconds the roll keeps quiet after a louder sound.
    roll_hold: f32,
    /// The ground last read under the ball, and seconds until it is read again.
    ground: Ground,
    ground_wait: f32,
    budget: Budget,
    stats: Stats,
}

impl Mix {
    /// Quiet the roll for at least `secs`.
    fn hold_roll(&mut self, secs: f32) {
        self.roll_hold = self.roll_hold.max(secs);
    }
}

/// The two lanes of the sound layer, so a system takes one parameter for both.
#[derive(SystemParam)]
struct Sink<'w> {
    flat: MessageWriter<'w, ServerSoundMessage>,
    at: MessageWriter<'w, SpellKitSound>,
}

impl Sink<'_> {
    /// A flat one-shot, heard at its kit's own level.
    fn two_d(&mut self, kit: Kit, why: &str) {
        info!("azeroll sfx: 2d {} ({}) {why}", kit.1, kit.0);
        self.flat.write(ServerSoundMessage {
            kind: ServerSoundKind::Sound2d,
            sound_id: kit.0,
            source: None,
        });
    }

    /// A one-shot at a world point.
    fn world(&mut self, kit: u32, name: &str, pos: Vec3, why: &str) {
        info!("azeroll sfx: 3d {name} ({kit}) {why}");
        self.at.write(SpellKitSound::PlayAt {
            pos,
            kit_sound: kit,
        });
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Voices>()
        .init_resource::<Mix>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(OnEnter(Phase::Results), summary)
        .add_systems(
            Update,
            (
                load_voices,
                gather,
                gather_touches,
                gather_cues,
                flush,
                touch_flush,
                play_cues,
                roll.run_if(super::playing),
            )
                .chain(),
        );
}

fn reset(mut mix: ResMut<Mix>) {
    // The ground read and the roll's clocks restart with the run; the stats are logged first.
    *mix = Mix::default();
}

/// One line on what the run sounded, when it ends.
fn summary(mix: Res<Mix>) {
    let s = &mix.stats;
    info!(
        "azeroll sfx: run summary: pops {} voices {} thumps {} bumps {} | knocks {} leans {} \
         wobbles {} dropped {} | rolls {} dropped {} | cues {} held {}",
        s.pops,
        s.voices,
        s.thumps,
        s.bumps,
        s.knocks,
        s.leans,
        s.wobbles,
        s.touches_dropped,
        s.rolls,
        s.rolls_dropped,
        s.cues,
        s.cues_held
    );
}

fn load_voices(mut voices: ResMut<Voices>, assets: Option<Res<WorldAssets>>) {
    if voices.tried {
        return;
    }
    let Some(assets) = assets else { return };
    voices.tried = true;
    let (loaded, kits) = {
        let mut chain = assets.chain.lock_recover();
        (
            benilla_formats::load_creature_voice_catalog(&mut chain),
            benilla_formats::load_sound_kit_catalog(&mut chain),
        )
    };
    match kits {
        Ok(kits) => {
            check_tables(&kits);
            voices.kits = Some(kits);
        }
        Err(e) => warn!("azeroll sfx: kit table unavailable, voices are not pre-checked: {e:#}"),
    }
    match loaded {
        Ok(catalog) => {
            info!("azeroll sfx: {} creature voice rows", catalog.len());
            voices.catalog = Some(catalog);
        }
        Err(e) => warn!("azeroll sfx: creature voices unavailable, no creature sounds: {e:#}"),
    }
}

const MATERIALS: [Material; 10] = [
    Material::Wood,
    Material::Metal,
    Material::Cloth,
    Material::Food,
    Material::Meat,
    Material::Glass,
    Material::Paper,
    Material::Plant,
    Material::Stone,
    Material::Chain,
];

/// Every kit this module can play, in groups for the log.
fn all_kits() -> Vec<(&'static str, Vec<Kit>)> {
    let mut pickups = LADDER.to_vec();
    let mut knocks = Vec::new();
    for m in MATERIALS {
        pickups.push(m.kit(false));
        pickups.push(m.kit(true));
        knocks.push(m.knock(false));
        knocks.push(m.knock(true));
    }
    let mut thumps = Vec::new();
    for w in 0..4 {
        for s in [Surface::Wood, Surface::Stone, Surface::Grass] {
            thumps.push(thump_kit(w, s));
        }
    }
    let mut rolls = Vec::new();
    for band in 0..4 {
        for g in GROUNDS {
            rolls.push(roll_kit(band, g));
        }
    }
    let cues = vec![
        GROWTH_CHIME,
        TIER_FLOURISH,
        DASH_LAUNCH,
        DASH_WINDUP,
        DASH_READY,
    ];
    vec![
        ("pickup", pickups),
        ("thump", thumps),
        ("knock", knocks),
        ("roll", rolls),
        ("cue", cues),
    ]
}

/// The kits of [`all_kits`] the table lacks or has no file for.
fn missing_kits(kits: &SoundKitCatalog) -> Vec<String> {
    all_kits()
        .into_iter()
        .flat_map(|(_, group)| group)
        .filter(|k| !kits.get(k.0).is_some_and(|e| !e.files.is_empty()))
        .map(|k| format!("{} ({})", k.1, k.0))
        .collect()
}

/// Log whether every kit this module hard-codes exists and has a file, once the table is read.
fn check_tables(kits: &SoundKitCatalog) {
    let groups = all_kits();
    let total: usize = groups.iter().map(|(_, g)| g.len()).sum();
    let missing = missing_kits(kits);
    let sizes: Vec<_> = groups
        .iter()
        .map(|(name, g)| format!("{name} {}", g.len()))
        .collect();
    info!(
        "azeroll sfx: kit table check: {total} kits ({}), {} missing {missing:?}",
        sizes.join(", "),
        missing.len()
    );
}

/// Queue this frame's pickups, bumps and rams.
#[allow(clippy::too_many_arguments)]
fn gather(
    mut collected: MessageReader<Collected>,
    mut bumped: MessageReader<Bumped>,
    mut rams: MessageReader<Ram>,
    ball: Res<BallState>,
    time: Res<Time<Real>>,
    mut mix: ResMut<Mix>,
    things: Query<(&GlobalTransform, Option<&NetEntity>)>,
) {
    let dt = time.delta_secs();
    mix.wait -= dt;
    mix.since_pickup += dt;
    mix.since_bump += dt;
    mix.since_thump += dt;
    let capacity = (ball.radius * COLLECT_RATIO).max(1.0e-3);
    for c in collected.read() {
        if mix.since_pickup > COMBO_SECS {
            mix.combo = 0;
        }
        mix.combo += 1;
        mix.since_pickup = 0.0;
        let (at, display) = match things.get(c.entity) {
            Ok((gt, net)) => (gt.translation(), net.and_then(|n| n.display_id)),
            Err(_) => (ball.pos, None),
        };
        mix.pickups.push(Pickup {
            name: c.name.to_string(),
            kind: c.kind,
            size: c.size,
            fill: c.pickup / capacity,
            at,
            display,
        });
    }
    for b in bumped.read() {
        if b.strength >= BUMP_MIN {
            mix.knocks.push(Knock {
                strength: b.strength,
                name: b.name.to_string(),
                attacker: None,
            });
        }
    }
    for r in rams.read() {
        let display = things
            .get(r.entity)
            .ok()
            .and_then(|(_, net)| net.and_then(|n| n.display_id));
        mix.knocks.push(Knock {
            strength: 18.0,
            name: r.name.to_string(),
            attacker: display,
        });
    }
}

/// Which rung of [`LADDER`] a streak of `combo` pickups is on.
fn rung(combo: u32) -> usize {
    match combo {
        0 | 1 => 0,
        2 | 3 => 1,
        4..=6 => 2,
        _ => 3,
    }
}

/// A thump's weight for something this full, 0 small .. 3 giant; `None` below the threshold.
fn thump_weight(fill: f32, kind: Kind) -> Option<usize> {
    if kind == Kind::Boss {
        Some(3)
    } else if fill >= HEAVY_FILL {
        Some(2)
    } else if fill >= THUMP_FILL {
        Some(1)
    } else {
        None
    }
}

/// Decide this window's sounds and send them.
#[allow(clippy::too_many_arguments)]
fn flush(
    mut mix: ResMut<Mix>,
    voices: Res<Voices>,
    ball: Res<BallState>,
    time: Res<Time<Real>>,
    mut sink: Sink,
) {
    if mix.wait > 0.0 || (mix.pickups.is_empty() && mix.knocks.is_empty()) {
        return;
    }
    mix.wait = WINDOW;
    let now = time.elapsed_secs();

    let mut pickups = std::mem::take(&mut mix.pickups);
    let knocks = std::mem::take(&mut mix.knocks);
    // Biggest first: those are the ones heard.
    let weight = |p: &Pickup| match p.kind {
        Kind::Boss => 1.0e6,
        _ => p.size + if p.kind == Kind::Creature { 0.5 } else { 0.0 },
    };
    pickups.sort_by(|a, b| weight(b).total_cmp(&weight(a)));

    let mut thumped = false;
    if let Some(first) = pickups.first() {
        // The pop, on the streak's rung.
        let rung = rung(mix.combo);
        sink.two_d(
            LADDER[rung],
            &format!(
                "pop combo={} rung={rung} ({} pickups this window)",
                mix.combo,
                pickups.len()
            ),
        );
        mix.budget.force(now, EST_POP);
        mix.stats.pops += 1;
        mix.hold_roll(HOLD_PICKUP);

        // The things' own sounds, the biggest two.
        let mut voiced = 0;
        for p in &pickups {
            if voiced >= MAX_VOICES {
                break;
            }
            match p.kind {
                Kind::Creature | Kind::Boss => {
                    let row = p
                        .display
                        .zip(voices.catalog.as_ref())
                        .and_then(|(d, cat)| cat.for_display(d));
                    match row.and_then(|v| voice_kit(v, voices.kits.as_ref())) {
                        Some((kit, which)) => {
                            if !mix.budget.admit(now, Prio::Pickup, EST_VOICE) {
                                info!(
                                    "azeroll sfx: voice:{which} for {} dropped, budget full",
                                    p.name
                                );
                                continue;
                            }
                            sink.world(
                                kit,
                                &format!("voice:{which}"),
                                p.at,
                                &format!("{} {:?} fill={:.2}", p.name, p.kind, p.fill),
                            );
                            mix.stats.voices += 1;
                            voiced += 1;
                        }
                        None => info!(
                            "azeroll sfx: {} has no voice (display {:?})",
                            p.name, p.display
                        ),
                    }
                }
                Kind::Prop => {
                    // Crumbs ring only on the pop; the rest say what they are made of.
                    if p.fill < 0.12 && voiced > 0 {
                        continue;
                    }
                    if let Some(m) = material_of(&p.name) {
                        let kit = m.kit(p.fill >= 0.45);
                        if !mix.budget.admit(now, Prio::Pickup, EST_VOICE) {
                            info!("azeroll sfx: {} for {} dropped, budget full", kit.1, p.name);
                            continue;
                        }
                        sink.world(
                            kit.0,
                            kit.1,
                            p.at,
                            &format!("{} {m:?} fill={:.2}", p.name, p.fill),
                        );
                        mix.stats.voices += 1;
                        voiced += 1;
                    } else {
                        info!("azeroll sfx: {} has no material", p.name);
                    }
                }
            }
        }

        // The thump for the biggest thing, if it is big for the ball.
        if let Some(w) =
            thump_weight(first.fill, first.kind).filter(|_| mix.since_thump >= THUMP_GAP)
        {
            mix.since_thump = 0.0;
            let surface = match first.kind {
                Kind::Prop => {
                    material_of(&first.name).map_or(Surface::Grass, Material::thump_surface)
                }
                _ => Surface::Grass,
            };
            let kit = thump_kit(w, surface);
            if mix.budget.admit(now, Prio::Pickup, EST_THUMP) {
                sink.world(
                    kit.0,
                    kit.1,
                    first.at,
                    &format!("thump {} fill={:.2}", first.name, first.fill),
                );
                mix.stats.thumps += 1;
                thumped = true;
            } else {
                info!("azeroll sfx: thump for {} dropped, budget full", first.name);
            }
        }
    }

    // A wall bump or a ram: a dull thud scaled by the hardest of them, once per [`BUMP_GAP`].
    if let Some(hard) = knocks
        .iter()
        .max_by(|a, b| a.strength.total_cmp(&b.strength))
    {
        if !thumped && mix.since_bump >= BUMP_GAP {
            mix.since_bump = 0.0;
            let w = match hard.strength {
                s if s < 5.0 => 0,
                s if s < 10.0 => 1,
                s if s < 16.0 => 2,
                _ => 3,
            };
            let kit = thump_kit(w, Surface::Stone);
            if mix.budget.admit(now, Prio::Pickup, EST_THUMP) {
                sink.world(
                    kit.0,
                    kit.1,
                    ball.pos,
                    &format!("bump {} strength={:.1}", hard.name, hard.strength),
                );
                mix.stats.bumps += 1;
                mix.hold_roll(HOLD_PICKUP);
            }
        }
        if let Some(display) = hard.attacker {
            let row = voices.catalog.as_ref().and_then(|c| c.for_display(display));
            if let Some(kit) = row.map(|v| v.aggro).filter(|k| *k != 0) {
                if mix.budget.admit(now, Prio::Pickup, EST_VOICE) {
                    sink.world(
                        kit,
                        "voice:aggro",
                        ball.pos,
                        &format!("ram by {}", hard.name),
                    );
                }
            }
        }
    }
}

/// Queue this frame's touches. A touch is skipped when it is too gentle, when the same entity
/// sounded lately, or when the same hit is also a [`Bumped`] (the pickup flush sounds that one).
fn gather_touches(
    mut touched: MessageReader<Touched>,
    mut bumped: MessageReader<Bumped>,
    ball: Res<BallState>,
    time: Res<Time<Real>>,
    things: Query<&Collectible>,
    mut mix: ResMut<Mix>,
) {
    let now = time.elapsed_secs();
    let hard: Vec<Entity> = bumped
        .read()
        .filter(|b| b.strength >= BUMP_MIN)
        .filter_map(|b| b.entity)
        .collect();
    for t in touched.read() {
        let (floor, gap) = match t.state {
            TouchState::Dislodge => (KNOCK_MIN, KNOCK_GAP),
            TouchState::Wall => (LEAN_MIN, LEAN_GAP),
        };
        if t.strength < floor || (t.state == TouchState::Wall && hard.contains(&t.entity)) {
            continue;
        }
        if mix
            .touched_at
            .get(&t.entity)
            .is_some_and(|last| now - *last < gap)
        {
            continue;
        }
        let Ok(thing) = things.get(t.entity) else {
            continue;
        };
        if mix.touches.len() >= 32 {
            break;
        }
        mix.touched_at.insert(t.entity, now);
        mix.touches.push(Touch {
            name: thing.name.to_string(),
            kind: thing.kind,
            state: t.state,
            strength: t.strength,
            point: t.point,
            axis: thing.pickup,
            ratio: ball.diameter() / thing.pickup.max(1.0e-3),
        });
    }
    if mix.touched_at.len() > 256 {
        mix.touched_at
            .retain(|_, last| now - *last < KNOCK_GAP.max(LEAN_GAP));
    }
}

/// Queue this frame's growth beats, tiers and dash edges.
fn gather_cues(
    mut beats: MessageReader<GrowthBeat>,
    mut tiers: MessageReader<TierReached>,
    mut dashes: MessageReader<Dash>,
    mut mix: ResMut<Mix>,
) {
    for b in beats.read() {
        mix.cues.push(Cue::Growth {
            step: b.step,
            diameter: b.diameter,
        });
    }
    for t in tiers.read() {
        mix.cues.push(Cue::Tier {
            tier: t.tier,
            name: t.name,
            diameter: t.diameter,
        });
    }
    for d in dashes.read() {
        match d {
            Dash::WindUp => mix.cues.push(Cue::DashWindUp),
            Dash::Started => mix.cues.push(Cue::DashStart),
            Dash::Ready => mix.cues.push(Cue::DashReady),
            // The launch whoosh carries itself out; there is nothing to say at the end.
            Dash::Ended => {}
        }
    }
}

/// The material a touched thing is taken to be: a prop's by its name (wood when the name says
/// nothing, since that is what most of Goldshire is), anything alive soft leather.
fn touch_material(kind: Kind, name: &str) -> Material {
    match kind {
        Kind::Prop => material_of(name).unwrap_or(Material::Wood),
        _ => Material::Cloth,
    }
}

/// Decide this window's touches, and play the second halves of earlier wobbles that are due.
#[allow(clippy::too_many_arguments)]
fn touch_flush(
    time: Res<Time<Real>>,
    voices: Res<Voices>,
    listener: Res<AudioListener>,
    mut mix: ResMut<Mix>,
    mut sink: Sink,
) {
    let now = time.elapsed_secs();
    mix.touch_wait -= time.delta_secs();
    let kits = voices.kits.as_ref();

    let (due, waiting): (Vec<Later>, Vec<Later>) = std::mem::take(&mut mix.later)
        .into_iter()
        .partition(|l| l.at <= now);
    mix.later = waiting;
    for l in due {
        if mix.budget.admit(now, Prio::Touch, EST_TOUCH) {
            let pos = emission_point(kits, l.kit.0, listener.pos, l.point, l.amp);
            sink.world(l.kit.0, l.kit.1, pos, &l.why);
        }
    }

    if mix.touch_wait > 0.0 || mix.touches.is_empty() {
        return;
    }
    mix.touch_wait = TOUCH_WINDOW;
    let mut touches = std::mem::take(&mut mix.touches);
    touches.sort_by(|a, b| b.strength.total_cmp(&a.strength));
    if touches.len() > MAX_TOUCHES {
        let dropped = touches.len() - MAX_TOUCHES;
        info!("azeroll sfx: {dropped} touches over the window cap, quietest dropped");
        mix.stats.touches_dropped += dropped as u32;
        touches.truncate(MAX_TOUCHES);
    }
    for t in touches {
        let m = touch_material(t.kind, &t.name);
        let large = t.axis >= LARGE_AXIS;
        match t.state {
            TouchState::Dislodge => {
                let kit = m.knock(large);
                let amp = (0.22 + 0.045 * t.strength).clamp(0.22, 0.6);
                if !mix.budget.admit(now, Prio::Touch, EST_TOUCH) {
                    info!("azeroll sfx: knock for {} dropped, budget full", t.name);
                    mix.stats.touches_dropped += 1;
                    continue;
                }
                let pos = emission_point(kits, kit.0, listener.pos, t.point, amp);
                sink.world(
                    kit.0,
                    kit.1,
                    pos,
                    &format!(
                        "knock {} {m:?} strength={:.1} amp={amp:.2} axis={:.2}",
                        t.name, t.strength, t.axis
                    ),
                );
                mix.stats.knocks += 1;
            }
            TouchState::Wall => {
                let surface = match t.kind {
                    Kind::Prop => {
                        material_of(&t.name).map_or(Surface::Stone, Material::thump_surface)
                    }
                    _ => Surface::Grass,
                };
                let kit = thump_kit(0, surface);
                let amp = (0.2 + 0.05 * t.strength).clamp(0.2, 0.45);
                if !mix.budget.admit(now, Prio::Touch, EST_TOUCH) {
                    info!("azeroll sfx: lean for {} dropped, budget full", t.name);
                    mix.stats.touches_dropped += 1;
                    continue;
                }
                let pos = emission_point(kits, kit.0, listener.pos, t.point, amp);
                let almost = t.ratio >= ALMOST_FROM;
                sink.world(
                    kit.0,
                    kit.1,
                    pos,
                    &format!(
                        "lean {} strength={:.1} amp={amp:.2} ratio={:.2}{}",
                        t.name,
                        t.strength,
                        t.ratio,
                        if almost { " almost" } else { "" }
                    ),
                );
                mix.stats.leans += 1;
                if almost {
                    // Nearly loose: it rocks, two quick knocks of its own material, each softer.
                    let knock = m.knock(large);
                    for i in 1..=2 {
                        mix.later.push(Later {
                            at: now + WOBBLE_DELAY * i as f32,
                            kit: knock,
                            point: t.point,
                            amp: (amp * 1.2).max(0.22) * WOBBLE_KEEP.powi(i - 1),
                            why: format!("wobble {i} {} {m:?} ratio={:.2}", t.name, t.ratio),
                        });
                    }
                    mix.stats.wobbles += 1;
                }
            }
        }
        mix.hold_roll(HOLD_TOUCH);
    }
}

/// Play the cues queued this frame. They are held back once Hogger is rolled up, so nothing sounds
/// over the finale's fanfare, and when the mode is not playing.
fn play_cues(
    time: Res<Time<Real>>,
    run: Res<Run>,
    flow: Res<Flow>,
    phase: Res<State<Phase>>,
    mut mix: ResMut<Mix>,
    mut sink: Sink,
) {
    if mix.cues.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let cues = std::mem::take(&mut mix.cues);
    let held = run.won || flow.finale.is_some() || *phase.get() != Phase::Playing;
    // A tier arrives with its growth beat when a big jump crosses both: the flourish covers it.
    let tier_now = cues.iter().any(|c| matches!(c, Cue::Tier { .. }));
    for cue in cues {
        let (kit, secs, hold, why) = match cue {
            Cue::Growth { step, diameter } => (
                GROWTH_CHIME,
                EST_GROWTH,
                HOLD_CHIME,
                format!("growth step={step} diameter={diameter:.2}"),
            ),
            Cue::Tier {
                tier,
                name,
                diameter,
            } => (
                TIER_FLOURISH,
                EST_TIER,
                HOLD_FLOURISH,
                format!("tier {tier} {name:?} diameter={diameter:.2}"),
            ),
            Cue::DashWindUp => (DASH_WINDUP, EST_DASH, HOLD_DASH, "dash wind-up".to_string()),
            Cue::DashStart => (DASH_LAUNCH, EST_DASH, HOLD_DASH, "dash launch".to_string()),
            Cue::DashReady => (DASH_READY, 0.2, 0.0, "dash ready".to_string()),
        };
        let skip = if held {
            Some("finale or not playing")
        } else if tier_now && kit.0 == GROWTH_CHIME.0 {
            Some("covered by the tier flourish")
        } else if kit.0 == DASH_READY.0 && mix.since_pickup < 0.4 {
            Some("a pickup is sounding")
        } else {
            None
        };
        if let Some(reason) = skip {
            info!("azeroll sfx: cue {} ({why}) held back: {reason}", kit.1);
            mix.stats.cues_held += 1;
            continue;
        }
        sink.two_d(kit, &why);
        mix.budget.force(now, secs);
        mix.hold_roll(hold);
        mix.stats.cues += 1;
    }
}

/// The ground under the ball for the roll: any building's floor is wood, outdoors the terrain
/// layer under it through the footstep tables (the character class stands in for the ball).
fn ground_under(cat: &FootstepCatalog, world: &WorldPoint, feet: Vec3) -> Option<Ground> {
    if world.indoors_at(feet) {
        return Some(Ground::Wood);
    }
    let terrain = world.terrain_type(cat, Subject::Player, feet)?;
    let (dry, _) = cat.resolve_terrain(CHARACTER_CLASS, terrain)?;
    Some(Ground::of_character_kit(dry))
}

/// The roll: while the ball is on the ground and moving, one quiet footstep-family tick per turn
/// of the ball. Its size band and the ground pick the kit (small, medium-large, huge, giant by
/// diameter, so a bigger ball sounds heavier); its speed sets the level and, with the radius, the
/// rate. The level is a final amplitude kept low (see [`ROLL_AMP_MAX`]) and the roll steps aside
/// for [`HOLD_PICKUP`] and the like after anything louder, so it stays a texture under the pops,
/// the voices and the music. It plays last in the chain and only when [`Budget`] has room at the
/// lowest priority.
#[allow(clippy::too_many_arguments)]
fn roll(
    time: Res<Time<Real>>,
    ball: Res<BallState>,
    run: Res<Run>,
    flow: Res<Flow>,
    voices: Res<Voices>,
    listener: Res<AudioListener>,
    footsteps: Option<Res<Footsteps>>,
    world: Option<WorldPoint>,
    mut mix: ResMut<Mix>,
    mut sink: Sink,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    mix.roll_wait -= dt;
    mix.roll_hold -= dt;
    mix.ground_wait -= dt;
    if run.won || flow.finale.is_some() {
        return;
    }
    let speed = Vec2::new(ball.vel.x, ball.vel.z).length();
    let frac = speed / super::ball::top_speed(ball.radius).max(1.0e-3);
    if !ball.grounded || frac < ROLL_MIN_FRAC {
        return;
    }

    if mix.ground_wait <= 0.0 {
        mix.ground_wait = GROUND_EVERY;
        let feet = ball.pos - Vec3::Y * ball.radius;
        if let Some(g) = footsteps
            .as_deref()
            .zip(world.as_ref())
            .and_then(|(fs, w)| ground_under(&fs.0, w, feet))
        {
            if g != mix.ground {
                info!("azeroll sfx: roll ground {:?} -> {g:?}", mix.ground);
                mix.ground = g;
            }
        }
    }
    if mix.roll_wait > 0.0 || mix.roll_hold > 0.0 {
        return;
    }

    let band = size_band(ball.diameter());
    let kit = roll_kit(band, mix.ground);
    let amp = roll_amp(frac, band);
    let interval = roll_interval(ball.radius, speed);
    if !mix.budget.admit(now, Prio::Roll, EST_ROLL) {
        mix.roll_wait = ROLL_GAP_MIN;
        mix.stats.rolls_dropped += 1;
        if mix.stats.rolls_dropped % 8 == 1 {
            info!(
                "azeroll sfx: roll tick dropped, budget full ({} dropped so far)",
                mix.stats.rolls_dropped
            );
        }
        return;
    }
    mix.roll_wait = interval;
    let pos = emission_point(voices.kits.as_ref(), kit.0, listener.pos, ball.pos, amp);
    sink.world(
        kit.0,
        kit.1,
        pos,
        &format!(
            "roll band={band} {:?} speed={speed:.1} ({:.0}% of top) amp={amp:.2} next={interval:.2}s",
            mix.ground,
            frac * 100.0
        ),
    );
    mix.stats.rolls += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::azeroll::level::{DISLODGE_RATIO, TIERS};

    fn id(k: Kit) -> u32 {
        k.0
    }

    #[test]
    fn roll_bands_are_the_hud_tiers_where_the_world_changes_scale() {
        // Critters, NPCs and horses, Hogger: rows 1, 3 and 5 of the HUD's tier table.
        assert_eq!(ROLL_BANDS, [TIERS[1].0, TIERS[3].0, TIERS[5].0]);
        assert_eq!(size_band(0.5), 0);
        assert_eq!(size_band(1.04), 1);
        assert_eq!(size_band(4.0), 1);
        assert_eq!(size_band(4.33), 2);
        assert_eq!(size_band(8.59), 2);
        assert_eq!(size_band(8.6), 3);
        assert_eq!(size_band(40.0), 3);
    }

    #[test]
    fn every_size_and_ground_has_its_own_roll_kit() {
        let mut ids: Vec<u32> = (0..4)
            .flat_map(|b| GROUNDS.map(|g| id(roll_kit(b, g))))
            .collect();
        assert_eq!(ids.len(), 20);
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 20, "a roll kit is used twice");
    }

    #[test]
    fn the_character_kits_map_back_to_their_ground() {
        for g in GROUNDS {
            // The medium-large kit of a ground names that ground again.
            assert_eq!(Ground::of_character_kit(id(roll_kit(1, g))), g);
        }
        assert_eq!(Ground::of_character_kit(2095), Ground::Dirt, "mud is dirt");
        assert_eq!(Ground::of_character_kit(0), Ground::Dirt);
    }

    #[test]
    fn a_bigger_ball_rolls_slower_and_a_faster_one_quicker() {
        use crate::azeroll::ball::top_speed;
        let gap = |r: f32| roll_interval(r, top_speed(r));
        assert!(gap(0.25) < gap(2.0));
        assert!(gap(2.0) <= gap(4.5));
        for r in [0.25, 1.0, 2.0, 4.5, 12.0] {
            for v in [0.1, 1.0, 5.0, 30.0] {
                let g = roll_interval(r, v);
                assert!((ROLL_GAP_MIN..=ROLL_GAP_MAX).contains(&g), "{r} {v} {g}");
            }
        }
        assert!(roll_interval(1.0, 2.0) > roll_interval(1.0, 8.0));
    }

    #[test]
    fn the_roll_stays_quiet_and_louder_with_speed_and_size() {
        assert!(roll_amp(0.2, 0) >= ROLL_AMP_MIN);
        assert!(roll_amp(1.0, 0) <= ROLL_AMP_MAX);
        assert!(roll_amp(0.2, 0) < roll_amp(1.0, 0));
        assert!(roll_amp(0.6, 0) < roll_amp(0.6, 3));
        // Even the loudest tick is well under the quietest pickup pop (PickupGems, 0.5).
        assert!(roll_amp(5.0, 3) < 0.5 * 0.8);
        assert_eq!(roll_amp(5.0, 2), roll_amp(1.0, 2), "a dash does not shout");
    }

    /// The kit player's gain at distance `d` for a kit of `min` (`sound::math::fmod_rolloff`).
    fn rolloff(min: f32, d: f32) -> f32 {
        if d <= min {
            1.0
        } else {
            min / (min + 4.0 * (d - min))
        }
    }

    #[test]
    fn the_emission_distance_gives_the_wanted_gain() {
        for min in [4.0, 5.0, 8.0, 9.0] {
            for want in [0.1, 0.25, 0.5, 0.9] {
                let d = emission_distance(min, want);
                assert!((rolloff(min, d) - want).abs() < 1e-4, "{min} {want} {d}");
                // Full inside the near-field ramp, never beyond the cutoff's 90%.
                assert!(d <= 0.9 * 45.0);
            }
            assert!(rolloff(min, emission_distance(min, 1.0)) == 1.0);
            // A request below the floor is the floor, not silence.
            assert_eq!(
                emission_distance(min, 0.0),
                emission_distance(min, MIN_ATTENUATION)
            );
        }
    }

    #[test]
    fn a_quiet_source_keeps_its_direction_from_the_listener() {
        let at = emission_point(None, 1, Vec3::ZERO, Vec3::new(3.0, 0.0, 4.0), 0.2);
        assert_eq!(
            at,
            Vec3::new(3.0, 0.0, 4.0),
            "an unknown kit plays where it is"
        );
        let listener = Vec3::new(1.0, 2.0, 3.0);
        let source = Vec3::new(1.0, 2.0, -7.0);
        let kits = SoundKitCatalog::empty_for_tests();
        let at = emission_point(Some(&kits), 1, listener, source, 0.2);
        assert_eq!(at, source, "a kit missing from the table plays where it is");
    }

    #[test]
    fn the_budget_gives_way_by_priority() {
        let mut b = Budget::default();
        for _ in 0..CEIL_ROLL {
            assert!(b.admit(0.0, Prio::Roll, 1.0));
        }
        assert!(!b.admit(0.0, Prio::Roll, 1.0), "roll is first to go");
        for _ in CEIL_ROLL..CEIL_TOUCH {
            assert!(b.admit(0.0, Prio::Touch, 1.0));
        }
        assert!(!b.admit(0.0, Prio::Touch, 1.0), "touches go before pickups");
        for _ in CEIL_TOUCH..CEIL_PICKUP {
            assert!(b.admit(0.0, Prio::Pickup, 1.0));
        }
        assert!(!b.admit(0.0, Prio::Pickup, 1.0));
        b.force(0.0, 1.0);
        assert_eq!(b.live(0.5), CEIL_PICKUP + 1, "a pop is made regardless");
        assert_eq!(b.live(1.5), 0, "voices end");
        assert!(b.admit(1.5, Prio::Roll, 1.0));
        assert!(CEIL_ROLL < CEIL_TOUCH && CEIL_TOUCH < CEIL_PICKUP);
        // The kit player has 12 software channels; leave the engine's own sounds some.
        assert!(CEIL_PICKUP < 12);
    }

    #[test]
    fn almost_is_a_band_below_the_knock_loose_line() {
        assert!(ALMOST_FROM < DISLODGE_RATIO);
        // A wall touch is always under the knock-loose ratio, so every ratio from here is "almost".
        assert!(ALMOST_FROM > 1.0);
        assert!(WOBBLE_KEEP < 1.0 && WOBBLE_DELAY < 0.25);
    }

    #[test]
    fn the_touch_material_is_soft_for_the_living_and_wood_by_default() {
        assert_eq!(touch_material(Kind::Prop, "Wine Barrel"), Material::Wood);
        assert_eq!(touch_material(Kind::Prop, "Apple Crate"), Material::Wood);
        assert_eq!(touch_material(Kind::Prop, "Ale Keg"), Material::Wood);
        assert_eq!(touch_material(Kind::Prop, "Bucket of Ale"), Material::Metal);
        assert_eq!(touch_material(Kind::Prop, "Bronze Kettle"), Material::Metal);
        assert_eq!(touch_material(Kind::Prop, "Zzyzx"), Material::Wood);
        assert_eq!(
            touch_material(Kind::Creature, "Young Wolf"),
            Material::Cloth
        );
        assert_eq!(touch_material(Kind::Boss, "Hogger"), Material::Cloth);
    }

    #[test]
    fn every_material_knocks_with_its_own_kit() {
        for m in MATERIALS {
            for large in [false, true] {
                assert!(m.knock(large).0 >= 1200 && m.knock(large).0 <= 1217);
            }
        }
    }

    #[test]
    fn the_cues_keep_clear_of_the_pickups_and_the_other_systems_kits() {
        // QUESTCOMPLETED (the finale), LEVELUP (the record), igQuestListOpen (the tier banner), the
        // level-up UI kit and the quest-added pair.
        let reserved = [619, 888, 875, 124, 618, 890];
        let mut taken: Vec<u32> = LADDER.iter().map(|k| k.0).collect();
        for (_, group) in all_kits() {
            taken.extend(group.iter().map(|k| k.0));
        }
        let cues = [
            GROWTH_CHIME,
            TIER_FLOURISH,
            DASH_LAUNCH,
            DASH_WINDUP,
            DASH_READY,
        ];
        for c in cues {
            assert!(
                !reserved.contains(&c.0),
                "{} collides with a reserved kit",
                c.1
            );
            assert!(
                !LADDER.iter().any(|k| k.0 == c.0),
                "{} is on the ladder",
                c.1
            );
            assert_eq!(
                all_kits()
                    .iter()
                    .flat_map(|(_, g)| g)
                    .filter(|k| k.0 == c.0)
                    .count(),
                1,
                "{} is in another group too",
                c.1
            );
        }
        let mut ids: Vec<u32> = cues.iter().map(|k| k.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), cues.len());
    }

    /// With a WoW install, every kit this module can play exists and has a file; without one the
    /// check is skipped (CI has no game data).
    #[test]
    fn every_kit_resolves_against_the_game_data() {
        let Some(data) = benilla_formats::wow_data() else {
            eprintln!("no WoW install: kit resolution not checked");
            return;
        };
        let Ok(mut chain) = benilla_formats::open_chain(&data) else {
            eprintln!("WoW install does not open: kit resolution not checked");
            return;
        };
        let kits = benilla_formats::load_sound_kit_catalog(&mut chain).expect("SoundEntries");
        let missing = missing_kits(&kits);
        assert!(missing.is_empty(), "kits without a file: {missing:?}");
        // The roll's gain trick needs each roll kit's rolloff; a kit without one is a 0 distance.
        for band in 0..4 {
            for g in GROUNDS {
                let k = kits.get(roll_kit(band, g).0).unwrap();
                assert!(k.min_distance > 0.0, "{} has no min distance", k.name);
                assert!(
                    k.distance_cutoff > 0.9 * 40.0,
                    "{} cuts off too near",
                    k.name
                );
            }
        }
    }
}
