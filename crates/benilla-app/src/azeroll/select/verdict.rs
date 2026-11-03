//! The results screen: a graded verdict (D24), docked beside the lifted clump (D31). A win stamps a
//! rosette medal with its grade (Scraped by, Respectable, Splendid, Preposterous!), names the run's
//! best find and the star it lights, flies a comet when the goal beat par, and announces any level
//! it opened; the commissioner passes judgement from his window. A loss says so and offers the
//! retry first. The world is not dimmed: the clump hangs lit in the left of the screen (or the top,
//! on a narrow one) with the final size under it, and the panel takes the rest. The numbers come
//! from `progress::Outcome` (judged and saved once, as the results open); this module only dresses
//! them. The ways out are `ritual`'s: the buttons and keys only ask (`ritual::Wish`).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::hub::{kind, style, Art, ArtMaterial};
use crate::azeroll::campaign::{Campaign, CurrentLevel};
use crate::azeroll::design::{Design, GoalKind};
use crate::azeroll::hud::{fmt_size, fmt_time};
use crate::azeroll::hud_models::{Commissioner, HudModels};
use crate::azeroll::lines;
use crate::azeroll::progress::{star_title, Flow, Grade};
use crate::azeroll::ritual::{self, Ritual, Wish};
use crate::azeroll::{BallState, Phase, Rng, Run};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const DIM: Color = Color::srgb(0.62, 0.6, 0.55);
const PARCHMENT: Color = Color::srgb(0.88, 0.80, 0.62);
const GREEN: Color = Color::srgb(0.45, 0.88, 0.45);
const INK: Color = Color::srgb(0.16, 0.10, 0.05);
const MAX_CHALLENGES: usize = 6;
/// Seconds after the panel docks at which each beat lands.
const MEDAL_AT: f32 = 0.05;
const MEDAL_SECS: f32 = 0.45;
const GRADE_AT: f32 = 0.25;
const JUDGE_AT: f32 = 0.4;
const UNLOCK_AT: f32 = 0.55;
/// The panel slides in over this long.
const SLIDE_SECS: f32 = 0.35;
/// The panel's share of the screen: width when wide, height when stacked.
const PANEL_WIDE: f32 = 46.0;
const PANEL_COMPACT: f32 = 48.0;

/// The grade's colour in text.
pub(super) fn grade_color(grade: Grade) -> Color {
    match grade {
        Grade::Scraped => Color::srgb(0.88, 0.68, 0.45),
        Grade::Respectable => Color::srgb(0.5, 0.92, 0.62),
        Grade::Splendid => Color::srgb(0.58, 0.78, 1.0),
        Grade::Preposterous => Color::srgb(1.0, 0.62, 0.95),
    }
}

/// The medal's stamp progress `0..1` at `t` seconds.
pub(super) fn stamp(t: f32) -> f32 {
    ((t - MEDAL_AT) / MEDAL_SECS).clamp(0.0, 1.0)
}

#[derive(Component)]
struct VerdictRoot;

/// The panel that docks.
#[derive(Component)]
struct CardNode;

/// Parts that only the wide layout shows.
#[derive(Component)]
struct WideOnly;

/// Parts that only the stacked layout shows.
#[derive(Component)]
struct CompactOnly;

#[derive(Component)]
struct MedalNode;

/// The headline beside the clump.
#[derive(Component)]
struct HeadBlock;

