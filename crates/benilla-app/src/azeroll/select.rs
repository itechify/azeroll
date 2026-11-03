//! The level-select hub: the Gnomish Reclamation Hub (D24). The pieces are kept apart so the
//! presentation can be replaced without touching the rest:
//!
//! * state: [`SelectState`] (`campaign.rs`), changed only by [`keys`];
//! * view model: [`view`] turns the registry, the recipes' `[level]` data, the player's records and
//!   the state into plain [`CardView`] strings and flags, with no UI types and unit tests;
//! * memory: [`HubMemory`] and [`Celebration`] remember what the hub last showed, so a win or an
//!   unlock that happened since is played once (the star lighting, the lock opening);
//! * widgets: [`hub`] draws the hub (the scene, the tags, the mission card, the commissioner's
//!   line) from the views; [`verdict`] is the results screen that leads back to it.

mod catalogue;
pub(crate) mod hub;
mod mapart;
mod maps;
mod verdict;

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::campaign::{Campaign, CurrentLevel, Request, SelectState, LEVELS};
use super::hud::{fmt_size, fmt_time};
use super::progress::{star_title, Grade, LevelBest, Progress};
use super::Phase;
use crate::loading_screen::LoadingScreen;

/// `WOW_AZEROLL_UI_ASPECT=1.78` (a capture aid): lay the screens out for that aspect whatever the
/// window's, at the matching UI scale, so a wide layout can be captured in a window the compositor
/// will not make wide. The UI then fills the top-left of the window.
pub(super) fn capture_aspect() -> Option<f32> {
    crate::webenv::var("WOW_AZEROLL_UI_ASPECT")?.parse().ok()
}

/// The aspect the screens are laid out for.
pub(super) fn aspect_of(window: &Window) -> f32 {
    capture_aspect().unwrap_or_else(|| window.width() / window.height().max(1.0))
}

/// How long a refusal ("locked") stays up, seconds.
const NOTE_SECS: f32 = 2.5;

// ---------------------------------------------------------------------------------------------
// The view model
// ---------------------------------------------------------------------------------------------

/// Where a level stands for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Standing {
    /// Behind a gate level that has not been won.
    Locked,
    /// Open, never won.
    Open,
    /// Won at least once.
    Won,
}

/// What one level says. Plain data: any presentation can draw it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CardView {
    /// "I", "II", "III".
    pub numeral: String,
    pub title: String,
    pub tagline: String,
    pub standing: Standing,
    /// "NEW REQUEST", "Finish X to unlock", "Won x2".
    pub status: String,
    pub objective: String,
    /// The commissioner's line.
    pub briefing: String,
    /// The ball's diameter at the start and the goal's, yards: the scale ring.
    pub start_diameter: f32,
    pub goal_diameter: f32,
    /// "from 8 in to 1 yd 18 in".
    pub scale_text: String,
    /// The time limit, seconds, and as the candle labels it.
    pub time_limit: f32,
    pub limit_text: String,
    /// The records, label and value, in order; empty before the first run.
    pub records: Vec<(&'static str, String)>,
    /// The best verdict, and whether the best time beat par (the comet).
    pub grade: Option<Grade>,
    pub comet: bool,
    /// The level's star, named for its best find; only a won level has one.
    pub star: Option<String>,
    /// The challenges: title, met.
    pub challenges: Vec<(String, bool)>,
    /// Free Roll is open (the level was won).
    pub free_roll: bool,
    /// The parcel and the cousin found (D33); `None` when the level hides none.
    pub parcel: Option<bool>,
    pub cousin: Option<bool>,
    pub highlighted: bool,
}

impl CardView {
    pub fn locked(&self) -> bool {
        self.standing == Standing::Locked
    }
    pub fn won(&self) -> bool {
        self.standing == Standing::Won
    }
}

/// "I", "II", "III", … for a level's place in the campaign.
pub(super) fn numeral(n: usize) -> String {
    const TABLE: [&str; 6] = ["I", "II", "III", "IV", "V", "VI"];
    TABLE
        .get(n)
        .map_or_else(|| (n + 1).to_string(), |s| (*s).to_string())
}

