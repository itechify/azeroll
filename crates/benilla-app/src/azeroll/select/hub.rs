//! The hub's widgets (D24): the planet Azeroth. A full-screen scene (`art.wgsl` kind 0) draws the
//! night, the globe turning (a texture composed from the game's own world-map art, `maps.rs`), a
//! beacon over Elwynn and the named stars of the levels already won; a second widget (kind 6) is
//! the zoomed Elwynn map with a pin for each level. Bevy UI lays the tags, the commissioner's
//! frame and speech bubble and the mission card over them. [`Layout`] places every piece from the
//! window's aspect (wide: planet and inset left, commissioner and card right; compact: stacked),
//! [`Anim`] eases the globe, the inset and the news, and everything reads the plain [`CardView`]s.

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::ui_render::ui_material::UiMaterial;
use bevy::ui_render::UiMaterialPlugin;
use bevy::window::PrimaryWindow;

use super::mapart::MapArt;
use super::{view, CardView, Celebration, Standing};
use crate::azeroll::campaign::{Campaign, Request, SelectState, LEVELS};
use crate::azeroll::hud_models::{self, Commissioner, HudModels};
use crate::azeroll::progress::{Grade, Progress};
use crate::azeroll::Phase;
use crate::loading_screen::LoadingScreen;
use crate::sound::GlueSound;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const PARCHMENT: Color = Color::srgb(0.88, 0.80, 0.62);
const GREEN: Color = Color::srgb(0.45, 0.88, 0.45);
const RED: Color = Color::srgb(1.0, 0.42, 0.32);
const INK: Color = Color::srgb(0.16, 0.10, 0.05);
/// The UI is laid out on a screen 768 units tall, like the HUD's.
pub(super) const UI_HEIGHT: f32 = 768.0;
/// The most levels the scene shader draws.
pub(super) const MAX_LEVELS: usize = 6;
/// Rows past the per-level ones: the globe's aim and the inset's geometry.
const GLOBE: usize = MAX_LEVELS * 3;
const LV_VECS: usize = GLOBE + 6;
/// The most challenge lines the card lists.
const MAX_CHALLENGES: usize = 5;
/// The card buttons' height, UI units (the shader scales their borders by it).
const BUTTON_HEIGHT: f32 = 40.0;

/// The globe is aimed this far west of Goldshire, so Kalimdor shows on its limb too.
const AIM_OFFSET: f32 = 0.30;
/// How far the Elwynn inset is zoomed (1 shows the whole map).
const INSET_ZOOM: f32 = 1.4;
/// The inset's shape: the map's own 1002 x 668.
const INSET_ASPECT: f32 = 1002.0 / 668.0;
const STAR_WAIT: f32 = 0.7;
const STAR_SECS: f32 = 2.2;
const UNLOCK_WAIT: f32 = 2.8;
const UNLOCK_SECS: f32 = 2.0;
/// The commissioner's line types at this many characters a second, at least; a long line is sped
/// up to take no more than `LINE_SECS`.
const MIN_CPS: f32 = 48.0;
const LINE_SECS: f32 = 2.2;

// ---------------------------------------------------------------------------------------------
// The material
// ---------------------------------------------------------------------------------------------

/// What the shader reads: `kt` (kind, seconds, alpha, spare), `p` (kind-specific) and the per-level
/// rows. See `orrery.wgsl`.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(crate) struct Data {
    pub kt: Vec4,
    pub p: Vec4,
    pub lv: [Vec4; LV_VECS],
}

/// The hub's and the results' one `UiMaterial`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct ArtMaterial {
    #[uniform(0)]
    pub data: Data,
    #[texture(1)]
    #[sampler(2)]
    pub tex: Handle<Image>,
}

impl UiMaterial for ArtMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/azeroll/select/art.wgsl".into()
    }
}

impl ArtMaterial {
    pub(crate) fn new(kind: f32, p: Vec4, tex: &Handle<Image>) -> Self {
        let mut data = Data::default();
        data.kt = Vec4::new(kind, 0.0, 1.0, 0.0);
        data.p = p;
        data.lv[0].x = 1.0;
        Self {
            data,
            tex: tex.clone(),
        }
    }
}

/// A node's material handle, for the systems that animate it.
#[derive(Component, Clone)]
pub(crate) struct Art(pub Handle<ArtMaterial>);

/// Shader kinds.
pub(crate) mod kind {
    pub const SCENE: f32 = 0.0;
    pub const PLAQUE: f32 = 1.0;
    pub const SCALE: f32 = 2.0;
    pub const CANDLE: f32 = 3.0;
    pub const MEDAL: f32 = 4.0;
    pub const PORTRAIT: f32 = 5.0;
    pub const INSET: f32 = 6.0;
}

/// Plaque styles (the shader's `p.x`).
pub(crate) mod style {
    pub const CARD: f32 = 0.0;
    pub const TAG: f32 = 1.0;
    pub const BUBBLE: f32 = 2.0;
    pub const BUTTON: f32 = 3.0;
}

// ---------------------------------------------------------------------------------------------
// The layout
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    #[cfg(test)]
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    #[cfg(test)]
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    #[cfg(test)]
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
}

/// Where everything goes, in UI units (the screen is [`UI_HEIGHT`] tall and `width` wide).
#[derive(Clone, Debug)]
pub(super) struct Layout {
    pub compact: bool,
    pub width: f32,
    /// The planet's centre, as a share of the screen, and its radius in screen heights.
    pub centre: Vec2,
    pub u: f32,
    /// The Elwynn inset.
    pub inset: Rect,
    pub tags: Vec<Rect>,
    pub portrait: Rect,
    pub bubble: Rect,
    pub card: Rect,
    pub title: Vec2,
}

/// The aspect below which the hub stacks instead of laying out in columns.
const COMPACT_BELOW: f32 = 1.4;

pub(super) fn layout(aspect: f32, n: usize) -> Layout {
    let h = UI_HEIGHT;
    let w = h * aspect;
    let compact = aspect < COMPACT_BELOW;
    let n = n.max(1);
    if !compact {
        let cw = (w * 0.38).clamp(400.0, 520.0);
        let x0 = w - 16.0 - cw;
        let area = x0 - 12.0;
        let inset_w = 290.0;
        let r = ((area - inset_w - 40.0) * 0.5).min(0.30 * h);
        let cx = (area - inset_w - 20.0) * 0.5 + 10.0;
        let cy = 372.0;
        let gap = 10.0;
        let tw = ((area - 32.0 - gap * (n as f32 - 1.0)) / n as f32).min(260.0);
        let row_w = tw * n as f32 + gap * (n as f32 - 1.0);
        let tx = (area - row_w) * 0.5 + 4.0;
        Layout {
            compact,
            width: w,
            centre: Vec2::new(cx / w, cy / h),
            u: r / h,
            inset: Rect::new(
                area - inset_w - 6.0,
                cy - inset_w / INSET_ASPECT * 0.5 + 24.0,
                inset_w,
                inset_w / INSET_ASPECT,
            ),
            tags: (0..n)
                .map(|i| Rect::new(tx + i as f32 * (tw + gap), 684.0, tw, 62.0))
                .collect(),
            portrait: Rect::new(x0, 10.0, 187.0, 250.0),
            bubble: Rect::new(x0 + 191.0, 22.0, cw - 191.0, 224.0),
            card: Rect::new(x0, 268.0, cw, 484.0),
            title: Vec2::new(22.0, 14.0),
        }
    } else {
        let r = (w * 0.17).min(0.14 * h);
        let cx = r * 1.15 + 10.0;
        let cy = 34.0 + 1.3 * r;
        let inset_w = (w - cx - r - 24.0).clamp(220.0, 300.0);
        let inset = Rect::new(
            w - 8.0 - inset_w,
            cy - 80.0,
            inset_w,
            inset_w / INSET_ASPECT,
        );
        let tag_y = (cy + r + 14.0).max(inset.y + inset.h + 10.0);
        let gap = 8.0;
        let tw = ((w - 24.0 - gap * (n as f32 - 1.0)) / n as f32).min(240.0);
        let row_w = tw * n as f32 + gap * (n as f32 - 1.0);
        let tx = (w - row_w) * 0.5;
        let top = tag_y + 58.0;
        let rest = h - 8.0 - top;
        let bubble_h = (rest * 0.3).max(120.0);
        Layout {
            compact,
            width: w,
            centre: Vec2::new(cx / w, cy / h),
            u: r / h,
            inset,
            tags: (0..n)
                .map(|i| Rect::new(tx + i as f32 * (tw + gap), tag_y, tw, 50.0))
                .collect(),
            portrait: Rect::new(8.0, h - 8.0 - 234.0, 176.0, 234.0),
            bubble: Rect::new(190.0, top, w - 198.0, bubble_h),
            card: Rect::new(
                190.0,
                top + bubble_h + 6.0,
                w - 198.0,
                rest - bubble_h - 6.0,
            ),
            title: Vec2::new(w - 150.0, 10.0),
        }
    }
}

impl Layout {
    /// A point of the planet's own space (its radius 1) in UI units.
    pub fn q_to_px(&self, q: Vec2) -> Vec2 {
        let u_px = self.u * UI_HEIGHT;
        Vec2::new(
            self.centre.x * self.width + q.x * u_px,
            self.centre.y * UI_HEIGHT + q.y * u_px,
        )
    }

