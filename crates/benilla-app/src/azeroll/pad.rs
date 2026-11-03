//! The gamepad (D32): Katamari's twin-stick scheme, the dash wind-up, and menu input.
//!
//! Everything here is *input*. [`read_pad`] runs in `PreUpdate`, after Bevy has folded the
//! gamepads' state in, and writes two resources: [`PadInput`] (what the ball and the camera read in
//! `Update`, beside the keyboard) and, through the same `ButtonInput<KeyCode>` the keyboard fills,
//! the key-equivalents that drive the hub, pause and results. So no physics lives here, and the
//! menus' own files know nothing of a pad.
//!
//! The maths is plain functions over plain values ([`twin_stick`], [`Wag`], [`DashCharge`]) so that
//! a machine without a pad can test it; `WOW_AZEROLL_FAKE_PAD` feeds the whole path synthetic
//! sticks, too.
//!
//! # Controls
//!
//! | | |
//! |---|---|
//! | both sticks up / down | roll forward / back; the mean of the two sticks is the speed |
//! | one up, one down | spin on the spot (tank); one stick only rolls and curves |
//! | both sticks sideways | strafe roll at 55 % of top speed |
//! | alternate the sticks (up/down) 3 times in a second | dash: wind-up, then the burst |
//! | `A` (hold, release) | the dash as a button, like Shift |
//! | `X`, or click both sticks | quick turn (Space) |
//! | `L1` / `R1` | swing the camera (Q / E); d-pad up / down zooms; `L2` / `R2` peeks from above |
//! | `Start` | pause; `Y` held calls a won size run (Enter); `Select` held unsticks (Backspace) |
//!
//! "Simple mode" (`Select` in the pause menu, or `WOW_AZEROLL_PAD_MODE=simple`): the left stick
//! steers like a car and the right stick works the camera.

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::InputSystems;
use bevy::prelude::*;

use super::Phase;

/// Inside this radius a stick reads zero; past it the throw is rescaled to start from zero.
const STICK_DEADZONE: f32 = 0.18;
/// The tank's turn input is ignored below this, so two sticks held "level" roll straight.
const TURN_DEADZONE: f32 = 0.12;
/// Both sticks must reach this far sideways before the roll strafes.
const STRAFE_DEADZONE: f32 = 0.3;
/// Sideways roll against the forward roll, as in Katamari.
pub(super) const STRAFE_SCALE: f32 = 0.55;
/// A stick this far up or down counts as "pushed" for the dash gesture.
const WAG_PUSH: f32 = 0.5;
/// Pushes (alternations) needed within [`WAG_WINDOW`] seconds.
const WAG_PUSHES: usize = 3;
const WAG_WINDOW: f32 = 1.0;
/// The gesture stays engaged this long after its last push; stopping it releases the dash.
const WAG_HOLD: f32 = 0.45;
/// A left stick this far from centre acts as a d-pad press in the menus.
const NAV_THRESHOLD: f32 = 0.6;

/// A dash held no longer than this is a tap: the dash it always was, with no wind-up.
pub(super) const TAP_SECS: f32 = 0.12;
/// The wind-up completes, and bursts by itself, after this long.
pub(super) const FULL_SECS: f32 = 0.45;
/// The ball's top speed while it winds up (the Prince holds it back).
pub(super) const WINDUP_SLOW: f32 = 0.25;
/// A fully charged dash lasts this much longer, and pulls this much harder.
pub(super) const BURST_LONGER: f32 = 0.45;
pub(super) const BURST_PULL: f32 = 0.6;

/// What the ball and the camera read from the pad, once per frame.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PadInput {
    pub forward: f32,
    pub turn: f32,
    pub strafe: f32,
    /// A (after it was let go in play) or the stick gesture.
    pub dash_held: bool,
    /// One frame: X, or both sticks clicked.
    pub quick_turn: bool,
    /// Camera swing, -1..1 (the Q / E of the keyboard).
    pub orbit: f32,
    pub tilt: f32,
    pub zoom: f32,
    pub peek: bool,
    /// Any stick past its deadzone or any button down.
    pub active: bool,
}

/// The device the player last used, which the on-screen prompts follow ("hold Enter" or "hold Y").
/// Pad activity (a stick past its deadzone or any button) makes it `Pad`; a real key or a mouse
/// button makes it `Keys`. `WOW_AZEROLL_FINISH_PROMPT=pad|keys` pins it, for captures.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum LastDevice {
    #[default]
    Keys,
    Pad,
}

/// Where the sticks are, cleaned: radial deadzone, rescaled to start from zero at its edge.
pub(super) fn stick(raw: Vec2) -> Vec2 {
    let m = raw.length();
    if !m.is_finite() || m <= STICK_DEADZONE {
        return Vec2::ZERO;
    }
    raw / m * ((m - STICK_DEADZONE) / (1.0 - STICK_DEADZONE)).min(1.0)
}

fn rescale(v: f32, dead: f32) -> f32 {
    if v.abs() <= dead {
        0.0
    } else {
        v.signum() * ((v.abs() - dead) / (1.0 - dead)).min(1.0)
    }
}

/// The ball's three inputs: roll (-1..1), the heading's turn (+ is right) and the sideways roll
/// (+ is right).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Drive {
    pub forward: f32,
    pub turn: f32,
    pub strafe: f32,
}

