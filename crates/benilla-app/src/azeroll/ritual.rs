//! The end of a run, from the moment it ends until the hub or the next try (D31, and
//! `docs/azeroll/ritual-design.md` §2). The phase leaves Playing at once: the world stops (virtual
//! time pauses, so nothing but the lens and the clump itself moves) and the camera eases to a
//! three-quarter view of the clump. A won run is then lifted clear of the ground by Fizzwick's
//! Reclamation Beam and hovers, lit, while the results dock beside it; a lost one just stops and
//! lets the player look at what they built. Everything is a pure function of the real seconds since
//! the ending, so skipping is a jump in time.
//!
//! The exits are the invariant: leaving by the hub, by Roll Again, by Free Roll, by the window or by
//! a debug shot always ends with virtual time running, the camera released, the lift and the beam
//! gone ([`teardown`], on every way out of Results and into Ready or Select).

use std::f32::consts::{PI, TAU};

use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;
use benilla_world::view::{CameraPoseSet, WorldCamera};
use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

use super::camera::CamRig;
use super::campaign::CurrentLevel;
use super::fizz::{FizzSays, Voice};
use super::lines::{self, Said};
use super::progress::{self, Flow};
use super::{Attached, Ball, BallSpin, BallState, Phase, Rng, Run, RunEntity};

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ending {
    /// The candle went out on a run that had met its goal.
    Snuff,
    /// The player held the finish key after the goal.
    Called,
    /// A single-target goal was captured (after its pre-roll).
    Captured,
    /// The candle went out under the goal.
    Failed,
}

impl Ending {
    /// Whether the clump is lifted (a lost run just stops).
    pub fn lifted(self) -> bool {
        self != Ending::Failed
    }
}

/// What the results screen asked for. Sent by the keys (`level::keys`) and by the buttons
/// (`select::verdict`), acted on here so every way out goes through the same exits.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wish {
    /// Back to the hub, the clump flying up the beam.
    Hub,
    /// Roll the level again.
    Again,
    /// Free Roll: the clump drops back and play resumes untimed.
    Free,
}

/// A sound the ritual wants; `sfx` plays it. Kept as names here so this file never needs the kit
/// tables.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cue {
    /// The candle going out.
    Bell,
    /// One of the last ten seconds, or the finish key filling.
    Tick,
    /// The beam lands.
    Land,
    /// The lift.
    Whoosh,
    /// The apex of the lift.
    Apex,
    /// The star at the end of the beam.
    StarPop,
    /// The clump back on the ground.
    Thud,
    /// The results arrive.
    Fanfare,
    /// The goal met mid-run: a quieter fanfare that does not sound like the end.
    GoalFanfare,
    /// A star lit on the dial.
    Star,
    /// A record fell.
    Record,
}

/// Seconds (real, from the ending) at which each beat lands.
pub(super) const VIEW_SECS: f32 = 1.0;
pub(super) const BEAM_AT: f32 = 0.6;
const BEAM_FADE: f32 = 0.4;
pub(super) const LIFT_AT: f32 = 1.0;
pub(super) const LIFT_END: f32 = 3.0;
pub(super) const HEADLINE_AT: f32 = 2.2;
/// The results dock; the fail moment's "TIME'S UP" and Fizzwick's line come before it.
pub(super) const DOCK_AT: f32 = 3.4;
pub(super) const FAIL_BANNER_AT: f32 = 1.2;
pub(super) const FAIL_LINE_AT: f32 = 0.4;
/// The leave for the hub: the clump climbs and shrinks over `SHRINK_SECS` and the whole exit lasts
/// `DEPART_SECS`; the drop for Free Roll takes `DROP_SECS` and the camera `RETURN_SECS`.
const SHRINK_SECS: f32 = 0.6;
const DEPART_SECS: f32 = 0.8;
const DROP_SECS: f32 = 0.5;
const RETURN_SECS: f32 = 0.6;
/// The lift in clump radii, outdoors and under a roof.
const LIFT_OUT: f32 = 1.5;
const LIFT_IN: f32 = 0.6;
/// The hover's bob (in radii), its period and a full turn every this many seconds.
const BOB: f32 = 0.15;
const BOB_PERIOD: f32 = 4.0;
const TURN_PERIOD: f32 = 14.0;
/// The three-quarter view: the lens swings this far round, at this elevation (indoors lower).
const VIEW_YAW: f32 = 35.0 * PI / 180.0;
const VIEW_PITCH: f32 = 22.0 * PI / 180.0;
const VIEW_PITCH_IN: f32 = 15.0 * PI / 180.0;
/// How much of the screen's height the clump fills, wide and compact; an indoor boom is capped at
/// this many clump extents so the lens stays in the room.
const FILL_WIDE: f32 = 0.46;
const FILL_COMPACT: f32 = 0.26;
const INDOOR_REACH: f32 = 4.2;
/// Below this aspect the screens stack (matches `select::hub`'s compact layout).
pub(super) const COMPACT_BELOW: f32 = 1.4;
/// The wish queue refuses a second exit while one runs; the finish key is ignored for this long
/// after the results open.
const BEAM_RADIUS: f32 = 1.1;
/// How much of the clump's rise the lens follows.
const FOLLOW_LIFT: f32 = 0.7;

pub(super) fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

// ---------------------------------------------------------------------------------------------
// The timeline, as pure functions of the seconds since the ending
// ---------------------------------------------------------------------------------------------

/// How far the camera has moved to the three-quarter view, 0..1.
pub(super) fn view_k(t: f32) -> f32 {
    ease(t / VIEW_SECS)
}

/// The clump's height above its resting point, in clump radii, `t` seconds in.
pub(super) fn lift_radii(t: f32, target: f32) -> f32 {
    if t < LIFT_AT {
        0.0
    } else if t < LIFT_END {
        target * ease_out((t - LIFT_AT) / (LIFT_END - LIFT_AT))
    } else {
        target + BOB * ((t - LIFT_END) * TAU / BOB_PERIOD).sin()
    }
}

