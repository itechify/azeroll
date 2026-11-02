//! The chase camera: behind the ball, framing it the same on screen however big it grows.
//!
//! Written the way `survivors::hero::place_camera` is: real time, the `WorldCamera` transform
//! written in the Input stage (never parented to the ball; the stock camera pose publishing assumes
//! a root camera), and a fixed field of view. The stock scoped-view system resets the FOV every
//! frame, so growth is carried by the boom distance and the pitch instead (decision D8).
//!
//! # Scale grammar
//!
//! A tier crossing ([`TierReached`]) is punctuated, not just announced: [`scale_beats`] widens the
//! boom and lifts the elevation in a ~1.2 s pull-up-and-back that settles into a slightly wider
//! standing frame, glances the camera toward the next area for about a second, and steps the far
//! clip (so the fog) out. A 25 % [`GrowthBeat`] every third step only breathes the boom. Nothing
//! here touches input, the ball or time: the player keeps full control throughout, steering the
//! camera cancels the glance, and every effect hangs off a per-run high-water mark, so shedding
//! and regrowing across a threshold never repeats a beat.

use std::f32::consts::PI;

use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;
use benilla_world::view::{ViewDistance, WorldCamera, FARCLIP_RANGE};
use benilla_world::world_point::WorldPoint;
use bevy::camera::primitives::Aabb;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::design::Design;
use super::level;
use super::progress::Flow;
use super::{Attached, BallState, GrowthBeat, Phase, TierReached};

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
/// Auto-align: the rate, per second, at which the yaw closes on the heading while the ball rolls
/// straight ahead, and the widest angle (radians) it will chase.
const ALIGN_RATE: f32 = 4.0;
/// A slow turn on the results screen.
const RESULTS_SPIN: f32 = 0.15;
/// Smoothing rates, per second: how fast growth glides (distance, pitch), how tightly the look
/// point follows the ball, how fast a blocked boom eases back out.
const GLIDE_RATE: f32 = 3.0;
const FOLLOW_RATE: f32 = 22.0;
const CLEAR_RATE: f32 = 4.0;
/// The eye stays this far above the terrain: a fixed margin plus a share of the radius.
const FLOOR_MARGIN: f32 = 0.35;
const FLOOR_PER_RADIUS: f32 = 0.2;

// -------------------------------------------------------------------------------------------
// Scale grammar: tuning
// -------------------------------------------------------------------------------------------

/// Permanent framing per tier reached: boom fraction and elevation (radians). The standing frame
/// only ever widens within a run, so it never jitters on a shed.
const TIER_BOOM: f32 = 0.05;
const TIER_PITCH: f32 = 1.0 * PI / 180.0;
/// Far clip (and so fog end, which follows it) added per tier, yards.
const TIER_FAR: f32 = 40.0;
/// The pull-back overshoot at full strength, on top of the standing frame.
const PULSE_BOOM: f32 = 0.16;
const PULSE_PITCH: f32 = 4.0 * PI / 180.0;
/// The pull takes this long to reach its peak, holds, then settles onto the standing frame.
const PULL_SECS: f32 = 1.2;
const HOLD_SECS: f32 = 0.5;
const SETTLE_SECS: f32 = 3.0;
/// The far clip opens out over this long.
const FAR_SECS: f32 = 1.6;
/// Pulse strength by tier row (zero-based), and for a milestone [`GrowthBeat`].
const TIER_STRENGTH: [f32; 6] = [0.0, 0.7, 0.9, 1.0, 1.0, 1.0];
const GROWTH_STRENGTH: f32 = 0.3;
/// Every this-many GrowthBeat steps (x1.95 diameter) gets a breath; the rest are silent.
const GROWTH_EVERY: u32 = 3;
/// Growth breaths keep clear of a tier beat for this long.
const GROWTH_QUIET: f32 = 4.0;
/// The glance toward the next area: it starts after a beat, eases in, holds, eases out. The
/// camera is only held off the ball's heading for about the 0.7 s in the middle.
const REVEAL_DELAY: f32 = 0.25;
const REVEAL_IN: f32 = 0.3;
const REVEAL_HOLD: f32 = 0.3;
const REVEAL_OUT: f32 = 0.45;
/// The widest it turns the view toward the target, radians; a target further off than
/// [`REVEAL_BEHIND`] (nearly behind the ball) gets no glance at all.
const REVEAL_YAW: f32 = 0.4;
const REVEAL_BEHIND: f32 = 2.4;
/// The look point slides this fraction of the boom toward the target (bounded by the ball's size).
const REVEAL_LOOK: f32 = 0.06;
/// Beats this soon after a run starts are the debug start size catching up: framing only, no show.
const SNAP_SECS: f32 = 2.0;
/// The ball's own far clip (`ball::drive_ball`): the beat's steps ride on top of it.
const FAR_BASE: f32 = 350.0;
const FAR_PER_RADIUS: f32 = 25.0;

