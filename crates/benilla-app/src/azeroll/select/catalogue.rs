//! Fizzwick's Catalogue on the hub (D31, `docs/azeroll/ritual-design.md` §4): every unique thing the
//! player has ever rolled up, with the level it was first found in and how many ever, grouped by
//! category under the three levels. Light on purpose: no models to unlock, no completion gate, a
//! count per level and one Fizzwick line per thing. `C` (or the line at the foot of the hub) opens
//! it, Esc or `C` closes it; W/S move down the list, A/D change level.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::hub::{kind, style, Art, ArtMaterial};
use crate::azeroll::campaign::{Campaign, LEVELS};
use crate::azeroll::hud::fmt_size;
use crate::azeroll::hud_models::HudModels;
use crate::azeroll::lines;
use crate::azeroll::names::Category;
use crate::azeroll::progress::{CatalogueEntry, Progress, WardrobeKeys};
use crate::azeroll::replay;
use crate::azeroll::Phase;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const DIM: Color = Color::srgb(0.55, 0.52, 0.47);
const PARCHMENT: Color = Color::srgb(0.88, 0.80, 0.62);
const WHITE: Color = Color::srgb(0.96, 0.94, 0.9);
/// Rows of the level list on screen at once.
const ROWS: usize = 12;
/// The family group above it: a header, the Prince, up to three outfits and three cousins.
const FAMILY_SLOTS: usize = 8;
/// Fizzwick's reply to Enter types over the detail pane for this long, seconds, at this speed.
const SAID_SECS: f32 = 9.0;
const SAID_CPS: f32 = 45.0;

/// What the page shows.
#[derive(Resource, Default, Debug)]
pub(super) struct CatalogueView {
    pub open: bool,
    /// The level whose list is shown, an index into [`LEVELS`].
    pub level: usize,
    /// The selected row of the flat row list: the family group first, then the level's list.
    pub row: usize,
    /// What Fizzwick said to the last Enter, how long ago (real seconds), and on which row.
    pub said: Option<(&'static str, f32, usize)>,
}

/// One line of the list.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Row {
    Header(Category, usize),
    /// A name, its count (0 = not found yet).
    Entry(String, u32),
    /// The family group's header: found of all six.
    FamilyHeader(usize, usize),
    /// A selectable member of the family group (D33).
    Family(Family),
}

/// What a family row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FamilyKind {
    Prince,
    Outfit,
    Cousin,
}

/// How a family row stands against the wardrobe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wear {
    /// Found, not in use: Enter puts it on.
    Free,
    /// The outfit on the Prince, or the Prince himself when nobody else pushes, or the pusher.
    On,
    /// An outfit that a pusher hides (the pusher wins).
    Hidden,
}

/// One member of the family: the Prince himself, an outfit (a parcel's reward) or a cousin.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Family {
    pub kind: FamilyKind,
    /// The wardrobe key; empty for the Prince.
    pub key: String,
    pub name: String,
    /// The level that hides it, an index into [`LEVELS`] (the Prince has none).
    pub level: Option<usize>,
    pub found: bool,
    /// A cousin caught inside their window (the Catalogue's note).
    pub fair: bool,
    pub wear: Wear,
}

