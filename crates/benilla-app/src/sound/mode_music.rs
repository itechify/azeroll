//! Mode-owned music: a game mode (Azeroll) names one embedded track to play and this module keeps
//! it looping, crossfades between tracks, and holds WoW's own zone music silent while any is
//! wanted. Ambience, SFX and the Music slider are the engine's.
//!
//! The mode writes [`ModeMusic`]; [`drive`] is the only thing that touches the mixer. The track's
//! bytes ride in the binary (`include_bytes!` at the mode), so no archive or host file is needed:
//! native decode-streams them like the glue theme, wasm decodes them off the main thread through
//! the browser (`web_load::begin_bytes`) and starts the sound when the PCM lands.

use bevy::prelude::*;

use super::kit::SoundCategory;
use super::{mixer, SoundConfig, SoundOutput};

/// The crossfade between two tracks, both legs, and the fade-in of the first one.
const FADE_MS: u64 = 1000;

/// One embedded track.
#[derive(Clone, Copy)]
pub(crate) struct ModeTrack {
    /// The name in the log, and the track's identity: a new name is a crossfade.
    pub name: &'static str,
    /// An MP3 (or any format kira decodes).
    pub bytes: &'static [u8],
    /// The track's own level, multiplied into the Music slider.
    pub volume: f32,
}

/// What the running mode wants heard. `None` = no mode music, and the zone music plays as usual.
#[derive(Resource)]
pub(crate) struct ModeMusic {
    pub track: Option<ModeTrack>,
    /// A further multiplier on the track, 0..1: the mode ducks it while paused.
    pub gain: f32,
}

impl Default for ModeMusic {
    fn default() -> Self {
        Self {
            track: None,
            gain: 1.0,
        }
    }
}

impl ModeMusic {
    /// A mode track is wanted, so the zone music stays down.
    pub(crate) fn active(&self) -> bool {
        self.track.is_some()
    }
}

type Handle = mixer::StreamingSoundHandle<kira::sound::FromFileError>;

/// A track in the mixer.
struct Playing {
    name: &'static str,
    handle: Handle,
    /// Real seconds the fade-in ends at; the volume feed stands off until then.
    settled_at: f64,
    /// The level last sent to the handle, to feed only on a change.
    sent: f32,
}

#[derive(Default)]
struct ModeMusicState {
    playing: Option<Playing>,
    /// Tracks fading out, until they report `Stopped`.
    fading: Vec<Handle>,
    /// wasm32: the wanted track's decode in flight, with the name it is for.
    #[cfg(target_arch = "wasm32")]
    pending: Option<(&'static str, super::web_load::Pending)>,
    /// The name whose start failed, so a bad track is reported once rather than every frame.
    failed: Option<&'static str>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ModeMusic>()
        .insert_non_send_resource(ModeMusicState::default())
        .add_systems(Update, drive);
}

fn drive(
    want: Res<ModeMusic>,
    config: Res<SoundConfig>,
    real: Res<Time<bevy::time::Real>>,
    mut state: NonSendMut<ModeMusicState>,
    mut out: NonSendMut<SoundOutput>,
) {
    let state = &mut *state;
    let now = real.elapsed_secs_f64();
    state
        .fading
        .retain(|h| h.state() != kira::sound::PlaybackState::Stopped);
    let Some(mixer) = out.mixer.as_mut() else {
        return;
    };

    let wanted = want.track;
    let wanted_name = wanted.map(|t| t.name);
    let live_name = state.playing.as_ref().map(|p| p.name);
    if wanted_name != live_name {
        // The old track leaves under its own fade-out; the handle is kept until it has stopped.
        if let Some(mut old) = state.playing.take() {
            old.handle.stop(mixer::fade(FADE_MS));
            state.fading.push(old.handle);
        }
        #[cfg(target_arch = "wasm32")]
        if state.pending.as_ref().map(|(n, _)| *n) != wanted_name {
            state.pending = None;
        }
        if wanted_name != state.failed {
            state.failed = None;
        }
    }
    let Some(track) = wanted else {
        return;
    };
    let target = config.category_amp(SoundCategory::Music) * track.volume * want.gain;

    if let Some(p) = state.playing.as_mut() {
        if now >= p.settled_at && (p.sent - target).abs() > 1e-3 {
            p.handle
                .set_volume(mixer::amp_to_db(target), mixer::glide());
            p.sent = target;
        }
        if p.handle.state() == kira::sound::PlaybackState::Stopped {
            // Looping, so only a decode failure ends it; start again next frame.
            warn!("mode music: {} stopped", track.name);
            state.playing = None;
        }
        return;
    }
    if state.failed == Some(track.name) {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let data = match mixer::stream_from_bytes(track.bytes.to_vec()) {
        Ok(d) => d,
        Err(e) => {
            warn!("mode music: {} — {e:#}", track.name);
            state.failed = Some(track.name);
            return;
        }
    };
    #[cfg(target_arch = "wasm32")]
    let data = {
        if state.pending.is_none() {
            state.pending = Some((
                track.name,
                super::web_load::begin_bytes(track.bytes.to_vec(), track.name.to_string()),
            ));
        }
        let Some((_, pending)) = state.pending.as_mut() else {
            return;
        };
        match pending.take() {
            None => return,
            Some(Ok(data)) => {
                state.pending = None;
                mixer::StreamingSoundData::from_static(data)
            }
            Some(Err(e)) => {
                warn!("mode music: {} — {e}", track.name);
                state.pending = None;
                state.failed = Some(track.name);
                return;
            }
        }
    };

    // In at silence, then up over the crossfade, so the old track's fade-out and this fade-in
    // overlap.
    let data = mixer::looped(data).volume(mixer::amp_to_db(0.0));
    match mixer.play_stream(data) {
        Ok(mut handle) => {
            handle.set_volume(mixer::amp_to_db(target), mixer::fade(FADE_MS));
            info!("mode music: {} (crossfade {FADE_MS} ms)", track.name);
            state.playing = Some(Playing {
                name: track.name,
                handle,
                settled_at: now + FADE_MS as f64 / 1000.0,
                sent: target,
            });
        }
        Err(e) => {
            warn!("mode music: {} — {e:#}", track.name);
            state.failed = Some(track.name);
        }
    }
}