/// The camera's own state. Yaw is the azimuth of the eye around the ball (0 looks toward -Z, as in
/// Survivors); the rest is smoothed so growth spurts glide.
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
    /// Where the lens was last placed (world); `props` ghosts canopies on the eye → look line.
    pub eye: Vec3,
    /// How much of the boom a wall leaves free, 0..1: snaps in, eases out.
    pub open: f32,
    /// The scale beats' framing, written each frame by [`scale_beats`]: a boom multiplier, extra
    /// elevation (radians), a glance yaw (radians) and a look-point shift (world yards).
    pub beat_boom: f32,
    pub beat_pitch: f32,
    pub beat_yaw: f32,
    pub beat_look: Vec3,
    /// Set after the first placement; until then the camera snaps to the ball.
    ready: bool,
    /// Obstruction-only pitch offset, eased independently of the player orbit.
    clearance_pitch: f32,
    clearance_yaw: f32,
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
            open: 1.0,
            beat_boom: 1.0,
            beat_pitch: 0.0,
            beat_yaw: 0.0,
            beat_look: Vec3::ZERO,
            ready: false,
            clearance_pitch: 0.0,
            clearance_yaw: 0.0,
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

/// Place the world camera behind the ball. Mouse drag (either button) and Q / E swing it, the wheel
/// zooms, and it drifts behind the heading while the ball rolls straight ahead and the player is
/// not steering the camera.
pub(super) fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    ball: Res<BallState>,
    trophies: Query<&Attached>,
    prop_bounds: Query<&Aabb>,
    props: Query<(&GlobalTransform, &super::props::Occluder, Option<&Children>), Without<Attached>>,
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
    }
    rig.yaw += turn * KEY_TURN_RATE * dt;
    if phase == Phase::Results {
        rig.yaw += RESULTS_SPIN * dt;
    }
    let steering = (dragging && motion.delta != Vec2::ZERO) || turn != 0.0;

    if phase == Phase::Playing && control && !steering {
        rig.yaw *= 1.0 - smooth(ALIGN_RATE, dt);
    }

    // Targets from the ball's size: the boom and the look height scale with the radius, the pitch
    // eases from low to high.
    let size = ((radius - level::START_RADIUS) / PITCH_SPAN).clamp(0.0, 1.0);
    let size = size * size * (3.0 - 2.0 * size);
    let extent = trophies
        .iter()
        .map(|a| a.radial + a.size)
        .fold(radius, f32::max);
    let huge = ((radius - level::START_RADIUS - PITCH_SPAN) / PITCH_SPAN_HUGE).clamp(0.0, 1.0);
    let huge = huge * huge * (3.0 - 2.0 * huge);
    let want_dist =
        (DIST_BASE + DIST_PER_RADIUS * radius) * (1.0 + DIST_HUGE_EXTRA * huge) * rig.zoom;
    let want_dist = want_dist.max(extent * 2.0 + radius);
    let want_pitch = (PITCH_SMALL
        + (PITCH_MID - PITCH_SMALL) * size
        + (PITCH_HUGE - PITCH_MID) * huge
        + rig.tilt)
        .clamp(PITCH_MIN, PITCH_MAX);
    let want_look = ball.pos + Vec3::Y * radius * LOOK_LIFT;
    if rig.ready {
        let glide = smooth(GLIDE_RATE, dt);
        rig.dist += (want_dist - rig.dist) * glide;
        rig.pitch += (want_pitch - rig.pitch) * glide;
        rig.look = rig.look.lerp(want_look, smooth(FOLLOW_RATE, dt));
        // A teleport or a restart: do not swoop across the map.
        if rig.look.distance(want_look) > 20.0 + radius * 4.0 {
            rig.look = want_look;
        }
    } else {
        rig.dist = want_dist;
        rig.pitch = want_pitch;
        rig.look = want_look;
        rig.open = 1.0;
        rig.ready = true;
    }

    // Silhouette clearance is a framing preference, never permission to cross a wall. The scale
    // beats ride on the smoothed boom, pitch and look point, so their timing is their own.
    let look = rig.look + rig.beat_look;
    let yaw = ball.heading + rig.yaw + rig.beat_yaw;
    let len = (rig.dist * rig.beat_boom).max(1.0e-3);
    let base_pitch = (rig.pitch + rig.beat_pitch).clamp(PITCH_MIN, PITCH_MAX);
    let direction = |pitch: f32, offset: f32| {
        let (sy, cy) = (yaw + offset).sin_cos();
        let (sp, cp) = pitch.sin_cos();
        Vec3::new(cp * sy, sp, cp * cy)
    };
    let blockers = super::props::camera_blockers(
        look,
        len + radius,
        ball.pos.y + radius * 1.15,
        &props,
        &prop_bounds,
    );
    let clear_length = |boom: Vec3| {
        let world_limit = collision
            .cast_camera(look, boom, false)
            .map_or(boom.length(), |d| (d - 0.08).max(0.0));
        world_limit.min(super::props::prop_boom_limit(
            look,
            boom,
            &blockers,
            radius * 1.8 + 0.3,
        ))
    };
    // A trunk shortens the boom. When a broad foliage box contains the look point, also try
    // modest side bearings: shortening alone cannot get the lens out of that box. The penalty
    // measures the fraction of the segment in opaque bounds and strongly rejects an embedded eye.
    let score = |pitch: f32, offset: f32| {
        let boom = direction(pitch, offset) * len;
        clear_length(boom) / len
            - 0.7 * super::props::prop_view_penalty(look, boom, &blockers)
            - offset.abs() * 0.12
            - (pitch - base_pitch).abs() * 0.12
    };
    let mut extra = (0.0, 0.0);
    let mut best = score(base_pitch, 0.0);
    if best < 0.85 {
        for offset in [0.0, -0.35, 0.35, -0.65, 0.65] {
            if steering && offset != 0.0 {
                continue;
            }
            for lift in [0.0, -0.18, 0.18, -0.35, 0.35] {
                let pitch = (base_pitch + lift).clamp(PITCH_MIN, PITCH_MAX);
                let candidate = score(pitch, offset);
                if candidate > best + 0.12 {
                    best = candidate;
                    extra = (pitch - base_pitch, offset);
                }
            }
        }
    }
    // A close canopy can enclose every modest bearing. Prefer the small offsets above, but
    // allow a side escape before accepting an opaque screen. Lower candidates can look under
    // foliage without ghosting its contact-height parts. Player camera input always wins.
    if !steering && best < 0.8 {
        for offset in [-1.1, 1.1, -1.55, 1.55] {
            for lift in [0.0, -0.35] {
                let pitch = (base_pitch + lift).clamp(PITCH_MIN, PITCH_MAX);
                let candidate = score(pitch, offset);
                if candidate > best + 0.06 {
                    best = candidate;
                    extra = (pitch - base_pitch, offset);
                }
            }
        }
    }
    // Keep the current side when it is nearly as good: left/right ties at a tree must not flip.
    if !steering
        && best < 0.95
        && score(base_pitch + rig.clearance_pitch, rig.clearance_yaw) > best - 0.08
    {
        extra = (rig.clearance_pitch, rig.clearance_yaw);
    }
    rig.clearance_pitch += (extra.0 - rig.clearance_pitch) * smooth(GLIDE_RATE, dt);
    rig.clearance_yaw += (extra.1 - rig.clearance_yaw) * smooth(GLIDE_RATE, dt);
    let boom = direction(base_pitch + rig.clearance_pitch, rig.clearance_yaw) * len;
    let open = (clear_length(boom) / len).clamp(0.0, 1.0);
    if open < rig.open {
        rig.open = open;
    } else {
        rig.open += (open - rig.open) * smooth(CLEAR_RATE, dt);
    }
    let mut eye = look + boom * rig.open;
    // Raising the lens over terrain changes the segment. Recast the corrected boom against WMO
    // walls/ceilings as well; a terrain-only lift must not put the lens through a building.
    if let Some(h) = world.terrain_height_under(eye) {
        eye.y = eye.y.max(h + FLOOR_MARGIN + FLOOR_PER_RADIUS * radius);
    }
    let corrected = eye - look;
    let corrected_len = corrected.length();
    if corrected_len > 1.0e-4 {
        eye = look + corrected * (clear_length(corrected) / corrected_len).clamp(0.0, 1.0);
    }
    // If the lens truly has no room, look along the intended bearing instead of constructing
    // an undefined look-at transform at a zero-length boom.
    let aim = if eye.distance_squared(look) < 1.0e-6 {
        eye - direction(base_pitch, rig.clearance_yaw)
    } else {
        look
    };

    rig.eye = eye;
    let placed = Transform::from_translation(eye).looking_at(aim, Vec3::Y);
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

