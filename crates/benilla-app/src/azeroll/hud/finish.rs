//! The Finish button: from the moment a size goal is met until the run ends, a brass button under
//! the clock that says "Call it!" and what to press ("hold Enter or click", or "hold Y" when the
//! last input was a pad). Clicking it ends the run as holding the finish key does
//! (`Ending::Called`); the hold still works and fills the button as it fills the clock's bezel.
//! The pause menu offers "Finish run" too. A player who has outgrown everything reachable gets a
//! nudge: Fizzwick pops in and points at the button.
//!
//! The first playtest ("I got to a large ball size early in the inn level and then I couldn't move
//! anywhere else so just had to wait until the time ran out") found the old hint, shown once for a
//! few seconds, undiscoverable.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy::window::PrimaryWindow;

use crate::azeroll::campaign::CurrentLevel;
use crate::azeroll::design::{Design, GoalKind};
use crate::azeroll::fizz::{FizzSays, Pops, Voice, BONUS_GAP};
use crate::azeroll::lines::{self, Said};
use crate::azeroll::pad::LastDevice;
use crate::azeroll::progress::Flow;
use crate::azeroll::ritual::Ending;
use crate::azeroll::{Attached, BallState, Collected, Collectible, Phase, Rng, Run};

use super::{centered, text, Pop, GOLD, PARCHMENT};

/// Past the goal, this many play seconds with no pickup *and* no meal cluster in reach make Fizzwick
/// point at the button.
const NUDGE_IDLE: f32 = 15.0;
/// With meals still in reach, a player this long without a pickup is wedged or lost: nudge anyway.
const NUDGE_ANY: f32 = 45.0;
/// How often the nudge looks for meals once the idle time is up, seconds.
const NUDGE_LOOK: f32 = 3.0;
/// The button fades and pops in over this long, seconds.
const SHOW_SECS: f32 = 0.3;

const BRASS: Color = Color::srgb(0.85, 0.62, 0.18);
const BRASS_HOT: Color = Color::srgb(1.0, 0.84, 0.35);
const PLATE: Color = Color::srgb(0.17, 0.10, 0.04);
const PLATE_HOT: Color = Color::srgb(0.32, 0.19, 0.05);

/// The HUD's Finish button, and the pieces inside it.
#[derive(Component)]
pub(super) struct FinishButton;

/// The gold fill that rises along the button while the key is held.
#[derive(Component)]
pub(super) struct FinishFill;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum FinishText {
    Label,
    Key,
}

/// The pause menu's "Finish run" button.
#[derive(Component)]
pub(super) struct PauseFinish;

/// What the button remembers between frames.
#[derive(Resource, Default)]
pub(super) struct Finish {
    /// 0..1: how far the button has come in.
    shown: f32,
    /// Real seconds it has been fully in, for its pulse.
    age: f32,
    /// Each button (HUD, pause) was pressed last frame, so a release over it is a click.
    held: [bool; 2],
    /// Play seconds since the goal or the last pickup, for the nudge.
    idle: f32,
    /// Seconds until the nudge looks for meals again.
    look: f32,
    /// A meal cluster was in reach at the last look.
    meals: bool,
    /// Fizzwick has pointed at the button this run.
    nudged: bool,
}

/// Whether the player may call the run now: after the goal of a size level, never in Free Roll
/// and never over a single-target goal's finale.
pub(super) fn callable(won: bool, free_roll: bool, finale: bool, kind: GoalKind) -> bool {
    won && !free_roll && !finale && kind == GoalKind::Size
}

/// What the button tells the player to press.
pub(super) fn prompt(device: LastDevice) -> &'static str {
    match device {
        LastDevice::Keys => "hold Enter or click",
        LastDevice::Pad => "hold Y",
    }
}

/// Whether Fizzwick should point at the button: the run is called-for (`callable`), `idle` play
/// seconds have passed without a pickup, and either nothing is left in reach or the player has
/// been stuck a long while. `after` overrides the idle threshold for captures.
fn nudge_due(idle: f32, nudged: bool, meals: bool, after: Option<f32>) -> bool {
    if nudged {
        return false;
    }
    match after {
        Some(secs) => idle >= secs,
        None => idle >= NUDGE_ANY || (idle >= NUDGE_IDLE && !meals),
    }
}