/// The family group: the header, the Prince and a row for every outfit and cousin the recipes
/// hide, found or not. Pure, so the layout is testable.
pub(super) fn family(campaign: &Campaign, progress: &Progress) -> Vec<Row> {
    let keys = progress.wardrobe();
    let known = replay::resolve(&keys, campaign);
    let mut out = vec![Row::Family(Family {
        kind: FamilyKind::Prince,
        key: String::new(),
        name: "The Prince".into(),
        level: None,
        found: true,
        fair: false,
        wear: if known.pusher.is_none() && known.outfit.is_none() {
            Wear::On
        } else {
            Wear::Free
        },
    })];
    for (ix, info) in campaign.levels.iter().enumerate() {
        let Some(p) = &info.present else { continue };
        let best = progress.best(&info.meta.id);
        let worn = keys.outfit.as_deref() == Some(p.id.as_str()) && known.outfit.is_some();
        out.push(Row::Family(Family {
            kind: FamilyKind::Outfit,
            key: p.id.clone(),
            name: p.title.clone(),
            level: Some(ix),
            found: best.parcel,
            fair: false,
            wear: match (worn, known.pusher.is_some()) {
                (true, false) => Wear::On,
                (true, true) => Wear::Hidden,
                _ => Wear::Free,
            },
        }));
    }
    for (ix, info) in campaign.levels.iter().enumerate() {
        let Some(c) = &info.cousin else { continue };
        let best = progress.best(&info.meta.id);
        out.push(Row::Family(Family {
            kind: FamilyKind::Cousin,
            key: c.id.clone(),
            name: c.name.clone(),
            level: Some(ix),
            found: best.cousin,
            fair: best.cousin_fair,
            wear: if keys.pusher.as_deref() == Some(c.id.as_str()) && known.pusher.is_some() {
                Wear::On
            } else {
                Wear::Free
            },
        }));
    }
    let found = out
        .iter()
        .filter(|r| matches!(r, Row::Family(f) if f.kind != FamilyKind::Prince && f.found))
        .count();
    let all = out.len() - 1;
    out.insert(0, Row::FamilyHeader(found, all));
    out
}

/// What Enter on a family row does to the wardrobe: the Prince restores him; an outfit is worn (or,
/// worn already, taken off) and sends the pusher home; a cousin pushes (or, pushing already, hands
/// the ball back). Not-found rows do nothing.
pub(super) fn equip(keys: &WardrobeKeys, row: &Family) -> Option<WardrobeKeys> {
    if !row.found {
        return None;
    }
    let mut next = keys.clone();
    match row.kind {
        FamilyKind::Prince => next = WardrobeKeys::default(),
        FamilyKind::Outfit => {
            next.pusher = None;
            next.outfit = if row.wear == Wear::On {
                None
            } else {
                Some(row.key.clone())
            };
        }
        FamilyKind::Cousin => {
            next.pusher = if row.wear == Wear::On {
                None
            } else {
                Some(row.key.clone())
            };
        }
    }
    Some(next)
}

/// The list for a level: its recipe's names (found or not) and anything first found there that the
/// recipe does not lay out, grouped by category with the found ones first.
pub(super) fn rows(
    level_id: &str,
    recipe: &[(String, u8)],
    saved: &BTreeMap<String, CatalogueEntry>,
) -> Vec<Row> {
    let mut by_cat: BTreeMap<Category, Vec<(String, u32)>> = BTreeMap::new();
    for (name, cat) in recipe {
        let n = saved.get(name).map_or(0, |e| e.n);
        by_cat
            .entry(Category::from_index(*cat))
            .or_default()
            .push((name.clone(), n));
    }
    for (name, e) in saved {
        if e.level == level_id && !recipe.iter().any(|(n, _)| n == name) {
            by_cat
                .entry(Category::from_index(e.cat))
                .or_default()
                .push((name.clone(), e.n));
        }
    }
    let mut out = Vec::new();
    for (cat, mut names) in by_cat {
        // Found first (most rolled first), then the ones still to find, each alphabetical.
        names.sort_by(|a, b| {
            (b.1 > 0)
                .cmp(&(a.1 > 0))
                .then(b.1.cmp(&a.1))
                .then(a.0.cmp(&b.0))
        });
        out.push(Row::Header(cat, names.iter().filter(|n| n.1 > 0).count()));
        out.extend(names.into_iter().map(|(n, c)| Row::Entry(n, c)));
    }
    out
}

