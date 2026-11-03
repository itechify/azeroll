//! Azeroll's music, embedded so the native and wasm builds both carry it: "A Wonderful Walk Through
//! Azeroth" for the hub, ready and results screens, and the level playlist for play ("Morning in
//! Goldshire", "Goldshire Mambo!", "Goldshire Go-Go!", "Kuru Kuru Azeroth" and the old title theme).
//! Nothing plays under the boot loading screen. Each level names its songs
//! (`campaign::LevelEntry::songs`) and rotates within them, a retry starting the first again; Free
//! Roll plays "Morning in Goldshire" on any level. The phase picks the track;
//! `sound::mode_music` loops it, crossfades the change and keeps WoW's zone music silent.
//!
//! The one thing on top of that is a brief [`Duck`] under the biggest moments, a tier reached and
//! Hogger rolled up, so their fanfare is heard. It dips the gain the pause menu also uses and always
//! lets it back up; tracks and playlist are not touched.

use bevy::prelude::*;

use super::campaign::{CurrentLevel, LEVELS};
use super::progress::Flow;
use super::ritual::Ritual;
use super::{Collected, GoalReached, Kind, Phase, Run, TierReached};
use crate::sound::{ModeMusic, ModeTrack};

/// The hub's theme: the level select, a level's ready screen and the plain results.
const HUB: ModeTrack = ModeTrack {
    name: "a-wonderful-walk-through-azeroth",
    bytes: include_bytes!("../../../../assets/music/a-wonderful-walk-through-azeroth.mp3"),
    volume: 0.8,
};

/// The level playlist: each track with its length in seconds (`ffprobe`), so the next one can be
/// faded in just before the current one would loop.
const PLAYLIST: &[(ModeTrack, f32)] = &[
    (
        ModeTrack {
            name: "morning-in-goldshire",
            bytes: include_bytes!("../../../../assets/music/morning-in-goldshire.mp3"),
            volume: 0.8,
        },
        179.2,
    ),
    (
        ModeTrack {
            name: "goldshire-mambo",
            bytes: include_bytes!("../../../../assets/music/goldshire-mambo.mp3"),
            volume: 0.8,
        },
        208.6,
    ),
    (
        ModeTrack {
            name: "goldshire-go-go",
            bytes: include_bytes!("../../../../assets/music/goldshire-go-go.mp3"),
            volume: 0.8,
        },
        179.2,
    ),
    (
        ModeTrack {
            name: "kuru-kuru-azeroth",
            bytes: include_bytes!("../../../../assets/music/kuru-kuru-azeroth.mp3"),
            volume: 0.8,
        },
        178.8,
    ),
    (
        ModeTrack {
            name: "azeroll-title",
            bytes: include_bytes!("../../../../assets/music/azeroll-title.mp3"),
            volume: 0.8,
        },
        149.4,
    ),
];

/// Seconds before a track's end at which the next one takes over (the mixer's crossfade).
const HANDOFF: f32 = 1.0;

/// How far the pause menu ducks the music.
const PAUSED_GAIN: f32 = 0.3;

/// How a big moment dips the music: the fraction taken off the gain, the seconds it is held, then
/// let back up over [`DUCK_RELEASE`]. A tier's flourish is 4.9 s long but its first beats carry it;
/// Hogger's lasts the finale (3.2 s) and the results screen resets it.
const TIER_DUCK: (f32, f32) = (0.35, 1.3);
const HOGGER_DUCK: (f32, f32) = (0.55, 2.4);
/// The send-off: the Hogger depth, held for its length (the results hold the music at half gain).
const RITUAL_DUCK: (f32, f32) = (0.55, 3.4);
/// The level track under the results.
const RESULTS_GAIN: f32 = 0.5;
/// Free Roll's song, on every level.
const FREE_ROLL_SONG: &str = "morning-in-goldshire";
/// Seconds to reach the dip, and to come back from it.
const DUCK_ATTACK: f32 = 0.12;
const DUCK_RELEASE: f32 = 0.9;

