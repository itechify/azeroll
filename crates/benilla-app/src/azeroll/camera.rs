//! The chase camera: behind the ball, framing it the same on screen however big it grows.
//!
//! Real time, the `WorldCamera` transform written in the Input stage (never parented to the ball;
//! the stock camera pose publishing assumes a root camera), and a fixed field of view. The stock
//! scoped-view system resets the FOV every frame, so growth is carried by the boom distance and the
//! pitch instead (decision D8).
//!
//! A Katamari camera: the boom and the pitch are a smooth function of the ball's radius (plus the
//! player's own zoom and orbit, and the yaw's slow auto-align behind the heading), and nothing in
//! the world ever moves them. Walls, props and trees do not shorten the boom or swing the view;
//! the lens may sit behind or inside them, and `cutout.rs` makes whatever lies between the lens and
//! the ball see-through instead. Only the terrain is respected: the lens rides above the ground.
//!
//! # Scale grammar
//!
//! A tier crossing ([`TierReached`]) widens the standing frame a little (boom and elevation) in a
//! slow ease that never comes back in, and steps the far clip (so the fog) out. Nothing here
//! touches input, the ball or time, and every effect hangs off a per-run high-water mark, so
//! shedding and regrowing across a threshold never repeats a beat.

use std::f32::consts::PI;

use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;
use benilla_world::view::{ViewDistance, WorldCamera, FARCLIP_RANGE};
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::{Attached, BallState, Phase, TierReached};

/// Boom length in yards: `DIST_BASE + DIST_PER_RADIUS * radius`, times the player's zoom.
const DIST_BASE: f32 = 2.2;
const DIST_PER_RADIUS: f32 = 7.0;
/// The point looked at is this fraction of a radius above the ball's centre.
const LOOK_LIFT: f32 = 0.6;
/// Elevation above the horizon, Katamari-low: a tiny ball is seen from low and behind, a huge one from
/// only a little higher, so a big clump keeps its depth instead of reading as a flat disc.
const PITCH_SMALL: f32 = 20.0 * PI / 180.0;
const PITCH_MID: f32 = 31.0 * PI / 180.0;
/// Past the mid radius the camera creeps up a little more, with the boom backing off, at late-game sizes.
const PITCH_HUGE: f32 = 38.0 * PI / 180.0;
/// The radius span over which the pitch eases from small to mid, then from mid to huge.
const PITCH_SPAN: f32 = 3.5;
const PITCH_SPAN_HUGE: f32 = 9.0;
/// Extra boom, as a fraction, at the end of the huge span: backing off lifts the lens over trees.
const DIST_HUGE_EXTRA: f32 = 0.3;
/// The total elevation clamp, radians.
const PITCH_MIN: f32 = 0.12;
const PITCH_MAX: f32 = 1.2;
/// The vertical drag's offset on the pitch, radians.
const TILT_MIN: f32 = -0.12;
const TILT_MAX: f32 = 0.45;
/// Wheel zoom multiplier range.
const ZOOM_MIN: f32 = 0.6;
const ZOOM_MAX: f32 = 1.6;
/// Radians of yaw per raw mouse unit while dragging.
const ORBIT_RATE: f32 = 0.006;
/// Radians per second of yaw on Q / E.
const KEY_TURN_RATE: f32 = 1.8;
/// The pad's d-pad zoom (fraction of the distance per second), right-stick tilt (radians per
/// second) and the ease of a trigger's peek.
const PAD_ZOOM_RATE: f32 = 0.9;
const PAD_TILT_RATE: f32 = 0.8;
const PEEK_RATE: f32 = 5.0;
/// Auto-align: the rate, per second, at which the yaw closes on the heading while the ball rolls
/// straight ahead, and the widest angle (radians) it will chase.
const ALIGN_RATE: f32 = 4.0;
/// A slow turn on the results screen.
const RESULTS_SPIN: f32 = 0.15;
/// Smoothing rates, per second: how fast growth glides (distance, pitch) out and back in, how
/// tightly the look point follows the ball.
const GLIDE_RATE: f32 = 3.0;
const GLIDE_IN_RATE: f32 = 1.5;
const FOLLOW_RATE: f32 = 22.0;
/// The trophy-extent floor on the boom eases out at the first rate and back in at the second, so
/// a long item attaching never jumps the lens away and shedding it never pulls the lens in fast.
const EXTENT_OUT_RATE: f32 = 1.0;
const EXTENT_IN_RATE: f32 = 0.25;
/// The eye stays this far above the terrain: a fixed margin plus a share of the radius. The lens
/// is lifted over a rise at the first rate, eased back down at the second.
const FLOOR_MARGIN: f32 = 0.35;
const FLOOR_PER_RADIUS: f32 = 0.2;
const LIFT_UP_RATE: f32 = 10.0;
const LIFT_DOWN_RATE: f32 = 1.5;