/// The turn of the clump about its tilted axis, radians: one full turn on the way up, a slow one
/// after.
pub(super) fn turn_at(t: f32) -> f32 {
    if t < LIFT_AT {
        0.0
    } else if t < LIFT_END {
        TAU * ease((t - LIFT_AT) / (LIFT_END - LIFT_AT))
    } else {
        TAU + (t - LIFT_END) * TAU / TURN_PERIOD
    }
}

/// The beam's brightness, 0..1.
pub(super) fn beam_alpha(t: f32) -> f32 {
    ((t - BEAM_AT) / BEAM_FADE).clamp(0.0, 1.0)
}

/// Where the screens put things, from the aspect they are laid out for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Composition {
    pub wide: bool,
    /// The share of the screen's height the clump fills.
    pub fill: f32,
    /// Where the clump's centre sits on screen, NDC (-1..1 each way, y up).
    pub at: Vec2,
    /// The headline's anchor, as fractions of the screen from the top left.
    pub headline: Vec2,
}

pub(super) fn composition(aspect: f32) -> Composition {
    if aspect >= COMPACT_BELOW {
        Composition {
            wide: true,
            fill: FILL_WIDE,
            at: Vec2::new(-0.46, 0.10),
            headline: Vec2::new(0.27, 0.74),
        }
    } else {
        Composition {
            wide: false,
            fill: FILL_COMPACT,
            at: Vec2::new(0.0, 0.5),
            headline: Vec2::new(0.5, 0.42),
        }
    }
}

/// The lens for the three-quarter view: where it looks, from what azimuth, elevation and distance.
#[derive(Clone, Copy, Debug)]
pub(super) struct Lens {
    pub look: Vec3,
    pub az: f32,
    pub pitch: f32,
    pub dist: f32,
}

impl Lens {
    pub fn eye(&self) -> Vec3 {
        self.look
            + Vec3::new(
                self.pitch.cos() * self.az.sin(),
                self.pitch.sin(),
                self.pitch.cos() * self.az.cos(),
            ) * self.dist
    }

    /// Read a lens off a placed eye and look point.
    fn of(eye: Vec3, look: Vec3) -> Self {
        let d = eye - look;
        Self {
            look,
            az: d.x.atan2(d.z),
            pitch: d.y.atan2(Vec2::new(d.x, d.z).length()),
            dist: d.length().max(1e-3),
        }
    }

    fn lerp(&self, to: &Lens, k: f32) -> Lens {
        let mut turn = (to.az - self.az) % TAU;
        if turn > PI {
            turn -= TAU;
        } else if turn < -PI {
            turn += TAU;
        }
        Lens {
            look: self.look.lerp(to.look, k),
            az: self.az + turn * k,
            pitch: self.pitch + (to.pitch - self.pitch) * k,
            dist: self.dist + (to.dist - self.dist) * k,
        }
    }
}

/// The lens for a clump of this extent centred at `centre`, seen from azimuth `az` and elevation
/// `pitch`, for a lens with this vertical field of view: far enough that the clump fills the
/// layout's share of the height, and looking past it so it lands where the layout wants it.
pub(super) fn lens_at(
    centre: Vec3,
    extent: f32,
    az: f32,
    pitch: f32,
    fov: f32,
    aspect: f32,
    indoors: bool,
) -> Lens {
    let comp = composition(aspect);
    let half = (fov * 0.5).tan().max(0.05);
    let mut dist = extent / (comp.fill * half);
    if indoors {
        dist = dist.min(extent * INDOOR_REACH);
    }
    dist = dist.max(extent * 1.8);
    let right = Vec3::new(az.cos(), 0.0, -az.sin());
    let fwd = -Vec3::new(pitch.cos() * az.sin(), pitch.sin(), pitch.cos() * az.cos());
    let up = right.cross(fwd).normalize_or(Vec3::Y);
    let h = dist * half;
    let look = centre - right * comp.at.x * h * aspect.max(0.3) - up * comp.at.y * h;
    Lens {
        look,
        az,
        pitch,
        dist,
    }
}

/// The three-quarter view: the lens swings 35 degrees round from `az0` and rises to 22 (15
/// indoors).
pub(super) fn three_quarter(
    centre: Vec3,
    extent: f32,
    az0: f32,
    fov: f32,
    aspect: f32,
    indoors: bool,
) -> Lens {
    let pitch = if indoors { VIEW_PITCH_IN } else { VIEW_PITCH };
    lens_at(centre, extent, az0 + VIEW_YAW, pitch, fov, aspect, indoors)
}

/// The views tried indoors, best first: the three-quarter view, then the other side, then further
/// round, each at the low and a higher elevation. `(degrees from the player's view, pitch)`.
const INDOOR_VIEWS: [(f32, f32); 12] = [
    (35.0, 15.0),
    (-35.0, 15.0),
    (35.0, 30.0),
    (-35.0, 30.0),
    (75.0, 22.0),
    (-75.0, 22.0),
    (115.0, 22.0),
    (-115.0, 22.0),
    (0.0, 30.0),
    (150.0, 24.0),
    (-150.0, 24.0),
    (180.0, 30.0),
];

/// The indoor three-quarter view with a clear line to the clump: walls, stairs and shelves between
/// the lens and the clump hide it behind solid geometry, so the views are tried in order and the
/// first whose boom is free wins; if none is, the freest, with its boom cut to what it can reach.
/// `free` is how far the boom from the clump to a point may run, as a fraction of its length
/// (1.0 clear).
pub(super) fn clear_view(
    centre: Vec3,
    extent: f32,
    az0: f32,
    fov: f32,
    aspect: f32,
    free: impl Fn(Vec3, Vec3) -> f32,
) -> Lens {
    let mut best: Option<(f32, Lens)> = None;
    for (turn, pitch) in INDOOR_VIEWS {
        let lens = lens_at(
            centre,
            extent,
            az0 + turn.to_radians(),
            pitch.to_radians(),
            fov,
            aspect,
            true,
        );
        let clear = free(centre, lens.eye());
        if clear >= 0.98 {
            return lens;
        }
        if best.as_ref().is_none_or(|(c, _)| clear > *c) {
            best = Some((clear, lens));
        }
    }
    let (clear, mut lens) = best.expect("the view list is not empty");
    lens.dist = (lens.dist * clear).max(extent * 1.8);
    lens
}