    /// Where a map point (a share of the Elwynn art) lies in UI units, the inset showing the map
    /// around `centre` at `zoom`.
    pub fn map_to_px(&self, uv: Vec2, centre: Vec2, zoom: f32) -> Vec2 {
        Vec2::new(
            self.inset.x + self.inset.w * (0.5 + (uv.x - centre.x) * zoom),
            self.inset.y + self.inset.h * (0.5 + (uv.y - centre.y) * zoom),
        )
    }
}

/// Where level `i` of `n` has its star in the sky (planet radii from its centre): an arc over the
/// planet's top.
pub(super) fn star_q(i: usize, n: usize) -> Vec2 {
    let from = -140.0_f32.to_radians();
    let to = -40.0_f32.to_radians();
    let a = if n < 2 {
        -90.0_f32.to_radians()
    } else {
        from + (to - from) * i as f32 / (n - 1) as f32
    };
    Vec2::new(a.cos(), a.sin()) * 1.3
}

// ---------------------------------------------------------------------------------------------
// The animation
// ---------------------------------------------------------------------------------------------

/// What eases between frames. Real time, so a paused or slowed game does not stall the hub.
#[derive(Resource)]
pub(super) struct Anim {
    focus: [f32; MAX_LEVELS],
    /// 1 when settled; running 0 → 1 while the lock opens.
    reveal: [f32; MAX_LEVELS],
    reveal_wait: [Option<f32>; MAX_LEVELS],
    /// 1 when settled; running 0 → 1 while the star flies up from the beacon.
    ignite: [f32; MAX_LEVELS],
    ignite_wait: [Option<f32>; MAX_LEVELS],
    /// Seconds since the hub opened.
    clock: f32,
    /// Seconds the current level's line has been typing, and the level it is.
    typed: f32,
    shown: usize,
    /// The highlight when the hub opened and whether the player has moved it since: a swing to a
    /// level that just opened only happens for a player who has not chosen.
    opened_on: usize,
    swing: Option<usize>,
    /// The line's characters shown.
    line_chars: usize,
    /// Extra turn of the globe, radians: it spins in from far away and settles to nothing.
    spin: f32,
    /// The map point at the inset's centre, easing toward the chosen level's pin.
    inset: Vec2,
    /// 1 falling to 0 when the choice changes: the inset pops.
    pop: f32,
    /// Fizzwick's cheer for a parcel or cousin found since the last visit (D33): the bubble types
    /// it in place of the level's quip until the player moves the highlight.
    news_line: Option<&'static str>,
    /// The highlight the news line was first shown on (set on the first frame, after the hub has
    /// chosen its level): the line holds until the player moves it.
    news_at: Option<usize>,
    /// The Royal Fireworks (D33): seconds since the hub opened while they play (negative: off).
    fireworks: f32,
}

impl Default for Anim {
    fn default() -> Self {
        Self {
            focus: [0.0; MAX_LEVELS],
            reveal: [1.0; MAX_LEVELS],
            reveal_wait: [None; MAX_LEVELS],
            ignite: [1.0; MAX_LEVELS],
            ignite_wait: [None; MAX_LEVELS],
            clock: 0.0,
            typed: 0.0,
            shown: usize::MAX,
            opened_on: 0,
            swing: None,
            line_chars: 0,
            spin: 0.0,
            inset: Vec2::splat(0.5),
            pop: 0.0,
            news_line: None,
            news_at: None,
            fireworks: -1.0,
        }
    }
}

/// How many characters of a `len`-long line show after `typed` seconds.
pub(super) fn chars_shown(typed: f32, len: usize) -> usize {
    let cps = MIN_CPS.max(len as f32 / LINE_SECS);
    ((typed.max(0.0) * cps) as usize).min(len)
}

/// The inset's resting centre for the chosen level: the middle of all the pins, drawn a good way
/// toward the chosen one, but never so far that a pin (with room for its head above it) leaves the
/// window, and always far enough from the edges that the window stays on the map.
pub(super) fn inset_target(pins: &[Vec2], chosen: usize, zoom: f32) -> Vec2 {
    if pins.is_empty() {
        return Vec2::splat(0.5);
    }
    let mid = pins.iter().copied().sum::<Vec2>() / pins.len() as f32;
    let at = pins[chosen.min(pins.len() - 1)];
    let mut c = mid.lerp(at, 0.45);
    let half = 0.5 / zoom;
    // Room round the pins, in map shares: the head and numeral above a pin, a margin elsewhere.
    let (above, side) = (0.2 / zoom, 0.05);
    let lo = pins.iter().fold(Vec2::splat(f32::MAX), |m, p| m.min(*p));
    let hi = pins.iter().fold(Vec2::splat(f32::MIN), |m, p| m.max(*p));
    // The window [c - half, c + half] must hold [lo.x - side, hi.x + side] and
    // [lo.y - above, hi.y + side]; when the pins are too spread for that, centre on them.
    let (x_min, x_max) = (hi.x + side - half, lo.x - side + half);
    let (y_min, y_max) = (hi.y + side - half, lo.y - above + half);
    c.x = if x_min <= x_max {
        c.x.clamp(x_min, x_max)
    } else {
        (lo.x + hi.x) * 0.5
    };
    c.y = if y_min <= y_max {
        c.y.clamp(y_min, y_max)
    } else {
        (lo.y + hi.y) * 0.5
    };
    Vec2::new(c.x.clamp(half, 1.0 - half), c.y.clamp(half, 1.0 - half))
}

/// Two pins nearer than this on the inset (UI units, across and down) would overlap their heads,
/// numerals and badges, so [`fan_out`] moves them apart.
pub(super) const PIN_SEP: Vec2 = Vec2::new(36.0, 42.0);
/// A pin's footprint for placing labels: half its width, and how far it stands above and below
/// its tip (UI units, the chosen pin's 1.3x growth included).
const PIN_HALF_W: f32 = 17.0;
const PIN_ABOVE: f32 = 40.0;
const PIN_BELOW: f32 = 4.0;
/// The chosen pin's name: its height, and the width per character of the 14 pt face.
const NAME_H: f32 = 18.0;
const NAME_CHAR_W: f32 = 7.6;

