//! Replayability: the finale's clock and camera, the bests that survive a restart, the optional
//! challenges, and the untimed Free Roll. [`Flow`] is the per-run state the level flow and the HUD
//! share; [`Progress`] is the persisted side, written through [`crate::local_state`] when a run ends
//! (a file natively, `localStorage` on the page, nothing when `$WOW_CAPTURE` is set).
//!
//! The finale itself is level flow (`level::tick` starts it on Hogger and ends it into the results);
//! this module only dresses it: slow motion through the virtual clock and a wider, higher camera
//! through [`CamRig`], both undone on the way out.

use std::collections::BTreeMap;

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::camera::CamRig;
use super::collect::Ram;
use super::design::Design;
use super::{units, Attached, BallState, Collectible, Phase, Run, Source};
use crate::sound::GlueSound;

/// Which level the bests belong to. One level today; the key keeps a second from sharing records.
pub(super) const LEVEL_ID: &str = "goldshire";

/// Real seconds from rolling up Hogger to the results screen.
pub(super) const FINALE_SECS: f32 = 3.2;
/// The celebration's fanfare, and the chime for a record: SoundEntries kit names.
pub(super) const FANFARE_SOUND: &str = "QUESTCOMPLETED";
const RECORD_SOUND: &str = "LEVELUP";

/// The time dilation: this speed until `SLOW_HOLD`, then easing back to 1 by `SLOW_END`.
const SLOW_SPEED: f32 = 0.2;
const SLOW_HOLD: f32 = 0.9;
const SLOW_END: f32 = 2.6;
/// The camera pulls out over `PULL_SECS`: this much more boom, this much more tilt.
const PULL_SECS: f32 = 1.0;
const PULL_ZOOM: f32 = 1.6;
const PULL_TILT: f32 = 0.3;

// ---------------------------------------------------------------------------------------------
// The optional challenges
// ---------------------------------------------------------------------------------------------

/// `(id, title)`: the ids are what the save file stores, so they never change.
pub(super) const CHALLENGES: [(&str, &str); 4] = [
    ("chickens", "Roll up every chicken in Goldshire"),
    ("speed", "Beat Hogger in under 7:00"),
    ("supper", "Roll up the Inn's whole supper spill"),
    ("unrammed", "Win without being rammed"),
];
const SPEED_SECS: f32 = 420.0;
/// The supper spill is the scene of this name in the recipe; its things lie within this many yards
/// of the scene's anchor.
const SPILL_SCENE: &str = "Inn supper spill";
const SPILL_RADIUS: f32 = 4.0;
const CHICKEN: &str = "Chicken";

// ---------------------------------------------------------------------------------------------
// The persisted bests
// ---------------------------------------------------------------------------------------------

/// What the player has done on one level.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub(super) struct LevelBest {
    /// Fastest win, seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_time: Option<f32>,
    /// Biggest final ball, diameter in yards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biggest: Option<f32>,
    /// Most things rolled up in one run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub most: Option<u32>,
    /// Hogger rolled up this many times.
    #[serde(default)]
    pub wins: u32,
    /// Ids of [`CHALLENGES`] met at least once.
    #[serde(default)]
    pub challenges: Vec<String>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct SaveFile {
    #[serde(default)]
    levels: BTreeMap<String, LevelBest>,
}

/// The bests, loaded once at startup.
#[derive(Resource)]
pub(super) struct Progress {
    saved: SaveFile,
    /// False for a debug-boosted run, which must not set records (see [`debug_radius`]).
    persist: bool,
}