// ---------------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------------

/// How a leave is going: `Hub` flies the clump up the beam, `Free` drops it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Exit {
    Hub,
    Free,
}

/// The camera and rig the player had, restored on the way out.
#[derive(Clone, Copy, Debug)]
struct Saved {
    yaw: f32,
    eye: Vec3,
    look: Vec3,
}

#[derive(Resource, Default)]
pub(super) struct Ritual {
    /// How the run ended; `None` outside the results.
    pub ending: Option<Ending>,
    /// Real seconds since the ending.
    pub t: f32,
    /// The results panel is docked.
    pub docked: bool,
    /// The try-again line of a lost run (Fizzwick says it, and the panel repeats it).
    pub fail_line: &'static str,
    said_fail: bool,
    exit: Option<(Exit, f32)>,
    /// The clump's resting centre and radius when it ended.
    rest: Vec3,
    radius: f32,
    extent: f32,
    indoors: bool,
    /// Render offsets: lift in yards and scale, applied to the ball root.
    pub lift: f32,
    /// The lift the hover settles at, yards (the lens is framed for half of it and follows the rest).
    lift_max: f32,
    pub scale: f32,
    saved: Option<Saved>,
    /// The lens at the end of the run, and the three-quarter lens it eases to.
    start: Option<Lens>,
    target: Option<Lens>,
    beam: Option<Entity>,
    beam_mat: Option<Handle<StandardMaterial>>,
    ring: Option<Entity>,
    ring_mat: Option<Handle<StandardMaterial>>,
    glint: Option<Entity>,
    glint_mat: Option<Handle<StandardMaterial>>,
    halo: Option<Entity>,
    halo_mat: Option<Handle<StandardMaterial>>,
    // One-shot cues already played.
    landed: bool,
    whooshed: bool,
    apexed: bool,
    popped: bool,
    thudded: bool,
    just_skipped: bool,
}

impl Ritual {
    /// The lens is overridden (the world's own camera placement is replaced).
    pub fn active(&self) -> bool {
        self.ending.is_some()
    }

    /// A leave is under way (further wishes are ignored).
    pub fn leaving(&self) -> bool {
        self.exit.is_some()
    }

    /// 0..1 of the results panel's slide-out during a leave.
    pub fn panel_out(&self) -> f32 {
        match self.exit {
            Some((_, e)) => (e / 0.4).clamp(0.0, 1.0),
            None => 0.0,
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Ritual>()
        .add_message::<Wish>()
        .add_message::<Cue>()
        .add_systems(
            OnEnter(Phase::Results),
            begin.after(progress::record_results),
        )
        .add_systems(OnExit(Phase::Results), teardown)
        .add_systems(OnEnter(Phase::Ready), teardown)
        .add_systems(OnEnter(Phase::Select), teardown)
        .add_systems(
            Update,
            (skip, advance, exits, apply_lift)
                .chain()
                .run_if(in_state(Phase::Results))
                .after(WorldStage::Input),
        )
        .add_systems(Update, shot.run_if(in_state(Phase::Results)).after(advance))
        .add_systems(Update, trace_children.run_if(in_state(Phase::Results)))
        .add_systems(
            Update,
            apply_camera
                .run_if(in_state(Phase::Results))
                .after(WorldStage::Input)
                .after(advance)
                .after(super::finale::animate)
                .before(super::cutout::publish)
                .before(CameraPoseSet),
        );
}

// ---------------------------------------------------------------------------------------------
// Begin and teardown
// ---------------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn begin(
    mut commands: Commands,
    mut ritual: ResMut<Ritual>,
    mut flow: ResMut<Flow>,
    run: Res<Run>,
    ball: Res<BallState>,
    rig: Res<CamRig>,
    mut time: ResMut<Time<Virtual>>,
    trophies: Query<&Attached>,
    (world, collision): (WorldPoint, WorldCollision),
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Projection, With<WorldCamera>>,
    (mut meshes, mut materials, mut images): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<Image>>,
    ),
    (mut said, mut rng, current): (ResMut<Said>, ResMut<Rng>, Res<CurrentLevel>),
    mut cues: MessageWriter<Cue>,
) {
    let ending = flow.ending.take().unwrap_or(if run.won {
        Ending::Captured
    } else {
        Ending::Failed
    });
    // The player's framing, kept for Free Roll (the finale's pull-out may already hold it).
    let (zoom, tilt) = flow.cam_before.unwrap_or((rig.zoom, rig.tilt));
    flow.cam_before = Some((zoom, tilt));
    let saved = Saved {
        yaw: rig.yaw,
        eye: rig.eye,
        look: rig.look,
    };
    // A far-reaching trophy (the Cathedral, 300 yd across, riding a 90 yd ball) must not pull the
    // lens so far out that the clump itself is a speck.
    let extent = trophies
        .iter()
        .map(|a| a.radial + a.size)
        .fold(ball.radius, f32::max)
        .min(ball.radius * 1.8);
    let indoors = world.indoors_at(ball.pos);
    let aspect = windows.single().map_or(1.0, super::select::aspect_of);
    let fov = cameras
        .iter()
        .find_map(|p| match p {
            Projection::Perspective(p) => Some(p.fov),
            _ => None,
        })
        .unwrap_or(1.0);
    let start = Lens::of(rig.eye, rig.look);
    let lift = if ending.lifted() {
        (if indoors { LIFT_IN } else { LIFT_OUT }) * ball.radius
    } else {
        0.0
    };
    // A huge clump carries its terminal trophy (the Cathedral) on top: frame that too.
    let crown = if ball.radius > 20.0 {
        0.45 * ball.radius
    } else {
        0.0
    };
    let centre = ball.pos + Vec3::Y * (lift * 0.5 + crown);
    // The lens frames the clump itself and follows most of its rise, so a long lift (a 90 yd
    // ball rises 130 yd) does not shrink it to a speck.
    let reach = extent + lift * 0.15 + crown * 0.5;
    // A huge clump is framed from no farther than the finale's own lens (6 radii): much farther
    // and the world's distance fade drops the trophies (the Cathedral) before the lens sees them.
    let far = (6.5 * ball.radius).max(60.0);
    let target = if indoors {
        // Keep the lens in the room, on the side the clump can be seen from.
        clear_view(centre, reach, start.az, fov, aspect, |from, to| {
            let boom = to - from;
            let len = boom.length().max(1.0e-3);
            collision
                .cast_camera(from, boom, false)
                .map_or(1.0, |d| (d / len).clamp(0.0, 1.0))
        })
    } else {
        let mut lens = three_quarter(centre, reach, start.az, fov, aspect, false);
        if lens.dist > far {
            // Keep the composition: the look point scales with the boom.
            let k = far / lens.dist;
            lens.look = centre + (lens.look - centre) * k;
            lens.dist = far;
        }
        lens
    };
    let mut r = Ritual {
        ending: Some(ending),
        rest: ball.pos,
        radius: ball.radius,
        extent,
        indoors,
        lift_max: lift,
        scale: 1.0,
        fail_line: if ending == Ending::Failed {
            lines::pick(&lines::TRY_AGAIN, current.0, &mut said, rng.next_u64())
        } else {
            ""
        },
        saved: Some(saved),
        start: Some(start),
        target: Some(target),
        ..default()
    };
    if ending.lifted() {
        spawn_beam(
            &mut commands,
            &mut r,
            &mut meshes,
            &mut materials,
            &mut images,
            &ball,
            extent,
            indoors,
        );
    }
    // The world stops; only the lens and the clump move from here.
    time.pause();
    cues.write(Cue::Bell);
    info!(
        "azeroll ritual: {ending:?} won={} t={:.1}s d={:.2} extent={extent:.2} indoors={indoors} aspect={aspect:.2} fov={:.2}",
        run.won,
        run.elapsed,
        ball.diameter(),
        fov
    );
    *ritual = r;
}

