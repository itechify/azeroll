//! The Elwynn Forest level: where it is, how big things are, what wins it. Shared tuning constants
//! live here so the ball, collection and spawning agree; module-local feel constants (acceleration,
//! camera damping) stay in their own modules.

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

use super::collect::{contact, Contact};
use super::progress::{self, Flow, Progress};
use super::{playing, BallState, Collected, Collectible, Kind, Phase, Run};
use crate::loading_screen::LoadingScreen;
use crate::sound::GlueSound;

/// Eastern Kingdoms.
pub(crate) const MAP: u32 = 0;

/// The ball's start, WoW world coordinates (x north, y west, z up): the road outside the Lion's
/// Pride Inn in Goldshire. `z` is a hint; the ball drops onto the terrain once its tile loads.
pub(crate) const START: [f32; 3] = [-9450.0, 60.0, 57.0];

/// The ball's radius at the start, yards. A mug is ~0.15, a barrel ~0.6, a wolf ~1, a tree ~6.
pub(crate) const START_RADIUS: f32 = 0.25;

/// Maximum full bbox axis allowed per yard of ball radius (D >= 2.15 * axis).
pub(crate) const COLLECT_RATIO: f32 = 2.0 / 2.15;
pub(crate) const DISLODGE_RATIO: f32 = 1.5;
/// Occupied fraction of the oriented box; credited in sphere-normalized yards cubed.
pub(crate) const FILL_FACTOR: f32 = 0.5;

/// Map doodads within this many yards of [`START`] are claimed from the streamer and become
/// collectible copies; beyond it the world keeps its optimized scenery.
pub(crate) const CLAIM_RADIUS: f32 = 650.0;

/// The ball may not wander further than this from [`START`], yards (a soft leash).
pub(crate) const LEASH: f32 = 680.0;

/// The level's growth tiers, by ball diameter in yards: the HUD announces each on crossing it. A
/// tier opens when every representative it names ([`TIER_REPS`]) is edible, i.e.
/// `collect::contact(radius, axis) == RollUp`, so the diameter is `2.15 x` the largest
/// representative's maximum full axis (the roster's fits: chicken 0.35, rabbit 0.48, sheep 0.8,
/// cow 1.2, farm horse 1.8, bear 2.3, gnoll 2.8, Hogger 4; adult NPCs measure 1.93-2.01).
///
/// These are the values for the shipped recipe. The running game derives the same table from the
/// resolved roster and the spawned NPCs ([`TierTable`]) and warns where this one has drifted
/// from it; a test pins the two together for `goldshire.toml`.
pub(crate) const TIERS: &[(f32, &str)] = &[
    (0.0, "Tiny things"),
    (1.04, "Critters"),
    (2.58, "Creatures"),
    (4.33, "NPCs and horses"),
    (6.02, "Forest giants"),
    (8.6, "Hogger"),
];

/// What a [`TIERS`] row names, for deriving its diameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rep {
    /// A roster species, by name; its fitted maximum axis.
    Species(&'static str),
    /// The grown-up NPCs of the authored cast (children are below [`ADULT_MIN_AXIS`]).
    Adults,
}

/// Parallel to [`TIERS`]: the things each tier is named for. The first tier is the start and
/// names nothing.
pub(crate) const TIER_REPS: &[&[Rep]] = &[
    &[],
    &[Rep::Species("Chicken"), Rep::Species("Rabbit")],
    &[Rep::Species("Sheep"), Rep::Species("Cow")],
    &[Rep::Adults, Rep::Species("Farm horse")],
    &[Rep::Species("Forest Bear"), Rep::Species("Riverpaw Gnoll")],
    &[Rep::Species("Hogger")],
];

/// Parallel to [`TIERS`]: the areas a tier's camera beat glances toward, from what the ball has
/// just outgrown to what it can now reach ("the market from the inn yard, the farms from the
/// road"). [`reveal_target`] takes the nearest one the ball is not already standing in. Names are
/// `goldshire.toml` area names; a test pins them to it.
pub(crate) const TIER_REVEALS: &[&[&str]] = &[
    &[],
    &["Market", "Northern goods", "Smithy"],
    &["Smithy", "Broken cart", "Roadside camp", "Southwest road"],
    &[
        "Roadside camp",
        "Southwest road",
        "Maclure harvest",
        "Stonefield harvest",
    ],
    &["Riverpaw camp", "Stonefield harvest", "Maclure harvest"],
    &["Riverpaw camp"],
];

