//! Fizzwick in play (D31, `docs/azeroll/ritual-design.md` §3): a small porthole pops in under the
//! clock with one line, at most three times a run, never over the ball. This module decides *when*
//! (the budget and the events); `hud` draws the porthole and `sfx` plays the gnome's voice. All the
//! words are `lines.rs`'s.

use bevy::prelude::*;

use super::campaign::{CurrentLevel, LEVELS};
use super::design::{Design, GoalKind};
use super::lines::{self, Pool, Said};
use super::progress::Flow;
use super::{BallState, Collected, GoalReached, Phase, Rng, Run, StarEarned, TierReached};

/// Pop-ins per run, the real seconds between them, and the play seconds at the start with none.
pub(super) const POPS_PER_RUN: u8 = 3;
pub(super) const POP_GAP: f32 = 20.0;
pub(super) const QUIET_START: f32 = 6.0;
/// A tier line only fills a silence at least this long (real seconds since the last pop-in).
const TIER_SILENCE: f32 = 60.0;
/// Play seconds left at which the sixty-seconds line fires.
const SIXTY: f32 = 60.0;

/// How Fizzwick sounds: the gnome's own emote voices (`sfx` resolves them), or only a gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Voice {
    Cheer,
    Laugh,
    Gasp,
    Sigh,
    Talk,
}

/// One line for the porthole.
#[derive(Message, Clone, Copy, Debug)]
pub(super) struct FizzSays {
    pub line: &'static str,
    pub voice: Voice,
}

/// What can move him to speak, best first (a lower number wins a contested frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Event {
    Goal,
    Sixty,
    Preposterous,
    Landmark,
    Tier,
}

/// The run's pop-in ledger.
#[derive(Resource, Default, Debug)]
pub(super) struct Pops {
    pub used: u8,
    /// The goal has had its line (until it has, one slot is held back for it).
    pub goal_done: bool,
    /// Real seconds at the last pop-in.
    pub last_at: Option<f32>,
    /// The sixty-seconds line fired.
    sixty_done: bool,
    /// The Preposterous line fired.
    preposterous_done: bool,
    /// Events dropped for want of a slot, for the log.
    pub dropped: u32,
    /// D33's bonus pop-ins, outside the budget: the parcel and the cousin each get one a run,
    /// waiting here until the porthole is free.
    pub bonus: Vec<Bonus>,
    parcel_done: bool,
    cousin_done: bool,
}

/// A replay-layer pop-in, outside the three-a-run budget (`replay-design.md` §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bonus {
    Parcel,
    CousinFair,
    CousinSwallowed,
}

/// Real seconds a bonus pop-in waits after any other pop-in (the porthole shows one line at a
/// time, and the next must not cut it off).
pub(super) const BONUS_GAP: f32 = 4.0;

/// Whether the bonus at the head of the queue may pop in now: the porthole has been quiet.
pub(super) fn bonus_ready(pops: &Pops, now: f32) -> bool {
    !pops.bonus.is_empty() && pops.last_at.is_none_or(|t| now - t >= BONUS_GAP)
}

/// `WOW_AZEROLL_FIZZ_TEST=1`: no gaps and no quiet start (still three a run), so a capture run shows
/// every kind of event.
pub(super) fn test_mode() -> bool {
    crate::webenv::var("WOW_AZEROLL_FIZZ_TEST").is_some()
}

/// Whether an event may pop in now. `play` is the run's play seconds, `now` real seconds.
pub(super) fn admit(
    pops: &Pops,
    event: Event,
    play: f32,
    now: f32,
    free: bool,
    test: bool,
) -> bool {
    if free || pops.used >= POPS_PER_RUN {
        return false;
    }
    if event == Event::Goal {
        return true;
    }
    // One slot is held for the goal until it has happened.
    if !pops.goal_done && pops.used + 1 >= POPS_PER_RUN {
        return false;
    }
    if test {
        return true;
    }
    if play < QUIET_START {
        return false;
    }
    let since = pops.last_at.map_or(f32::INFINITY, |t| now - t);
    if event == Event::Tier {
        return since >= TIER_SILENCE;
    }
    since >= POP_GAP
}

/// The voice for an event.
pub(super) fn voice_for(event: Event, goal_met: bool) -> Voice {
    match event {
        Event::Goal | Event::Landmark => Voice::Cheer,
        Event::Preposterous => Voice::Laugh,
        Event::Sixty if goal_met => Voice::Laugh,
        Event::Sixty => Voice::Gasp,
        Event::Tier => Voice::Talk,
    }
}