/// Where each pin is *drawn* for the map points `tips` (UI units): the same place unless two would
/// stand within [`PIN_SEP`], when the pair moves apart (sideways, unless that is far further than
/// moving them up and down) so no two pins overlap; a leader line joins each moved pin to its map
/// point. Only the pins' relative positions matter, so the result does not shift as the inset eases.
pub(super) fn fan_out(tips: &[Vec2]) -> Vec<Vec2> {
    let mut at = tips.to_vec();
    for _ in 0..40 {
        let mut moved = false;
        for i in 0..at.len() {
            for j in i + 1..at.len() {
                let d = at[j] - at[i];
                if d.x.abs() >= PIN_SEP.x || d.y.abs() >= PIN_SEP.y {
                    continue;
                }
                let need = Vec2::new(PIN_SEP.x - d.x.abs(), PIN_SEP.y - d.y.abs());
                let push = if need.x <= need.y + 12.0 {
                    Vec2::new((need.x + 0.05) * if d.x < 0.0 { -0.5 } else { 0.5 }, 0.0)
                } else {
                    Vec2::new(0.0, (need.y + 0.05) * if d.y < 0.0 { -0.5 } else { 0.5 })
                };
                at[j] += push;
                at[i] -= push;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    at
}

/// Each pin's map point and where it is drawn, UI units, the inset showing the map round `centre`.
pub(super) fn pin_places(l: &Layout, pins: &[Vec2], centre: Vec2) -> Vec<(Vec2, Vec2)> {
    let at: Vec<Vec2> = pins
        .iter()
        .map(|p| l.map_to_px(*p, centre, INSET_ZOOM))
        .collect();
    let shown = fan_out(&at);
    at.into_iter().zip(shown).collect()
}

/// Where the chosen pin's name goes: the top-left of a `chars`-long line, UI units. Under the
/// pin if that is clear, else over it, else beside it: whichever side the other pins (and the
/// chosen one's own head) leave clear, kept inside the inset.
pub(super) fn name_slot(shown: &[Vec2], chosen: usize, inset: &Rect, chars: usize) -> (Vec2, f32) {
    let w = (chars as f32 * NAME_CHAR_W + 6.0).min(inset.w - 8.0);
    let Some(&tip) = shown.get(chosen) else {
        return (Vec2::new(inset.x + 4.0, inset.y + 4.0), w);
    };
    let tries = [
        Vec2::new(tip.x - w * 0.5, tip.y + 6.0),
        Vec2::new(tip.x - w * 0.5, tip.y - PIN_ABOVE - 6.0 - NAME_H),
        Vec2::new(tip.x + PIN_HALF_W + 4.0, tip.y - 26.0),
        Vec2::new(tip.x - PIN_HALF_W - 4.0 - w, tip.y - 26.0),
    ];
    let lo = Vec2::new(inset.x + 4.0, inset.y + 4.0);
    let hi = Vec2::new(
        inset.x + inset.w - 4.0 - w,
        inset.y + inset.h - 4.0 - NAME_H,
    );
    let mut best: Option<(f32, Vec2)> = None;
    for (rank, want) in tries.iter().enumerate() {
        let at = want.clamp(lo, hi.max(lo));
        let mut cost = rank as f32 * 0.5 + (at - *want).length();
        for (i, other) in shown.iter().enumerate() {
            // The chosen pin's own head counts for every side but the one under it.
            if i == chosen && rank == 0 {
                continue;
            }
            let ox = (at.x + w).min(other.x + PIN_HALF_W) - at.x.max(other.x - PIN_HALF_W);
            let oy = (at.y + NAME_H).min(other.y + PIN_BELOW) - at.y.max(other.y - PIN_ABOVE);
            if ox > 0.0 && oy > 0.0 {
                cost += 1000.0 + ox * oy;
            }
        }
        if best.is_none_or(|(c, _)| cost < c) {
            best = Some((cost, at));
        }
    }
    (best.map_or(tries[0], |(_, at)| at), w)
}

/// One step of the globe's spin-in, the focus and the inset.
pub(super) fn ease(anim: &mut Anim, highlighted: usize, n: usize, pins: &[Vec2], dt: f32) {
    let f = 1.0 - (-7.0 * dt).exp();
    for i in 0..n.min(MAX_LEVELS) {
        let want = if i == highlighted { 1.0 } else { 0.0 };
        anim.focus[i] += (want - anim.focus[i]) * f;
    }
    anim.spin *= (-1.7 * dt).exp();
    if anim.spin.abs() < 1e-3 {
        anim.spin = 0.0;
    }
    let target = inset_target(pins, highlighted, INSET_ZOOM);
    anim.inset += (target - anim.inset) * (1.0 - (-5.0 * dt).exp());
    anim.pop = (anim.pop - dt * 3.0).max(0.0);
}

/// Start the news playing: a star for each first win, a lock opening for each new level.
fn arrive(
    mut anim: ResMut<Anim>,
    news: Res<Celebration>,
    campaign: Res<Campaign>,
    progress: Res<Progress>,
    state: Res<SelectState>,
    art: Res<MapArt>,
    mut commissioner: ResMut<Commissioner>,
) {
    anim.clock = 0.0;
    anim.typed = 0.0;
    anim.shown = usize::MAX;
    anim.opened_on = state.highlighted;
    anim.swing = None;
    anim.news_line = news.replay_line();
    anim.news_at = None;
    // The fireworks play on every arrival once the last parcel is found (a trinket, not news).
    let all_parcels = campaign
        .levels
        .iter()
        .all(|l| l.present.is_some() && progress.best(&l.meta.id).parcel);
    anim.fireworks = if all_parcels || crate::webenv::var("WOW_AZEROLL_FIREWORKS").is_some() {
        0.0
    } else {
        -1.0
    };
    for i in 0..MAX_LEVELS {
        anim.reveal[i] = 1.0;
        anim.reveal_wait[i] = None;
        anim.ignite[i] = 1.0;
        anim.ignite_wait[i] = None;
        anim.focus[i] = 0.0;
    }
    if state.highlighted < MAX_LEVELS {
        anim.focus[state.highlighted] = 1.0;
    }
    anim.spin = std::f32::consts::TAU * 1.25;
    anim.inset = inset_target(&art.pins, state.highlighted, INSET_ZOOM);
    anim.pop = 1.0;
    for &ix in &news.stars {
        if ix < MAX_LEVELS {
            anim.ignite[ix] = 0.0;
            anim.ignite_wait[ix] = Some(STAR_WAIT);
        }
    }
    for &ix in &news.unlocks {
        if ix < MAX_LEVELS {
            anim.reveal[ix] = 0.0;
            anim.reveal_wait[ix] = Some(UNLOCK_WAIT);
            anim.swing = Some(ix);
        }
    }
    commissioner.cheer +=
        u32::from(!news.stars.is_empty() || !news.unlocks.is_empty() || anim.news_line.is_some());
}

/// What the commissioner's bubble types: the news of a find (D33) while the player has not moved the
/// highlight, else the highlighted level's quip.
fn bubble_line(anim: &Anim, campaign: &Campaign, state: &SelectState) -> &'static str {
    anim.news_line
        .filter(|_| anim.news_at == Some(state.highlighted))
        .unwrap_or_else(|| {
            campaign
                .levels
                .get(state.highlighted)
                .map_or("", |l| l.quip)
        })
}

/// Advance the animation: the globe, the inset, the news, the typewriter.
#[allow(clippy::too_many_arguments)]
fn animate(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    loading: Res<LoadingScreen>,
    campaign: Res<Campaign>,
    progress: Res<Progress>,
    art: Res<MapArt>,
    mut state: ResMut<SelectState>,
    mut anim: ResMut<Anim>,
    mut commissioner: ResMut<Commissioner>,
    mut sounds: MessageWriter<GlueSound>,
) {
    // Nothing plays while the loading screen hides the hub.
    if *phase.get() != Phase::Select || loading.covering() {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let n = LEVELS.len();
    anim.clock += dt;
    if anim.news_line.is_some() && anim.news_at.is_none() {
        anim.news_at = Some(state.highlighted);
    }
    if anim.fireworks >= 0.0 {
        anim.fireworks += dt;
        if anim.fireworks > 4.0 {
            anim.fireworks = -1.0;
        }
    }
    // The player moved the highlight: a pending swing is off.
    if state.highlighted != anim.opened_on {
        anim.swing = None;
    }
    for i in 0..MAX_LEVELS {
        if let Some(wait) = anim.ignite_wait[i] {
            if wait <= dt {
                anim.ignite_wait[i] = None;
                sounds.write(GlueSound("LEVELUP"));
            } else {
                anim.ignite_wait[i] = Some(wait - dt);
            }
        } else if anim.ignite[i] < 1.0 {
            anim.ignite[i] = (anim.ignite[i] + dt / STAR_SECS).min(1.0);
        }
        if let Some(wait) = anim.reveal_wait[i] {
            if wait <= dt {
                anim.reveal_wait[i] = None;
                sounds.write(GlueSound(crate::azeroll::progress::FANFARE_SOUND));
                commissioner.cheer += 1;
                if anim.swing == Some(i) && campaign.unlocked(i, &progress) {
                    state.highlighted = i;
                    anim.opened_on = i;
                }
            } else {
                anim.reveal_wait[i] = Some(wait - dt);
            }
        } else if anim.reveal[i] < 1.0 {
            anim.reveal[i] = (anim.reveal[i] + dt / UNLOCK_SECS).min(1.0);
        }
    }
    ease(&mut anim, state.highlighted, n, &art.pins, dt);
    // The typewriter and the inset's pop restart when the highlight moves to another level.
    if anim.shown != state.highlighted {
        anim.shown = state.highlighted;
        anim.typed = 0.0;
        anim.pop = 1.0;
    }
    anim.typed += dt;
    let len = bubble_line(&anim, &campaign, &state).chars().count();
    anim.line_chars = chars_shown(anim.typed, len);
    commissioner.talking = anim.line_chars < len;
}

// ---------------------------------------------------------------------------------------------
// The widgets
// ---------------------------------------------------------------------------------------------

#[derive(Component)]
struct HubRoot;

/// An absolutely placed piece of the layout.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Scene,
    Title,
    Tag(usize),
    Portrait,
    Bubble,
    Card,
    Inset,
    Help,
}

#[derive(Component, Clone, Copy)]
enum TagText {
    Numeral,
    Title,
    State,
    /// The numeral on the inset's pin.
    Pin,
}

#[derive(Component)]
struct InsetMark;

#[derive(Component)]
struct PinNameText;

#[derive(Component)]
struct TagOf(usize);

#[derive(Component)]
struct StarLabel(usize);

/// A level's numeral over its pin on the inset.
#[derive(Component)]
struct PinLabel(usize);

/// The chosen level's name under its pin.
#[derive(Component)]
struct PinName;

/// The briefing under the quip in the speech bubble.
#[derive(Component)]
struct BubbleSub;

/// One cell of the records grid: two to a row, or four when stacked.
#[derive(Component)]
struct RecCell;

#[derive(Component)]
struct BubbleText;

#[derive(Component)]
struct Nameplate;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Tagline,
    Status,
    Objective,
    ObjectiveSub,
    Limit,
    LimitSub,
    Grade,
    Finds,
    Records,
    RecLabel(usize),
    RecValue(usize),
    ChallengesHeader,
    Challenge(usize),
    Note,
    Help,
    RollText,
    FreeText,
}

/// The two card buttons.
#[derive(Component, Clone, Copy)]
struct CardButton {
    free: bool,
}

#[derive(Component)]
struct ScaleIcon;
#[derive(Component)]
struct CandleIcon;

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

fn abs_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        ..default()
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    models: Res<HudModels>,
    mut mats: ResMut<Assets<ArtMaterial>>,
) {
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;
    let blank = models.blank.clone();
    let n = LEVELS.len();
    let mut art = |kind: f32, p: Vec4, tex: &Handle<Image>| {
        let h = mats.add(ArtMaterial::new(kind, p, tex));
        (Art(h.clone()), MaterialNode(h))
    };
    let plaque = |s: f32| Vec4::new(s, 0.0, 0.0, 0.5);

    commands
        .spawn((
            HubRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(450),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Block,
        ))
        .with_children(|root| {
            // The scene fills the screen.
            root.spawn((
                Slot::Scene,
                SceneMark,
                art(kind::SCENE, Vec4::ZERO, &blank),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            // The title.
            root.spawn((
                Slot::Title,
                Node {
                    flex_direction: FlexDirection::Column,
                    ..abs_node()
                },
            ))
            .with_children(|t| {
                t.spawn(text(f, "AZEROLL", 30.0, GOLD));
            });
            // The stars' names: left of a star on the left, right of one on the right, above the
            // one in the middle.
            for i in 0..n {
                let side = star_q(i, n).x;
                let (justify, align) = if side < -0.3 {
                    (Justify::Left, JustifyContent::FlexStart)
                } else if side > 0.3 {
                    (Justify::Left, JustifyContent::FlexStart)
                } else {
                    (Justify::Center, JustifyContent::Center)
                };
                root.spawn((
                    StarLabel(i),
                    Node {
                        width: Val::Px(240.0),
                        justify_content: align,
                        ..abs_node()
                    },
                    Visibility::Hidden,
                ))
                .with_children(|l| {
                    l.spawn((
                        StarLabelText(i),
                        text(f, "", 15.0, Color::srgb(1.0, 0.93, 0.7)),
                        TextLayout::new_with_justify(justify),
                    ));
                });
            }
            // The Elwynn inset, with each level's numeral over its pin and the chosen one's name.
            root.spawn((
                Slot::Inset,
                InsetMark,
                Button,
                art(kind::INSET, Vec4::new(0.5, 0.5, INSET_ZOOM, 0.0), &blank),
                abs_node(),
            ));
            for i in 0..n {
                root.spawn((
                    PinLabel(i),
                    Node {
                        width: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        ..abs_node()
                    },
                    Visibility::Hidden,
                ))
                .with_children(|l| {
                    l.spawn((
                        TagOf(i),
                        TagText::Pin,
                        text(f, "", 14.0, INK),
                        TextLayout::new_with_justify(Justify::Center),
                    ));
                });
            }
            root.spawn((
                PinName,
                Node {
                    width: Val::Px(240.0),
                    justify_content: JustifyContent::Center,
                    ..abs_node()
                },
                Visibility::Hidden,
            ))
            .with_children(|l| {
                l.spawn((
                    PinNameText,
                    text(f, "", 14.0, Color::srgb(1.0, 0.95, 0.8)),
                    TextLayout::new(Justify::Center, LineBreak::NoWrap),
                ));
            });
            // The tags under the planet.
            for i in 0..n {
                root.spawn((
                    Slot::Tag(i),
                    Button,
                    art(kind::PLAQUE, plaque(style::TAG), &blank),
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                        ..abs_node()
                    },
                ))
                .with_children(|tag| {
                    tag.spawn((TagOf(i), TagText::Numeral, text(f, "", 28.0, GOLD)));
                    tag.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 1.0,
                        flex_basis: Val::Px(0.0),
                        flex_grow: 1.0,
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((TagOf(i), TagText::Title, text(f, "", 14.0, Color::WHITE)));
                        c.spawn((TagOf(i), TagText::State, text(f, "", 12.0, PARCHMENT)));
                    });
                });
            }
            // The commissioner's window, his nameplate, and the line he speaks.
            root.spawn((
                Slot::Portrait,
                PortraitMark,
                art(kind::PORTRAIT, Vec4::ZERO, &models.commissioner),
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexEnd,
                    padding: UiRect::bottom(Val::Px(8.0)),
                    ..abs_node()
                },
            ))
            .with_children(|p| {
                p.spawn((
                    Nameplate,
                    text(
                        f,
                        "Grand Tinker Fizzwick",
                        12.0,
                        Color::srgb(1.0, 0.92, 0.65),
                    ),
                ));
            });
            root.spawn((
                Slot::Bubble,
                art(kind::PLAQUE, plaque(style::BUBBLE), &blank),
                Node {
                    padding: UiRect {
                        left: Val::Px(32.0),
                        right: Val::Px(16.0),
                        top: Val::Px(14.0),
                        bottom: Val::Px(12.0),
                    },
                    align_items: AlignItems::Center,
                    ..abs_node()
                },
            ))
            .with_children(|b| {
                b.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    flex_basis: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        BubbleText,
                        Text::new(""),
                        TextFont {
                            font: font.clone(),
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(INK),
                    ));
                    col.spawn((
                        BubbleSub,
                        Text::new(""),
                        TextFont {
                            font: font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.36, 0.27, 0.16)),
                    ));
                });
            });
            // The mission card.
            root.spawn((
                Slot::Card,
                art(kind::PLAQUE, plaque(style::CARD), &blank),
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(18.0)),
                    row_gap: Val::Px(7.0),
                    ..abs_node()
                },
            ))
            .with_children(|card| {
                card.spawn((Field::Title, text(f, "", 28.0, GOLD)));
                card.spawn((Field::Tagline, text(f, "", 15.0, PARCHMENT)));
                card.spawn((Field::Status, text(f, "", 16.0, GREEN)));
                // The objective: the scale ring and the candle.
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(10.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|row| {
                    for candle in [false, true] {
                        row.spawn(Node {
                            width: Val::Percent(50.0),
                            column_gap: Val::Px(10.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|half| {
                            let node = Node {
                                width: Val::Px(62.0),
                                height: Val::Px(62.0),
                                flex_shrink: 0.0,
                                ..default()
                            };
                            if candle {
                                half.spawn((
                                    CandleIcon,
                                    art(kind::CANDLE, Vec4::new(0.7, 0.0, 0.0, 0.0), &blank),
                                    node,
                                ));
                            } else {
                                half.spawn((
                                    ScaleIcon,
                                    art(kind::SCALE, Vec4::new(0.2, 0.0, 0.0, 0.0), &blank),
                                    node,
                                ));
                            }
                            half.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                flex_shrink: 1.0,
                                ..default()
                            })
                            .with_children(|c| {
                                if candle {
                                    c.spawn((Field::Limit, text(f, "", 22.0, Color::WHITE)));
                                    c.spawn((Field::LimitSub, text(f, "", 12.0, PARCHMENT)));
                                } else {
                                    c.spawn((Field::Objective, text(f, "", 16.0, Color::WHITE)));
                                    c.spawn((Field::ObjectiveSub, text(f, "", 12.0, PARCHMENT)));
                                }
                            });
                        });
                    }
                });
                // The records.
                card.spawn((Field::Records, text(f, "", 14.0, PARCHMENT)));
                card.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    for k in 0..4 {
                        grid.spawn((
                            RecCell,
                            Node {
                                width: Val::Percent(50.0),
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                        ))
                        .with_children(|cell| {
                            cell.spawn((Field::RecLabel(k), text(f, "", 11.0, GOLD)));
                            cell.spawn((Field::RecValue(k), text(f, "", 16.0, Color::WHITE)));
                        });
                    }
                });
                card.spawn((Field::Grade, text(f, "", 15.0, GOLD)));
                card.spawn((Field::Finds, text(f, "", 14.0, GOLD)));
                card.spawn((Field::ChallengesHeader, text(f, "", 11.0, GOLD)));
                for k in 0..MAX_CHALLENGES {
                    card.spawn((Field::Challenge(k), text(f, "", 14.0, PARCHMENT)));
                }
                card.spawn((Field::Note, text(f, "", 14.0, RED)));
                // The buttons, pinned to the card's foot.
                card.spawn(Node {
                    column_gap: Val::Px(10.0),
                    margin: UiRect::top(Val::Auto),
                    ..default()
                })
                .with_children(|row| {
                    for free in [false, true] {
                        row.spawn((
                            CardButton { free },
                            Button,
                            art(kind::PLAQUE, plaque(style::BUTTON), &blank),
                            Node {
                                width: Val::Percent(if free { 54.0 } else { 44.0 }),
                                height: Val::Px(BUTTON_HEIGHT),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|b| {
                            b.spawn((
                                if free {
                                    Field::FreeText
                                } else {
                                    Field::RollText
                                },
                                text(f, "", 15.0, Color::WHITE),
                                TextLayout::new_with_justify(Justify::Center),
                            ));
                        });
                    }
                });
            });
            // The keys, under the tags.
            root.spawn((
                Slot::Help,
                Field::Help,
                text(f, "", 12.0, Color::srgb(0.7, 0.66, 0.55)),
                abs_node(),
            ));
        });
}

