//! The campaign: the ordered level registry, which level is current, and the hub's state. Only the
//! embedding lives in Rust ([`LEVELS`]); everything else about a level (title, briefing, goal,
//! unlock rule, claim zones) is recipe data, read through [`Campaign`]. D21 in ARCHITECTURE.md.

use bevy::prelude::*;

use super::design::{ClaimZone, LevelMeta};
use super::progress::Progress;
use super::{units, Phase};

/// One registered level: its recipe, and the NPC cast file it brings (if any). Both are
/// `include_str!`'d so the browser build needs no file access.
pub(super) struct LevelEntry {
    pub id: &'static str,
    pub recipe: &'static str,
    pub cast: Option<&'static str>,
    /// Where the level is on the map: WoW (x north, y west) of the place that names it. The hub's
    /// Elwynn inset puts its pin there.
    pub pin: [f32; 2],
    /// The commissioner's one-line request, typed first in the speech bubble (the recipe's
    /// `briefing` follows it in smaller type). Kept here so the recipes stay the level designers'.
    pub quip: &'static str,
    /// The level's songs (names in `music::PLAYLIST`), in the order they play; with more than one
    /// the level rotates within them. A retry restarts the first (D31, audit #11).
    pub songs: &'static [&'static str],
    /// The level's big named thing, which earns Fizzwick's landmark line when rolled up.
    pub landmark: &'static str,
}

/// The levels in hub order.
pub(super) const LEVELS: &[LevelEntry] = &[
    LevelEntry {
        id: "lions-pride",
        recipe: include_str!("lions-pride.toml"),
        cast: Some(include_str!("lions-pride-npcs.toml")),
        // The Lion's Pride Inn, Goldshire.
        pin: [-9467.0, 36.0],
        quip: "A mouse ate my fuses! Be the mouse. Be BETTER than the mouse!",
        // The band is still playing after the regiment has gone.
        songs: &["goldshire-mambo", "azeroll-title"],
        landmark: "Captain's Table",
    },
    LevelEntry {
        id: "market-day",
        recipe: include_str!("goldshire.toml"),
        cast: Some(include_str!("goldshire-npcs.toml")),
        // The middle of the fair's ground: stalls, farms and the Riverpaw camp's forest.
        pin: [-9620.0, 210.0],
        quip: "Everything at the fair is for sale, and I have bought YOU. Go and collect!",
        // The fair's bustle and the chase.
        songs: &["goldshire-go-go", "azeroll-title"],
        landmark: "Hogger",
    },
    LevelEntry {
        id: "stormwind",
        recipe: include_str!("stormwind.toml"),
        cast: Some(include_str!("stormwind-npcs.toml")),
        // The Cathedral of Light, Stormwind.
        pin: [-8520.0, 880.0],
        quip: "The Cathedral has not RSVP'd. Deliver the invitation. It is rather large.",
        // The finale's scale, then a calm second track so a 30 minute candle does not loop one.
        songs: &["kuru-kuru-azeroth", "morning-in-goldshire", "azeroll-title"],
        landmark: "House",
    },
];

/// The level a bare `WOW_AZEROLL_AUTO` run (and every old tooling line) plays.
pub(super) const DEFAULT_LEVEL: &str = "market-day";

/// Where the camera orbits behind the level-select hub: the Lion's Pride Inn's yard (WoW
/// coordinates). Always loaded: the first two levels start beside it.
pub(super) const HUB_VIEW: [f32; 3] = [-9450.0, 60.0, 57.0];

pub(super) fn index_of(id: &str) -> Option<usize> {
    LEVELS.iter().position(|l| l.id == id)
}

/// The level being played (or about to be): an index into [`LEVELS`].
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CurrentLevel(pub usize);

impl CurrentLevel {
    pub fn entry(&self) -> &'static LevelEntry {
        &LEVELS[self.0]
    }
    pub fn id(&self) -> &'static str {
        LEVELS[self.0].id
    }
}