/// Everything the ritual changed, put back: time runs, the lens is released, the lift, the scale
/// and the beam are gone. Safe to run on any of the ways out, twice, and when nothing was begun.
#[allow(clippy::too_many_arguments)]
fn teardown(
    mut commands: Commands,
    mut ritual: ResMut<Ritual>,
    mut time: ResMut<Time<Virtual>>,
    mut rig: ResMut<CamRig>,
    mut roots: Query<&mut Transform, (With<Ball>, Without<BallSpin>)>,
) {
    time.unpause();
    time.set_relative_speed(1.0);
    if let Some(saved) = ritual.saved.take() {
        rig.yaw = saved.yaw;
        rig.look = saved.look;
        rig.eye = saved.eye;
    }
    for e in [ritual.beam, ritual.ring, ritual.glint, ritual.halo]
        .into_iter()
        .flatten()
    {
        commands.entity(e).try_despawn();
    }
    if let Ok(mut t) = roots.single_mut() {
        if t.scale != Vec3::ONE {
            t.scale = Vec3::ONE;
        }
    }
    if ritual.active() {
        info!("azeroll ritual: torn down");
    }
    *ritual = Ritual::default();
}

// ---------------------------------------------------------------------------------------------
// Per frame
// ---------------------------------------------------------------------------------------------