#[derive(Component)]
struct Fade(f32);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Line {
    Headline,
    HeadTitle,
    Title,
    Grade,
    Detail,
    Comet,
    Remark,
    Goal,
    Time,
    Size,
    Count,
    GoalNote,
    TimeNote,
    SizeNote,
    CountNote,
    Find,
    Star,
    Catalogue,
    Parcel,
    Cousin,
    Most,
    ChallengesHeader,
    Challenge(usize),
    Unlocked,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Act {
    Hub,
    Retry,
    Free,
}

/// A button's place in the row; the act it carries follows the outcome (a lost run offers the
/// retry first).
#[derive(Component)]
struct Slot(usize);

#[derive(Resource, Default)]
struct Clock {
    /// Seconds since the panel docked.
    t: f32,
    judged: bool,
    /// Fizzwick's remark, picked once per results.
    remark: &'static str,
}

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

fn build(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    models: Res<HudModels>,
    mut mats: ResMut<Assets<ArtMaterial>>,
) {
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;
    let mut art = |kind: f32, p: Vec4, tex: &Handle<Image>| {
        let h = mats.add(ArtMaterial::new(kind, p, tex));
        (Art(h.clone()), MaterialNode(h))
    };
    commands
        .spawn((
            VerdictRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(440),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            // --- The headline, under the clump. ---
            root.spawn((
                HeadBlock,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(2.0),
                    width: Val::Percent(50.0),
                    top: Val::Percent(74.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ))
            .with_children(|col| {
                col.spawn((Line::Headline, centered(f, "", 56.0, GOLD)));
                col.spawn((Line::HeadTitle, centered(f, "", 24.0, PARCHMENT)));
            });

            // --- The panel. ---
            root.spawn((
                CardNode,
                art(
                    kind::PLAQUE,
                    Vec4::new(style::CARD, 0.0, 0.0, 0.5),
                    &models.blank,
                ),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Percent(2.0),
                    top: Val::Percent(3.0),
                    bottom: Val::Percent(3.0),
                    width: Val::Percent(PANEL_WIDE),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(16.0)),
                    row_gap: Val::Px(5.0),
                    ..default()
                },
                UiTransform::default(),
            ))
            .with_children(|card| {
                // The verdict: the medal beside the title, the grade, the comet.
                card.spawn(Node {
                    column_gap: Val::Px(14.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        MedalNode,
                        art(kind::MEDAL, Vec4::ZERO, &models.blank),
                        Node {
                            width: Val::Px(130.0),
                            height: Val::Px(130.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn((Line::Title, text(f, "", 20.0, GOLD)));
                        col.spawn((Line::Grade, Fade(GRADE_AT), text(f, "", 38.0, GOLD)));
                        col.spawn((Line::Detail, Fade(GRADE_AT), text(f, "", 14.0, PARCHMENT)));
                        col.spawn((
                            Line::Comet,
                            Fade(JUDGE_AT),
                            text(f, "", 15.0, Color::srgb(0.75, 0.9, 1.0)),
                        ));
                    });
                });
                // The commissioner's judgement.
                card.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(92.0),
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Stretch,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    CompactFizz,
                ))
                .with_children(|row| {
                    row.spawn((
                        art(kind::PORTRAIT, Vec4::ZERO, &models.commissioner),
                        Node {
                            width: Val::Px(72.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        NeedsHeight,
                        FizzPortrait,
                    ));
                    row.spawn((
                        art(
                            kind::PLAQUE,
                            Vec4::new(style::BUBBLE, 0.0, 0.0, 0.42),
                            &models.blank,
                        ),
                        Node {
                            flex_grow: 1.0,
                            padding: UiRect {
                                left: Val::Px(30.0),
                                right: Val::Px(12.0),
                                top: Val::Px(8.0),
                                bottom: Val::Px(8.0),
                            },
                            align_items: AlignItems::Center,
                            margin: UiRect::vertical(Val::Px(10.0)),
                            ..default()
                        },
                        NeedsHeight,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Line::Remark,
                            Fade(JUDGE_AT),
                            Text::new(""),
                            TextFont {
                                font: font.clone(),
                                font_size: 15.0,
                                ..default()
                            },
                            TextColor(INK),
                            // The lines query updates every `Line` with a shadow; this one has
                            // none to show on parchment.
                            TextShadow {
                                offset: Vec2::ZERO,
                                color: Color::NONE,
                            },
                            Node {
                                flex_basis: Val::Px(0.0),
                                flex_grow: 1.0,
                                flex_shrink: 1.0,
                                ..default()
                            },
                        ));
                    });
                });

                // --- The tally. ---
                card.spawn((WideOnly, text(f, "THE TALLY", 12.0, GOLD)));
                for (line, note) in [
                    (Line::Goal, Line::GoalNote),
                    (Line::Time, Line::TimeNote),
                    (Line::Size, Line::SizeNote),
                    (Line::Count, Line::CountNote),
                ] {
                    card.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Baseline,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((line, text(f, "", 20.0, Color::WHITE)));
                        row.spawn((note, Fade(JUDGE_AT), text(f, "", 14.0, GOLD)));
                    });
                }
                card.spawn(Node {
                    height: Val::Px(2.0),
                    margin: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                });
                card.spawn((Line::Find, text(f, "", 16.0, Color::WHITE)));
                card.spawn((
                    Line::Star,
                    Fade(JUDGE_AT),
                    text(f, "", 15.0, Color::srgb(1.0, 0.93, 0.7)),
                ));
                card.spawn((Line::Catalogue, Fade(JUDGE_AT), text(f, "", 14.0, GREEN)));
                card.spawn((Line::Parcel, Fade(JUDGE_AT), text(f, "", 14.0, GOLD)));
                card.spawn((Line::Cousin, Fade(JUDGE_AT), text(f, "", 14.0, GOLD)));
                card.spawn((WideOnly, Line::Most, text(f, "", 13.0, PARCHMENT)));
                card.spawn((Line::ChallengesHeader, text(f, "", 12.0, GOLD)))
                    .insert(Node {
                        margin: UiRect::top(Val::Px(3.0)),
                        ..default()
                    });
                for i in 0..MAX_CHALLENGES {
                    card.spawn((WideOnly, Line::Challenge(i), text(f, "", 14.0, DIM)));
                }
                card.spawn((Line::Unlocked, Fade(UNLOCK_AT), text(f, "", 20.0, GREEN)))
                    .insert(Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    });
                // The ways out.
                card.spawn(Node {
                    column_gap: Val::Px(8.0),
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                })
                .with_children(|row| {
                    for (slot, (act, width)) in
                        [(Act::Hub, 40.0), (Act::Retry, 28.0), (Act::Free, 30.0)]
                            .into_iter()
                            .enumerate()
                    {
                        row.spawn((
                            act,
                            Slot(slot),
                            Button,
                            art(
                                kind::PLAQUE,
                                Vec4::new(style::BUTTON, 0.0, 0.0, 0.5),
                                &models.blank,
                            ),
                            Node {
                                width: Val::Percent(width),
                                height: Val::Px(38.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|b| {
                            b.spawn((ActLabel(slot), centered(f, "", 13.0, Color::WHITE)));
                        });
                    }
                });
            });
        });
}