/// The dip under the music: `held` is how much is taken off right now, easing toward `depth` while
/// `hold` seconds remain and back to nothing afterwards.
#[derive(Resource, Default, Debug)]
struct Duck {
    depth: f32,
    hold: f32,
    held: f32,
}

impl Duck {
    /// A new moment: at least this deep, for at least this long.
    fn hit(&mut self, (depth, hold): (f32, f32)) {
        self.depth = self.depth.max(depth);
        self.hold = self.hold.max(hold);
    }

    /// Advance by `dt` real seconds.
    fn step(&mut self, dt: f32) {
        if self.hold > 0.0 {
            self.hold -= dt;
            let k = (dt / DUCK_ATTACK).min(1.0);
            self.held += (self.depth - self.held) * k;
        } else {
            self.held = (self.held - self.depth.max(0.05) * dt / DUCK_RELEASE).max(0.0);
            if self.held == 0.0 {
                self.depth = 0.0;
            }
        }
    }

    /// Whether it is dipping at all.
    fn active(&self) -> bool {
        self.hold > 0.0 || self.held > 0.0
    }
}

/// Where the level playlist is: which track, how long it has played, and which track the next run
/// opens with.
#[derive(Resource, Default)]
struct Playlist {
    index: usize,
    played: f32,
    next_run: usize,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Playlist>()
        .init_resource::<Duck>()
        .add_systems(OnEnter(Phase::Ready), (next_run, clear_duck))
        .add_systems(OnEnter(Phase::Results), (clear_duck, ritual_duck).chain())
        .add_systems(Update, (duck_moments, pick_track).chain());
}

/// A new run, or the results screen, starts with the music at full: nothing carries over.
fn clear_duck(mut duck: ResMut<Duck>) {
    if duck.active() {
        info!("azeroll music: duck cleared ({:?})", *duck);
    }
    *duck = Duck::default();
}

/// The send-off ducks the music for its length.
fn ritual_duck(mut duck: ResMut<Duck>) {
    info!(
        "azeroll music: duck {:.0}% for {:.1}s under the send-off",
        RITUAL_DUCK.0 * 100.0,
        RITUAL_DUCK.1
    );
    duck.hit(RITUAL_DUCK);
}

/// Dip the music under a tier reached and under Hogger, and step the dip along. Logged at info
/// since the audio can't be heard on the build host. A tier that lands with the win is left to the
/// finale's own dip.
fn duck_moments(
    flow: Res<super::progress::Flow>,
    mut was_finale: Local<bool>,
    time: Res<Time<Real>>,
    run: Res<Run>,
    mut tiers: MessageReader<TierReached>,
    mut goals: MessageReader<GoalReached>,
    mut collected: MessageReader<Collected>,
    mut duck: ResMut<Duck>,
) {
    for _ in goals.read() {
        info!("azeroll music: duck under the goal beat");
        duck.hit(TIER_DUCK);
    }
    for t in tiers.read() {
        if !run.won {
            info!(
                "azeroll music: duck {:.0}% for {:.1}s under tier {} {:?}",
                TIER_DUCK.0 * 100.0,
                TIER_DUCK.1,
                t.tier,
                t.name
            );
            duck.hit(TIER_DUCK);
        }
    }
    for c in collected.read() {
        if c.kind == Kind::Boss {
            info!(
                "azeroll music: duck {:.0}% for {:.1}s under {}",
                HOGGER_DUCK.0 * 100.0,
                HOGGER_DUCK.1,
                c.name
            );
            duck.hit(HOGGER_DUCK);
        }
    }
    let finale = flow.finale.is_some();
    if finale && !*was_finale {
        duck.hit(HOGGER_DUCK);
    }
    *was_finale = finale;
    let was = duck.active();
    duck.step(time.delta_secs());
    if was && !duck.active() {
        info!("azeroll music: duck released, gain back to full");
    }
}