/// A glance toward an area nearer than this (beyond its own half radius) would look at the
/// ball's feet, so it is not one.
const REVEAL_MIN_DIST: f32 = 12.0;

/// Where tier `tier`'s camera beat looks, in Bevy yards, and what it is called: the nearest of
/// [`TIER_REVEALS`] that is open ground and not already underfoot.
pub(super) fn reveal_target(
    design: &super::design::Design,
    tier: usize,
    from: Vec3,
) -> Option<(&str, Vec3)> {
    let flat = |c: Vec3| Vec3::new(c.x - from.x, 0.0, c.z - from.z).length();
    TIER_REVEALS
        .get(tier)?
        .iter()
        .filter_map(|name| design.areas.iter().find(|a| a.name == *name))
        .filter(|a| !a.enclosed)
        .map(|a| (a, super::units::wow_to_bevy(a.centre)))
        .filter(|(a, at)| flat(*at) > (a.radius * 0.5).max(REVEAL_MIN_DIST))
        .min_by(|a, b| flat(a.1).total_cmp(&flat(b.1)))
        .map(|(a, at)| (a.name.as_str(), at))
}

/// An NPC with at least this maximum axis counts as an adult (kids measure 1.16-1.29, adults
/// 1.93-2.01).
pub(crate) const ADULT_MIN_AXIS: f32 = 1.5;

/// A tier, once reached, is kept until the ball falls this fraction below its diameter, so a ball
/// shedding trophies at the threshold does not flicker the readout.
const TIER_HYSTERESIS: f32 = 0.04;

/// The smallest ball diameter at which an object of this maximum axis is rolled up, by the same
/// classifier that does the rolling.
pub(crate) fn edible_diameter(axis: f32) -> f32 {
    let mut radius = axis / COLLECT_RATIO;
    // The division can land a rounding step short of the `<=` in `contact`.
    for _ in 0..8 {
        if contact(radius, axis) == Contact::RollUp {
            break;
        }
        radius = radius.next_up();
    }
    2.0 * radius
}

/// The tier table for these representative axes (`None`: not known yet, keep the default row).
/// Diameters never decrease down the table.
pub(crate) fn derive_tiers(axis_of: impl Fn(Rep) -> Option<f32>) -> Vec<TierRow> {
    let mut rows: Vec<TierRow> = Vec::with_capacity(TIERS.len());
    for (&(default, name), reps) in TIERS.iter().zip(TIER_REPS) {
        let known: Vec<(Rep, f32)> = reps
            .iter()
            .filter_map(|&r| axis_of(r).map(|a| (r, a)))
            .collect();
        let at = if known.len() == reps.len() && !known.is_empty() {
            edible_diameter(known.iter().map(|k| k.1).fold(0.0, f32::max))
        } else {
            default
        };
        let at = rows.last().map_or(at, |prev| at.max(prev.at));
        rows.push(TierRow {
            at,
            name,
            reps: known,
        });
    }
    rows
}

/// One derived tier.
#[derive(Debug, Clone)]
pub(crate) struct TierRow {
    pub at: f32,
    pub name: &'static str,
    /// The representatives whose axes are known.
    pub reps: Vec<(Rep, f32)>,
}

/// The tier table the running level announces from: [`TIERS`] where nothing better is known, the
/// roster's and the cast's real sizes where it is.
#[derive(Resource)]
pub(crate) struct TierTable {
    pub rows: Vec<TierRow>,
}

impl Default for TierTable {
    fn default() -> Self {
        Self {
            rows: derive_tiers(|_| None),
        }
    }
}

/// Hogger's full stand-box maximum axis, yards: the ball must be able to
/// roll him up, so its radius has to reach `HOGGER_SIZE / COLLECT_RATIO`. "Hogger, enlarged by the
/// device's backwash."
pub(crate) const HOGGER_SIZE: f32 = 4.0;

// ---------------------------------------------------------------------------------------------
// The creature roster
// ---------------------------------------------------------------------------------------------

/// Which model a [`Species`] wears, resolved at runtime through the creature DBC catalog (see
/// `creatures::resolve_roster`).
#[derive(Debug, Clone)]
pub(crate) enum Look {
    /// A `CreatureDisplayInfo` row.
    DisplayId(u32),
    /// An M2 path (`Creature\Rabbit\Rabbit.m2`; the case, slashes and the `.mdx`/`.m2` extension
    /// do not matter): the first display that wears this model and has a skin.
    ModelPath(String),
}