/// Fizzwick's window; the stacked layout keeps it (shorter) but gives it less room.
#[derive(Component)]
struct CompactFizz;

#[derive(Component)]
struct FizzPortrait;

/// What a button says for its act.
fn label(act: Act) -> &'static str {
    match act {
        Act::Hub => "Back to the hub  (Enter)",
        Act::Retry => "Roll again  (R)",
        Act::Free => "Free Roll  (F)",
    }
}

#[derive(Component)]
struct ActLabel(usize);

/// A plaque or portrait inside the tally whose on-screen height the shader needs told.
#[derive(Component)]
struct NeedsHeight;

fn open(mut clock: ResMut<Clock>) {
    *clock = Clock::default();
}

/// The panel's layout follows the window: docked right when wide, along the bottom when not.
#[allow(clippy::type_complexity)]
fn shape(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: Query<(
        &mut Node,
        Option<&CardNode>,
        Option<&WideOnly>,
        Option<&CompactOnly>,
        Option<&MedalNode>,
        Option<&HeadBlock>,
        Option<&Line>,
        Option<&CompactFizz>,
        Option<&mut TextFont>,
    )>,
) {
    let Ok(window) = windows.single() else { return };
    let comp = ritual::composition(super::aspect_of(window));
    let compact = !comp.wide;
    for (mut node, card, wide, stacked, medal, head, line, fizz, font) in &mut nodes {
        if card.is_some() {
            let (left, right, top, bottom, width, height, pad) = if compact {
                (
                    Val::Percent(2.0),
                    Val::Percent(2.0),
                    Val::Auto,
                    Val::Percent(1.5),
                    Val::Auto,
                    Val::Percent(PANEL_COMPACT),
                    UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                )
            } else {
                (
                    Val::Auto,
                    Val::Percent(2.0),
                    Val::Percent(3.0),
                    Val::Percent(3.0),
                    Val::Percent(PANEL_WIDE),
                    Val::Auto,
                    UiRect::axes(Val::Px(22.0), Val::Px(16.0)),
                )
            };
            if node.left != left || node.height != height {
                node.left = left;
                node.right = right;
                node.top = top;
                node.bottom = bottom;
                node.width = width;
                node.height = height;
                node.padding = pad;
                node.row_gap = Val::Px(if compact { 2.0 } else { 5.0 });
            }
        }
        if wide.is_some() || stacked.is_some() {
            let show = if wide.is_some() { !compact } else { compact };
            let d = if show { Display::Flex } else { Display::None };
            if node.display != d {
                node.display = d;
            }
        }
        if medal.is_some() {
            let s = Val::Px(if compact { 74.0 } else { 130.0 });
            if node.width != s {
                node.width = s;
                node.height = s;
            }
        }
        if head.is_some() {
            let (left, top, width) = (
                Val::Percent((comp.headline.x * 100.0 - 25.0).max(0.0)),
                Val::Percent(comp.headline.y * 100.0),
                Val::Percent(50.0),
            );
            if node.left != left || node.top != top {
                node.left = left;
                node.top = top;
                node.width = width;
            }
        }
        if fizz.is_some() {
            let h = Val::Px(if compact { 58.0 } else { 92.0 });
            if node.height != h {
                node.height = h;
            }
        }
        // The stacked panel is short: the tally and the small lines shrink to fit it.
        if let (Some(line), Some(mut font)) = (line, font) {
            let (wide_px, compact_px) = match line {
                Line::Headline => (56.0, 40.0),
                Line::HeadTitle => (24.0, 18.0),
                Line::Title => (20.0, 16.0),
                Line::Grade => (38.0, 28.0),
                Line::Goal | Line::Time | Line::Size | Line::Count => (20.0, 15.0),
                Line::Find => (16.0, 14.0),
                Line::Star | Line::Catalogue => (15.0, 13.0),
                Line::Parcel | Line::Cousin => (15.0, 13.0),
                Line::Unlocked => (20.0, 16.0),
                _ => (font.font_size, font.font_size),
            };
            let px = if compact { compact_px } else { wide_px };
            if font.font_size != px {
                font.font_size = px;
            }
        }
        // What the stacked screen can do without: the challenge list and the modes' small print.
        if matches!(line, Some(Line::Detail)) && compact {
            node.display = Display::None;
        } else if matches!(line, Some(Line::Detail)) {
            node.display = Display::Flex;
        }
    }
}