// -------------------------------------------------------------------------------------------
// Scale grammar: tuning
// -------------------------------------------------------------------------------------------

/// Permanent framing per tier reached: boom fraction and elevation (radians). The standing frame
/// only ever widens within a run, so it never jitters on a shed.
const TIER_BOOM: f32 = 0.05;
const TIER_PITCH: f32 = 1.0 * PI / 180.0;
/// Far clip (and so fog end, which follows it) added per tier, yards.
const TIER_FAR: f32 = 40.0;
/// The standing frame widens over this long, a slow zoom-out that never returns.
const FRAMING_SECS: f32 = 3.0;
/// The far clip opens out over this long.
const FAR_SECS: f32 = 1.6;
/// The highest tier row.
const MAX_TIER: usize = 5;
/// Beats this soon after a run starts are the debug start size catching up: framing only.
const SNAP_SECS: f32 = 2.0;
/// The ball's own far clip (`ball::drive_ball`): the beat's steps ride on top of it.
const FAR_BASE: f32 = 350.0;
const FAR_PER_RADIUS: f32 = 25.0;

/// The camera's own state. Yaw is the azimuth of the eye around the ball (0 looks toward -Z); the
/// rest is smoothed so growth spurts glide.
#[derive(Resource)]
pub(super) struct CamRig {
    /// Temporary orbit offset from the tank heading, radians.
    pub yaw: f32,
    /// The player's vertical drag, added to the size-based pitch.
    pub tilt: f32,
    /// Smoothed elevation above the horizon, radians.
    pub pitch: f32,
    /// Smoothed boom length, yards.
    pub dist: f32,
    /// The wheel's multiplier on the boom length.
    pub zoom: f32,
    /// Smoothed look point.
    pub look: Vec3,
    /// Where the lens was last placed (world).
    pub eye: Vec3,
    /// The scale beats' standing frame, written each frame by [`scale_beats`]: a boom multiplier
    /// and extra elevation (radians). It only ever grows within a run.
    pub beat_boom: f32,
    pub beat_pitch: f32,
    /// Set after the first placement; until then the camera snaps to the ball.
    ready: bool,
    /// The largest attached radial extent, smoothed slowly (out) and slower still (in).
    extent: f32,
    /// How far the lens is lifted over rising terrain, yards.
    lift: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            tilt: 0.0,
            pitch: PITCH_SMALL,
            dist: DIST_BASE,
            zoom: 1.0,
            look: Vec3::ZERO,
            eye: Vec3::ZERO,
            beat_boom: 1.0,
            beat_pitch: 0.0,
            ready: false,
            extent: 0.0,
            lift: 0.0,
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CamRig>()
        .insert_resource(ScaleBeat::new())
        .add_systems(OnEnter(Phase::Ready), reset_rig)
        .add_systems(
            Update,
            scale_beats
                .in_set(WorldStage::Input)
                .after(super::collect::collect)
                .before(place_camera),
        );
}

/// A new run frames the start fresh.
fn reset_rig(mut rig: ResMut<CamRig>, mut beat: ResMut<ScaleBeat>) {
    *rig = CamRig::default();
    *beat = ScaleBeat::new();
}