/// `found` of `total` names of the recipe, and how many more were found roaming (first found on
/// this level, not in its recipe).
pub(super) fn tally(
    level_id: &str,
    recipe: &[(String, u8)],
    saved: &BTreeMap<String, CatalogueEntry>,
) -> (usize, usize, usize) {
    let found = recipe.iter().filter(|(n, _)| saved.contains_key(n)).count();
    let roaming = saved
        .iter()
        .filter(|(n, e)| e.level == level_id && !recipe.iter().any(|(r, _)| r == *n))
        .count();
    (found, recipe.len(), roaming)
}

/// The first entry row at or after `row`, so the cursor never rests on a header.
fn snap(rows: &[Row], row: usize, forward: bool) -> usize {
    let n = rows.len();
    if n == 0 {
        return 0;
    }
    let mut r = row.min(n - 1);
    for _ in 0..n {
        if matches!(rows[r], Row::Entry(..) | Row::Family(_)) {
            return r;
        }
        r = if forward {
            (r + 1) % n
        } else {
            (r + n - 1) % n
        };
    }
    0
}

#[derive(Component)]
struct CatalogueRoot;

#[derive(Component)]
struct PanelNode;

#[derive(Component)]
struct HintLine;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Level(usize),
    Family(usize),
    Row(usize),
    DetailName,
    DetailFirst,
    DetailSize,
    DetailQuip,
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