/// What a creature does when the ball comes near.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Behavior {
    /// Wanders and ignores the ball.
    Passive,
    /// Wanders, and runs from the ball once the ball can eat it.
    Skittish,
    /// Chases the ball while the ball cannot eat it, and runs once it can.
    Hostile,
}

/// Where a [`Species`] spawns.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Place {
    /// At random within `radius` yards of a WoW position.
    Around { at: [f32; 3], radius: f32 },
}

/// One kind of creature in Elwynn.
#[derive(Debug, Clone)]
pub(crate) struct Species {
    pub name: String,
    pub look: Look,
    pub count: u32,
    /// A multiplier on the display's own DBC scale (1.0 = as the game draws it).
    pub scale: f32,
    /// When set, the scale is chosen so the collection size is exactly this many yards, whatever
    /// the model: how Hogger is enlarged.
    pub fit: Option<f32>,
    pub place: Place,
    pub behavior: Behavior,
    pub kind: Kind,
    /// Yards per second when wandering (the walk animation plays below 5).
    pub walk: f32,
    /// Yards per second when fleeing or chasing (the run animation plays above 5).
    pub run: f32,
    /// Chance that a spawn is a rare elite: [`ELITE_SCALE`] times bigger.
    pub elite: f32,
}

/// How much bigger an elite is than its kin.
pub(crate) const ELITE_SCALE: f32 = 1.6;

// ---------------------------------------------------------------------------------------------
// The level flow
// ---------------------------------------------------------------------------------------------

/// How far the ball has come along [`TierTable`] this run.
#[derive(Resource)]
struct Tiers {
    /// Rows announced so far. The first is the ball's own start and is never announced.
    announced: usize,
    /// Rows the ball currently holds, with hysteresis on the way down.
    current: usize,
}

impl Default for Tiers {
    fn default() -> Self {
        let n = tiers_reached(&TierTable::default().rows, 2.0 * START_RADIUS);
        Self {
            announced: n,
            current: n,
        }
    }
}

/// How many rows a ball of this diameter has reached.
fn tiers_reached(rows: &[TierRow], diameter: f32) -> usize {
    rows.iter().take_while(|r| diameter >= r.at).count()
}

/// [`tiers_reached`] with hysteresis: a held row is only lost once the diameter is
/// [`TIER_HYSTERESIS`] below it.
fn tiers_held(rows: &[TierRow], held: usize, diameter: f32) -> usize {
    let up = tiers_reached(rows, diameter);
    if up >= held {
        return up;
    }
    let mut now = held;
    while now > up && now > 1 && diameter < rows[now - 1].at * (1.0 - TIER_HYSTERESIS) {
        now -= 1;
    }
    now.max(up)
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<GrowthSteps>()
        .init_resource::<Tiers>()
        .init_resource::<TierTable>()
        .add_systems(OnEnter(Phase::Ready), reset_tiers)
        .add_systems(Update, (keys, begin, tick_banner, refresh_tiers))
        .add_systems(
            Update,
            (tick, announce, growth_beats)
                .chain()
                .after(WorldStage::Input)
                .run_if(playing),
        );
}

fn reset_tiers(mut tiers: ResMut<Tiers>, table: Res<TierTable>, mut growth: ResMut<GrowthSteps>) {
    *growth = GrowthSteps::default();
    let n = tiers_reached(&table.rows, 2.0 * START_RADIUS);
    *tiers = Tiers {
        announced: n,
        current: n,
    };
}

/// Private high-water cursor; reset per run, independent of size lost by shedding.
#[derive(Resource, Default)]
struct GrowthSteps(u32);

/// The 25 % step a diameter has reached, if it is a new high for the run: shedding and regrowing
/// across a step never reports it twice.
fn new_growth_step(steps: &mut GrowthSteps, base: f32, diameter: f32) -> Option<u32> {
    let step = (diameter / base).log(1.25).floor().max(0.0) as u32;
    (step > steps.0).then(|| {
        steps.0 = step;
        step
    })
}

fn growth_beats(
    ball: Res<BallState>,
    design: Res<super::design::Design>,
    mut steps: ResMut<GrowthSteps>,
    mut beats: MessageWriter<super::GrowthBeat>,
) {
    let base = 2.0 * design.start_radius;
    if let Some(step) = new_growth_step(&mut steps, base, ball.diameter()) {
        beats.write(super::GrowthBeat {
            step,
            threshold: base * 1.25_f32.powf(step as f32),
            diameter: ball.diameter(),
        });
    }
}