/// Everything the screen says, from the run and its outcome.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    (time, phase, run, ball, flow, design, ritual, current): (
        Res<Time<Real>>,
        Res<State<Phase>>,
        Res<Run>,
        Res<BallState>,
        Res<Flow>,
        Res<Design>,
        Res<Ritual>,
        Res<crate::azeroll::campaign::CurrentLevel>,
    ),
    (mut rng, mut said, mut clock, mut who, mut mats): (
        ResMut<Rng>,
        ResMut<lines::Said>,
        ResMut<Clock>,
        ResMut<Commissioner>,
        ResMut<Assets<ArtMaterial>>,
    ),
    windows: Query<&Window, With<PrimaryWindow>>,
    mut root: Query<&mut Visibility, With<VerdictRoot>>,
    mut card: Query<(&mut UiTransform, &mut Visibility), (With<CardNode>, Without<VerdictRoot>)>,
    mut lines: Query<(
        &Line,
        &mut Text,
        &mut TextColor,
        &mut TextShadow,
        Option<&Fade>,
    )>,
    mut medal: Query<(&Art, &mut Node), With<MedalNode>>,
    plaques: Query<(&Art, &ComputedNode, Option<&NeedsHeight>), Without<MedalNode>>,
    mut buttons: Query<(&mut Act, &Slot, &Art, &Interaction)>,
    mut labels: Query<(&ActLabel, &mut Text, &mut TextColor), Without<Line>>,
) {
    let on = *phase.get() == Phase::Results && ritual.ending.is_some();
    for mut vis in &mut root {
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    if !on {
        return;
    }
    let docked = ritual.docked;
    if docked {
        clock.t += time.delta_secs().min(0.1);
    }
    let t = clock.t;
    let out = &flow.outcome;
    // What the run reports as its size (a terminal trophy's, if the level has one).
    let shown_size = run.final_diameter.unwrap_or_else(|| ball.diameter());
    let verdict = out.verdict.filter(|_| run.won);
    let size_goal = design.goal.kind == GoalKind::Size;
    if clock.remark.is_empty() {
        clock.remark = match verdict {
            Some(v) => v.grade.remark(rng.next_u64()),
            None if !ritual.fail_line.is_empty() => ritual.fail_line,
            None => lines::pick(&lines::TRY_AGAIN, current.0, &mut said, rng.next_u64()),
        };
    }
    // The panel slides in as it docks and out as the player leaves.
    let slide_in = 1.0 - ritual::ease(t / SLIDE_SECS);
    let slide = (slide_in + ritual.panel_out()).clamp(0.0, 1.0);
    for (mut tf, mut vis) in &mut card {
        let want = if docked {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        let wide = windows
            .single()
            .is_ok_and(|w| ritual::composition(super::aspect_of(w)).wide);
        tf.translation = if wide {
            Val2::percent(slide * 115.0, 0.0)
        } else {
            Val2::percent(0.0, slide * 115.0)
        };
    }
    // The biggest thing rolled up: judged into the outcome on a win, read off the run on a loss.
    let find: Option<(String, f32)> = out
        .find
        .clone()
        .or_else(|| run.biggest.as_ref().map(|(n, s)| (n.to_string(), s * 2.0)));
    let proper: Vec<&str> = std::iter::once(design.goal.target.as_str())
        .chain(design.roster.iter().map(|s| s.name.as_str()))
        .filter(|s| !s.is_empty())
        .collect();
    let mut most: Vec<(&str, u32)> = run.log.iter().map(|(n, &c)| (&**n, c)).collect();
    most.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let most_line = most
        .iter()
        .take(3)
        .map(|(n, c)| format!("{n} x{c}"))
        .collect::<Vec<_>>()
        .join("   \u{b7}   ");
    let note = |new: bool, best: Option<String>| -> (String, Color) {
        if new {
            ("NEW BEST!".to_string(), GOLD)
        } else {
            (best.map(|b| format!("best {b}")).unwrap_or_default(), DIM)
        }
    };

    // The commissioner reacts once the judgement lands.
    if docked && !clock.judged && t >= JUDGE_AT {
        clock.judged = true;
        who.gesture = Some(match verdict.map(|v| v.grade) {
            Some(Grade::Preposterous) => 70,
            Some(Grade::Splendid) => 68,
            Some(Grade::Respectable) => 64,
            Some(Grade::Scraped) => 65,
            None => 65,
        });
    }

    // The headline beside the clump: the final size, and what it was for.
    let fail = !run.won;
    let head_at = if fail {
        ritual::FAIL_BANNER_AT
    } else {
        ritual::HEADLINE_AT
    };
    let head_alpha = ((ritual.t - head_at) / 0.35).clamp(0.0, 1.0)
        * (1.0 - (ritual.panel_out() * 2.0).clamp(0.0, 1.0));

    for (line, mut text, mut color, mut shadow, fade) in &mut lines {
        let mut alpha_scale = 1.0;
        let (s, c): (String, Color) = match *line {
            Line::Headline => {
                alpha_scale = head_alpha;
                if fail {
                    ("TIME'S UP".to_string(), GOLD)
                } else {
                    (fmt_size(shown_size), GOLD)
                }
            }
            Line::HeadTitle => {
                alpha_scale = head_alpha;
                if fail {
                    (
                        format!(
                            "{} of the {} asked for",
                            fmt_size(ball.diameter()),
                            fmt_size(design.goal_diameter())
                        ),
                        PARCHMENT,
                    )
                } else {
                    (design.win_title(), PARCHMENT)
                }
            }
            Line::Title => {
                if run.won {
                    (design.win_title(), GOLD)
                } else {
                    (design.fail_line(), PARCHMENT)
                }
            }
            Line::Grade => match verdict {
                Some(v) => (v.grade.name().to_uppercase(), grade_color(v.grade)),
                None => ("NOT QUITE".to_string(), Color::srgb(0.8, 0.5, 0.45)),
            },
            Line::Detail => match verdict {
                Some(v) => (
                    format!(
                        "{:.2}x the request{}",
                        v.size_ratio,
                        if run.called { "  \u{b7}  called" } else { "" }
                    ),
                    PARCHMENT,
                ),
                None => (String::new(), PARCHMENT),
            },
            Line::Comet => match verdict {
                Some(v) if v.comet => (
                    format!(
                        "COMET!  Goal at {} (par {})",
                        fmt_time(run.goal_time.unwrap_or(run.elapsed)),
                        fmt_time(v.par)
                    ),
                    Color::srgb(0.75, 0.9, 1.0),
                ),
                Some(v) => (
                    format!("Par was {}: no comet this time", fmt_time(v.par)),
                    DIM,
                ),
                None => (String::new(), DIM),
            },
            Line::Remark => (clock.remark.to_string(), INK),
            Line::Goal => match (size_goal, run.goal_time) {
                (true, Some(g)) => (format!("Goal at   {}", fmt_time(g)), Color::WHITE),
                _ => (String::new(), Color::WHITE),
            },
            Line::Time => (
                if run.called {
                    format!("Called at   {}", fmt_time(run.elapsed))
                } else if size_goal && run.won {
                    format!("Time   {}  (the candle)", fmt_time(run.elapsed))
                } else {
                    format!("Time   {}", fmt_time(run.elapsed))
                },
                Color::WHITE,
            ),
            Line::Size => (format!("Size   {}", fmt_size(shown_size)), Color::WHITE),
            Line::Count => (format!("Rolled up   {}", run.count), Color::WHITE),
            Line::GoalNote => {
                if size_goal && run.won {
                    note(out.new_time, out.before.best_time.map(fmt_time))
                } else {
                    (String::new(), DIM)
                }
            }
            Line::TimeNote => {
                if run.won && !size_goal {
                    note(out.new_time, out.before.best_time.map(fmt_time))
                } else {
                    (String::new(), DIM)
                }
            }
            Line::SizeNote => note(out.new_size, out.before.biggest.map(fmt_size)),
            Line::CountNote => note(out.new_count, out.before.most.map(|n| n.to_string())),
            Line::Find => (
                match &find {
                    Some((name, across)) => format!("Best find: {name} ({})", fmt_size(*across)),
                    None => "Best find: nothing at all".to_string(),
                },
                Color::WHITE,
            ),
            Line::Star => match (&out.find, run.won) {
                (Some((name, _)), true) => {
                    let title = star_title(name, &proper);
                    (
                        if out.new_star {
                            format!("A new star for the dome: {title}")
                        } else {
                            format!("{title} keeps its star")
                        },
                        Color::srgb(1.0, 0.93, 0.7),
                    )
                }
                _ => (String::new(), DIM),
            },
            Line::Catalogue => (catalogue_line(&out.new_catalogue), GREEN),
            Line::Parcel => (parcel_line(out.parcel.as_ref()), GOLD),
            Line::Cousin => (cousin_line(out.cousin.as_ref()), GOLD),
            Line::Most => (
                if most_line.is_empty() {
                    String::new()
                } else {
                    format!("Most rolled up:  {most_line}")
                },
                PARCHMENT,
            ),
            Line::ChallengesHeader => (
                if out.challenges.is_empty() {
                    String::new()
                } else {
                    let met = out.challenges.iter().filter(|c| c.met || c.earned).count();
                    format!("OPTIONAL CHALLENGES  {}/{}", met, out.challenges.len())
                },
                GOLD,
            ),
            Line::Challenge(i) => match out.challenges.get(i) {
                Some(c) if c.met => (
                    format!("[X] {}{}", c.title, if c.fresh { "   NEW!" } else { "" }),
                    GOLD,
                ),
                Some(c) if c.earned => (format!("[X] {}   (earned before)", c.title), GREEN),
                Some(c) => (format!("[ ] {}", c.title), DIM),
                None => (String::new(), DIM),
            },
            Line::Unlocked => (
                match out.unlocked.first() {
                    Some(title) => format!("New level unlocked: {title}"),
                    None => String::new(),
                },
                // It pulses a little, bright to green.
                GREEN.mix(&Color::WHITE, 0.25 + 0.25 * (t * 5.0).sin()),
            ),
        };
        let alpha = match fade {
            Some(Fade(at)) => ((t - at) / 0.15).clamp(0.0, 1.0),
            None => 1.0,
        } * alpha_scale;
        if text.0 != s {
            text.0 = s;
        }
        let c = c.with_alpha(alpha);
        if color.0 != c {
            color.0 = c;
        }
        // The shadow fades with its text: a faded line must not leave a dark ghost.
        let sh = Color::BLACK.with_alpha(alpha);
        if shadow.color != sh && *line != Line::Remark {
            shadow.color = sh;
        }
    }

    // The medal.
    for (Art(h), mut node) in &mut medal {
        // A loss has no medal, and its place is given to the tally.
        let d = if run.won {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != d {
            node.display = d;
        }
        if let Some(m) = mats.get_mut(h) {
            let grade = verdict.map_or(0.0, |v| v.grade as u8 as f32);
            m.data.kt.y = t;
            m.data.kt.z = if run.won { 1.0 } else { 0.0 };
            m.data.p = Vec4::new(
                grade,
                stamp(t),
                f32::from(u8::from(verdict.is_some_and(|v| v.comet))),
                if out.new_grade { 0.7 } else { 0.0 },
            );
        }
    }
    // Plaques and portraits learn their on-screen height so borders keep their thickness.
    for (Art(h), node, size) in &plaques {
        if let Some(m) = mats.get_mut(h) {
            let logical = node.size().y * node.inverse_scale_factor();
            if logical > 1.0 && (m.data.lv[0].x - logical).abs() > 0.5 {
                m.data.lv[0].x = logical;
            }
            m.data.kt.y = t;
            if size.is_some() && m.data.kt.x == kind::PORTRAIT {
                m.data.p.y = if who.talking { 1.0 } else { 0.0 };
                // His window opens once his stage has rendered a real frame.
                m.data.kt.z = who.alpha;
            }
            // The panel lets the lit world show faintly through it.
            if m.data.kt.x == kind::PLAQUE && m.data.p.x == style::CARD {
                m.data.kt.z = 0.93;
            }
        }
    }
    // A lost run offers the retry first, lit.
    let order = if run.won {
        [Act::Hub, Act::Retry, Act::Free]
    } else {
        [Act::Retry, Act::Hub, Act::Free]
    };
    for (mut act, Slot(i), Art(h), interaction) in &mut buttons {
        if *act != order[*i] {
            *act = order[*i];
        }
        if let Some(m) = mats.get_mut(h) {
            let hot = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
            let off = *act == Act::Free && !run.won;
            let lit = if hot && !off {
                1.0
            } else if !run.won && *i == 0 {
                0.7
            } else {
                0.2
            };
            m.data.kt.y = t;
            m.data.p = Vec4::new(style::BUTTON, lit, if off { 0.8 } else { 0.0 }, 0.5);
        }
    }
    for (ActLabel(i), mut text, mut color) in &mut labels {
        let a = order[*i];
        let s = label(a);
        if text.0 != s {
            text.0 = s.to_string();
        }
        let off = a == Act::Free && !run.won;
        let c = Color::WHITE.with_alpha(if off { 0.45 } else { 1.0 });
        if color.0 != c {
            color.0 = c;
        }
    }
}

/// The Catalogue line: how many names were new, and the first few.
/// "PARCEL FOUND: the Monocle   NEW!" (D33); "(again)" and no stamp on a repeat; nothing when the
/// run found none.
pub(super) fn parcel_line(found: Option<&(String, bool)>) -> String {
    match found {
        Some((title, true)) => format!("PARCEL FOUND: {title}   NEW!"),
        Some((title, false)) => format!("PARCEL FOUND: {title} (again)"),
        None => String::new(),
    }
}

/// "COUSIN CAUGHT: Pippa Fizzwick   NEW!", or "swallowed whole" when the ball was past their window.
pub(super) fn cousin_line(found: Option<&(String, bool, bool)>) -> String {
    match found {
        Some((name, fair, new)) => format!(
            "COUSIN {}: {name}{}",
            if *fair { "CAUGHT" } else { "SWALLOWED WHOLE" },
            if *new { "   NEW!" } else { " (again)" }
        ),
        None => String::new(),
    }
}

fn catalogue_line(new: &[String]) -> String {
    if new.is_empty() {
        return String::new();
    }
    let shown = new.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    let more = if new.len() > 3 { " \u{2026}" } else { "" };
    format!("NEW to the Catalogue: {}  \u{b7}  {shown}{more}", new.len())
}

/// The three ways out, by pointer (the keys are `ritual`'s; both only ask).
fn pointer(
    phase: Res<State<Phase>>,
    ritual: Res<Ritual>,
    mut wishes: MessageWriter<Wish>,
    buttons: Query<(&Act, &Interaction), Changed<Interaction>>,
) {
    if *phase.get() != Phase::Results || !ritual.docked {
        return;
    }
    for (act, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        wishes.write(match act {
            Act::Hub => Wish::Hub,
            Act::Retry => Wish::Again,
            Act::Free => Wish::Free,
        });
    }
}

/// `WOW_AZEROLL_RESULT_SHOT=<file.png>` (a capture aid): save the results `WOW_AZEROLL_RESULT_SHOT_AT`
/// seconds (default 1.9) after they open.
fn result_shot(
    mut commands: Commands,
    clock: Res<Clock>,
    ritual: Res<Ritual>,
    phase: Res<State<Phase>>,
    mut done: Local<bool>,
) {
    let Some(path) = crate::webenv::var("WOW_AZEROLL_RESULT_SHOT") else {
        return;
    };
    if *phase.get() != Phase::Results || *done {
        return;
    }
    let at: f32 = crate::webenv::var("WOW_AZEROLL_RESULT_SHOT_AT")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.9);
    if clock.t >= at && ritual.docked {
        *done = true;
        use bevy::render::view::screenshot::{save_to_disk, Screenshot};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::PathBuf::from(path)));
    }
}

