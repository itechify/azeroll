//! The ball: its three entities, WASD rolling over the terrain, and the streaming focus on it.
//!
//! There is no rigid-body simulation. [`BallState`] is the authority: a horizontal velocity driven
//! toward the input direction (heavier balls take longer to get going), gravity only when the ground
//! falls away, and the terrain heightfield for grounding. avian's sphere slide
//! ([`WorldCollision::slide_body`]) is used for the horizontal move only, so buildings and the
//! engine-owned doodads stop the ball; everything the mode owns is handled in [`super::collect`].
//!
//! # Material
//!
//! The core is a plain Bevy `Mesh3d` sphere with an unlit `StandardMaterial`. The world's lighting
//! is the engine's own gamma-space model law (`WowModelMaterial`, fed by a shared light buffer), not
//! Bevy's PBR lights, and there are no Bevy lights in the scene, so a lit `StandardMaterial` would
//! come out near black. An unlit one renders reliably (the nameplates and the fallback cubes use
//! the same lane), and the "lighting" is painted into the texture: a procedural brass skin of
//! riveted panels, dials and dark seams ([`device_texture`]), checkered so the roll reads clearly.
//! The texture is `Rgba8Unorm`, not sRGB: the engine puts texture bytes on the screen as they are,
//! as it does for BLP albedo. Swapping in a BLP through `WorldAssets` later is a one-line change.

use std::borrow::Cow;
use std::f32::consts::FRAC_PI_4;
use std::time::Duration;

use avian3d::character_controller::move_and_slide::{MoveAndSlideConfig, MoveAndSlideHitResponse};
use avian3d::prelude::Collider;
use benilla_assets::coords::wow_to_bevy;
use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;
use benilla_world::view::ViewDistance;
use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::Dir3;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::{Attached, Ball, BallSpin, BallState, Bumped, Collectible, Phase, RunEntity};

/// The brass core's radius: fixed for a level, a little under the radius it starts at.
pub(super) fn core_radius(start_radius: f32) -> f32 {
    start_radius * 0.85
}

/// Shared seating/render radius: only buried credited volume grows the shell.
pub(super) fn shell_radius(start_radius: f32, buried_volume: f32) -> f32 {
    (core_radius(start_radius).powi(3) + 0.55 * buried_volume).cbrt()
}

/// Angular-speed scale and radius taper: omega = base / sqrt(1 + taper*r).
const TOP_BASE: f32 = 12.0;
const TOP_PER_RADIUS: f32 = 0.65;
/// Shift: a short sprint boost to top speed and acceleration.
const SPRINT: f32 = 1.5;
/// Seconds to reach top speed from rest: `REACH_BASE + REACH_PER_RADIUS * radius` (radius capped at
/// [`REACH_RADIUS_CAP`]). The weight of the ball.
const REACH_BASE: f32 = 0.4;
const REACH_PER_RADIUS: f32 = 0.15;
const REACH_RADIUS_CAP: f32 = 15.0;
/// With no input: the share of the acceleration spent braking, plus a rolling resistance per
/// yard/s of speed. Together a stop from top speed in about a second.
const COAST_BRAKE: f32 = 0.5;
const ROLL_RESISTANCE: f32 = 0.3;
/// Speed lost per second when the input turns the ball (scaled 0 straight on .. 1 reversing).
const TURN_DRAG: f32 = 1.6;
/// Control left in the air.
const AIR_CONTROL: f32 = 0.2;
/// Yards/s² of rolling pull per unit of ground slope (rise over run), so hills matter.
const SLOPE_PULL: f32 = 8.0;
/// Downward acceleration while airborne, yards/s².
const GRAVITY: f32 = 22.0;
/// A landing faster than this bounces, at [`BOUNCE`] of the impact speed.
const BOUNCE_ABOVE: f32 = 5.0;
const BOUNCE: f32 = 0.3;
/// How much of the speed into a wall comes back out.
const WALL_RESTITUTION: f32 = 0.25;
/// A wall hit faster than this is a [`Bumped`] message.
const WALL_BUMP_MIN: f32 = 2.0;
/// The longest a stun runs, seconds; control drops to 20 % at the full stun.
const STUN_MAX: f32 = 0.6;
/// Physics step cap, so a stalled frame does not tunnel the ball.
const MAX_DT: f32 = 0.05;
/// Soft leash: the ball is eased back from [`LEASH_SOFT`] of the leash and held at the leash.
const LEASH_SOFT: f32 = 0.9;

/// Fixed-size reclamation device under the collected debris.
#[derive(Component)]
struct BallCore;

#[derive(Component)]
struct ClumpShell;

/// A pickup's squash: the core swells by this fraction and settles back. Written by
/// [`super::collect`], decayed here.
#[derive(Resource, Default)]
pub(super) struct CorePop(pub f32);

/// Top speed on the flat for a ball of `radius`, yards per second.
pub(super) fn top_speed(radius: f32) -> f32 {
    radius * TOP_BASE / (1.0 + TOP_PER_RADIUS * radius).sqrt()
}

/// Horizontal push/braking, shared by play and the deterministic handling measurements.
#[allow(clippy::too_many_arguments)]
fn traction(
    mut flat: Vec3,
    wish: Vec3,
    top: f32,
    accel: f32,
    grip: f32,
    grounded: bool,
    dt: f32,
) -> Vec3 {
    if wish != Vec3::ZERO {
        // Turning costs speed: the more the input opposes the roll, the more it sheds.
        let speed = flat.length();
        if speed > 0.1 {
            let turn = (1.0 - flat.dot(wish) / speed) * 0.5;
            flat *= (1.0 - TURN_DRAG * turn * dt * grip).max(0.0);
        }
        flat += (wish * top - flat).clamp_length_max(accel * grip * dt);
    } else if grounded {
        let speed = flat.length();
        let slow = (accel * COAST_BRAKE + speed * ROLL_RESISTANCE) * dt;
        flat = if speed > slow {
            flat * ((speed - slow) / speed)
        } else {
            Vec3::ZERO
        };
    }
    flat
}

fn turn_rate(radius: f32, start_radius: f32) -> f32 {
    2.2 / (1.0 + 0.035 * (radius - start_radius).max(0.0))
}

/// Maximum reachable step above the full-radius ground support.
pub(super) fn step_height(radius: f32) -> f32 {
    // At city scale a round clump rolls onto low roofs/terraces; small-scale handling is
    // unchanged. The reachable lift remains below the ball's radius.
    let base = 0.2 + 0.45 * radius;
    let blend = ((radius - 12.) / 8.).clamp(0., 1.);
    base + (radius * 0.95 - base) * blend
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CorePop>()
        .init_resource::<StallTrace>()
        .init_resource::<Shown>()
        .add_systems(OnEnter(Phase::Ready), spawn_ball.after(super::reset_run))
        .add_systems(
            Update,
            (
                drop_stray_buttons.before(drive_ball),
                sync_visuals.after(super::collect::collect),
                present
                    .after(sync_visuals)
                    .after(super::city_support::guard)
                    .after(super::unstuck::Recovery)
                    .before(super::camera::place_camera),
            )
                .in_set(WorldStage::Input),
        );
}

/// Opt-in traversal probe. `WOW_AZEROLL_PATH` is a semicolon-separated list of
/// `x,y,z,goal_x,goal_y,goal_z,radius,seconds` in WoW coordinates. Each leg resets
/// once, then holds W toward its goal (no teleport recovery or autopilot steering).
/// `WOW_AZEROLL_NEAREST=1` instead holds W toward the nearest edible after each leg reset.
#[derive(Default)]
struct PathProbe {
    stage: Option<usize>,
    next_log: f32,
    stalled: f32,
    longest: f32,
    last: Option<Vec3>,
}