/// What the hub is showing. Pure state: the widgets in [`super::select`] read it and nothing else
/// about the hub, so the presentation can be replaced wholesale.
#[derive(Resource, Default, Debug)]
pub(super) struct SelectState {
    /// Index into [`LEVELS`].
    pub highlighted: usize,
    /// "Free Roll" was asked for from the hub: start it as soon as the chosen level has loaded.
    pub free_pending: bool,
    /// Why the last key press did nothing ("locked"), shown for a moment.
    pub note: Option<(String, f32)>,
    /// What the pointer asked for since the last frame; `select::keys` acts on it like a key.
    pub request: Option<Request>,
}

/// A pointer's wish on the hub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Request {
    /// Swing this level to the front.
    Highlight(usize),
    /// Roll (or Free Roll) the highlighted level.
    Play { free: bool },
}

/// The recipe facts the hub and the claim install need for every level, parsed once.
pub(super) struct LevelInfo {
    pub meta: LevelMeta,
    pub claims: Vec<ClaimZone>,
    /// The challenges' titles, in recipe order (the hub's mission card lists them).
    pub challenge_titles: Vec<String>,
    /// And their ids, which the saves record.
    pub challenge_ids: Vec<String>,
    /// The ball's diameter at the start, the goal's, and the time limit: the card's scale ring
    /// and candle.
    pub start_diameter: f32,
    pub goal_diameter: f32,
    pub time_limit: f32,
    /// Species and names that read as proper nouns when a star is named after them.
    pub proper: Vec<String>,
    /// The registry's map pin and quip.
    pub pin: [f32; 2],
    pub quip: &'static str,
    /// Every unique display name the recipe's scenes, herds and cast can produce, with its
    /// `names::Category` index: the Catalogue's denominator for this level (the map's own doodads
    /// and the city's pieces are not in it).
    pub catalogue: Vec<(String, u8)>,
    /// The parcel's reward and the cousin (D33), if the level hides them.
    pub present: Option<super::replay::PresentInfo>,
    pub cousin: Option<super::replay::CousinInfo>,
}

/// The display names a recipe and its cast can lay out, sorted and unique, each with its category.
pub(super) fn catalogue_names(
    design: &super::design::Design,
    cast: Option<&str>,
) -> Vec<(String, u8)> {
    use super::names::{category, Category};
    use super::Kind;
    let mut names: Vec<(String, u8)> = Vec::new();
    for item in design.scenes.iter().flat_map(|s| &s.items) {
        for model in std::iter::once(&item.model).chain(&item.models) {
            if !model.is_empty() {
                let n = item.label(model);
                let c = category(&n, Kind::Prop, false) as u8;
                names.push((n, c));
            }
        }
        if !item.species.is_empty() {
            names.push((item.species.clone(), Category::Critters as u8));
        }
    }
    for h in &design.herds {
        let c = if h.species == design.goal.target {
            Category::Folk
        } else {
            Category::Critters
        };
        names.push((h.species.clone(), c as u8));
    }
    if let Some(cast) = cast.and_then(|c| toml::from_str::<toml::Value>(c).ok()) {
        if let Some(npcs) = cast.get("npc").and_then(|n| n.as_array()) {
            names.extend(
                npcs.iter()
                    .filter_map(|n| n.get("name")?.as_str())
                    .map(|n| (n.to_owned(), Category::Folk as u8)),
            );
        }
    }
    names.retain(|(n, _)| !n.is_empty());
    names.sort();
    names.dedup_by(|a, b| a.0 == b.0);
    names
}

#[derive(Resource)]
pub(super) struct Campaign {
    pub levels: Vec<LevelInfo>,
}