/// `WOW_AZEROLL_RESULT_DEMO=<elapsed secs>` (a capture aid): from the hub, fake a won run of the
/// current level (a ball a bit over the goal, that many seconds on the clock, a few finds) and open
/// the results; the real recording and judging then run on it. `WOW_AZEROLL_RESULT_DEMO_LOSS=1`
/// fakes a run that ran out of time instead. The shot exits after it is taken.
#[allow(clippy::too_many_arguments)]
fn demo(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    design: Res<Design>,
    mut run: ResMut<Run>,
    mut ball: ResMut<BallState>,
    mut next: ResMut<NextState<Phase>>,
    mut waited: Local<f32>,
    mut exit: MessageWriter<AppExit>,
    clock: Res<Clock>,
    ritual: Res<Ritual>,
    campaign: Res<Campaign>,
    current: Res<CurrentLevel>,
) {
    let Some(secs) =
        crate::webenv::var("WOW_AZEROLL_RESULT_DEMO").and_then(|v| v.parse::<f32>().ok())
    else {
        return;
    };
    if *phase.get() == Phase::Results {
        let at: f32 = crate::webenv::var("WOW_AZEROLL_RESULT_SHOT_AT")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.9);
        if clock.t >= at + 1.5 && ritual.docked {
            exit.write(AppExit::Success);
        }
        return;
    }
    if *phase.get() != Phase::Select || loading.covering() {
        return;
    }
    *waited += time.delta_secs();
    if *waited < 1.0 {
        return;
    }
    let loss = crate::webenv::var("WOW_AZEROLL_RESULT_DEMO_LOSS").is_some();
    let goal = design.goal_diameter();
    let diameter = if loss { goal * 0.7 } else { goal * 1.3 };
    ball.radius = diameter / 2.0;
    run.elapsed = secs;
    run.won = !loss;
    run.count = 412;
    run.biggest = Some(("Cheese Wheel".into(), 0.4));
    for (name, n) in [
        ("Chicken", 14),
        ("Barrel", 9),
        ("Mug", 7),
        ("Loaf of Bread", 5),
    ] {
        run.log.insert(name.into(), n);
    }
    fake_finds(&mut run, &campaign, &current);
    next.set(Phase::Results);
}

