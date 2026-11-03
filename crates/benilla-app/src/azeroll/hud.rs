//! The mode's screens, in bevy_ui since FrameXML is off. Play is nearly wordless, as Katamari's
//! is: the size dial, the clock, the item bubble and the Prince over the world, with the goal said
//! once at the start and a few contextual moments after it
//! (the boss is ready; a gentle arrow when it is off screen or the player has stalled). The count,
//! the requirement and the pickup log live on the pause and results screens. Also the banner and
//! the ready, pause, results and finale overlays. Everything is built once at startup and shown
//! or hidden by phase.

use benilla_world::schedule::WorldStage;
use benilla_world::view::WorldCamera;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::campaign::CurrentLevel;
use super::collect::contact;
use super::design::{Design, GoalKind};
use super::fizz::{FizzSays, Voice};
use super::hud_models::{self, Commissioner, HudMaterial, HudModels, ItemBubble};
use super::level::COLLECT_RATIO;
use super::progress::{Flow, Progress};
use super::ritual::Cue;
use super::select::hub::{kind as art_kind, style as art_style, Art, ArtMaterial};
use super::{
    Attached, BallState, Collected, Collectible, Contact, GoalReached, GrowthBeat, Kind, Phase,
    Run, StarEarned, TierReached,
};
use crate::loading_screen::LoadingScreen;
use crate::sound::GlueSound;

mod finish;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const DIM: Color = Color::srgb(0.6, 0.6, 0.6);
const PARCHMENT: Color = Color::srgb(0.85, 0.78, 0.6);
const RED: Color = Color::srgb(1.0, 0.2, 0.2);
const GREEN: Color = Color::srgb(0.4, 0.85, 0.4);

/// The `SoundEntries` kit name the glue screens already play for banners: the quest log's page turn.
/// (Pickup sounds are `sfx`'s.)
const BANNER_SOUND: &str = "igQuestListOpen";

/// The goal is said for this many seconds of play at the start, then fades over `INTRO_FADE`.
const INTRO_HOLD: f32 = 4.0;
const INTRO_FADE: f32 = 1.2;
/// "<Boss> is ready": seconds held, and the fades either side.
const READY_HOLD: f32 = 2.4;
const READY_IN: f32 = 0.18;
const READY_OUT: f32 = 0.8;
/// A player with no pickup for this many seconds of play is gently pointed at the nearest meal,
/// for `HINT_SECS`, at most `HINT_MAX` times a run and not again for `HINT_COOLDOWN` seconds.
const PLATEAU_SECS: f32 = 20.0;
const HINT_SECS: f32 = 7.0;
const HINT_COOLDOWN: f32 = 60.0;
const HINT_MAX: u32 = 3;
/// How far the hint looks for a meal (yards), and the grid it counts clusters on.
const CLUSTER_RANGE: f32 = 110.0;
const CLUSTER_CELL: f32 = 6.0;
/// Things smaller than this fraction of what the ball could just take are crumbs, no meal.
const CRUMB: f32 = 0.2;
/// The guide arrow's box, UI units, and how far out toward the screen's edge it rides (NDC).
const ARROW: f32 = 58.0;
const ARROW_MARGIN: Vec2 = Vec2::new(0.9, 0.84);
/// How fast the arrow fades in and out, per second.
const ARROW_RATE: f32 = 4.0;
/// A banner fades over its last seconds.
const BANNER_FADE: f32 = 0.6;
/// The intro on the ring is this much bigger than the resting label.
const INTRO_SCALE: f32 = 1.6;
/// The reference lays its UI out on a screen 768 units tall.
const UI_HEIGHT: f32 = 768.0;
/// Widget sizes, UI units: the size dial (its ring included), the clock, the item bubble and the
/// Prince corner.
const DIAL: f32 = 196.0;
const CLOCK: f32 = 92.0;
const BUBBLE: f32 = 150.0;
const PRINCE_BOX: f32 = 176.0;
/// The goal title's gutters, UI units: past the dial (left 8 + DIAL) and the clock block (right 16
/// + its timer, gap and dial), so the title never sits under either.
const GOAL_INSET_LEFT: f32 = 216.0;
const GOAL_INSET_RIGHT: f32 = 220.0;
/// How fast the ring's fill and the dial's pulse settle, per second.
const FILL_RATE: f32 = 5.0;

// Item-quality colours, rgb: poor, common, uncommon, rare, epic, legendary.
const Q_GREY: [f32; 3] = [0.62, 0.62, 0.62];
const Q_WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const Q_GREEN: [f32; 3] = [0.12, 1.0, 0.0];
const Q_BLUE: [f32; 3] = [0.0, 0.44, 0.87];
const Q_PURPLE: [f32; 3] = [0.64, 0.21, 0.93];
const Q_ORANGE: [f32; 3] = [1.0, 0.5, 0.0];

/// A screen-sized panel shown in some phases.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Panel {
    /// The in-run readouts.
    Hud,
    Ready,
    Pause,
    /// The celebration over the world after the goal is met.
    Finale,
    /// Fizzwick's porthole and bubble, popping in under the clock.
    Fizz,
}

/// A text of the in-run HUD, kept up to date by [`refresh_hud`].
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudText {
    /// The number in the dial.
    Size,
    /// The goal's size, tilted by the end of the ring.
    RingGoal,
    Timer,
    Goal,
    GoalSub,
    Banner,
    /// The name under the item bubble, and its combo counter.
    ItemName,
    Combo,
    /// A contextual moment, under the banner ("Hogger is ready").
    Moment,
}

/// A [`HudMaterial`] widget of the in-run HUD, driven by [`refresh_widgets`].
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Widget {
    Dial,
    Clock,
    Item,
    Prince,
}

#[derive(Component)]
struct WidgetMat(Handle<HudMaterial>);

/// The sliding row of Fizzwick's porthole and bubble.
#[derive(Component)]
struct FizzSlide;

/// His bubble's text, and the porthole's material.
#[derive(Component)]
struct FizzText;

/// His bubble's plaque. `refresh_pop` writes this height into the plaque's material, so it must
/// find only this plaque: matching every [`Art`] also rewrote `lv[0].x` of the hub's inset, which
/// is the first pin's state, and drew the first level's pin as won.
#[derive(Component)]
struct FizzBubble;

#[derive(Component)]
struct PortholeMat(Handle<HudMaterial>);

/// The guide arrow, placed and turned by [`refresh_guide`].
#[derive(Component)]
struct GuideArrow;

/// A text of the pause screen, filled by [`refresh_pause`].
#[derive(Component, Clone, Copy)]
enum PauseText {
    Detail,
    Goal,
}

/// A text of the celebration, animated by [`refresh_finale`].
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum FinaleText {
    Title,
    Sub,
}

/// The white flash as the goal goes in.
#[derive(Component)]
struct FinaleFlash;

/// A text of the title screen's bests, filled by [`refresh_ready`].
#[derive(Component, Clone, Copy)]
enum ReadyText {
    /// The chosen level's title and objective.
    Level,
    Best,
    Free,
}

/// What the guide arrow points at.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum GuideKind {
    #[default]
    None,
    /// Hogger, edible and off screen: the goal.
    Hogger,
    /// The nearest cluster of meals, for a player who has stalled.
    Cluster,
}

/// The play-time cues the widgets and the guidance share. Reset with each run.
#[derive(Resource, Default)]
struct Cues {
    /// 1 → 0 after a growth beat: the pulse running along the ring.
    tick: f32,
    /// 1 → 0 after a tier, or Hogger coming ready: the rosette's bloom.
    bloom: f32,
    /// 1 → 0 after a growth beat or a tier: the dial's swell.
    pulse: f32,
    /// [`Run::elapsed`] at the last pickup, for the plateau.
    last_pickup: f32,
    /// Hogger has been announced this run (never twice).
    ready_seen: bool,
    /// Seconds since the announcement, while it shows.
    ready_age: Option<f32>,
    guide: GuideKind,
    target: Vec3,
    /// 0..1: the arrow's visibility, eased.
    guide_alpha: f32,
    /// The plateau hint: seconds it has left, the cooldown to the next, how many were shown and
    /// when to look for a meal again.
    hint_left: f32,
    hint_cooldown: f32,
    hints: u32,
    rescan: f32,
    /// 1 → 0 after a star is lit: its pop.
    star_pop: f32,
    /// The last whole second shown on the clock, for the last ten seconds' ticks.
    last_left: Option<u32>,
    /// How many of the hold's three ticks have sounded.
    hold_ticks: u32,
}

