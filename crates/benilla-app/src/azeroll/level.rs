//! What every level shares: the map, how big things are, how growth is credited, the tier table
//! and what wins a level. Where a level starts, how far it reaches and what its tiers are called is
//! recipe data (`design::Design`); the global tuning lives here so the ball, collection and spawning
//! agree. Module-local feel constants (acceleration, camera damping) stay in their own modules.

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

use super::collect::{contact, Contact};
use super::progress::{self, Flow, Progress};
use super::ritual::{Cue, Ending};
use super::{
    playing, BallState, Collected, Collectible, GoalReached, Kind, Phase, Run, StarEarned,
};
use crate::loading_screen::LoadingScreen;
use crate::sound::GlueSound;

/// Eastern Kingdoms: all three levels are on map 0.
pub(crate) const MAP: u32 = 0;

/// Maximum full bbox axis allowed per yard of ball radius (D >= 2.15 * axis).
pub(crate) const COLLECT_RATIO: f32 = 2.0 / 2.15;
pub(crate) const DISLODGE_RATIO: f32 = 1.5;
/// Occupied fraction of the oriented box; credited in sphere-normalized yards cubed.
pub(crate) const FILL_FACTOR: f32 = 0.5;

/// What a tier row names, for deriving its diameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rep {
    /// A roster species, by name; its fitted maximum axis.
    Species(String),
    /// The grown-up NPCs of the authored cast (children are below [`ADULT_MIN_AXIS`]).
    Adults,
}

impl Rep {
    /// `species:<Name>` or `adults`, as a recipe writes them.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text.split_once(':') {
            Some(("species", name)) if !name.is_empty() => Some(Self::Species(name.to_string())),
            None if text == "adults" => Some(Self::Adults),
            _ => None,
        }
    }
}