/// `WOW_AZEROLL_FAKE_FINDS=parcel,cousin,swallowed` (a capture aid): the faked run also found the
/// level's parcel and cousin, so the results and the hub's news can be shot without a real pickup.
fn fake_finds(run: &mut Run, campaign: &Campaign, current: &CurrentLevel) {
    let Some(what) = crate::webenv::var("WOW_AZEROLL_FAKE_FINDS") else {
        return;
    };
    run.parcel_found = what.contains("parcel");
    if what.contains("cousin") {
        let key = campaign.levels[current.0]
            .cousin
            .as_ref()
            .map(|c| c.id.clone());
        run.cousin = key.map(|k| (k, !what.contains("swallowed")));
    }
}

/// Where the capture loop is (`WOW_AZEROLL_LOOP_SHOT`).
#[derive(Default)]
struct LoopState {
    in_ready: f32,
    faked: bool,
    was_results: bool,
    back: f32,
    shot: bool,
}

/// `WOW_AZEROLL_LOOP_SHOT=<file.png>` with `WOW_AZEROLL_HUB_KEYS=e` and a save whose next level is
/// open (a capture aid for the whole loop): the hub rolls the chosen level, which loads for real;
/// four seconds into its Ready the run is faked a win and the results open (shoot them with
/// `WOW_AZEROLL_RESULT_SHOT`); two seconds later Enter is pressed for the player, and back in the
/// hub (where the news plays) the shot is taken and the game exits.
#[allow(clippy::too_many_arguments)]
fn capture_loop(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    clock: Res<Clock>,
    mut run: ResMut<Run>,
    mut ball: ResMut<BallState>,
    design: Res<Design>,
    mut next: ResMut<NextState<Phase>>,
    mut st: Local<LoopState>,
    mut exit: MessageWriter<AppExit>,
    ritual: Res<Ritual>,
    mut wishes: MessageWriter<Wish>,
    campaign: Res<Campaign>,
    current: Res<CurrentLevel>,
) {
    let Some(path) = crate::webenv::var("WOW_AZEROLL_LOOP_SHOT") else {
        return;
    };
    let dt = time.delta_secs();
    match phase.get() {
        Phase::Ready if !loading.covering() && !st.faked => {
            st.in_ready += dt;
            if st.in_ready > 4.0 {
                st.faked = true;
                ball.radius = design.goal_diameter() * 0.65;
                run.elapsed = 300.0;
                run.won = true;
                run.count = 250;
                run.biggest = Some(("Cheese Wheel".into(), 0.3));
                for (name, n) in [("Chicken", 12), ("Mug", 8)] {
                    run.log.insert(name.into(), n);
                }
                fake_finds(&mut run, &campaign, &current);
                info!("azeroll loop: level loaded, faking its win");
                next.set(Phase::Results);
            }
        }
        Phase::Results => {
            st.was_results = true;
            if ritual.docked && clock.t > 2.2 && !ritual.leaving() {
                info!("azeroll loop: Enter, back to the hub");
                wishes.write(Wish::Hub);
            }
        }
        Phase::Select if st.was_results && !loading.covering() => {
            st.back += dt;
            if st.back > 3.8 && !st.shot {
                st.shot = true;
                use bevy::render::view::screenshot::{save_to_disk, Screenshot};
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(std::path::PathBuf::from(path)));
            }
            if st.back > 5.5 {
                exit.write(AppExit::Success);
            }
        }
        _ => {}
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Clock>()
        .add_systems(Startup, build.after(crate::azeroll::hud_models::setup))
        .add_systems(OnEnter(Phase::Results), open)
        .add_systems(
            Update,
            (shape, refresh, pointer, result_shot, demo, capture_loop).chain(),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_replay_tally_says_only_what_happened() {
        assert_eq!(parcel_line(None), "");
        assert_eq!(
            parcel_line(Some(&("the Monocle".to_string(), true))),
            "PARCEL FOUND: the Monocle   NEW!"
        );
        assert_eq!(
            parcel_line(Some(&("the Monocle".to_string(), false))),
            "PARCEL FOUND: the Monocle (again)"
        );
        assert_eq!(cousin_line(None), "");
        assert_eq!(
            cousin_line(Some(&("Pippa Fizzwick".to_string(), true, true))),
            "COUSIN CAUGHT: Pippa Fizzwick   NEW!"
        );
        assert_eq!(
            cousin_line(Some(&("Thrum".to_string(), false, false))),
            "COUSIN SWALLOWED WHOLE: Thrum (again)"
        );
    }

    #[test]
    fn the_stamp_lands_after_its_delay_and_stays() {
        assert_eq!(stamp(0.0), 0.0);
        assert_eq!(stamp(MEDAL_AT), 0.0);
        assert!(stamp(MEDAL_AT + MEDAL_SECS * 0.5) > 0.4);
        assert_eq!(stamp(MEDAL_AT + MEDAL_SECS), 1.0);
        assert_eq!(stamp(10.0), 1.0);
    }

    #[test]
    fn every_beat_lands_soon_after_the_panel_docks() {
        // A player should not wait for the fanfare: every beat has landed within a second.
        for at in [MEDAL_AT + MEDAL_SECS, GRADE_AT, JUDGE_AT, UNLOCK_AT] {
            assert!(at + 0.15 <= 0.75, "{at}");
        }
        assert!(GRADE_AT > MEDAL_AT && JUDGE_AT > GRADE_AT && UNLOCK_AT > JUDGE_AT);
    }

    #[test]
    fn the_catalogue_line_counts_and_names_the_first_three() {
        assert_eq!(catalogue_line(&[]), "");
        let names: Vec<String> = ["Chair", "Pillow", "Hearth Cat", "Mug"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let line = catalogue_line(&names);
        assert!(line.contains(": 4") && line.contains("Chair, Pillow, Hearth Cat"));
        assert!(!line.contains("Mug"));
    }

    #[test]
    fn grades_have_distinct_colours() {
        let colours: Vec<Color> = Grade::ALL.iter().map(|&g| grade_color(g)).collect();
        for (i, a) in colours.iter().enumerate() {
            for b in &colours[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
