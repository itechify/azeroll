//! Replayability: the finale's clock and camera, the bests that survive a restart, the optional
//! challenges, and the untimed Free Roll. [`Flow`] is the per-run state the level flow and the HUD
//! share; [`Progress`] is the persisted side, written through [`crate::local_state`] when a run ends
//! (a file natively, `localStorage` on the page, nothing when `$WOW_CAPTURE` is set).
//!
//! The finale itself is level flow (`level::tick` starts it on the goal and ends it into the
//! results); this module only dresses it: slow motion through the virtual clock and a wider, higher
//! camera through [`CamRig`], both undone on the way out. Records and challenges are per level id
//! (`campaign::CurrentLevel`); the challenges themselves are recipe data (`design::ChallengeSpec`).

use std::collections::BTreeMap;

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::camera::CamRig;
use super::campaign::{Campaign, CurrentLevel};
use super::collect::Ram;
use super::design::{ChallengeKind, ChallengeSpec, Design};
use std::borrow::Cow;

use super::{units, Attached, BallState, Collected, Collectible, Find, Phase, Run, Source};

/// The id this game's first level was saved under, before there was a campaign.
const LEGACY_LEVEL_ID: &str = "goldshire";
/// The level that save belongs to now.
const LEGACY_LEVEL_NOW: &str = "market-day";

/// Real seconds from capturing a single-target goal to the send-off: the Cathedral's flight is 3.2 s
/// (`finale.rs`), a creature goal's slow-motion pull-out 2.0 s. A size goal has none: it keeps
/// rolling to the candle.
pub(super) const FINALE_SECS: f32 = 3.2;
pub(super) const COLLECT_FINALE_SECS: f32 = 2.0;
/// The celebration's fanfare: a SoundEntries kit name. (The record's chime and the star's pop are
/// `ritual`'s.)
pub(super) const FANFARE_SOUND: &str = "QUESTCOMPLETED";

/// The time dilation: this speed until `SLOW_HOLD`, then easing back to 1 by `SLOW_END`.
const SLOW_SPEED: f32 = 0.2;
const SLOW_HOLD: f32 = 0.9;
const SLOW_END: f32 = 2.6;
/// The camera pulls out over `PULL_SECS`: this much more boom, this much more tilt.
const PULL_SECS: f32 = 1.0;
const PULL_ZOOM: f32 = 1.6;
const PULL_TILT: f32 = 0.3;

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
    /// The goal met this many times.
    #[serde(default)]
    pub wins: u32,
    /// Ids of the recipe's challenges met at least once.
    #[serde(default)]
    pub challenges: Vec<String>,
    /// The best verdict a win has earned, as a [`Grade`] index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade: Option<u8>,
    /// The biggest find of the best-finding win, and its size: the level's star is named after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub find: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub find_size: Option<f32>,
    /// The level's parcel was rolled up at least once (D33).
    #[serde(default, skip_serializing_if = "is_false")]
    pub parcel: bool,
    /// The level's cousin was rolled up at least once, and at least once inside their window.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cousin: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cousin_fair: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// What the Prince wears and who pushes, by key (a recipe's `[present] id` and a cast row's
/// `cousin`). Keys no recipe knows are ignored at resolution, not an error.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub(super) struct WardrobeKeys {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outfit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pusher: Option<String>,
}

impl WardrobeKeys {
    fn is_empty(&self) -> bool {
        self.outfit.is_none() && self.pusher.is_none()
    }
}

impl LevelBest {
    /// The best verdict so far, if the level has been won with one.
    pub fn best_grade(&self) -> Option<Grade> {
        self.grade.and_then(Grade::from_index)
    }
}

// ---------------------------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------------------------

/// How well a win went: four grades, from squeaking by to a ridiculous overshoot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Grade {
    Scraped = 0,
    Respectable = 1,
    Splendid = 2,
    Preposterous = 3,
}

impl Grade {
    pub const ALL: [Grade; 4] = [
        Grade::Scraped,
        Grade::Respectable,
        Grade::Splendid,
        Grade::Preposterous,
    ];

    pub fn from_index(i: u8) -> Option<Self> {
        Self::ALL.get(i as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Grade::Scraped => "Scraped by",
            Grade::Respectable => "Respectable",
            Grade::Splendid => "Splendid",
            Grade::Preposterous => "Preposterous!",
        }
    }

    /// The commissioner's reaction, one line; `roll` picks from the grade's pool of three.
    pub fn remark(self, roll: u64) -> &'static str {
        let pool = super::lines::REMARKS[self as usize];
        pool[(roll % pool.len() as u64) as usize]
    }
}

/// Grade boundaries as multiples of the goal diameter: Respectable, Splendid, Preposterous.
pub(super) const DEFAULT_SIZE_GRADES: [f32; 3] = [1.2, 1.5, 2.0];
/// Grade boundaries as the share of the time limit used (smaller is better): Respectable,
/// Splendid, Preposterous. A run ends on its goal, so a size multiple above 1 only comes from
/// overshoot (a `collect` goal whose target needs less than the ball had); pace is what separates
/// two wins of a size goal, and a win takes whichever grade is higher.
pub(super) const DEFAULT_PACE_GRADES: [f32; 3] = [0.8, 0.6, 0.4];
/// The time-bonus comet: finished within this share of the time limit, unless the recipe sets
/// `par` in seconds.
pub(super) const DEFAULT_PAR_SHARE: f32 = 0.5;

/// The grade for a final diameter `ratio` times the goal's.
pub(super) fn grade_by_size(ratio: f32, bounds: [f32; 3]) -> Grade {
    let beaten = bounds.iter().filter(|&&b| ratio >= b).count();
    Grade::ALL[beaten.min(3)]
}

/// The grade for finishing in `used` of the time limit.
pub(super) fn grade_by_pace(used: f32, bounds: [f32; 3]) -> Grade {
    let beaten = bounds.iter().filter(|&&b| used <= b).count();
    Grade::ALL[beaten.min(3)]
}