fn pool_for(event: Event, goal_met: bool) -> Option<&'static Pool> {
    Some(match event {
        Event::Goal => &lines::GOAL,
        Event::Sixty if goal_met => &lines::SIXTY_AFTER,
        Event::Sixty => &lines::SIXTY_BEFORE,
        Event::Preposterous => &lines::PREPOSTEROUS,
        Event::Landmark => &lines::LANDMARK,
        Event::Tier => return None,
    })
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Pops>()
        .add_message::<FizzSays>()
        .add_systems(OnEnter(Phase::Ready), reset)
        .add_systems(
            Update,
            bonus.run_if(
                in_state(Phase::Playing)
                    .or(in_state(Phase::Paused))
                    .or(in_state(Phase::Results)),
            ),
        )
        .add_systems(
            Update,
            pops.run_if(in_state(Phase::Playing))
                .after(benilla_world::schedule::WorldStage::Input)
                .after(super::level::tick),
        );
}

fn reset(mut pops: ResMut<Pops>) {
    if pops.dropped > 0 {
        info!(
            "azeroll fizz: {} event(s) were dropped last run",
            pops.dropped
        );
    }
    *pops = Pops::default();
}

/// Gather this frame's events, take the best, and let it pop in if the budget allows.
#[allow(clippy::too_many_arguments)]
fn pops(
    real: Res<Time<Real>>,
    run: Res<Run>,
    flow: Res<Flow>,
    design: Res<Design>,
    ball: Res<BallState>,
    current: Res<CurrentLevel>,
    mut goals: MessageReader<GoalReached>,
    mut stars: MessageReader<StarEarned>,
    mut tiers: MessageReader<TierReached>,
    mut collected: MessageReader<Collected>,
    mut ledger: ResMut<Pops>,
    mut said: ResMut<Said>,
    mut rng: ResMut<Rng>,
    mut out: MessageWriter<FizzSays>,
    mut last_left: Local<Option<f32>>,
) {
    let now = real.elapsed_secs();
    let mut events: Vec<Event> = Vec::new();
    if goals.read().count() > 0 && !flow.free_roll {
        events.push(Event::Goal);
    }
    // Twice the request: the top grade's star on a size level; the top of the grade bounds against
    // the diameter the target needs on a single-target one.
    let top = design
        .level
        .grades
        .map_or(super::progress::DEFAULT_SIZE_GRADES[2], |g| g[2]);
    let preposterous = if design.goal.kind == GoalKind::Size {
        stars.read().any(|s| s.n >= 4)
    } else {
        stars.read().count() > 0 || ball.diameter() >= top * design.goal_diameter()
    };
    if preposterous && !ledger.preposterous_done && !flow.free_roll {
        events.push(Event::Preposterous);
    }
    let landmark = LEVELS[current.0].landmark;
    if collected
        .read()
        .any(|c| c.name.eq_ignore_ascii_case(landmark))
    {
        events.push(Event::Landmark);
    }
    if tiers.read().count() > 0 {
        events.push(Event::Tier);
    }
    let left = design.time_limit - run.elapsed;
    if last_left.is_some_and(|prev| prev > SIXTY)
        && left <= SIXTY
        && !ledger.sixty_done
        && !flow.free_roll
    {
        events.push(Event::Sixty);
    }
    *last_left = Some(left);

    events.sort();
    let Some(&event) = events.first() else { return };
    let test = test_mode();
    let goal_met = run.won;
    if !admit(&ledger, event, run.elapsed, now, flow.free_roll, test) {
        ledger.dropped += 1;
        info!(
            "azeroll fizz: {event:?} dropped (used {}, last {:?})",
            ledger.used, ledger.last_at
        );
        return;
    }
    let line = match pool_for(event, goal_met) {
        Some(pool) => lines::pick(pool, current.0, &mut said, rng.next_u64()),
        None => lines::pick_flat(lines::TIER, &mut said, rng.next_u64()),
    };
    if line.is_empty() {
        return;
    }
    ledger.used += 1;
    ledger.last_at = Some(now);
    match event {
        Event::Goal => ledger.goal_done = true,
        Event::Sixty => ledger.sixty_done = true,
        Event::Preposterous => ledger.preposterous_done = true,
        _ => {}
    }
    info!("azeroll fizz: {event:?} #{} {line:?}", ledger.used);
    out.write(FizzSays {
        line,
        voice: voice_for(event, goal_met),
    });
}