/// The playlist positions of a level's songs, in its order; names the playlist does not have are
/// skipped.
fn songs_of(names: &[&str]) -> Vec<usize> {
    names
        .iter()
        .filter_map(|n| PLAYLIST.iter().position(|(t, _)| t.name == *n))
        .collect()
}

/// The song after `index` within a level's own list; `None` when the list is empty (the global
/// rotation applies).
fn next_song(songs: &[usize], index: usize) -> Option<usize> {
    let at = songs.iter().position(|&s| s == index)?;
    Some(songs[(at + 1) % songs.len()])
}

/// A new run opens the playlist on the level's first song, always: a retry does not advance it. A
/// level with no songs gets the global rotation, one track further on than the last run did.
fn next_run(mut playlist: ResMut<Playlist>, current: Res<CurrentLevel>) {
    let songs = songs_of(LEVELS[current.0].songs);
    playlist.index = match songs.first() {
        Some(&first) => first,
        None => {
            let i = playlist.next_run % PLAYLIST.len();
            playlist.next_run += 1;
            i
        }
    };
    playlist.played = 0.0;
}

/// The boot's silence: the loading screen is not up on the very first frames, so the music waits
/// for the first cover to rise and drop. A boot that never raises one gets music after
/// [`BOOT_GRACE`] seconds.
#[derive(Default)]
struct Boot {
    seen_cover: bool,
    waited: f32,
    done: bool,
}

const BOOT_GRACE: f32 = 20.0;

impl Boot {
    /// Advance by one frame; whether music may play.
    fn step(&mut self, covering: bool, dt: f32) -> bool {
        if !self.done {
            self.seen_cover |= covering;
            self.waited += dt;
            self.done =
                (self.seen_cover && !covering) || (!self.seen_cover && self.waited >= BOOT_GRACE);
        }
        self.done
    }
}