#[derive(Resource, Default)]
pub(super) struct StallTrace {
    pub input: bool,
    pub hits: Vec<String>,
    /// For `glitch.rs`: this frame's pre-sweep step lift (yards), the support that settled the
    /// ball (`Support`), the engine floor collider it rested on, and the hardest wall closing.
    pub lift: f32,
    pub support: Support,
    pub floor: Option<Entity>,
    pub closing: f32,
}

/// Which surface held the ball up at the end of `drive_ball`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Support {
    #[default]
    Air,
    Terrain,
    Floor,
    Prop,
}

#[derive(bevy::ecs::system::SystemParam)]
#[allow(clippy::type_complexity)] // Disjoint loose-prop query.
pub(super) struct ProbeParams<'w, 's> {
    commands: Commands<'w, 's>,
    probe_stage: Local<'s, Option<usize>>,
    path: Local<'s, PathProbe>,
    trace: ResMut<'w, StallTrace>,
    fresh_solids: Query<'w, 's, (), (Added<Collectible>, Without<Attached>)>,
    solids: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Collectible,
            Has<super::design::Gate>,
        ),
        (
            Without<Attached>,
            Without<super::collect::Shed>,
            Without<Ball>,
            Without<BallSpin>,
        ),
    >,
    loose: Query<
        'w,
        's,
        (&'static Transform, &'static Collectible),
        (Without<Attached>, Without<Ball>, Without<BallSpin>),
    >,
}

/// The ball at the start: [`Ball`] (translation only) -> [`BallSpin`] (the roll) -> the core mesh.
/// Every trophy later becomes another child of the spin frame.
fn spawn_ball(
    mut commands: Commands,
    ball: Res<BallState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().uv(24, 12));
    let material = materials.add(StandardMaterial {
        // Toned down so the trophies, not the brass, read first.
        base_color: Color::srgb(0.78, 0.72, 0.66),
        base_color_texture: Some(images.add(device_texture())),
        unlit: true,
        ..default()
    });
    let root = commands
        .spawn((
            Ball,
            Transform::from_translation(ball.pos),
            Visibility::default(),
            RunEntity,
        ))
        .id();
    let spin = commands
        .spawn((
            BallSpin,
            Transform::from_rotation(ball.spin),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    // Coarse packed debris: angular silhouette and mottled, shadowed wood/stone/cloth colors.
    let mut shell_mesh = Sphere::new(1.0).mesh().uv(24, 12);
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
        shell_mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for point in points {
            let n = Vec3::from(*point);
            let noise = (n.x * 19.0 + n.z * 11.0).sin() * (n.y * 23.0 - n.x * 7.0).cos();
            *point = (n * (1.0 + 0.22 * noise)).to_array();
        }
    }
    shell_mesh.compute_normals();
    let shell_mat = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(debris_texture())),
        unlit: true,
        ..default()
    });
    commands.spawn((
        ClumpShell,
        Mesh3d(meshes.add(shell_mesh)),
        MeshMaterial3d(shell_mat),
        Transform::from_scale(Vec3::splat(shell_radius(
            ball.start_radius,
            ball.buried_volume,
        ))),
        Visibility::Hidden,
        ChildOf(spin),
    ));
    commands.spawn((
        BallCore,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_scale(Vec3::splat(core_radius(ball.start_radius))),
        Visibility::default(),
        ChildOf(spin),
    ));
}