fn build(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    models: Res<HudModels>,
    mut mats: ResMut<Assets<ArtMaterial>>,
) {
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;
    let h = mats.add(ArtMaterial::new(
        kind::PLAQUE,
        Vec4::new(style::CARD, 0.0, 0.0, 0.5),
        &models.blank,
    ));
    // The invitation at the foot of the hub.
    commands.spawn((
        HintLine,
        Button,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(6.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-170.0)),
            width: Val::Px(340.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(430),
        Visibility::Hidden,
        children![(
            Field::Title,
            text(f, "", 15.0, PARCHMENT),
            TextLayout::new_with_justify(Justify::Center)
        )],
    ));
    commands
        .spawn((
            CatalogueRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(460),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Block,
        ))
        .with_children(|root| {
            root.spawn((
                PanelNode,
                Art(h.clone()),
                MaterialNode(h.clone()),
                Node {
                    width: Val::Percent(90.0),
                    height: Val::Percent(86.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(26.0), Val::Px(18.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|card| {
                card.spawn((Field::Title, text(f, "", 26.0, GOLD)));
                for i in 0..LEVELS.len() {
                    card.spawn((Field::Level(i), text(f, "", 17.0, WHITE)));
                }
                card.spawn(Node {
                    height: Val::Px(4.0),
                    ..default()
                });
                card.spawn(Node {
                    flex_grow: 1.0,
                    column_gap: Val::Px(24.0),
                    ..default()
                })
                .with_children(|body| {
                    body.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_basis: Val::Percent(60.0),
                        row_gap: Val::Px(1.0),
                        ..default()
                    })
                    .with_children(|list| {
                        for i in 0..FAMILY_SLOTS {
                            list.spawn((Field::Family(i), text(f, "", 16.0, WHITE)));
                        }
                        for i in 0..ROWS {
                            list.spawn((Field::Row(i), text(f, "", 16.0, WHITE)));
                        }
                    });
                    body.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_basis: Val::Percent(40.0),
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|detail| {
                        detail.spawn((Field::DetailName, text(f, "", 28.0, GOLD)));
                        detail.spawn((Field::DetailFirst, text(f, "", 16.0, PARCHMENT)));
                        detail.spawn((Field::DetailSize, text(f, "", 16.0, PARCHMENT)));
                        detail.spawn((Field::DetailQuip, text(f, "", 19.0, WHITE)));
                    });
                });
            });
        });
}

/// The family group's row as text (D33): the cursor, the name, and what it is doing. A row not yet
/// found is three dots and its level's numeral, which is the only hint there is.
fn family_text(row: &Row, here: bool, campaign: &Campaign) -> (String, Color) {
    let cursor = if here { "> " } else { "  " };
    match row {
        Row::FamilyHeader(found, all) => (
            format!("THE FAMILY, AND THE POST  ({found} of {all})"),
            GOLD,
        ),
        Row::Family(f) => {
            let numeral = f
                .level
                .map_or(String::new(), |l| format!("   ({})", super::numeral(l)));
            let indent = if f.kind == FamilyKind::Outfit {
                "    "
            } else {
                ""
            };
            let (s, c) = if f.kind == FamilyKind::Prince {
                (
                    format!(
                        "{cursor}{indent}{}   {}",
                        f.name,
                        if f.wear == Wear::On { "pushing" } else { "" }
                    ),
                    WHITE,
                )
            } else if !f.found {
                (
                    format!("{cursor}{indent}\u{b7} \u{b7} \u{b7}{numeral}"),
                    DIM,
                )
            } else {
                let state = match (f.kind, f.wear) {
                    (_, Wear::On) if f.kind == FamilyKind::Outfit => "wearing",
                    (_, Wear::On) => "pushing",
                    (_, Wear::Hidden) => "owned",
                    (FamilyKind::Outfit, _) => "Enter: wear",
                    _ => "Enter: push",
                };
                (
                    format!("{cursor}{indent}{}   {state}{numeral}", f.name),
                    WHITE,
                )
            };
            let _ = campaign;
            (s, if here { GOLD } else { c })
        }
        _ => (String::new(), WHITE),
    }
}

/// The detail pane's second line for a family row: where it was found, or that it is not yet.
fn family_first(f: &Family, campaign: &Campaign) -> String {
    match (f.kind, f.found, f.level) {
        (FamilyKind::Prince, _, _) => "The one who pushes.".to_string(),
        (_, false, _) => "Not found yet. Keep rolling.".to_string(),
        (FamilyKind::Outfit, true, Some(l)) => {
            format!("A parcel, found in: {}", campaign.levels[l].meta.title)
        }
        (_, true, Some(l)) => format!(
            "{} in: {}",
            if f.fair {
                "Caught fair"
            } else {
                "Swallowed whole"
            },
            campaign.levels[l].meta.title
        ),
        _ => String::new(),
    }
}

/// The detail pane's quip for a family row.
fn family_quip(f: &Family) -> &'static str {
    match (f.kind, f.found) {
        (_, false) => "",
        (FamilyKind::Prince, _) => {
            "Push, boy. Do not look back. Looking back is how you get rolled up."
        }
        (FamilyKind::Outfit, _) => lines::item_quip(lines::PARCEL_QUIP_KEY).unwrap_or(""),
        (FamilyKind::Cousin, _) => lines::item_quip(&f.name).unwrap_or(""),
    }
}

/// The flat list the cursor walks: the family group, then the level's list.
fn combined(campaign: &Campaign, progress: &Progress, level: usize) -> Vec<Row> {
    let mut list = family(campaign, progress);
    list.extend(rows(
        LEVELS[level].id,
        &campaign.levels[level].catalogue,
        progress.catalogue(),
    ));
    list
}