/// Katamari's controls: each stick is a track of the tank. Both up rolls, one up and one down
/// spins, both out to the same side strafes. `left` and `right` are [`stick`] outputs, y up.
pub(super) fn twin_stick(left: Vec2, right: Vec2) -> Drive {
    let forward = (left.y + right.y) * 0.5;
    let turn = rescale((left.y - right.y) * 0.5, TURN_DEADZONE);
    let strafe = if left.x * right.x > 0.0 {
        rescale(
            left.x.signum() * left.x.abs().min(right.x.abs()),
            STRAFE_DEADZONE,
        )
    } else {
        0.0
    };
    Drive {
        forward,
        turn,
        strafe,
    }
}

/// "Simple mode": the left stick steers like a car.
pub(super) fn simple_stick(left: Vec2) -> Drive {
    Drive {
        forward: left.y,
        turn: rescale(left.x, TURN_DEADZONE),
        strafe: 0.0,
    }
}

/// The dash gesture: the sticks pushed opposite ways, up/down, and swapped over and over. Three
/// pushes within a second engage it; it stays engaged while the waggle goes on, and lets go
/// [`WAG_HOLD`] after the last push (which is the release that bursts).
#[derive(Debug, Default, Clone)]
pub(super) struct Wag {
    side: i8,
    pushes: Vec<f32>,
    engaged: bool,
}

impl Wag {
    pub(super) fn update(&mut self, now: f32, left_y: f32, right_y: f32) -> bool {
        let side = if left_y >= WAG_PUSH && right_y <= -WAG_PUSH {
            1
        } else if left_y <= -WAG_PUSH && right_y >= WAG_PUSH {
            -1
        } else {
            0
        };
        if side != 0 && side != self.side {
            self.pushes.push(now);
        }
        if side != 0 {
            self.side = side;
        }
        self.pushes.retain(|t| now - t <= WAG_WINDOW);
        let last = self.pushes.last().copied();
        if self.engaged {
            if last.is_none_or(|t| now - t > WAG_HOLD) {
                self.engaged = false;
                self.pushes.clear();
                self.side = 0;
            }
        } else if self.pushes.len() >= WAG_PUSHES {
            self.engaged = true;
        }
        self.engaged
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// What a [`DashCharge::step`] asks the ball to do.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct Charged {
    /// The wind-up proper has just begun: tell the Prince and the speakers.
    pub windup: bool,
    /// Release or completion: burst, with this charge (0 for a tap .. 1 for a full wind-up).
    pub burst: Option<f32>,
}

/// The dash as a state machine: press arms it, holding past [`TAP_SECS`] winds up (the ball is
/// held back, the Prince scrambles), release, or [`FULL_SECS`], bursts. A press during the
/// cooldown does nothing, and a held press that has burst must be let go before it arms again.
#[derive(Resource, Debug, Default, Clone)]
pub(super) struct DashCharge {
    held_for: Option<f32>,
    announced: bool,
    latched: bool,
    /// The charge of the dash that is running, for the ball's pull.
    pub boost: f32,
    /// The burst had no direction to go in: roll straight ahead until it ends.
    pub forced: bool,
}

impl DashCharge {
    /// One frame. `held`: the dash input is down; `can_start`: the cooldown is over and the ball is
    /// free to dash; `alive`: the game is being played (anything else cancels a wind-up).
    pub(super) fn step(&mut self, dt: f32, held: bool, can_start: bool, alive: bool) -> Charged {
        let mut out = Charged::default();
        if !alive {
            *self = Self::default();
            return out;
        }
        if !held {
            self.latched = false;
        }
        match self.held_for {
            Some(t) => {
                let t = t + dt;
                self.held_for = Some(t);
                if !held || t >= FULL_SECS {
                    out.burst = Some(charge_of(t));
                    self.held_for = None;
                    self.announced = false;
                    self.latched = held;
                } else if !self.announced && t >= TAP_SECS {
                    self.announced = true;
                    out.windup = true;
                }
            }
            None if held && !self.latched && can_start => {
                self.held_for = Some(0.0);
                self.latched = true;
            }
            None => {}
        }
        out
    }