/// The ball's skin: brass panels in a checker, riveted corners, a dial on every other panel and dark
/// seams, with a little grain. The grid is 12 x 6 square panels, so the texture tiles around the
/// equator without a seam.
fn device_texture() -> Image {
    const W: u32 = 512;
    const H: u32 = 256;
    const CELLS_U: f32 = 12.0;
    const CELLS_V: f32 = 6.0;
    let (cw, ch) = (W as f32 / CELLS_U, H as f32 / CELLS_V);
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            let (iu, iv) = ((fx / cw) as u32, (fy / ch) as u32);
            // Position inside the panel, pixels from its corner.
            let (px, py) = (fx - iu as f32 * cw, fy - iv as f32 * ch);
            let dial = (iu + iv) % 2 == 0;
            let mut rgb = if dial {
                [206.0, 154.0, 62.0]
            } else {
                [178.0, 128.0, 46.0]
            };
            if dial {
                let r = (px - cw * 0.5).hypot(py - ch * 0.5);
                if (10.0..13.0).contains(&r) {
                    rgb = [92.0, 62.0, 24.0];
                } else if r < 4.0 {
                    rgb = [236.0, 200.0, 112.0];
                }
            }
            // Rivets, inset from each corner of the panel.
            for (rx, ry) in [
                (7.0, 7.0),
                (cw - 7.0, 7.0),
                (7.0, ch - 7.0),
                (cw - 7.0, ch - 7.0),
            ] {
                let d = (px - rx).hypot(py - ry);
                if d < 3.0 {
                    rgb = [242.0, 214.0, 138.0];
                } else if d < 4.4 {
                    rgb = [70.0, 46.0, 18.0];
                }
            }
            // Seams between panels.
            let edge = px.min(cw - px).min(py.min(ch - py));
            if edge < 2.2 {
                rgb = [58.0, 38.0, 16.0];
            } else if edge < 4.0 {
                rgb = [rgb[0] * 0.8, rgb[1] * 0.8, rgb[2] * 0.8];
            }
            let grain = 1.0 + (hash01(x / 3, y / 3) - 0.5) * 0.12 + (hash01(x, y) - 0.5) * 0.05;
            for c in rgb {
                data.push((c * grain).clamp(0.0, 255.0) as u8);
            }
            data.push(255);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// Procedural packed fragments; generated locally, no game assets are copied into the repo.
fn debris_texture() -> Image {
    let mut image = device_texture();
    let mut data = Vec::with_capacity(512 * 256 * 4);
    let palette = [
        [52.0, 44.0, 31.0],
        [69.0, 57.0, 39.0],
        [45.0, 51.0, 44.0],
        [74.0, 66.0, 54.0],
        [40.0, 35.0, 29.0],
        [61.0, 44.0, 33.0],
    ];
    for y in 0..256u32 {
        for x in 0..512u32 {
            let row = y / 64;
            let xx = x + (hash01(row, 77) * 29.0) as u32;
            let cell = xx / 64;
            let rgb = palette[(hash01(cell, row) * 5.99) as usize];
            let edge = (xx % 64).min(63 - xx % 64).min((y % 64).min(63 - y % 64));
            let shade = if edge < 2 {
                0.45
            } else {
                0.7 + hash01(x, y) * 0.5
            };
            for c in rgb {
                data.push((c * shade) as u8);
            }
            data.push(255);
        }
    }
    image.data = Some(data);
    image
}

/// A cheap integer hash to `[0, 1]`, for the grain.
fn hash01(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Where a ball of `radius` centred over `at` comes to rest on the terrain heightfield, and the
/// ground's slope there (rise over run, as world x and z). The ball rests on the highest of the
/// heights around it, so a big ball rides over a bump instead of sinking into the hill beside it.
/// A WMO floor also supports the ball over an ADT hole. `None` only without either surface.
fn footing(
    world: &WorldPoint<'_, '_>,
    collision: &WorldCollision<'_, '_>,
    at: Vec3,
    radius: f32,
) -> Option<(f32, Vec2)> {
    // WMO interiors deliberately cut holes in the ADT. A loaded building floor is
    // sufficient support there; requiring terrain creates an invisible cell-edge wall.
    let Some(centre) = world.terrain_height_under(at) else {
        return floor_rest(collision, at, radius).map(|rest| (rest, Vec2::ZERO));
    };
    // The ADT continues above underground WMO rooms. It is not a floor for a
    // body below it: prefer nearby walking geometry, or wait for it to stream.
    if centre > at.y + 0.1 {
        return floor_rest(collision, at, radius).map(|h| (h, Vec2::ZERO));
    }
    let mut rest = centre + radius;
    let mut gradient = Vec2::ZERO;
    for (ring, fraction) in [0.45_f32, 0.9].into_iter().enumerate() {
        let d = radius * fraction;
        let lift = (radius * radius - d * d).max(0.0).sqrt();
        for i in 0..8 {
            let a = i as f32 * FRAC_PI_4;
            let dir = Vec2::new(a.cos(), a.sin());
            let p = at + Vec3::new(dir.x * d, 0.0, dir.y * d);
            let h = world.terrain_height_under(p).unwrap_or(centre);
            rest = rest.max(h + lift);
            if ring == 1 {
                gradient += dir * (h - centre);
            }
        }
    }
    // Eight samples at distance d: sum(h_i * dir_i) = slope * d * 8 / 2.
    Some((rest, gradient / (radius * 0.9 * 4.0)))
}

/// Full-radius sphere support from walkable collision surfaces across its footprint.
pub(super) fn floor_rest(world: &WorldCollision<'_, '_>, centre: Vec3, radius: f32) -> Option<f32> {
    floor_support(world, centre, radius).map(|(h, _)| h)
}

/// [`floor_rest`] and the collider that supplies it.
pub(super) fn floor_support(
    world: &WorldCollision<'_, '_>,
    centre: Vec3,
    radius: f32,
) -> Option<(f32, Entity)> {
    let _profile = super::profile::span("ball::floor_support");
    // Cast from the reachable step height above the bottom, not above the head:
    // an upper storey/ceiling must never pull the ball up through the inn.
    let origin_y = centre.y - radius + step_height(radius) + 0.05;
    ring_support(radius, |offset| {
        let origin = Vec3::new(centre.x + offset.x, origin_y, centre.z + offset.y);
        world
            .ray_body(origin, Dir3::NEG_Y, 4.0 + 2.0 * radius)
            .filter(|hit| hit.normal.y > 0.6)
            .map(|hit| (origin.y - hit.distance, hit.normal, hit.entity))
    })
}

/// The D6 footprint: a centre ray and rings at 0.3, 0.5, 0.7, 0.85, 0.95 and 0.99 r along 12
/// spokes, each walkable hit lifted by the sphere's cap at its offset. Fixed rings alone climb an
/// edge as a staircase (the support only rises when the next ring crosses it, up to 0.2 r in one
/// frame), so where a spoke's sample stands higher than the plane of the sample inside it
/// predicts, an edge lies between them and four bisection samples find it: the centre then
/// follows the sphere's own contact circle over steps, curbs and box tops. `sample(offset)`
/// returns a walkable surface's height, normal and owner under `offset` (world x, z).
pub(super) fn ring_support<T: Copy>(
    radius: f32,
    mut sample: impl FnMut(Vec2) -> Option<(f32, Vec3, T)>,
) -> Option<(f32, T)> {
    const RINGS: [f32; 6] = [0.3, 0.5, 0.7, 0.85, 0.95, 0.99];
    fn keep<T>(rest: &mut Option<(f32, T)>, height: f32, owner: T) {
        if rest.as_ref().is_none_or(|(h, _)| height > *h) {
            *rest = Some((height, owner));
        }
    }
    let lift = |d: f32| (radius * radius - d * d).max(0.0).sqrt();
    let tolerance = 0.02 + 0.01 * radius;
    let mut rest = None;
    let centre = sample(Vec2::ZERO);
    if let Some((y, _, owner)) = centre {
        keep(&mut rest, y + radius, owner);
    }
    for i in 0..12 {
        let angle = i as f32 * std::f32::consts::TAU / 12.0;
        let dir = Vec2::new(angle.cos(), angle.sin());
        // The height a walkable plane through (`from`, `y`) reaches at `d` along this spoke.
        let along =
            |y: f32, n: Vec3, from: f32, d: f32| y - (n.x * dir.x + n.z * dir.y) / n.y * (d - from);
        let mut inner = (0.0, centre.map(|(y, n, _)| (y, n)));
        for fraction in RINGS {
            let d = radius * fraction;
            let hit = sample(dir * d);
            if let Some((y, n, owner)) = hit {
                keep(&mut rest, y + lift(d), owner);
                let edge = inner
                    .1
                    .is_none_or(|(iy, inormal)| y > along(iy, inormal, inner.0, d) + tolerance);
                // Whatever the bisection finds lies on this surface between the two rings, so
                // it cannot beat `y + lift(inner)`: skip edges that cannot raise the support.
                let useful = rest
                    .as_ref()
                    .is_none_or(|(h, _)| y + tolerance + lift(inner.0) > *h);
                if edge && useful {
                    let (mut lo, mut hi) = (inner.0, d);
                    for _ in 0..4 {
                        let mid = 0.5 * (lo + hi);
                        match sample(dir * mid) {
                            Some((my, _, mo)) if (my - along(y, n, d, mid)).abs() <= tolerance => {
                                hi = mid;
                                keep(&mut rest, my + lift(mid), mo);
                            }
                            _ => lo = mid,
                        }
                    }
                }
            }
            inner = (d, hit.map(|(y, n, _)| (y, n)));
        }
    }
    rest
}

fn ready_height(current: f32, authored_feet: f32, radius: f32) -> f32 {
    current.max(authored_feet + radius)
}

/// Landing integration also releases a grounded ball when its platform disappears.
pub(super) fn settle_vertical(
    mut y: f32,
    mut vy: f32,
    mut grounded: bool,
    rest: f32,
    snap: f32,
    dt: f32,
) -> (f32, f32, bool) {
    let mut landed = false;
    if y - rest <= snap && vy <= 0. {
        landed = !grounded;
        y = rest;
        grounded = true;
        if !landed {
            vy = 0.;
        }
    } else {
        vy -= GRAVITY * dt;
        y += vy * dt;
        grounded = false;
        if y <= rest {
            y = rest;
            landed = true;
            grounded = true;
        }
    }
    if landed {
        let impact = -vy;
        vy = if impact > BOUNCE_ABOVE {
            impact * BOUNCE
        } else {
            0.
        };
        grounded = vy <= 0.;
    }
    (y, vy, grounded)
}

/// The sphere the slide sweeps, rebuilt only when the ball has grown a percent.
fn sweep_shape(cache: &mut Option<(f32, Collider)>, radius: f32) -> &Collider {
    let stale = cache
        .as_ref()
        .is_none_or(|(r, _)| (r - radius).abs() > 0.01 * radius);
    if stale {
        *cache = Some((radius, Collider::sphere(radius)));
    }
    &cache.as_ref().expect("filled just above").1
}

/// Put the ball's root and spin frame where [`BallState`] says.
fn write_back(
    ball: &BallState,
    roots: &mut Query<&mut Transform, (With<Ball>, Without<BallSpin>)>,
    spins: &mut Query<&mut Transform, (With<BallSpin>, Without<Ball>)>,
) {
    if let Ok(mut t) = roots.single_mut() {
        if t.translation != ball.pos {
            t.translation = ball.pos;
        }
    }
    if let Ok(mut t) = spins.single_mut() {
        if t.rotation != ball.spin {
            t.rotation = ball.spin;
        }
    }
}

/// W/S roll, A/D turn the heading, Space reverses, Shift dashes. Runs in every phase (the
/// ball coasts to a stop on the results screen, and sits on the terrain while the world loads), but
/// takes input only while playing.
#[allow(clippy::type_complexity)] // Disjoint Bevy transform queries.
pub(super) fn drive_ball(
    time: Res<Time>,
    design: Res<super::design::Design>,
    run: Res<super::Run>,
    probe: ProbeParams,
    input: (
        Res<ButtonInput<KeyCode>>,
        Res<super::pad::PadInput>,
        ResMut<super::pad::DashCharge>,
    ),
    phase: Res<State<Phase>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    clump: (
        Res<super::lod::Clump>,
        Query<(Entity, &Transform, &Attached, &Collectible), (Without<Ball>, Without<BallSpin>)>,
    ),
    world: WorldPoint,
    collision: WorldCollision,
    mut ball: ResMut<BallState>,
    mut shape_cache: Local<Option<(f32, Collider)>>,
    events: (MessageWriter<Bumped>, MessageWriter<super::Dash>),
    view: Option<ResMut<ViewDistance>>,
    mut roots: Query<&mut Transform, (With<Ball>, Without<BallSpin>)>,
    mut spins: Query<&mut Transform, (With<BallSpin>, Without<Ball>)>,
) {
    let _profile = super::profile::span("ball::drive_ball");
    let (clump, trophies) = clump;
    let (keys, pad, mut charge) = input;
    let (mut bumped, mut dash_events) = events;
    let ProbeParams {
        mut commands,
        mut probe_stage,
        mut path,
        mut trace,
        loose,
        solids,
        fresh_solids,
    } = probe;
    let support = |pos, r| {
        let engine = floor_rest(&collision, pos, r);
        solids
            .iter()
            .filter_map(|(tf, c, gate)| super::collect::solid_rest(tf, c, pos, r, gate))
            .chain(engine)
            .max_by(f32::total_cmp)
    };
    let live = *phase.get() == Phase::Playing && !loading.covering();
    let ready = matches!(phase.get(), Phase::Ready | Phase::Select);
    let dt = time.delta_secs().min(MAX_DT);
    let radius = ball.radius;
    let mut reset_clump = false;

    // The far clip grows with the ball: the camera backs off and sees further.
    if let Some(mut view) = view {
        let want = (350.0 + radius * 25.0).clamp(350.0, 777.0);
        if (view.farclip - want).abs() > 8.0 {
            view.farclip = want;
        }
    }

    // Recipes author FEET, BallState stores the CENTRE. At r>.4545 the
    // reachable-step ray of a feet-height centre starts below the very floor
    // whose arrival Ready waits for, permanently closing the floor_ready gate.
    if ready && design.start_indoors && !ball.floor_ready {
        ball.pos.y = ready_height(ball.pos.y, design.start[2], radius);
    }
    if ready || !fresh_solids.is_empty() {
        let from = ball.pos;
        let pos = super::collect::resolve_solid_overlaps(from, radius, solids.iter());
        if pos.distance_squared(from) > 1e-6 {
            info!("azeroll solid overlap resolved on spawn/Ready: {from:?} -> {pos:?} r={radius}");
            ball.pos = pos;
            ball.vel = Vec3::ZERO;
        }
    }
    // Until the terrain under the ball is loaded, hold it where it is.
    let Some((rest0, slope)) = footing(&world, &collision, ball.pos, radius) else {
        ball.vel = Vec3::ZERO;
        ball.grounded = false;
        write_back(&ball, &mut roots, &mut spins);
        return;
    };

    let mut pos = ball.pos;
    let mut vel = ball.vel;
    let mut grounded = ball.grounded;
    let start = design.leash_centre();
    let leash = design.leash_radius();

    if ready {
        // Waiting at the start: sit on whatever the ground turns out to be.
        vel = Vec3::ZERO;
        let floor = support(pos, radius);
        // An indoor start is supported once the building's floor (not the terrain under it, which
        // the support rays also hit) has loaded: a floor at the authored height.
        let on_floor = design.start_indoors && floor.is_some_and(|h| h >= design.start[2]);

        if on_floor || !design.start_indoors {
            ball.floor_ready = true;
        }
        pos.y = match floor {
            Some(h) if on_floor || !design.start_indoors => h.max(rest0),
            // Until then hold the authored height: settling on the terrain would put the ball
            // under the floor, out of the support rays' reach for good.
            _ if design.start_indoors => pos.y.max(design.start[2]),
            Some(h) => h.max(rest0),
            None => rest0,
        };
        grounded = true;
    } else if dt > 0.0 {
        ball.stun = (ball.stun - dt).max(0.0);

        // Tank inputs: lateral keys turn the heading rather than strafing. The pad adds its own
        // roll, turn and (both sticks sideways) strafe on top.
        let mut input = Vec2::ZERO;
        let mut strafe = 0.0;
        if live {
            if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
                input.y += 1.0;
            }
            if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
                input.y -= 1.0;
            }
            if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
                input.x += 1.0;
            }
            if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
                input.x -= 1.0;
            }
            input = (input + Vec2::new(pad.turn, pad.forward)).clamp(Vec2::NEG_ONE, Vec2::ONE);
            strafe = pad.strafe;
        }
        // Deterministic entrance regression route; normal play never enters this branch.
        if live && crate::webenv::var("WOW_AZEROLL_FLOOR_TEST").is_some() {
            let stage = ((run.elapsed / 18.0) as usize).min(5);
            // `WOW_AZEROLL_FLOOR_RADII=a,b,c` swaps the three probe radii (small balls first).
            let radii = crate::webenv::var("WOW_AZEROLL_FLOOR_RADII")
                .and_then(|v| {
                    let r: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    <[f32; 3]>::try_from(r).ok()
                })
                .unwrap_or([0.5, 1.0, 1.8]);
            let r = radii[stage % 3];
            let inn = stage < 3;
            let start = if inn {
                [-9459.0, 48.0, 57.1]
            } else {
                [-9459.8, 77.0, 57.0]
            };
            let goal = wow_to_bevy(if inn {
                [-9460.0, 32.0, 57.0]
            } else {
                [-9460.0, 94.0, 57.0]
            });
            if *probe_stage != Some(stage) {
                *probe_stage = Some(stage);
                ball.radius = r;
                ball.volume = r.powi(3);
                pos = wow_to_bevy(start) + Vec3::Y * r;
                vel = Vec3::ZERO;
                info!("azeroll floor-test: stage={stage} inn={inn} radius={r}");
            }
            ball.radius = r;
            ball.volume = r.powi(3);
            let delta = Vec3::new(goal.x - pos.x, 0.0, goal.z - pos.z);
            ball.heading = (-delta.x).atan2(-delta.z);
            input = Vec2::new(0.0, if delta.length() > 0.5 { 0.3 } else { 0.0 });
        }
        if live {
            if let Some(spec) = crate::webenv::var("WOW_AZEROLL_PATH") {
                let legs: Vec<Vec<f32>> = spec
                    .split(';')
                    .map(|leg| {
                        leg.split(',')
                            .filter_map(|v| v.trim().parse().ok())
                            .collect()
                    })
                    .filter(|v: &Vec<f32>| {
                        v.len() == 8 && v.iter().all(|x| x.is_finite()) && v[6] > 0.0 && v[7] > 0.0
                    })
                    .collect();
                let mut elapsed = run.elapsed;
                for (stage, leg) in legs.iter().enumerate() {
                    if elapsed >= leg[7] {
                        elapsed -= leg[7];
                        continue;
                    }
                    if path.stage != Some(stage) {
                        info!(
                            "azeroll path: end stage={:?} longest_stall={:.2}",
                            path.stage, path.longest
                        );
                        path.stage = Some(stage);
                        for (entity, _, _, _) in &trophies {
                            commands.entity(entity).despawn();
                        }
                        ball.buried_volume = 0.0;
                        reset_clump = true;
                        path.longest = 0.0;
                        path.stalled = 0.0;
                        path.last = None;
                        pos = wow_to_bevy([leg[0], leg[1], leg[2]]) + Vec3::Y * leg[6];
                        vel = Vec3::ZERO;
                        ball.stun = 0.0;
                        info!("azeroll path: begin stage={stage} leg={leg:?}");
                    }
                    ball.radius = leg[6];
                    ball.volume = leg[6].powi(3);
                    let mut goal = wow_to_bevy([leg[3], leg[4], leg[5]]);
                    if crate::webenv::var("WOW_AZEROLL_NEAREST").is_some() {
                        if let Some((_, at)) = loose
                            .iter()
                            .filter(|(_, c)| {
                                super::collect::contact(ball.radius, c.pickup)
                                    == super::collect::Contact::RollUp
                            })
                            .map(|(tf, c)| (c, tf.transform_point(c.center)))
                            .min_by(|a, b| {
                                a.1.distance_squared(pos)
                                    .total_cmp(&b.1.distance_squared(pos))
                            })
                        {
                            goal = at;
                        }
                    }
                    let delta = super::units::flat(goal - pos);
                    ball.heading = (-delta.x).atan2(-delta.z);
                    input = Vec2::new(0.0, if delta.length() > 0.15 { 1.0 } else { 0.0 });
                    if input.y != 0.0
                        && path
                            .last
                            .is_some_and(|last| super::units::flat(pos - last).length() < 0.15 * dt)
                    {
                        path.stalled += dt;
                        path.longest = path.longest.max(path.stalled);
                    } else {
                        path.stalled = 0.0;
                    }
                    path.last = Some(pos);
                    if run.elapsed >= path.next_log {
                        path.next_log = run.elapsed + 1.0;
                        info!(
                            "azeroll path: stage={stage} t={elapsed:.1} r={:.2} pos={pos:?} goal={goal:?} distance={:.2} speed={:.2} stalled={:.2} longest={:.2}",
                            ball.radius,
                            delta.length(),
                            super::units::flat(vel).length(),
                            path.stalled,
                            path.longest
                        );
                    }
                    break;
                }
            }
        }
        trace.input = input.y != 0.0 || strafe != 0.0;
        trace.hits.clear();
        trace.lift = 0.0;
        trace.closing = 0.0;
        let debug = crate::webenv::var("WOW_AZEROLL_STALL_DEBUG").is_some();
        let radius = ball.radius;
        let turn_rate = turn_rate(radius, ball.start_radius);
        ball.heading -= input.x * turn_rate * dt;
        if live && (keys.just_pressed(KeyCode::Space) || pad.quick_turn) {
            ball.heading += std::f32::consts::PI;
        }
        ball.heading = ball.heading.rem_euclid(std::f32::consts::TAU);
        let (sy, cy) = ball.heading.sin_cos();
        let ahead = Vec3::new(-sy, 0.0, -cy);
        let mut wish = (ahead * input.y
            + Vec3::new(cy, 0.0, -sy) * strafe * super::pad::STRAFE_SCALE)
            .clamp_length_max(1.0);
        let was_dashing = ball.dash > 0.0;
        let was_cooling = ball.dash_cooldown > 0.0;
        ball.dash = (ball.dash - dt).max(0.0);
        ball.dash_cooldown = (ball.dash_cooldown - dt).max(0.0);
        if was_dashing && ball.dash == 0.0 {
            dash_events.write(super::Dash::Ended);
        }
        if was_cooling && ball.dash_cooldown == 0.0 {
            dash_events.write(super::Dash::Ready);
        }
        // The dash is wound up: press (Shift, the pad's A, or the sticks waggled) arms it, a hold
        // past a tap scrambles the Prince and holds the ball back, and release or the full
        // 0.45 s bursts it, longer and harder for the charge. A tap is the dash it always was.
        let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let charged = charge.step(
            dt,
            live && (shift || pad.dash_held),
            ball.dash_cooldown == 0.0 && ball.dash == 0.0 && (wish != Vec3::ZERO || pad.dash_held),
            live,
        );
        if charged.windup {
            dash_events.write(super::Dash::WindUp);
        }
        if let Some(c) = charged.burst {
            ball.dash = 0.65 * (1.0 + super::pad::BURST_LONGER * c);
            ball.dash_cooldown = 1.8;
            charge.boost = c;
            charge.forced = wish == Vec3::ZERO;
            dash_events.write(super::Dash::Started);
        }
        let sprint = live && ball.dash > 0.0;
        if !sprint {
            charge.forced = false;
        } else if charge.forced && wish == Vec3::ZERO {
            // A burst with no stick pushed (the waggle, the A button) rolls straight ahead.
            wish = ahead;
        }

        let moment = clump.moment;
        let com = if reset_clump {
            Vec3::ZERO
        } else {
            ball.spin * moment / ball.volume.max(0.001)
        };
        let eccentricity = (com.length() / radius).min(0.3);
        let inertia = 1.0 + 2.0 * eccentricity;
        let limp = (com.y / radius).clamp(-0.25, 0.25);
        let full = top_speed(radius) * (1.0 - 0.3 * limp) * if sprint { SPRINT } else { 1.0 };
        // Winding up holds the ball back; the pull it brakes and builds with is the unheld one's.
        let top = if charge.winding() {
            full * super::pad::WINDUP_SLOW
        } else {
            full
        };
        let reach = REACH_BASE + REACH_PER_RADIUS * radius.min(REACH_RADIUS_CAP);
        let accel = full / (reach * inertia)
            * if sprint {
                1.3 * (1.0 + super::pad::BURST_PULL * charge.boost)
            } else {
                1.0
            };
        let stun_scale = 1.0 - 0.8 * (ball.stun / STUN_MAX).clamp(0.0, 1.0);
        let grip = if grounded { 1.0 } else { AIR_CONTROL } * stun_scale;

        let mut flat = Vec3::new(vel.x, 0.0, vel.z);
        flat = traction(flat, wish, top, accel, grip, grounded, dt);
        if grounded {
            flat -= Vec3::new(slope.x, 0.0, slope.y) * SLOPE_PULL * dt;
        }
        flat += Vec3::new(com.z, 0.0, -com.x) * (eccentricity * dt);
        flat = flat.clamp_length_max(top_speed(radius) * SPRINT * 1.3);

        // The slide: the sphere, lifted over small steps, sweeps the horizontal move against
        // buildings and engine doodads. The terrain's own mesh is in its way too; those hits are
        // slopes and slide, and only wall-like contacts count as a bump. The height afterwards comes
        // from the heightfield, so the slide's `y` is discarded.
        let mut wall: Option<Vec3> = None;
        let mut closing = 0.0_f32;
        if flat.length_squared() > 1.0e-6 {
            let ahead = pos + flat * dt;
            if grounded {
                if let Some(rest) = support(ahead, radius) {
                    if rest > pos.y && rest - pos.y <= step_height(radius) {
                        trace.lift = rest - pos.y;
                        pos.y = rest;
                    }
                }
            }
            let from = pos;
            let out = collision.slide_body(
                sweep_shape(&mut shape_cache, radius * 0.85),
                from,
                flat,
                Duration::from_secs_f32(dt),
                &MoveAndSlideConfig::default(),
                |hit| {
                    let n: Vec3 = **hit.normal;
                    if debug && trace.hits.len() < 8 {
                        trace
                            .hits
                            .push(format!("normal={n:?} velocity={:?}", hit.velocity));
                    }
                    // Ride walkable contacts instead of projecting horizontal intent to zero
                    // while already touching the terrain. Height is settled by footing below.
                    if n.y > 0.6 {
                        let horizontal = Vec3::new(hit.velocity.x, 0.0, hit.velocity.z);
                        hit.velocity.y = -horizontal.dot(n) / n.y;
                    }
                    if n.y.abs() < 0.6 {
                        let into = -hit.velocity.dot(n);
                        if into > closing {
                            closing = into;
                            wall = Some(n);
                        }
                    }
                    MoveAndSlideHitResponse::Accept
                },
            );
            if live
                && (crate::webenv::var("WOW_AZEROLL_FLOOR_TEST").is_some()
                    || crate::webenv::var("WOW_AZEROLL_SLIDE_DEBUG").is_some())
                && run.elapsed.fract() < dt
            {
                let ahead_rest = support(ahead, radius);
                info!(
                    "azeroll floor-test: slide from={from:?} to={:?} flat={flat:?} wall={wall:?} \
                     closing={closing:.2} ahead_rest={ahead_rest:?} y={:.3} terrain_rest={:?} \
                     floor_rest={:?}",
                    out.position,
                    pos.y,
                    footing(&world, &collision, pos, radius).map(|f| f.0),
                    support(pos, radius)
                );
            }
            pos.x = out.position.x;
            pos.z = out.position.z;
            flat = match wall {
                Some(n) => {
                    // Slide along it, and rebound a little.
                    let n = Vec3::new(n.x, 0.0, n.z).normalize_or_zero();
                    let slid = Vec3::new(out.projected_velocity.x, 0.0, out.projected_velocity.z);
                    slid + n * closing * WALL_RESTITUTION
                }
                None => flat,
            };
        }
        trace.closing = closing;
        if closing > WALL_BUMP_MIN {
            bumped.write(Bumped {
                entity: None,
                name: Cow::Borrowed("wall"),
                strength: closing,
            });
            ball.stun = ball
                .stun
                .max(((closing - WALL_BUMP_MIN) * 0.04).min(STUN_MAX * 0.5));
        }

        // The soft leash around the start.
        let off = Vec3::new(pos.x - start.x, 0.0, pos.z - start.z);
        let dist = off.length();
        if dist > leash * LEASH_SOFT {
            let n = off / dist;
            let outward = flat.dot(n);
            if outward > 0.0 {
                let held =
                    ((dist - leash * LEASH_SOFT) / (leash * (1.0 - LEASH_SOFT))).clamp(0.0, 1.0);
                flat -= n * outward * held;
            }
            if dist > leash {
                pos.x = start.x + n.x * leash;
                pos.z = start.z + n.z * leash;
            }
        }

        // Roll: the top of the ball moves with the velocity, the bottom stays on the ground.
        let speed = flat.length();
        if speed > 0.01 {
            let axis = Vec3::Y.cross(flat).normalize();
            ball.spin = (Quat::from_axis_angle(axis, speed * dt / radius * (1.0 - limp) / inertia)
                * ball.spin)
                .normalize();
        }

        // Settle on the ground at the new spot: the heightfield, or a floor above it.
        match footing(&world, &collision, pos, radius) {
            Some((terrain_rest, _)) => {
                let mut rest = terrain_rest;
                trace.support = Support::Terrain;
                trace.floor = None;
                let top = solids
                    .iter()
                    .filter_map(|(tf, c, gate)| {
                        super::collect::solid_rest(tf, c, pos, radius, gate)
                    })
                    .max_by(f32::total_cmp);
                let floor = floor_support(&collision, pos, radius);
                if let Some((h, entity)) = floor.filter(|(h, _)| *h >= rest) {
                    rest = h;
                    trace.support = Support::Floor;
                    trace.floor = Some(entity);
                }
                if let Some(h) = top.filter(|h| *h > rest) {
                    rest = h;
                    trace.support = Support::Prop;
                    trace.floor = None;
                }
                // Keep the support sphere round. Eccentric attachments drive roll inertia
                // and a bounded visual limp; they cannot lever the collider through a floor.
                let snap = 0.05 + 0.02 * radius + 0.8 * flat.length() * dt;
                (pos.y, vel.y, grounded) = settle_vertical(pos.y, vel.y, grounded, rest, snap, dt);
                if !grounded {
                    trace.support = Support::Air;
                }
            }
            None => {
                // Rolled off the loaded terrain: stay put.
                pos.x = ball.pos.x;
                pos.z = ball.pos.z;
                flat = Vec3::ZERO;
            }
        }
        vel = Vec3::new(flat.x, vel.y, flat.z);
    } else {
        // Paused: no time passes, but stay on the terrain.
        if grounded {
            pos.y = support(pos, radius).map_or(rest0, |h| h.max(rest0));
        }
    }
    if live && crate::webenv::var("WOW_AZEROLL_FLOOR_TEST").is_some() && run.elapsed.fract() < dt {
        let floor = collision
            .ray_body(pos, Dir3::NEG_Y, ball.radius + 2.0)
            .filter(|h| h.normal.y > 0.6)
            .map(|h| pos.y - h.distance);
        info!(
            "azeroll floor-test: t={:.1} r={:.2} pos={pos:?} centre_floor={floor:?} bottom={:.3}",
            run.elapsed,
            ball.radius,
            pos.y - ball.radius
        );
    }
    ball.pos = pos;
    ball.vel = vel;
    ball.grounded = grounded;
    write_back(&ball, &mut roots, &mut spins);
}