/// What a win was judged to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Verdict {
    pub grade: Grade,
    /// Final diameter over the goal's.
    pub size_ratio: f32,
    /// Share of the time limit the win used.
    pub used: f32,
    /// Under par: the time-bonus comet.
    pub comet: bool,
    /// Par, seconds.
    pub par: f32,
}

/// Judge a win: `diameter` against the goal, `elapsed` against the limit, with the recipe's
/// boundaries (or the defaults). A size goal keeps rolling after it is met, so every uncalled run
/// lasts the candle and pace no longer separates two wins: it is graded on the final size alone, and
/// the comet is for reaching the goal under par (`goal_time`). A single-target goal ends on capture
/// and takes the better of size and pace.
pub(super) fn judge_win(
    diameter: f32,
    goal_diameter: f32,
    elapsed: f32,
    limit: f32,
    meta: &super::design::LevelMeta,
    kind: super::design::GoalKind,
    goal_time: Option<f32>,
) -> Verdict {
    let size_ratio = if goal_diameter > 0.0 {
        diameter / goal_diameter
    } else {
        1.0
    };
    let used = if limit > 0.0 { elapsed / limit } else { 1.0 };
    let by_size = grade_by_size(size_ratio, meta.grades.unwrap_or(DEFAULT_SIZE_GRADES));
    let by_pace = grade_by_pace(used, meta.pace.unwrap_or(DEFAULT_PACE_GRADES));
    let par = meta.par.unwrap_or(limit * DEFAULT_PAR_SHARE);
    let size_goal = kind == super::design::GoalKind::Size;
    Verdict {
        grade: if size_goal {
            by_size
        } else {
            by_size.max(by_pace)
        },
        size_ratio,
        used,
        comet: if size_goal {
            goal_time.unwrap_or(elapsed) < par
        } else {
            elapsed < par
        },
        par,
    }
}

/// A level's star, named for the best thing its player rolled up: "The Cheese Wheel". A proper name
/// (the goal's target, a species of the roster) has no article.
pub(super) fn star_title(find: &str, proper: &[&str]) -> String {
    let find = find.trim();
    if find.is_empty() {
        return "The Unnamed Star".to_string();
    }
    let article = find.starts_with("The ")
        || find.contains(['"', '\''])
        || proper.iter().any(|p| p.eq_ignore_ascii_case(find));
    if article {
        find.to_string()
    } else {
        format!("The {find}")
    }
}

/// One name in Fizzwick's Catalogue: every unique display name ever rolled up, kept across runs.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub(super) struct CatalogueEntry {
    /// Level id of the first find.
    pub level: String,
    /// Ever rolled up, all runs.
    pub n: u32,
    /// The biggest axis seen, yards.
    #[serde(default)]
    pub axis: f32,
    /// A `names::Category` index.
    #[serde(default)]
    pub cat: u8,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct SaveFile {
    #[serde(default)]
    levels: BTreeMap<String, LevelBest>,
    /// Absent in saves from before the Catalogue.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    catalogue: BTreeMap<String, CatalogueEntry>,
    /// Absent in saves from before the replay layer.
    #[serde(default, skip_serializing_if = "WardrobeKeys::is_empty")]
    wardrobe: WardrobeKeys,
}