    /// The wind-up proper is under way: the ball is held back.
    pub(super) fn winding(&self) -> bool {
        self.held_for.is_some_and(|t| t >= TAP_SECS)
    }
}

/// How much of a wind-up a hold of `t` seconds is: 0 for a tap, 1 for the full wind-up.
pub(super) fn charge_of(t: f32) -> f32 {
    ((t - TAP_SECS) / (FULL_SECS - TAP_SECS)).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------------------------

/// A set of pad buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Buttons(pub u32);

impl Buttons {
    pub(super) const A: u32 = 1 << 0;
    pub(super) const B: u32 = 1 << 1;
    pub(super) const X: u32 = 1 << 2;
    pub(super) const Y: u32 = 1 << 3;
    pub(super) const START: u32 = 1 << 4;
    pub(super) const SELECT: u32 = 1 << 5;
    pub(super) const L1: u32 = 1 << 6;
    pub(super) const R1: u32 = 1 << 7;
    pub(super) const L2: u32 = 1 << 8;
    pub(super) const R2: u32 = 1 << 9;
    pub(super) const L3: u32 = 1 << 10;
    pub(super) const R3: u32 = 1 << 11;
    pub(super) const UP: u32 = 1 << 12;
    pub(super) const DOWN: u32 = 1 << 13;
    pub(super) const LEFT: u32 = 1 << 14;
    pub(super) const RIGHT: u32 = 1 << 15;

    fn has(self, mask: u32) -> bool {
        self.0 & mask == mask
    }
    fn any(self, mask: u32) -> bool {
        self.0 & mask != 0
    }
}

const BUTTONS: [(GamepadButton, u32); 16] = [
    (GamepadButton::South, Buttons::A),
    (GamepadButton::East, Buttons::B),
    (GamepadButton::West, Buttons::X),
    (GamepadButton::North, Buttons::Y),
    (GamepadButton::Start, Buttons::START),
    (GamepadButton::Select, Buttons::SELECT),
    (GamepadButton::LeftTrigger, Buttons::L1),
    (GamepadButton::RightTrigger, Buttons::R1),
    (GamepadButton::LeftTrigger2, Buttons::L2),
    (GamepadButton::RightTrigger2, Buttons::R2),
    (GamepadButton::LeftThumb, Buttons::L3),
    (GamepadButton::RightThumb, Buttons::R3),
    (GamepadButton::DPadUp, Buttons::UP),
    (GamepadButton::DPadDown, Buttons::DOWN),
    (GamepadButton::DPadLeft, Buttons::LEFT),
    (GamepadButton::DPadRight, Buttons::RIGHT),
];

/// One reading of a pad.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Frame {
    pub left: Vec2,
    pub right: Vec2,
    pub buttons: Buttons,
}

impl Frame {
    fn from_pad(pad: &Gamepad) -> Self {
        let mut buttons = 0;
        for (button, bit) in BUTTONS {
            if pad.pressed(button) {
                buttons |= bit;
            }
        }
        Self {
            left: pad.left_stick(),
            right: pad.right_stick(),
            buttons: Buttons(buttons),
        }
    }

    fn activity(&self) -> f32 {
        stick(self.left).length() + stick(self.right).length() + self.buttons.0.count_ones() as f32
    }
}

/// Which screen the menus' key-equivalents are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ctx {
    Hub,
    Ready,
    Playing,
    Paused,
    Results,
}

impl Ctx {
    fn of(phase: Phase) -> Self {
        match phase {
            Phase::Select => Ctx::Hub,
            Phase::Ready => Ctx::Ready,
            Phase::Playing => Ctx::Playing,
            Phase::Paused => Ctx::Paused,
            Phase::Results => Ctx::Results,
        }
    }
}