/// The pull-back overshoot: rise to a peak, hold, settle to nothing.
#[derive(Clone, Copy, Default)]
struct Pulse {
    age: f32,
    from: f32,
    peak: f32,
}

impl Pulse {
    fn level(&self) -> f32 {
        let a = self.age;
        if a < PULL_SECS {
            self.from + (self.peak - self.from) * smoothstep(a / PULL_SECS)
        } else if a < PULL_SECS + HOLD_SECS {
            self.peak
        } else {
            self.peak * (1.0 - smoothstep((a - PULL_SECS - HOLD_SECS) / SETTLE_SECS))
        }
    }

    /// A stronger pulse takes over from the current level; a weaker one is already covered.
    fn trigger(&mut self, peak: f32) -> bool {
        let now = self.level();
        if peak <= now {
            return false;
        }
        *self = Self {
            age: 0.0,
            from: now,
            peak,
        };
        true
    }
}

/// How far into its glance the camera is, 0..1, `age` seconds after the beat.
fn reveal_weight(age: f32) -> f32 {
    let a = age - REVEAL_DELAY;
    if a < 0.0 {
        0.0
    } else if a < REVEAL_IN {
        smoothstep(a / REVEAL_IN)
    } else if a < REVEAL_IN + REVEAL_HOLD {
        1.0
    } else {
        1.0 - smoothstep((a - REVEAL_IN - REVEAL_HOLD) / REVEAL_OUT)
    }
}