impl Campaign {
    /// Parses every embedded recipe (the native `WOW_AZEROLL_LEVEL` override replaces the boot
    /// level's, so its claim zones count too).
    pub fn load(boot: usize) -> Self {
        let levels = LEVELS
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let text = if i == boot {
                    over_ride().unwrap_or_else(|| entry.recipe.to_owned())
                } else {
                    entry.recipe.to_owned()
                };
                let d = super::design::parse(&text)
                    .or_else(|e| {
                        error!("azeroll campaign: {} does not parse: {e}", entry.id);
                        super::design::parse(entry.recipe)
                    })
                    .unwrap_or_else(|e| panic!("embedded level {}: {e}", entry.id));
                LevelInfo {
                    claims: d.claim_zones(),
                    challenge_titles: d.challenges.iter().map(|c| c.title.clone()).collect(),
                    challenge_ids: d.challenges.iter().map(|c| c.id.clone()).collect(),
                    start_diameter: 2.0 * d.start_radius,
                    goal_diameter: d.goal_diameter(),
                    time_limit: d.time_limit,
                    pin: entry.pin,
                    quip: entry.quip,
                    catalogue: catalogue_names(&d, entry.cast),
                    present: d.present.as_ref().map(|p| super::replay::PresentInfo {
                        id: p.id.clone(),
                        title: p.title.clone(),
                        display: p.display,
                    }),
                    cousin: super::replay::cousin_in_cast(entry.cast),
                    proper: std::iter::once(d.goal.target.clone())
                        .chain(d.roster.iter().map(|s| s.name.to_string()))
                        .filter(|s| !s.is_empty())
                        .collect(),
                    meta: LevelMeta {
                        id: entry.id.to_owned(),
                        ..d.level
                    },
                }
            })
            .collect();
        Self { levels }
    }

    /// Whether the hub lets the player into level `ix`: it has no gate, its gate level has been
    /// won, or the player already won it (a save from before the gate existed).
    pub fn unlocked(&self, ix: usize, progress: &Progress) -> bool {
        unlocked(&self.levels[ix].meta, progress)
    }

    /// The title of every level that winning `id` just opened, for the results line.
    pub fn opened_by(&self, id: &str, before_wins: &dyn Fn(&str) -> u32) -> Vec<&str> {
        self.levels
            .iter()
            .filter(|l| l.meta.unlock_after.as_deref() == Some(id))
            .filter(|l| before_wins(id) == 0 && before_wins(&l.meta.id) == 0)
            .map(|l| l.meta.title.as_str())
            .collect()
    }
}

/// The unlock rule on its own, for tests.
pub(super) fn unlocked(meta: &LevelMeta, progress: &Progress) -> bool {
    match &meta.unlock_after {
        None => true,
        Some(gate) => progress.wins(gate) > 0 || progress.wins(&meta.id) > 0,
    }
}

fn over_ride() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(std::env::var("WOW_AZEROLL_LEVEL").ok()?).ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// The level and phase the app boots into. `WOW_AZEROLL_LEVEL_ID=<id>` (the `?...` query on the
/// page) goes straight to that level's Ready; a bare `WOW_AZEROLL_AUTO` plays [`DEFAULT_LEVEL`];
/// otherwise the hub.
pub(super) fn boot() -> (CurrentLevel, Phase) {
    let default = index_of(DEFAULT_LEVEL).unwrap_or(0);
    match crate::webenv::var("WOW_AZEROLL_LEVEL_ID") {
        Some(id) => match index_of(&id) {
            Some(ix) => (CurrentLevel(ix), Phase::Ready),
            None => {
                warn!("azeroll: WOW_AZEROLL_LEVEL_ID={id:?} is not a level; opening the hub");
                (CurrentLevel(default), Phase::Select)
            }
        },
        None if crate::webenv::var("WOW_AZEROLL_AUTO").is_some() => {
            (CurrentLevel(default), Phase::Ready)
        }
        None => (CurrentLevel(default), Phase::Select),
    }
}

pub(super) fn plugin(app: &mut App, boot: CurrentLevel) {
    super::buildings::plugin(app);
    app.insert_resource(boot)
        .insert_resource(Campaign::load(boot.0))
        .init_resource::<SelectState>()
        .add_systems(OnEnter(Phase::Select), enter_select);
}