/// End the run as a call: the same exit as holding Enter (`level::keys`).
pub(super) fn call_it(run: &mut Run, flow: &mut Flow, next: &mut NextState<Phase>, how: &str) {
    flow.call_hold = 0.0;
    flow.ending = Some(Ending::Called);
    run.called = true;
    info!("azeroll: the run was called at {:.1}s ({how})", run.elapsed);
    next.set(Phase::Results);
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Finish>()
        .add_systems(OnEnter(Phase::Ready), |mut st: ResMut<Finish>| {
            *st = Finish::default();
        });
    if crate::webenv::var("WOW_AZEROLL_FINISH_CLICK").is_some() {
        app.add_systems(
            PreUpdate,
            fake_click
                .after(bevy::input::InputSystems)
                .before(UiSystems::Focus),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------------------------

/// The HUD button, a child of the clock's column (so it sits right under the clock, above
/// Fizzwick's porthole).
pub(super) fn spawn_button(col: &mut ChildSpawnerCommands, f: &Handle<Font>) {
    col.spawn((
        FinishButton,
        Button,
        Node {
            display: Display::None,
            margin: UiRect::top(Val::Px(5.0)),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(9.0)),
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PLATE),
        BorderColor::all(BRASS),
        UiTransform::default(),
    ))
    .with_children(|b| {
        b.spawn((
            FinishFill,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.95, 0.68, 0.14, 0.55)),
        ));
        b.spawn((FinishText::Label, text(f, "Call it!", 22.0, GOLD)));
        b.spawn((FinishText::Key, text(f, "", 14.0, PARCHMENT)));
    });
}