/// Place every piece when the window's shape changes (and once at the start).
#[allow(clippy::type_complexity)]
fn apply_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut mats: ResMut<Assets<ArtMaterial>>,
    mut last: Local<f32>,
    mut placed: Query<(&Slot, &mut Node, Option<&Art>), Without<StarLabel>>,
    mut labels: Query<(&StarLabel, &mut Node), Without<Slot>>,
    mut cells: Query<&mut Node, (With<RecCell>, Without<Slot>, Without<StarLabel>)>,
) {
    let Ok(window) = windows.single() else { return };
    if window.height() <= 0.0 {
        return;
    }
    let aspect = super::aspect_of(window);
    if (aspect - *last).abs() < 1e-4 {
        return;
    }
    *last = aspect;
    let n = LEVELS.len();
    let l = layout(aspect, n);
    for (slot, mut node, art) in &mut placed {
        let rect = match slot {
            Slot::Scene => Rect::new(0.0, 0.0, l.width, UI_HEIGHT),
            Slot::Title => Rect::new(l.title.x, l.title.y, 320.0, 60.0),
            Slot::Tag(i) => l.tags[(*i).min(l.tags.len() - 1)],
            Slot::Portrait => l.portrait,
            Slot::Bubble => l.bubble,
            Slot::Card => l.card,
            Slot::Inset => l.inset,
            Slot::Help => {
                let t = l.tags[0];
                Rect::new(t.x, t.y + t.h + 3.0, 420.0, 14.0)
            }
        };
        node.left = Val::Px(rect.x);
        node.top = Val::Px(rect.y);
        if *slot != Slot::Title && *slot != Slot::Help {
            node.width = Val::Px(rect.w);
            node.height = Val::Px(rect.h);
        }
        if *slot == Slot::Scene {
            continue;
        }
        if let Some(Art(h)) = art {
            if let Some(m) = mats.get_mut(h) {
                if *slot == Slot::Inset {
                    // The inset keeps its own geometry past the per-level rows.
                    m.data.lv[GLOBE].x = rect.h;
                    m.data.lv[GLOBE + 2] = Vec4::new(rect.w, rect.h, 0.0, 0.0);
                } else {
                    m.data.lv[0].x = rect.h;
                }
            }
        }
        if *slot == Slot::Bubble {
            if let Some(Art(h)) = art {
                if let Some(m) = mats.get_mut(h) {
                    // The tail aims at the commissioner: level with his head in the wide layout,
                    // low and to the left of the card when stacked.
                    m.data.p.w = if l.compact { 0.84 } else { 0.5 };
                }
            }
        }
        if *slot == Slot::Card {
            node.padding = if l.compact {
                UiRect::axes(Val::Px(16.0), Val::Px(10.0))
            } else {
                UiRect::axes(Val::Px(22.0), Val::Px(15.0))
            };
            node.row_gap = Val::Px(if l.compact { 3.0 } else { 5.0 });
        }
    }
    for mut node in &mut cells {
        node.width = Val::Percent(if l.compact { 25.0 } else { 50.0 });
    }
    for (StarLabel(i), mut node) in &mut labels {
        let q = star_q(*i, n);
        let at = l.q_to_px(q);
        // A left star's name starts at it and runs right, above it; a right star's runs on from it;
        // the middle one's sits above it.
        let (dx, dy) = if q.x < -0.3 {
            (-14.0, -32.0)
        } else if q.x > 0.3 {
            (16.0, -10.0)
        } else {
            (-120.0, -34.0)
        };
        node.left = Val::Px((at.x + dx).clamp(6.0, (l.width - 246.0).max(6.0)));
        node.top = Val::Px((at.y + dy).max(4.0));
    }
}

