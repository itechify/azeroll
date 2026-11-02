//! Embedded level recipe; native overrides are read atomically on R, never during a run.
use super::{
    level::{Behavior, Look, Place, Species},
    *,
};
use benilla_world::{collision::WorldCollision, world_point::WorldPoint};
use bevy::prelude::*;
use serde::Deserialize;

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Design {
    pub start: [f32; 3],
    pub start_radius: f32,
    pub time_limit: f32,
    pub goal: String,
    pub camp: [f32; 3],
    pub patrol: [f32; 3],
    pub patrol_at: f32,
    pub patrol_until: f32,
    pub areas: Vec<Area>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub trails: Vec<Trail>,
    #[serde(default)]
    pub herds: Vec<Herd>,
    #[serde(skip)]
    pub roster: Vec<Species>,
    /// FNV-1a of the recipe text this was parsed from, so a report names the level it measured.
    #[serde(skip)]
    pub recipe_hash: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Area {
    pub name: String,
    pub centre: [f32; 3],
    pub radius: f32,
    #[serde(default)]
    pub enclosed: bool,
    #[serde(default)]
    pub clearance: f32,
    #[serde(default)]
    pub doors: Vec<[f32; 3]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scene {
    pub name: String,
    pub anchor: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub indoors: bool,
    pub items: Vec<Item>,
}
/// One group of props in a scene. Items are the unit of composition: a scene is a few
/// groups that tell a story, each a different arrangement and usually a mix of models.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Item {
    /// One model, or `models` for a mix (a creature item uses `species` instead).
    #[serde(default)]
    pub model: String,
    /// Mixed models for the group, chosen per prop by `pick`; repeat an entry to weight it.
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub pick: Pick,
    #[serde(default)]
    pub species: String,
    pub count: usize,
    pub arrangement: Arrangement,
    /// Arrangement size, yards: step for `line`/`grid`/`spill`, radius for `ring`, `cluster`,
    /// `heap` and `table-set`, density for `pile`/`scatter`.
    #[serde(default = "one")]
    pub spacing: f32,
    #[serde(default = "one")]
    pub scale: f32,
    /// Random scale, plus or minus this fraction.
    #[serde(default)]
    pub jitter: f32,
    #[serde(default)]
    pub solid: bool,
    /// Where the group sits relative to the scene anchor (east, south), yards.
    #[serde(default)]
    pub offset: [f32; 2],
    /// Height above the ground of the group's base, yards.
    #[serde(default)]
    pub lift: f32,
    /// A fixed yaw for every prop; omitted, each prop gets its own.
    #[serde(default)]
    pub yaw: Option<f32>,
    /// Random lean of up to this many radians, for small things that were dropped.
    #[serde(default)]
    pub tilt: f32,
    /// Fraction of props lying on their side, and the height a toppled prop is raised by.
    #[serde(default)]
    pub topple: f32,
    #[serde(default)]
    pub topple_lift: f32,
    /// Sideways noise on a `line`, `grid` or `ring`, yards.
    #[serde(default)]
    pub wobble: f32,
    /// Closest two props of a `cluster`, `heap` or `table-set` may sit, yards.
    #[serde(default)]
    pub gap: f32,
    /// Height of each `heap` layer, yards.
    #[serde(default)]
    pub step: f32,
    /// Direction of a `spill` or a table row: radians, 0 east, pi/2 south.
    #[serde(default)]
    pub heading: f32,
    /// How much a `spill` may turn per prop, radians.
    #[serde(default)]
    pub bend: f32,
    /// A `table-set` puts its props on this model's top, `surface` yards up.
    #[serde(default)]
    pub table: String,
    #[serde(default = "one_count")]
    pub tables: usize,
    #[serde(default = "one")]
    pub table_gap: f32,
    #[serde(default = "one")]
    pub table_scale: f32,
    #[serde(default)]
    pub surface: f32,
}
fn one() -> f32 {
    1.0
}
fn one_count() -> usize {
    1
}
#[derive(Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Pick {
    #[default]
    Random,
    Cycle,
}
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Arrangement {
    /// A sunflower spiral, even density.
    Pile,
    Ring,
    Line,
    Grid,
    /// Props on a table or bench model (or the floor when none is given).
    TableSet,
    /// Even density with random positions.
    Scatter,
    /// A dense centre with a falloff and a few stragglers.
    Cluster,
    /// Props resting on props, in layers.
    Heap,
    /// A wandering trail of uneven steps, for things that were dropped on the way.
    Spill,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Trail {
    pub name: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub spacing: f32,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub pick: Pick,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default = "trail_jitter")]
    pub jitter: f32,
    /// How far the trail wanders off the straight line, yards.
    #[serde(default)]
    pub wobble: f32,
    /// Plus or minus this fraction of `spacing` between props.
    #[serde(default = "trail_variance")]
    pub variance: f32,
    /// Chance a drop is a pair of props rather than one.
    #[serde(default)]
    pub clump: f32,
    #[serde(default = "trail_tilt")]
    pub tilt: f32,
}
fn trail_jitter() -> f32 {
    0.12
}
fn trail_variance() -> f32 {
    0.4
}
fn trail_tilt() -> f32 {
    0.15
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Herd {
    pub species: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub display: u32,
    pub count: u32,
    pub at: [f32; 3],
    pub radius: f32,
    pub fit: f32,
    pub behaviour: String,
}
struct Prop {
    model: String,
    tf: Transform,
    indoors: bool,
    solid: bool,
    lift: f32,
}
#[derive(Resource, Default)]
struct Pending {
    props: Vec<Prop>,
    settled: bool,
    frames: u32,
    inventory: std::collections::HashMap<Entity, (Vec3, f32, f32)>,
    planned: Vec<(Vec3, Vec3, Handle<benilla_assets::M2Model>)>,
    /// Where each trail's props sit in `props`/`planned` (trails are planned after the scenes).
    trail_ranges: Vec<std::ops::Range<usize>>,
    /// Each planned prop's authored lift, parallel to `planned` (`props` shrinks as they spawn).
    lifts: Vec<f32>,
    report: ReportState,
    /// Claimed map doodads seen so far (they stream in with the terrain).
    claimed: usize,
}
#[derive(Default)]
struct ReportState {
    frames: u32,
    emitted: u32,
    /// Inventory size at the last report.
    seen: usize,
}
#[derive(Component)]
struct Designed;
#[derive(Component)]
pub(super) struct Gate;

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(load())
        .init_resource::<Pending>()
        .add_systems(OnEnter(Phase::Ready), reload.before(super::reset_run))
        .add_systems(
            Update,
            (spawn, validate, report)
                .chain()
                .after(benilla_world::schedule::WorldStage::Stream),
        );
}
fn load() -> Design {
    let embedded = include_str!("goldshire.toml");
    #[cfg(not(target_arch = "wasm32"))]
    let text = std::env::var("WOW_AZEROLL_LEVEL")
        .ok()
        .map(|p| {
            std::fs::read_to_string(&p).unwrap_or_else(|e| {
                error!("azeroll LEVEL read {p}: {e}; using embedded");
                embedded.into()
            })
        })
        .unwrap_or_else(|| embedded.into());
    #[cfg(target_arch = "wasm32")]
    let text = embedded.to_owned();
    let mut d: Design = toml::from_str(&text).unwrap_or_else(|e| {
        error!("azeroll LEVEL parse: {e}; using embedded");
        toml::from_str(embedded).expect("embedded level")
    });
    assert!(d.start_radius.is_finite() && d.start_radius > 0.0 && d.time_limit > 0.0);
    d.recipe_hash = fnv(&text);
    d.roster = d
        .herds
        .iter()
        .map(|h| Species {
            name: h.species.clone(),
            look: if h.display > 0 {
                Look::DisplayId(h.display)
            } else {
                Look::ModelPath(h.model.clone())
            },
            count: h.count,
            scale: 1.0,
            fit: Some(h.fit),
            place: Place::Around {
                at: h.at,
                radius: h.radius,
            },
            behavior: match h.behaviour.as_str() {
                "passive" => Behavior::Passive,
                "skittish" => Behavior::Skittish,
                "hostile" => Behavior::Hostile,
                other => panic!("unknown behaviour {other}"),
            },
            kind: if h.species == d.goal {
                Kind::Boss
            } else {
                Kind::Creature
            },
            walk: 1.0,
            run: 5.8,
            elite: 0.0,
        })
        .collect();
    let mut extras = Vec::new();
    for scene in &d.scenes {
        for (ix, item) in scene.items.iter().enumerate() {
            if item.species.is_empty() {
                continue;
            }
            let template = d
                .roster
                .iter()
                .find(|s| s.name == item.species)
                .unwrap_or_else(|| panic!("unknown scene species {}", item.species));
            for placed in plan_item(scene, ix, item) {
                let mut spec = template.clone();
                spec.count = 1;
                spec.place = Place::Around {
                    at: units::bevy_to_wow(placed.tf.translation),
                    radius: 0.0,
                };
                extras.push(spec);
            }
        }
    }
    d.roster.extend(extras);
    info!(
        "azeroll LEVEL: {} areas, {} scenes, {} trails, {} herds; goal={} limit={}",
        d.areas.len(),
        d.scenes.len(),
        d.trails.len(),
        d.herds.len(),
        d.goal,
        d.time_limit
    );
    d
}
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}
/// Small deterministic generator (splitmix64): a level edit moves only what it touches.
struct Rng(u64);
impl Rng {
    fn new(seed: &str, salt: usize) -> Self {
        let mut h = 0xcbf29ce484222325_u64;
        for b in seed.bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        Rng(h ^ (salt as u64).wrapping_mul(0x9e3779b97f4a7c15))
    }
    fn bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn f(&mut self) -> f32 {
        (self.bits() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Uniform in (-1, 1).
    fn signed(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }
    fn angle(&mut self) -> f32 {
        self.f() * std::f32::consts::TAU
    }
    /// A point in the disc of radius `r`, evenly spread.
    fn disc(&mut self, r: f32) -> Vec2 {
        Vec2::from_angle(self.angle()) * r * self.f().sqrt()
    }
}

/// A prop the plan wants: the model (empty for a creature), where, and its height off the ground.
struct Placed {
    model: String,
    tf: Transform,
    lift: f32,
}

/// Picks the model for prop `i` of a group.
fn pick_model(model: &str, models: &[String], pick: Pick, i: usize, rng: &mut Rng) -> String {
    match models.len() {
        0 => model.to_owned(),
        n => match pick {
            Pick::Cycle => models[i % n].clone(),
            Pick::Random => models[rng.bits() as usize % n].clone(),
        },
    }
}

/// A prop's rotation: its yaw, a lean for things that were dropped, or on its side.
fn tumble(rng: &mut Rng, yaw: f32, tilt: f32, topple: f32) -> (Quat, bool) {
    let axis = Quat::from_rotation_y(rng.angle()) * Vec3::X;
    let toppled = rng.f() < topple;
    let lean = if toppled {
        1.25 + 0.4 * rng.f()
    } else {
        rng.f() * tilt
    };
    (
        Quat::from_rotation_y(yaw) * Quat::from_axis_angle(axis, lean),
        toppled,
    )
}

/// Spots (east/south, yards) and heights for a group's props, before the scene's own yaw.
fn spots(item: &Item, rng: &mut Rng) -> Vec<(Vec2, f32)> {
    let n = item.count;
    let nf = n as f32;
    let sp = item.spacing;
    let turn = Vec2::from_angle(item.heading);
    let mut out: Vec<(Vec2, f32)> = Vec::with_capacity(n);
    // Rejection sampling so a cluster's props do not sit inside each other.
    let free = |out: &[(Vec2, f32)], rng: &mut Rng, draw: &dyn Fn(&mut Rng) -> Vec2| {
        let mut p = draw(rng);
        for _ in 0..10 {
            if item.gap <= 0.0 || out.iter().all(|(q, _)| q.distance(p) >= item.gap) {
                break;
            }
            p = draw(rng);
        }
        p
    };
    match item.arrangement {
        Arrangement::Line => {
            for i in 0..n {
                let mut along = (i as f32 - (nf - 1.0) * 0.5) * sp;
                if item.wobble > 0.0 {
                    along += rng.signed() * sp * 0.15;
                }
                out.push((
                    turn.rotate(Vec2::new(along, rng.signed() * item.wobble)),
                    0.0,
                ));
            }
        }
        Arrangement::Grid => {
            let w = nf.sqrt().ceil();
            for i in 0..n {
                let t = i as f32;
                let p = Vec2::new(
                    (t % w - (w - 1.0) * 0.5) * sp,
                    ((t / w).floor() - (w - 1.0) * 0.5) * sp,
                );
                out.push((p + Vec2::new(rng.signed(), rng.signed()) * item.wobble, 0.0));
            }
        }
        Arrangement::Ring => {
            for i in 0..n {
                let a = (i as f32 + rng.signed() * 0.3) / nf * std::f32::consts::TAU;
                out.push((Vec2::from_angle(a) * (sp + rng.signed() * item.wobble), 0.0));
            }
        }
        Arrangement::Pile => {
            for i in 0..n {
                let t = i as f32;
                out.push((Vec2::from_angle(t * 2.399963) * t.sqrt() * sp, 0.0));
            }
        }
        Arrangement::Scatter => {
            for i in 0..n {
                let t = i as f32;
                let p = Vec2::from_angle(t * 2.399963) * t.sqrt() * sp;
                out.push((p + Vec2::new(rng.signed(), rng.signed()) * sp * 0.45, 0.0));
            }
        }
        Arrangement::Cluster => {
            for _ in 0..n {
                let straggler = rng.f() < 0.12;
                let p = free(&out, rng, &|rng: &mut Rng| {
                    let r = if straggler {
                        sp * (1.4 + 1.2 * rng.f())
                    } else {
                        sp * 1.15 * rng.f().powf(1.6)
                    };
                    Vec2::from_angle(rng.angle()) * r
                });
                out.push((p, 0.0));
            }
        }
        Arrangement::Heap => {
            let low = (nf * 0.55).ceil() as usize;
            let mid = low + (nf * 0.3).ceil() as usize;
            for i in 0..n {
                let layer = if i < low {
                    0
                } else if i < mid {
                    1
                } else {
                    2
                };
                let r = sp * (1.0 - 0.4 * layer as f32);
                let p = free(&out, rng, &|rng: &mut Rng| rng.disc(r));
                out.push((p, layer as f32 * item.step));
            }
        }
        Arrangement::TableSet => {
            let per = n.div_ceil(item.tables.max(1)).max(1);
            for i in 0..n {
                let k = (i / per) as f32 - (item.tables.max(1) as f32 - 1.0) * 0.5;
                let centre = turn.rotate(Vec2::new(k * item.table_gap, 0.0));
                let p = free(&out, rng, &|rng: &mut Rng| {
                    centre + Vec2::new(rng.signed(), rng.signed()) * sp
                });
                out.push((p, item.surface));
            }
        }
        Arrangement::Spill => {
            let mut p = Vec2::ZERO;
            let mut h = item.heading;
            for _ in 0..n {
                out.push((p, 0.0));
                h += rng.signed() * item.bend;
                let mut step = sp * (0.3 + 1.4 * rng.f());
                if rng.f() < 0.25 {
                    step *= 0.25;
                }
                let dir = Vec2::from_angle(h);
                p += dir * step + dir.perp() * rng.signed() * sp * 0.3;
            }
        }
    }
    let at = Vec2::from(item.offset);
    for (p, _) in out.iter_mut() {
        *p += at;
    }
    out
}

/// The props of one scene item, deterministic for its scene name and position in the scene.
fn plan_item(scene: &Scene, ix: usize, item: &Item) -> Vec<Placed> {
    let mut rng = Rng::new(&scene.name, ix);
    let anchor = units::wow_to_bevy(scene.anchor);
    let scene_rot = Quat::from_rotation_y(scene.yaw);
    let mut out = Vec::new();
    let spots = spots(item, &mut rng);
    if matches!(item.arrangement, Arrangement::TableSet) && !item.table.is_empty() {
        let turn = Vec2::from_angle(item.heading);
        let tables = item.tables.max(1);
        for k in 0..tables {
            let along = (k as f32 - (tables as f32 - 1.0) * 0.5) * item.table_gap;
            let p = Vec2::from(item.offset) + turn.rotate(Vec2::new(along, 0.0));
            out.push(Placed {
                model: item.table.clone(),
                tf: Transform {
                    translation: anchor + scene_rot * Vec3::new(p.x, 0.0, p.y),
                    rotation: scene_rot
                        * Quat::from_rotation_y(-item.heading + rng.signed() * 0.12),
                    scale: Vec3::splat(item.table_scale),
                },
                lift: 0.0,
            });
        }
    }
    for (i, (p, layer_lift)) in spots.into_iter().enumerate() {
        let model = pick_model(&item.model, &item.models, item.pick, i, &mut rng);
        let scale = item.scale * (1.0 + item.jitter * rng.signed());
        let yaw = item.yaw.unwrap_or_else(|| rng.angle());
        let (rot, toppled) = tumble(&mut rng, yaw, item.tilt, item.topple);
        out.push(Placed {
            model,
            tf: Transform {
                translation: anchor + scene_rot * Vec3::new(p.x, 0.0, p.y),
                rotation: scene_rot * rot,
                scale: Vec3::splat(scale),
            },
            lift: item.lift
                + layer_lift
                + if toppled {
                    item.topple_lift * scale
                } else {
                    0.0
                },
        });
    }
    out
}

/// A trail that wanders: uneven steps, a drift off the straight line, the odd pair of drops.
fn plan_trail(t: &Trail) -> Vec<Placed> {
    let a = units::wow_to_bevy(t.from);
    let b = units::wow_to_bevy(t.to);
    let line = units::flat(b - a);
    let len = line.length().max(0.01);
    let dir = line / len;
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let mut rng = Rng::new(&t.name, 0);
    let (p1, p2) = (rng.angle(), rng.angle());
    let spacing = t.spacing.max(0.5);
    let mut out = Vec::new();
    let mut s = 0.0;
    let mut i = 0;
    while s <= len {
        let drift = t.wobble * (0.6 * (s * 0.17 + p1).sin() + 0.4 * (s * 0.41 + p2).sin());
        let at = a.lerp(b, s / len) + side * (drift + rng.signed() * t.wobble * 0.2);
        let drops = if rng.f() < t.clump { 2 } else { 1 };
        for d in 0..drops {
            let scale = t.scale * (1.0 + t.jitter * rng.signed());
            let yaw = rng.angle();
            let (rot, _) = tumble(&mut rng, yaw, t.tilt, 0.0);
            let nudge = if d == 0 {
                Vec3::ZERO
            } else {
                Vec3::new(rng.signed(), 0.0, rng.signed()) * spacing * 0.3
            };
            out.push(Placed {
                model: pick_model(&t.model, &t.models, t.pick, i, &mut rng),
                tf: Transform {
                    translation: at + nudge,
                    rotation: rot,
                    scale: Vec3::splat(scale),
                },
                lift: 0.0,
            });
            i += 1;
        }
        s += spacing * (1.0 + t.variance * rng.signed());
    }
    out
}
fn reload(mut d: ResMut<Design>, mut pending: ResMut<Pending>, assets: Res<AssetServer>) {
    *d = load();
    pending.props.clear();
    pending.settled = false;
    pending.frames = 0;
    pending.inventory.clear();
    pending.planned.clear();
    pending.trail_ranges.clear();
    pending.lifts.clear();
    pending.report = ReportState::default();
    pending.claimed = 0;
    for scene in &d.scenes {
        let mut total = 0;
        for (ix, item) in scene.items.iter().enumerate() {
            if !item.species.is_empty() {
                continue;
            }
            for placed in plan_item(scene, ix, item) {
                pending.props.push(Prop {
                    model: placed.model,
                    tf: placed.tf,
                    indoors: scene.indoors,
                    solid: item.solid,
                    lift: placed.lift,
                });
                total += 1;
            }
        }
        info!("azeroll scene: {} ({total} props)", scene.name);
    }
    for trail in &d.trails {
        let mut total = 0;
        let first = pending.props.len();
        for placed in plan_trail(trail) {
            pending.props.push(Prop {
                model: placed.model,
                tf: placed.tf,
                indoors: false,
                solid: false,
                lift: placed.lift,
            });
            total += 1;
        }
        let end = pending.props.len();
        pending.trail_ranges.push(first..end);
        info!("azeroll trail: {} ({total} props)", trail.name);
    }
    pending.lifts = pending.props.iter().map(|p| p.lift).collect();
    pending.planned = pending
        .props
        .iter()
        .map(|p| {
            (
                p.tf.translation,
                p.tf.scale,
                assets.load(benilla_assets::m2_url(&p.model)),
            )
        })
        .collect();
}
fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    world: WorldPoint,
    progress: Res<benilla_world::terrain_stream::WorldLoadProgress>,
    collision: WorldCollision,
    mut pending: ResMut<Pending>,
) {
    if !progress.complete {
        return;
    }
    let mut budget = 24;
    pending.props.retain(|prop| {
        if budget == 0 {
            return true;
        }
        let tf = &prop.tf;
        let ground = if prop.indoors {
            collision
                .ray_body(tf.translation + Vec3::Y * 4.0, Dir3::NEG_Y, 8.0)
                .map(|h| tf.translation.y + 4.0 - h.distance)
        } else {
            world.terrain_height_under(tf.translation)
        };
        let Some(y) = ground else {
            return true;
        };
        let mut tf = *tf;
        tf.translation.y = y + prop.lift;
        let e = super::props::spawn_copy(&mut commands, &assets, &prop.model, tf, None);
        commands.entity(e).insert((RunEntity, Designed));
        if prop.solid {
            commands.entity(e).insert(Gate);
        }
        budget -= 1;
        false
    });
}
/// Conservative max-axis cube bound for rooms whose exact object boxes are unavailable.
/// Volume addition is order independent.
#[cfg(test)]
pub(super) fn exit_bound(entry: f32, sizes: &[f32]) -> f32 {
    (entry.powi(3)
        + sizes
            .iter()
            .map(|s| super::collect::growth(*s, entry))
            .sum::<f32>())
    .cbrt()
}
/// What the level holds, as `(position, pickup axis, credited volume)`.
struct Inventory {
    /// Everything, trails included: what the per-area band check counts.
    all: Vec<(Vec3, f32, f32)>,
    /// Everything but the trails' props.
    loose: Vec<(Vec3, Food)>,
    /// Each trail's props in walking order from its `from` end.
    trails: Vec<Vec<Food>>,
    /// Planned props whose model bounds have not loaded yet.
    missing: usize,
    /// Creatures counted from the roster by the max-axis cube bound, not by a spawned box.
    estimated: usize,
}

/// A prop's pickup axis and credited volume from its model bounds, by the live law
/// ([`Collectible::volume`]) so the report cannot drift from what the ball is credited.
fn planned_food(bbox_min: [f32; 3], bbox_max: [f32; 3], scale: Vec3, rise: f32) -> Food {
    let half = units::wow_to_bevy(std::array::from_fn(|i| {
        (bbox_max[i] - bbox_min[i]).abs() * 0.5
    }))
    .abs();
    let probe = Collectible {
        name: "".into(),
        kind: Kind::Prop,
        source: Source::Spawned,
        size: 0.0,
        pickup: 0.0,
        half_extents: half,
        center: Vec3::ZERO,
        solid: 0.0,
    };
    Food {
        pickup: (2.0 * half * scale.abs()).max_element(),
        volume: probe.volume(scale),
        rise,
    }
}

fn inventory(
    pending: &Pending,
    d: &Design,
    models: &Assets<benilla_assets::M2Model>,
    ground: &dyn Fn(Vec3) -> Option<f32>,
) -> Inventory {
    let mut loose: Vec<(Vec3, Food)> = pending
        .inventory
        .values()
        .map(|&(pos, pickup, volume)| {
            let rise = ground(pos).map_or(0.0, |g| (pos.y - g).max(0.0));
            (
                pos,
                Food {
                    pickup,
                    volume,
                    rise,
                },
            )
        })
        .collect();
    let mut trails = vec![Vec::new(); d.trails.len()];
    let mut missing = 0;
    for (i, (pos, scale, handle)) in pending.planned.iter().enumerate() {
        let Some(bounds) = models.get(handle).and_then(|m| m.bounds) else {
            missing += 1;
            continue;
        };
        let rise = pending.lifts.get(i).copied().unwrap_or(0.0);
        let food = planned_food(bounds.bbox_min, bounds.bbox_max, *scale, rise);
        match pending.trail_ranges.iter().position(|r| r.contains(&i)) {
            Some(t) => trails[t].push((*pos, food)),
            None => loose.push((*pos, food)),
        }
    }
    let mut estimated = 0;
    for spec in &d.roster {
        let Place::Around { at, .. } = spec.place;
        for _ in 0..spec.count {
            let axis = spec.fit.unwrap_or(0.0);
            loose.push((
                units::wow_to_bevy(at),
                Food {
                    pickup: axis,
                    volume: super::collect::growth(axis, 0.0),
                    rise: 0.0,
                },
            ));
            estimated += 1;
        }
    }
    let mut all: Vec<(Vec3, f32, f32)> = loose
        .iter()
        .map(|(p, f)| (*p, f.pickup, f.volume))
        .collect();
    all.extend(
        trails
            .iter()
            .flatten()
            .map(|(p, f)| (*p, f.pickup, f.volume)),
    );
    Inventory {
        all,
        loose,
        trails: trails
            .into_iter()
            .map(|t| t.into_iter().map(|(_, f)| f).collect())
            .collect(),
        missing,
        estimated,
    }
}
type InventoryQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Collectible,
        Has<super::npcs::Npc>,
    ),
    Without<Attached>,