/// The key a button stands for on this screen. In play, only the few that have no stick or
/// shoulder job: pause, "call it" and unstick.
pub(super) fn menu_key(ctx: Ctx, button: u32) -> Option<KeyCode> {
    use Buttons as B;
    let nav = match button {
        B::UP => Some(KeyCode::ArrowUp),
        B::DOWN => Some(KeyCode::ArrowDown),
        B::LEFT => Some(KeyCode::ArrowLeft),
        B::RIGHT => Some(KeyCode::ArrowRight),
        _ => None,
    };
    match (ctx, button) {
        (Ctx::Hub, _) if nav.is_some() => nav,
        (Ctx::Hub, B::A | B::START) => Some(KeyCode::Enter),
        (Ctx::Hub, B::Y) => Some(KeyCode::KeyF),
        (Ctx::Hub, B::X) => Some(KeyCode::KeyC),
        (Ctx::Hub, B::B) => Some(KeyCode::Escape),
        (Ctx::Ready, B::A | B::START) => Some(KeyCode::Enter),
        (Ctx::Playing, B::START) => Some(KeyCode::Escape),
        (Ctx::Playing, B::Y) => Some(KeyCode::Enter),
        (Ctx::Playing, B::SELECT) => Some(KeyCode::Backspace),
        (Ctx::Paused, B::START | B::B | B::A) => Some(KeyCode::Escape),
        (Ctx::Paused, B::X) => Some(KeyCode::KeyR),
        (Ctx::Paused, B::Y) => Some(KeyCode::KeyL),
        (Ctx::Results, B::A | B::START) => Some(KeyCode::Enter),
        (Ctx::Results, B::B) => Some(KeyCode::Escape),
        (Ctx::Results, B::X) => Some(KeyCode::KeyR),
        (Ctx::Results, B::Y) => Some(KeyCode::KeyF),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------------------------------

/// What carries from frame to frame.
#[derive(Resource, Debug, Default)]
pub(super) struct PadState {
    prev: Buttons,
    wag: Wag,
    /// "Simple mode": the left stick steers like a car, the right works the camera.
    pub simple: bool,
    /// The dash button counts only once it has been let go since play began (so the A that
    /// resumed from pause, or started the run, does not also wind a dash up).
    dash_armed: bool,
    keys: Vec<(u32, KeyCode)>,
    pulsing: bool,
}

/// One frame of pad: `now` is a monotonic clock in seconds.
pub(super) fn derive(frame: Frame, st: &mut PadState, now: f32, ctx: Ctx) -> PadInput {
    let left = stick(frame.left);
    let right = stick(frame.right);
    let b = frame.buttons;
    let drive = if st.simple {
        simple_stick(left)
    } else {
        twin_stick(left, right)
    };
    let waggling = !st.simple && st.wag.update(now, left.y, right.y);
    let playing = ctx == Ctx::Playing;
    if !playing {
        st.dash_armed = !b.any(Buttons::A);
    } else if !b.any(Buttons::A) {
        st.dash_armed = true;
    }
    let dash_button = b.any(Buttons::A) && st.dash_armed;
    let clicked = b.has(Buttons::L3 | Buttons::R3);
    let clicked_before = st.prev.has(Buttons::L3 | Buttons::R3);
    let quick_turn =
        (clicked && !clicked_before) || (b.any(Buttons::X) && !st.prev.any(Buttons::X));
    let (orbit, tilt) = if st.simple {
        (right.x, right.y)
    } else {
        (0.0, 0.0)
    };
    let shoulders = b.any(Buttons::R1) as i32 as f32 - b.any(Buttons::L1) as i32 as f32;
    // While the waggle winds a dash up the sticks are busy: the ball takes nothing from them.
    let drive = if waggling { Drive::default() } else { drive };
    PadInput {
        forward: drive.forward,
        turn: drive.turn,
        strafe: drive.strafe,
        dash_held: dash_button || waggling,
        quick_turn: playing && quick_turn,
        orbit: (orbit + shoulders).clamp(-1.0, 1.0),
        tilt,
        zoom: b.any(Buttons::UP) as i32 as f32 - b.any(Buttons::DOWN) as i32 as f32,
        peek: b.any(Buttons::L2 | Buttons::R2),
        active: left != Vec2::ZERO || right != Vec2::ZERO || b.0 != 0,
    }
}

/// A scripted pad, `WOW_AZEROLL_FAKE_PAD`: segments `secs:lx,ly,rx,ry[:buttons]` joined by `;`,
/// played from the start of the run (y is up). `buttons` is `+`-joined `a b x y start select l1 r1
/// l2 r2 l3 r3 up down left right`. `wag<n>@<period>` expands to `n` alternating up/down pushes of
/// that many seconds each. After the last segment the pad rests.
#[derive(Resource, Debug, Clone)]
pub(super) struct FakePad {
    segments: Vec<(f32, Frame)>,
}

impl FakePad {
    pub(super) fn parse(script: &str) -> Self {
        let mut segments = Vec::new();
        for item in script.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(rest) = item.strip_prefix("wag") {
                let (n, period) = rest.split_once('@').unwrap_or((rest, "0.18"));
                let n: usize = n.parse().unwrap_or(4);
                let period: f32 = period.parse().unwrap_or(0.18);
                for i in 0..n {
                    let up = if i % 2 == 0 { 1.0 } else { -1.0 };
                    segments.push((
                        period,
                        Frame {
                            left: Vec2::new(0.0, up),
                            right: Vec2::new(0.0, -up),
                            buttons: Buttons(0),
                        },
                    ));
                }
                continue;
            }
            let mut parts = item.split(':');
            let secs: f32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let axes: Vec<f32> = parts
                .next()
                .unwrap_or("")
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            let mut buttons = 0;
            for name in parts.next().unwrap_or("").split('+') {
                buttons |= match name.trim() {
                    "a" => Buttons::A,
                    "b" => Buttons::B,
                    "x" => Buttons::X,
                    "y" => Buttons::Y,
                    "start" => Buttons::START,
                    "select" => Buttons::SELECT,
                    "l1" => Buttons::L1,
                    "r1" => Buttons::R1,
                    "l2" => Buttons::L2,
                    "r2" => Buttons::R2,
                    "l3" => Buttons::L3,
                    "r3" => Buttons::R3,
                    "up" => Buttons::UP,
                    "down" => Buttons::DOWN,
                    "left" => Buttons::LEFT,
                    "right" => Buttons::RIGHT,
                    _ => 0,
                };
            }
            let at = |i: usize| axes.get(i).copied().unwrap_or(0.0);
            segments.push((
                secs,
                Frame {
                    left: Vec2::new(at(0), at(1)),
                    right: Vec2::new(at(2), at(3)),
                    buttons: Buttons(buttons),
                },
            ));
        }
        Self { segments }
    }

    /// The pad `t` seconds into the script.
    pub(super) fn at(&self, t: f32) -> Frame {
        let mut left = t;
        for (secs, frame) in &self.segments {
            if left < *secs {
                return *frame;
            }
            left -= secs;
        }
        Frame::default()
    }

    pub(super) fn duration(&self) -> f32 {
        self.segments.iter().map(|(s, _)| s).sum()
    }
}

pub(super) fn plugin(app: &mut App) {
    let mut state = PadState::default();
    if crate::webenv::var("WOW_AZEROLL_PAD_MODE").is_some_and(|m| m == "simple") {
        state.simple = true;
    }
    app.init_resource::<PadInput>()
        .init_resource::<LastDevice>()
        .init_resource::<DashCharge>()
        .insert_resource(state)
        .add_systems(PreUpdate, read_pad.after(InputSystems));
    if let Some(script) = crate::webenv::var("WOW_AZEROLL_FAKE_PAD") {
        let fake = FakePad::parse(&script);
        info!(
            "azeroll pad: fake pad of {} segments, {:.1}s",
            fake.segments.len(),
            fake.duration()
        );
        app.insert_resource(fake).add_systems(Update, debug_shot);
    }
}

#[allow(clippy::too_many_arguments)]
fn read_pad(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    run: Res<super::Run>,
    pads: Query<&Gamepad>,
    fake: Option<Res<FakePad>>,
    mut st: ResMut<PadState>,
    mut out: ResMut<PadInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut device: ResMut<LastDevice>,
    mut pinned: Local<Option<Option<LastDevice>>>,
) {
    let ctx = Ctx::of(*phase.get());
    let pinned = *pinned.get_or_insert_with(|| {
        match crate::webenv::var("WOW_AZEROLL_FINISH_PROMPT").as_deref() {
            Some("pad") => Some(LastDevice::Pad),
            Some("keys") => Some(LastDevice::Keys),
            _ => None,
        }
    });
    // Real keys only: this frame's synthesised ones are pressed below, after this look.
    if keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some() {
        *device = LastDevice::Keys;
    }
    if let Some(p) = pinned {
        *device = p;
    }
    let (frame, now) = match &fake {
        Some(fake) => {
            let t = if ctx == Ctx::Playing {
                run.elapsed
            } else {
                0.0
            };
            (Some(fake.at(t)), time.elapsed_secs())
        }
        None => (
            pads.iter()
                .map(Frame::from_pad)
                .max_by(|a, b| a.activity().total_cmp(&b.activity())),
            time.elapsed_secs(),
        ),
    };
    let Some(mut frame) = frame else {
        if *out != PadInput::default() || !st.keys.is_empty() {
            *out = PadInput::default();
            for (_, key) in st.keys.drain(..) {
                keys.release(key);
            }
            st.prev = Buttons(0);
            st.wag.reset();
        }
        return;
    };
    // In the hub the left stick is a d-pad, too.
    if ctx == Ctx::Hub {
        let l = frame.left;
        if l.length() > NAV_THRESHOLD {
            frame.buttons.0 |= if l.x.abs() > l.y.abs() {
                if l.x > 0.0 {
                    Buttons::RIGHT
                } else {
                    Buttons::LEFT
                }
            } else if l.y > 0.0 {
                Buttons::UP
            } else {
                Buttons::DOWN
            };
        }
    }
    let prev = st.prev;
    // The simple-mode switch lives on the pause screen.
    if ctx == Ctx::Paused && frame.buttons.any(Buttons::SELECT) && !prev.any(Buttons::SELECT) {
        st.simple = !st.simple;
        info!(
            "azeroll pad: simple mode {}",
            if st.simple { "on" } else { "off" }
        );
    }
    *out = derive(frame, &mut st, now, ctx);
    if out.active {
        *device = LastDevice::Pad;
    }
    if let Some(p) = pinned {
        *device = p;
    }

    // Buttons let go: their keys come up, whatever screen they went down on.
    let released = prev.0 & !frame.buttons.0;
    let mut i = 0;
    while i < st.keys.len() {
        if st.keys[i].0 & released != 0 {
            keys.release(st.keys.remove(i).1);
        } else {
            i += 1;
        }
    }
    // Buttons pressed: the key they stand for on this screen.
    let pressed = frame.buttons.0 & !prev.0;
    for (_, bit) in BUTTONS {
        if pressed & bit != 0 {
            if let Some(key) = menu_key(ctx, bit) {
                if !keys.pressed(key) {
                    keys.press(key);
                    st.keys.push((bit, key));
                }
            }
        }
    }
    // Touching a stick starts a run waiting at its start, as W does.
    if ctx == Ctx::Ready && out.active {
        keys.release(KeyCode::Enter);
        keys.press(KeyCode::Enter);
        st.pulsing = true;
    } else if st.pulsing {
        st.pulsing = false;
        if !st.keys.iter().any(|(_, k)| *k == KeyCode::Enter) {
            keys.release(KeyCode::Enter);
        }
    }
    st.prev = Buttons(frame.buttons.0);
}

/// `WOW_AZEROLL_PAD_SHOT=<png>`: one screenshot once the dash has wound up for a moment, and
/// `WOW_AZEROLL_PAD_EXIT=<s>` exits after that many seconds of play. Logs each wind-up and burst.
fn debug_shot(
    mut commands: Commands,
    run: Res<super::Run>,
    charge: Res<DashCharge>,
    ball: Res<super::BallState>,
    mut dashes: MessageReader<super::Dash>,
    mut taken: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    for d in dashes.read() {
        info!(
            "azeroll pad: {d:?} t={:.2} dash={:.2} boost={:.2} vel={:.2}",
            run.elapsed,
            ball.dash,
            charge.boost,
            ball.vel.length()
        );
    }
    if let Some(path) = crate::webenv::var("WOW_AZEROLL_PAD_SHOT") {
        if !*taken && charge.winding() {
            *taken = true;
            use bevy::render::view::screenshot::{save_to_disk, Screenshot};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(std::path::PathBuf::from(path)));
        }
    }
    if let Some(secs) =
        crate::webenv::var("WOW_AZEROLL_PAD_EXIT").and_then(|s| s.parse::<f32>().ok())
    {
        if run.elapsed >= secs {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn deadzone_hides_drift_and_rescales_the_rest() {
        assert_eq!(stick(v(0.1, -0.1)), Vec2::ZERO);
        assert_eq!(stick(v(0.0, 0.18)), Vec2::ZERO);
        let s = stick(v(0.0, 0.59));
        assert!((s.y - 0.5).abs() < 1e-5 && s.x == 0.0, "{s:?}");
        let full = stick(v(0.8, 0.8));
        assert!(
            (full.length() - 1.0).abs() < 1e-5,
            "a corner clamps to the circle: {full:?}"
        );
        assert_eq!(stick(v(f32::NAN, 0.0)), Vec2::ZERO);
    }

    #[test]
    fn both_sticks_forward_or_back_roll() {
        let d = twin_stick(v(0.0, 1.0), v(0.0, 1.0));
        assert_eq!(
            d,
            Drive {
                forward: 1.0,
                turn: 0.0,
                strafe: 0.0
            }
        );
        let d = twin_stick(v(0.0, -1.0), v(0.0, -1.0));
        assert_eq!(d.forward, -1.0);
        // Analog: half throw is half speed.
        let d = twin_stick(v(0.0, 0.5), v(0.0, 0.5));
        assert!((d.forward - 0.5).abs() < 1e-6);
    }

    #[test]
    fn one_up_one_down_spins_on_the_spot() {
        let right = twin_stick(v(0.0, 1.0), v(0.0, -1.0));
        assert_eq!(right.forward, 0.0);
        assert!(
            (right.turn - 1.0).abs() < 1e-6,
            "left up, right down turns right"
        );
        let left = twin_stick(v(0.0, -1.0), v(0.0, 1.0));
        assert!((left.turn + 1.0).abs() < 1e-6);
        // One stick alone rolls and curves, away from the stick that is up.
        let d = twin_stick(v(0.0, 1.0), Vec2::ZERO);
        assert!((d.forward - 0.5).abs() < 1e-6 && d.turn > 0.0);
    }

    #[test]
    fn level_sticks_roll_straight_despite_hand_wobble() {
        // Two sticks held "forward" 0.1 apart: below the turn deadzone, no curve.
        let d = twin_stick(v(0.0, 1.0), v(0.0, 0.9));
        assert_eq!(d.turn, 0.0);
        let d = twin_stick(v(0.0, 1.0), v(0.0, 0.5));
        assert!(d.turn > 0.0, "a real difference still steers");
    }

    #[test]
    fn both_sticks_sideways_strafe() {
        let d = twin_stick(v(1.0, 0.0), v(1.0, 0.0));
        assert!((d.strafe - 1.0).abs() < 1e-6 && d.forward == 0.0 && d.turn == 0.0);
        let d = twin_stick(v(-1.0, 0.0), v(-1.0, 0.0));
        assert!(d.strafe < -0.99);
        // Opposite sideways pushes cancel; one stick alone does not strafe.
        assert_eq!(twin_stick(v(1.0, 0.0), v(-1.0, 0.0)).strafe, 0.0);
        assert_eq!(twin_stick(v(1.0, 0.0), Vec2::ZERO).strafe, 0.0);
        // A small shared sideways drift while rolling ahead is ignored.
        assert_eq!(twin_stick(v(0.2, 1.0), v(0.2, 1.0)).strafe, 0.0);
        // Diagonals roll and strafe at once.
        let d = twin_stick(v(0.9, 0.9), v(0.9, 0.9));
        assert!(d.forward > 0.8 && d.strafe > 0.8);
    }

    #[test]
    fn simple_mode_steers_like_a_car() {
        let d = simple_stick(v(0.0, 1.0));
        assert_eq!(
            d,
            Drive {
                forward: 1.0,
                turn: 0.0,
                strafe: 0.0
            }
        );
        let d = simple_stick(v(1.0, 0.0));
        assert!(d.forward == 0.0 && (d.turn - 1.0).abs() < 1e-6);
    }

    fn waggle(wag: &mut Wag, start: f32, period: f32, pushes: usize) -> (f32, bool) {
        let mut t = start;
        let mut engaged = false;
        for i in 0..pushes {
            let up = if i % 2 == 0 { 1.0 } else { -1.0 };
            // Hold each push for the period, a frame at a time.
            let until = t + period;
            while t < until {
                engaged = wag.update(t, up, -up);
                t += DT;
            }
        }
        (t, engaged)
    }

    #[test]
    fn three_quick_alternations_engage_the_dash_gesture() {
        let mut wag = Wag::default();
        let (_, engaged) = waggle(&mut wag, 0.0, 0.2, 2);
        assert!(!engaged, "two pushes are a turn, not a dash");
        let mut wag = Wag::default();
        let (t, engaged) = waggle(&mut wag, 0.0, 0.2, 3);
        assert!(engaged);
        // Still engaged while the last push is held, and for the hold after; then it lets go.
        assert!(wag.update(t + 0.2, 0.0, 0.0));
        assert!(!wag.update(t + 1.0, 0.0, 0.0));
        assert!(
            !wag.update(t + 1.1, 0.0, 0.0),
            "and needs three fresh pushes again"
        );
    }

    #[test]
    fn a_slow_waggle_or_one_sided_push_does_not_dash() {
        let mut wag = Wag::default();
        let (_, engaged) = waggle(&mut wag, 0.0, 0.7, 4);
        assert!(!engaged, "three pushes must fit within a second");
        // Both sticks forward is not the gesture however it is jiggled.
        let mut wag = Wag::default();
        for i in 0..120 {
            let y = if i % 6 < 3 { 1.0 } else { 0.4 };
            assert!(!wag.update(i as f32 * DT, y, y));
        }
        // Pushes too gentle to count.
        let mut wag = Wag::default();
        for i in 0..120 {
            let y = if i % 12 < 6 { 0.4 } else { -0.4 };
            assert!(!wag.update(i as f32 * DT, y, -y));
        }
    }

    fn hold(charge: &mut DashCharge, secs: f32, can_start: bool) -> (Vec<Charged>, f32) {
        let mut seen = Vec::new();
        let mut t = 0.0;
        while t < secs {
            let c = charge.step(DT, true, can_start, true);
            if c != Charged::default() {
                seen.push(c);
            }
            t += DT;
        }
        (seen, t)
    }

    #[test]
    fn a_tap_bursts_on_release_with_no_wind_up() {
        let mut c = DashCharge::default();
        let (seen, _) = hold(&mut c, 0.08, true);
        assert!(seen.is_empty() && !c.winding());
        let out = c.step(DT, false, true, true);
        assert_eq!(
            out,
            Charged {
                windup: false,
                burst: Some(0.0)
            }
        );
        // Nothing left over.
        assert_eq!(c.step(DT, false, true, true), Charged::default());
    }

    #[test]
    fn holding_winds_up_then_release_bursts_scaled_by_the_charge() {
        let mut c = DashCharge::default();
        let (seen, _) = hold(&mut c, 0.3, true);
        assert_eq!(
            seen,
            vec![Charged {
                windup: true,
                burst: None
            }],
            "one wind-up announcement"
        );
        assert!(c.winding());
        let out = c.step(DT, false, true, true);
        let charge = out.burst.expect("release bursts");
        assert!(charge > 0.3 && charge < 1.0, "{charge}");
        assert!(!c.winding());
    }

    #[test]
    fn a_complete_wind_up_bursts_by_itself_and_must_be_let_go_to_arm_again() {
        let mut c = DashCharge::default();
        let (seen, _) = hold(&mut c, 1.0, true);
        let bursts: Vec<_> = seen.iter().filter_map(|s| s.burst).collect();
        assert_eq!(
            bursts.len(),
            1,
            "held on: one burst, not a stream: {seen:?}"
        );
        assert!(bursts[0] > 0.95);
        // Let go, press again: it arms (the ball's cooldown, not this machine, says no).
        assert_eq!(c.step(DT, false, true, true), Charged::default());
        c.step(DT, true, true, true);
        assert!(c.held_for.is_some());
    }

    #[test]
    fn a_press_in_the_cooldown_does_nothing_and_a_pause_cancels() {
        let mut c = DashCharge::default();
        let (seen, _) = hold(&mut c, 0.6, false);
        assert!(seen.is_empty() && c.held_for.is_none());
        let mut c = DashCharge::default();
        hold(&mut c, 0.3, true);
        assert!(c.winding());
        assert_eq!(c.step(DT, true, true, false), Charged::default());
        assert!(
            !c.winding() && c.held_for.is_none(),
            "leaving play drops the wind-up"
        );
    }

    #[test]
    fn charge_runs_zero_to_one_between_the_tap_and_the_full_wind_up() {
        assert_eq!(charge_of(0.0), 0.0);
        assert_eq!(charge_of(TAP_SECS), 0.0);
        assert!((charge_of((TAP_SECS + FULL_SECS) * 0.5) - 0.5).abs() < 1e-5);
        assert_eq!(charge_of(2.0), 1.0);
    }

    fn frame(left: Vec2, right: Vec2, buttons: u32) -> Frame {
        Frame {
            left,
            right,
            buttons: Buttons(buttons),
        }
    }

    #[test]
    fn derive_maps_sticks_and_buttons() {
        let mut st = PadState::default();
        let p = derive(
            frame(v(0.0, 1.0), v(0.0, 1.0), 0),
            &mut st,
            0.0,
            Ctx::Playing,
        );
        assert!(p.forward > 0.99 && p.turn == 0.0 && p.active && !p.dash_held);
        // L3 + R3 is a quick turn, once.
        let click = Buttons::L3 | Buttons::R3;
        let p = derive(
            frame(Vec2::ZERO, Vec2::ZERO, click),
            &mut st,
            0.1,
            Ctx::Playing,
        );
        assert!(p.quick_turn);
        st.prev = Buttons(click);
        let p = derive(
            frame(Vec2::ZERO, Vec2::ZERO, click),
            &mut st,
            0.2,
            Ctx::Playing,
        );
        assert!(
            !p.quick_turn,
            "held clicks do not spin the ball round and round"
        );
        // Shoulders swing the camera; the pad's d-pad zooms; a trigger peeks.
        let p = derive(
            frame(
                Vec2::ZERO,
                Vec2::ZERO,
                Buttons::R1 | Buttons::UP | Buttons::L2,
            ),
            &mut PadState::default(),
            0.0,
            Ctx::Playing,
        );
        assert!(p.orbit == 1.0 && p.zoom == 1.0 && p.peek);
    }

    #[test]
    fn simple_mode_gives_the_right_stick_to_the_camera() {
        let mut st = PadState {
            simple: true,
            ..Default::default()
        };
        let p = derive(
            frame(v(0.0, 1.0), v(1.0, 0.0), 0),
            &mut st,
            0.0,
            Ctx::Playing,
        );
        assert!(p.forward > 0.99 && p.orbit > 0.99 && p.strafe == 0.0);
        let p = derive(
            frame(v(0.0, 1.0), v(0.0, -1.0), 0),
            &mut st,
            0.1,
            Ctx::Playing,
        );
        assert!(p.forward > 0.99, "the right stick is not a track here");
    }

    #[test]
    fn the_a_that_resumed_does_not_wind_up_a_dash() {
        let mut st = PadState::default();
        // A pressed on the pause screen, still down as play resumes.
        let p = derive(
            frame(Vec2::ZERO, Vec2::ZERO, Buttons::A),
            &mut st,
            0.0,
            Ctx::Paused,
        );
        assert!(!p.dash_held);
        let p = derive(
            frame(Vec2::ZERO, Vec2::ZERO, Buttons::A),
            &mut st,
            0.1,
            Ctx::Playing,
        );
        assert!(!p.dash_held, "held over from the menu");
        derive(frame(Vec2::ZERO, Vec2::ZERO, 0), &mut st, 0.2, Ctx::Playing);
        let p = derive(
            frame(Vec2::ZERO, Vec2::ZERO, Buttons::A),
            &mut st,
            0.3,
            Ctx::Playing,
        );
        assert!(p.dash_held, "a fresh press dashes");
    }

    #[test]
    fn the_waggle_takes_the_sticks_off_the_ball_and_holds_the_dash() {
        let mut st = PadState::default();
        let mut engaged_at = None;
        let mut t = 0.0;
        for i in 0..6 {
            let up = if i % 2 == 0 { 1.0 } else { -1.0 };
            for _ in 0..10 {
                let p = derive(frame(v(0.0, up), v(0.0, -up), 0), &mut st, t, Ctx::Playing);
                if p.dash_held && engaged_at.is_none() {
                    engaged_at = Some(t);
                }
                if p.dash_held {
                    assert_eq!((p.forward, p.turn, p.strafe), (0.0, 0.0, 0.0));
                }
                t += DT;
            }
        }
        let at = engaged_at.expect("engaged");
        assert!(at < 0.5, "engaged on the third push: {at}");
    }

    #[test]
    fn menus_have_their_keys_and_play_keeps_the_sticks_free() {
        assert_eq!(
            menu_key(Ctx::Hub, Buttons::RIGHT),
            Some(KeyCode::ArrowRight)
        );
        assert_eq!(menu_key(Ctx::Hub, Buttons::A), Some(KeyCode::Enter));
        assert_eq!(menu_key(Ctx::Paused, Buttons::START), Some(KeyCode::Escape));
        assert_eq!(menu_key(Ctx::Paused, Buttons::X), Some(KeyCode::KeyR));
        assert_eq!(menu_key(Ctx::Results, Buttons::A), Some(KeyCode::Enter));
        assert_eq!(
            menu_key(Ctx::Playing, Buttons::START),
            Some(KeyCode::Escape)
        );
        // In play A is the dash and the d-pad is the zoom: neither presses a key.
        assert_eq!(menu_key(Ctx::Playing, Buttons::A), None);
        assert_eq!(menu_key(Ctx::Playing, Buttons::UP), None);
        assert_eq!(menu_key(Ctx::Playing, Buttons::X), None);
    }

    #[test]
    fn the_fake_pad_script_plays_in_order() {
        let fake = FakePad::parse("1:0,1,0,1;0.5:0,0,0,0:a+x;wag4@0.2;0.3:1,0,1,0");
        assert_eq!(fake.at(0.5).left, v(0.0, 1.0));
        assert_eq!(fake.at(1.2).buttons, Buttons(Buttons::A | Buttons::X));
        // The waggle alternates.
        assert_eq!(fake.at(1.6).left, v(0.0, 1.0));
        assert_eq!(fake.at(1.8).left, v(0.0, -1.0));
        assert_eq!(fake.at(1.8).right, v(0.0, 1.0));
        assert_eq!(fake.at(2.4).left, v(1.0, 0.0));
        assert!((fake.duration() - 2.6).abs() < 1e-4);
        assert_eq!(fake.at(50.0).left, Vec2::ZERO);
    }

    #[test]
    fn a_scripted_waggle_reaches_the_dash_through_the_whole_chain() {
        // Fake pad → derive → charge: the path the debug run takes, without the window.
        let fake = FakePad::parse("0.3:0,1,0,1;wag6@0.17;0.5:0,0,0,0");
        let mut st = PadState::default();
        let mut charge = DashCharge::default();
        let (mut winds, mut bursts) = (0, 0);
        let mut t = 0.0;
        while t < 3.0 {
            let p = derive(fake.at(t), &mut st, t, Ctx::Playing);
            let c = charge.step(DT, p.dash_held, true, true);
            winds += c.windup as u32;
            bursts += c.burst.is_some() as u32;
            t += DT;
        }
        assert_eq!((winds, bursts), (1, 1));
    }
}