/// The candle's label: minutes and seconds of the limit.
pub(super) fn limit_label(secs: f32) -> String {
    let s = secs.round().max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The records of one level, as label and value, once there is something to show.
pub(super) fn records(best: &LevelBest) -> Vec<(&'static str, String)> {
    if best.best_time.is_none() && best.most.is_none() {
        return Vec::new();
    }
    vec![
        ("Best time", best.best_time.map_or("--".into(), fmt_time)),
        ("Biggest", best.biggest.map_or("--".into(), fmt_size)),
        (
            "Most rolled up",
            best.most.map_or("--".into(), |n| n.to_string()),
        ),
        ("Wins", best.wins.to_string()),
    ]
}

/// Every card, in registry order.
pub(super) fn view(campaign: &Campaign, progress: &Progress, state: &SelectState) -> Vec<CardView> {
    campaign
        .levels
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let meta = &info.meta;
            let best = progress.best(&meta.id);
            let standing = if !campaign.unlocked(i, progress) {
                Standing::Locked
            } else if best.wins > 0 {
                Standing::Won
            } else {
                Standing::Open
            };
            let status = match standing {
                Standing::Locked => {
                    let gate = meta
                        .unlock_after
                        .as_deref()
                        .and_then(|id| campaign.levels.iter().find(|l| l.meta.id == id))
                        .map_or("the previous request", |l| l.meta.title.as_str());
                    format!("Finish \"{gate}\" to unlock")
                }
                Standing::Open => "NEW REQUEST".to_string(),
                Standing::Won if best.wins == 1 => "Fulfilled".to_string(),
                Standing::Won => format!("Fulfilled x{}", best.wins),
            };
            let proper: Vec<&str> = info.proper.iter().map(String::as_str).collect();
            let star = (standing == Standing::Won)
                .then(|| best.find.as_deref().map(|f| star_title(f, &proper)))
                .flatten();
            let par = meta
                .par
                .unwrap_or(info.time_limit * super::progress::DEFAULT_PAR_SHARE);
            CardView {
                numeral: numeral(i),
                title: meta.title.clone(),
                tagline: meta.subtitle.clone(),
                standing,
                status,
                objective: meta.objective.clone(),
                briefing: meta.briefing.clone(),
                start_diameter: info.start_diameter,
                goal_diameter: info.goal_diameter,
                scale_text: format!(
                    "from {} to {}",
                    fmt_size(info.start_diameter),
                    fmt_size(info.goal_diameter)
                ),
                time_limit: info.time_limit,
                limit_text: limit_label(info.time_limit),
                records: records(&best),
                grade: best.best_grade(),
                comet: best.best_time.is_some_and(|t| t < par),
                star,
                challenges: info
                    .challenge_titles
                    .iter()
                    .zip(&info.challenge_ids)
                    .map(|(title, id)| (title.clone(), best.challenges.contains(id)))
                    .collect(),
                free_roll: best.wins > 0,
                parcel: info.present.as_ref().map(|_| best.parcel),
                cousin: info.cousin.as_ref().map(|_| best.cousin),
                highlighted: i == state.highlighted,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// What the hub remembers
// ---------------------------------------------------------------------------------------------

/// What the hub last showed: wins per level, and which levels were open. A difference at the next
/// visit is news.
#[derive(Resource, Default, Clone)]
pub(super) struct HubMemory {
    wins: BTreeMap<String, u32>,
    open: Vec<bool>,
    parcels: Vec<bool>,
    cousins: Vec<bool>,
}

/// News the hub has to play on arrival: levels whose star is new (a first win since the last
/// visit) and levels that just opened. Set when the hub opens, read by the hub's animation.
#[derive(Resource, Default, Debug, PartialEq)]
pub(super) struct Celebration {
    pub stars: Vec<usize>,
    pub unlocks: Vec<usize>,
    /// Levels whose parcel or cousin was found since the last visit (D33), and whether that
    /// completed the set.
    pub parcels: Vec<usize>,
    pub cousins: Vec<usize>,
    pub all_parcels: bool,
    pub all_cousins: bool,
}

impl Celebration {
    /// Fizzwick's cheer for the news, if there is any of the replay kind: the completions win over
    /// a single find, a parcel over a cousin.
    pub fn replay_line(&self) -> Option<&'static str> {
        use super::lines::{NEWS_ALL_COUSINS, NEWS_ALL_PARCELS, NEWS_COUSIN, NEWS_PARCEL};
        if self.all_parcels {
            Some(NEWS_ALL_PARCELS)
        } else if self.all_cousins {
            Some(NEWS_ALL_COUSINS)
        } else if !self.parcels.is_empty() {
            Some(NEWS_PARCEL)
        } else if !self.cousins.is_empty() {
            Some(NEWS_COUSIN)
        } else {
            None
        }
    }
}

impl HubMemory {
    fn of(campaign: &Campaign, progress: &Progress) -> Self {
        Self {
            wins: campaign
                .levels
                .iter()
                .map(|l| (l.meta.id.clone(), progress.wins(&l.meta.id)))
                .collect(),
            open: (0..campaign.levels.len())
                .map(|i| campaign.unlocked(i, progress))
                .collect(),
            parcels: campaign
                .levels
                .iter()
                .map(|l| l.present.is_some() && progress.best(&l.meta.id).parcel)
                .collect(),
            cousins: campaign
                .levels
                .iter()
                .map(|l| l.cousin.is_some() && progress.best(&l.meta.id).cousin)
                .collect(),
        }
    }

    /// The news since this memory was made; this memory becomes `now`.
    fn news(&mut self, now: &Self) -> Celebration {
        let mut news = Celebration::default();
        for (ix, open) in now.open.iter().enumerate() {
            if *open && !self.open.get(ix).copied().unwrap_or(false) {
                news.unlocks.push(ix);
            }
        }
        // `wins` is keyed by id, so find each level's index through the registry.
        for (ix, entry) in LEVELS.iter().enumerate() {
            let before = self.wins.get(entry.id).copied().unwrap_or(0);
            let after = now.wins.get(entry.id).copied().unwrap_or(0);
            if before == 0 && after > 0 {
                news.stars.push(ix);
            }
        }
        let found = |before: &[bool], after: &[bool]| -> Vec<usize> {
            after
                .iter()
                .enumerate()
                .filter(|(i, now)| **now && !before.get(*i).copied().unwrap_or(false))
                .map(|(i, _)| i)
                .collect()
        };
        news.parcels = found(&self.parcels, &now.parcels);
        news.cousins = found(&self.cousins, &now.cousins);
        news.all_parcels = !news.parcels.is_empty() && now.parcels.iter().all(|p| *p);
        news.all_cousins = !news.cousins.is_empty() && now.cousins.iter().all(|c| *c);
        *self = now.clone();
        news
    }
}

fn seed_memory(mut commands: Commands, campaign: Res<Campaign>, progress: Res<Progress>) {
    let mut memory = HubMemory::of(&campaign, &progress);
    // A capture aid: pretend this level (and what it opens) is news, so the hub plays the star and
    // the unlock on arrival.
    if let Some(id) = crate::webenv::var("WOW_AZEROLL_HUB_CELEBRATE") {
        if let Some(ix) = super::campaign::index_of(&id) {
            memory.wins.insert(id, 0);
            // And any parcel or cousin already in the save is news too.
            memory.parcels.iter_mut().for_each(|p| *p = false);
            memory.cousins.iter_mut().for_each(|c| *c = false);
            if let Some(slot) = memory.open.get_mut(ix + 1) {
                *slot = false;
            }
        }
    }
    commands.insert_resource(memory);
    commands.init_resource::<Celebration>();
}

/// Arriving at the hub: what is new since it was last seen.
fn remember(
    campaign: Res<Campaign>,
    progress: Res<Progress>,
    mut memory: ResMut<HubMemory>,
    mut news: ResMut<Celebration>,
) {
    let now = HubMemory::of(&campaign, &progress);
    *news = memory.news(&now);
    if *news != Celebration::default() {
        info!("azeroll hub: news {:?}", *news);
    }
}

// ---------------------------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------------------------

/// The next highlight, one step `forward` or back, wrapping.
pub(super) fn step(highlighted: usize, n: usize, forward: bool) -> usize {
    if forward {
        (highlighted + 1) % n
    } else {
        (highlighted + n - 1) % n
    }
}

/// What Enter or F does on a level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Decision {
    Locked,
    NeedsWin,
    Go { free: bool },
}

pub(super) fn decide(unlocked: bool, wins: u32, free: bool) -> Decision {
    if !unlocked {
        Decision::Locked
    } else if free && wins == 0 {
        Decision::NeedsWin
    } else {
        Decision::Go { free }
    }
}

/// W/A/Up/Left and S/D/Down/Right move; Enter or Space rolls the highlighted level; F rolls it as
/// an untimed Free Roll when it has been won. The pointer's wishes arrive as a [`Request`].
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    campaign: Res<Campaign>,
    progress: Res<Progress>,
    mut state: ResMut<SelectState>,
    mut current: ResMut<CurrentLevel>,
    mut next: ResMut<NextState<Phase>>,
    catalogue: Res<catalogue::CatalogueView>,
) {
    if let Some((_, left)) = state.note.as_mut() {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            state.note = None;
        }
    }
    // The Catalogue page takes the keys while it is open.
    if *phase.get() != Phase::Select || loading.covering() || catalogue.open {
        state.request = None;
        return;
    }
    let n = LEVELS.len();
    let mut play = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]);
    let mut free = keys.just_pressed(KeyCode::KeyF);
    if keys.any_just_pressed([
        KeyCode::KeyW,
        KeyCode::KeyA,
        KeyCode::ArrowUp,
        KeyCode::ArrowLeft,
    ]) {
        state.highlighted = step(state.highlighted, n, false);
    }
    if keys.any_just_pressed([
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::ArrowDown,
        KeyCode::ArrowRight,
    ]) {
        state.highlighted = step(state.highlighted, n, true);
    }
    match state.request.take() {
        Some(Request::Highlight(ix)) if ix < n => state.highlighted = ix,
        Some(Request::Play { free: f }) => {
            play |= !f;
            free |= f;
        }
        _ => {}
    }
    if !(play || free) {
        return;
    }
    let ix = state.highlighted;
    let meta = &campaign.levels[ix].meta;
    match decide(
        campaign.unlocked(ix, &progress),
        progress.wins(&meta.id),
        free,
    ) {
        Decision::Locked => {
            state.note = Some((
                format!("{} is locked: finish the request before it.", meta.title),
                NOTE_SECS,
            ));
        }
        Decision::NeedsWin => {
            state.note = Some((
                format!("Fulfil \"{}\" once to unlock its Free Roll.", meta.title),
                NOTE_SECS,
            ));
        }
        Decision::Go { free } => {
            info!(
                "azeroll hub: {} {}",
                if free { "free roll" } else { "play" },
                meta.id
            );
            state.free_pending = free;
            *current = CurrentLevel(ix);
            next.set(Phase::Ready);
        }
    }
}