/// The pop-in on screen: its line, the real seconds since it began and how long it holds.
#[derive(Resource, Default)]
struct Pop {
    line: &'static str,
    age: Option<f32>,
    hold: f32,
    /// A line waiting for Fizzwick's stage to have rendered (so the window is never empty), the
    /// voice it was said with, and how long it has waited.
    waiting: Option<(FizzSays, f32)>,
}

/// A waiting line is dropped after this long without his stage coming up.
const POP_WAIT: f32 = 3.0;

/// Slide in/out seconds, the typing speed (characters per second) and the shortest hold.
const POP_SLIDE: f32 = 0.3;
const POP_CPS: f32 = 48.0;
const POP_HOLD: f32 = 2.6;
/// The porthole's size and the bubble's width, UI units.
const PORTHOLE: f32 = 104.0;
const POP_BUBBLE: f32 = 300.0;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(hud_models::plugin)
        .init_resource::<Cues>()
        .init_resource::<Pop>()
        .add_plugins(finish::plugin)
        .add_systems(Startup, (hud_models::setup, setup_hud).chain())
        .add_systems(OnEnter(Phase::Ready), clear_cues)
        .add_systems(
            Update,
            (
                apply_ui_scale,
                show_panels,
                read_cues,
                guidance,
                refresh_hud,
                refresh_widgets,
                refresh_guide,
                refresh_finale,
                refresh_pop,
                finish::refresh,
                finish::nudge,
            )
                .chain()
                .after(WorldStage::Input),
        )
        .add_systems(Update, refresh_ready.run_if(in_state(Phase::Ready)))
        .add_systems(Update, refresh_pause.run_if(in_state(Phase::Paused)));
}

// ---------------------------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------------------------

fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::BLACK,
        },
    )
}

fn centered(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        text(font, s, size, color),
        TextLayout::new_with_justify(Justify::Center),
    )
}

/// A full-screen column, its children centred.
fn overlay(gap: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        row_gap: Val::Px(gap),
        ..default()
    }
}