const REVEAL_SECS: f32 = REVEAL_DELAY + REVEAL_IN + REVEAL_HOLD + REVEAL_OUT;

/// The glance toward `target` at weight `w`: the extra yaw (radians) and the look-point shift
/// (world yards) that turn the view toward it from a camera that sits at `yaw` behind `ball`.
fn reveal_offsets(
    ball: Vec3,
    yaw: f32,
    radius: f32,
    boom: f32,
    target: Vec3,
    w: f32,
) -> (f32, Vec3) {
    let to = Vec3::new(target.x - ball.x, 0.0, target.z - ball.z);
    if w <= 0.0 || to.length_squared() < 1.0 {
        return (0.0, Vec3::ZERO);
    }
    // The eye sits opposite the way it looks, so the yaw that looks at the target is the one that
    // faces away from it.
    let want = (-to.x).atan2(-to.z);
    let diff = (want - yaw + PI).rem_euclid(2.0 * PI) - PI;
    let falloff = 1.0 - smoothstep((diff.abs() - (REVEAL_BEHIND - 0.8)) / 0.8);
    let w = w * falloff;
    let shift = (REVEAL_LOOK * boom).min(0.5 * radius + 0.15);
    (
        diff.clamp(-REVEAL_YAW, REVEAL_YAW) * w,
        to.normalize() * shift * w,
    )
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
    pulse: Pulse,
    /// The glance: seconds since it began, and where it looks.
    reveal: Option<(f32, Vec3)>,
    /// 1 when the player leaves the camera alone, easing to 0 while they steer it.
    gate: f32,
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
            pulse: Pulse::default(),
            reveal: None,
            gate: 1.0,
            shots: crate::webenv::var("WOW_AZEROLL_BEAT_SHOTS").map(std::path::PathBuf::from),
            shots_due: Vec::new(),
            shot_tier: 0,
            trace: crate::webenv::var("WOW_AZEROLL_BEAT_TRACE").is_some(),
            trace_at: 0.0,
        }
    }
}