/// `WOW_AZEROLL_HUB_SHOT=<file.png>` (a capture aid): save the hub a few seconds after it is up,
/// then exit. `WOW_AZEROLL_HUB_KEYS=sdd` first presses those keys (w/a/s/d to move, e to roll, f to free roll) so a card other than the
/// first is highlighted; `WOW_AZEROLL_HUB_SHOT_AT=<secs>` moves the capture (default 3.5), so an
/// animation can be caught part way.
fn hub_shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    mut state: ResMut<SelectState>,
    mut shown: Local<f32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = crate::webenv::var("WOW_AZEROLL_HUB_SHOT") else {
        return;
    };
    if *phase.get() != Phase::Select || loading.covering() {
        return;
    }
    let at: f32 = crate::webenv::var("WOW_AZEROLL_HUB_SHOT_AT")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3.5);
    let before = *shown;
    *shown += time.delta_secs();
    if before < 0.5 && *shown >= 0.5 {
        for key in crate::webenv::var("WOW_AZEROLL_HUB_KEYS")
            .unwrap_or_default()
            .chars()
        {
            let n = LEVELS.len();
            match key {
                's' | 'd' => state.highlighted = step(state.highlighted, n, true),
                'w' | 'a' => state.highlighted = step(state.highlighted, n, false),
                'e' => state.request = Some(Request::Play { free: false }),
                'f' => state.request = Some(Request::Play { free: true }),
                _ => {}
            }
        }
    }
    if before < at && *shown >= at {
        use bevy::render::view::screenshot::{save_to_disk, Screenshot};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::PathBuf::from(path)));
    }
    if *shown >= at + 1.5 {
        exit.write(AppExit::Success);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((hub::plugin, verdict::plugin, catalogue::plugin))
        .add_systems(Startup, seed_memory)
        .add_systems(OnEnter(Phase::Select), remember)
        .add_systems(Update, (keys, hub_shot).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_lists_every_level_in_order_with_lock_states() {
        let campaign = Campaign::load(usize::MAX);
        let mut progress = Progress::for_tests();
        let state = SelectState::default();
        let cards = view(&campaign, &progress, &state);
        assert_eq!(cards.len(), LEVELS.len());
        assert_eq!(cards[0].title, campaign.levels[0].meta.title);
        assert!(!cards[0].locked() && cards[0].highlighted);
        assert_eq!(cards[0].status, "NEW REQUEST");
        assert!(cards[1].locked() && cards[2].locked());
        assert!(cards[1].status.starts_with("Finish "));
        assert!(cards[1].status.contains(&cards[0].title));
        assert!(cards
            .iter()
            .all(|c| !c.briefing.is_empty() && !c.objective.is_empty()));
        assert!(cards[0].records.is_empty() && cards[0].star.is_none() && !cards[0].free_roll);
        assert_eq!(cards[0].numeral, "I");
        assert_eq!(cards[2].numeral, "III");

        progress.record_win("lions-pride");
        let cards = view(&campaign, &progress, &state);
        assert!(cards[0].won() && cards[0].status == "Fulfilled" && cards[0].free_roll);
        assert!(!cards[1].locked() && cards[2].locked());
    }

    #[test]
    fn a_won_level_carries_its_star_grade_and_comet() {
        let campaign = Campaign::load(usize::MAX);
        let mut progress = Progress::for_tests();
        let par = campaign.levels[0]
            .meta
            .par
            .unwrap_or(campaign.levels[0].time_limit * super::super::progress::DEFAULT_PAR_SHARE);
        progress.set_best(
            "lions-pride",
            LevelBest {
                wins: 2,
                best_time: Some(par - 1.0),
                biggest: Some(1.8),
                most: Some(120),
                grade: Some(Grade::Splendid as u8),
                find: Some("Cheese Wheel".into()),
                find_size: Some(0.6),
                challenges: vec![],
                ..default()
            },
        );
        let cards = view(&campaign, &progress, &SelectState::default());
        let c = &cards[0];
        assert_eq!(c.grade, Some(Grade::Splendid));
        assert!(c.comet);
        assert_eq!(c.star.as_deref(), Some("The Cheese Wheel"));
        assert_eq!(c.status, "Fulfilled x2");
        assert_eq!(c.records.len(), 4);
        assert_eq!(c.records[2], ("Most rolled up", "120".to_string()));
        // Slower than par: no comet.
        progress.set_best(
            "lions-pride",
            LevelBest {
                wins: 1,
                best_time: Some(par + 5.0),
                ..default()
            },
        );
        assert!(!view(&campaign, &progress, &SelectState::default())[0].comet);
    }

    #[test]
    fn records_read_cleanly() {
        assert!(records(&LevelBest::default()).is_empty());
        let best = LevelBest {
            best_time: Some(301.5),
            biggest: Some(8.7),
            most: Some(412),
            wins: 2,
            ..default()
        };
        let rows = records(&best);
        assert_eq!(rows[0], ("Best time", "05:01.5".to_string()));
        assert_eq!(rows[2].1, "412");
        assert_eq!(rows[3].1, "2");
    }

    #[test]
    fn the_candle_and_numerals_label_themselves() {
        assert_eq!(limit_label(300.0), "5:00");
        assert_eq!(limit_label(620.0), "10:20");
        assert_eq!(numeral(0), "I");
        assert_eq!(numeral(5), "VI");
        assert_eq!(numeral(8), "9");
    }

    #[test]
    fn moving_wraps_and_actions_follow_the_locks() {
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(decide(false, 0, false), Decision::Locked);
        assert_eq!(decide(false, 0, true), Decision::Locked);
        assert_eq!(decide(true, 0, true), Decision::NeedsWin);
        assert_eq!(decide(true, 0, false), Decision::Go { free: false });
        assert_eq!(decide(true, 3, true), Decision::Go { free: true });
    }

    #[test]
    fn a_find_since_the_last_visit_is_news_and_the_whole_set_is_a_completion() {
        let campaign = Campaign::load(usize::MAX);
        let mut progress = Progress::for_tests();
        let mut memory = HubMemory::of(&campaign, &progress);
        let found = |parcel, cousin| LevelBest {
            parcel,
            cousin,
            ..default()
        };
        progress.set_best("lions-pride", found(true, false));
        let news = memory.news(&HubMemory::of(&campaign, &progress));
        assert_eq!(news.parcels, vec![0]);
        assert!(news.cousins.is_empty());
        assert_eq!(news.replay_line(), Some(crate::azeroll::lines::NEWS_PARCEL));
        // A level whose set is not whole is not a completion, and the news is told once.
        assert!(!news.all_parcels);
        assert_eq!(
            memory.news(&HubMemory::of(&campaign, &progress)),
            Celebration::default()
        );
        // A cousin is news of its own kind, and a completion when every level hides one found.
        progress.set_best("lions-pride", found(true, true));
        let news = memory.news(&HubMemory::of(&campaign, &progress));
        assert_eq!(news.cousins, vec![0]);
        assert_eq!(news.replay_line(), Some(crate::azeroll::lines::NEWS_COUSIN));
        // One find among several hiding cousins is not yet a completion.
        let hiding: Vec<_> = campaign
            .levels
            .iter()
            .filter(|l| l.cousin.is_some())
            .map(|l| l.meta.id.clone())
            .collect();
        assert_eq!(news.all_cousins, hiding.len() == 1);
        // Finding the rest makes the completion.
        for id in hiding.iter().filter(|id| id.as_str() != "lions-pride") {
            progress.set_best(id, found(false, true));
        }
        let news = memory.news(&HubMemory::of(&campaign, &progress));
        assert!(news.all_cousins);
        assert_eq!(
            news.replay_line(),
            Some(crate::azeroll::lines::NEWS_ALL_COUSINS)
        );
        // Completions win over a single find.
        let all = Celebration {
            all_parcels: true,
            parcels: vec![0],
            ..default()
        };
        assert_eq!(
            all.replay_line(),
            Some(crate::azeroll::lines::NEWS_ALL_PARCELS)
        );
    }

    #[test]
    fn the_card_shows_what_the_level_hides_and_what_was_found() {
        let campaign = Campaign::load(usize::MAX);
        let mut progress = Progress::for_tests();
        let state = SelectState::default();
        let card = |p: &Progress| view(&campaign, p, &state).remove(0);
        let c = card(&progress);
        assert_eq!(c.parcel, campaign.levels[0].present.as_ref().map(|_| false));
        assert_eq!(c.cousin, campaign.levels[0].cousin.as_ref().map(|_| false));
        progress.set_best(
            "lions-pride",
            LevelBest {
                parcel: true,
                ..default()
            },
        );
        let c = card(&progress);
        assert_eq!(c.parcel, campaign.levels[0].present.as_ref().map(|_| true));
    }

    #[test]
    fn news_is_a_first_win_or_a_new_opening_and_is_told_once() {
        let campaign = Campaign::load(usize::MAX);
        let mut progress = Progress::for_tests();
        let mut memory = HubMemory::of(&campaign, &progress);
        assert_eq!(
            memory.news(&HubMemory::of(&campaign, &progress)),
            Celebration::default()
        );
        progress.record_win("lions-pride");
        let now = HubMemory::of(&campaign, &progress);
        let news = memory.news(&now);
        assert_eq!(news.stars, vec![0]);
        assert_eq!(news.unlocks, vec![1]);
        // Told once.
        assert_eq!(memory.news(&now), Celebration::default());
        // A second win of the same level is not news.
        progress.record_win("lions-pride");
        assert_eq!(
            memory.news(&HubMemory::of(&campaign, &progress)),
            Celebration::default()
        );
    }
}