fn setup_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    models: Res<HudModels>,
    mut mats: ResMut<Assets<HudMaterial>>,
    mut art_mats: ResMut<Assets<ArtMaterial>>,
) {
    let mut widget = |kind: f32, tex: &Handle<Image>, p: Vec4| {
        mats.add(HudMaterial {
            kt: Vec4::new(kind, 0.0, 1.0, 0.0),
            p,
            tex: tex.clone(),
        })
    };
    let dial_mat = widget(0.0, &models.blank, Vec4::ZERO);
    let clock_mat = widget(1.0, &models.blank, Vec4::ZERO);
    let item_mat = widget(2.0, &models.item, Vec4::ZERO);
    let prince_mat = widget(2.0, &models.prince, Vec4::new(1.0, 0.0, 0.0, 0.0));
    let arrow_mat = widget(3.0, &models.blank, Vec4::new(1.0, 0.5, 0.0, 0.0));
    let porthole_mat = widget(4.0, &models.commissioner, Vec4::ZERO);
    let bubble_mat = art_mats.add(ArtMaterial::new(
        art_kind::PLAQUE,
        Vec4::new(art_style::BUBBLE, 0.0, 0.0, 0.5),
        &models.blank,
    ));
    let font = crate::char_select::wow_font(&asset_server);
    let number_font: Handle<Font> = asset_server.load("mpq://Fonts/ARIALN.ttf");
    let f = &font;
    let n = &number_font;

    // --- The in-run HUD. ---
    commands
        .spawn((
            Panel::Hud,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(400),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            // The size dial, top left: the rosette with the size in it and the goal ring round it.
            root.spawn((
                Widget::Dial,
                WidgetMat(dial_mat.clone()),
                MaterialNode(dial_mat.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: Val::Px(6.0),
                    width: Val::Px(DIAL),
                    height: Val::Px(DIAL),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                UiTransform::default(),
            ))
            .with_children(|dial| {
                dial.spawn((HudText::Size, centered(n, "18in", 25.0, Color::WHITE)));
            });
            // The goal's size, tilted at the end of the ring like the reference's "300m".
            root.spawn((
                HudText::RingGoal,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(DIAL * 0.66),
                    top: Val::Px(DIAL * 0.86),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::degrees(-24.0)),
                text(f, "", 17.0, PARCHMENT),
            ));

            // The clock, top right.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|col| {
                col.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((HudText::Timer, text(n, "10:00", 44.0, Color::WHITE)));
                    row.spawn((
                        Widget::Clock,
                        WidgetMat(clock_mat.clone()),
                        MaterialNode(clock_mat.clone()),
                        Node {
                            width: Val::Px(CLOCK),
                            height: Val::Px(CLOCK),
                            ..default()
                        },
                    ));
                });
                // The Finish button, from the goal on (`finish`).
                finish::spawn_button(col, f);
            });

            // The goal, said once at the start, top centre. It is inset past the dial on the left
            // and the clock block on the right, so its text wraps inside the gap between them at
            // any width.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(GOAL_INSET_LEFT),
                right: Val::Px(GOAL_INSET_RIGHT),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((HudText::Goal, centered(f, "", 30.0, GOLD)));
                    col.spawn((HudText::GoalSub, centered(f, "", 16.0, PARCHMENT)));
                });
            });

            // What was just rolled up, in a glass bubble, bottom left.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                bottom: Val::Px(12.0),
                width: Val::Px(BUBBLE + 40.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    Widget::Item,
                    WidgetMat(item_mat.clone()),
                    MaterialNode(item_mat.clone()),
                    Node {
                        width: Val::Px(BUBBLE),
                        height: Val::Px(BUBBLE),
                        ..default()
                    },
                    UiTransform::default(),
                ));
                col.spawn((HudText::ItemName, centered(f, "", 17.0, Color::WHITE)));
            });
            // The combo counter, on the bubble's upper right.
            root.spawn((
                HudText::Combo,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(14.0 + BUBBLE * 0.62),
                    bottom: Val::Px(12.0 + BUBBLE * 0.7),
                    ..default()
                },
                UiTransform::default(),
                text(n, "", 38.0, GOLD),
            ));

            // The Prince, bottom right.
            root.spawn((
                Widget::Prince,
                WidgetMat(prince_mat.clone()),
                MaterialNode(prince_mat.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(12.0),
                    bottom: Val::Px(10.0),
                    width: Val::Px(PRINCE_BOX),
                    height: Val::Px(PRINCE_BOX),
                    ..default()
                },
                UiTransform::default(),
            ));

            // The guide arrow: parked at the screen's middle, hidden until it has a target.
            root.spawn((
                GuideArrow,
                WidgetMat(arrow_mat.clone()),
                MaterialNode(arrow_mat.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    width: Val::Px(ARROW),
                    height: Val::Px(ARROW),
                    margin: UiRect::all(Val::Px(-ARROW * 0.5)),
                    ..default()
                },
                UiTransform::default(),
                Visibility::Hidden,
            ));

            // The banner, upper centre: one small line, clear of the dial's label.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(14.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((HudText::Banner, centered(f, "", 26.0, GOLD)));
            });

            // A contextual moment under the banner.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                // Below the banner (14 %), the dial's label (26 %) and Fizzwick's porthole (to
                // 32 %), so a tier opening and "<Boss> is ready!" never overlap.
                top: Val::Percent(38.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    HudText::Moment,
                    centered(f, "", 34.0, Color::srgb(1.0, 0.6, 0.15)),
                ));
            });
        });

    // --- Ready. ---
    commands
        .spawn((
            Panel::Ready,
            overlay(12.0),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(420),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn(centered(f, "AZEROLL", 104.0, GOLD));
            root.spawn(centered(f, "Everything is Bind on Pickup.", 30.0, PARCHMENT));
            root.spawn((ReadyText::Level, centered(f, "", 34.0, Color::WHITE)));
            root.spawn((ReadyText::Best, centered(f, "", 22.0, PARCHMENT)));
            root.spawn(Node {
                margin: UiRect::top(Val::Px(14.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn(centered(
                    f,
                    "W/S roll \u{b7} A/D turn \u{b7} Shift dash \u{b7} Space reverse \u{b7} Drag/wheel camera\nEsc pause \u{b7} R restart \u{b7} Hold Backspace unstuck",
                    18.0,
                    Color::WHITE,
                ));
            });
            root.spawn(centered(f, "Roll to begin", 20.0, GOLD));
            root.spawn((ReadyText::Free, centered(f, "", 18.0, GREEN)));
        });

    // --- Pause. ---
    commands
        .spawn((
            Panel::Pause,
            overlay(12.0),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(430),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn(centered(f, "Paused", 56.0, GOLD));
            root.spawn((PauseText::Goal, centered(f, "", 24.0, PARCHMENT)));
            root.spawn((PauseText::Detail, centered(f, "", 20.0, Color::WHITE)));
            root.spawn(centered(
                f,
                "Esc to resume \u{b7} R to start over \u{b7} L for level select",
                18.0,
                DIM,
            ));
            // After a size goal: finish the run from here (`finish`).
            finish::spawn_pause(root, f);
        });

    // --- The finale: the banner, the line under it and the flash. ---
    commands
        .spawn((
            Panel::Finale,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(Val::Percent(12.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(410),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn((
                FinaleFlash,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                FinaleText::Title,
                text(f, "", 64.0, GOLD),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                Node::default(),
            ));
            root.spawn((FinaleText::Sub, centered(f, "", 24.0, PARCHMENT)));
        });

    // --- Fizzwick's porthole: a window and a one-line bubble under the clock. ---
    commands
        .spawn((
            Panel::Fizz,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(445),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn((
                FizzSlide,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(12.0),
                    top: Val::Px(150.0),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    ..default()
                },
                UiTransform::default(),
            ))
            .with_children(|row| {
                row.spawn((
                    PortholeMat(porthole_mat.clone()),
                    MaterialNode(porthole_mat.clone()),
                    Node {
                        width: Val::Px(PORTHOLE),
                        height: Val::Px(PORTHOLE),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                row.spawn((
                    FizzBubble,
                    Art(bubble_mat.clone()),
                    MaterialNode(bubble_mat.clone()),
                    Node {
                        width: Val::Px(POP_BUBBLE),
                        min_height: Val::Px(54.0),
                        padding: UiRect {
                            left: Val::Px(30.0),
                            right: Val::Px(14.0),
                            top: Val::Px(10.0),
                            bottom: Val::Px(10.0),
                        },
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|b| {
                    b.spawn((
                        FizzText,
                        Text::new(""),
                        TextFont {
                            font: font.clone(),
                            font_size: 19.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.16, 0.10, 0.05)),
                        Node {
                            flex_basis: Val::Px(0.0),
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));
                });
            });
        });
}

// ---------------------------------------------------------------------------------------------
// Per frame
// ---------------------------------------------------------------------------------------------

/// The UI is laid out on a screen 768 units tall at any resolution.
fn apply_ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let Ok(window) = windows.single() else { return };
    // A capture aid lays the screens out for another aspect (see `select::capture_aspect`).
    let want = match super::select::capture_aspect() {
        Some(aspect) => window.width() / (UI_HEIGHT * aspect),
        None => window.height() / UI_HEIGHT,
    };
    if want > 0.0 && (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}

/// Show each panel in its phases. A hidden panel leaves the layout too, so it cannot take clicks.
fn show_panels(
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    flow: Res<Flow>,
    mut panels: Query<(&Panel, &mut Visibility, &mut Node)>,
) {
    let p = *phase.get();
    for (panel, mut vis, mut node) in &mut panels {
        let on = match panel {
            Panel::Hud => !matches!(p, Phase::Results | Phase::Select),
            // The title waits for the world: the loading screen is up until it is ready.
            Panel::Ready => p == Phase::Ready && !loading.covering(),
            Panel::Pause => p == Phase::Paused,
            Panel::Finale => flow.finale.is_some() && p != Phase::Results,
            // The pop-in shows itself (`refresh_pop`); the panel only has to be out of the hub.
            Panel::Fizz => !matches!(p, Phase::Select | Phase::Ready) && !loading.covering(),
        };
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

fn clear_cues(mut cues: ResMut<Cues>) {
    *cues = Cues::default();
}

/// The colour of a thing rolled up, by how much of the ball's capacity it took: grey for crumbs up
/// through white, green, blue and purple for the whole mouthful; Hogger is legendary.
fn quality(kind: Kind, pickup: f32, radius: f32) -> [f32; 3] {
    if kind == Kind::Boss {
        return Q_ORANGE;
    }
    let fill = pickup / (radius * super::level::COLLECT_RATIO).max(1e-3);
    if fill < 0.25 {
        Q_GREY
    } else if fill < 0.5 {
        Q_WHITE
    } else if fill < 0.75 {
        Q_GREEN
    } else if fill < 0.92 {
        Q_BLUE
    } else {
        Q_PURPLE
    }
}

/// How much of the ball's capacity a thing takes, 0..1 (Hogger takes it all).
fn fill(kind: Kind, pickup: f32, radius: f32) -> f32 {
    if kind == Kind::Boss {
        return 1.0;
    }
    (pickup / (radius * COLLECT_RATIO).max(1e-3)).clamp(0.0, 1.0)
}

/// Age the dial's cues and take in the growth, tier, goal and star messages; tick the last ten
/// seconds and the finish key's hold.
#[allow(clippy::too_many_arguments)]
fn read_cues(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    flow: Res<Flow>,
    design: Res<Design>,
    mut beats: MessageReader<GrowthBeat>,
    mut tiers: MessageReader<TierReached>,
    mut goals: MessageReader<GoalReached>,
    mut stars: MessageReader<StarEarned>,
    mut cues: ResMut<Cues>,
    mut sounds: MessageWriter<Cue>,
) {
    let dt = time.delta_secs();
    cues.tick = (cues.tick - dt * 1.8).max(0.0);
    cues.bloom = (cues.bloom - dt * 0.8).max(0.0);
    cues.pulse = (cues.pulse - dt * 3.0).max(0.0);
    cues.star_pop = (cues.star_pop - dt * 4.0).max(0.0);
    if beats.read().count() > 0 {
        cues.tick = 1.0;
        cues.pulse = 1.0;
    }
    if tiers.read().count() > 0 {
        cues.bloom = 1.0;
        cues.pulse = 1.0;
    }
    // The goal beat: the rosette blooms.
    for _ in goals.read() {
        cues.bloom = 1.0;
        cues.pulse = 1.0;
    }
    for _ in stars.read() {
        cues.star_pop = 1.0;
        cues.pulse = 1.0;
        sounds.write(Cue::Star);
    }
    // The last ten seconds tick once each; the finish key rises in three notes.
    if *phase.get() == Phase::Playing && !flow.free_roll && run.elapsed > 0.0 {
        let left = (design.time_limit - run.elapsed).max(0.0).ceil() as u32;
        if cues.last_left != Some(left) {
            if (1..=10).contains(&left) && cues.last_left.is_some() {
                sounds.write(Cue::Tick);
            }
            cues.last_left = Some(left);
        }
    }
    let notes = (flow.call_hold * 3.0).floor() as u32;
    if notes > cues.hold_ticks {
        sounds.write(Cue::Tick);
    }
    cues.hold_ticks = notes;
}
/// The nearest cluster of meals: things the ball could take right now, not crumbs, within
/// `CLUSTER_RANGE` on the ground plane. Things are counted on a grid of `CLUSTER_CELL` and each
/// cell scored with its eight neighbours; the best cell is the nearest with at least three
/// things (a lone thing counts only when nothing better exists), weighted toward the bigger
/// clusters. Returns the cell's centroid.
fn find_cluster(from: Vec3, radius: f32, items: impl Iterator<Item = (Vec3, f32)>) -> Option<Vec3> {
    let cap = radius * COLLECT_RATIO;
    let mut cells: HashMap<(i32, i32), (u32, Vec3)> = HashMap::default();
    for (pos, pickup) in items {
        if pickup > cap || pickup < cap * CRUMB {
            continue;
        }
        if Vec2::new(pos.x - from.x, pos.z - from.z).length() > CLUSTER_RANGE {
            continue;
        }
        let key = (
            (pos.x / CLUSTER_CELL).floor() as i32,
            (pos.z / CLUSTER_CELL).floor() as i32,
        );
        let e = cells.entry(key).or_default();
        e.0 += 1;
        e.1 += pos;
    }
    let mut best: Option<(f32, Vec3)> = None;
    for (&(cx, cz), &(n, _)) in &cells {
        if n == 0 {
            continue;
        }
        let mut count = 0;
        let mut total = Vec3::ZERO;
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(&(m, s)) = cells.get(&(cx + dx, cz + dz)) {
                    count += m;
                    total += s;
                }
            }
        }
        let centre = total / count as f32;
        let dist = Vec2::new(centre.x - from.x, centre.z - from.z).length();
        // Lower is better: near, and rich; a cluster of three or more beats any lone thing.
        let score = dist - 4.0 * count.min(8) as f32 + if count < 3 { 400.0 } else { 0.0 };
        if best.is_none_or(|(b, _)| score < b) {
            best = Some((score, centre));
        }
    }
    best.map(|(_, at)| at)
}

/// What the guide arrow points at, and the "Hogger is ready" moment.
///
/// Never a nag: Hogger is announced once a run; the arrow to him shows only while he is edible and
/// off screen; and a stalled player (no pickup for `PLATEAU_SECS` of play) gets one arrow toward
/// the nearest meal for `HINT_SECS`, at most `HINT_MAX` times and `HINT_COOLDOWN` apart, which any
/// pickup cancels.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn guidance(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    flow: Res<Flow>,
    ball: Res<BallState>,
    design: Res<Design>,
    mut collected: MessageReader<Collected>,
    things: Query<(Entity, &Collectible, &GlobalTransform), Without<Attached>>,
    mut hogger: Local<Option<Entity>>,
    mut test: Local<Option<bool>>,
    mut cues: ResMut<Cues>,
    mut sounds: MessageWriter<GlueSound>,
) {
    let dt = time.delta_secs();
    // `WOW_AZEROLL_HINT_TEST`: the autopilot never stalls, so for captures the plateau is four
    // seconds after the last hint and pickups neither reset nor cancel it.
    let test = *test.get_or_insert_with(|| crate::webenv::var("WOW_AZEROLL_HINT_TEST").is_some());
    if collected.read().count() > 0 && !test {
        cues.last_pickup = run.elapsed;
        cues.hint_left = 0.0;
    }
    cues.hint_cooldown = (cues.hint_cooldown - dt).max(0.0);
    if let Some(age) = cues.ready_age.as_mut() {
        *age += dt;
    }
    if cues
        .ready_age
        .is_some_and(|a| a > READY_IN + READY_HOLD + READY_OUT)
    {
        cues.ready_age = None;
    }
    let live = *phase.get() == Phase::Playing && !run.won && flow.finale.is_none();
    if !live {
        cues.guide = GuideKind::None;
        return;
    }

    // Only a creature goal has anything to point at.
    let mut boss = (*hogger).and_then(|e| things.get(e).ok());
    if boss.is_none() && design.goal.kind == GoalKind::Collect {
        boss = things
            .iter()
            .find(|(_, c, _)| c.kind == Kind::Boss && c.name == design.goal.target.as_str());
        *hogger = boss.map(|b| b.0);
    }
    let edible = boss.is_some_and(|(_, c, _)| contact(ball.radius, c.pickup) == Contact::RollUp);
    if edible && !cues.ready_seen {
        cues.ready_seen = true;
        cues.ready_age = Some(0.0);
        cues.bloom = 1.0;
        sounds.write(GlueSound(BANNER_SOUND));
    }
    if let (true, Some((_, _, gt))) = (edible, boss) {
        cues.guide = GuideKind::Hogger;
        cues.target = gt.translation() + Vec3::Y;
        cues.hint_left = 0.0;
        return;
    }

    if cues.hint_left > 0.0 {
        cues.hint_left -= dt;
        cues.rescan -= dt;
        if cues.rescan <= 0.0 {
            cues.rescan = 1.0;
            if let Some(at) = cluster_of(&things, &ball) {
                cues.target = at;
            }
        }
        if cues.hint_left <= 0.0 {
            cues.guide = GuideKind::None;
        }
        return;
    }
    cues.guide = GuideKind::None;
    let idle = run.elapsed - cues.last_pickup;
    let plateau = if test { 4.0 } else { PLATEAU_SECS };
    if idle > plateau && cues.hint_cooldown <= 0.0 && cues.hints < HINT_MAX {
        match cluster_of(&things, &ball) {
            Some(at) => {
                cues.target = at;
                cues.guide = GuideKind::Cluster;
                cues.hint_left = HINT_SECS;
                cues.rescan = 1.0;
                cues.hints += 1;
                cues.hint_cooldown = HINT_SECS + HINT_COOLDOWN;
            }
            // Nothing worth pointing at: look again in a few seconds, not every frame.
            None => cues.hint_cooldown = 8.0,
        }
    }
}

/// The nearest meal cluster to the ball among the things still loose in the world.
#[allow(clippy::type_complexity)]
fn cluster_of(
    things: &Query<(Entity, &Collectible, &GlobalTransform), Without<Attached>>,
    ball: &BallState,
) -> Option<Vec3> {
    find_cluster(
        ball.pos,
        ball.radius,
        things
            .iter()
            .filter(|(_, c, _)| c.kind != Kind::Boss)
            .map(|(_, c, gt)| (gt.translation(), c.pickup)),
    )
}

/// Place and turn the guide arrow. Off screen it rides the screen's edge toward the target; a
/// meal cluster that is on screen gets the arrow bobbing above it, pointing down. Hogger, once on
/// screen, needs none.
fn refresh_guide(
    time: Res<Time<Real>>,
    mut cues: ResMut<Cues>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    mut mats: ResMut<Assets<HudMaterial>>,
    mut arrow: Query<(&WidgetMat, &mut Node, &mut UiTransform, &mut Visibility), With<GuideArrow>>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let Ok((cam, cgt)) = cameras.single() else {
        return;
    };
    let Some(view) = cam
        .logical_viewport_size()
        .filter(|v| v.min_element() > 1.0)
    else {
        return;
    };
    let local = cgt.affine().inverse().transform_point3(cues.target);
    let ndc = cam.world_to_ndc(cgt, cues.target);
    let on_screen = local.z < 0.0 && ndc.is_some_and(|n| n.x.abs() < 0.85 && n.y.abs() < 0.8);

    let want = match cues.guide {
        GuideKind::None => 0.0,
        GuideKind::Hogger if on_screen => 0.0,
        _ => 1.0,
    };
    let k = 1.0 - (-ARROW_RATE * dt).exp();
    cues.guide_alpha += (want - cues.guide_alpha) * k;
    let alpha = cues.guide_alpha;

    let (x, y, angle) = match (on_screen, ndc) {
        (true, Some(n)) => {
            // Above the target, bobbing down at it.
            let bob = 1.2 * (now * 4.0).sin();
            (
                (n.x + 1.0) * 50.0,
                (1.0 - n.y) * 50.0 - 9.0 - bob,
                std::f32::consts::PI,
            )
        }
        _ => {
            // The screen-space direction toward it; directly behind, point down.
            let mut dir = Vec2::new(local.x, local.y);
            if dir.length() < 1e-3 {
                dir = Vec2::NEG_Y;
            }
            let dir = dir.normalize();
            let ndc_dir = Vec2::new(dir.x * 2.0 / view.x, dir.y * 2.0 / view.y);
            let reach = (ndc_dir.x.abs() / ARROW_MARGIN.x).max(ndc_dir.y.abs() / ARROW_MARGIN.y);
            let at = if reach > 1e-6 {
                ndc_dir / reach
            } else {
                Vec2::ZERO
            };
            ((at.x + 1.0) * 50.0, (1.0 - at.y) * 50.0, dir.x.atan2(dir.y))
        }
    };
    let rgb = if cues.guide == GuideKind::Hogger {
        Vec3::new(1.0, 0.55, 0.12)
    } else {
        Vec3::new(1.0, 0.92, 0.6)
    };
    let a = alpha * 0.8 * (0.82 + 0.18 * (now * 3.2).sin());
    for (mat, mut node, mut tf, mut vis) in &mut arrow {
        if let Some(m) = mats.get_mut(&mat.0) {
            m.kt.y = now;
            m.kt.z = a;
            m.p = Vec4::new(rgb.x, rgb.y, rgb.z, 0.0);
        }
        let want_vis = if alpha > 0.01 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want_vis {
            *vis = want_vis;
        }
        node.left = Val::Percent(x);
        node.top = Val::Percent(y);
        tf.rotation = Rot2::radians(angle);
    }
}

/// A length as the readout prints it: yards and inches below ten yards, tenths of a yard above.
pub(super) fn fmt_size(yards: f32) -> String {
    if yards >= 100.0 {
        return format!("{yards:.0} yd");
    }
    if yards >= 10.0 {
        return format!("{yards:.1} yd");
    }
    let inches = (yards * 36.0).round().max(0.0) as u32;
    match (inches / 36, inches % 36) {
        (0, i) => format!("{i} in"),
        (y, 0) => format!("{y} yd"),
        (y, i) => format!("{y} yd {i} in"),
    }
}

/// The size as the dial prints it, tight so it sits inside the rosette: `3yd 12in`, then `23.4yd`.
fn fmt_dial(yards: f32) -> String {
    if yards >= 100.0 {
        return format!("{yards:.0}yd");
    }
    if yards >= 10.0 {
        return format!("{yards:.1}yd");
    }
    let inches = (yards * 36.0).round().max(0.0) as u32;
    match (inches / 36, inches % 36) {
        (0, i) => format!("{i}in"),
        (y, 0) => format!("{y}yd"),
        (y, i) => format!("{y}yd {i}in"),
    }
}

/// A time as the records print it: `07:12.4`.
pub(super) fn fmt_time(secs: f32) -> String {
    let tenths = (secs.max(0.0) * 10.0).floor() as u32;
    format!(
        "{:02}:{:02}.{}",
        tenths / 600,
        tenths / 10 % 60,
        tenths % 10
    )
}

/// The size, the clock, the goal (once, at the start), the moment and the banner.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_hud(
    design: Res<super::design::Design>,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    flow: Res<Flow>,
    ball: Res<BallState>,
    cues: Res<Cues>,
    mut shown: Local<Option<f32>>,
    mut last_banner: Local<String>,
    mut texts: Query<(
        &HudText,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
        &mut TextShadow,
    )>,
    mut sounds: MessageWriter<GlueSound>,
) {
    // The number glides to the real size; a fresh run starts on it.
    let target = shown_diameter(&run, &ball, &design, &flow);
    let cur = shown.get_or_insert(target);
    if run.elapsed == 0.0 && run.count == 0 {
        *cur = target;
    } else {
        *cur += (target - *cur) * (1.0 - (-8.0 * time.delta_secs()).exp());
        if (target - *cur).abs() < 0.0005 {
            *cur = target;
        }
    }
    let size = fmt_dial(*cur);

    let left = (design.time_limit - run.elapsed).max(0.0).ceil() as u32;
    let timer = format!("{:02}:{:02}", left / 60, left % 60);
    let timer_color = if left < 60 { RED } else { Color::WHITE };

    // The radius the goal needs: for a creature, the one at which its size is within
    // `COLLECT_RATIO`; for a size goal, half its diameter.
    let need = design.goal_diameter() / 2.0;

    // The goal is said once as the run starts, then fades for good; the hub already said it.
    let started = matches!(phase.get(), Phase::Playing | Phase::Paused) && !run.won;
    let intro = if started {
        ((INTRO_HOLD + INTRO_FADE - run.elapsed) / INTRO_FADE).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let size_goal = design.goal.kind == GoalKind::Size;
    let (goal, sub) = if flow.free_roll {
        (
            "Free Roll".to_string(),
            "No clock. Roll up what you like.".to_string(),
        )
    } else {
        (goal_lines(&design), String::new())
    };

    let (timer, timer_color) = if flow.free_roll {
        ("Free Roll".to_string(), GOLD)
    } else {
        (timer, timer_color)
    };
    // The ring's end says the goal; for a size goal the first seconds say it big, with a "!" (this
    // is the whole intro). After the goal it names the next star.
    let mut ring_scale = 1.0;
    let ring_goal = if flow.free_roll {
        String::new()
    } else if run.won {
        match next_star(&design, run.stars).filter(|_| size_goal) {
            Some((_, at)) => format!("Next {}", fmt_dial(at)),
            None => String::new(),
        }
    } else if size_goal && intro > 0.0 && run.elapsed < INTRO_HOLD {
        ring_scale = INTRO_SCALE;
        format!("{}!", fmt_dial(2.0 * need))
    } else {
        format!("Goal {}", fmt_dial(2.0 * need))
    };

    let mut banner = String::new();
    let mut banner_alpha = 1.0;
    if *phase.get() != Phase::Results {
        if let Some((msg, secs_left)) = &run.banner {
            banner = msg.clone();
            banner_alpha = (secs_left / BANNER_FADE).clamp(0.0, 1.0);
        }
    }
    if !banner.is_empty() && *last_banner != banner {
        sounds.write(GlueSound(BANNER_SOUND));
    }
    if *last_banner != banner {
        *last_banner = banner.clone();
    }

    // "<Boss> is ready": pops in, holds, fades.
    let (moment, moment_alpha, moment_pop) = match cues.ready_age {
        Some(age) => {
            let a_in = (age / READY_IN).clamp(0.0, 1.0);
            let a_out = ((READY_IN + READY_HOLD + READY_OUT - age) / READY_OUT).clamp(0.0, 1.0);
            (
                format!("{} is ready!", design.goal.target),
                a_in.min(a_out),
                1.0 - (age / 0.35).clamp(0.0, 1.0),
            )
        }
        None => (String::new(), 0.0, 0.0),
    };

    for (which, mut text, mut color, mut font, mut shadow) in &mut texts {
        let (s, c) = match which {
            HudText::Size => (size.clone(), Color::WHITE),
            HudText::RingGoal => {
                let size = 17.0 * ring_scale;
                if (font.font_size - size).abs() > 0.5 {
                    font.font_size = size;
                }
                (
                    ring_goal.clone(),
                    if ring_scale > 1.0 { GOLD } else { PARCHMENT },
                )
            }
            // Set by `refresh_widgets`, with the bubble they label.
            HudText::ItemName | HudText::Combo => continue,
            HudText::Timer => (timer.clone(), timer_color),
            HudText::Goal => (goal.clone(), GOLD.with_alpha(intro)),
            HudText::GoalSub => (sub.clone(), PARCHMENT.with_alpha(intro)),
            HudText::Banner => (banner.clone(), GOLD.with_alpha(banner_alpha)),
            HudText::Moment => {
                let size = 34.0 + 16.0 * moment_pop * moment_pop;
                if (font.font_size - size).abs() > 0.5 {
                    font.font_size = size;
                }
                (moment.clone(), Color::srgba(1.0, 0.6, 0.15, moment_alpha))
            }
        };
        if text.0 != s {
            text.0 = s;
        }
        if color.0 != c {
            color.0 = c;
        }
        // The shadow fades with its text, or a fading line leaves a dark ghost.
        let shade = Color::BLACK.with_alpha(c.alpha());
        if shadow.color != shade {
            shadow.color = shade;
        }
    }
}

/// The goal said at the start of a run: one centred line for a single-target goal. A size goal is
/// said on the dial's ring instead, so it has none here; the level's title is the hub's, not play's.
fn goal_lines(design: &Design) -> String {
    match design.goal.kind {
        GoalKind::Size => String::new(),
        GoalKind::Collect | GoalKind::Finale => format!("Roll up {}!", design.goal.target),
    }
}

/// The D26 terminal display size: what the finale's title and the dial agree on.
const FINALE_DISPLAY: f32 = 595.0;

/// The size the readouts show: the finale's display size once it is staged, else the ball's own.
fn shown_diameter(run: &Run, ball: &BallState, design: &Design, flow: &Flow) -> f32 {
    run.final_diameter.unwrap_or_else(|| {
        if design.goal.kind == GoalKind::Finale && flow.finale.is_some() {
            FINALE_DISPLAY
        } else {
            ball.diameter()
        }
    })
}

/// The next star's size on the ring after the goal: the stars lit so far, and the diameter the next
/// needs; `None` once all four are lit.
fn next_star(design: &Design, stars: u8) -> Option<(u8, f32)> {
    let bounds = design
        .level
        .grades
        .unwrap_or(super::progress::DEFAULT_SIZE_GRADES);
    let goal = design.goal_diameter();
    (stars >= 1 && (stars as usize) <= bounds.len())
        .then(|| (stars + 1, bounds[stars as usize - 1] * goal))
}

/// How full the re-armed ring is between the star just lit and the next.
fn star_fill(design: &Design, stars: u8, diameter: f32) -> f32 {
    let goal = design.goal_diameter();
    match next_star(design, stars) {
        None => 1.0,
        Some((_, upper)) => {
            let bounds = design
                .level
                .grades
                .unwrap_or(super::progress::DEFAULT_SIZE_GRADES);
            let lower = if stars <= 1 {
                goal
            } else {
                bounds[stars as usize - 2] * goal
            };
            ((diameter - lower) / (upper - lower).max(1e-3)).clamp(0.0, 1.0)
        }
    }
}

/// What the dial animates between frames.
#[derive(Default)]
struct DialAnim {
    /// The ring's drawn fill.
    fill: f32,
    prev_diameter: f32,
    /// A flash and swell after the ball grows.
    pulse: f32,
    /// 0..1 once the goal is met: the ring completes and the rosette celebrates.
    celebrate: f32,
}

/// The dial, the clock and the two model bubbles: their shader uniforms and the little motions
/// around them (the growth pulse and ring tick, the tier bloom, the pickup pop and colour flash,
/// the combo counter, the Prince's dash lean and wince).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_widgets(
    design: Res<super::design::Design>,
    time: Res<Time<Real>>,
    run: Res<Run>,
    flow: Res<Flow>,
    ball: Res<BallState>,
    bubble: Res<ItemBubble>,
    cues: Res<Cues>,
    mut mats: ResMut<Assets<HudMaterial>>,
    mut anim: Local<DialAnim>,
    widgets: Query<(&Widget, &WidgetMat, Option<&mut UiTransform>)>,
    mut names: Query<
        (
            &HudText,
            &mut Text,
            &mut TextColor,
            &mut TextShadow,
            Option<&mut UiTransform>,
        ),
        Without<Widget>,
    >,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();

    // The ring fills toward the ball size the goal needs (the same measure the goal text gives).
    let need = design.goal_diameter() / 2.0;
    let size_stars = design.goal.kind == GoalKind::Size && !flow.free_roll && run.won;
    let target = if size_stars {
        // After the goal the ring re-arms toward the next star.
        star_fill(&design, run.stars, ball.diameter())
    } else if run.won {
        1.0
    } else {
        (ball.radius / need).clamp(0.0, 1.0)
    };
    if run.elapsed == 0.0 && run.count == 0 {
        anim.fill = target;
        anim.prev_diameter = ball.diameter();
    }
    let k = 1.0 - (-FILL_RATE * dt).exp();
    anim.fill += (target - anim.fill) * k;
    if (target - anim.fill).abs() < 0.0005 {
        anim.fill = target;
    }
    let growth = (ball.diameter() - anim.prev_diameter) / anim.prev_diameter.max(0.01);
    anim.prev_diameter = ball.diameter();
    if growth > 0.004 {
        anim.pulse = anim.pulse.max((growth * 14.0).clamp(0.35, 1.0));
    }
    anim.pulse = (anim.pulse - dt * 3.5).max(0.0);
    let pulse = anim.pulse.max(cues.pulse);
    let cel_target = if run.won { 1.0 } else { 0.0 };
    anim.celebrate += (cel_target - anim.celebrate) * (1.0 - (-3.0 * dt).exp());

    // The clock drains; the last minute turns it red.
    let limit = design.time_limit.max(1.0);
    let elapsed = (run.elapsed / limit).clamp(0.0, 1.0);
    let left = (limit - run.elapsed).max(0.0);
    let urgent = (1.0 - left / 60.0).clamp(0.0, 1.0);

    // The bubble's rim flashes the colour of the thing, harder the bigger it was for the ball.
    let rgb = bubble
        .kind
        .map_or(Q_WHITE, |kind| quality(kind, bubble.pickup, ball.radius));
    let size_fill = bubble
        .kind
        .map_or(0.0, |kind| fill(kind, bubble.pickup, ball.radius));
    let item_flash = bubble.pop * (0.3 + 0.7 * size_fill);

    for (widget, mat, tf) in widgets {
        let Some(m) = mats.get_mut(&mat.0) else {
            continue;
        };
        m.kt.y = now;
        match widget {
            Widget::Dial => {
                // Kind 10+ is the dial with stars: 10 + the stars lit + the newest one's pop.
                m.kt.x = if size_stars && run.stars > 0 {
                    10.0 + f32::from(run.stars.min(4)) + 0.99 * cues.star_pop
                } else {
                    0.0
                };
                m.p = Vec4::new(anim.fill, anim.celebrate, cues.tick, cues.bloom);
                m.kt.w = pulse;
                if let Some(mut tf) = tf {
                    let swell = (cues.bloom * std::f32::consts::PI).sin();
                    let s = 1.0
                        + 0.07 * pulse
                        + 0.06 * swell
                        + 0.03 * anim.celebrate * (now * 4.0).sin().abs();
                    tf.scale = Vec2::splat(s);
                }
            }
            Widget::Clock => {
                m.p = Vec4::new(elapsed, urgent, f32::from(flow.free_roll), flow.call_hold);
            }
            Widget::Item => {
                m.kt.z = bubble.alpha;
                m.kt.w = item_flash;
                m.p = Vec4::new(0.0, rgb[0], rgb[1], rgb[2]);
                if let Some(mut tf) = tf {
                    let s = 1.0 + 0.22 * bubble.pop * bubble.pop;
                    tf.scale = Vec2::splat(s);
                }
            }
            Widget::Prince => {
                m.kt.z = bubble.prince_alpha;
                m.kt.w = bubble.prince_flash;
                let c = bubble.prince_flash_rgb;
                m.p = Vec4::new(1.0, c[0], c[1], c[2]);
                if let Some(mut tf) = tf {
                    // Dashing leans him in; a wince rattles the whole corner.
                    let shake = bubble.prince_shake;
                    tf.scale = Vec2::splat(1.0 + 0.06 * bubble.prince_effort);
                    tf.rotation = Rot2::radians((now * 61.0).sin() * 0.07 * shake);
                    tf.translation = Val2::px(
                        (now * 83.0).sin() * 7.0 * shake,
                        (now * 71.0).cos() * 4.0 * shake,
                    );
                }
            }
        }
    }

    for (which, mut text, mut color, mut shadow, tf) in &mut names {
        match which {
            HudText::ItemName => {
                if text.0 != bubble.name {
                    text.0.clone_from(&bubble.name);
                }
                let c = Color::srgba(rgb[0], rgb[1], rgb[2], bubble.name_alpha);
                if color.0 != c {
                    color.0 = c;
                    shadow.color = Color::BLACK.with_alpha(bubble.name_alpha);
                }
            }
            HudText::Combo => {
                let s = if bubble.combo_alpha > 0.01 {
                    format!("\u{d7}{}", bubble.combo)
                } else {
                    String::new()
                };
                if text.0 != s {
                    text.0 = s;
                }
                // Warms from gold toward orange as the chain grows.
                let heat = (bubble.combo as f32 / 24.0).clamp(0.0, 1.0);
                let c = Color::srgba(
                    1.0,
                    0.86 - 0.34 * heat,
                    0.2 - 0.08 * heat,
                    bubble.combo_alpha,
                );
                if color.0 != c {
                    color.0 = c;
                    shadow.color = Color::BLACK.with_alpha(bubble.combo_alpha);
                }
                if let Some(mut tf) = tf {
                    tf.scale = Vec2::splat(1.0 + 0.45 * bubble.combo_pop * bubble.combo_pop);
                    tf.rotation = Rot2::degrees(-8.0);
                }
            }
            _ => {}
        }
    }
}

/// The celebration: the banner pops in, holds and fades; the line under it gives the run's time,
/// size and count; a white flash opens it.
fn refresh_finale(
    flow: Res<Flow>,
    run: Res<Run>,
    ball: Res<BallState>,
    design: Res<Design>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut texts: Query<(
        &FinaleText,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
        &mut TextShadow,
        &mut Node,
    )>,
    mut flash: Query<&mut BackgroundColor, With<FinaleFlash>>,
) {
    let Some(t) = flow.finale else { return };
    let fade_out = (1.0 - (t - (flow.finale_len - 0.5)) / 0.5).clamp(0.0, 1.0);
    let width = UI_HEIGHT * windows.single().map_or(1.0, super::select::aspect_of);
    for (which, mut text, mut color, mut font, mut shadow, mut node) in &mut texts {
        match which {
            FinaleText::Title => {
                let headline = design.win_title().to_uppercase();
                let size = fit_title(headline.chars().count(), width);
                if text.0 != headline {
                    text.0 = headline;
                }
                if (font.font_size - size).abs() > 0.5 {
                    font.font_size = size;
                }
                // Between the dial and the clock when it fits there, else under them (the dial's
                // label ends near a quarter of the height).
                let room = width - GOAL_INSET_LEFT - GOAL_INSET_RIGHT;
                let wide = size * 0.58 * text.0.chars().count() as f32 <= room;
                let top = Val::Percent(if wide { 0.0 } else { 19.0 });
                if node.margin.top != top {
                    node.margin.top = top;
                }
                let alpha = if design.goal.kind == GoalKind::Finale {
                    ((t - 2.3) / 0.2).clamp(0.0, 1.0) * fade_out
                } else {
                    (t / 0.15).clamp(0.0, 1.0) * fade_out
                };
                color.0 = GOLD.with_alpha(alpha);
                shadow.color = Color::BLACK.with_alpha(alpha);
            }
            FinaleText::Sub => {
                let s = format!(
                    "{}   \u{b7}   {}   \u{b7}   {} rolled up",
                    fmt_time(run.elapsed),
                    fmt_size(shown_diameter(&run, &ball, &design, &flow)),
                    run.count
                );
                if text.0 != s {
                    text.0 = s;
                }
                let alpha = ((t - 0.5) / 0.4).clamp(0.0, 1.0) * fade_out;
                color.0 = PARCHMENT.with_alpha(alpha);
                shadow.color = Color::BLACK.with_alpha(alpha);
            }
        }
    }
    for mut bg in &mut flash {
        bg.0 = Color::srgba(1.0, 0.95, 0.75, 0.6 * (1.0 - t / 0.6).clamp(0.0, 1.0));
    }
}

/// The win title's size so it fits one line across the screen's `width` UI units: about 0.58 em
/// per capital, 90 % of the width, between 28 and 64.
fn fit_title(chars: usize, width: f32) -> f32 {
    (width * 0.9 / (0.58 * chars.max(1) as f32)).clamp(28.0, 64.0)
}

/// Fizzwick's porthole: starts on a [`FizzSays`], types the line, holds, slides away. It runs on
/// real time, so a pause menu does not freeze it (it is small and in a corner), and it gives the
/// commissioner stage its gesture and its talking flag.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_pop(
    real: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    ritual: Res<super::ritual::Ritual>,
    mut says: MessageReader<FizzSays>,
    mut pop: ResMut<Pop>,
    mut who: ResMut<Commissioner>,
    mut hud_mats: ResMut<Assets<HudMaterial>>,
    mut art_mats: ResMut<Assets<ArtMaterial>>,
    mut slide: Query<(&mut UiTransform, &mut Visibility), With<FizzSlide>>,
    porthole: Query<&PortholeMat>,
    bubble: Query<(&Art, &ComputedNode), With<FizzBubble>>,
    mut texts: Query<&mut Text, With<FizzText>>,
) {
    let dt = real.delta_secs();
    for s in says.read() {
        pop.waiting = Some((*s, 0.0));
    }
    // It starts once his stage is warm: the window never shows a camera's first, empty frames.
    if let Some((s, waited)) = pop.waiting.as_mut() {
        *waited += dt;
        if who.alpha >= 0.95 {
            let s = *s;
            pop.waiting = None;
            pop.line = s.line;
            pop.age = Some(0.0);
            pop.hold = POP_HOLD.max(s.line.chars().count() as f32 / POP_CPS + 1.4);
            // The gnome's own gesture for the feeling: cheer, laugh or a cry for the lost run.
            who.gesture = match s.voice {
                Voice::Cheer => Some(68),
                Voice::Laugh => Some(70),
                Voice::Sigh => Some(77),
                Voice::Gasp => Some(64),
                Voice::Talk => None,
            };
        } else if *waited > POP_WAIT {
            pop.waiting = None;
        }
    }
    // A pop-in belongs to play (and to the lost run's moment before the panel docks).
    let place = match phase.get() {
        Phase::Playing | Phase::Paused => true,
        Phase::Results => !ritual.docked || ritual.leaving(),
        _ => false,
    };
    if !place {
        pop.age = None;
        pop.waiting = None;
    }
    let mut x = 1.0;
    let mut typed = 0;
    if let Some(age) = pop.age.as_mut() {
        *age += dt;
        let a = *age;
        let total = POP_SLIDE * 2.0 + pop.hold;
        if a >= total {
            pop.age = None;
        } else {
            let into = (a / POP_SLIDE).clamp(0.0, 1.0);
            let out = ((total - a) / POP_SLIDE).clamp(0.0, 1.0);
            x = 1.0 - super::ritual::ease(into.min(out));
            typed = (((a - POP_SLIDE).max(0.0)) * POP_CPS) as usize;
        }
    }
    let shown = pop.age.is_some();
    who.pop = shown;
    who.talking = shown && typed < pop.line.chars().count();
    for (mut tf, mut vis) in &mut slide {
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        tf.translation = Val2::px(x * (PORTHOLE + POP_BUBBLE + 40.0), 0.0);
    }
    for mut t in &mut texts {
        let s: String = pop.line.chars().take(typed).collect();
        if t.0 != s {
            t.0 = s;
        }
    }
    for p in &porthole {
        if let Some(m) = hud_mats.get_mut(&p.0) {
            m.kt.y = real.elapsed_secs();
            m.kt.z = if shown { who.alpha } else { 0.0 };
        }
    }
    for (Art(h), node) in &bubble {
        if let Some(m) = art_mats.get_mut(h) {
            let logical = node.size().y * node.inverse_scale_factor();
            if logical > 1.0 && (m.data.lv[0].x - logical).abs() > 0.5 {
                m.data.lv[0].x = logical;
            }
            m.data.kt.y = real.elapsed_secs();
        }
    }
}

/// The pause screen carries what play no longer prints: the requirement and the tally.
fn refresh_pause(
    run: Res<Run>,
    ball: Res<BallState>,
    flow: Res<Flow>,
    design: Res<Design>,
    mut texts: Query<(&PauseText, &mut Text)>,
) {
    let need = design.goal_diameter() / 2.0;
    let target = design.goal.target.clone();
    for (which, mut text) in &mut texts {
        let s = match which {
            PauseText::Goal => match design.goal.kind {
                GoalKind::Collect if run.won => format!("{target} is rolled up."),
                GoalKind::Size if run.won && !flow.free_roll => {
                    "Goal reached. Finish whenever you like.".to_string()
                }
                GoalKind::Size if run.won => "Goal reached.".to_string(),
                GoalKind::Collect if ball.radius >= need => {
                    format!("Big enough now: roll up {target}!")
                }
                GoalKind::Collect => format!(
                    "Roll up {target}: grow to {} across ({:.0}%)",
                    fmt_size(2.0 * need),
                    (ball.radius / need * 100.0).clamp(0.0, 100.0)
                ),
                GoalKind::Size | GoalKind::Finale => format!(
                    "Grow to {} across ({:.0}%)",
                    fmt_size(2.0 * need),
                    (ball.radius / need * 100.0).clamp(0.0, 100.0)
                ),
            },
            PauseText::Detail => {
                if flow.free_roll {
                    format!(
                        "{}  \u{b7}  {} rolled up",
                        fmt_size(ball.diameter()),
                        run.count
                    )
                } else {
                    format!(
                        "{}  \u{b7}  {}  \u{b7}  {} rolled up",
                        fmt_time(run.elapsed),
                        fmt_size(ball.diameter()),
                        run.count
                    )
                }
            }
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// The title's bests and the Free Roll offer.
fn refresh_ready(
    progress: Res<Progress>,
    current: Res<CurrentLevel>,
    design: Res<Design>,
    mut texts: Query<(&ReadyText, &mut Text)>,
) {
    let best = progress.best(current.id());
    for (which, mut text) in &mut texts {
        let s = match which {
            ReadyText::Level => format!("{}\n{}", design.level.title, design.level.objective),
            ReadyText::Best => {
                if best.best_time.is_none() && best.most.is_none() {
                    format!("Win {} to set a best time.", design.level.title)
                } else {
                    format!(
                        "Best time {}   \u{b7}   Biggest {}\nMost rolled up {}   \u{b7}   Challenges {}/{}",
                        best.best_time.map_or("--".to_string(), fmt_time),
                        best.biggest.map_or("--".to_string(), fmt_size),
                        best.most.map_or("--".to_string(), |n| n.to_string()),
                        best.challenges.len(),
                        design.challenges.len()
                    )
                }
            }
            ReadyText::Free => {
                if best.wins > 0 {
                    "Press F for Free Roll: no timer, no pressure".to_string()
                } else {
                    String::new()
                }
            }
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    #[test]
    fn cluster_prefers_a_group_over_a_nearer_lone_thing() {
        let radius = 1.0;
        let cap = radius * COLLECT_RATIO;
        let meal = cap * 0.6;
        let mut items = vec![(at(8.0, 0.0), meal)];
        for i in 0..5 {
            items.push((at(40.0 + i as f32 * 0.8, 3.0), meal));
        }
        let got = find_cluster(Vec3::ZERO, radius, items.into_iter()).unwrap();
        assert!((got.x - 41.6).abs() < 1.5, "{got:?}");
    }

    #[test]
    fn cluster_skips_crumbs_giants_and_the_out_of_range() {
        let radius = 1.0;
        let cap = radius * COLLECT_RATIO;
        let items = vec![
            (at(5.0, 0.0), cap * 0.05),
            (at(6.0, 0.0), cap * 1.5),
            (at(CLUSTER_RANGE + 20.0, 0.0), cap * 0.6),
        ];
        assert!(find_cluster(Vec3::ZERO, radius, items.into_iter()).is_none());
    }

    #[test]
    fn a_lone_meal_is_still_better_than_nothing() {
        let radius = 1.0;
        let cap = radius * COLLECT_RATIO;
        let got = find_cluster(Vec3::ZERO, radius, [(at(12.0, 4.0), cap * 0.5)].into_iter());
        assert!(got.is_some());
    }

    #[test]
    fn sizes_print_in_inches_then_yards_then_whole_yards() {
        assert_eq!(fmt_size(0.5), "18 in");
        assert_eq!(fmt_size(2.4), "2 yd 14 in");
        assert_eq!(fmt_size(36.0 / 36.0 * 7.0), "7 yd");
        assert_eq!(fmt_size(23.44), "23.4 yd");
        assert_eq!(fmt_size(595.0), "595 yd");
        assert_eq!(fmt_dial(2.4), "2yd 14in");
        assert_eq!(fmt_dial(595.0), "595yd");
    }

    #[test]
    fn the_dial_and_the_finale_agree_on_the_display_size() {
        let design =
            crate::azeroll::design::parse(crate::azeroll::campaign::LEVELS[2].recipe).unwrap();
        let run = Run::default();
        let ball = BallState::default();
        let mut flow = Flow::default();
        assert!(shown_diameter(&run, &ball, &design, &flow) < 10.0);
        flow.finale = Some(0.5);
        assert_eq!(shown_diameter(&run, &ball, &design, &flow), FINALE_DISPLAY);
    }

    #[test]
    fn the_ring_rearms_toward_the_next_star() {
        let design =
            crate::azeroll::design::parse(crate::azeroll::campaign::LEVELS[0].recipe).unwrap();
        let goal = design.goal_diameter();
        assert_eq!(star_fill(&design, 1, goal), 0.0);
        assert!((star_fill(&design, 1, goal * 1.1) - 0.5).abs() < 0.02);
        assert_eq!(star_fill(&design, 4, goal * 3.0), 1.0);
        assert_eq!(next_star(&design, 4), None);
        assert_eq!(next_star(&design, 1).map(|n| n.0), Some(2));
    }

    #[test]
    fn titles_fit_one_line_at_every_aspect() {
        // The three shipped win titles, narrow and wide.
        for title in [
            "LAST CALL!",
            "PRIZE PUMPKIN, ALL ROLLED UP!",
            "THE CATHEDRAL IS ROLLED UP!",
        ] {
            for width in [706.0, 1024.0, 1365.0] {
                let size = fit_title(title.chars().count(), width);
                assert!((28.0..=64.0).contains(&size));
                // At the larger sizes the line really is narrower than the screen.
                if size > 28.0 {
                    assert!(size * 0.58 * title.chars().count() as f32 <= width * 0.9 + 0.5);
                }
            }
        }
        assert!(fit_title(10, 1365.0) == 64.0 && fit_title(80, 706.0) == 28.0);
    }

    #[test]
    fn fill_counts_hogger_whole() {
        assert_eq!(fill(Kind::Boss, 0.1, 5.0), 1.0);
        assert!(fill(Kind::Prop, 0.1, 5.0) < 0.1);
    }
}