/// A recipe string with `'static` lifetime, for the messages that carry a tier's name. Each
/// distinct name is leaked once, so reloading a level on every restart costs nothing more.
fn intern(s: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap();
    if let Some(&known) = names.get(s) {
        return known;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    names.insert(leaked);
    leaked
}

/// The row a recipe with no `[[tiers]]` gets: just the start.
fn default_specs() -> Vec<super::design::TierSpec> {
    vec![super::design::TierSpec {
        name: "Tiny things".into(),
        at: 0.0,
        reps: Vec::new(),
    }]
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

/// The tier table for these specs and representative axes (`None`: not known yet, keep the
/// spec's own diameter). Diameters never decrease down the table.
pub(super) fn derive_tiers(
    specs: &[super::design::TierSpec],
    axis_of: impl Fn(&Rep) -> Option<f32>,
) -> Vec<TierRow> {
    let fallback;
    let specs = if specs.is_empty() {
        fallback = default_specs();
        &fallback
    } else {
        specs
    };
    let mut rows: Vec<TierRow> = Vec::with_capacity(specs.len());
    for spec in specs {
        let reps: Vec<Rep> = spec.reps.iter().filter_map(|r| Rep::parse(r)).collect();
        let known: Vec<(Rep, f32)> = reps
            .iter()
            .filter_map(|r| axis_of(r).map(|a| (r.clone(), a)))
            .collect();
        let at = if known.len() == reps.len() && !known.is_empty() {
            edible_diameter(known.iter().map(|k| k.1).fold(0.0, f32::max))
        } else {
            spec.at
        };
        let at = rows.last().map_or(at, |prev: &TierRow| at.max(prev.at));
        rows.push(TierRow {
            at,
            name: intern(&spec.name),
            reps: known,
        });
    }
    rows
}

/// The table for a level, given the biggest adult in its cast if that is known.
pub(super) fn table_for(design: &super::design::Design, adults: Option<f32>) -> Vec<TierRow> {
    derive_tiers(&design.tiers, |rep| match rep {
        Rep::Species(name) => design
            .roster
            .iter()
            .find(|s| s.name == *name)
            .and_then(|s| s.fit),
        Rep::Adults => adults,
    })
}

/// One derived tier.
#[derive(Debug, Clone)]
pub(crate) struct TierRow {
    pub at: f32,
    pub name: &'static str,
    /// The representatives whose axes are known.
    pub reps: Vec<(Rep, f32)>,
}

/// The tier table the running level announces from: the recipe's diameters where nothing better is
/// known, the roster's and the cast's real sizes where it is.
#[derive(Resource)]
pub(crate) struct TierTable {
    pub rows: Vec<TierRow>,
}

impl Default for TierTable {
    fn default() -> Self {
        Self {
            rows: derive_tiers(&[], |_| None),
        }
    }
}

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
    /// the model: how a level's boss is enlarged.
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
        Self {
            announced: 1,
            current: 1,
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
        .add_systems(
            OnEnter(Phase::Ready),
            reset_tiers.after(super::design::reload),
        )
        .add_systems(Update, (keys, begin, tick_banner, refresh_tiers))
        .add_systems(
            Update,
            (tick, stars, announce, growth_beats)
                .chain()
                .after(WorldStage::Input)
                .run_if(playing),
        );
}

fn reset_tiers(
    mut tiers: ResMut<Tiers>,
    mut table: ResMut<TierTable>,
    design: Res<super::design::Design>,
    mut growth: ResMut<GrowthSteps>,
) {
    *growth = GrowthSteps::default();
    // This level's table, without its cast (refresh_tiers adds the adults once they spawn).
    table.rows = table_for(&design, None);
    let n = tiers_reached(&table.rows, 2.0 * design.start_radius);
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
/// and say where it disagrees with the recipe's own diameters.
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
    let rows = table_for(&design, adults);
    for (row, spec) in rows.iter().zip(&design.tiers) {
        if (row.at - spec.at).abs() > 0.05 {
            warn!(
                "azeroll: tier {:?} derives to {:.2} yd, the recipe says {:.2}",
                row.name, row.at, spec.at
            );
        }
    }
    table.rows = rows;
}

/// How long Enter is held to call a size-goal run (the clock's rim fills over it), real seconds.
pub(super) const CALL_HOLD: f32 = 1.0;
/// The rim drains this much faster than it fills when the key is let go.
const CALL_DRAIN: f32 = 3.0;

/// `WOW_AZEROLL_CALL_AT=<play secs>` (a capture and test aid): hold the finish key from that play
/// time on, as a player would.
fn call_at() -> Option<f32> {
    crate::webenv::var("WOW_AZEROLL_CALL_AT")?.parse().ok()
}

/// Esc pauses and resumes; R restarts from play, pause or the results; L leaves a paused run for
/// the level select; F starts the untimed Free Roll from the title once the level has been won
/// before. Holding Enter after a size goal is met calls the run. (The results' keys are
/// [`super::ritual`]'s, the hub's [`super::select`]'s.)
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    real: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    design: Res<super::design::Design>,
    mut run: ResMut<Run>,
    progress: Res<Progress>,
    current: Res<super::campaign::CurrentLevel>,
    mut select: ResMut<super::campaign::SelectState>,
    mut flow: ResMut<Flow>,
    mut next: ResMut<NextState<Phase>>,
    mut held: Local<f32>,
) {
    let restart = keys.just_pressed(KeyCode::KeyR);
    let escape = keys.just_pressed(KeyCode::Escape);
    let free = keys.just_pressed(KeyCode::KeyF);
    if *phase.get() != Phase::Playing {
        *held = 0.0;
        flow.call_hold = 0.0;
    }
    match phase.get() {
        Phase::Select | Phase::Results => {}
        Phase::Ready => {
            if (free || select.free_pending) && !loading.covering() {
                select.free_pending = false;
                if progress.wins(current.id()) > 0 {
                    progress::start_free_roll(&mut flow, &mut next);
                }
            }
        }
        Phase::Playing => {
            if restart {
                next.set(Phase::Ready);
            } else if escape {
                next.set(Phase::Paused);
            }
            // Calling it: only after the goal of a size level, never in Free Roll.
            let callable = run.won
                && !flow.free_roll
                && flow.finale.is_none()
                && design.goal.kind == super::design::GoalKind::Size;
            let down =
                keys.pressed(KeyCode::Enter) || call_at().is_some_and(|at| run.elapsed >= at);
            let dt = real.delta_secs().min(0.1);
            if callable && down {
                *held += dt;
            } else {
                *held = (*held - dt * CALL_DRAIN).max(0.0);
            }
            flow.call_hold = (*held / CALL_HOLD).clamp(0.0, 1.0);
            if callable && *held >= CALL_HOLD {
                *held = 0.0;
                flow.call_hold = 0.0;
                flow.ending = Some(Ending::Called);
                run.called = true;
                info!("azeroll: the run was called at {:.1}s", run.elapsed);
                next.set(Phase::Results);
            }
        }
        Phase::Paused => {
            if restart {
                next.set(Phase::Ready);
            } else if keys.just_pressed(KeyCode::KeyL) {
                next.set(Phase::Select);
            } else if escape {
                next.set(Phase::Playing);
            }
        }
    }
}

/// Ready becomes Playing on the first movement key or Enter, once the loading screen is down.
fn begin(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    ball: Res<BallState>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::Ready || loading.covering() || !ball.floor_ready {
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

/// Seconds of play before a size goal can be met: a debug start above the goal must not end the
/// run in Ready's first frame.
const GOAL_AFTER: f32 = 0.5;
/// The goal beat's banner.
const GOAL_BANNER_SECS: f32 = 2.0;

/// The clock, and the ways a run ends. A single-target goal (a named creature, the Cathedral) ends
/// on capture, after its pre-roll; a size goal is only a beat (the dial, the banner, Fizzwick): the
/// run goes on to the candle or to the player calling it, and the final size is what is graded.
/// The clock stops at the capture of a single-target goal and at the candle, never at a size goal.
/// Free Roll has no clock and no end: meeting the goal in it only earns the banner.
pub(super) fn tick(
    time: Res<Time>,
    real: Res<Time<Real>>,
    design: Res<super::design::Design>,
    ball: Res<BallState>,
    mut run: ResMut<Run>,
    mut flow: ResMut<Flow>,
    finale: Res<super::finale::Finale>,
    mut collected: MessageReader<Collected>,
    mut next: ResMut<NextState<Phase>>,
    mut sounds: MessageWriter<GlueSound>,
    mut reached: MessageWriter<GoalReached>,
    mut cues: MessageWriter<Cue>,
) {
    use super::design::GoalKind;
    if flow.free_roll
        && design
            .live_radius_limit
            .is_some_and(|limit| ball.radius >= limit)
    {
        next.set(Phase::Results);
        return;
    }
    let size_goal = design.goal.kind == GoalKind::Size;
    if !flow.free_roll && (!run.won || size_goal) {
        run.elapsed += time.delta_secs();
    }
    // The goal's own target: Hogger can be a Boss without being the goal (Market Day).
    let boss = collected
        .read()
        .any(|c| c.kind == Kind::Boss && c.name == design.goal.target.as_str());
    let settled = flow.free_roll || run.elapsed > GOAL_AFTER;
    if !run.won
        && settled
        && (goal_met(&design.goal, boss, ball.diameter())
            || (design.goal.kind == GoalKind::Finale && finale.triggered))
    {
        run.won = true;
        if flow.free_roll {
            run.banner = Some((design.win_title().to_uppercase(), 4.0));
            reached.write(GoalReached {
                diameter: ball.diameter(),
                goal: design.goal_diameter(),
            });
        } else if size_goal {
            run.goal_time = Some(run.elapsed);
            run.stars = 1;
            run.banner = Some((design.win_title().to_uppercase(), GOAL_BANNER_SECS));
            cues.write(Cue::GoalFanfare);
            reached.write(GoalReached {
                diameter: ball.diameter(),
                goal: design.goal_diameter(),
            });
            info!(
                "azeroll: goal reached at {:.1}s, D={:.2} (goal {:.2}); rolling on to the candle",
                run.elapsed,
                ball.diameter(),
                design.goal_diameter()
            );
        } else {
            run.banner = None;
            run.goal_time = Some(run.elapsed);
            flow.finale = Some(0.0);
            flow.finale_len = match design.goal.kind {
                GoalKind::Finale => progress::FINALE_SECS,
                _ => progress::COLLECT_FINALE_SECS,
            };
            sounds.write(GlueSound(progress::FANFARE_SOUND));
            reached.write(GoalReached {
                diameter: ball.diameter(),
                goal: design.goal_diameter(),
            });
        }
    }
    if let Some(t) = flow.finale.as_mut() {
        *t += real.delta_secs();
        if *t >= flow.finale_len {
            flow.finale = None;
            flow.ending = Some(Ending::Captured);
            next.set(Phase::Results);
        }
    } else if !flow.free_roll && run.elapsed >= design.time_limit {
        run.elapsed = design.time_limit;
        flow.ending = Some(if run.won {
            Ending::Snuff
        } else {
            Ending::Failed
        });
        next.set(Phase::Results);
    }
}

/// The dial's stars: one at the goal, one more at each of the level's grade boundaries (1.2x, 1.5x
/// and 2x the goal by default). A star once lit stays lit, however the ball sheds afterwards.
fn stars(
    design: Res<super::design::Design>,
    ball: Res<BallState>,
    flow: Res<Flow>,
    mut run: ResMut<Run>,
    mut earned: MessageWriter<StarEarned>,
) {
    if flow.free_roll || !run.won || design.goal.kind != super::design::GoalKind::Size {
        return;
    }
    let bounds = design.level.grades.unwrap_or(progress::DEFAULT_SIZE_GRADES);
    let now = stars_for(ball.diameter(), design.goal_diameter(), bounds);
    if now > run.stars {
        run.stars = now;
        earned.write(StarEarned { n: now });
    }
}

/// Stars lit by a ball this big: one at the goal, plus one per grade boundary reached.
pub(super) fn stars_for(diameter: f32, goal: f32, bounds: [f32; 3]) -> u8 {
    if goal <= 0.0 || diameter < goal {
        return 0;
    }
    1 + bounds.iter().filter(|&&b| diameter >= b * goal).count() as u8
}

/// Whether a level's goal is met: its boss was rolled up this frame (`boss`), or the ball is at
/// least the goal diameter across.
pub(super) fn goal_met(goal: &super::design::Goal, boss: bool, diameter: f32) -> bool {
    match goal.kind {
        super::design::GoalKind::Collect => boss,
        super::design::GoalKind::Size => diameter >= goal.diameter,
        super::design::GoalKind::Finale => false,
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
    flow: Res<Flow>,
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
        if flow.finale.is_none() {
            run.banner = Some((row.name.to_string(), 2.6));
        }
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
    use crate::azeroll::campaign::LEVELS;
    use crate::azeroll::design::{self, Design, Goal, GoalKind};

    /// The adult NPC axis the shipped cast measures at (`azeroll NPC: ... axis=` in the log).
    const ADULT_AXIS: f32 = 2.01;

    fn recipe(id: &str) -> Design {
        let entry = LEVELS.iter().find(|l| l.id == id).unwrap();
        design::parse(entry.recipe).unwrap()
    }

    fn market() -> Design {
        recipe("market-day")
    }

    fn shipped() -> Vec<TierRow> {
        table_for(&market(), Some(ADULT_AXIS))
    }

    /// The tier diameters Market Day announces (Hogger's, then the Prize Pumpkin's).
    const MARKET_TIERS: &[(f32, &str)] = &[
        (0.0, "Crumbs and coppers"),
        (1.04, "Critters"),
        (2.58, "Livestock"),
        (4.33, "Shoppers and horses"),
        (6.02, "Raiders at the gate"),
        (8.6, "Hogger"),
        (12.03, "The Prize Pumpkin"),
    ];

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
    fn market_day_derives_the_tiers_it_always_had() {
        let derived = shipped();
        let design = market();
        assert_eq!(derived.len(), MARKET_TIERS.len());
        assert_eq!(design.tiers.len(), MARKET_TIERS.len());
        for ((row, spec), &(default, name)) in derived.iter().zip(&design.tiers).zip(MARKET_TIERS) {
            assert_eq!(row.name, name);
            assert_eq!(spec.name, name);
            assert!(
                (spec.at - default).abs() < 1e-3,
                "{name}: recipe {}",
                spec.at
            );
            assert!(
                (row.at - default).abs() < 0.02,
                "{name}: derived {} default {default}",
                row.at
            );
        }
        // Authored tier labels are rounded hundredths; the goal's actual contact gate is
        // derived from the full geometry axis, so a rounded tier may be just below RollUp.
        assert!((derived.last().unwrap().at - design.goal_diameter()).abs() < 0.02);
        assert!((design.goal_diameter() - 12.023).abs() < 0.01);
    }

    #[test]
    fn every_tier_names_only_edible_things_and_none_earlier_than_it_can() {
        for row in shipped().iter().skip(1) {
            // A tier with no representatives (the Prize Pumpkin's) is a plain diameter.
            if row.reps.is_empty() {
                continue;
            }
            let radius = row.at / 2.0;
            let mut hardest = 0.0_f32;
            for &(ref rep, axis) in &row.reps {
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
    fn every_level_has_an_ascending_table_starting_at_zero() {
        for entry in LEVELS {
            let d = design::parse(entry.recipe).unwrap();
            let rows = table_for(&d, Some(ADULT_AXIS));
            assert!(!rows.is_empty(), "{}", entry.id);
            assert_eq!(rows[0].at, 0.0, "{}: the first tier is the start", entry.id);
            assert!(
                rows.windows(2).all(|w| w[0].at <= w[1].at),
                "{}: tiers must ascend",
                entry.id
            );
            // The start is inside the first tier only: nothing is announced at the start.
            assert_eq!(
                tiers_reached(&rows, 2.0 * d.start_radius),
                1,
                "{}",
                entry.id
            );
            // And the goal is at or beyond the last tier.
            assert!(
                d.goal_diameter() >= rows.last().unwrap().at - 0.02,
                "{}: goal {} before the last tier {}",
                entry.id,
                d.goal_diameter(),
                rows.last().unwrap().at
            );
        }
    }

    #[test]
    fn tiers_ascend_and_unknown_axes_keep_defaults() {
        let design = market();
        let table = derive_tiers(&design.tiers, |_| None);
        for (row, spec) in table.iter().zip(&design.tiers) {
            assert_eq!(row.at, spec.at);
        }
        // A species that comes out bigger than the next tier pushes it along.
        let pushed = derive_tiers(&design.tiers, |rep| {
            (*rep == Rep::Species("Cow".into())).then_some(5.0)
        });
        assert!(pushed.windows(2).all(|w| w[0].at <= w[1].at));
        // A recipe with no tiers still has its start row.
        assert_eq!(derive_tiers(&[], |_| None).len(), 1);
    }

    #[test]
    fn reps_parse_as_the_recipe_writes_them() {
        assert_eq!(Rep::parse("adults"), Some(Rep::Adults));
        assert_eq!(
            Rep::parse("species:Farm horse"),
            Some(Rep::Species("Farm horse".into()))
        );
        assert_eq!(Rep::parse("species:"), None);
        assert_eq!(Rep::parse("mystery"), None);
    }

    #[test]
    fn tiers_are_held_through_a_shed() {
        let rows = derive_tiers(&market().tiers, |_| None);
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
        let rows = derive_tiers(&market().tiers, |_| None);
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

    #[test]
    fn goals_are_met_by_their_own_kind() {
        let collect = Goal {
            kind: GoalKind::Collect,
            target: "Hogger".into(),
            diameter: 0.0,
            city_pieces: 0,
            axis: 0.0,
            area: None,
            title: None,
            fail: None,
        };
        // Rolling up the boss wins a creature goal at any size; size alone never does.
        assert!(goal_met(&collect, true, 1.0));
        assert!(!goal_met(&collect, false, 50.0));
        let size = Goal {
            kind: GoalKind::Size,
            target: String::new(),
            diameter: 1.5,
            ..collect.clone()
        };
        // A size goal is met at the diameter, boss or no boss, and not a hair before.
        assert!(goal_met(&size, false, 1.5));
        assert!(goal_met(&size, false, 2.0));
        assert!(!goal_met(&size, false, 1.499));
        assert!(!goal_met(&size, true, 1.0), "a size goal has no boss");
    }

    #[test]
    fn stars_are_one_at_the_goal_and_one_per_grade_boundary() {
        let b = progress::DEFAULT_SIZE_GRADES;
        assert_eq!(stars_for(1.99, 2.0, b), 0);
        assert_eq!(stars_for(2.0, 2.0, b), 1);
        assert_eq!(stars_for(2.39, 2.0, b), 1);
        assert_eq!(stars_for(2.4, 2.0, b), 2);
        assert_eq!(stars_for(3.0, 2.0, b), 3);
        assert_eq!(stars_for(4.0, 2.0, b), 4);
        assert_eq!(stars_for(40.0, 2.0, b), 4);
        assert_eq!(stars_for(5.0, 0.0, b), 0);
    }

    #[test]
    fn the_shipped_size_level_keeps_rolling_and_the_others_end_on_capture() {
        use crate::azeroll::design::GoalKind;
        let kinds: Vec<GoalKind> = LEVELS
            .iter()
            .map(|l| design::parse(l.recipe).unwrap().goal.kind)
            .collect();
        assert_eq!(kinds[0], GoalKind::Size, "L1 is the size level");
        assert_ne!(kinds[1], GoalKind::Size);
        assert_eq!(kinds[2], GoalKind::Finale);
    }

    #[test]
    fn interned_names_are_stable() {
        let a = intern("Some tier");
        let b = intern("Some tier");
        assert!(std::ptr::eq(a, b), "the same name must not leak twice");
    }
}