/// Enter, Space or Esc jump to the docked results.
fn skip(keys: Res<ButtonInput<KeyCode>>, mut ritual: ResMut<Ritual>) {
    ritual.just_skipped = false;
    if ritual.docked || ritual.exit.is_some() || ritual.ending.is_none() {
        return;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::Escape])
        && ritual.t < DOCK_AT
    {
        ritual.t = DOCK_AT;
        // The same Enter must not also leave for the hub.
        ritual.just_skipped = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn advance(
    real: Res<Time<Real>>,
    mut ritual: ResMut<Ritual>,
    mut cues: MessageWriter<Cue>,
    mut says: MessageWriter<FizzSays>,
    flow: Res<Flow>,
    mut next: ResMut<NextState<Phase>>,
) {
    let Some(ending) = ritual.ending else { return };
    let dt = real.delta_secs().min(0.1);
    ritual.t += dt;
    let t = ritual.t;
    if ending == Ending::Failed && t >= FAIL_LINE_AT && !ritual.said_fail {
        ritual.said_fail = true;
        says.write(FizzSays {
            line: ritual.fail_line,
            voice: Voice::Sigh,
        });
    }
    if ending.lifted() {
        if t >= BEAM_AT && !ritual.landed {
            ritual.landed = true;
            cues.write(Cue::Land);
        }
        if t >= LIFT_AT && !ritual.whooshed {
            ritual.whooshed = true;
            cues.write(Cue::Whoosh);
        }
        if t >= LIFT_END && !ritual.apexed {
            ritual.apexed = true;
            cues.write(Cue::Apex);
        }
    }
    if t >= DOCK_AT && !ritual.docked {
        ritual.docked = true;
        // A single-target goal's fanfare played at the capture; the rest arrive with the panel.
        if ending != Ending::Captured {
            cues.write(Cue::Fanfare);
        }
        if flow.outcome.record {
            cues.write(Cue::Record);
        }
    }
    let target = if ending.lifted() {
        if ritual.indoors {
            LIFT_IN
        } else {
            LIFT_OUT
        }
    } else {
        0.0
    };
    let r = ritual.radius;
    let mut lift = lift_radii(t, target) * r;
    let mut scale = 1.0;
    if let Some((exit, e)) = ritual.exit.as_mut() {
        *e += dt;
        let e = *e;
        match *exit {
            Exit::Hub => {
                let u = ease(e / SHRINK_SECS);
                lift = (target + 3.0 * u) * r;
                scale = 1.0 - u;
                if e >= SHRINK_SECS && !ritual.popped {
                    ritual.popped = true;
                    cues.write(Cue::StarPop);
                }
                if e >= DEPART_SECS {
                    next.set(Phase::Select);
                }
            }
            Exit::Free => {
                let u = (e / DROP_SECS).clamp(0.0, 1.0);
                lift = lift * (1.0 - u * u);
                if u >= 1.0 && !ritual.thudded {
                    ritual.thudded = true;
                    cues.write(Cue::Thud);
                }
            }
        }
    }
    ritual.lift = lift;
    ritual.scale = scale;
}

/// Turn the results' wishes into exits.
fn exits(
    keys: Res<ButtonInput<KeyCode>>,
    run: Res<Run>,
    mut ritual: ResMut<Ritual>,
    mut wishes: MessageReader<Wish>,
    mut flow: ResMut<Flow>,
    mut next: ResMut<NextState<Phase>>,
    mut says: MessageWriter<FizzSays>,
) {
    let Some(_) = ritual.ending else {
        wishes.clear();
        return;
    };
    let mut asked: Vec<Wish> = wishes.read().copied().collect();
    if keys.just_pressed(KeyCode::KeyR) {
        asked.push(Wish::Again);
    }
    if ritual.docked && ritual.exit.is_none() {
        if keys.just_pressed(KeyCode::Enter) && !ritual.just_skipped {
            asked.push(Wish::Hub);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            asked.push(Wish::Free);
        }
    }
    for wish in asked {
        match wish {
            // Restart is always allowed: Ready tears everything down.
            Wish::Again => {
                next.set(Phase::Ready);
                return;
            }
            Wish::Hub if ritual.docked && ritual.exit.is_none() => {
                ritual.exit = Some((Exit::Hub, 0.0));
                return;
            }
            Wish::Free if ritual.docked && ritual.exit.is_none() && run.won => {
                ritual.exit = Some((Exit::Free, 0.0));
                says.write(FizzSays {
                    line: lines::KEEP_IT,
                    voice: Voice::Talk,
                });
                return;
            }
            _ => {}
        }
    }
    // The drop finishes with Free Roll once the clump is down and the camera is back.
    if let Some((Exit::Free, e)) = ritual.exit {
        if e >= RETURN_SECS {
            progress::start_free_roll(&mut flow, &mut next);
        }
    }
}

/// Put the clump where the ritual says: lifted, turning, shrunken for the leave.
#[allow(clippy::type_complexity)]
fn apply_lift(
    ritual: Res<Ritual>,
    ball: Res<BallState>,
    mut roots: Query<&mut Transform, (With<Ball>, Without<BallSpin>)>,
    mut spins: Query<&mut Transform, (With<BallSpin>, Without<Ball>)>,
) {
    if ritual.ending.is_none() {
        return;
    }
    if let Ok(mut t) = roots.single_mut() {
        let pos = ball.pos + Vec3::Y * ritual.lift;
        if t.translation != pos {
            t.translation = pos;
        }
        let scale = Vec3::splat(ritual.scale.max(0.001));
        if t.scale != scale {
            t.scale = scale;
        }
    }
    if let Ok(mut t) = spins.single_mut() {
        let axis = Vec3::new(0.3, 1.0, 0.2).normalize();
        let turn = if ritual.ending.is_some_and(Ending::lifted) {
            turn_at(ritual.t)
        } else {
            0.0
        };
        t.rotation = Quat::from_axis_angle(axis, turn) * ball.spin;
    }
}

/// The lens, the beam and its glint. Runs after the world's camera placement and replaces it.
#[allow(clippy::too_many_arguments)]
fn apply_camera(
    ritual: Res<Ritual>,
    mut rig: ResMut<CamRig>,
    ball: Res<BallState>,
    real: Res<Time<Real>>,
    world: WorldPoint,
    mut cameras: Query<&mut Transform, (With<WorldCamera>, Without<BeamPart>)>,
    mut parts: Query<(&BeamPart, &mut Transform, &mut Visibility), Without<WorldCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(start), Some(target), Some(ending)) = (ritual.start, ritual.target, ritual.ending)
    else {
        return;
    };
    // The lens eases out, and back for the drop.
    let mut k = view_k(ritual.t);
    if let Some((Exit::Free, e)) = ritual.exit {
        k *= 1.0 - ease(e / RETURN_SECS);
    }
    let mut lens = start.lerp(&target, k);
    lens.look.y += FOLLOW_LIFT * (ritual.lift - ritual.lift_max * 0.5) * k;
    let mut eye = lens.eye();
    if !ritual.indoors {
        if let Some(h) = world.terrain_height_under(eye) {
            eye.y = eye.y.max(h + 0.35 + 0.2 * ritual.radius);
        }
    }
    rig.eye = eye;
    rig.look = lens.look;
    let placed = Transform::from_translation(eye).looking_at(lens.look, Vec3::Y);
    let cam_rot = placed.rotation;
    for mut cam in &mut cameras {
        *cam = placed;
    }

    // The beam, its ground ring and the departure glint.
    let t = ritual.t;
    let alpha = if ending.lifted() { beam_alpha(t) } else { 0.0 };
    let gone = match ritual.exit {
        Some((Exit::Hub, e)) => {
            1.0 - ((e - SHRINK_SECS) / (DEPART_SECS - SHRINK_SECS)).clamp(0.0, 1.0)
        }
        Some((Exit::Free, e)) => 1.0 - (e / 0.1).clamp(0.0, 1.0),
        None => 1.0,
    };
    let a = alpha * gone;
    if let Some(mat) = ritual.beam_mat.as_ref().and_then(|h| materials.get_mut(h)) {
        // Outdoors the daylit trophies are pale (the Cathedral's stone): a light beam must not
        // glare them out.
        let k = if ritual.indoors { 0.55 } else { 0.32 };
        mat.base_color = Color::srgba(0.6, 0.82, 1.0, k * a);
        mat.uv_transform =
            bevy::math::Affine2::from_translation(Vec2::new(0.0, -real.elapsed_secs() * 0.45));
    }
    let ring_k = ((t - BEAM_AT) / 0.9).clamp(0.0, 1.0);
    if let Some(mat) = ritual.ring_mat.as_ref().and_then(|h| materials.get_mut(h)) {
        mat.base_color = Color::srgba(
            0.85,
            0.95,
            1.0,
            0.7 * (1.0 - ring_k) * gone * f32::from(u8::from(t >= BEAM_AT)),
        );
    }
    let halo_a = a * if ritual.indoors { 0.62 } else { 0.1 };
    if let Some(mat) = ritual.halo_mat.as_ref().and_then(|h| materials.get_mut(h)) {
        mat.base_color = Color::srgba(1.0, 0.82, 0.5, halo_a);
    }
    let glint_k = match ritual.exit {
        Some((Exit::Hub, e)) if e >= SHRINK_SECS => {
            ((e - SHRINK_SECS) / (DEPART_SECS - SHRINK_SECS)).clamp(0.0, 1.0)
        }
        _ => -1.0,
    };
    if let Some(mat) = ritual.glint_mat.as_ref().and_then(|h| materials.get_mut(h)) {
        let fade = if glint_k < 0.0 {
            0.0
        } else {
            (1.0 - glint_k * glint_k).max(0.0)
        };
        mat.base_color = Color::srgba(1.0, 0.92, 0.55, fade);
    }
    let r = ritual.radius;
    let ext = ritual.extent;
    let ritual_centre = ball.pos + Vec3::Y * ritual.lift;
    let rig_eye = eye;
    let top = if ritual.indoors {
        7.0
    } else {
        60.0 + 20.0 * ext
    };
    let base_y = ritual.rest.y - r;
    for (part, mut tf, mut vis) in &mut parts {
        match part {
            BeamPart::Beam => {
                let radius = (BEAM_RADIUS * ext).max(r * 1.1);
                tf.translation = Vec3::new(ball.pos.x, base_y + top * 0.5, ball.pos.z);
                tf.scale = Vec3::new(radius, top, radius);
                *vis = if a > 0.01 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            BeamPart::Ring => {
                let radius = (BEAM_RADIUS * ext).max(r * 1.1) * (1.0 + 2.2 * ring_k);
                tf.translation = Vec3::new(ball.pos.x, base_y + 0.04, ball.pos.z);
                tf.rotation = Quat::from_rotation_x(-PI * 0.5);
                tf.scale = Vec3::splat(radius);
                *vis = if t >= BEAM_AT && ring_k < 1.0 && gone > 0.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            BeamPart::Halo => {
                // Behind the clump, on the far side from the lens, so it rims it with light.
                let away = (ritual_centre - rig_eye).normalize_or(Vec3::NEG_Z);
                tf.translation = ritual_centre + away * ext * 0.7;
                tf.rotation = cam_rot;
                tf.scale = Vec3::splat(ext * 4.6);
                *vis = if halo_a > 0.01 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            BeamPart::Glint => {
                let size = r * (1.0 + 2.5 * glint_k.max(0.0));
                tf.translation = ball.pos + Vec3::Y * (ritual.lift + 0.2 * r);
                tf.rotation = cam_rot;
                tf.scale = Vec3::splat(size * 2.0);
                *vis = if glint_k >= 0.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

/// `WOW_AZEROLL_RITUAL_TRACE=1`: once a second in the results, what rides the clump and whether it
/// is visible (the Cathedral's root is the one with a big scale).
#[allow(clippy::type_complexity)]
fn trace_children(
    time: Res<Time<Real>>,
    mut at: Local<f32>,
    spin: Query<&Children, With<BallSpin>>,
    kids: Query<(
        &Visibility,
        &InheritedVisibility,
        &ViewVisibility,
        &Transform,
        Option<&Children>,
    )>,
) {
    if crate::webenv::var("WOW_AZEROLL_RITUAL_TRACE").is_none() {
        return;
    }
    *at += time.delta_secs();
    if *at < 1.0 {
        return;
    }
    *at = 0.0;
    let Ok(children) = spin.single() else { return };
    for c in children.iter() {
        if let Ok((v, iv, vv, tf, ch)) = kids.get(c) {
            if tf.scale.x > 0.04 && tf.scale.x != 1.0 || ch.is_some_and(|c| c.len() > 20) {
                info!(
                    "azeroll ritual trace: {c:?} vis={v:?} inherited={} view={} scale={:?} children={}",
                    iv.get(),
                    vv.get(),
                    tf.scale,
                    ch.map_or(0, |c| c.len())
                );
            }
        }
    }
}

/// `WOW_AZEROLL_RITUAL_SHOT=<file.png>` (a capture aid) with `WOW_AZEROLL_RITUAL_SHOT_AT=0.8,2.0,3.4`
/// (real seconds from the ending; default 3.4 and the dock plus 1.5): save a frame at each, as
/// `<file>-<at>.png` when there are several, and exit half a second after the last.
fn shot(
    mut commands: Commands,
    ritual: Res<Ritual>,
    mut taken: Local<usize>,
    mut departed: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = crate::webenv::var("WOW_AZEROLL_RITUAL_SHOT")
        .or_else(|| crate::webenv::var("WOW_AZEROLL_RITUAL_DEPART_SHOT").map(|_| String::new()))
    else {
        return;
    };
    if ritual.ending.is_none() {
        return;
    }
    // `WOW_AZEROLL_RITUAL_DEPART_SHOT=<png>`: one frame half way through the departure.
    if let (Some(depart), Some((Exit::Hub, e))) = (
        crate::webenv::var("WOW_AZEROLL_RITUAL_DEPART_SHOT"),
        ritual.exit,
    ) {
        if e >= 0.3 && !*departed {
            *departed = true;
            use bevy::render::view::screenshot::{save_to_disk, Screenshot};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(std::path::PathBuf::from(depart)));
        }
    }
    if path.is_empty() {
        return;
    }
    let ats: Vec<f32> = crate::webenv::var("WOW_AZEROLL_RITUAL_SHOT_AT")
        .map(|v| v.split(',').filter_map(|a| a.trim().parse().ok()).collect())
        .filter(|v: &Vec<f32>| !v.is_empty())
        .unwrap_or_else(|| vec![DOCK_AT + 1.5]);
    let Some(&at) = ats.get(*taken) else {
        // Left to run on when the results are being shot too (`select::verdict`'s own exit).
        if ritual.t > ats[ats.len() - 1] + 0.5
            && crate::webenv::var("WOW_AZEROLL_RESULT_SHOT").is_none()
        {
            exit.write(AppExit::Success);
        }
        return;
    };
    if ritual.t >= at {
        let file = if ats.len() == 1 {
            std::path::PathBuf::from(&path)
        } else {
            let base = path.trim_end_matches(".png");
            std::path::PathBuf::from(format!("{base}-{at:.1}.png"))
        };
        use bevy::render::view::screenshot::{save_to_disk, Screenshot};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(file));
        *taken += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// The beam
// ---------------------------------------------------------------------------------------------

#[derive(Component, Clone, Copy)]
enum BeamPart {
    Beam,
    Ring,
    Glint,
    /// A warm glow behind the clump, so it reads against a dark room.
    Halo,
}

/// A soft vertical column's texture: bright in the middle, streaky, fading at the sides.
fn beam_texture() -> Image {
    const W: u32 = 32;
    const H: u32 = 64;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let u = (x as f32 + 0.5) / W as f32;
            let v = (y as f32 + 0.5) / H as f32;
            let edge = (1.0 - (2.0 * u - 1.0).abs()).clamp(0.0, 1.0);
            let soft = edge.powf(0.7);
            let streak = 0.62 + 0.38 * ((u * 9.0 + (v * TAU * 2.0).sin() * 0.6) * PI).sin().powi(2);
            let core = 0.55 + 0.45 * (1.0 - (2.0 * u - 1.0).abs()).powi(3);
            let a = (soft * streak * core).clamp(0.0, 1.0);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::ClampToEdge,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    });
    img
}

/// A four-point sparkle on a soft glow.
fn glint_texture() -> Image {
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let px = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let py = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let r = (px * px + py * py).sqrt();
            let star = (1.0 - (px.abs().sqrt() + py.abs().sqrt())).max(0.0).powi(2);
            let glow = (1.0 - r).max(0.0).powi(3) * 0.8;
            let a = (star + glow).clamp(0.0, 1.0);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// A soft round glow.
fn halo_texture() -> Image {
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let px = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let py = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let r = (px * px + py * py).sqrt();
            let a = (1.0 - r).max(0.0).powf(1.6);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// An open-ended unit cylinder (radius 1, height 1, centred): the beam's body.
fn open_cylinder() -> Mesh {
    const SEG: u32 = 40;
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut nor = Vec::new();
    let mut idx = Vec::new();
    for i in 0..=SEG {
        let u = i as f32 / SEG as f32;
        let a = u * TAU;
        let (s, c) = a.sin_cos();
        pos.push([c, -0.5, s]);
        pos.push([c, 0.5, s]);
        uv.push([u, 1.0]);
        uv.push([u, 0.0]);
        nor.push([c, 0.0, s]);
        nor.push([c, 0.0, s]);
    }
    for i in 0..SEG {
        let a = i * 2;
        idx.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

#[allow(clippy::too_many_arguments)]
fn spawn_beam(
    commands: &mut Commands,
    r: &mut Ritual,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    ball: &BallState,
    extent: f32,
    indoors: bool,
) {
    let additive = |tex: Option<Handle<Image>>, c: Color| StandardMaterial {
        base_color: c,
        base_color_texture: tex,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    };
    let beam_tex = images.add(beam_texture());
    let beam_mat = materials.add(additive(Some(beam_tex), Color::srgba(0.72, 0.88, 1.0, 0.0)));
    let radius = (BEAM_RADIUS * extent).max(ball.radius * 1.1);
    let top = if indoors { 7.0 } else { 60.0 + 20.0 * extent };
    let base_y = ball.pos.y - ball.radius;
    let beam = commands
        .spawn((
            RunEntity,
            BeamPart::Beam,
            Mesh3d(meshes.add(open_cylinder())),
            MeshMaterial3d(beam_mat.clone()),
            Transform::from_translation(Vec3::new(ball.pos.x, base_y + top * 0.5, ball.pos.z))
                .with_scale(Vec3::new(radius, top, radius)),
            Visibility::Hidden,
        ))
        .id();
    let ring_mat = materials.add(additive(None, Color::srgba(0.85, 0.95, 1.0, 0.0)));
    let ring = commands
        .spawn((
            RunEntity,
            BeamPart::Ring,
            Mesh3d(meshes.add(Annulus::new(0.82, 1.0))),
            MeshMaterial3d(ring_mat.clone()),
            Transform::from_translation(Vec3::new(ball.pos.x, base_y + 0.04, ball.pos.z))
                .with_rotation(Quat::from_rotation_x(-PI * 0.5))
                .with_scale(Vec3::splat(radius)),
            Visibility::Hidden,
        ))
        .id();
    let glint_tex = images.add(glint_texture());
    let glint_mat = materials.add(additive(
        Some(glint_tex),
        Color::srgba(1.0, 0.92, 0.55, 0.0),
    ));
    let glint = commands
        .spawn((
            RunEntity,
            BeamPart::Glint,
            Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial3d(glint_mat.clone()),
            Transform::from_translation(ball.pos),
            Visibility::Hidden,
        ))
        .id();
    let halo_tex = images.add(halo_texture());
    let halo_mat = materials.add(additive(Some(halo_tex), Color::srgba(1.0, 0.82, 0.5, 0.0)));
    let halo = commands
        .spawn((
            RunEntity,
            BeamPart::Halo,
            Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial3d(halo_mat.clone()),
            Transform::from_translation(ball.pos),
            Visibility::Hidden,
        ))
        .id();
    r.halo = Some(halo);
    r.halo_mat = Some(halo_mat);
    r.beam = Some(beam);
    r.beam_mat = Some(beam_mat);
    r.ring = Some(ring);
    r.ring_mat = Some(ring_mat);
    r.glint = Some(glint);
    r.landed = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clump_rests_then_lifts_then_hovers() {
        assert_eq!(lift_radii(0.0, 1.5), 0.0);
        assert_eq!(lift_radii(LIFT_AT, 1.5), 0.0);
        let mid = lift_radii(2.0, 1.5);
        assert!(mid > 0.7 && mid < 1.5, "{mid}");
        assert!((lift_radii(LIFT_END, 1.5) - 1.5).abs() < 1e-3);
        // The hover bobs a little round the top and never sinks below the start of the bob.
        for i in 0..200 {
            let l = lift_radii(LIFT_END + i as f32 * 0.1, 1.5);
            assert!((l - 1.5).abs() <= BOB + 1e-3);
        }
        // A lost run has no lift at all.
        assert_eq!(lift_radii(5.0, 0.0).abs() <= BOB, true);
    }

    #[test]
    fn the_clump_turns_a_full_turn_on_the_way_up() {
        assert_eq!(turn_at(0.5), 0.0);
        assert!((turn_at(LIFT_END) - TAU).abs() < 1e-3);
        assert!(turn_at(LIFT_END + 7.0) > TAU);
    }

    #[test]
    fn the_beam_lands_after_the_camera_starts_and_the_panel_after_the_headline() {
        assert_eq!(beam_alpha(BEAM_AT - 0.01), 0.0);
        assert!((beam_alpha(BEAM_AT + BEAM_FADE) - 1.0).abs() < 1e-5);
        assert!((view_k(VIEW_SECS) - 1.0).abs() < 1e-5);
        assert!(BEAM_AT < LIFT_AT && LIFT_AT < LIFT_END);
        assert!(HEADLINE_AT < DOCK_AT && DOCK_AT > LIFT_END);
        // The whole send-off is the spec's 3.4 s.
        assert!((DOCK_AT - 3.4).abs() < 1e-6);
    }

    #[test]
    fn wide_puts_the_clump_left_and_compact_puts_it_high() {
        let w = composition(1.78);
        assert!(w.wide && w.at.x < -0.3);
        let c = composition(0.92);
        assert!(!c.wide && c.at.y > 0.3 && c.at.x == 0.0);
        assert!(c.fill < w.fill);
    }

    #[test]
    fn the_three_quarter_lens_turns_the_view_and_stays_in_the_room_indoors() {
        let centre = Vec3::new(10.0, 3.0, -4.0);
        let open = three_quarter(centre, 2.0, 0.5, 1.0, 1.78, false);
        assert!((open.az - 0.5 - VIEW_YAW).abs() < 1e-5);
        assert!((open.pitch - VIEW_PITCH).abs() < 1e-5);
        let indoors = three_quarter(centre, 2.0, 0.5, 1.0, 1.78, true);
        assert!(indoors.dist <= 2.0 * INDOOR_REACH + 1e-4);
        assert!(indoors.pitch < open.pitch);
        // The clump's centre lies left of the screen's middle in the wide layout.
        let eye = open.eye();
        let fwd = (open.look - eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        assert!((centre - open.look).dot(right) < 0.0);
    }

    #[test]
    fn an_indoor_view_avoids_what_blocks_the_boom() {
        let centre = Vec3::new(0.0, 2.0, 0.0);
        let az0 = 0.3;
        // Nothing in the way: the plain three-quarter view.
        let open = clear_view(centre, 2.0, az0, 0.8, 0.92, |_, _| 1.0);
        let plain = three_quarter(centre, 2.0, az0, 0.8, 0.92, true);
        assert!((open.az - plain.az).abs() < 1e-5 && (open.pitch - plain.pitch).abs() < 1e-5);
        // A wall on the +35 degree side: the other side is taken.
        let wall_az = az0 + VIEW_YAW;
        let side = clear_view(centre, 2.0, az0, 0.8, 0.92, |from, to| {
            let d = to - from;
            let az = d.x.atan2(d.z);
            if (az - wall_az).abs() < 0.2 {
                0.3
            } else {
                1.0
            }
        });
        assert!((side.az - (az0 - VIEW_YAW)).abs() < 1e-4, "{}", side.az);
        // Walled in on every side: the freest view, with its boom cut short but never inside the
        // clump.
        let boxed = clear_view(centre, 2.0, az0, 0.8, 0.92, |_, _| 0.5);
        assert!(boxed.dist <= plain.dist * 0.5 + 1e-4 || boxed.dist >= 2.0 * 1.8 - 1e-4);
        assert!(boxed.dist >= 2.0 * 1.8 - 1e-4);
    }

    #[test]
    fn a_lens_round_trips_through_an_eye_and_look() {
        let lens = Lens {
            look: Vec3::new(1.0, 2.0, 3.0),
            az: 1.1,
            pitch: 0.4,
            dist: 6.0,
        };
        let back = Lens::of(lens.eye(), lens.look);
        assert!((back.az - lens.az).abs() < 1e-4);
        assert!((back.pitch - lens.pitch).abs() < 1e-4);
        assert!((back.dist - lens.dist).abs() < 1e-4);
    }

    #[test]
    fn the_azimuth_takes_the_short_way_round() {
        let a = Lens {
            look: Vec3::ZERO,
            az: 3.0,
            pitch: 0.3,
            dist: 4.0,
        };
        let b = Lens { az: -3.0, ..a };
        let half = a.lerp(&b, 0.5);
        // Halfway between +3 and -3 rad the short way is just past pi, not zero.
        assert!(half.az.abs() > 3.0, "{}", half.az);
    }

    #[test]
    fn teardown_restores_time_the_lens_and_the_beam_from_any_stage() {
        use bevy::ecs::system::RunSystemOnce;
        for stage in 0..4 {
            let mut world = World::new();
            let mut time = Time::<Virtual>::default();
            time.pause();
            time.set_relative_speed(0.2);
            world.insert_resource(time);
            world.insert_resource(CamRig::default());
            let mut r = Ritual {
                ending: Some(Ending::Snuff),
                t: [0.2, 1.5, 3.2, 3.6][stage],
                docked: stage > 1,
                scale: 0.4,
                lift: 2.0,
                saved: Some(Saved {
                    yaw: 0.3,
                    eye: Vec3::X,
                    look: Vec3::Y,
                }),
                ..default()
            };
            if stage == 3 {
                r.exit = Some((Exit::Hub, 0.5));
            }
            world.insert_resource(r);
            let root = world
                .spawn((Ball, Transform::from_scale(Vec3::splat(0.4))))
                .id();
            world.run_system_once(teardown).unwrap();
            assert!(
                !world.resource::<Time<Virtual>>().is_paused(),
                "stage {stage}"
            );
            assert_eq!(world.resource::<Time<Virtual>>().relative_speed(), 1.0);
            let r = world.resource::<Ritual>();
            assert!(!r.active() && r.lift == 0.0 && r.beam.is_none() && !r.leaving());
            assert_eq!(world.resource::<CamRig>().yaw, 0.3);
            assert_eq!(world.get::<Transform>(root).unwrap().scale, Vec3::ONE);
        }
    }
}