/// Re-derive [`TierTable`] from the roster's fits and the spawned cast whenever either changes,
/// and say where it disagrees with the [`TIERS`] defaults.
fn refresh_tiers(
    design: Res<super::design::Design>,
    cast: Query<&Collectible, With<super::npcs::Npc>>,
    added: Query<(), Added<super::npcs::Npc>>,
    mut table: ResMut<TierTable>,
) {
    if !design.is_changed() && added.is_empty() {
        return;
    }
    let adults = cast
        .iter()
        .map(|c| c.pickup)
        .filter(|a| *a >= ADULT_MIN_AXIS)
        .fold(None, |m: Option<f32>, a| Some(m.map_or(a, |m| m.max(a))));
    let rows = derive_tiers(|rep| match rep {
        Rep::Species(name) => design
            .roster
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.fit),
        Rep::Adults => adults,
    });
    for (row, &(default, _)) in rows.iter().zip(TIERS) {
        if (row.at - default).abs() > 0.05 {
            warn!(
                "azeroll: tier {:?} derives to {:.2} yd, the TIERS default says {default:.2}",
                row.name, row.at
            );
        }
    }
    table.rows = rows;
}

/// Esc pauses and resumes; R restarts from play or pause; Enter or R leaves the results; F starts
/// the untimed Free Roll from the results after a win, or from the title once Hogger has been
/// rolled up before.
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    run: Res<Run>,
    progress: Res<Progress>,
    mut flow: ResMut<Flow>,
    mut next: ResMut<NextState<Phase>>,
) {
    let restart = keys.just_pressed(KeyCode::KeyR);
    let escape = keys.just_pressed(KeyCode::Escape);
    let free = keys.just_pressed(KeyCode::KeyF);
    match phase.get() {
        Phase::Ready => {
            if free && !loading.covering() && progress.best().wins > 0 {
                progress::start_free_roll(&mut flow, &mut next);
            }
        }
        Phase::Playing => {
            if restart {
                next.set(Phase::Ready);
            } else if escape {
                next.set(Phase::Paused);
            }
        }
        Phase::Paused => {
            if restart {
                next.set(Phase::Ready);
            } else if escape {
                next.set(Phase::Playing);
            }
        }
        Phase::Results => {
            if restart || keys.just_pressed(KeyCode::Enter) {
                next.set(Phase::Ready);
            } else if free && run.won {
                progress::start_free_roll(&mut flow, &mut next);
            }
        }
    }
}

/// Ready becomes Playing on the first movement key or Enter, once the loading screen is down.
fn begin(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::Ready || loading.covering() {
        return;
    }
    if keys.any_just_pressed([
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::Enter,
    ]) {
        next.set(Phase::Playing);
    }
}

/// The clock, and the two ways a run ends: Hogger rolled up (after the celebration) or the time
/// limit. The clock stops at the win, so the celebration costs nothing. Free Roll has no clock and
/// no end: rolling up Hogger in it only earns the banner.
fn tick(
    time: Res<Time>,
    real: Res<Time<Real>>,
    design: Res<super::design::Design>,
    mut run: ResMut<Run>,
    mut flow: ResMut<Flow>,
    mut collected: MessageReader<Collected>,
    mut next: ResMut<NextState<Phase>>,
    mut sounds: MessageWriter<GlueSound>,
) {
    if !run.won && !flow.free_roll {
        run.elapsed += time.delta_secs();
    }
    for c in collected.read() {
        if c.kind == Kind::Boss && !run.won {
            run.won = true;
            if flow.free_roll {
                run.banner = Some(("HOGGER, ROLLED UP!".to_string(), 4.0));
            } else {
                run.banner = None;
                flow.finale = Some(0.0);
                sounds.write(GlueSound(progress::FANFARE_SOUND));
            }
        }
    }
    if let Some(t) = flow.finale.as_mut() {
        *t += real.delta_secs();
        if *t >= progress::FINALE_SECS {
            flow.finale = None;
            next.set(Phase::Results);
        }
    } else if !flow.free_roll && !run.won && run.elapsed >= design.time_limit {
        run.elapsed = design.time_limit;
        next.set(Phase::Results);
    }
}