/// The keys: C opens and closes, Esc closes, W/S move the cursor, A/D change level, Enter wears an
/// outfit or picks a pusher while the cursor is in the family group.
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    campaign: Res<Campaign>,
    mut progress: ResMut<Progress>,
    time: Res<Time<Real>>,
    hint: Query<&Interaction, With<HintLine>>,
    mut view: ResMut<CatalogueView>,
) {
    if *phase.get() != Phase::Select || loading.covering() {
        view.open = false;
        return;
    }
    if let Some((_, age, _)) = view.said.as_mut() {
        *age += time.delta_secs();
        if *age > SAID_SECS {
            view.said = None;
        }
    }
    let first_level_row = family(&campaign, &progress).len();
    let clicked = hint.iter().any(|i| *i == Interaction::Pressed);
    if keys.just_pressed(KeyCode::KeyC) || (clicked && !view.open) {
        view.open = !view.open;
        if view.open {
            view.row = first_level_row;
            view.said = None;
        }
        return;
    }
    if !view.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        view.open = false;
        return;
    }
    let n = LEVELS.len();
    if keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        view.level = (view.level + n - 1) % n;
        if view.row >= first_level_row {
            view.row = first_level_row;
        }
    }
    if keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        view.level = (view.level + 1) % n;
        if view.row >= first_level_row {
            view.row = first_level_row;
        }
    }
    let list = combined(&campaign, &progress, view.level);
    if keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        view.row = snap(&list, view.row + 1, true);
    } else if keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        view.row = snap(&list, view.row.saturating_sub(1), false);
    } else {
        view.row = snap(&list, view.row, true);
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        if let Some(Row::Family(f)) = list.get(view.row) {
            if let Some(next) = equip(&progress.wardrobe(), f) {
                progress.set_wardrobe(next);
                // The Prince back (or an outfit or pusher taken off) is the same line.
                let key = match (f.kind, f.wear) {
                    (FamilyKind::Prince, _) | (_, Wear::On) => "",
                    _ => f.key.as_str(),
                };
                view.said = Some((lines::equip_line(key), 0.0, view.row));
                info!("azeroll catalogue: Enter on {:?} ({key:?})", f.name);
            }
        }
    }
}