impl SaveFile {
    /// Read a save, moving the pre-campaign `goldshire` record to `market-day`.
    fn parse(text: &str) -> Result<Self, toml::de::Error> {
        let mut file: Self = toml::from_str(text)?;
        if let Some(old) = file.levels.remove(LEGACY_LEVEL_ID) {
            file.levels
                .entry(LEGACY_LEVEL_NOW.to_string())
                .or_insert(old);
        }
        Ok(file)
    }
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
            .and_then(|text| match SaveFile::parse(&text) {
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
    pub fn best(&self, id: &str) -> LevelBest {
        self.saved.levels.get(id).cloned().unwrap_or_default()
    }

    /// Everything ever rolled up, by display name.
    pub fn catalogue(&self) -> &BTreeMap<String, CatalogueEntry> {
        &self.saved.catalogue
    }

    /// What the Prince wears and who pushes.
    pub fn wardrobe(&self) -> WardrobeKeys {
        self.saved.wardrobe.clone()
    }

    /// Dress the Prince (`outfit`) and pick the pusher, then save.
    pub fn set_wardrobe(&mut self, keys: WardrobeKeys) {
        if self.saved.wardrobe != keys {
            self.saved.wardrobe = keys;
            self.save();
        }
    }

    /// Tell the save of this run's parcel and cousin (D33). Idempotent, like the Catalogue merge:
    /// it runs on pause, restart, leaving and the results, so a find is kept whichever way the run
    /// ends. `run.parcel_new` / `run.cousin_new` remember whether the flag was new to the save.
    pub fn merge_replay(&mut self, run: &mut Run, level: &str) {
        let mut changed = false;
        if run.parcel_found && !run.parcel_merged {
            run.parcel_merged = true;
            let best = self.saved.levels.entry(level.to_string()).or_default();
            run.parcel_new = !best.parcel;
            best.parcel = true;
            changed = true;
        }
        if let Some((_, fair)) = run.cousin.clone() {
            if !run.cousin_merged {
                run.cousin_merged = true;
                let best = self.saved.levels.entry(level.to_string()).or_default();
                run.cousin_new = !best.cousin;
                best.cousin = true;
                best.cousin_fair |= fair;
                changed = true;
            }
        }
        if changed {
            self.save();
        }
    }

    /// Times the level's goal has been met.
    pub fn wins(&self, id: &str) -> u32 {
        self.saved.levels.get(id).map_or(0, |b| b.wins)
    }

    /// Tell the Catalogue of every pickup of this run it has not yet heard of. A name's first level
    /// is where it was first found; the names new to the Catalogue go on `run.new_names`.
    /// Idempotent: call it as often as is convenient.
    pub fn merge_catalogue(&mut self, run: &mut Run, level: &str) {
        let mut todo: Vec<(Cow<'static, str>, u32, Find)> = Vec::new();
        for (name, &n) in &run.log {
            let done = run.merged.get(name).copied().unwrap_or(0);
            if n <= done {
                continue;
            }
            let find = run.finds.get(name).copied().unwrap_or(Find {
                axis: 0.0,
                cat: super::names::category(name, super::Kind::Prop, false) as u8,
                seq: u32::MAX,
            });
            todo.push((name.clone(), n - done, find));
        }
        if todo.is_empty() {
            return;
        }
        todo.sort_by_key(|(_, _, f)| f.seq);
        for (name, add, find) in todo {
            run.merged
                .entry(name.clone())
                .and_modify(|d| *d += add)
                .or_insert(add);
            match self.saved.catalogue.get_mut(&*name) {
                Some(e) => {
                    e.n += add;
                    e.axis = e.axis.max(find.axis);
                }
                None => {
                    self.saved.catalogue.insert(
                        name.to_string(),
                        CatalogueEntry {
                            level: level.to_string(),
                            n: add,
                            axis: find.axis,
                            cat: find.cat,
                        },
                    );
                    run.new_names.push(name.to_string());
                }
            }
        }
        self.save();
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            saved: SaveFile::default(),
            persist: false,
        }
    }

    #[cfg(test)]
    pub fn set_best(&mut self, id: &str, best: LevelBest) {
        self.saved.levels.insert(id.to_string(), best);
    }

    #[cfg(test)]
    pub fn record_win(&mut self, id: &str) {
        self.saved.levels.entry(id.to_string()).or_default().wins += 1;
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
    pub title: String,
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
    /// Titles of the levels this win opened for the first time.
    pub unlocked: Vec<String>,
    /// How the win was judged (a win only).
    pub verdict: Option<Verdict>,
    /// The verdict is better than any before.
    pub new_grade: bool,
    /// The biggest thing rolled up, named, with its size across in yards.
    pub find: Option<(String, f32)>,
    /// This win named (or renamed) the level's star.
    pub new_star: bool,
    /// Any record or challenge fell (the results' chime).
    pub record: bool,
    /// The names this run found for the first time ever (the Catalogue), in the order rolled up.
    pub new_catalogue: Vec<String>,
    /// The parcel found this run: its reward's title, and whether it is new to the save.
    pub parcel: Option<(String, bool)>,
    /// The cousin found this run: their name, whether caught inside the window, and whether new.
    pub cousin: Option<(String, bool, bool)>,
}

/// The state of the run beyond [`Run`]: the finale, Free Roll, rams and the results' extras.
#[derive(Resource, Default)]
pub(super) struct Flow {
    /// Real seconds since a single-target goal was captured, while its pre-roll plays.
    pub finale: Option<f32>,
    /// How long that pre-roll lasts (`COLLECT_FINALE_SECS` or `FINALE_SECS`).
    pub finale_len: f32,
    /// How the run ended, set the frame the phase leaves Playing; the ritual takes it from here.
    pub ending: Option<super::ritual::Ending>,
    /// 0..1: how far the finish key has been held (the clock's rim fills with it).
    pub call_hold: f32,
    /// The untimed mode: no clock, no limit, no results, no records.
    pub free_roll: bool,
    /// Rams taken this run.
    pub rams: u32,
    /// Filled when the run ends on the results screen.
    pub outcome: Outcome,
    /// The player's zoom and tilt before the finale took the camera, restored for Free Roll.
    pub cam_before: Option<(f32, f32)>,
}

/// Start Free Roll: the same run, untimed. Used by the results screen, the title, the hub (once
/// its level has loaded) and the autopilot.
pub(super) fn start_free_roll(flow: &mut Flow, next: &mut NextState<Phase>) {
    flow.free_roll = true;
    next.set(Phase::Playing);
}

/// `WOW_AZEROLL_DEBUG_RADIUS=<yards>`: start every run with a ball this big, to reach the goal fast.
fn debug_radius() -> Option<f32> {
    crate::webenv::var("WOW_AZEROLL_DEBUG_RADIUS")?.parse().ok()
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(Progress::load())
        .init_resource::<Flow>()
        .add_systems(
            OnEnter(Phase::Ready),
            (
                merge_finds.before(super::reset_run),
                reset_flow,
                debug_boost.after(super::reset_run),
            ),
        )
        .add_systems(
            OnEnter(Phase::Select),
            (
                merge_finds.before(super::campaign::enter_select),
                reset_flow,
            ),
        )
        .add_systems(OnEnter(Phase::Paused), merge_finds)
        .add_systems(OnEnter(Phase::Playing), restore_camera)
        .add_systems(
            OnEnter(Phase::Results),
            (normal_speed, record_results).chain(),
        )
        .add_systems(
            Update,
            (
                count_rams.run_if(in_state(Phase::Playing)),
                note_finds,
                finale_fx.before(WorldStage::Input),
            ),
        );
}

/// Remember, for the Catalogue, what each name rolled up this frame was: its axis and its category.
fn note_finds(
    mut collected: MessageReader<Collected>,
    mut run: ResMut<Run>,
    npcs: Query<(), With<super::npcs::Npc>>,
) {
    for c in collected.read() {
        let seq = run.finds.len() as u32;
        let t = run.elapsed;
        run.picks.push(super::Picked {
            name: c.name.clone(),
            species: (c.kind != super::Kind::Prop).then(|| c.name.clone()),
            t,
        });
        let cat = super::names::category(&c.name, c.kind, npcs.contains(c.entity)) as u8;
        let f = run.finds.entry(c.name.clone()).or_insert(Find {
            axis: 0.0,
            cat,
            seq,
        });
        f.axis = f.axis.max(c.pickup);
    }
}

/// Tell the Catalogue what has been rolled up so far (idempotent), as a run pauses, restarts or
/// leaves for the hub; the results and Free Roll's exits rely on the same call.
fn merge_finds(mut run: ResMut<Run>, mut progress: ResMut<Progress>, current: Res<CurrentLevel>) {
    if run.log.is_empty() {
        return;
    }
    progress.merge_catalogue(&mut run, current.id());
    progress.merge_replay(&mut run, current.id());
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
fn finale_fx(
    mut flow: ResMut<Flow>,
    mut virt: ResMut<Time<Virtual>>,
    mut rig: ResMut<CamRig>,
    design: Res<Design>,
) {
    let Some(t) = flow.finale else { return };
    let speed = 1.0 - (1.0 - SLOW_SPEED) * (1.0 - ease((t - SLOW_HOLD) / (SLOW_END - SLOW_HOLD)));
    if (virt.relative_speed() - speed).abs() > 1e-3 {
        virt.set_relative_speed(speed);
    }
    let (zoom0, tilt0) = *flow.cam_before.get_or_insert((rig.zoom, rig.tilt));
    // Indoor celebrations keep the playable framing: the outdoor pull places the lens
    // beyond the inn and shrinks its see-through window into a view of the exterior.
    let k = if design.start_indoors {
        0.0
    } else {
        ease(t / PULL_SECS)
    };
    rig.zoom = zoom0 + (PULL_ZOOM - zoom0).max(0.0) * k;
    rig.tilt = tilt0 + (PULL_TILT - tilt0).max(0.0) * k;
}

// ---------------------------------------------------------------------------------------------
// The record
// ---------------------------------------------------------------------------------------------

/// The run is over: set the records, judge the challenges and save, once, as the results open.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn record_results(
    mut run: ResMut<Run>,
    ball: Res<BallState>,
    design: Res<Design>,
    current: Res<CurrentLevel>,
    campaign: Res<Campaign>,
    mut flow: ResMut<Flow>,
    mut progress: ResMut<Progress>,
    ground: Query<(&Transform, &Collectible), Without<Attached>>,
) {
    let id = current.id();
    progress.merge_catalogue(&mut run, id);
    progress.merge_replay(&mut run, id);
    let before = progress.best(id);
    let mut now = before.clone();
    let diameter = run.final_diameter.unwrap_or_else(|| ball.diameter());
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
        // A size goal's best time is the fastest to the goal: that is what a replay beats.
        let reached = run.goal_time.unwrap_or(run.elapsed);
        if before.best_time.is_none_or(|b| reached < b - 0.05) {
            now.best_time = Some(reached);
            out.new_time = true;
        }
        let wins = |level: &str| progress.wins(level);
        out.unlocked = campaign
            .opened_by(id, &wins)
            .into_iter()
            .map(str::to_owned)
            .collect();
        // The ball's own size, not a terminal trophy's: a finale that attaches a 595 yd Cathedral
        // must not make every win of that level Preposterous.
        let verdict = judge_win(
            ball.diameter(),
            design.goal_diameter(),
            run.elapsed,
            design.time_limit,
            &design.level,
            design.goal.kind,
            run.goal_time,
        );
        if before.best_grade().is_none_or(|g| verdict.grade > g) {
            now.grade = Some(verdict.grade as u8);
            out.new_grade = true;
        }
        out.verdict = Some(verdict);
        if let Some((name, size)) = &run.biggest {
            let across = size * 2.0;
            out.find = Some((name.to_string(), across));
            if before.find_size.is_none_or(|b| across > b + 0.005) {
                now.find = Some(name.to_string());
                now.find_size = Some(across);
                out.new_star = true;
            }
        }
        evaluate(&run, &flow, &ball, &design, &ground)
    } else {
        vec![false; design.challenges.len()]
    };
    for (spec, &met) in design.challenges.iter().zip(&met) {
        let earned = before.challenges.iter().any(|c| *c == spec.id);
        let fresh = met && !earned;
        if fresh {
            now.challenges.push(spec.id.clone());
        }
        out.challenges.push(ChallengeResult {
            title: spec.title.clone(),
            met,
            fresh,
            earned,
        });
    }

    let record =
        out.new_time || out.new_size || out.new_count || out.challenges.iter().any(|c| c.fresh);
    info!(
        "azeroll: results level={id} won={} verdict={:?} t={:.1}s size={:.2}yd count={} rams={} new_time={} new_size={} new_count={} challenges={:?} unlocked={:?}",
        run.won,
        out.verdict.map(|v| (v.grade.name(), v.size_ratio, v.used, v.comet)),
        run.elapsed,
        diameter,
        run.count,
        flow.rams,
        out.new_time,
        out.new_size,
        out.new_count,
        met,
        out.unlocked
    );
    progress.saved.levels.insert(id.to_string(), now);
    progress.save();
    out.record = record;
    out.new_catalogue = run.new_names.clone();
    let level = campaign.levels.iter().find(|l| l.meta.id == id);
    if run.parcel_found {
        out.parcel = Some((
            level.and_then(|l| l.present.as_ref()).map_or_else(
                || super::replay::PARCEL_NAME.to_owned(),
                |p| p.title.clone(),
            ),
            run.parcel_new,
        ));
    }
    if let Some((key, fair)) = &run.cousin {
        let name = level
            .and_then(|l| l.cousin.as_ref())
            .filter(|c| c.id == *key)
            .map_or_else(|| key.clone(), |c| c.name.clone());
        out.cousin = Some((name, *fair, run.cousin_new));
    }
    flow.outcome = out;
}

/// What the scene named `name` laid, as `display name -> count`, and where its anchor is.
fn scene_wants(design: &Design, name: &str) -> Option<(BTreeMap<String, u32>, Vec3)> {
    let scene = design.scenes.iter().find(|s| s.name == name)?;
    let mut want: BTreeMap<String, u32> = BTreeMap::new();
    for item in scene.items.iter().filter(|i| !i.model.is_empty()) {
        *want.entry(item.label(&item.model)).or_default() += item.count as u32;
    }
    Some((want, units::wow_to_bevy(scene.anchor)))
}

/// What a `scene` challenge asks for: the scene `scene`, or (with `prefix`) the union of every
/// scene whose name starts with it. The wants add up; every anchor is kept for the "none left" test.
fn scenes_wanted(
    design: &Design,
    spec: &ChallengeSpec,
) -> Option<(BTreeMap<String, u32>, Vec<Vec3>)> {
    if spec.prefix.is_empty() {
        let (want, anchor) = scene_wants(design, &spec.scene)?;
        return Some((want, vec![anchor]));
    }
    let mut want: BTreeMap<String, u32> = BTreeMap::new();
    let mut anchors = Vec::new();
    for scene in design
        .scenes
        .iter()
        .filter(|s| s.name.starts_with(&spec.prefix))
    {
        let (w, anchor) = scene_wants(design, &scene.name)?;
        for (name, n) in w {
            *want.entry(name).or_default() += n;
        }
        anchors.push(anchor);
    }
    (!anchors.is_empty()).then_some((want, anchors))
}

/// How many props of display name `name` the recipe's scenes lay.
fn planned_named(design: &Design, name: &str) -> u32 {
    design
        .scenes
        .iter()
        .flat_map(|s| &s.items)
        .filter(|i| !i.model.is_empty() && i.label(&i.model) == name)
        .map(|i| i.count as u32)
        .sum()
}

/// How many of species `name` the level lays (its herds and the scenes' creature items).
fn planned_species(design: &Design, name: &str) -> u32 {
    design
        .roster
        .iter()
        .filter(|s| s.name == name)
        .map(|s| s.count)
        .sum()
}

/// One for each city piece (`stormwind-pieces.toml`) of this name: a building is a single pickup.
fn planned_piece(name: &str) -> u32 {
    toml::from_str::<toml::Value>(include_str!("stormwind-pieces.toml"))
        .ok()
        .and_then(|v| v.get("parts")?.as_array().cloned())
        .map_or(0, |parts| {
            parts
                .iter()
                .filter(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))
                .count() as u32
        })
}

/// The pickups matching a set: by species (a creature), else by display name.
fn matching<'a>(
    picks: &'a [super::Picked],
    species: &'a str,
    name: &'a str,
) -> impl Iterator<Item = (usize, &'a super::Picked)> + 'a {
    picks.iter().enumerate().filter(move |(_, p)| {
        if !species.is_empty() {
            p.species.is_some() && p.name == species
        } else {
            !name.is_empty() && p.name == name
        }
    })
}

/// `before`: every member of the set was rolled up and the first pickup of the gate came after the
/// last of them (or the gate never came).
pub(super) fn before_met(picks: &[super::Picked], design: &Design, spec: &ChallengeSpec) -> bool {
    let planned = if !spec.species.is_empty() {
        planned_species(design, &spec.species)
    } else {
        planned_named(design, &spec.name)
    };
    let set: Vec<usize> = matching(picks, &spec.species, &spec.name)
        .map(|(i, _)| i)
        .collect();
    if planned == 0 || (set.len() as u32) < planned {
        return false;
    }
    let last = set[planned as usize - 1];
    matching(picks, &spec.before_species, &spec.before_name)
        .next()
        .is_none_or(|(gate, _)| gate > last)
}

/// `within`: the scene's things, each as many as were laid, were all rolled up inside `secs` of
/// each other. Names shared with other scenes are fine: it looks for any window that is enough.
pub(super) fn within_met(picks: &[super::Picked], want: &BTreeMap<String, u32>, secs: f32) -> bool {
    if want.is_empty() {
        return false;
    }
    let cand: Vec<&super::Picked> = picks
        .iter()
        .filter(|p| want.contains_key(&*p.name))
        .collect();
    (0..cand.len()).any(|s| {
        let mut need = want.clone();
        for p in &cand[s..] {
            if p.t - cand[s].t > secs {
                break;
            }
            if let Some(n) = need.get_mut(&*p.name) {
                *n = n.saturating_sub(1);
            }
            if need.values().all(|n| *n == 0) {
                return true;
            }
        }
        false
    })
}

/// Whether any prop of the wanted names still lies near an anchor (rolled up elsewhere, or shed).
fn scene_left(
    want: &BTreeMap<String, u32>,
    anchors: &[Vec3],
    radius: f32,
    ground: &Query<(&Transform, &Collectible), Without<Attached>>,
) -> bool {
    ground.iter().any(|(tf, c)| {
        c.source == Source::Spawned
            && want.contains_key(&*c.name)
            && anchors
                .iter()
                .any(|a| units::flat(tf.transform_point(c.center) - *a).length() < radius)
    })
}

/// Whether one challenge was met by this winning run, from the log, the rams, the final size and
/// what is left on the ground.
fn judge(
    spec: &ChallengeSpec,
    run: &Run,
    flow: &Flow,
    ball: &BallState,
    design: &Design,
    ground: &Query<(&Transform, &Collectible), Without<Attached>>,
) -> bool {
    let rolled = |name: &str| run.log.get(name).copied().unwrap_or(0);
    match spec.kind {
        ChallengeKind::Time => run.goal_time.unwrap_or(run.elapsed) < spec.secs,
        ChallengeKind::Unrammed => flow.rams == 0,
        ChallengeKind::Size => ball.diameter() >= spec.diameter,
        ChallengeKind::Before => before_met(&run.picks, design, spec),
        // The whole herd (or every prop of that model) was rolled up, and none is left lying or
        // running about (a shed one is). `names` and `species_list` ask the same of each member.
        ChallengeKind::CollectAll => {
            let one = |name: String, planned: u32| {
                let loose = ground
                    .iter()
                    .filter(|(_, c)| c.name.contains(&name))
                    .count();
                planned > 0 && rolled(&name) >= planned && loose == 0
            };
            let mut members: Vec<(String, u32)> = Vec::new();
            if !spec.species.is_empty() {
                let herd: u32 = design
                    .herds
                    .iter()
                    .filter(|h| h.species == spec.species)
                    .map(|h| h.count)
                    .sum();
                members.push((spec.species.clone(), herd));
            } else if !spec.model.is_empty() || !spec.name.is_empty() {
                let name = if spec.model.is_empty() {
                    spec.name.clone()
                } else {
                    super::names::display_name(&spec.model)
                };
                let planned = planned_named(design, &name);
                members.push((name, planned));
            }
            for name in &spec.names {
                let planned = planned_named(design, name).max(planned_piece(name));
                members.push((name.clone(), planned));
            }
            for species in &spec.species_list {
                members.push((species.clone(), planned_species(design, species)));
            }
            !members.is_empty()
                && members
                    .into_iter()
                    .all(|(name, planned)| one(name, planned))
        }
        // The scene's things: as many of each as the recipe laid, and none left near its anchor.
        ChallengeKind::Scene => match scenes_wanted(design, spec) {
            Some((want, anchors)) => {
                !want.is_empty()
                    && !scene_left(&want, &anchors, spec.radius, ground)
                    && want.iter().all(|(name, n)| rolled(name) >= *n)
            }
            None => {
                warn!(
                    "azeroll: no scene named {:?}; challenge {:?} cannot be met",
                    spec.scene, spec.id
                );
                false
            }
        },
        // The scene's things, all inside `secs` of each other.
        ChallengeKind::Within => match scenes_wanted(design, spec) {
            Some((want, anchors)) => {
                within_met(&run.picks, &want, spec.secs)
                    && !scene_left(&want, &anchors, spec.radius, ground)
            }
            None => false,
        },
    }
}

/// Which of the recipe's challenges this winning run met, in recipe order.
fn evaluate(
    run: &Run,
    flow: &Flow,
    ball: &BallState,
    design: &Design,
    ground: &Query<(&Transform, &Collectible), Without<Attached>>,
) -> Vec<bool> {
    design
        .challenges
        .iter()
        .map(|spec| judge(spec, run, flow, ball, design, ground))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_goldshire_save_reads_as_market_day() {
        let old = "[levels.goldshire]\nwins = 2\nbest_time = 301.5\nchallenges = [\"speed\"]\n";
        let file = SaveFile::parse(old).unwrap();
        assert!(!file.levels.contains_key("goldshire"));
        let b = &file.levels["market-day"];
        assert_eq!((b.wins, b.best_time), (2, Some(301.5)));
        assert_eq!(b.challenges, vec!["speed".to_string()]);
        // A save that already has both keeps the new one.
        let both = "[levels.goldshire]\nwins = 2\n[levels.market-day]\nwins = 5\n";
        assert_eq!(SaveFile::parse(both).unwrap().levels["market-day"].wins, 5);
        // And a round trip does not bring the old key back.
        let text = toml::to_string(&file).unwrap();
        assert!(!text.contains("goldshire"));
    }

    #[test]
    fn size_grades_step_at_the_multiples() {
        let b = DEFAULT_SIZE_GRADES;
        assert_eq!(grade_by_size(1.0, b), Grade::Scraped);
        assert_eq!(grade_by_size(1.19, b), Grade::Scraped);
        assert_eq!(grade_by_size(1.2, b), Grade::Respectable);
        assert_eq!(grade_by_size(1.49, b), Grade::Respectable);
        assert_eq!(grade_by_size(1.5, b), Grade::Splendid);
        assert_eq!(grade_by_size(1.99, b), Grade::Splendid);
        assert_eq!(grade_by_size(2.0, b), Grade::Preposterous);
        assert_eq!(grade_by_size(40.0, b), Grade::Preposterous);
    }

    #[test]
    fn pace_grades_reward_finishing_early() {
        let b = DEFAULT_PACE_GRADES;
        assert_eq!(grade_by_pace(1.0, b), Grade::Scraped);
        assert_eq!(grade_by_pace(0.81, b), Grade::Scraped);
        assert_eq!(grade_by_pace(0.8, b), Grade::Respectable);
        assert_eq!(grade_by_pace(0.6, b), Grade::Splendid);
        assert_eq!(grade_by_pace(0.4, b), Grade::Preposterous);
        assert_eq!(grade_by_pace(0.0, b), Grade::Preposterous);
    }

    #[test]
    fn a_single_target_verdict_takes_the_better_measure_and_the_recipes_bounds() {
        use super::super::design::GoalKind::Collect;
        let meta = super::super::design::LevelMeta::default();
        // Barely the goal size, near the limit: scraped by, no comet.
        let v = judge_win(1.5, 1.5, 290.0, 300.0, &meta, Collect, None);
        assert_eq!(v.grade, Grade::Scraped);
        assert!(!v.comet);
        // Barely the size, but in 100 s of 300: the pace carries it; and under par (150 s).
        let v = judge_win(1.5, 1.5, 100.0, 300.0, &meta, Collect, None);
        assert_eq!(v.grade, Grade::Preposterous);
        assert!(v.comet && v.par == 150.0);
        // A huge overshoot at a leisurely pace.
        let v = judge_win(3.2, 1.5, 280.0, 300.0, &meta, Collect, None);
        assert_eq!(v.grade, Grade::Preposterous);
        assert!(!v.comet);
        // A recipe's own bounds and par win over the defaults.
        let meta = super::super::design::LevelMeta {
            grades: Some([1.05, 1.1, 1.2]),
            par: Some(60.0),
            ..default()
        };
        let v = judge_win(1.7, 1.5, 200.0, 300.0, &meta, Collect, None);
        assert_eq!(v.grade, Grade::Splendid);
        assert!(!v.comet);
        assert!(judge_win(1.7, 1.5, 59.0, 300.0, &meta, Collect, None).comet);
    }

    #[test]
    fn a_size_goal_is_graded_on_the_final_size_and_the_comet_on_the_goal_time() {
        use super::super::design::GoalKind::Size;
        let meta = super::super::design::LevelMeta::default();
        // Called at exactly the goal: Scraped, whatever the clock said.
        let v = judge_win(2.0, 2.0, 70.0, 300.0, &meta, Size, Some(70.0));
        assert_eq!(v.grade, Grade::Scraped);
        assert!(v.comet, "70 s is under the 150 s par");
        // 2.1x the request at the candle is Preposterous.
        let v = judge_win(4.2, 2.0, 300.0, 300.0, &meta, Size, Some(100.0));
        assert_eq!(v.grade, Grade::Preposterous);
        assert!((v.size_ratio - 2.1).abs() < 1e-4);
        // A late goal is no comet, however big the ball ended.
        let v = judge_win(4.2, 2.0, 300.0, 300.0, &meta, Size, Some(200.0));
        assert!(!v.comet);
        // Pace never lifts a size goal's grade.
        let v = judge_win(2.2, 2.0, 100.0, 300.0, &meta, Size, Some(60.0));
        assert_eq!(v.grade, Grade::Scraped);
        // 1.2x and 1.5x land on their grades.
        assert_eq!(
            judge_win(2.4, 2.0, 300.0, 300.0, &meta, Size, Some(60.0)).grade,
            Grade::Respectable
        );
        assert_eq!(
            judge_win(3.0, 2.0, 300.0, 300.0, &meta, Size, Some(60.0)).grade,
            Grade::Splendid
        );
    }

    #[test]
    fn stars_are_named_for_the_find() {
        assert_eq!(star_title("Cheese Wheel", &["Hogger"]), "The Cheese Wheel");
        assert_eq!(star_title("Hogger", &["Hogger"]), "Hogger");
        assert_eq!(star_title("The Last Stew", &[]), "The Last Stew");
        assert_eq!(star_title("Remy \"Two Times\"", &[]), "Remy \"Two Times\"");
        assert_eq!(star_title("", &[]), "The Unnamed Star");
    }

    #[test]
    fn grades_and_finds_survive_a_save_and_old_saves_still_read() {
        let mut file = SaveFile::default();
        file.levels.insert(
            "lions-pride".into(),
            LevelBest {
                wins: 1,
                grade: Some(Grade::Splendid as u8),
                find: Some("Cheese Wheel".into()),
                find_size: Some(0.6),
                ..default()
            },
        );
        let text = toml::to_string(&file).unwrap();
        let back = SaveFile::parse(&text).unwrap();
        let b = &back.levels["lions-pride"];
        assert_eq!(b.best_grade(), Some(Grade::Splendid));
        assert_eq!(b.find.as_deref(), Some("Cheese Wheel"));
        // A save from before grades has none of the keys.
        let old = "[levels.market-day]\nwins = 3\nbest_time = 250.0\n";
        let b = &SaveFile::parse(old).unwrap().levels["market-day"];
        assert_eq!((b.best_grade(), b.find.clone()), (None, None));
        assert_eq!(b.wins, 3);
    }

    fn run_with(log: &[(&'static str, u32)]) -> Run {
        let mut run = Run::default();
        for (i, (name, n)) in log.iter().enumerate() {
            run.log.insert((*name).into(), *n);
            run.finds.insert(
                (*name).into(),
                Find {
                    axis: 0.5,
                    cat: 0,
                    seq: i as u32,
                },
            );
        }
        run
    }

    #[test]
    fn the_catalogue_remembers_first_levels_and_counts_each_pickup_once() {
        let mut p = Progress::for_tests();
        let mut run = run_with(&[("Muffin", 3), ("Chair", 1)]);
        p.merge_catalogue(&mut run, "lions-pride");
        assert_eq!(
            run.new_names,
            vec!["Muffin".to_string(), "Chair".to_string()]
        );
        assert_eq!(p.catalogue()["Muffin"].n, 3);
        // Merging again (a pause, then the results) adds nothing.
        p.merge_catalogue(&mut run, "lions-pride");
        assert_eq!(p.catalogue()["Muffin"].n, 3);
        // More pickups add the difference.
        run.log.insert("Muffin".into(), 5);
        p.merge_catalogue(&mut run, "lions-pride");
        assert_eq!(p.catalogue()["Muffin"].n, 5);
        assert_eq!(run.new_names.len(), 2, "Muffin was new only once");
        // A later run in another level keeps the first level of a name found twice.
        let mut later = run_with(&[("Muffin", 2), ("Pumpkin", 1)]);
        p.merge_catalogue(&mut later, "market-day");
        assert_eq!(p.catalogue()["Muffin"].level, "lions-pride");
        assert_eq!(p.catalogue()["Muffin"].n, 7);
        assert_eq!(p.catalogue()["Pumpkin"].level, "market-day");
        assert_eq!(later.new_names, vec!["Pumpkin".to_string()]);
    }

    #[test]
    fn an_old_save_without_a_catalogue_loads_and_a_new_one_round_trips() {
        let old = "[levels.lions-pride]\nwins = 1\n";
        let file = SaveFile::parse(old).unwrap();
        assert!(file.catalogue.is_empty());
        // An empty catalogue is not written at all.
        assert!(!toml::to_string(&file).unwrap().contains("catalogue"));
        let mut file = SaveFile::default();
        file.catalogue.insert(
            "Cheese Wheel".into(),
            CatalogueEntry {
                level: "lions-pride".into(),
                n: 12,
                axis: 0.41,
                cat: 0,
            },
        );
        let text = toml::to_string(&file).unwrap();
        assert!(text.contains("[catalogue.\"Cheese Wheel\"]"), "{text}");
        let back = SaveFile::parse(&text).unwrap();
        assert_eq!(
            back.catalogue["Cheese Wheel"],
            file.catalogue["Cheese Wheel"]
        );
    }

    fn pick(name: &'static str, creature: bool, t: f32) -> super::super::Picked {
        super::super::Picked {
            name: name.into(),
            species: creature.then(|| name.into()),
            t,
        }
    }

    fn lions_pride() -> Design {
        let entry = super::super::campaign::LEVELS
            .iter()
            .find(|l| l.id == "lions-pride")
            .unwrap();
        super::super::design::parse(entry.recipe).unwrap()
    }

    fn challenge(d: &Design, id: &str) -> ChallengeSpec {
        d.challenges.iter().find(|c| c.id == id).cloned().unwrap()
    }

    #[test]
    fn before_wants_the_whole_set_first_and_a_gate_that_never_came_counts() {
        let d = lions_pride();
        let rats = challenge(&d, "rats");
        let n = planned_species(&d, "Cellar Rat") as usize;
        assert!(n >= 10, "{n} cellar rats");
        let rat = |t: f32| pick("Cellar Rat", true, t);
        let all: Vec<_> = (0..n).map(|i| rat(i as f32)).collect();
        assert!(before_met(&all, &d, &rats), "the gate never came");
        // The gate after the last rat: met. Before the last rat: not.
        let mut after = all.clone();
        after.push(pick("Cheese Wheel", false, 99.0));
        assert!(before_met(&after, &d, &rats));
        let mut early = all.clone();
        early.insert(n - 1, pick("Cheese Wheel", false, 5.0));
        assert!(!before_met(&early, &d, &rats));
        // One rat short is not the set.
        assert!(!before_met(&all[1..], &d, &rats));
    }

    #[test]
    fn within_wants_every_prop_of_the_scene_inside_a_window_of_seconds() {
        let want: BTreeMap<String, u32> = [("Mug".to_string(), 2), ("Plate".to_string(), 1)].into();
        let at = |m1: f32, p: f32, m2: f32| {
            vec![
                pick("Mug", false, m1),
                pick("Plate", false, p),
                pick("Mug", false, m2),
            ]
        };
        assert!(within_met(&at(0.0, 10.0, 20.0), &want, 20.0));
        assert!(!within_met(&at(0.0, 10.0, 20.5), &want, 20.0));
        // Other pickups in between do not count against it; an earlier stray mug does not help.
        let mut noisy = vec![pick("Mug", false, -100.0)];
        noisy.extend(at(0.0, 10.0, 15.0));
        noisy.insert(2, pick("Pie", false, 12.0));
        assert!(within_met(&noisy, &want, 20.0));
        assert!(!within_met(&[], &want, 20.0));
        assert!(!within_met(&at(0.0, 1.0, 2.0), &BTreeMap::new(), 20.0));
    }

    #[test]
    fn a_prefix_gathers_the_scenes_and_a_name_list_widens_collect_all() {
        let d = lions_pride();
        let spec = ChallengeSpec {
            id: "x".into(),
            title: String::new(),
            kind: ChallengeKind::Scene,
            species: String::new(),
            model: String::new(),
            name: String::new(),
            secs: 0.0,
            scene: String::new(),
            radius: 4.0,
            diameter: 0.0,
            before_species: String::new(),
            before_name: String::new(),
            names: vec![],
            species_list: vec![],
            prefix: "The ".into(),
        };
        let (want, anchors) = scenes_wanted(&d, &spec).unwrap();
        let scenes = d
            .scenes
            .iter()
            .filter(|s| s.name.starts_with("The "))
            .count();
        assert!(scenes >= 3 && anchors.len() == scenes, "{scenes} scenes");
        assert!(want.values().sum::<u32>() > 20);
        // No scene by that prefix: nothing to be met.
        assert!(scenes_wanted(
            &d,
            &ChallengeSpec {
                prefix: "Zzz".into(),
                ..spec
            }
        )
        .is_none());
        // A city piece is one pickup of its own name.
        assert_eq!(planned_piece("Trade District cobbler"), 1);
        assert_eq!(planned_piece("Nothing"), 0);
    }

    #[test]
    fn a_parcel_and_a_cousin_are_kept_once_and_marked_new_only_the_first_time() {
        let mut p = Progress::for_tests();
        let mut run = Run::default();
        run.parcel_found = true;
        run.cousin = Some(("pippa".into(), false));
        p.merge_replay(&mut run, "lions-pride");
        assert!((run.parcel_new, run.cousin_new) == (true, true));
        let b = p.best("lions-pride");
        assert_eq!((b.parcel, b.cousin, b.cousin_fair), (true, true, false));
        // Merging again changes nothing, and does not mark it new twice.
        p.merge_replay(&mut run, "lions-pride");
        assert_eq!(p.best("lions-pride"), b);
        // A later run finding them again is not new, and a fair catch upgrades the flag.
        let mut again = Run::default();
        again.parcel_found = true;
        again.cousin = Some(("pippa".into(), true));
        p.merge_replay(&mut again, "lions-pride");
        assert!(!again.parcel_new && !again.cousin_new);
        assert!(p.best("lions-pride").cousin_fair);
    }

    #[test]
    fn the_replay_fields_and_the_wardrobe_round_trip_and_old_saves_still_read() {
        let mut file = SaveFile::default();
        file.levels.insert(
            "lions-pride".into(),
            LevelBest {
                parcel: true,
                cousin: true,
                cousin_fair: true,
                ..default()
            },
        );
        file.wardrobe = WardrobeKeys {
            outfit: Some("monocle".into()),
            pusher: Some("pippa".into()),
        };
        let text = toml::to_string(&file).unwrap();
        assert!(
            text.contains("[wardrobe]") && text.contains("parcel = true"),
            "{text}"
        );
        let back = SaveFile::parse(&text).unwrap();
        assert_eq!(back.levels["lions-pride"], file.levels["lions-pride"]);
        assert_eq!(back.wardrobe, file.wardrobe);
        // A save from before the replay layer has none of it; an empty wardrobe is not written.
        let old = "[levels.lions-pride]\nwins = 1\n";
        let b = SaveFile::parse(old).unwrap();
        assert_eq!(b.levels["lions-pride"].parcel, false);
        assert_eq!(b.wardrobe, WardrobeKeys::default());
        assert!(!toml::to_string(&b).unwrap().contains("wardrobe"));
        // A wardrobe naming keys no recipe has still parses (it is ignored at resolution).
        let odd = "[wardrobe]\noutfit = \"nonesuch\"\n";
        assert_eq!(
            SaveFile::parse(odd).unwrap().wardrobe.outfit.as_deref(),
            Some("nonesuch")
        );
    }

    #[test]
    fn records_are_per_level() {
        let mut p = Progress::for_tests();
        p.record_win("lions-pride");
        assert_eq!(p.wins("lions-pride"), 1);
        assert_eq!(p.wins("market-day"), 0);
        assert_eq!(p.best("stormwind"), LevelBest::default());
    }
}