>;

/// Remember each loose thing's first-seen position, pickup axis and credited volume. Designed
/// props and creatures are planned or rostered instead, so only claimed doodads and NPCs go in.
fn remember(pending: &mut Pending, q: &InventoryQuery) {
    for (e, tf, c, npc) in q {
        if c.source == Source::Spawned && !npc {
            continue;
        }
        if pending
            .inventory
            .insert(e, (tf.translation, c.pickup, c.volume(tf.scale)))
            .is_none()
            && matches!(c.source, Source::Doodad(_))
        {
            pending.claimed += 1;
        }
    }
}

fn validate(
    d: Res<Design>,
    mut pending: ResMut<Pending>,
    q: InventoryQuery,
    unresolved: Query<(), (With<Designed>, Without<Collectible>)>,
    models: Res<Assets<benilla_assets::M2Model>>,
    npcs: Res<super::npcs::Pending>,
) {
    if pending.settled {
        return;
    }
    remember(&mut pending, &q);
    pending.frames += 1;
    if !pending.frames.is_multiple_of(300) {
        return;
    }
    let Inventory {
        all: inventory,
        missing,
        ..
    } = inventory(&pending, &d, &models, &|_| None);
    let bands =
        [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, f32::INFINITY].map(|r| r * level::COLLECT_RATIO);
    for area in &d.areas {
        let centre = units::wow_to_bevy(area.centre);
        let sizes: Vec<f32> = inventory
            .iter()
            .filter(|(pos, _, _)| {
                units::flat(*pos - centre).length() <= area.radius
                    && (!area.enclosed || (pos.y - centre.y).abs() < 3.0)
            })
            .map(|(_, size, _)| *size)
            .collect();
        let counts: Vec<usize> = bands
            .windows(2)
            .map(|b| sizes.iter().filter(|s| **s >= b[0] && **s < b[1]).count())
            .collect();
        info!(
            "azeroll VALIDATE {} bands={counts:?} pending={} unresolved={}",
            area.name,
            missing,
            unresolved.iter().count()
        );
        if area.enclosed {
            // Exact bbox volumes, independent of pickup order, including every object in
            // the room even if it cannot be eaten yet. The max-axis cube is needlessly loose
            // for long furniture and used only when creature bounds are unavailable.
            let credit: f32 = inventory
                .iter()
                .filter(|(pos, _, _)| {
                    units::flat(*pos - centre).length() <= area.radius
                        && (pos.y - centre.y).abs() < 3.0
                })
                .map(|(_, _, v)| *v)
                .sum();
            let bound = (area.clearance.powi(3) + credit).cbrt();
            // Footing keeps the centre one full radius above the floor, while the
            // swept sphere extends 0.85r upward. Height therefore needs 1.85r.
            let fits = 1.85 * bound <= 2.0 * area.clearance;
            if fits {
                info!(
                    "azeroll D10 PASS {} entry={} upper={} exit={} doors={}",
                    area.name,
                    area.clearance,
                    bound,
                    2.0 * area.clearance / 1.85,
                    area.doors.len()
                );
            } else {
                error!(
                    "azeroll D10 FAIL {} upper={} exit={}",
                    area.name,
                    bound,
                    2.0 * area.clearance / 1.85
                );
            }
        }
    }
    let sizes: Vec<f32> = inventory.iter().map(|(_, size, _)| *size).collect();
    for b in bands.windows(2) {
        if !sizes.iter().any(|s| *s >= b[0] && *s < b[1]) {
            error!("azeroll TIER GAP {}..{} (loaded coverage only)", b[0], b[1]);
        }
    }
    pending.settled = missing == 0 && unresolved.is_empty() && super::npcs::all_spawned(npcs);
}