/// Everything the page and the hint say.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    campaign: Res<Campaign>,
    progress: Res<Progress>,
    view: Res<CatalogueView>,
    time: Res<Time<Real>>,
    mut mats: ResMut<Assets<ArtMaterial>>,
    mut root: Query<&mut Visibility, (With<CatalogueRoot>, Without<HintLine>)>,
    mut hint: Query<&mut Visibility, (With<HintLine>, Without<CatalogueRoot>)>,
    panel: Query<(&Art, &ComputedNode), With<PanelNode>>,
    mut fields: Query<(&Field, &mut Text, &mut TextColor)>,
) {
    let hub = *phase.get() == Phase::Select && !loading.covering();
    for mut v in &mut root {
        let want = if hub && view.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for mut v in &mut hint {
        let want = if hub && !view.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for (Art(h), node) in &panel {
        if let Some(m) = mats.get_mut(h) {
            let logical = node.size().y * node.inverse_scale_factor();
            if logical > 1.0 && (m.data.lv[0].x - logical).abs() > 0.5 {
                m.data.lv[0].x = logical;
            }
            m.data.kt.y = time.elapsed_secs();
            m.data.kt.z = 1.0;
        }
    }
    let saved = progress.catalogue();
    let total_found: usize = campaign
        .levels
        .iter()
        .map(|l| {
            l.catalogue
                .iter()
                .filter(|(n, _)| saved.contains_key(n))
                .count()
        })
        .sum();
    let total: usize = campaign.levels.iter().map(|l| l.catalogue.len()).sum();
    let level = view.level.min(LEVELS.len() - 1);
    let fam = family(&campaign, &progress);
    let list = rows(LEVELS[level].id, &campaign.levels[level].catalogue, saved);
    // The window of level rows round the cursor (the family group above it never scrolls).
    let lvl_row = view.row.saturating_sub(fam.len());
    let first = lvl_row
        .saturating_sub(ROWS * 2 / 3)
        .min(list.len().saturating_sub(ROWS));
    let selected = if view.row < fam.len() {
        fam.get(view.row).cloned()
    } else {
        list.get(lvl_row).cloned()
    };
    let (name, entry) = match &selected {
        Some(Row::Entry(n, _)) => (Some(n.clone()), saved.get(n)),
        Some(Row::Family(f)) => (
            Some(if f.found {
                f.name.clone()
            } else {
                "\u{b7} \u{b7} \u{b7}".to_string()
            }),
            None,
        ),
        _ => (None, None),
    };
    let in_family = view.row < fam.len();
    let said_age = view.said.filter(|(_, _, row)| *row == view.row);
    for (field, mut text, mut color) in &mut fields {
        let (s, c): (String, Color) = match *field {
            Field::Title => {
                if hub && !view.open {
                    (
                        format!("C \u{b7} Fizzwick's Catalogue \u{b7} {total_found} of {total}"),
                        PARCHMENT,
                    )
                } else {
                    (
                        if in_family {
                            "FIZZWICK'S CATALOGUE   (W/S  A/D  Enter wears / pushes  Esc)"
                        } else {
                            "FIZZWICK'S CATALOGUE   (W/S  A/D  Esc)"
                        }
                        .to_string(),
                        GOLD,
                    )
                }
            }
            Field::Level(i) => {
                let info = &campaign.levels[i];
                let (found, of, roaming) = tally(info.meta.id.as_str(), &info.catalogue, saved);
                let roam = if roaming > 0 {
                    format!("  \u{b7}  +{roaming} found roaming")
                } else {
                    String::new()
                };
                (
                    format!(
                        "{}{}  {}   {found} of {of}{roam}",
                        if i == level { "> " } else { "  " },
                        super::numeral(i),
                        info.meta.title
                    ),
                    if i == level { GOLD } else { DIM },
                )
            }
            Field::Family(i) => match fam.get(i) {
                Some(row) => family_text(row, i == view.row, &campaign),
                None => (String::new(), WHITE),
            },
            Field::Row(i) => match list.get(first + i) {
                Some(Row::Header(cat, found)) => {
                    (format!("{}  ({found})", cat.title().to_uppercase()), GOLD)
                }
                Some(Row::Entry(n, count)) => {
                    let here = fam.len() + first + i == view.row;
                    let s = if *count > 0 {
                        format!("{}{n}  x{count}", if here { "> " } else { "    " })
                    } else {
                        format!(
                            "{}{n}  \u{b7} \u{b7} \u{b7}",
                            if here { "> " } else { "    " }
                        )
                    };
                    (
                        s,
                        if here {
                            GOLD
                        } else if *count > 0 {
                            WHITE
                        } else {
                            DIM
                        },
                    )
                }
                _ => (String::new(), WHITE),
            },
            Field::DetailName => (name.clone().unwrap_or_default(), GOLD),
            Field::DetailFirst => (
                match (&name, entry) {
                    _ if in_family => match &selected {
                        Some(Row::Family(f)) => family_first(f, &campaign),
                        _ => String::new(),
                    },
                    (Some(_), Some(e)) => {
                        let first = LEVELS
                            .iter()
                            .position(|l| l.id == e.level)
                            .map_or(e.level.clone(), |ix| campaign.levels[ix].meta.title.clone());
                        format!("First found: {first}  \u{b7}  rolled up x{}", e.n)
                    }
                    (Some(_), None) => "Not found yet. Keep rolling.".to_string(),
                    _ => String::new(),
                },
                PARCHMENT,
            ),
            Field::DetailSize => (
                match entry {
                    _ if in_family => String::new(),
                    Some(e) if e.axis > 0.0 => format!("Biggest seen: {} across", fmt_size(e.axis)),
                    _ => String::new(),
                },
                PARCHMENT,
            ),
            Field::DetailQuip => (
                match (&name, entry) {
                    _ if in_family => match (&selected, said_age) {
                        (_, Some((line, age, _))) => {
                            // Fizzwick types his answer to Enter, then it stays a while.
                            let shown = ((age * SAID_CPS) as usize).min(line.chars().count());
                            line.chars().take(shown).collect()
                        }
                        (Some(Row::Family(f)), None) => family_quip(f).to_string(),
                        _ => String::new(),
                    },
                    (Some(n), Some(e)) => lines::item_quip(n)
                        .unwrap_or(lines::CATALOGUE_CATEGORY[(e.cat as usize).min(7)])
                        .to_string(),
                    _ => String::new(),
                },
                WHITE,
            ),
        };
        if text.0 != s {
            text.0 = s;
        }
        if color.0 != c {
            color.0 = c;
        }
    }
}

/// `WOW_AZEROLL_CATALOGUE_SHOT=<file.png>` (a capture aid): open the page a few seconds after the
/// hub is up (`WOW_AZEROLL_CATALOGUE_LEVEL=<index>` picks the level, `_ROW=<n>` the cursor), save
/// it, then exit.
fn shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut view: ResMut<CatalogueView>,
    mut waited: Local<f32>,
    mut exit: MessageWriter<AppExit>,
    windows: Query<(), With<PrimaryWindow>>,
) {
    let Some(path) = crate::webenv::var("WOW_AZEROLL_CATALOGUE_SHOT") else {
        return;
    };
    if *phase.get() != Phase::Select || loading.covering() || windows.is_empty() {
        return;
    }
    *waited += time.delta_secs();
    if *waited > 2.5 && !view.open && *waited < 3.0 {
        view.open = true;
        view.level = crate::webenv::var("WOW_AZEROLL_CATALOGUE_LEVEL")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        view.row = crate::webenv::var("WOW_AZEROLL_CATALOGUE_ROW")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
    }
    if *waited > 4.0 && *waited < 4.2 {
        use bevy::render::view::screenshot::{save_to_disk, Screenshot};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::PathBuf::from(path)));
        *waited = 4.3;
    }
    if *waited > 5.5 {
        exit.write(AppExit::Success);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CatalogueView>()
        .add_systems(Startup, build.after(crate::azeroll::hud_models::setup))
        .add_systems(Update, (keys, refresh, shot).chain().before(super::keys));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(entries: &[(&str, &str, u32, u8)]) -> BTreeMap<String, CatalogueEntry> {
        entries
            .iter()
            .map(|(n, l, c, cat)| {
                (
                    n.to_string(),
                    CatalogueEntry {
                        level: l.to_string(),
                        n: *c,
                        axis: 0.4,
                        cat: *cat,
                    },
                )
            })
            .collect()
    }

    fn recipe() -> Vec<(String, u8)> {
        vec![
            ("Muffin".into(), Category::Food as u8),
            ("Cheese Wheel".into(), Category::Food as u8),
            ("Chair".into(), Category::Furniture as u8),
        ]
    }

    #[test]
    fn the_list_groups_by_category_found_first_and_adds_roaming_finds() {
        let saved = saved(&[
            ("Muffin", "lions-pride", 9, 0),
            ("Sundial", "lions-pride", 1, 7),
            ("Muffin Crumb", "market-day", 4, 0),
        ]);
        let r = rows("lions-pride", &recipe(), &saved);
        assert_eq!(r[0], Row::Header(Category::Food, 1));
        assert_eq!(r[1], Row::Entry("Muffin".into(), 9));
        assert_eq!(r[2], Row::Entry("Cheese Wheel".into(), 0));
        assert!(r.contains(&Row::Entry("Sundial".into(), 1)), "{r:?}");
        // Another level's find is not listed here.
        assert!(!r
            .iter()
            .any(|x| matches!(x, Row::Entry(n, _) if n == "Muffin Crumb")));
    }

    #[test]
    fn the_tally_counts_found_of_all_and_roaming() {
        let saved = saved(&[
            ("Muffin", "lions-pride", 9, 0),
            ("Sundial", "lions-pride", 1, 7),
        ]);
        assert_eq!(tally("lions-pride", &recipe(), &saved), (1, 3, 1));
        assert_eq!(tally("market-day", &recipe(), &saved), (1, 3, 0));
    }

    fn member(kind: FamilyKind, key: &str, found: bool, wear: Wear) -> Family {
        Family {
            kind,
            key: key.into(),
            name: key.into(),
            level: Some(0),
            found,
            fair: true,
            wear,
        }
    }

    #[test]
    fn the_family_group_starts_with_the_prince_and_hides_what_is_not_found() {
        let campaign = Campaign::load(usize::MAX);
        let mut progress = Progress::for_tests();
        let rows = family(&campaign, &progress);
        assert!(matches!(rows[0], Row::FamilyHeader(0, _)));
        assert!(
            matches!(&rows[1], Row::Family(f) if f.kind == FamilyKind::Prince && f.wear == Wear::On)
        );
        assert!(rows[2..]
            .iter()
            .all(|r| matches!(r, Row::Family(f) if !f.found)));
        assert!(rows.len() <= FAMILY_SLOTS, "{} rows", rows.len());
        // The cursor may rest on a family row, never on the header.
        assert!(matches!(rows[snap(&rows, 0, true)], Row::Family(_)));
        // A found outfit shows as owned and wearable, and a pusher hides it.
        let Some(info) = campaign.levels.iter().find(|l| l.present.is_some()) else {
            return;
        };
        let key = info.present.as_ref().unwrap().id.clone();
        progress.set_best(
            &info.meta.id,
            crate::azeroll::progress::LevelBest {
                parcel: true,
                ..default()
            },
        );
        let row = |p: &Progress| {
            family(&campaign, p)
                .into_iter()
                .find_map(|r| match r {
                    Row::Family(f) if f.key == key => Some(f),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(
            (row(&progress).found, row(&progress).wear),
            (true, Wear::Free)
        );
        progress.set_wardrobe(WardrobeKeys {
            outfit: Some(key.clone()),
            pusher: None,
        });
        assert_eq!(row(&progress).wear, Wear::On);
    }

    #[test]
    fn enter_wears_toggles_and_restores_and_refuses_what_is_not_found() {
        let none = WardrobeKeys::default();
        let outfit = member(FamilyKind::Outfit, "monocle", true, Wear::Free);
        let worn = equip(&none, &outfit).unwrap();
        assert_eq!(worn.outfit.as_deref(), Some("monocle"));
        // Wearing again takes it off.
        let on = member(FamilyKind::Outfit, "monocle", true, Wear::On);
        assert_eq!(equip(&worn, &on).unwrap().outfit, None);
        // A cousin pushes and is handed the ball back by a second Enter; the outfit stays owned.
        let cousin = member(FamilyKind::Cousin, "pippa", true, Wear::Free);
        let pushing = equip(&worn, &cousin).unwrap();
        assert_eq!(
            (pushing.outfit.as_deref(), pushing.pusher.as_deref()),
            (Some("monocle"), Some("pippa"))
        );
        let back = equip(
            &pushing,
            &member(FamilyKind::Cousin, "pippa", true, Wear::On),
        )
        .unwrap();
        assert_eq!(back.pusher, None);
        // Wearing an outfit sends the pusher home; the Prince's own row restores everything.
        assert_eq!(equip(&pushing, &outfit).unwrap().pusher, None);
        let prince = member(FamilyKind::Prince, "", true, Wear::Free);
        assert_eq!(equip(&pushing, &prince).unwrap(), WardrobeKeys::default());
        // Not found: nothing happens.
        assert_eq!(
            equip(
                &none,
                &member(FamilyKind::Cousin, "twinkle", false, Wear::Free)
            ),
            None
        );
    }

    #[test]
    fn the_cursor_never_rests_on_a_header() {
        let r = rows("lions-pride", &recipe(), &BTreeMap::new());
        assert!(matches!(r[snap(&r, 0, true)], Row::Entry(..)));
        for from in 0..r.len() {
            assert!(matches!(r[snap(&r, from, false)], Row::Entry(..)));
        }
        assert_eq!(snap(&[], 3, true), 0);
    }
}