/// Back at the hub: the last run's things go, the clock and Free Roll reset, and the ball waits at
/// the hub's view point with the camera orbiting it. The level is chosen from here.
pub(super) fn enter_select(
    mut commands: Commands,
    spawned: Query<Entity, With<super::RunEntity>>,
    mut run: ResMut<super::Run>,
    mut ball: ResMut<super::BallState>,
    mut select: ResMut<SelectState>,
    campaign: Res<Campaign>,
    progress: Res<Progress>,
    current: Res<CurrentLevel>,
) {
    for e in &spawned {
        commands.entity(e).despawn();
    }
    let next_guid = run.next_guid;
    *run = super::Run {
        next_guid,
        ..default()
    };
    *ball = super::BallState::default();
    ball.pos = units::wow_to_bevy(HUB_VIEW);
    // Open on the level the player was in, else the first one they can play and have not won.
    let was = current.0;
    select.free_pending = false;
    select.highlighted = if campaign.unlocked(was, &progress) {
        was
    } else {
        (0..LEVELS.len())
            .find(|&i| campaign.unlocked(i, &progress))
            .unwrap_or(0)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, gate: Option<&str>) -> LevelMeta {
        LevelMeta {
            id: id.into(),
            title: id.into(),
            unlock_after: gate.map(Into::into),
            ..default()
        }
    }

    #[test]
    fn every_embedded_recipe_parses_and_names_itself() {
        let campaign = Campaign::load(usize::MAX);
        assert_eq!(campaign.levels.len(), LEVELS.len());
        for (info, entry) in campaign.levels.iter().zip(LEVELS) {
            assert_eq!(info.meta.id, entry.id);
            assert!(!info.meta.title.is_empty(), "{}", entry.id);
            assert!(!info.meta.objective.is_empty(), "{}", entry.id);
            assert!(!info.claims.is_empty());
        }
        // The recipe's own [level] id agrees with the registry.
        for entry in LEVELS {
            let d = super::super::design::parse(entry.recipe).unwrap();
            assert_eq!(d.level.id, entry.id);
        }
    }

    #[test]
    fn every_level_has_a_catalogue_to_fill() {
        let campaign = Campaign::load(usize::MAX);
        for (info, entry) in campaign.levels.iter().zip(LEVELS) {
            // Stormwind lays no scenes of its own: its city is the map's.
            if entry.id != "stormwind" {
                assert!(
                    info.catalogue.len() > 5,
                    "{}: {:?}",
                    entry.id,
                    info.catalogue
                );
            }
            let mut sorted: Vec<&String> = info.catalogue.iter().map(|(n, _)| n).collect();
            sorted.dedup();
            assert_eq!(sorted.len(), info.catalogue.len());
        }
        // The inn's cast is in its catalogue.
        assert!(campaign.levels[0]
            .catalogue
            .iter()
            .any(|(n, _)| n == "Innkeeper Farley"));
    }

    #[test]
    fn every_level_has_songs_and_a_landmark() {
        for entry in LEVELS {
            assert!(!entry.songs.is_empty(), "{}", entry.id);
            assert!(!entry.landmark.is_empty(), "{}", entry.id);
        }
    }

    #[test]
    fn levels_unlock_in_order_and_grandfather_old_wins() {
        let lions = meta("lions-pride", None);
        let market = meta("market-day", Some("lions-pride"));
        let mut p = Progress::for_tests();
        assert!(unlocked(&lions, &p));
        assert!(!unlocked(&market, &p));
        p.record_win("lions-pride");
        assert!(unlocked(&market, &p));
        // A player who won market-day before the gate existed keeps it.
        let mut old = Progress::for_tests();
        old.record_win("market-day");
        assert!(unlocked(&market, &old));
    }

    #[test]
    fn the_shipped_chain_is_linear_and_acyclic() {
        let campaign = Campaign::load(usize::MAX);
        for (i, info) in campaign.levels.iter().enumerate() {
            if let Some(gate) = &info.meta.unlock_after {
                let at = index_of(gate).unwrap_or_else(|| panic!("{gate} is not a level"));
                assert!(at < i, "{} is gated behind a later level", info.meta.id);
            }
        }
        assert!(campaign.levels[0].meta.unlock_after.is_none());
    }

    #[test]
    fn winning_names_what_it_opens() {
        let campaign = Campaign::load(usize::MAX);
        let none = |_: &str| 0;
        assert_eq!(
            campaign.opened_by("lions-pride", &none),
            vec![campaign.levels[1].meta.title.as_str()]
        );
        // Already beaten once: nothing is new.
        let beaten = |id: &str| u32::from(id == "lions-pride");
        assert!(campaign.opened_by("lions-pride", &beaten).is_empty());
    }
}