/// Once the level has settled: write the per-segment volume table and the greedy growth curves
/// ([`progression`]). Claimed doodads stream in with the terrain, so far areas fill in later and
/// the report is redone (at most three more times) when the inventory has grown noticeably.
fn report(
    mut pending: ResMut<Pending>,
    d: Res<Design>,
    q: InventoryQuery,
    models: Res<Assets<benilla_assets::M2Model>>,
    world: WorldPoint,
) {
    if !pending.settled {
        return;
    }
    pending.report.frames += 1;
    if pending.report.emitted > 0 && !pending.report.frames.is_multiple_of(600) {
        return;
    }
    remember(&mut pending, &q);
    let seen = pending.inventory.len();
    let grown = seen >= pending.report.seen + (pending.report.seen / 20).max(100);
    if pending.report.emitted > 0 && (!grown || pending.report.emitted >= 4) {
        return;
    }
    let inv = inventory(&pending, &d, &models, &|p| world.terrain_height_under(p));
    let text = progression::render(&d, &inv, pending.claimed);
    pending.report.emitted += 1;
    pending.report.seen = seen;
    progression::emit(&text);
}
/// One thing the ball could eat: the maximum axis it must clear and the volume it credits.
#[derive(Clone, Copy, Debug)]
struct Food {
    pickup: f32,
    volume: f32,
    /// Height of the thing's base above the ground, yards: a plate on a table is not on the floor.
    rise: f32,
}