#[allow(clippy::too_many_arguments)]
fn pick_track(
    duck: Res<Duck>,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    flow: Res<Flow>,
    ritual: Res<Ritual>,
    current: Res<CurrentLevel>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut boot: Local<Boot>,
    mut playlist: ResMut<Playlist>,
    mut music: ResMut<ModeMusic>,
) {
    // Silence under the boot loading screen; the hub's theme starts once that first cover drops.
    if !boot.step(loading.covering(), time.delta_secs()) {
        if music.track.is_some() {
            music.track = None;
        }
        return;
    }
    let (track, gain) = match phase.get() {
        Phase::Select | Phase::Ready => (HUB, 1.0),
        // The send-off and the results stay in the level, under half the music.
        Phase::Results if ritual.active() => (PLAYLIST[playlist.index].0, RESULTS_GAIN),
        Phase::Results => (HUB, 1.0),
        Phase::Playing | Phase::Paused => {
            // The track keeps sounding (ducked) under the pause menu, so its clock keeps running.
            playlist.played += time.delta_secs();
            let songs = songs_of(LEVELS[current.0].songs);
            if flow.free_roll {
                if let Some(free) = PLAYLIST.iter().position(|(t, _)| t.name == FREE_ROLL_SONG) {
                    if playlist.index != free {
                        playlist.index = free;
                        playlist.played = 0.0;
                    }
                }
            } else if playlist.played >= PLAYLIST[playlist.index].1 - HANDOFF {
                playlist.index = match (songs.len(), next_song(&songs, playlist.index)) {
                    // One song: it loops, as the mixer loops a track left in place.
                    (1, _) => playlist.index,
                    (_, Some(next)) => next,
                    (_, None) => (playlist.index + 1) % PLAYLIST.len(),
                };
                playlist.played = 0.0;
            }
            let gain = if *phase.get() == Phase::Paused {
                PAUSED_GAIN
            } else {
                1.0
            };
            (PLAYLIST[playlist.index].0, gain)
        }
    };
    if music.track.map(|t| t.name) != Some(track.name) {
        music.track = Some(track);
    }
    let gain = gain * (1.0 - duck.held);
    if music.gain != gain {
        music.gain = gain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Step a duck at 60 fps until `secs` have passed, returning the deepest dip seen.
    fn run(duck: &mut Duck, secs: f32) -> f32 {
        let mut deepest = 0.0_f32;
        for _ in 0..(secs * 60.0) as usize {
            duck.step(1.0 / 60.0);
            deepest = deepest.max(duck.held);
        }
        deepest
    }

    #[test]
    fn the_boot_is_silent_until_its_first_cover_drops() {
        let mut boot = Boot::default();
        assert!(!boot.step(false, 0.1), "before the cover rises");
        assert!(!boot.step(true, 60.0), "under the cover, however long");
        assert!(boot.step(false, 0.1), "the world is shown");
        assert!(boot.step(true, 0.1), "a later load keeps its music");
        let mut never = Boot::default();
        assert!(!never.step(false, BOOT_GRACE - 1.0));
        assert!(
            never.step(false, 1.0),
            "no cover at all: music after the grace"
        );
    }

    #[test]
    fn a_tier_duck_dips_holds_and_is_gone_afterwards() {
        let mut duck = Duck::default();
        duck.hit(TIER_DUCK);
        let deepest = run(&mut duck, TIER_DUCK.1 + DUCK_RELEASE + 0.3);
        assert!((deepest - TIER_DUCK.0).abs() < 0.02, "dip {deepest}");
        assert!(!duck.active(), "still ducked: {duck:?}");
        assert_eq!(duck.held, 0.0);
        assert_eq!(duck.depth, 0.0, "the depth must not outlive the dip");
    }

    #[test]
    fn the_music_is_back_at_full_after_hogger() {
        let mut duck = Duck::default();
        duck.hit(HOGGER_DUCK);
        let secs = HOGGER_DUCK.1 + DUCK_RELEASE + 0.2;
        run(&mut duck, secs);
        assert_eq!(1.0 - duck.held, 1.0);
    }

    #[test]
    fn a_deeper_moment_deepens_the_dip_and_a_lighter_one_does_not_lift_it() {
        let mut duck = Duck::default();
        duck.hit(TIER_DUCK);
        run(&mut duck, 0.5);
        duck.hit(HOGGER_DUCK);
        let deepest = run(&mut duck, 0.6);
        assert!(deepest > TIER_DUCK.0 + 0.1);
        duck.hit(TIER_DUCK);
        assert_eq!(duck.depth, HOGGER_DUCK.0);
    }

    #[test]
    fn each_level_plays_its_songs_in_order_and_a_retry_restarts_the_first() {
        let names: Vec<&str> = PLAYLIST.iter().map(|(t, _)| t.name).collect();
        for entry in LEVELS {
            let songs = songs_of(entry.songs);
            assert_eq!(songs.len(), entry.songs.len(), "{}: unknown song", entry.id);
            // Rotating through the list comes back to its first song.
            let mut at = songs[0];
            for _ in 0..songs.len() {
                at = next_song(&songs, at).unwrap();
            }
            assert_eq!(at, songs[0], "{}", entry.id);
        }
        // The L3 handoff goes kuru-kuru -> morning -> kuru-kuru.
        let l3 = songs_of(LEVELS[2].songs);
        assert_eq!(names[l3[0]], "kuru-kuru-azeroth");
        assert_eq!(
            names[next_song(&l3, l3[0]).unwrap()],
            "morning-in-goldshire"
        );
        assert!(names.contains(&FREE_ROLL_SONG));
    }

    #[test]
    fn the_send_off_ducks_for_its_length_and_leaves_the_results_audible() {
        assert_eq!(RITUAL_DUCK.1, super::super::ritual::DOCK_AT);
        assert!(1.0 - RITUAL_DUCK.0 >= 0.4);
        assert!(RESULTS_GAIN > 0.3);
    }

    #[test]
    fn the_dip_never_silences_the_music() {
        // The deepest dip leaves at least 40% of the gain, and a pause on top still leaves some.
        assert!(1.0 - HOGGER_DUCK.0 >= 0.4);
        assert!(PAUSED_GAIN * (1.0 - HOGGER_DUCK.0) > 0.1);
    }
}