/// Exponential smoothing factor for a rate in 1/s: `1 - e^(-rate * dt)`, frame-rate independent.
fn smooth(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Ease `value` toward `goal` at `up` per second when rising, `down` when falling.
fn ease_toward(value: f32, goal: f32, up: f32, down: f32, dt: f32) -> f32 {
    let rate = if goal > value { up } else { down };
    value + (goal - value) * smooth(rate, dt)
}

/// Place the world camera behind the ball. Mouse drag (either button) and Q / E swing it, the wheel
/// zooms, and it drifts behind the heading while the ball rolls straight ahead and the player is
/// not steering the camera. Obstacles never move it: only the radius, the player and the terrain
/// floor do.
pub(super) fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    pad: Res<super::pad::PadInput>,
    mut peeking: Local<bool>,
    ball: Res<BallState>,
    shown: Res<super::ball::Shown>,
    trophies: Query<&Attached>,
    world: WorldPoint,
    collision: WorldCollision,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    let dt = time.delta_secs().min(0.1);
    let phase = *phase.get();
    let control = !loading.covering() && matches!(phase, Phase::Playing | Phase::Ready);
    let radius = ball.radius;

    // The wheel is multiplicative, so a notch moves as far by eye close in as far out.
    for w in wheel.read() {
        if !control {
            continue;
        }
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.zoom = (rig.zoom * (1.0 - 0.1 * notches).clamp(0.7, 1.4)).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    // Either mouse button drags the camera around the ball, as in WoW; the same drag tilts it.
    let dragging = control && mouse.any_pressed([MouseButton::Left, MouseButton::Right]);
    if dragging {
        rig.yaw -= motion.delta.x * ORBIT_RATE;
        rig.tilt = (rig.tilt + motion.delta.y * ORBIT_RATE).clamp(TILT_MIN, TILT_MAX);
    }
    let mut turn = 0.0;
    if control {
        if keys.pressed(KeyCode::KeyE) {
            turn += 1.0;
        }
        if keys.pressed(KeyCode::KeyQ) {
            turn -= 1.0;
        }
        // The pad's shoulders (and, in simple mode, the right stick) are Q / E.
        turn = (turn + pad.orbit).clamp(-1.0, 1.0);
        // Its d-pad zooms like the wheel, its right stick (simple mode) tilts like a drag, and a
        // trigger peeks: the camera rises to look down on the ball, and settles back on release.
        if pad.zoom != 0.0 {
            rig.zoom = (rig.zoom * (1.0 - PAD_ZOOM_RATE * pad.zoom * dt)).clamp(ZOOM_MIN, ZOOM_MAX);
        }
        if pad.tilt != 0.0 {
            rig.tilt = (rig.tilt + pad.tilt * PAD_TILT_RATE * dt).clamp(TILT_MIN, TILT_MAX);
        }
        if pad.peek {
            rig.tilt += (TILT_MAX - rig.tilt) * smooth(PEEK_RATE, dt);
        } else if *peeking && !dragging {
            rig.tilt -= rig.tilt * smooth(PEEK_RATE, dt);
        }
        *peeking = pad.peek || (*peeking && rig.tilt.abs() > 0.01 && !dragging);
    }
    rig.yaw += turn * KEY_TURN_RATE * dt;
    if matches!(phase, Phase::Results | Phase::Select) {
        rig.yaw += RESULTS_SPIN * dt;
    }
    let steering = (dragging && motion.delta != Vec2::ZERO)
        || turn != 0.0
        || (control && (pad.tilt != 0.0 || pad.peek));

    if phase == Phase::Playing && control && !steering {
        rig.yaw *= 1.0 - smooth(ALIGN_RATE, dt);
    }

    // Targets from the ball's size: the boom and the look height scale with the radius, the pitch
    // eases from low to high.
    let size = ((radius - ball.start_radius) / PITCH_SPAN).clamp(0.0, 1.0);
    let size = size * size * (3.0 - 2.0 * size);
    let extent = trophies
        .iter()
        .map(|a| a.radial + a.size)
        .fold(radius, f32::max);
    let huge = ((radius - ball.start_radius - PITCH_SPAN) / PITCH_SPAN_HUGE).clamp(0.0, 1.0);
    let huge = huge * huge * (3.0 - 2.0 * huge);
    // The drawn centre: a step or settle pop is eased there (`ball::Shown`), so the lens glides too.
    let want_look = shown.at(&ball) + Vec3::Y * radius * LOOK_LIFT;
    if rig.ready {
        // A protruding trophy raises the boom's floor slowly and lowers it slower still.
        rig.extent =
            ease_toward(rig.extent, extent, EXTENT_OUT_RATE, EXTENT_IN_RATE, dt).max(radius);
    } else {
        rig.extent = extent;
    }
    let want_dist =
        (DIST_BASE + DIST_PER_RADIUS * radius) * (1.0 + DIST_HUGE_EXTRA * huge) * rig.zoom;
    let want_dist = want_dist.max(rig.extent * 2.0 + radius);
    let want_pitch = (PITCH_SMALL
        + (PITCH_MID - PITCH_SMALL) * size
        + (PITCH_HUGE - PITCH_MID) * huge
        + rig.tilt)
        .clamp(PITCH_MIN, PITCH_MAX);
    if rig.ready {
        rig.dist = ease_toward(rig.dist, want_dist, GLIDE_RATE, GLIDE_IN_RATE, dt);
        rig.pitch += (want_pitch - rig.pitch) * smooth(GLIDE_RATE, dt);
        rig.look = rig.look.lerp(want_look, smooth(FOLLOW_RATE, dt));
        // A teleport or a restart: do not swoop across the map.
        if rig.look.distance(want_look) > 20.0 + radius * 4.0 {
            rig.look = want_look;
            rig.lift = 0.0;
        }
    } else {
        rig.dist = want_dist;
        rig.pitch = want_pitch;
        rig.look = want_look;
        rig.lift = 0.0;
        rig.ready = true;
    }

    // The scale beats' standing frame rides on the smoothed boom and pitch. Distance and
    // elevation come from the radius alone: no wall, prop or tree shortens the boom or swings the
    // view (`cutout.rs` makes what is in the way see-through instead).
    let look = rig.look;
    let yaw = ball.heading + rig.yaw;
    // D26: reserve far-plane space beyond the ball even for diagnostic r=300.
    let len = (rig.dist * rig.beat_boom).clamp(1.0e-3, 480.0);
    let pitch = (rig.pitch + rig.beat_pitch).clamp(PITCH_MIN, PITCH_MAX);
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * len;
    // The one thing the world does: the lens stays over the ground. Terrain above it lifts the
    // lens (the boom stays as long as it was), fast on the way up and slowly back down, so a rise
    // never snaps the view. The hard floor underneath only holds the lens above the ground.
    let inside = world.indoors_at(ball.pos)
        || (world
            .terrain_height_under(ball.pos)
            .is_some_and(|h| h > ball.pos.y + 0.1)
            && super::ball::floor_rest(&collision, ball.pos, radius).is_some());
    // Terrain above a cellar is its ceiling, not a floor for the camera. Keep
    // the radius-driven boom; the existing eased lift decays and cutout clears
    // intervening WMO walls/ceilings without a camera collision detour.
    let floor = (!inside)
        .then(|| world.terrain_height_under(eye))
        .flatten()
        .map(|h| h + FLOOR_MARGIN + FLOOR_PER_RADIUS * radius);
    let want_lift = floor.map_or(0.0, |f| (f - eye.y).max(0.0));
    rig.lift = ease_toward(rig.lift, want_lift, LIFT_UP_RATE, LIFT_DOWN_RATE, dt);
    eye.y += rig.lift;
    if let Some(f) = floor {
        eye.y = eye.y.max(f);
    }

    rig.eye = eye;
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

// -------------------------------------------------------------------------------------------
// Scale grammar
// -------------------------------------------------------------------------------------------

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// An eased move between two values over a fixed time.
#[derive(Clone, Copy, Default)]
struct Tween {
    from: f32,
    to: f32,
    age: f32,
    secs: f32,
}

impl Tween {
    fn value(&self) -> f32 {
        if self.secs <= 0.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * smoothstep(self.age / self.secs)
    }

    /// Head for `to` from wherever the value is now.
    fn retarget(&mut self, to: f32, secs: f32) {
        *self = Self {
            from: self.value(),
            to,
            age: 0.0,
            secs,
        };
    }

    fn snap(&mut self, to: f32) {
        *self = Self {
            from: to,
            to,
            age: 0.0,
            secs: 0.0,
        };
    }
}

/// The far clip the ball asks for on its own (`ball::drive_ball`).
fn base_farclip(radius: f32) -> f32 {
    (FAR_BASE + radius * FAR_PER_RADIUS).clamp(FAR_BASE, *FARCLIP_RANGE.end())
}

/// What the scale grammar has done this run. Reset on Ready; only ever advances, so a ball that
/// sheds below a threshold and regrows across it finds the beat already taken.
#[derive(Resource)]
pub(super) struct ScaleBeat {
    /// Seconds of play (virtual time) since the run started.
    clock: f32,
    /// The highest tier row beaten, and when (on `clock`).
    tier: usize,
    tier_at: f32,
    /// Tiers' standing frame, and the far clip's steps, in tiers.
    framing: Tween,
    far: Tween,
    /// `WOW_AZEROLL_BEAT_SHOTS`: a directory to screenshot a tier beat into, with the seconds after
    /// the beat each frame is due.
    shots: Option<std::path::PathBuf>,
    shots_due: Vec<f32>,
    shot_tier: usize,
    trace: bool,
    trace_at: f32,
}

/// Seconds after a tier beat at which `WOW_AZEROLL_BEAT_SHOTS` takes its frames.
const SHOT_TIMES: [f32; 7] = [0.0, 0.4, 0.8, 1.2, 1.8, 3.0, 6.0];

impl ScaleBeat {
    fn new() -> Self {
        Self {
            clock: 0.0,
            tier: 0,
            tier_at: f32::NEG_INFINITY,
            framing: Tween::default(),
            far: Tween::default(),
            shots: crate::webenv::var("WOW_AZEROLL_BEAT_SHOTS").map(std::path::PathBuf::from),
            shots_due: Vec::new(),
            shot_tier: 0,
            trace: crate::webenv::var("WOW_AZEROLL_BEAT_TRACE").is_some(),
            trace_at: 0.0,
        }
    }
}

/// Take the tier beats, move the standing frame and far clip they drive, and write the frame into
/// the rig for [`place_camera`], which runs next.
pub(super) fn scale_beats(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    ball: Res<BallState>,
    mut tiers: MessageReader<TierReached>,
    mut beat: ResMut<ScaleBeat>,
    mut rig: ResMut<CamRig>,
    view: Option<ResMut<ViewDistance>>,
) {
    let dt = time.delta_secs().min(0.1);
    let playing = *phase.get() == Phase::Playing;
    if playing {
        beat.clock += dt;
    }
    beat.framing.age += dt;
    beat.far.age += dt;

    let tier = tiers.read().max_by_key(|t| t.tier).cloned();
    if let Some(t) = tier.filter(|t| playing && t.tier > beat.tier) {
        beat.tier = t.tier;
        beat.tier_at = beat.clock;
        let steps = t.tier.min(MAX_TIER) as f32;
        if beat.clock < SNAP_SECS {
            beat.framing.snap(steps);
            beat.far.snap(steps);
        } else {
            beat.framing.retarget(steps, FRAMING_SECS);
            beat.far.retarget(steps, FAR_SECS);
            info!(
                "azeroll scale beat: tier {} {:?} at play {:.1}s D={:.2} frame +{:.0}%",
                t.tier,
                t.name,
                beat.clock,
                ball.diameter(),
                100.0 * TIER_BOOM * steps,
            );
            if beat.shots.is_some() {
                beat.shots_due = SHOT_TIMES.to_vec();
                beat.shot_tier = t.tier;
            }
        }
    }

    let steps = beat.framing.value();
    rig.beat_boom = 1.0 + TIER_BOOM * steps;
    rig.beat_pitch = TIER_PITCH * steps;

    // The far clip, and the fog that ends at it, steps out with each tier. `drive_ball` sets its
    // own continuous value first each frame; the steps are added on top and only while there are
    // any, so a fresh run hands the clip straight back to it.
    let bonus = TIER_FAR * beat.far.value();
    if bonus > 0.5 {
        if let Some(mut view) = view {
            let want = (base_farclip(ball.radius) + bonus)
                .clamp(*FARCLIP_RANGE.start(), *FARCLIP_RANGE.end());
            if (view.farclip - want).abs() > 0.5 {
                view.farclip = want;
            }
        }
    }

    if beat.trace && (beat.clock - beat.tier_at) < 8.0 && beat.clock - beat.trace_at > 0.2 {
        beat.trace_at = beat.clock;
        info!(
            "azeroll beat trace: +{:.2}s boom x{:.3} pitch +{:.1}deg far +{:.0}",
            beat.clock - beat.tier_at,
            rig.beat_boom,
            rig.beat_pitch.to_degrees(),
            bonus,
        );
    }

    if !beat.shots_due.is_empty() {
        let since = beat.clock - beat.tier_at;
        while beat.shots_due.first().is_some_and(|due| since >= *due) {
            let due = beat.shots_due.remove(0);
            if let Some(dir) = beat.shots.as_ref() {
                let name = format!(
                    "tier{}-{:04}ms-play{:.1}.png",
                    beat.shot_tier,
                    (due * 1000.0) as u32,
                    beat.clock
                );
                info!("azeroll beat shot {name} at +{since:.2}s");
                commands
                    .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                    .observe(bevy::render::view::screenshot::save_to_disk(dir.join(name)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standing_frame_only_widens_and_never_jumps() {
        let mut t = Tween::default();
        t.retarget(1.0, FRAMING_SECS);
        let dt = 1.0 / 60.0;
        let mut last = 0.0;
        for i in 0..400 {
            t.age = i as f32 * dt;
            let v = t.value();
            assert!(v >= last - 1e-6, "pulled in {last} -> {v} at {}", t.age);
            assert!(v - last < 0.03, "jump {last} -> {v}");
            last = v;
        }
        assert!((last - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_retrigger_continues_from_the_current_level() {
        let mut t = Tween::default();
        t.retarget(1.0, FRAMING_SECS);
        t.age = 1.0;
        let now = t.value();
        t.retarget(2.0, FRAMING_SECS);
        assert!((t.value() - now).abs() < 1e-6);
    }

    #[test]
    fn the_trophy_floor_eases_out_slowly_and_in_slower() {
        // A long item attaches: half a second later the floor is still well short of it.
        let mut out = 0.5_f32;
        for _ in 0..30 {
            out = ease_toward(out, 3.0, EXTENT_OUT_RATE, EXTENT_IN_RATE, 1.0 / 60.0);
        }
        assert!(out > 0.5 && out < 1.6, "{out}");
        // Shed again: it takes seconds to come back, never a sudden pull-in.
        let mut inn = 3.0_f32;
        for _ in 0..60 {
            inn = ease_toward(inn, 0.5, EXTENT_OUT_RATE, EXTENT_IN_RATE, 1.0 / 60.0);
        }
        assert!(inn > 2.4, "{inn}");
    }

    #[test]
    fn the_far_clip_steps_stay_inside_the_engines_range() {
        for radius in [0.25, 2.2, 5.0, 40.0] {
            let top = base_farclip(radius) + TIER_FAR * MAX_TIER as f32;
            assert!(base_farclip(radius) >= *FARCLIP_RANGE.start());
            // Clamped on write; Hogger's own size leaves room for every step to be seen.
            if radius <= 2.2 {
                assert!(top <= *FARCLIP_RANGE.end(), "r {radius}: {top}");
            }
        }
    }
}
