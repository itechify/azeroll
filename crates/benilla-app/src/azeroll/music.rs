//! Azeroll's music, embedded so the native and wasm builds both carry it: the title theme for the
//! ready and results screens, and the Goldshire playlist for play ("Morning in Goldshire",
//! "Goldshire Mambo!" and "Goldshire Go-Go!" in turn, each run opening on the next track). The
//! phase picks the track; `sound::mode_music` loops it, crossfades the change and keeps WoW's zone
//! music silent.
//!
//! The one thing on top of that is a brief [`Duck`] under the biggest moments, a tier reached and
//! Hogger rolled up, so their fanfare is heard. It dips the gain the pause menu also uses and always
//! lets it back up; tracks and playlist are not touched.

use bevy::prelude::*;

use super::{Collected, Kind, Phase, Run, TierReached};
use crate::sound::{ModeMusic, ModeTrack};

const TITLE: ModeTrack = ModeTrack {
    name: "azeroll-title",
    bytes: include_bytes!("../../../../assets/music/azeroll-title.mp3"),
    volume: 0.8,
};

/// The level playlist: each track with its length in seconds (`ffprobe`), so the next one can be
/// faded in just before the current one would loop.
const GOLDSHIRE: &[(ModeTrack, f32)] = &[
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
        .add_systems(OnEnter(Phase::Results), clear_duck)
        .add_systems(Update, (duck_moments, pick_track).chain());
}

/// A new run, or the results screen, starts with the music at full: nothing carries over.
fn clear_duck(mut duck: ResMut<Duck>) {
    if duck.active() {
        info!("azeroll music: duck cleared ({:?})", *duck);
    }
    *duck = Duck::default();
}

/// Dip the music under a tier reached and under Hogger, and step the dip along. Logged at info
/// since the audio can't be heard on the build host. A tier that lands with the win is left to the
/// finale's own dip.
fn duck_moments(
    time: Res<Time<Real>>,
    run: Res<Run>,
    mut tiers: MessageReader<TierReached>,
    mut collected: MessageReader<Collected>,
    mut duck: ResMut<Duck>,
) {
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
    let was = duck.active();
    duck.step(time.delta_secs());
    if was && !duck.active() {
        info!("azeroll music: duck released, gain back to full");
    }
}

/// A new run opens the playlist one track further on than the last run did.
fn next_run(mut playlist: ResMut<Playlist>) {
    playlist.index = playlist.next_run % GOLDSHIRE.len();
    playlist.next_run += 1;
    playlist.played = 0.0;
}

fn pick_track(
    duck: Res<Duck>,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut playlist: ResMut<Playlist>,
    mut music: ResMut<ModeMusic>,
) {
    let (track, gain) = match phase.get() {
        Phase::Ready | Phase::Results => (TITLE, 1.0),
        Phase::Playing | Phase::Paused => {
            // The track keeps sounding (ducked) under the pause menu, so its clock keeps running.
            playlist.played += time.delta_secs();
            if playlist.played >= GOLDSHIRE[playlist.index].1 - HANDOFF {
                playlist.index = (playlist.index + 1) % GOLDSHIRE.len();
                playlist.played = 0.0;
            }
            let gain = if *phase.get() == Phase::Paused {
                PAUSED_GAIN
            } else {
                1.0
            };
            (GOLDSHIRE[playlist.index].0, gain)
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
    fn the_dip_never_silences_the_music() {
        // The deepest dip leaves at least 40% of the gain, and a pause on top still leaves some.
        assert!(1.0 - HOGGER_DUCK.0 >= 0.4);
        assert!(PAUSED_GAIN * (1.0 - HOGGER_DUCK.0) > 0.1);
    }
}