pub(in crate::azeroll) use progression::{
    area_at, build_id, plan_route, segment_offset, trail_ends, utc_now, RouteKind, Step,
};

/// Routes through the level, and what is on them: the autopilot's progression and the per-segment
/// volume report. Everything about contact and growth is asked of the live rules
/// ([`super::super::collect::contact`], [`Collectible::volume`]), never restated.
mod progression {
    use super::super::ball::top_speed;
    use super::super::collect::{contact, Contact};
    use super::*;
    use std::fmt::Write as _;

    // -----------------------------------------------------------------------------------------
    // Areas and routes
    // -----------------------------------------------------------------------------------------

    /// The area a Bevy-space point is inside (nearest by fraction of radius when they overlap);
    /// an enclosed area also needs the point within three yards of its floor.
    pub(in crate::azeroll) fn area_at(d: &Design, p: Vec3) -> Option<usize> {
        d.areas
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let c = units::wow_to_bevy(a.centre);
                if a.enclosed && (p.y - c.y).abs() >= 3.0 {
                    return None;
                }
                let f = units::flat(p - c).length() / a.radius.max(0.1);
                (f <= 1.0).then_some((i, f))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn nearest_area(d: &Design, p: Vec3) -> Option<usize> {
        d.areas
            .iter()
            .enumerate()
            .filter(|(_, a)| !a.enclosed)
            .min_by(|a, b| {
                let f = |x: &Area| {
                    units::flat(p - units::wow_to_bevy(x.centre)).length() / x.radius.max(0.1)
                };
                f(a.1).total_cmp(&f(b.1))
            })
            .map(|(i, _)| i)
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::azeroll) enum RouteKind {
        /// The shortest trail walk from the start area to the camp.
        Primary,
        /// Every open area on the trail graph, then the camp.
        Tour,
    }

    /// One leg of a route: an area to clear, or a trail to walk (`reversed`: from its `to` end).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::azeroll) enum Step {
        Area(usize),
        Trail { index: usize, reversed: bool },
    }