/// Take the tier and growth beats, move the framing they drive, and write it into the rig for
/// [`place_camera`], which runs next. Real input is never read for anything but whether the
/// player is steering the camera themselves, which cancels the glance.
#[allow(clippy::too_many_arguments)]
pub(super) fn scale_beats(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    ball: Res<BallState>,
    design: Res<Design>,
    flow: Res<Flow>,
    mut tiers: MessageReader<TierReached>,
    mut growth: MessageReader<GrowthBeat>,
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
    beat.pulse.age += dt;

    let tier = tiers.read().max_by_key(|t| t.tier).cloned();
    let step = growth.read().map(|g| g.step).max();
    if let Some(t) = tier.filter(|t| playing && t.tier > beat.tier) {
        beat.tier = t.tier;
        beat.tier_at = beat.clock;
        let steps = t.tier as f32;
        if beat.clock < SNAP_SECS {
            beat.framing.snap(steps);
            beat.far.snap(steps);
        } else {
            beat.framing.retarget(steps, PULL_SECS);
            beat.far.retarget(steps, FAR_SECS);
            let strength = TIER_STRENGTH[t.tier.min(TIER_STRENGTH.len() - 1)];
            beat.pulse.trigger(strength);
            let target = level::reveal_target(&design, t.tier, ball.pos);
            beat.reveal = target.map(|(_, at)| (0.0, at));
            info!(
                "azeroll scale beat: tier {} {:?} at play {:.1}s D={:.2} pull +{:.0}% reveal {}",
                t.tier,
                t.name,
                beat.clock,
                ball.diameter(),
                100.0 * (TIER_BOOM * steps + PULSE_BOOM * strength),
                target.map_or("none".to_string(), |(name, at)| format!(
                    "{name} {:.0} yd",
                    Vec3::new(at.x - ball.pos.x, 0.0, at.z - ball.pos.z).length()
                )),
            );
            if beat.shots.is_some() {
                beat.shots_due = SHOT_TIMES.to_vec();
                beat.shot_tier = t.tier;
            }
        }
    } else if let Some(step) = step {
        if playing
            && step % GROWTH_EVERY == 0
            && beat.clock >= SNAP_SECS
            && beat.clock - beat.tier_at > GROWTH_QUIET
        {
            beat.pulse.trigger(GROWTH_STRENGTH);
        }
    }

    // A glance is the one thing the player can override: steering the camera cancels it.
    let steering = keys.any_pressed([KeyCode::KeyQ, KeyCode::KeyE])
        || (mouse.any_pressed([MouseButton::Left, MouseButton::Right])
            && motion.delta != Vec2::ZERO);
    let gate_goal = if steering || flow.finale.is_some() {
        0.0
    } else {
        1.0
    };
    let rate = if gate_goal < beat.gate { 14.0 } else { 6.0 };
    beat.gate += (gate_goal - beat.gate) * smooth(rate, dt);
    let mut glance = (0.0, Vec3::ZERO);
    if let Some((age, target)) = beat.reveal.as_mut() {
        *age += dt;
        if *age >= REVEAL_SECS {
            beat.reveal = None;
        } else {
            let (age, target) = (*age, *target);
            let boom = DIST_BASE + DIST_PER_RADIUS * ball.radius;
            glance = reveal_offsets(
                ball.pos,
                ball.heading + rig.yaw,
                ball.radius,
                boom,
                target,
                reveal_weight(age) * beat.gate,
            );
        }
    }

    let steps = beat.framing.value();
    let pulse = beat.pulse.level();
    rig.beat_boom = 1.0 + TIER_BOOM * steps + PULSE_BOOM * pulse;
    rig.beat_pitch = TIER_PITCH * steps + PULSE_PITCH * pulse;
    rig.beat_yaw = glance.0;
    rig.beat_look = glance.1;

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
            "azeroll beat trace: +{:.2}s boom x{:.3} pitch +{:.1}deg yaw {:+.1}deg look {:.2} far +{:.0}",
            beat.clock - beat.tier_at,
            rig.beat_boom,
            rig.beat_pitch.to_degrees(),
            rig.beat_yaw.to_degrees(),
            rig.beat_look.length(),
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
    fn the_pull_rises_holds_and_settles_without_a_jump() {
        let mut p = Pulse::default();
        assert!(p.trigger(1.0));
        let mut last = 0.0;
        let dt = 1.0 / 60.0;
        let mut peak_at = None;
        for i in 0..600 {
            p.age = i as f32 * dt;
            let v = p.level();
            assert!((v - last).abs() < 0.05, "jump {last} -> {v} at {}", p.age);
            if v >= 0.999 && peak_at.is_none() {
                peak_at = Some(p.age);
            }
            last = v;
        }
        // About 1.2 s to the peak, back to nothing by the end of the settle.
        assert!((peak_at.unwrap() - PULL_SECS).abs() < 0.15);
        assert!(last < 1e-3);
    }

    #[test]
    fn a_weaker_pulse_never_cuts_a_stronger_one_short() {
        let mut p = Pulse::default();
        p.trigger(0.9);
        p.age = 1.0;
        let now = p.level();
        assert!(!p.trigger(0.3));
        assert_eq!(p.level(), now);
        // A stronger one carries on from the current level instead of snapping.
        assert!(p.trigger(1.0));
        assert!((p.level() - now).abs() < 1e-6);
    }

    #[test]
    fn the_glance_is_brief_and_returns_to_nothing() {
        assert_eq!(reveal_weight(0.0), 0.0);
        assert_eq!(reveal_weight(REVEAL_SECS), 0.0);
        let held: f32 = (0..1300)
            .map(|i| i as f32 / 1000.0)
            .filter(|&a| reveal_weight(a) > 0.5)
            .count() as f32
            / 1000.0;
        assert!(held < 1.0, "holds the camera off the ball for {held}s");
        assert!(held > 0.4);
    }

    #[test]
    fn the_glance_turns_toward_the_target_and_never_far() {
        // Camera behind a ball facing -Z (heading 0, eye at +Z); the target is off to the east.
        let ball = Vec3::ZERO;
        let east = Vec3::new(100.0, 0.0, -20.0);
        let (yaw, look) = reveal_offsets(ball, 0.0, 0.5, 6.0, east, 1.0);
        // Eye azimuth (sin y, cos y): swinging toward -X puts the lens west, looking east.
        assert!(
            yaw < 0.0 && (-REVEAL_YAW - 1e-6..0.0).contains(&yaw),
            "{yaw}"
        );
        assert!(look.x > 0.0 && look.y == 0.0);
        assert!(look.length() <= 0.5 * 0.5 + 0.15 + 1e-6);
        let west = Vec3::new(-100.0, 0.0, -20.0);
        assert!(reveal_offsets(ball, 0.0, 0.5, 6.0, west, 1.0).0 > 0.0);
        // Straight ahead needs no turn, directly behind gets no glance, weight zero gets none.
        assert!(
            reveal_offsets(ball, 0.0, 0.5, 6.0, Vec3::new(0.0, 0.0, -50.0), 1.0)
                .0
                .abs()
                < 1e-5
        );
        assert_eq!(
            reveal_offsets(ball, 0.0, 0.5, 6.0, Vec3::new(0.0, 0.0, 50.0), 1.0).0,
            0.0
        );
        assert_eq!(
            reveal_offsets(ball, 0.0, 0.5, 6.0, east, 0.0),
            (0.0, Vec3::ZERO)
        );
    }

    #[test]
    fn the_far_clip_steps_stay_inside_the_engines_range() {
        for radius in [0.25, 2.2, 5.0, 40.0] {
            let top = base_farclip(radius) + TIER_FAR * (TIER_STRENGTH.len() - 1) as f32;
            assert!(base_farclip(radius) >= *FARCLIP_RANGE.start());
            // Clamped on write; Hogger's own size leaves room for every step to be seen.
            if radius <= 2.2 {
                assert!(top <= *FARCLIP_RANGE.end(), "r {radius}: {top}");
            }
        }
    }
}