/// Move the held row for this diameter and return the rows never announced before: the announced
/// mark only rises, so a tier that was lost to a shed and regained is not announced twice.
fn advance_tiers(tiers: &mut Tiers, rows: &[TierRow], diameter: f32) -> std::ops::Range<usize> {
    tiers.current = tiers_held(rows, tiers.current, diameter);
    let fresh = tiers.announced..tiers.current.max(tiers.announced);
    tiers.announced = tiers.announced.max(tiers.current);
    fresh
}

/// Announce each tier the first time the ball's diameter reaches it, and log the representatives
/// that made it true next to the real contact state.
fn announce(
    ball: Res<BallState>,
    table: Res<TierTable>,
    mut tiers: ResMut<Tiers>,
    mut run: ResMut<Run>,
    mut reached: MessageWriter<super::TierReached>,
) {
    let fresh = advance_tiers(&mut tiers, &table.rows, ball.diameter());
    if !fresh.is_empty() {
        for tier in fresh {
            let row = &table.rows[tier];
            reached.write(super::TierReached {
                tier,
                name: row.name,
                threshold: row.at,
                diameter: ball.diameter(),
            });
        }
        let row = &table.rows[tiers.current - 1];
        // Two decimals, rounded up: the banner must not name a size the ball is still short of.
        let shown = (row.at * 100.0).ceil() / 100.0;
        run.banner = Some((format!("{}  ·  {shown:.2} yd", row.name), 3.5));
        let reps: Vec<String> = row
            .reps
            .iter()
            .map(|(rep, axis)| {
                format!(
                    "{rep:?} axis={axis:.2} contact={:?}",
                    contact(ball.radius, *axis)
                )
            })
            .collect();
        info!(
            "azeroll tier: {:?} at D={:.2} (opens {:.2}) r={:.3}; {}",
            row.name,
            ball.diameter(),
            row.at,
            ball.radius,
            reps.join(", ")
        );
    }
}