/// After pickups: update root, growing debris shell, and the fixed core's temporary pickup pop.
#[allow(clippy::type_complexity)] // Disjoint root, core and shell transforms.
fn sync_visuals(
    time: Res<Time>,
    ball: Res<BallState>,
    mut pop: ResMut<CorePop>,
    mut roots: Query<&mut Transform, (With<Ball>, Without<BallCore>, Without<ClumpShell>)>,
    mut cores: Query<&mut Transform, (With<BallCore>, Without<Ball>, Without<ClumpShell>)>,
    mut shells: Query<
        (&mut Transform, &mut Visibility),
        (With<ClumpShell>, Without<Ball>, Without<BallCore>),
    >,
) {
    for (mut tf, mut vis) in &mut shells {
        tf.scale = Vec3::splat(shell_radius(ball.start_radius, ball.buried_volume));
        *vis = if ball.buried_volume > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if pop.0 > 0.0 {
        pop.0 *= (-12.0 * time.delta_secs()).exp();
        if pop.0 < 0.001 {
            pop.0 = 0.0;
        }
    }
    if let Ok(mut t) = roots.single_mut() {
        if t.translation != ball.pos {
            t.translation = ball.pos;
        }
    }
    if let Ok(mut t) = cores.single_mut() {
        let scale = Vec3::splat(core_radius(ball.start_radius) * (1.0 + pop.0));
        if t.scale != scale {
            t.scale = scale;
        }
    }
}

/// Render-only vertical blend. The physical centre may rise onto a step, settle onto a deck or
/// snap down a curb in one frame; the clump that is drawn and the camera that frames it ease the
/// jump's excess out instead (`Shown::offset` below the physics, decaying at [`SHOWN_RATE`]).
/// Collection, support and every planner keep reading [`BallState::pos`].
#[derive(Resource, Default)]
pub(super) struct Shown {
    /// Drawn centre minus physical centre, yards (vertical).
    pub offset: f32,
    /// The continuous part of the last frame's vertical motion, yards.
    carried: f32,
    last: Option<(Vec3, bool)>,
    last_dt: f32,
}

/// Per second: about 95 % of a pop is gone in a quarter second.
const SHOWN_RATE: f32 = 12.0;

impl Shown {
    /// Absorb this frame's vertical jerk beyond a small tolerance. A grounded ball's pop up (a
    /// step, a settle) or snap down (a curb, a building's floor settle) is eased; so is an
    /// airborne ball moved faster than either its own fall or its ballistic step `vy·dt` (the
    /// surface rising under it off a roof's edge, a settle mid-fall). Falls, landings and bounces
    /// are left as they are, and a teleport (Unstuck, a restart) resets the blend because the
    /// camera cuts there anyway.
    pub(super) fn step(&mut self, pos: Vec3, vy: f32, radius: f32, grounded: bool, dt: f32) -> f32 {
        let Some((last, was_grounded)) = self.last.replace((pos, grounded)) else {
            *self = Self {
                last: Some((pos, grounded)),
                ..default()
            };
            return 0.0;
        };
        if dt <= 0.0 {
            self.last = Some((last, was_grounded));
            return self.offset;
        }
        let flat = Vec2::new(pos.x - last.x, pos.z - last.z).length();
        if flat > top_speed(radius) * SPRINT * 2.6 * dt + radius {
            *self = Self {
                last: Some((pos, grounded)),
                ..default()
            };
            return 0.0;
        }
        let dy = pos.y - last.y;
        // Without a previous frame's motion there is nothing to call a jump.
        let predicted = if self.last_dt > 0.0 {
            self.carried * dt / self.last_dt
        } else {
            dy
        };
        let tolerance = (0.02 * radius + 0.01) * dt * 60.0;
        let (high, low) = if was_grounded {
            (dy - predicted, dy - predicted)
        } else {
            let ballistic = vy * dt;
            (dy - predicted.max(ballistic), dy - predicted.min(ballistic))
        };
        let absorbed = if high > tolerance {
            high - tolerance
        } else if low < -tolerance {
            low + tolerance
        } else {
            0.0
        };
        // A landing ends the fall: what follows is measured from rest, not from the drop.
        self.carried = if grounded && !was_grounded {
            0.0
        } else {
            dy - absorbed
        };
        self.last_dt = dt;
        // The jump is taken whole this frame and eased out from the next.
        self.offset =
            (self.offset * (-SHOWN_RATE * dt).exp() - absorbed).clamp(-0.9 * radius, 0.9 * radius);
        if self.offset.abs() < 1e-4 {
            self.offset = 0.0;
        }
        self.offset
    }

    /// The drawn centre.
    pub(super) fn at(&self, ball: &BallState) -> Vec3 {
        ball.pos + Vec3::Y * self.offset
    }
}

/// After every system that may move the ball this frame: the drawn root follows the physics with
/// the vertical blend. Outside play the blend is cleared and the root is left to its owners.
pub(super) fn present(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    ball: Res<BallState>,
    mut shown: ResMut<Shown>,
    mut roots: Query<&mut Transform, (With<Ball>, Without<BallSpin>)>,
) {
    if *phase.get() != Phase::Playing {
        *shown = default();
        return;
    }
    let offset = shown.step(
        ball.pos,
        ball.vel.y,
        ball.radius,
        ball.grounded,
        time.delta_secs(),
    );
    if offset != 0.0 {
        if let Ok(mut t) = roots.single_mut() {
            t.translation = shown.at(&ball);
        }
    }
}

/// The terrain streams around the ball in every phase, and the loading screen waits on the ground
/// under it: published to `Player`, which the stock streaming and loading reads follow.
pub(super) fn publish_focus(ball: Res<BallState>, mut player: ResMut<crate::player::Player>) {
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = ball.pos;
}

/// Let go of the mouse buttons when the page loses focus or the pointer leaves it. A release that
/// lands elsewhere (alt-tab mid-drag, a browser or OS popup) never reaches the game, so the button
/// would read as held and the camera would swing with every move until both buttons were clicked
/// again.
fn drop_stray_buttons(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    mut left: MessageReader<bevy::window::CursorLeft>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    let lost = focus.read().any(|f| !f.focused);
    if lost | (left.read().count() > 0) {
        mouse.release_all();
    }
}

#[cfg(test)]
mod handling_tests {
    use super::*;

    /// Feed a path at 60 Hz; an airborne frame's vertical speed is its own motion (ballistic).
    fn roll(shown: &mut Shown, path: &[(Vec3, bool)], r: f32) -> Vec<f32> {
        let dt = 1.0 / 60.0;
        let mut last: Option<f32> = None;
        path.iter()
            .map(|&(p, g)| {
                let vy = if g {
                    0.0
                } else {
                    last.map_or(0.0, |y| (p.y - y) / dt)
                };
                last = Some(p.y);
                p.y + shown.step(p, vy, r, g, dt)
            })
            .collect()
    }

    #[test]
    fn an_edge_climb_follows_the_sphere_contact_circle() {
        // A ledge of 0.6 r: ground for x < 0, the top beyond. Fixed rings alone were off by up to
        // 0.2 r, rising in jumps whenever the next ring crossed the edge.
        for r in [0.5_f32, 3.4, 25.0] {
            let top = 0.6 * r;
            let mut worst = 0.0_f32;
            for step in 0..=200 {
                let x = -r + step as f32 * r / 200.0;
                let rest = ring_support(r, |o| {
                    Some((if x + o.x >= 0.0 { top } else { 0.0 }, Vec3::Y, ()))
                })
                .unwrap()
                .0;
                let truth = r.max(top + (r * r - x * x).max(0.0).sqrt());
                assert!(
                    rest <= truth + 1e-4,
                    "r={r} x={x}: {rest} above the true contact {truth}"
                );
                worst = worst.max(truth - rest);
            }
            assert!(
                worst < 0.03 * r,
                "r={r}: support lags the contact circle by {worst}"
            );
        }
    }

    #[test]
    fn a_slope_is_not_mistaken_for_an_edge() {
        let mut samples = 0;
        let rest = ring_support(2.0, |o| {
            samples += 1;
            let n = Vec3::new(-0.5, 1.0, 0.0).normalize();
            Some((0.5 * o.x, n, ()))
        });
        assert_eq!(samples, 73, "no bisection on a plane");
        assert!(rest.is_some());
    }

    #[test]
    fn a_step_pop_is_eased_out_and_the_drawn_ball_never_jumps() {
        let r = 4.0;
        let mut path: Vec<_> = (0..10)
            .map(|i| (Vec3::new(i as f32 * 0.3, r, 0.), true))
            .collect();
        // A 0.5r rise in one frame (an instant step-up), then flat on top.
        path.extend((10..40).map(|i| (Vec3::new(i as f32 * 0.3, r * 1.5, 0.), true)));
        let mut shown = Shown::default();
        let drawn = roll(&mut shown, &path, r);
        let worst = drawn
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0., f32::max);
        assert!(worst < 0.25 * r, "a drawn jump of {worst} yd");
        assert!(
            (drawn[39] - r * 1.5).abs() < 0.05 * r,
            "settled at {}",
            drawn[39]
        );
        assert!(
            drawn.iter().all(|y| *y <= r * 1.5 + 1e-4),
            "never overshoots the physics"
        );
    }

    #[test]
    fn a_steady_slope_and_a_landing_are_drawn_as_they_are() {
        let r = 2.0;
        let slope: Vec<_> = (0..30)
            .map(|i| (Vec3::new(i as f32 * 0.3, r + i as f32 * 0.15, 0.), true))
            .collect();
        let mut shown = Shown::default();
        let drawn = roll(&mut shown, &slope, r);
        assert!(drawn
            .iter()
            .zip(&slope)
            .all(|(d, (p, _))| (d - p.y).abs() < 1e-4));
        // Falling, then landing: no blend below the ground.
        let mut shown = Shown::default();
        let fall: Vec<_> = (0..10)
            .map(|i| (Vec3::new(0., 10. - i as f32 * 0.5, 0.), false))
            .chain((0..5).map(|_| (Vec3::new(0., 5., 0.), true)))
            .collect();
        let drawn = roll(&mut shown, &fall, r);
        assert!(drawn
            .iter()
            .zip(&fall)
            .all(|(d, (p, _))| (d - p.y).abs() < 1e-4));
    }

    #[test]
    fn landing_on_a_higher_surface_is_eased_too() {
        let r = 50.0;
        let mut shown = Shown::default();
        let path = [
            (Vec3::new(0., 170., 0.), true),
            (Vec3::new(0.3, 169.99, 0.), false),
            (Vec3::new(0.6, 169.97, 0.), false),
            (Vec3::new(0.9, 184., 0.), true),
            (Vec3::new(1.2, 184., 0.), true),
        ];
        let drawn = roll(&mut shown, &path, r);
        assert!(drawn[3] - drawn[2] < 0.05 * r, "drawn {drawn:?}");
    }

    #[test]
    fn an_airborne_snap_is_eased_but_a_bounce_is_not() {
        let r = 27.0;
        let dt = 1.0 / 60.0;
        // Falling, then the deck under the ball rises 6.5 yd while it is still in the air.
        let mut shown = Shown::default();
        let mut drawn = vec![];
        for (y, vy) in [(133.5, -6.0), (133.4, -6.3), (133.29, -6.6), (139.8, 4.0)] {
            drawn.push(y + shown.step(Vec3::new(0., y, 0.), vy, r, false, dt));
        }
        assert!(drawn[3] - drawn[2] < 0.05 * r, "drawn {drawn:?}");
        // A ballistic rise after a bounce is drawn as it is.
        let mut shown = Shown::default();
        for (i, y) in [100.0_f32, 100.25, 100.5, 100.74].into_iter().enumerate() {
            let offset = shown.step(Vec3::new(0., y, 0.), 15.0 - i as f32 * 0.37, 1.0, false, dt);
            assert!(offset.abs() < 1e-3, "bounce frame {i} offset {offset}");
        }
        // A settle that drops a falling ball onto the floor far below its fall is eased.
        let mut shown = Shown::default();
        let mut drawn = vec![];
        for (y, vy, g) in [
            (147.8, -6.0, false),
            (147.7, -6.0, false),
            (137.9, 0.0, true),
        ] {
            drawn.push(y + shown.step(Vec3::new(0., y, 0.), vy, r, g, dt));
        }
        assert!(drawn[1] - drawn[2] < 0.05 * r, "drawn {drawn:?}");
    }

    #[test]
    fn a_teleport_resets_the_blend() {
        let r = 3.0;
        let mut shown = Shown::default();
        let flat = [(Vec3::new(0., r, 0.), true), (Vec3::new(0.1, r, 0.), true)];
        roll(&mut shown, &flat, r);
        roll(&mut shown, &[(Vec3::new(0.2, 2. * r, 0.), true)], r);
        assert!(shown.offset < 0.);
        roll(&mut shown, &[(Vec3::new(200., r, 0.), true)], r);
        assert_eq!(shown.offset, 0.);
    }

    #[test]
    fn eaten_platform_releases_a_grounded_ball_and_settles_on_floor() {
        let (mut y, mut vy, mut grounded) = (1., 0., true);
        // An edible/Shed table is excluded, leaving the floor's full-radius rest.
        (y, vy, grounded) = settle_vertical(y, vy, grounded, 0.3, 0.05, 0.016);
        assert!(!grounded);
        assert!(y < 1. && vy < 0.);
        for _ in 0..150 {
            (y, vy, grounded) = settle_vertical(y, vy, grounded, 0.3, 0.05, 0.016);
            assert!(y >= 0.3, "cannot fall through the floor");
        }
        assert_eq!((y, vy, grounded), (0.3, 0., true));
    }
    #[test]
    fn building_floor_supports_a_ball_over_a_terrain_hole() {
        use avian3d::prelude::{PhysicsPlugins, RigidBody};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<benilla_world::collision::MoverTraceExclusions>();
        app.add_plugins((
            MinimalPlugins,
            bevy::transform::TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            PhysicsPlugins::new(PostUpdate),
        ));
        app.init_asset::<Mesh>();
        // The inn's lower floor is 56.9496, its first raised tread 57.3849 (0.4353 yd).
        // Shift to the origin, extrude the two floors and their front-facing riser.
        let profile = [(-2.0, 0.0), (0.0, 0.0), (0.0, 0.4353), (2.0, 0.4353)];
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for pair in profile.windows(2) {
            let [(x0, y0), (x1, y1)] = [pair[0], pair[1]];
            let b = vertices.len() as u32;
            vertices.extend([
                Vec3::new(x0, y0, -2.0),
                Vec3::new(x1, y1, -2.0),
                Vec3::new(x1, y1, 2.0),
                Vec3::new(x0, y0, 2.0),
            ]);
            triangles.extend([[b, b + 2, b + 1], [b, b + 3, b + 2]]);
        }
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::trimesh(vertices, triangles),
            Transform::default(),
        ));
        benilla_world::world_point::init_world_point_resources(app.world_mut());
        app.update();
        app.world_mut()
            .run_system_once(|world: WorldPoint, collision: WorldCollision| {
                assert!(
                    floor_rest(&collision, Vec3::new(-1., 0., 0.), 0.5).is_none(),
                    "feet mistaken for centre misses the floor"
                );
                for r in [0.25, 0.3, 0.5, 0.6, 1.2] {
                    for (x, floor) in [(-1.0, 0.0), (1.0, 0.4353)] {
                        let at = Vec3::new(x, ready_height(floor, floor, r), 0.0);
                        assert!(world.terrain_height_under(at).is_none());
                        let (support, slope) =
                            footing(&world, &collision, at, r).expect("WMO floor over an ADT hole");
                        assert!((support - floor - r).abs() < 1e-4);
                        assert_eq!(slope, Vec2::ZERO);
                    }
                }
                assert!(footing(&world, &collision, Vec3::new(20.0, 1.0, 0.0), 0.3).is_none());
            })
            .unwrap();
    }

    #[test]
    fn handling_envelope() {
        // Balanced clump, level ground, fixed 120 Hz. Uses the shipping traction function;
        // world fixtures separately exercise floor/wall integration.
        let dt = 1.0 / 120.0;
        for radius in [0.3, 1.0, 3.0, 8.0] {
            let top = top_speed(radius);
            let accel = top / (REACH_BASE + REACH_PER_RADIUS * radius);
            let mut velocity = Vec3::ZERO;
            let mut t90 = 0.0;
            for frame in 1..1200 {
                velocity = traction(velocity, Vec3::NEG_Z, top, accel, 1.0, true, dt);
                if velocity.length() >= top * 0.9 {
                    t90 = frame as f32 * dt;
                    break;
                }
            }
            assert!(t90 > 0.0 && t90 < 2.0);
            let mut reverse = 0.0;
            velocity = Vec3::NEG_Z * top;
            for frame in 1..1200 {
                velocity = traction(velocity, Vec3::Z, top, accel, 1.0, true, dt);
                if velocity.z >= 0.9 * top {
                    reverse = frame as f32 * dt;
                    break;
                }
            }
            assert!(reverse > t90);
            let mut coast = 0.0;
            velocity = Vec3::NEG_Z * top;
            for frame in 1..1200 {
                velocity = traction(velocity, Vec3::ZERO, top, accel, 1.0, true, dt);
                if velocity.length() < 0.01 {
                    coast = frame as f32 * dt;
                    break;
                }
            }
            velocity = Vec3::NEG_Z * top;
            let mut position = Vec3::ZERO;
            let mut heading = 0.0_f32;
            for _ in 0..2400 {
                heading += turn_rate(radius, 0.25) * dt;
                let wish = Vec3::new(-heading.sin(), 0.0, -heading.cos());
                velocity = traction(velocity, wish, top, accel, 1.0, true, dt);
                position += velocity * dt;
            }
            let circle = velocity.length() / turn_rate(radius, 0.25);
            println!(
                "handling r={radius:.1} top={top:.3} turn_deg={:.2} t90={t90:.3} reverse90={reverse:.3} coast={coast:.3} steady_turn_radius={circle:.3} turn_speed={:.3} dash_top={:.3}",
                turn_rate(radius, 0.25).to_degrees(),
                velocity.length(),
                top * SPRINT
            );
            assert!(position.is_finite() && circle > radius * 0.5);
        }
    }
}