    /// A trail's two ends in Bevy space, in walking order.
    pub(in crate::azeroll) fn trail_ends(d: &Design, index: usize, reversed: bool) -> (Vec3, Vec3) {
        let t = &d.trails[index];
        let (a, b) = (units::wow_to_bevy(t.from), units::wow_to_bevy(t.to));
        if reversed {
            (b, a)
        } else {
            (a, b)
        }
    }

    /// Where `p` sits relative to the flat segment `a`..`b`: `(distance off the line, yards along
    /// it from `a`)`. The along value is not clamped, so callers can tell before from beyond.
    pub(in crate::azeroll) fn segment_offset(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
        let line = units::flat(b - a);
        let len = line.length().max(0.001);
        let dir = line / len;
        let rel = units::flat(p - a);
        let along = rel.dot(dir);
        ((rel - dir * along).length(), along)
    }

    struct Edge {
        a: usize,
        b: usize,
        trail: usize,
        len: f32,
    }

    /// The trails as a graph. Nodes `0..areas` are the areas (so a node index is an area index);
    /// further nodes are trail ends that lie in no open area, merged when within twelve yards.
    struct Graph {
        pts: Vec<Vec3>,
        edges: Vec<Edge>,
        areas: usize,
    }

    impl Graph {
        fn new(d: &Design) -> Self {
            let mut pts: Vec<Vec3> = d
                .areas
                .iter()
                .map(|a| units::wow_to_bevy(a.centre))
                .collect();
            let areas = pts.len();
            let mut edges = Vec::new();
            let node = |pts: &mut Vec<Vec3>, p: Vec3| -> usize {
                let inside = d
                    .areas
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| !a.enclosed)
                    .map(|(i, a)| {
                        let f = units::flat(p - units::wow_to_bevy(a.centre)).length()
                            / a.radius.max(0.1);
                        (i, f)
                    })
                    .filter(|(_, f)| *f <= 1.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((i, _)) = inside {
                    return i;
                }
                if let Some(i) =
                    (areas..pts.len()).find(|&i| units::flat(pts[i] - p).length() < 12.0)
                {
                    return i;
                }
                pts.push(p);
                pts.len() - 1
            };
            for (trail, t) in d.trails.iter().enumerate() {
                let (from, to) = (units::wow_to_bevy(t.from), units::wow_to_bevy(t.to));
                let (a, b) = (node(&mut pts, from), node(&mut pts, to));
                if a != b {
                    edges.push(Edge {
                        a,
                        b,
                        trail,
                        len: units::flat(to - from).length(),
                    });
                }
            }
            Self { pts, edges, areas }
        }

        /// The shortest walk: its length and `(edge, travelled a→b)` hops.
        fn walk(&self, from: usize, to: usize) -> Option<(f32, Vec<(usize, bool)>)> {
            let n = self.pts.len();
            let mut dist = vec![f32::INFINITY; n];
            let mut via: Vec<Option<(usize, usize, bool)>> = vec![None; n];
            let mut done = vec![false; n];
            dist[from] = 0.0;
            while let Some(u) = (0..n)
                .filter(|&i| !done[i] && dist[i].is_finite())
                .min_by(|&a, &b| dist[a].total_cmp(&dist[b]))
            {
                done[u] = true;
                for (ei, e) in self.edges.iter().enumerate() {
                    let (v, fwd) = if e.a == u {
                        (e.b, true)
                    } else if e.b == u {
                        (e.a, false)
                    } else {
                        continue;
                    };
                    if dist[u] + e.len < dist[v] {
                        dist[v] = dist[u] + e.len;
                        via[v] = Some((u, ei, fwd));
                    }
                }
            }
            if !dist[to].is_finite() {
                return None;
            }
            let (mut hops, mut at) = (Vec::new(), to);
            while at != from {
                let (u, ei, fwd) = via[at]?;
                hops.push((ei, fwd));
                at = u;
            }
            hops.reverse();
            Some((dist[to], hops))
        }

        /// The steps of a walk: each trail, and each open area passed through or reached.
        fn steps(&self, from: usize, hops: &[(usize, bool)]) -> Vec<Step> {
            let mut out = Vec::new();
            if from < self.areas {
                out.push(Step::Area(from));
            }
            for &(ei, fwd) in hops {
                let e = &self.edges[ei];
                out.push(Step::Trail {
                    index: e.trail,
                    reversed: !fwd,
                });
                let at = if fwd { e.b } else { e.a };
                if at < self.areas {
                    out.push(Step::Area(at));
                }
            }
            out
        }
    }

    /// The cheapest order for visiting `optional` between `start` and `goal` (exact up to eight).
    fn best_order(g: &Graph, start: usize, goal: usize, optional: &[usize]) -> Vec<usize> {
        let cost = |a: usize, b: usize| g.walk(a, b).map_or(f32::INFINITY, |w| w.0);
        fn go(
            cost: &dyn Fn(usize, usize) -> f32,
            at: usize,
            goal: usize,
            left: &mut Vec<usize>,
            path: &mut Vec<usize>,
            sum: f32,
            best: &mut (f32, Vec<usize>),
        ) {
            if sum >= best.0 {
                return;
            }
            if left.is_empty() {
                let total = sum + cost(at, goal);
                if total < best.0 {
                    *best = (total, path.clone());
                }
                return;
            }
            for i in 0..left.len() {
                let next = left.remove(i);
                path.push(next);
                go(cost, next, goal, left, path, sum + cost(at, next), best);
                path.pop();
                left.insert(i, next);
            }
        }
        if optional.len() <= 8 {
            let mut best = (f32::INFINITY, optional.to_vec());
            go(
                &cost,
                start,
                goal,
                &mut optional.to_vec(),
                &mut Vec::new(),
                0.0,
                &mut best,
            );
            best.1
        } else {
            let (mut at, mut left, mut out) = (start, optional.to_vec(), Vec::new());
            while !left.is_empty() {
                let i = (0..left.len())
                    .min_by(|&a, &b| cost(at, left[a]).total_cmp(&cost(at, left[b])))
                    .unwrap_or(0);
                at = left.remove(i);
                out.push(at);
            }
            out
        }
    }

    /// The legs the autopilot walks, and the report's greedy simulation follows. Empty when the
    /// recipe has no area to start in. Enclosed areas are never on a route (no trail enters them).
    pub(in crate::azeroll) fn plan_route(d: &Design, kind: RouteKind) -> Vec<Step> {
        let g = Graph::new(d);
        let here = units::wow_to_bevy(d.start);
        let Some(start) = area_at(d, here).or_else(|| nearest_area(d, here)) else {
            return Vec::new();
        };
        let camp = units::wow_to_bevy(d.camp);
        let goal = area_at(d, camp)
            .or_else(|| nearest_area(d, camp))
            .unwrap_or(start);
        let mut stops = vec![start];
        if kind == RouteKind::Tour {
            let optional: Vec<usize> = (0..g.areas)
                .filter(|&i| {
                    i != start
                        && i != goal
                        && !d.areas[i].enclosed
                        && g.walk(start, i).is_some()
                        && g.walk(i, goal).is_some()
                })
                .collect();
            stops.extend(best_order(&g, start, goal, &optional));
        }
        stops.push(goal);
        let mut steps: Vec<Step> = Vec::new();
        for pair in stops.windows(2) {
            let Some((_, hops)) = g.walk(pair[0], pair[1]) else {
                continue;
            };
            for s in g.steps(pair[0], &hops) {
                if steps.last() != Some(&s) {
                    steps.push(s);
                }
            }
        }
        if steps.is_empty() {
            steps.push(Step::Area(start));
        }
        steps
    }