/// The banner runs down in virtual time, so pause holds it.
fn tick_banner(time: Res<Time>, mut run: ResMut<Run>) {
    let dt = time.delta_secs();
    let expired = match run.banner.as_mut() {
        Some((_, left)) => {
            *left -= dt;
            *left <= 0.0
        }
        None => false,
    };
    if expired {
        run.banner = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Recipe {
        herds: Vec<Herd>,
        areas: Vec<AreaName>,
    }
    #[derive(Deserialize)]
    struct AreaName {
        name: String,
    }
    #[derive(Deserialize)]
    struct Herd {
        species: String,
        fit: f32,
    }

    /// The adult NPC axis the shipped cast measures at (`azeroll NPC: ... axis=` in the log).
    const ADULT_AXIS: f32 = 2.01;

    fn shipped() -> Vec<TierRow> {
        let recipe: Recipe = toml::from_str(include_str!("goldshire.toml")).unwrap();
        derive_tiers(|rep| match rep {
            Rep::Species(name) => recipe
                .herds
                .iter()
                .find(|h| h.species == name)
                .map(|h| h.fit),
            Rep::Adults => Some(ADULT_AXIS),
        })
    }

    #[test]
    fn edible_diameter_is_the_first_roll_up() {
        for axis in [0.35, 0.48, 0.8, 1.2, 2.01, 2.8, 4.0] {
            let d = edible_diameter(axis);
            assert_eq!(contact(d / 2.0, axis), Contact::RollUp, "axis {axis}");
            assert_ne!(
                contact(d / 2.0 * 0.999, axis),
                Contact::RollUp,
                "axis {axis}"
            );
            assert!((d - 2.15 * axis).abs() < 1e-3);
        }
    }

    #[test]
    fn default_table_matches_the_shipped_recipe() {
        let derived = shipped();
        assert_eq!(derived.len(), TIERS.len());
        assert_eq!(TIER_REPS.len(), TIERS.len());
        for (row, &(default, name)) in derived.iter().zip(TIERS) {
            assert_eq!(row.name, name);
            assert!(
                (row.at - default).abs() < 0.02,
                "{name}: derived {} default {default}",
                row.at
            );
        }
        // Hogger's tier is Hogger's own roll-up size.
        assert_eq!(
            contact(derived.last().unwrap().at / 2.0, HOGGER_SIZE),
            Contact::RollUp
        );
    }

    #[test]
    fn every_tier_names_only_edible_things_and_none_earlier_than_it_can() {
        for row in shipped().iter().skip(1) {
            let radius = row.at / 2.0;
            let mut hardest = 0.0_f32;
            for &(rep, axis) in &row.reps {
                assert_eq!(
                    contact(radius, axis),
                    Contact::RollUp,
                    "{rep:?} at {}",
                    row.name
                );
                hardest = hardest.max(axis);
            }
            // A hair smaller and the largest named thing is no longer edible.
            assert_ne!(
                contact(radius * 0.99, hardest),
                Contact::RollUp,
                "{}",
                row.name
            );
        }
    }

    #[test]
    fn tiers_ascend_and_unknown_axes_keep_defaults() {
        let table = derive_tiers(|_| None);
        for (row, &(default, _)) in table.iter().zip(TIERS) {
            assert_eq!(row.at, default);
        }
        assert!(TIERS.windows(2).all(|w| w[0].0 < w[1].0));
        // A species that comes out bigger than the next tier pushes it along.
        let pushed = derive_tiers(|rep| (rep == Rep::Species("Cow")).then_some(5.0));
        assert!(pushed.windows(2).all(|w| w[0].at <= w[1].at));
    }

    #[test]
    fn tiers_are_held_through_a_shed() {
        let rows = derive_tiers(|_| None);
        let at = rows[3].at;
        assert_eq!(tiers_reached(&rows, at), 4);
        assert_eq!(tiers_reached(&rows, at * 0.99), 3);
        // Shedding to just below keeps the tier; a real drop loses it.
        assert_eq!(tiers_held(&rows, 4, at * 0.99), 4);
        assert_eq!(tiers_held(&rows, 4, at * 0.9), 3);
        // Growing past the next one always moves up.
        assert_eq!(tiers_held(&rows, 4, rows[4].at), 5);
    }

    #[test]
    fn beats_fire_once_through_a_shed_and_regrow() {
        let rows = derive_tiers(|_| None);
        let mut tiers = Tiers {
            announced: 1,
            current: 1,
        };
        // Up through two tiers in one frame: both come out, in order.
        assert_eq!(advance_tiers(&mut tiers, &rows, rows[2].at + 0.01), 1..3);
        // Shed through the hysteresis and back over the line: nothing new.
        for d in [rows[2].at * 0.9, rows[1].at * 0.8, rows[2].at + 0.5] {
            assert!(advance_tiers(&mut tiers, &rows, d).is_empty(), "D {d}");
        }
        assert_eq!(tiers.announced, 3);
        // Genuinely new ground still announces.
        assert_eq!(advance_tiers(&mut tiers, &rows, rows[3].at), 3..4);

        let mut steps = GrowthSteps::default();
        let base = 0.5;
        assert_eq!(new_growth_step(&mut steps, base, 0.5), None);
        assert_eq!(new_growth_step(&mut steps, base, 1.0), Some(3));
        assert_eq!(new_growth_step(&mut steps, base, 0.6), None);
        assert_eq!(new_growth_step(&mut steps, base, 1.0), None);
        assert_eq!(new_growth_step(&mut steps, base, 1.3), Some(4));
    }

    fn recipe_design() -> super::super::design::Design {
        toml::from_str(include_str!("goldshire.toml")).unwrap()
    }

    #[test]
    fn reveal_names_exist_in_the_recipe() {
        let recipe: Recipe = toml::from_str(include_str!("goldshire.toml")).unwrap();
        assert_eq!(TIER_REVEALS.len(), TIERS.len());
        assert!(TIER_REVEALS[0].is_empty());
        for (tier, names) in TIER_REVEALS.iter().enumerate().skip(1) {
            assert!(!names.is_empty(), "tier {tier}");
            for name in *names {
                assert!(
                    recipe.areas.iter().any(|a| a.name == *name),
                    "tier {tier}: no area {name:?}"
                );
            }
        }
    }

    #[test]
    fn reveals_look_ahead_from_where_the_ball_is() {
        let design = recipe_design();
        let at = |p: [f32; 3]| super::super::units::wow_to_bevy(p);
        // The inn yard looks to the market; the market's own tier looks on to the smithy.
        let (name, _) = reveal_target(&design, 1, at(design.start)).unwrap();
        assert_eq!(name, "Market");
        let (name, _) = reveal_target(&design, 2, at([-9497.0, 83.0, 57.0])).unwrap();
        assert_ne!(name, "Market");
        // Standing in the camp, the last tier has nowhere further to look.
        assert!(reveal_target(&design, 5, at(design.camp)).is_none());
        // Tier 0 never glances.
        assert!(reveal_target(&design, 0, at(design.start)).is_none());
    }
}