/// What `refresh` reads.
#[derive(bevy::ecs::system::SystemParam)]
struct HubData<'w, 's> {
    time: Res<'w, Time<Real>>,
    phase: Res<'w, State<Phase>>,
    loading: Res<'w, LoadingScreen>,
    campaign: Res<'w, Campaign>,
    progress: Res<'w, Progress>,
    state: Res<'w, SelectState>,
    anim: Res<'w, Anim>,
    art: Res<'w, MapArt>,
    commissioner: Res<'w, Commissioner>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

/// The widgets `refresh` writes (the texts are a `ParamSet`: they overlap).
#[derive(bevy::ecs::system::SystemParam)]
struct HubWidgets<'w, 's> {
    mats: ResMut<'w, Assets<ArtMaterial>>,
    root: Query<'w, 's, (&'static mut Visibility, &'static mut Node), With<HubRoot>>,
    scene: Query<'w, 's, &'static Art, With<SceneMark>>,
    tags: Query<'w, 's, (&'static Slot, &'static Art, &'static Interaction), Without<CardButton>>,
    buttons: Query<'w, 's, (&'static CardButton, &'static Art, &'static Interaction)>,
    icons: Query<
        'w,
        's,
        (
            &'static Art,
            Option<&'static ScaleIcon>,
            Option<&'static CandleIcon>,
        ),
    >,
    portrait: Query<'w, 's, &'static Art, With<PortraitMark>>,
    labels: Query<'w, 's, (&'static StarLabel, &'static mut Visibility), Without<HubRoot>>,
    inset: Query<'w, 's, &'static Art, With<InsetMark>>,
    pins: Query<
        'w,
        's,
        (
            &'static PinLabel,
            &'static mut Node,
            &'static mut Visibility,
        ),
        (
            Without<StarLabel>,
            Without<HubRoot>,
            Without<Field>,
            Without<PinName>,
        ),
    >,
    pin_name: Query<
        'w,
        's,
        (&'static mut Node, &'static mut Visibility),
        (
            With<PinName>,
            Without<PinLabel>,
            Without<StarLabel>,
            Without<HubRoot>,
            Without<Field>,
        ),
    >,
}

/// Show the hub (once the world is up) and write the views into the widgets.
#[allow(clippy::type_complexity)]
fn refresh(
    data: HubData,
    mut w: HubWidgets,
    mut texts: ParamSet<(
        Query<(&Field, &mut Text, &mut TextColor, &mut Node), Without<HubRoot>>,
        Query<(&TagOf, &TagText, &mut Text, &mut TextColor)>,
        Query<(&mut Text, &StarLabelText)>,
        Query<&mut Text, With<BubbleText>>,
        Query<&mut Text, With<PinNameText>>,
        Query<(&mut Text, &mut TextColor), With<BubbleSub>>,
    )>,
) {
    let HubData {
        time,
        phase,
        loading,
        campaign,
        progress,
        state,
        anim,
        art,
        commissioner,
        windows,
    } = data;
    let mats = &mut w.mats;
    let on = *phase.get() == Phase::Select && !loading.covering();
    for (mut vis, mut node) in &mut w.root {
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
    if !on {
        return;
    }
    let t = time.elapsed_secs_wrapped();
    let n = LEVELS.len();
    let views = view(&campaign, &progress, &state);
    let hi = state.highlighted.min(n - 1);
    let cur = &views[hi];
    let aspect = windows.single().map_or(1.5, super::aspect_of);
    let l = layout(aspect, n);

    // --- The scene: the planet, turned to Goldshire, and the sky's stars. ---
    let ready = art.ready();
    let lon = art.aim.x - AIM_OFFSET + anim.spin + 0.07 * (t * 0.31).sin();
    let lat = art.aim.y * 0.6 + 0.03 * (t * 0.43).sin();
    let globe_row = Vec4::new(lon, lat, art.aim.x, art.aim.y);
    for Art(h) in &w.scene {
        let Some(m) = mats.get_mut(h) else { continue };
        if ready && m.tex != *art.globe.as_ref().unwrap() {
            m.tex = art.globe.clone().unwrap();
        }
        m.data.kt.y = t;
        m.data.kt.w = n.min(MAX_LEVELS) as f32;
        m.data.p = Vec4::new(l.centre.x, l.centre.y, l.u, if ready { 1.0 } else { 0.0 });
        m.data.lv[GLOBE] = globe_row;
        m.data.lv[GLOBE + 3] = Vec4::new(anim.fireworks, 0.0, 0.0, 0.0);
        // The beam to the inset: its left edge, top and bottom, as shares of the screen.
        m.data.lv[GLOBE + 1] = Vec4::new(
            l.inset.x / l.width,
            l.inset.y / UI_HEIGHT,
            (l.inset.y + l.inset.h) / UI_HEIGHT,
            if ready { 1.0 } else { 0.0 },
        );
        for (i, v) in views.iter().enumerate().take(MAX_LEVELS) {
            let star = star_q(i, n);
            m.data.lv[i * 3] =
                Vec4::new(level_state(v, &anim, i), 0.0, anim.focus[i], anim.reveal[i]);
            m.data.lv[i * 3 + 1] = Vec4::new(
                0.0,
                anim.ignite[i],
                f32::from(u8::from(v.comet)),
                v.grade.map_or(0.0, |g| g as u8 as f32 + 1.0),
            );
            m.data.lv[i * 3 + 2] =
                Vec4::new(star.x, star.y, 0.0, f32::from(u8::from(v.highlighted)));
        }
    }

    // --- The inset: the Elwynn map and the pins. ---
    let places = pin_places(&l, &art.pins, anim.inset);
    for Art(h) in &w.inset {
        let Some(m) = mats.get_mut(h) else { continue };
        if ready && m.tex != *art.elwynn.as_ref().unwrap() {
            m.tex = art.elwynn.clone().unwrap();
        }
        m.data.kt.y = t;
        m.data.kt.w = if ready { n.min(MAX_LEVELS) as f32 } else { 0.0 };
        m.data.p = Vec4::new(anim.inset.x, anim.inset.y, INSET_ZOOM, anim.pop);
        for (i, v) in views.iter().enumerate().take(MAX_LEVELS) {
            // How far the pin is drawn from its map point (a leader line joins them).
            let off = places.get(i).map_or(Vec2::ZERO, |(at, shown)| *shown - *at);
            let pin = art.pins.get(i).copied().unwrap_or(Vec2::splat(0.5));
            m.data.lv[i * 3] =
                Vec4::new(level_state(v, &anim, i), 0.0, anim.focus[i], anim.reveal[i]);
            m.data.lv[i * 3 + 1] = Vec4::new(
                off.x,
                off.y,
                f32::from(u8::from(v.comet)),
                v.grade.map_or(0.0, |g| g as u8 as f32 + 1.0),
            );
            m.data.lv[i * 3 + 2] = Vec4::new(pin.x, pin.y, 0.0, f32::from(u8::from(v.highlighted)));
        }
    }
    // The numerals over the pins and the chosen one's name under its.
    for (PinLabel(i), mut node, mut vis) in &mut w.pins {
        let shown = ready && views[(*i).min(n - 1)].standing != Standing::Locked;
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if let Some((_, tip)) = places.get(*i) {
            let scale = 1.0 + 0.3 * anim.focus[*i];
            node.left = Val::Px(tip.x - 15.0);
            node.top = Val::Px(tip.y - 15.0 * scale - 9.0);
        }
    }
    for (mut node, mut vis) in &mut w.pin_name {
        let shown = ready && art.pins.get(hi).is_some();
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if places.get(hi).is_some() {
            let shown: Vec<Vec2> = places.iter().map(|(_, s)| *s).collect();
            let (at, width) = name_slot(&shown, hi, &l.inset, cur.title.chars().count());
            node.left = Val::Px(at.x);
            node.top = Val::Px(at.y);
            node.width = Val::Px(width);
        }
    }
    for mut text in &mut texts.p4() {
        let s = cur.title.clone();
        if text.0 != s {
            text.0 = s;
        }
    }

    // --- The stars' names. ---
    for (mut text, StarLabelText(i)) in &mut texts.p2() {
        let v = &views[(*i).min(n - 1)];
        let s = match (&v.star, anim.ignite[*i] >= 1.0) {
            (Some(star), true) => star.clone(),
            _ => String::new(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for (StarLabel(i), mut vis) in &mut w.labels {
        let want = if views[(*i).min(n - 1)].star.is_some() && anim.ignite[*i] >= 1.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }

    // --- The tags. ---
    for (slot, Art(h), interaction) in &w.tags {
        let Slot::Tag(i) = slot else { continue };
        let Some(m) = mats.get_mut(h) else { continue };
        let v = &views[(*i).min(n - 1)];
        let hot = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let lit = if v.highlighted {
            1.0
        } else if hot {
            0.45
        } else {
            0.0
        };
        m.data.kt.y = t;
        m.data.p = Vec4::new(style::TAG, lit, if v.locked() { 0.7 } else { 0.0 }, 0.5);
    }
    for (TagOf(i), which, mut text, mut color) in &mut texts.p1() {
        let v = &views[(*i).min(n - 1)];
        let (s, c) = match which {
            TagText::Numeral => (
                v.numeral.clone(),
                if v.locked() {
                    Color::srgb(0.55, 0.5, 0.42)
                } else {
                    GOLD
                },
            ),
            TagText::Title => (
                v.title.clone(),
                if v.locked() {
                    Color::srgb(0.6, 0.57, 0.5)
                } else {
                    Color::WHITE
                },
            ),
            TagText::Pin => (
                if v.locked() {
                    String::new()
                } else {
                    v.numeral.clone()
                },
                INK,
            ),
            TagText::State => match v.standing {
                Standing::Locked => ("Locked".to_string(), RED),
                Standing::Open => ("New request".to_string(), GOLD),
                Standing::Won => (
                    v.grade.map_or("Fulfilled".to_string(), |g| {
                        format!("Fulfilled: {}", g.name())
                    }),
                    GREEN,
                ),
            },
        };
        if text.0 != s {
            text.0 = s;
        }
        if color.0 != c {
            color.0 = c;
        }
    }

    // --- The card's icons. ---
    for (Art(h), scale, candle) in &w.icons {
        let Some(m) = mats.get_mut(h) else { continue };
        m.data.kt.y = t;
        if scale.is_some() {
            // The start ball's share of the goal ring, log-compressed so a 20x climb still shows.
            let ratio = (cur.goal_diameter / cur.start_diameter.max(1e-3)).max(1.0);
            m.data.p.x = (0.62 - 0.14 * ratio.ln()).clamp(0.12, 0.6);
        }
        if candle.is_some() {
            m.data.p.x = (cur.time_limit / 600.0).clamp(0.35, 1.0);
        }
    }
    for Art(h) in &w.portrait {
        if let Some(m) = mats.get_mut(h) {
            m.data.kt.y = t;
            m.data.p.y = if commissioner.talking { 1.0 } else { 0.0 };
        }
    }

    // --- The bubble: the commissioner's quip, typed out, and the briefing under it. ---
    let quip = bubble_line(&anim, &campaign, &state);
    for mut text in &mut texts.p3() {
        let shown: String = quip.chars().take(anim.line_chars).collect();
        if text.0 != shown {
            text.0 = shown;
        }
    }
    let typed_secs =
        quip.chars().count() as f32 / MIN_CPS.max(quip.chars().count() as f32 / LINE_SECS);
    let sub_alpha = ((anim.typed - typed_secs) / 0.25).clamp(0.0, 1.0);
    for (mut text, mut color) in &mut texts.p5() {
        if text.0 != cur.briefing {
            text.0 = cur.briefing.clone();
        }
        let c = Color::srgb(0.36, 0.27, 0.16).with_alpha(sub_alpha);
        if color.0 != c {
            color.0 = c;
        }
    }

    // --- The card. ---
    let dim = if cur.locked() { 0.55 } else { 1.0 };
    for (field, mut text, mut color, mut node) in &mut texts.p0() {
        let (s, c) = card_field(*field, cur, &state, l.compact);
        // An empty line takes no room; the compact card also drops what only the roomy one has.
        let drop = s.is_empty()
            || (l.compact
                && matches!(
                    field,
                    Field::Tagline | Field::Help | Field::ChallengesHeader | Field::Challenge(_)
                ));
        let display = if drop { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
        if text.0 != s {
            text.0 = s;
        }
        let c = c.with_alpha(match field {
            Field::Status | Field::Note | Field::Help | Field::RollText | Field::FreeText => 1.0,
            _ => dim,
        });
        let c = match (field, cur.locked()) {
            (Field::RollText, true) | (Field::FreeText, true) => c.with_alpha(0.45),
            (Field::FreeText, false) if !cur.free_roll => c.with_alpha(0.55),
            _ => c,
        };
        if color.0 != c {
            color.0 = c;
        }
    }
    for (button, Art(h), interaction) in &w.buttons {
        let Some(m) = mats.get_mut(h) else { continue };
        let hot = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let disabled = cur.locked() || (button.free && !cur.free_roll);
        m.data.kt.y = t;
        m.data.lv[0].x = BUTTON_HEIGHT;
        m.data.p = Vec4::new(
            style::BUTTON,
            if hot && !disabled {
                1.0
            } else if disabled {
                0.0
            } else {
                0.25
            },
            if disabled { 0.8 } else { 0.0 },
            0.5,
        );
    }
}

#[derive(Component)]
struct StarLabelText(usize);
#[derive(Component)]
struct SceneMark;
#[derive(Component)]
struct PortraitMark;

/// What the shader is told of a level's standing: 0 shut, 1 open, 2 won. A level whose lock has
/// not yet been opened on screen still looks shut, whatever the saves say.
fn level_state(v: &CardView, anim: &Anim, i: usize) -> f32 {
    match v.standing {
        Standing::Open | Standing::Won if anim.reveal[i] < 1.0 && anim.reveal_wait[i].is_some() => {
            0.0
        }
        Standing::Locked => 0.0,
        Standing::Open => 1.0,
        Standing::Won => 2.0,
    }
}

/// The text and colour of one card field.
/// The card's finds line (D33): a `[X]` (the WoW font has no check glyph; challenges use it too) and the word for what has been found, a dim dot and no
/// word for what has not; empty when the level hides nothing or nothing is open yet. The second
/// value is whether everything the level hides has been found.
pub(super) fn finds_line(v: &CardView) -> (String, bool) {
    if v.locked() || (v.parcel.is_none() && v.cousin.is_none()) {
        return (String::new(), false);
    }
    let part = |found: Option<bool>, word: &str| match found {
        Some(true) => Some(format!("{word} [X]")),
        Some(false) => Some("\u{b7}".to_string()),
        None => None,
    };
    let line = [part(v.parcel, "Parcel"), part(v.cousin, "Cousin")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("   ");
    let all = v.parcel.is_none_or(|p| p) && v.cousin.is_none_or(|c| c);
    (line, all)
}

fn card_field(field: Field, v: &CardView, state: &SelectState, compact: bool) -> (String, Color) {
    match field {
        Field::Title => (v.title.clone(), GOLD),
        Field::Tagline => (v.tagline.clone(), PARCHMENT),
        Field::Status => (
            v.status.clone(),
            match v.standing {
                Standing::Locked => RED,
                Standing::Open => GOLD,
                Standing::Won => GREEN,
            },
        ),
        Field::Objective => (v.objective.clone(), Color::WHITE),
        Field::ObjectiveSub => (v.scale_text.clone(), PARCHMENT),
        Field::Limit => (v.limit_text.clone(), Color::WHITE),
        Field::LimitSub => ("on the candle".to_string(), PARCHMENT),
        Field::Grade => {
            let met = v.challenges.iter().filter(|c| c.1).count();
            let mut s = match (v.grade, v.comet) {
                (Some(g), true) => format!("Best verdict: {}   +   comet for speed", g.name()),
                (Some(g), false) => format!("Best verdict: {}", g.name()),
                (None, _) => String::new(),
            };
            if compact && !v.challenges.is_empty() && !v.locked() {
                if !s.is_empty() {
                    s.push_str("   \u{b7}   ");
                }
                s.push_str(&format!("Challenges {}/{}", met, v.challenges.len()));
            }
            (s, GOLD)
        }
        Field::Finds => {
            let (line, all) = finds_line(v);
            (line, if all { GOLD } else { PARCHMENT })
        }
        Field::Records => (
            if v.records.is_empty() && !v.locked() {
                "Nobody has rolled this one yet.".to_string()
            } else if v.locked() {
                "Records unlock with the request.".to_string()
            } else {
                String::new()
            },
            PARCHMENT,
        ),
        Field::RecLabel(k) => (
            v.records
                .get(k)
                .map(|r| r.0.to_uppercase())
                .unwrap_or_default(),
            GOLD,
        ),
        Field::RecValue(k) => (
            v.records.get(k).map(|r| r.1.clone()).unwrap_or_default(),
            Color::WHITE,
        ),
        Field::ChallengesHeader => {
            let met = v.challenges.iter().filter(|c| c.1).count();
            (
                if v.challenges.is_empty() {
                    String::new()
                } else {
                    format!("OPTIONAL CHALLENGES  {}/{}", met, v.challenges.len())
                },
                GOLD,
            )
        }
        Field::Challenge(k) => match v.challenges.get(k) {
            Some((title, true)) => (format!("[X] {title}"), GREEN),
            Some((title, false)) => (format!("[ ] {title}"), PARCHMENT),
            None => (String::new(), PARCHMENT),
        },
        Field::Note => (
            state
                .note
                .as_ref()
                .map_or(String::new(), |(s, _)| s.clone()),
            RED,
        ),
        Field::Help => (
            if compact {
                "Arrows choose  \u{b7}  Enter rolls  \u{b7}  F Free Roll".to_string()
            } else {
                "Arrows or A/D choose  \u{b7}  Enter rolls  \u{b7}  F is Free Roll".to_string()
            },
            Color::srgb(0.7, 0.66, 0.55),
        ),
        Field::RollText => (
            if v.won() {
                "Roll again!   (Enter)"
            } else {
                "Roll!   (Enter)"
            }
            .to_string(),
            Color::WHITE,
        ),
        Field::FreeText => (
            if v.free_roll {
                "Free Roll: no candle   (F)".to_string()
            } else {
                "Free Roll: win it first".to_string()
            },
            Color::WHITE,
        ),
    }
}

/// Pointer: hovering a tag swings it to the front; clicking the front one rolls it; the card's
/// buttons roll and free-roll.
#[allow(clippy::type_complexity)]
fn pointer(
    phase: Res<State<Phase>>,
    mut state: ResMut<SelectState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    anim: Res<Anim>,
    art: Res<MapArt>,
    mut last_cursor: Local<Vec2>,
    tags: Query<(&Slot, &Interaction), (Changed<Interaction>, Without<CardButton>)>,
    buttons: Query<(&CardButton, &Interaction), Changed<Interaction>>,
    inset: Query<&Interaction, (With<InsetMark>, Without<CardButton>)>,
) {
    if *phase.get() != Phase::Select {
        return;
    }
    for (slot, interaction) in &tags {
        let Slot::Tag(i) = slot else { continue };
        match interaction {
            Interaction::Hovered if state.highlighted != *i => {
                state.request = Some(Request::Highlight(*i))
            }
            Interaction::Pressed => {
                state.request = Some(if state.highlighted == *i {
                    Request::Play { free: false }
                } else {
                    Request::Highlight(*i)
                });
            }
            _ => {}
        }
    }
    for (button, interaction) in &buttons {
        if *interaction == Interaction::Pressed {
            state.request = Some(Request::Play { free: button.free });
        }
    }
    // The inset: a pin under the cursor is chosen by hovering it (only when the mouse moves, so
    // the keyboard is not fought) and rolled by a second click.
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let moved = cursor != *last_cursor;
    *last_cursor = cursor;
    let l = layout(super::aspect_of(window), LEVELS.len());
    let ui = cursor / ui_scale.0.max(1e-3);
    let nearest = pin_places(&l, &art.pins, anim.inset)
        .into_iter()
        .enumerate()
        .map(|(i, (_, tip))| {
            let head = tip - Vec2::new(0.0, 15.0);
            (i, head.distance(ui).min(tip.distance(ui)))
        })
        .filter(|(_, d)| *d < 26.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    for interaction in &inset {
        match (interaction, nearest) {
            (Interaction::Pressed, Some(i)) => {
                state.request = Some(if state.highlighted == i {
                    Request::Play { free: false }
                } else {
                    Request::Highlight(i)
                });
            }
            (Interaction::Hovered, Some(i)) if moved && state.highlighted != i => {
                state.request = Some(Request::Highlight(i));
            }
            _ => {}
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    embedded_asset!(app, "art.wgsl");
    app.add_plugins(UiMaterialPlugin::<ArtMaterial>::default())
        .init_resource::<Anim>()
        .init_resource::<MapArt>()
        .add_systems(Update, super::mapart::load.run_if(in_state(Phase::Select)))
        .add_systems(Startup, build.after(hud_models::setup))
        .add_systems(OnEnter(Phase::Select), arrive.after(super::remember))
        .add_systems(Update, (apply_layout, animate, pointer, refresh).chain());
}

/// Whether a grade has art in the hub (all do); kept so a new grade is a compile error here.
#[allow(dead_code)]
fn _grades_covered(g: Grade) -> u8 {
    match g {
        Grade::Scraped | Grade::Respectable | Grade::Splendid | Grade::Preposterous => g as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_finds_line_says_only_what_has_been_found() {
        let mut v = CardView {
            numeral: "I".into(),
            title: String::new(),
            tagline: String::new(),
            standing: Standing::Open,
            status: String::new(),
            objective: String::new(),
            briefing: String::new(),
            start_diameter: 0.5,
            goal_diameter: 2.0,
            scale_text: String::new(),
            time_limit: 300.0,
            limit_text: String::new(),
            records: Vec::new(),
            grade: None,
            comet: false,
            star: None,
            challenges: Vec::new(),
            free_roll: false,
            parcel: Some(true),
            cousin: Some(false),
            highlighted: true,
        };
        assert_eq!(finds_line(&v), ("Parcel [X]   \u{b7}".to_string(), false));
        v.cousin = Some(true);
        assert_eq!(
            finds_line(&v),
            ("Parcel [X]   Cousin [X]".to_string(), true)
        );
        v.parcel = None;
        v.cousin = None;
        assert_eq!(finds_line(&v).0, "");
        v.parcel = Some(true);
        v.standing = Standing::Locked;
        assert_eq!(finds_line(&v).0, "", "a locked level says nothing");
    }

    #[test]
    fn a_line_types_out_within_the_time_a_vignette_allows() {
        assert_eq!(chars_shown(0.0, 100), 0);
        assert_eq!(chars_shown(10.0, 100), 100);
        // A long line finishes in LINE_SECS, not at the slow rate.
        let long = 400;
        assert_eq!(chars_shown(LINE_SECS, long), long);
        assert!(chars_shown(LINE_SECS * 0.5, long) < long);
        // A short one is not rushed past the minimum rate.
        assert_eq!(chars_shown(0.5, 1000), (0.5 * 1000.0 / LINE_SECS) as usize);
        assert_eq!(chars_shown(0.25, 40), 12);
    }

    #[test]
    fn the_globe_spins_in_and_settles_and_the_inset_follows_the_choice() {
        let pins = [
            Vec2::new(0.43, 0.66),
            Vec2::new(0.38, 0.73),
            Vec2::new(0.19, 0.25),
        ];
        let mut anim = Anim::default();
        anim.spin = std::f32::consts::TAU * 1.25;
        anim.focus[0] = 1.0;
        anim.inset = inset_target(&pins, 0, INSET_ZOOM);
        let start = anim.inset;
        for _ in 0..60 {
            ease(&mut anim, 2, 3, &pins, 1.0 / 60.0);
        }
        // One second in: the spin is well down, and focus has moved to the new choice.
        assert!(anim.spin < std::f32::consts::TAU * 0.5, "{}", anim.spin);
        assert!(anim.focus[2] > 0.95 && anim.focus[0] < 0.05);
        // The inset is moving toward Stormwind (north-west), not there yet.
        assert!(anim.inset.x < start.x && anim.inset.y < start.y);
        for _ in 0..600 {
            ease(&mut anim, 2, 3, &pins, 1.0 / 60.0);
        }
        assert_eq!(anim.spin, 0.0);
        assert!((anim.inset - inset_target(&pins, 2, INSET_ZOOM)).length() < 1e-3);
    }

    /// Whether two pins drawn at `a` and `b` have their heads clear of each other.
    fn apart(a: Vec2, b: Vec2) -> bool {
        (a.x - b.x).abs() >= PIN_SEP.x - 0.01 || (a.y - b.y).abs() >= PIN_SEP.y - 0.01
    }

    #[test]
    fn pins_that_stand_close_are_fanned_apart_and_far_ones_stay() {
        // Goldshire's two levels land within a pin's width of each other; Stormwind is far off.
        let at = [
            Vec2::new(100.0, 100.0),
            Vec2::new(122.0, 121.0),
            Vec2::new(-80.0, -150.0),
        ];
        let shown = fan_out(&at);
        for i in 0..3 {
            for j in i + 1..3 {
                assert!(apart(shown[i], shown[j]), "{i} {j}: {shown:?}");
            }
        }
        assert_eq!(shown[2], at[2]);
        // Two on the same spot still part, deterministically.
        let same = fan_out(&[Vec2::splat(5.0), Vec2::splat(5.0)]);
        assert!(apart(same[0], same[1]));
        assert_eq!(same, fan_out(&[Vec2::splat(5.0), Vec2::splat(5.0)]));
        // Pins already apart do not move.
        let clear = [Vec2::ZERO, Vec2::new(40.0, 3.0)];
        assert_eq!(fan_out(&clear), clear);
    }

    #[test]
    fn fanning_does_not_shift_as_the_inset_eases() {
        let pins = [
            Vec2::new(0.43, 0.66),
            Vec2::new(0.38, 0.73),
            Vec2::new(0.19, 0.25),
        ];
        for aspect in [1.0_f32, 1.78] {
            let l = layout(aspect, 3);
            let a = pin_places(&l, &pins, Vec2::new(0.4, 0.6));
            let b = pin_places(&l, &pins, Vec2::new(0.31, 0.52));
            for ((at_a, shown_a), (at_b, shown_b)) in a.iter().zip(&b) {
                assert!(((*shown_a - *at_a) - (*shown_b - *at_b)).length() < 1e-3);
            }
        }
    }

    #[test]
    fn every_pin_and_its_head_are_inside_the_inset_whichever_level_is_chosen() {
        // The real pins (Lion's Pride, Market Day, Stormwind) on the Elwynn sheet.
        let pins = [
            Vec2::new(0.432, 0.660),
            Vec2::new(0.382, 0.726),
            Vec2::new(0.189, 0.251),
        ];
        for aspect in [1.0_f32, 1.4, 1.78] {
            let l = layout(aspect, 3);
            for chosen in 0..3 {
                let c = inset_target(&pins, chosen, INSET_ZOOM);
                for (_, tip) in pin_places(&l, &pins, c) {
                    // The head and badge stand about 24 units over the tip (31 when chosen).
                    assert!(
                        tip.y - 31.0 >= l.inset.y,
                        "{aspect} {chosen}: head cut off at {tip:?}"
                    );
                    assert!(
                        tip.y <= l.inset.y + l.inset.h - 3.0,
                        "{aspect} {chosen}: {tip:?}"
                    );
                    assert!(tip.x - 16.0 >= l.inset.x && tip.x + 16.0 <= l.inset.x + l.inset.w);
                }
            }
        }
    }

    #[test]
    fn no_two_level_pins_overlap_in_any_layout_or_choice() {
        let pins = [
            Vec2::new(0.43, 0.66),
            Vec2::new(0.38, 0.73),
            Vec2::new(0.19, 0.25),
        ];
        for aspect in [1.0_f32, 1.2, 1.4, 1.78, 2.4] {
            let l = layout(aspect, 3);
            for chosen in 0..3 {
                let c = inset_target(&pins, chosen, INSET_ZOOM);
                let shown: Vec<Vec2> = pin_places(&l, &pins, c)
                    .into_iter()
                    .map(|(_, s)| s)
                    .collect();
                for i in 0..3 {
                    for j in i + 1..3 {
                        assert!(apart(shown[i], shown[j]), "{aspect} {chosen}: {shown:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_chosen_pins_name_stays_off_the_other_pins_and_inside_the_inset() {
        let pins = [
            Vec2::new(0.43, 0.66),
            Vec2::new(0.38, 0.73),
            Vec2::new(0.19, 0.25),
        ];
        let title = "Last Call at the Lion's Pride";
        for aspect in [1.0_f32, 1.78] {
            let l = layout(aspect, 3);
            for chosen in 0..3 {
                let c = inset_target(&pins, chosen, INSET_ZOOM);
                let shown: Vec<Vec2> = pin_places(&l, &pins, c)
                    .into_iter()
                    .map(|(_, s)| s)
                    .collect();
                let (at, w) = name_slot(&shown, chosen, &l.inset, title.chars().count());
                assert!(
                    at.x >= l.inset.x && at.x + w <= l.inset.x + l.inset.w + 0.01,
                    "{at:?} {w}"
                );
                assert!(
                    at.y >= l.inset.y && at.y + NAME_H <= l.inset.y + l.inset.h + 0.01,
                    "{at:?}"
                );
                for (i, o) in shown.iter().enumerate().filter(|(i, _)| *i != chosen) {
                    let hit = at.x < o.x + PIN_HALF_W
                        && at.x + w > o.x - PIN_HALF_W
                        && at.y < o.y + PIN_BELOW
                        && at.y + NAME_H > o.y - PIN_ABOVE;
                    assert!(!hit, "aspect {aspect} chosen {chosen}: the name at {at:?} covers pin {i} at {o:?}");
                }
            }
        }
    }

    #[test]
    fn the_inset_window_always_stays_on_the_map_and_holds_every_pin() {
        let pins = [
            Vec2::new(0.43, 0.66),
            Vec2::new(0.38, 0.73),
            Vec2::new(0.19, 0.25),
        ];
        let half = 0.5 / INSET_ZOOM;
        for chosen in 0..3 {
            let c = inset_target(&pins, chosen, INSET_ZOOM);
            assert!(c.x >= half && c.x <= 1.0 - half && c.y >= half && c.y <= 1.0 - half);
            // The chosen pin is in view.
            let p = pins[chosen];
            assert!(
                (p.x - c.x).abs() <= half && (p.y - c.y).abs() <= half,
                "{chosen}: {c:?}"
            );
        }
        assert_eq!(inset_target(&[], 0, 1.5), Vec2::splat(0.5));
    }

    #[test]
    fn the_layout_keeps_every_piece_on_screen_and_apart() {
        for aspect in [1.0_f32, 1.2, 1.39, 1.4, 1.6, 1.78, 2.1, 2.4] {
            let l = layout(aspect, 3);
            let screen = Rect::new(0.0, 0.0, l.width, UI_HEIGHT);
            let mut pieces = vec![
                ("portrait", l.portrait),
                ("bubble", l.bubble),
                ("card", l.card),
                ("inset", l.inset),
            ];
            for t in &l.tags {
                pieces.push(("tag", *t));
            }
            for (name, r) in &pieces {
                assert!(
                    r.x >= -0.5
                        && r.y >= -0.5
                        && r.right() <= screen.right() + 0.5
                        && r.bottom() <= UI_HEIGHT + 0.5,
                    "{name} off screen at aspect {aspect}: {r:?}"
                );
                assert!(
                    r.w > 40.0 && r.h > 30.0,
                    "{name} too small at {aspect}: {r:?}"
                );
            }
            for (i, (an, a)) in pieces.iter().enumerate() {
                for (bn, b) in &pieces[i + 1..] {
                    assert!(
                        !a.overlaps(b),
                        "{an} overlaps {bn} at aspect {aspect}: {a:?} {b:?}"
                    );
                }
            }
            // The planet's disc is clear of the inset, the card and the tags.
            let r_px = l.u * UI_HEIGHT;
            let c = l.q_to_px(Vec2::ZERO);
            assert!(
                c.x + r_px <= l.inset.x + 1.0,
                "planet runs into the inset at {aspect}"
            );
            assert!(c.x - r_px >= 0.0, "planet off the left edge at {aspect}");
            assert!(
                c.y + r_px <= l.tags[0].y + 2.0,
                "planet under the tags at {aspect}"
            );
            // The stars are on screen.
            for i in 0..3 {
                let p = l.q_to_px(star_q(i, 3));
                assert!(
                    p.x > 0.0 && p.x < l.width && p.y > 6.0,
                    "star {i} at {aspect}: {p:?}"
                );
            }
        }
    }

    #[test]
    fn stars_arc_over_the_planet_and_stay_apart() {
        let pts: Vec<Vec2> = (0..3).map(|i| star_q(i, 3)).collect();
        assert!(pts.windows(2).all(|w| w[1].x - w[0].x > 0.5));
        // All above the planet's equator, outside its disc.
        assert!(pts.iter().all(|p| p.y < -0.5 && p.length() > 1.2));
        // A single level still gets a star, straight above.
        let one = star_q(0, 1);
        assert!(one.x.abs() < 1e-5 && one.y < -1.0);
    }

    #[test]
    fn the_map_projection_puts_the_inset_pins_inside_the_inset() {
        let l = layout(1.78, 3);
        let pins = [
            Vec2::new(0.43, 0.66),
            Vec2::new(0.38, 0.73),
            Vec2::new(0.19, 0.25),
        ];
        for chosen in 0..3 {
            let c = inset_target(&pins, chosen, INSET_ZOOM);
            for p in pins {
                let px = l.map_to_px(p, c, INSET_ZOOM);
                assert!(
                    px.x > l.inset.x
                        && px.x < l.inset.x + l.inset.w
                        && px.y > l.inset.y
                        && px.y < l.inset.y + l.inset.h,
                    "{p:?} -> {px:?} outside {:?} with the choice {chosen}",
                    l.inset
                );
            }
        }
        // The centre of the window is the centre of the inset.
        let c = Vec2::new(0.4, 0.5);
        let mid = l.map_to_px(c, c, INSET_ZOOM);
        assert!((mid.x - (l.inset.x + l.inset.w * 0.5)).abs() < 1e-3);
    }
}