    impl Step {
        pub(in crate::azeroll) fn label(&self, d: &Design) -> String {
            match *self {
                Step::Area(i) => d.areas[i].name.clone(),
                Step::Trail { index, reversed } => format!(
                    "{}{}",
                    d.trails[index].name,
                    if reversed { " (back)" } else { "" }
                ),
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Build identity
    // -----------------------------------------------------------------------------------------

    /// `YYYY-MM-DD HH:MM:SSZ` for seconds since the Unix epoch.
    fn utc(secs: u64) -> String {
        let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
    }

    pub(in crate::azeroll) fn utc_now() -> String {
        utc(web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()))
    }

    /// The running binary's path, size and modification time: what ties a report to a build.
    pub(in crate::azeroll) fn build_id() -> String {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Ok(exe) = std::env::current_exe() else {
                return "unknown binary".into();
            };
            let Ok(meta) = std::fs::metadata(&exe) else {
                return format!("{} (unreadable)", exe.display());
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or_else(|| "?".into(), |d| utc(d.as_secs()));
            format!("{} {} bytes, mtime {mtime}", exe.display(), meta.len())
        }
        #[cfg(target_arch = "wasm32")]
        {
            "wasm build".into()
        }
    }

    // -----------------------------------------------------------------------------------------
    // The volume report
    // -----------------------------------------------------------------------------------------

    /// Ball radii that separate the report's size bands, by the radius needed to roll a thing up.
    const EDGES: [f32; 6] = [0.5, 1.0, 1.5, 2.5, 4.0, 6.0];
    const BANDS: [&str; 7] = ["<0.5", "0.5-1", "1-1.5", "1.5-2.5", "2.5-4", "4-6", "6-12"];
    /// A thing that needs a ball this big is scenery for pacing: three times what Hogger asks.
    /// It is counted, not summed (one house-sized box outweighs everything the player eats).
    const SCENERY_NEED: f32 = 12.0;

    /// The smallest ball radius that rolls up a thing with this maximum axis, by the live rule.
    fn need(pickup: f32) -> f32 {
        let (mut lo, mut hi) = (0.0_f32, 64.0_f32);
        if contact(hi, pickup) != Contact::RollUp {
            return f32::INFINITY;
        }
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if contact(mid, pickup) == Contact::RollUp {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// The smallest ball radius that gets at a thing: big enough to roll it up (the live rule) and
    /// to reach it. A ball can touch what sits up to about 1.5 radii (plus the thing's own reach)
    /// above the ground, and has no way up a table.
    fn food_need(f: &Food) -> f32 {
        need(f.pickup).max((f.rise - 0.14 * f.pickup) / 1.5)
    }

    fn band(need: f32) -> usize {
        EDGES.iter().position(|e| need < *e).unwrap_or(EDGES.len())
    }

    /// The level sorted into the places a route can pass through.
    struct Level {
        areas: Vec<Vec<Food>>,
        /// Each trail's props in walking order from its `from` end.
        trails: Vec<Vec<Food>>,
        /// Not inside any open or enclosed area (farms between the areas, forest).
        elsewhere: Vec<Food>,
        /// How many things per area, trail and elsewhere (last) are scenery ([`SCENERY_NEED`]).
        scenery: Vec<usize>,
    }

    impl Level {
        /// Drop the scenery from `foods`, returning how much there was.
        fn split(foods: Vec<Food>) -> (Vec<Food>, usize) {
            let (keep, scenery): (Vec<Food>, Vec<Food>) =
                foods.into_iter().partition(|f| food_need(f) < SCENERY_NEED);
            (keep, scenery.len())
        }
    }

    fn level(d: &Design, inv: &Inventory) -> Level {
        let mut areas = vec![Vec::new(); d.areas.len()];
        let mut elsewhere = Vec::new();
        for &(pos, food) in &inv.loose {
            match area_at(d, pos) {
                Some(i) => areas[i].push(food),
                None => elsewhere.push(food),
            }
        }
        let mut scenery = Vec::new();
        let mut cut = |foods: Vec<Food>| {
            let (keep, n) = Level::split(foods);
            scenery.push(n);
            keep
        };
        let areas: Vec<_> = areas.into_iter().map(&mut cut).collect();
        let trails: Vec<_> = inv.trails.iter().cloned().map(&mut cut).collect();
        let elsewhere = cut(elsewhere);
        Level {
            areas,
            trails,
            elsewhere,
            scenery,
        }
    }

    fn cells(foods: &[Food]) -> ([usize; 7], [f32; 7]) {
        let (mut n, mut v) = ([0; 7], [0.0; 7]);
        for f in foods {
            let b = band(food_need(f));
            n[b] += 1;
            v[b] += f.volume;
        }
        (n, v)
    }

    struct Row {
        label: String,
        arrive_r: f32,
        avail: (usize, f32),
        ate: (usize, f32),
        exit_r: f32,
        left: (usize, f32),
        /// The smallest ball radius that would eat something still left here.
        next_need: f32,
        /// Cumulative pure travel time to the end of this step, seconds.
        clock: f32,
        flag: &'static str,
    }

    /// If a player ate everything edible, step by step, what radius do they carry out of each?
    /// An area is cleared smallest first (it can be roamed in any order); a trail is walked once,
    /// taking what the ball can take as it passes. The clock counts travel only.
    fn simulate(d: &Design, lv: &Level, steps: &[Step]) -> Vec<Row> {
        let mut volume = d.start_radius.powi(3);
        let mut eaten_area: Vec<Vec<bool>> =
            lv.areas.iter().map(|a| vec![false; a.len()]).collect();
        let mut eaten_trail: Vec<Vec<bool>> =
            lv.trails.iter().map(|a| vec![false; a.len()]).collect();
        let mut at = units::wow_to_bevy(d.start);
        let mut clock = 0.0;
        let mut rows = Vec::new();
        for &step in steps {
            let arrive_r = volume.cbrt();
            let edible = |f: &Food, r: f32| r >= food_need(f);
            let (label, foods, eaten, order, reach, travel): (_, &[Food], &mut Vec<bool>, _, _, _) =
                match step {
                    Step::Area(i) => {
                        let centre = units::wow_to_bevy(d.areas[i].centre);
                        let mut order: Vec<usize> = (0..lv.areas[i].len()).collect();
                        order.sort_by(|a, b| {
                            lv.areas[i][*a].pickup.total_cmp(&lv.areas[i][*b].pickup)
                        });
                        let travel = units::flat(centre - at).length();
                        at = centre;
                        (
                            step.label(d),
                            &lv.areas[i][..],
                            &mut eaten_area[i],
                            order,
                            true,
                            travel,
                        )
                    }
                    Step::Trail { index, reversed } => {
                        let (from, to) = trail_ends(d, index, reversed);
                        let mut order: Vec<usize> = (0..lv.trails[index].len()).collect();
                        if reversed {
                            order.reverse();
                        }
                        let travel =
                            units::flat(from - at).length() + units::flat(to - from).length();
                        at = to;
                        (
                            step.label(d),
                            &lv.trails[index][..],
                            &mut eaten_trail[index],
                            order,
                            false,
                            travel,
                        )
                    }
                };
            clock += travel / top_speed(arrive_r).max(0.5);
            let (mut avail, mut ate) = ((0, 0.0), (0, 0.0));
            for (i, f) in foods.iter().enumerate() {
                if !eaten[i] && edible(f, arrive_r) {
                    avail = (avail.0 + 1, avail.1 + f.volume);
                }
            }
            for i in order {
                if eaten[i] {
                    continue;
                }
                if edible(&foods[i], volume.cbrt()) {
                    eaten[i] = true;
                    volume += foods[i].volume;
                    ate = (ate.0 + 1, ate.1 + foods[i].volume);
                } else if reach {
                    // Smallest first: nothing later in an area is any smaller.
                    break;
                }
            }
            let mut left = (0, 0.0);
            let mut next_need = f32::INFINITY;
            for (i, f) in foods.iter().enumerate() {
                if !eaten[i] {
                    left = (left.0 + 1, left.1 + f.volume);
                    next_need = next_need.min(food_need(f));
                }
            }
            let v_in = arrive_r.powi(3);
            let flag = if foods.is_empty() {
                "empty"
            } else if avail.1 < 0.05 * v_in && left.0 > 0 {
                "STARVE"
            } else if avail.1 < 0.25 * v_in && left.0 > 0 {
                "thin"
            } else {
                ""
            };
            rows.push(Row {
                label,
                arrive_r,
                avail,
                ate,
                exit_r: volume.cbrt(),
                left,
                next_need,
                clock,
                flag,
            });
        }
        rows
    }

    /// The whole report as text: the segment-by-band table, then a greedy curve per route.
    pub(super) fn render(d: &Design, inv: &Inventory, claimed: usize) -> String {
        let lv = level(d, inv);
        let mut out = String::new();
        let _ = writeln!(
            out,
            "== azeroll level report: recipe fnv={:016x} start_r={} limit={}s goal={} ==",
            d.recipe_hash, d.start_radius, d.time_limit, d.goal
        );
        let _ = writeln!(out, "build: {}   report time: {}", build_id(), utc_now());
        let _ = writeln!(
            out,
            "items: {} ({} claimed map doodads streamed so far, {} planned props, {} creatures by cube bound, {} planned props unresolved)",
            inv.all.len(),
            claimed,
            inv.all.len() - claimed.min(inv.all.len()) - inv.estimated,
            inv.estimated,
            inv.missing
        );
        let _ = writeln!(
            out,
            "Cells are count/volume. Volume is in ball volume units (a ball of radius r holds r^3), by the live additive box law."
        );
        let _ = writeln!(
            out,
            "Bands are the ball radius needed to roll a thing up (collect::contact), not its size. \
             Scenery (needs r >= {SCENERY_NEED}) is counted, not summed, and never 'left'.\n"
        );
        let _ = write!(out, "{:<28} {:>6} {:>7}", "segment", "kind", "items");
        for b in BANDS {
            let _ = write!(out, " {b:>10}");
        }
        let _ = writeln!(out, " {:>8} {:>8}", "total", "scenery");
        let mut line = |name: &str, kind: &str, foods: &[Food], scenery: usize| {
            let (n, v) = cells(foods);
            let _ = write!(out, "{name:<28.28} {kind:>6} {:>7}", foods.len());
            for i in 0..7 {
                let cell = if n[i] == 0 {
                    "-".to_string()
                } else {
                    format!("{}/{:.2}", n[i], v[i])
                };
                let _ = write!(out, " {cell:>10}");
            }
            let _ = writeln!(out, " {:>8.2} {:>8}", v.iter().sum::<f32>(), scenery);
        };
        let (na, nt) = (d.areas.len(), d.trails.len());
        for (i, a) in d.areas.iter().enumerate() {
            let kind = if a.enclosed { "indoor" } else { "area" };
            line(&a.name, kind, &lv.areas[i], lv.scenery[i]);
        }
        for (i, t) in d.trails.iter().enumerate() {
            line(&t.name, "trail", &lv.trails[i], lv.scenery[na + i]);
        }
        line(
            "(outside every area)",
            "-",
            &lv.elsewhere,
            lv.scenery[na + nt],
        );
        out.push('\n');

        for kind in [RouteKind::Primary, RouteKind::Tour] {
            let steps = plan_route(d, kind);
            let rows = simulate(d, &lv, &steps);
            let _ = writeln!(
                out,
                "== greedy growth, {kind:?} route (eat everything edible, step by step; clock = travel only) =="
            );
            let _ = writeln!(
                out,
                "{:<28} {:>6} {:>12} {:>12} {:>6} {:>12} {:>8} {:>7}  flag",
                "step", "in_r", "edible_in", "ate", "out_r", "left", "next_r", "clock_s"
            );
            for r in &rows {
                let next = if r.next_need.is_finite() {
                    format!("{:.2}", r.next_need)
                } else {
                    "-".into()
                };
                let _ = writeln!(
                    out,
                    "{:<28.28} {:>6.2} {:>12} {:>12} {:>6.2} {:>12} {:>8} {:>7.0}  {}",
                    r.label,
                    r.arrive_r,
                    format!("{}/{:.2}", r.avail.0, r.avail.1),
                    format!("{}/{:.2}", r.ate.0, r.ate.1),
                    r.exit_r,
                    format!("{}/{:.2}", r.left.0, r.left.1),
                    next,
                    r.clock,
                    r.flag
                );
            }
            let hogger = super::super::level::HOGGER_SIZE;
            let need_r = need(hogger);
            let end = rows.last();
            let _ = writeln!(
                out,
                "Hogger (axis {hogger}) needs r={need_r:.2}; this route ends at r={:.2} after {:.0}s of travel ({}).\n",
                end.map_or(d.start_radius, |r| r.exit_r),
                end.map_or(0.0, |r| r.clock),
                if end.is_some_and(|r| r.exit_r >= need_r) {
                    "edible on arrival at the camp"
                } else {
                    "NOT edible on this route alone"
                }
            );
        }
        out
    }

    /// Log the report line by line, and write it to `$WOW_AZEROLL_REPORT` when that is set.
    pub(super) fn emit(text: &str) {
        for l in text.lines() {
            info!("azeroll REPORT {l}");
        }
        if let Some(path) = crate::webenv::var("WOW_AZEROLL_REPORT") {
            if let Some(dir) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::write(&path, text) {
                Ok(()) => info!("azeroll REPORT written to {path}"),
                Err(e) => warn!("azeroll REPORT write {path}: {e}"),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn embedded() -> Design {
            toml::from_str(include_str!("goldshire.toml")).expect("embedded level parses")
        }
        fn foods(items: &[(f32, f32)]) -> Vec<Food> {
            items
                .iter()
                .map(|&(pickup, volume)| Food {
                    pickup,
                    volume,
                    rise: 0.0,
                })
                .collect()
        }

        fn food(pickup: f32, volume: f32, rise: f32) -> Food {
            Food {
                pickup,
                volume,
                rise,
            }
        }

        #[test]
        fn what_sits_on_a_table_waits_for_a_bigger_ball() {
            let plate = food(0.3, 0.01, 0.0);
            let on_a_table = food(0.3, 0.01, 1.0);
            assert!((food_need(&plate) - need(0.3)).abs() < 1e-6);
            assert!(
                food_need(&on_a_table) > 0.6,
                "a one-yard table is out of reach of a small ball"
            );
            assert!(food_need(&on_a_table) < 1.0);
            // A big thing is limited by its size, not its height.
            assert_eq!(food_need(&food(2.0, 1.0, 0.1)), need(2.0));
        }

        #[test]
        fn utc_matches_known_instants() {
            assert_eq!(utc(0), "1970-01-01 00:00:00Z");
            assert_eq!(utc(86_399), "1970-01-01 23:59:59Z");
            assert_eq!(utc(951_782_400), "2000-02-29 00:00:00Z");
            assert_eq!(utc(1_793_604_833), "2026-11-02 07:33:53Z");
        }

        #[test]
        fn need_is_the_edge_of_the_live_contact_rule() {
            for pickup in [0.1, 0.5, 1.0, 2.8, 4.0] {
                let r = need(pickup);
                assert_eq!(contact(r + 0.001, pickup), Contact::RollUp, "{pickup}");
                assert_ne!(contact(r - 0.01, pickup), Contact::RollUp, "{pickup}");
            }
            assert!(need(1.0e9).is_infinite());
            assert!(band(0.1) < band(1.2) && band(1.2) < band(5.0) && band(5.0) < band(50.0));
        }

        #[test]
        fn routes_run_from_the_start_area_to_the_camp() {
            let d = embedded();
            let primary = plan_route(&d, RouteKind::Primary);
            let tour = plan_route(&d, RouteKind::Tour);
            for route in [&primary, &tour] {
                assert!(matches!(route.first(), Some(Step::Area(_))), "{route:?}");
                assert!(matches!(route.last(), Some(Step::Area(_))), "{route:?}");
                // A trail may chain into another (through a bend that is in no area), but two
                // areas are never adjacent: something is walked between them.
                assert!(
                    route
                        .windows(2)
                        .all(|p| !matches!(p, [Step::Area(_), Step::Area(_)])),
                    "{route:?}"
                );
            }
            assert_eq!(primary.first(), tour.first());
            assert_eq!(primary.last(), tour.last());
            let Some(&Step::Area(start)) = primary.first() else {
                unreachable!()
            };
            assert_eq!(
                area_at(&d, units::wow_to_bevy(d.start)).unwrap_or(start),
                start,
                "the route starts where the ball does"
            );
            // The tour may walk its own trails (the north road skips the inn yard), but it visits
            // every area the primary route does.
            assert!(
                primary
                    .iter()
                    .filter(|s| matches!(s, Step::Area(_)))
                    .all(|s| tour.contains(s)),
                "the tour visits every area the primary route does"
            );
            assert!(tour.len() >= primary.len());
        }

        #[test]
        fn a_route_clears_areas_smallest_first_and_stops_at_what_is_too_big() {
            let d = embedded();
            assert!(d.areas.len() >= 2);
            let mut areas = vec![Vec::new(); d.areas.len()];
            // Crumbs a start ball eats, then a thing only a bigger ball can; the second area
            // holds only a giant.
            areas[0] = foods(&[(0.05, 0.001), (0.05, 0.001), (0.12, 0.5), (500.0, 9.0)]);
            areas[1] = foods(&[(500.0, 9.0)]);
            let lv = Level {
                areas,
                trails: d.trails.iter().map(|_| Vec::new()).collect(),
                elsewhere: Vec::new(),
                scenery: Vec::new(),
            };
            let rows = simulate(&d, &lv, &[Step::Area(0), Step::Area(1)]);
            assert_eq!(rows.len(), 2);
            let v0 = d.start_radius.powi(3);
            assert!((rows[0].arrive_r - d.start_radius).abs() < 1e-5);
            // The crumbs, then the half-volume thing they made edible; never the giant.
            assert_eq!(rows[0].ate.0, 3);
            assert!((rows[0].exit_r.powi(3) - (v0 + 0.502)).abs() < 1e-4);
            assert_eq!(rows[0].left.0, 1);
            assert_eq!(rows[1].ate.0, 0);
            assert_eq!(rows[1].flag, "STARVE");
            assert!((rows[1].exit_r - rows[1].arrive_r).abs() < 1e-6);
            assert!(rows[1].clock >= rows[0].clock);
        }

        #[test]
        fn scenery_is_counted_but_never_summed_or_left() {
            let d = embedded();
            let centre = units::wow_to_bevy(d.areas[1].centre);
            let inv = Inventory {
                all: Vec::new(),
                loose: vec![
                    (centre, food(0.3, 0.01, 0.0)),
                    (centre, food(30.0, 4000.0, 0.0)),
                    (centre + Vec3::X * 1.0e5, food(30.0, 4000.0, 0.0)),
                ],
                trails: d.trails.iter().map(|_| Vec::new()).collect(),
                missing: 0,
                estimated: 0,
            };
            let lv = level(&d, &inv);
            assert_eq!(lv.areas[1].len(), 1, "only the crumb is food");
            assert_eq!(lv.scenery[1], 1);
            assert!(lv.elsewhere.is_empty());
            assert_eq!(lv.scenery[d.areas.len() + d.trails.len()], 1);
            let text = render(&d, &inv, 0);
            assert!(
                text.contains("Primary route") && text.contains("Tour route"),
                "{text}"
            );
            assert!(text.contains("scenery"));
        }

        #[test]
        fn a_trail_is_walked_once_in_order() {
            let d = embedded();
            let Some(t) = d.trails.iter().position(|t| !t.name.is_empty()) else {
                return;
            };
            let mut trails: Vec<Vec<Food>> = d.trails.iter().map(|_| Vec::new()).collect();
            // A boulder first, then crumbs: walked forward it is passed over, never come back to.
            trails[t] = foods(&[(500.0, 9.0), (0.05, 0.001), (0.05, 0.001)]);
            let lv = Level {
                areas: vec![Vec::new(); d.areas.len()],
                trails,
                elsewhere: Vec::new(),
                scenery: Vec::new(),
            };
            let forward = simulate(
                &d,
                &lv,
                &[Step::Trail {
                    index: t,
                    reversed: false,
                }],
            );
            assert_eq!((forward[0].ate.0, forward[0].left.0), (2, 1));
            let back = simulate(
                &d,
                &lv,
                &[Step::Trail {
                    index: t,
                    reversed: true,
                }],
            );
            assert_eq!((back[0].ate.0, back[0].left.0), (2, 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn embedded() -> Design {
        toml::from_str(include_str!("goldshire.toml")).expect("embedded level parses")
    }
    #[test]
    fn plans_are_deterministic_and_complete() {
        let d = embedded();
        for scene in &d.scenes {
            for (ix, item) in scene.items.iter().enumerate() {
                let a = plan_item(scene, ix, item);
                let b = plan_item(scene, ix, item);
                let tables = usize::from(
                    matches!(item.arrangement, Arrangement::TableSet) && !item.table.is_empty(),
                ) * item.tables.max(1);
                assert_eq!(a.len(), item.count + tables, "{} item {ix}", scene.name);
                for (p, q) in a.iter().zip(&b) {
                    assert_eq!(p.tf.translation, q.tf.translation);
                    assert_eq!(p.lift, q.lift);
                    assert!(p.tf.translation.is_finite() && p.lift.is_finite());
                }
                if item.species.is_empty() {
                    assert!(
                        a.iter().all(|p| !p.model.is_empty()),
                        "{} item {ix}",
                        scene.name
                    );
                }
            }
        }
        assert!(d.trails.iter().all(|t| plan_trail(t).len() >= 2));
    }
    #[test]
    fn mixed_groups_use_more_than_one_model() {
        let d = embedded();
        for scene in d.scenes.iter().filter(|s| !s.items.iter().any(|i| i.solid)) {
            let models: std::collections::HashSet<_> = scene
                .items
                .iter()
                .enumerate()
                .flat_map(|(ix, item)| plan_item(scene, ix, item))
                .map(|p| p.model)
                .collect();
            assert!(models.len() >= 5, "{} is nearly one model", scene.name);
        }
    }
    #[test]
    fn heaps_stack_and_table_sets_sit_on_the_surface() {
        let scene = Scene {
            name: "t".into(),
            anchor: [0.0; 3],
            yaw: 0.0,
            indoors: false,
            items: vec![],
        };
        let item = |arrangement, step, surface| Item {
            model: "m".into(),
            models: vec![],
            pick: Pick::Random,
            species: String::new(),
            count: 20,
            arrangement,
            spacing: 1.0,
            scale: 1.0,
            jitter: 0.0,
            solid: false,
            offset: [0.0; 2],
            lift: 0.0,
            yaw: None,
            tilt: 0.0,
            topple: 0.0,
            topple_lift: 0.0,
            wobble: 0.0,
            gap: 0.0,
            step,
            heading: 0.0,
            bend: 0.0,
            table: String::new(),
            tables: 1,
            table_gap: 1.0,
            table_scale: 1.0,
            surface,
        };
        let heap = plan_item(&scene, 0, &item(Arrangement::Heap, 0.5, 0.0));
        assert!(heap.iter().any(|p| p.lift == 0.0) && heap.iter().any(|p| p.lift >= 0.5));
        let table = plan_item(&scene, 0, &item(Arrangement::TableSet, 0.0, 1.0));
        assert!(table.iter().all(|p| p.lift == 1.0));
    }
    #[test]
    fn d10_bound_covers_actual_growth_and_rejects_overfilled_room() {
        let items = vec![0.12; 80];
        let mut r = 1.89_f32;
        for s in &items {
            r = (r.powi(3) + super::super::collect::growth(*s, r)).cbrt();
        }
        assert!(exit_bound(1.89, &items) >= r - 0.00001);
        assert!(0.925 * exit_bound(1.89, &items) < 1.89);
        assert!(0.925 * exit_bound(1.89, &vec![1.0; 100]) > 1.89);
    }
}