impl Progress {
    fn load() -> Self {
        let saved = crate::local_state::azeroll_progress_path()
            .and_then(|p| crate::local_state::read_to_string(&p).ok())
            .and_then(|text| match toml::from_str::<SaveFile>(&text) {
                Ok(file) => Some(file),
                Err(e) => {
                    warn!("azeroll: progress file unreadable, starting fresh: {e}");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            saved,
            persist: debug_radius().is_none() || crate::webenv::var("BENILLA_HOME").is_some(),
        }
    }

    /// This level's record.
    pub fn best(&self) -> LevelBest {
        self.saved.levels.get(LEVEL_ID).cloned().unwrap_or_default()
    }

    fn save(&self) {
        if !self.persist {
            return;
        }
        let Some(path) = crate::local_state::azeroll_progress_path() else {
            return;
        };
        match toml::to_string(&self.saved) {
            Ok(text) => {
                if let Err(e) = crate::local_state::write_atomic(&path, &text) {
                    warn!(
                        "azeroll: could not save progress to {}: {e}",
                        path.display()
                    );
                }
            }
            Err(e) => warn!("azeroll: could not encode progress: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The run's flow
// ---------------------------------------------------------------------------------------------

/// How one challenge went in the run that just ended.
#[derive(Debug, Clone)]
pub(super) struct ChallengeResult {
    pub title: &'static str,
    /// Met by this run.
    pub met: bool,
    /// Met by this run for the first time.
    pub fresh: bool,
    /// Met by an earlier run (so it stays earned whatever this run did).
    pub earned: bool,
}

/// What the results screen shows beyond the run's own numbers.
#[derive(Debug, Clone, Default)]
pub(super) struct Outcome {
    pub new_time: bool,
    pub new_size: bool,
    pub new_count: bool,
    /// The records before this run.
    pub before: LevelBest,
    pub challenges: Vec<ChallengeResult>,
}

/// The state of the run beyond [`Run`]: the finale, Free Roll, rams and the results' extras.
#[derive(Resource, Default)]
pub(super) struct Flow {
    /// Real seconds since Hogger was rolled up, while the celebration plays.
    pub finale: Option<f32>,
    /// The untimed mode: no clock, no limit, no results, no records.
    pub free_roll: bool,
    /// Rams taken this run.
    pub rams: u32,
    /// Filled when the run ends on the results screen.
    pub outcome: Outcome,
    /// The player's zoom and tilt before the finale took the camera, restored for Free Roll.
    cam_before: Option<(f32, f32)>,
}

/// Start Free Roll: the same run, untimed. Used by the results screen, the title and the autopilot.
pub(super) fn start_free_roll(flow: &mut Flow, next: &mut NextState<Phase>) {
    flow.free_roll = true;
    next.set(Phase::Playing);
}

/// `WOW_AZEROLL_DEBUG_RADIUS=<yards>`: start every run with a ball this big, to reach Hogger fast.
fn debug_radius() -> Option<f32> {
    crate::webenv::var("WOW_AZEROLL_DEBUG_RADIUS")?.parse().ok()
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(Progress::load())
        .init_resource::<Flow>()
        .add_systems(
            OnEnter(Phase::Ready),
            (reset_flow, debug_boost.after(super::reset_run)),
        )
        .add_systems(OnEnter(Phase::Playing), restore_camera)
        .add_systems(
            OnEnter(Phase::Results),
            (normal_speed, record_results).chain(),
        )
        .add_systems(
            Update,
            (
                count_rams.run_if(in_state(Phase::Playing)),
                finale_fx.before(WorldStage::Input),
            ),
        );
}

fn reset_flow(mut flow: ResMut<Flow>, mut time: ResMut<Time<Virtual>>) {
    *flow = Flow::default();
    time.set_relative_speed(1.0);
}

/// The debug boost: a bigger starting ball, applied after the run resets.
fn debug_boost(mut ball: ResMut<BallState>) {
    if let Some(r) = debug_radius() {
        ball.radius = r;
        ball.volume = r * r * r;
        info!("azeroll: debug start radius {r}");
    }
}

fn count_rams(mut rams: MessageReader<Ram>, mut flow: ResMut<Flow>) {
    for _ in rams.read() {
        flow.rams += 1;
    }
}

fn normal_speed(mut time: ResMut<Time<Virtual>>) {
    time.set_relative_speed(1.0);
}

/// Free Roll resumes with the camera the player had.
fn restore_camera(mut flow: ResMut<Flow>, mut rig: ResMut<CamRig>) {
    // Resuming from pause mid-finale re-enters Playing with the camera still pulled out.
    if flow.finale.is_some() {
        return;
    }
    if let Some((zoom, tilt)) = flow.cam_before.take() {
        rig.zoom = zoom;
        rig.tilt = tilt;
    }
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Slow motion and the pull-out while the celebration plays.
fn finale_fx(mut flow: ResMut<Flow>, mut virt: ResMut<Time<Virtual>>, mut rig: ResMut<CamRig>) {
    let Some(t) = flow.finale else { return };
    let speed = 1.0 - (1.0 - SLOW_SPEED) * (1.0 - ease((t - SLOW_HOLD) / (SLOW_END - SLOW_HOLD)));
    if (virt.relative_speed() - speed).abs() > 1e-3 {
        virt.set_relative_speed(speed);
    }
    let (zoom0, tilt0) = *flow.cam_before.get_or_insert((rig.zoom, rig.tilt));
    let k = ease(t / PULL_SECS);
    rig.zoom = zoom0 + (PULL_ZOOM - zoom0).max(0.0) * k;
    rig.tilt = tilt0 + (PULL_TILT - tilt0).max(0.0) * k;
}

// ---------------------------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------------------------

/// The run is over: set the records, judge the challenges and save, once, as the results open.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn record_results(
    run: Res<Run>,
    ball: Res<BallState>,
    design: Res<Design>,
    mut flow: ResMut<Flow>,
    mut progress: ResMut<Progress>,
    ground: Query<(&Transform, &Collectible), Without<Attached>>,
    mut sounds: MessageWriter<GlueSound>,
) {
    let before = progress.best();
    let mut now = before.clone();
    let diameter = ball.diameter();
    let mut out = Outcome {
        before: before.clone(),
        ..default()
    };

    // Size and count are records of any timed run; time and challenges need the win.
    if before.biggest.is_none_or(|b| diameter > b + 0.005) {
        now.biggest = Some(diameter);
        out.new_size = true;
    }
    if before.most.is_none_or(|m| run.count > m) {
        now.most = Some(run.count);
        out.new_count = true;
    }
    let met = if run.won {
        now.wins += 1;
        if before.best_time.is_none_or(|b| run.elapsed < b - 0.05) {
            now.best_time = Some(run.elapsed);
            out.new_time = true;
        }
        evaluate(&run, &flow, &design, &ground)
    } else {
        [false; CHALLENGES.len()]
    };
    for (i, (id, title)) in CHALLENGES.iter().enumerate() {
        let earned = before.challenges.iter().any(|c| c == id);
        let fresh = met[i] && !earned;
        if fresh {
            now.challenges.push((*id).to_string());
        }
        out.challenges.push(ChallengeResult {
            title,
            met: met[i],
            fresh,
            earned,
        });
    }

    let record =
        out.new_time || out.new_size || out.new_count || out.challenges.iter().any(|c| c.fresh);
    info!(
        "azeroll: results won={} t={:.1}s size={:.2}yd count={} rams={} new_time={} new_size={} new_count={} challenges={:?}",
        run.won,
        run.elapsed,
        diameter,
        run.count,
        flow.rams,
        out.new_time,
        out.new_size,
        out.new_count,
        met
    );
    progress.saved.levels.insert(LEVEL_ID.to_string(), now);
    progress.save();
    if record {
        sounds.write(GlueSound(RECORD_SOUND));
    }
    flow.outcome = out;
}

/// Which of [`CHALLENGES`] this winning run met, from the log, the rams and what is left on the
/// ground.
fn evaluate(
    run: &Run,
    flow: &Flow,
    design: &Design,
    ground: &Query<(&Transform, &Collectible), Without<Attached>>,
) -> [bool; CHALLENGES.len()] {
    // Every chicken: the whole herd was rolled up, and none is left running about (a shed one is).
    let herd: u32 = design
        .herds
        .iter()
        .filter(|h| h.species == CHICKEN)
        .map(|h| h.count)
        .sum();
    let rolled = |name: &str| run.log.get(name).copied().unwrap_or(0);
    let loose_chickens = ground
        .iter()
        .filter(|(_, c)| c.name.contains(CHICKEN))
        .count();
    let chickens = herd > 0 && rolled(CHICKEN) >= herd && loose_chickens == 0;

    // The supper spill: as many of each of the scene's things as the recipe laid, and none left
    // near the table.
    let supper = match design.scenes.iter().find(|s| s.name == SPILL_SCENE) {
        Some(scene) => {
            let mut want: BTreeMap<String, u32> = BTreeMap::new();
            for item in scene.items.iter().filter(|i| !i.model.is_empty()) {
                *want
                    .entry(super::names::display_name(&item.model))
                    .or_default() += item.count as u32;
            }
            let anchor = units::wow_to_bevy(scene.anchor);
            let left = ground.iter().any(|(tf, c)| {
                c.source == Source::Spawned
                    && want.contains_key(&*c.name)
                    && units::flat(tf.transform_point(c.center) - anchor).length() < SPILL_RADIUS
            });
            !want.is_empty() && !left && want.iter().all(|(name, n)| rolled(name) >= *n)
        }
        None => {
            warn!("azeroll: no scene named {SPILL_SCENE:?}; its challenge cannot be met");
            false
        }
    };

    [chickens, run.elapsed < SPEED_SECS, supper, flow.rams == 0]
}