/// The parcel and the cousin: queue their pop-ins, and let each out when the porthole is quiet.
/// They ignore the budget and Free Roll (a find is a find) but keep the gap, so a goal line is
/// never cut off.
#[allow(clippy::too_many_arguments)]
fn bonus(
    real: Res<Time<Real>>,
    current: Res<CurrentLevel>,
    mut parcels: MessageReader<super::replay::ParcelFound>,
    mut cousins: MessageReader<super::replay::CousinCaught>,
    mut ledger: ResMut<Pops>,
    mut said: ResMut<Said>,
    mut rng: ResMut<Rng>,
    mut out: MessageWriter<FizzSays>,
) {
    let now = real.elapsed_secs();
    for p in parcels.read() {
        if !ledger.parcel_done {
            info!("azeroll fizz: parcel {:?} queued", p.title);
            ledger.parcel_done = true;
            ledger.bonus.push(Bonus::Parcel);
        }
    }
    for c in cousins.read() {
        if !ledger.cousin_done {
            info!("azeroll fizz: cousin {} ({}) queued", c.name, c.key);
            ledger.cousin_done = true;
            ledger.bonus.push(if c.fair {
                Bonus::CousinFair
            } else {
                Bonus::CousinSwallowed
            });
        }
    }
    if !bonus_ready(&ledger, now) {
        return;
    }
    let which = ledger.bonus.remove(0);
    let pool = match which {
        Bonus::Parcel => &lines::PARCEL,
        Bonus::CousinFair => &lines::COUSIN_FAIR,
        Bonus::CousinSwallowed => &lines::COUSIN_SWALLOWED,
    };
    let line = lines::pick(pool, current.0, &mut said, rng.next_u64());
    if line.is_empty() {
        return;
    }
    ledger.last_at = Some(now);
    info!("azeroll fizz: bonus {which:?} {line:?}");
    out.write(FizzSays {
        line,
        voice: Voice::Cheer,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bonus_waits_for_a_quiet_porthole_and_never_spends_the_budget() {
        let mut p = Pops::default();
        assert!(!bonus_ready(&p, 10.0), "nothing queued");
        p.bonus.push(Bonus::Parcel);
        assert!(bonus_ready(&p, 10.0));
        p.last_at = Some(8.0);
        assert!(!bonus_ready(&p, 10.0), "a goal line is still showing");
        assert!(bonus_ready(&p, 8.0 + BONUS_GAP));
        // The budget is untouched: a run with the goal, a parcel, a cousin and sixty seconds
        // still has its three budgeted pops.
        assert_eq!(p.used, 0);
    }

    fn pops(used: u8, goal_done: bool, last: Option<f32>) -> Pops {
        Pops {
            used,
            goal_done,
            last_at: last,
            ..default()
        }
    }

    #[test]
    fn the_goal_always_gets_a_slot_while_one_is_left() {
        // Two tiers spent: only the goal may still speak.
        let p = pops(2, false, Some(10.0));
        assert!(admit(&p, Event::Goal, 100.0, 30.0, false, false));
        assert!(!admit(&p, Event::Sixty, 100.0, 90.0, false, false));
        assert!(!admit(&p, Event::Landmark, 100.0, 90.0, false, false));
        // Once the goal has spoken the last slot is anyone's.
        let p = pops(2, true, Some(10.0));
        assert!(admit(&p, Event::Sixty, 100.0, 90.0, false, false));
        // Three used: nothing, the goal included.
        assert!(!admit(
            &pops(3, true, Some(10.0)),
            Event::Goal,
            100.0,
            90.0,
            false,
            false
        ));
    }

    #[test]
    fn gaps_and_the_quiet_start_hold_back_everything_but_the_goal() {
        let p = pops(1, false, Some(50.0));
        assert!(!admit(&p, Event::Landmark, 80.0, 60.0, false, false), "gap");
        assert!(admit(&p, Event::Landmark, 80.0, 71.0, false, false));
        assert!(
            !admit(
                &pops(0, false, None),
                Event::Landmark,
                3.0,
                3.0,
                false,
                false
            ),
            "quiet"
        );
        assert!(admit(
            &pops(0, false, None),
            Event::Goal,
            3.0,
            3.0,
            false,
            false
        ));
        // A tier only fills a long silence.
        assert!(!admit(
            &pops(0, false, Some(10.0)),
            Event::Tier,
            80.0,
            40.0,
            false,
            false
        ));
        assert!(admit(
            &pops(0, false, Some(10.0)),
            Event::Tier,
            100.0,
            80.0,
            false,
            false
        ));
    }

    #[test]
    fn free_roll_has_no_pop_ins_and_the_test_mode_has_no_gaps() {
        assert!(!admit(
            &pops(0, false, None),
            Event::Goal,
            50.0,
            50.0,
            true,
            false
        ));
        let p = pops(1, false, Some(50.0));
        assert!(admit(&p, Event::Landmark, 2.0, 51.0, false, true));
        // But three is still three.
        assert!(!admit(
            &pops(3, true, None),
            Event::Goal,
            50.0,
            50.0,
            false,
            true
        ));
    }

    #[test]
    fn priority_orders_goal_first_and_tier_last() {
        let mut v = vec![
            Event::Tier,
            Event::Landmark,
            Event::Sixty,
            Event::Goal,
            Event::Preposterous,
        ];
        v.sort();
        assert_eq!(
            v,
            vec![
                Event::Goal,
                Event::Sixty,
                Event::Preposterous,
                Event::Landmark,
                Event::Tier
            ]
        );
    }

    #[test]
    fn the_voice_fits_the_feeling() {
        assert_eq!(voice_for(Event::Goal, false), Voice::Cheer);
        assert_eq!(voice_for(Event::Sixty, false), Voice::Gasp);
        assert_eq!(voice_for(Event::Sixty, true), Voice::Laugh);
        assert_eq!(voice_for(Event::Preposterous, true), Voice::Laugh);
        assert_eq!(voice_for(Event::Tier, false), Voice::Talk);
    }
}