/// The pause menu's button, shown once a size goal is met.
pub(super) fn spawn_pause(root: &mut ChildSpawnerCommands, f: &Handle<Font>) {
    root.spawn((
        PauseFinish,
        Button,
        Node {
            display: Display::None,
            margin: UiRect::top(Val::Px(10.0)),
            padding: UiRect::axes(Val::Px(22.0), Val::Px(7.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(PLATE),
        BorderColor::all(BRASS),
    ))
    .with_children(|b| {
        b.spawn(centered(f, "Finish run", 26.0, GOLD));
        b.spawn(text(f, "Enter", 15.0, PARCHMENT));
    });
}

// ---------------------------------------------------------------------------------------------
// Per frame
// ---------------------------------------------------------------------------------------------

/// Ease `x` toward on (1) or off (0) at `rate` per second.
fn approach(x: f32, on: bool, rate: f32, dt: f32) -> f32 {
    if on {
        (x + rate * dt).min(1.0)
    } else {
        (x - rate * dt).max(0.0)
    }
}

/// Show or hide the buttons, dress them, and act on a click, the Enter of the pause menu or a
/// release over either.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    real: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    device: Res<LastDevice>,
    design: Res<Design>,
    mut run: ResMut<Run>,
    mut flow: ResMut<Flow>,
    mut st: ResMut<Finish>,
    mut next: ResMut<NextState<Phase>>,
    mut button: Query<
        (
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut UiTransform,
        ),
        (
            With<FinishButton>,
            Without<FinishFill>,
            Without<PauseFinish>,
        ),
    >,
    mut fill: Query<
        &mut Node,
        (
            With<FinishFill>,
            Without<FinishButton>,
            Without<PauseFinish>,
        ),
    >,
    mut pause: Query<
        (
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (
            With<PauseFinish>,
            Without<FinishButton>,
            Without<FinishFill>,
        ),
    >,
    mut texts: Query<(&FinishText, &mut Text, &mut TextColor, &mut TextShadow)>,
) {
    let dt = real.delta_secs().min(0.1);
    let p = *phase.get();
    let can = callable(
        run.won,
        flow.free_roll,
        flow.finale.is_some(),
        design.goal.kind,
    );
    let in_play = can && p == Phase::Playing;
    let in_pause = can && p == Phase::Paused;
    st.shown = approach(st.shown, in_play, 1.0 / SHOW_SECS, dt);
    if st.shown >= 1.0 {
        st.age += dt;
    } else if st.shown <= 0.0 {
        st.age = 0.0;
    }
    let a = super::super::ritual::ease(st.shown);
    let mut fire: Option<&str> = None;

    for (interaction, mut node, mut bg, mut border, mut tf) in &mut button {
        let down = *interaction == Interaction::Pressed;
        if st.held[0] && *interaction == Interaction::Hovered && in_play && st.shown > 0.5 {
            fire = Some("the Finish button");
        }
        st.held[0] = down && in_play;
        let display = if st.shown > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        let hot = matches!(*interaction, Interaction::Hovered | Interaction::Pressed);
        let glow = if hot || flow.call_hold > 0.0 {
            1.0
        } else {
            0.5 + 0.5 * (st.age * 3.0).sin()
        };
        let plate = if hot { PLATE_HOT } else { PLATE };
        bg.0 = plate.with_alpha(0.9 * a);
        let rim = if hot { BRASS_HOT } else { BRASS };
        let rim = rim.with_alpha(a * (0.7 + 0.3 * glow));
        *border = BorderColor::all(rim);
        let scale = (0.82 + 0.18 * a) * if down { 0.97 } else { 1.0 };
        tf.scale = Vec2::splat(scale);
    }
    for mut node in &mut fill {
        let w = Val::Percent(100.0 * flow.call_hold);
        if node.width != w {
            node.width = w;
        }
    }
    for (which, mut s, mut color, mut shadow) in &mut texts {
        let (want, c) = match which {
            FinishText::Label => ("Call it!".to_string(), GOLD.with_alpha(a)),
            FinishText::Key => (prompt(*device).to_string(), PARCHMENT.with_alpha(0.9 * a)),
        };
        if s.0 != want {
            s.0 = want;
        }
        if color.0 != c {
            color.0 = c;
        }
        let shade = Color::BLACK.with_alpha(a);
        if shadow.color != shade {
            shadow.color = shade;
        }
    }

    for (interaction, mut node, mut bg, mut border) in &mut pause {
        let down = *interaction == Interaction::Pressed;
        if st.held[1] && *interaction == Interaction::Hovered && in_pause {
            fire = Some("the pause menu's button");
        }
        st.held[1] = down && in_pause;
        let display = if in_pause {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        let hot = matches!(*interaction, Interaction::Hovered | Interaction::Pressed);
        bg.0 = if hot { PLATE_HOT } else { PLATE };
        *border = BorderColor::all(if hot { BRASS_HOT } else { BRASS });
    }
    if in_pause && keys.just_pressed(KeyCode::Enter) {
        fire = Some("Enter on the pause menu");
    }

    if let Some(how) = fire {
        call_it(&mut run, &mut flow, &mut next, how);
    }
}

/// `WOW_AZEROLL_FINISH_NUDGE=<play secs>`: nudge that long after the goal whatever the player has
/// rolled up (the autopilot never idles), for captures.
fn nudge_override() -> Option<f32> {
    crate::webenv::var("WOW_AZEROLL_FINISH_NUDGE")?.parse().ok()
}

/// A player who has met the goal and has nothing left to roll up gets one pop-in from Fizzwick
/// pointing at the button. It is outside the three-a-run budget (like the parcel's), waits for the
/// porthole to be quiet, and happens at most once a run.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn nudge(
    real: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    flow: Res<Flow>,
    design: Res<Design>,
    ball: Res<BallState>,
    current: Res<CurrentLevel>,
    pop: Res<Pop>,
    mut collected: MessageReader<Collected>,
    things: Query<(Entity, &Collectible, &GlobalTransform), Without<Attached>>,
    mut st: ResMut<Finish>,
    mut ledger: ResMut<Pops>,
    mut said: ResMut<Said>,
    mut rng: ResMut<Rng>,
    mut out: MessageWriter<FizzSays>,
    mut after: Local<Option<Option<f32>>>,
) {
    let after = *after.get_or_insert_with(nudge_override);
    let picked = collected.read().count() > 0;
    let can = callable(
        run.won,
        flow.free_roll,
        flow.finale.is_some(),
        design.goal.kind,
    );
    if !can || *phase.get() != Phase::Playing || st.nudged {
        st.idle = 0.0;
        return;
    }
    if picked && after.is_none() {
        st.idle = 0.0;
    }
    let dt = real.delta_secs().min(0.1);
    st.idle += dt;
    st.look -= dt;
    if after.is_none() && st.idle >= NUDGE_IDLE && st.look <= 0.0 {
        st.look = NUDGE_LOOK;
        st.meals = super::cluster_of(&things, &ball).is_some();
    }
    if !nudge_due(st.idle, st.nudged, st.meals, after) {
        return;
    }
    // The porthole is busy, or a line has only just gone: wait, do not drop it.
    let now = real.elapsed_secs();
    if pop.age.is_some()
        || pop.waiting.is_some()
        || ledger.last_at.is_some_and(|t| now - t < BONUS_GAP)
    {
        return;
    }
    let line = lines::pick(&lines::NUDGE, current.0, &mut said, rng.next_u64());
    if line.is_empty() {
        return;
    }
    st.nudged = true;
    ledger.last_at = Some(now);
    info!(
        "azeroll fizz: nudge to the Finish button after {:.0}s idle: {line:?}",
        st.idle
    );
    out.write(FizzSays {
        line,
        voice: Voice::Talk,
    });
}

/// `WOW_AZEROLL_FINISH_CLICK=<secs>`: a capture and test aid that clicks the Finish button for real
/// (through bevy_ui's focus), that many real seconds after it is fully in: the cursor goes to its
/// middle, the left button goes down for a moment and comes up. Logs each step.
fn fake_click(
    real: Res<Time<Real>>,
    st: Res<Finish>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    button: Query<(&ComputedNode, &UiGlobalTransform), With<FinishButton>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut waited: Local<f32>,
) {
    let delay: f32 = crate::webenv::var("WOW_AZEROLL_FINISH_CLICK")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.5);
    if st.shown < 1.0 {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok((_, at)) = button.single() else { return };
    let before = *waited;
    *waited += real.delta_secs();
    let centre = at.translation;
    if *waited >= delay - 0.3 && *waited < delay + 0.5 {
        window.set_physical_cursor_position(Some(DVec2::new(centre.x as f64, centre.y as f64)));
    }
    if before < delay && *waited >= delay {
        info!("azeroll finish: click down at {centre:?}");
        mouse.press(MouseButton::Left);
    }
    if before < delay + 0.2 && *waited >= delay + 0.2 {
        info!("azeroll finish: click up");
        mouse.release(MouseButton::Left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_won_size_level_can_be_called() {
        assert!(callable(true, false, false, GoalKind::Size));
        assert!(!callable(false, false, false, GoalKind::Size));
        assert!(!callable(true, true, false, GoalKind::Size));
        assert!(!callable(true, false, true, GoalKind::Size));
        assert!(!callable(true, false, false, GoalKind::Collect));
        assert!(!callable(true, false, false, GoalKind::Finale));
    }

    #[test]
    fn the_prompt_follows_the_device() {
        assert!(prompt(LastDevice::Keys).contains("Enter"));
        assert!(prompt(LastDevice::Keys).contains("click"));
        assert!(prompt(LastDevice::Pad).contains('Y'));
        assert!(!prompt(LastDevice::Pad).contains("Enter"));
    }

    #[test]
    fn the_nudge_waits_for_an_empty_reach_or_a_long_stall_and_comes_once() {
        // Nothing in reach: after the idle time.
        assert!(!nudge_due(NUDGE_IDLE - 1.0, false, false, None));
        assert!(nudge_due(NUDGE_IDLE, false, false, None));
        // Meals in reach: only after the long stall.
        assert!(!nudge_due(NUDGE_IDLE + 10.0, false, true, None));
        assert!(nudge_due(NUDGE_ANY, false, true, None));
        // Once per run.
        assert!(!nudge_due(NUDGE_ANY * 2.0, true, false, None));
        // The capture lever overrides the thresholds.
        assert!(nudge_due(3.0, false, true, Some(3.0)));
        assert!(!nudge_due(2.9, false, true, Some(3.0)));
    }

    #[test]
    fn every_nudge_line_names_the_button() {
        for l in lines::NUDGE
            .generic
            .iter()
            .chain(lines::NUDGE.per_level.iter().flat_map(|p| p.iter()))
        {
            assert!(l.contains("Call it!"), "{l}");
        }
    }

    #[test]
    fn the_button_eases_in_and_out() {
        let mut x = 0.0;
        for _ in 0..30 {
            x = approach(x, true, 1.0 / SHOW_SECS, 0.02);
        }
        assert_eq!(x, 1.0);
        for _ in 0..30 {
            x = approach(x, false, 1.0 / SHOW_SECS, 0.02);
        }
        assert_eq!(x, 0.0);
    }

    #[test]
    fn calling_it_sets_the_ending_and_asks_for_the_results() {
        let mut run = Run::default();
        let mut flow = Flow::default();
        flow.call_hold = 0.7;
        let mut next = NextState::<Phase>::default();
        call_it(&mut run, &mut flow, &mut next, "a test");
        assert!(run.called);
        assert_eq!(flow.ending, Some(Ending::Called));
        assert_eq!(flow.call_hold, 0.0);
        assert!(matches!(next, NextState::Pending(Phase::Results)));
    }
}
